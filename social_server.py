#!/usr/bin/env python3
"""社交服务器：内存存储，支持注册/登录/好友/聊天/对战邀请。"""
import json
import socket
import struct
import threading
import time

DEFAULT_HOST = "0.0.0.0"
DEFAULT_PORT = 42044
DEFAULT_RELAY_PORT = 42043
MAX_FRAME_LEN = 64 * 1024

# 内存存储
next_user_id = 1
users_lock = threading.Lock()
users = {}  # user_id(str) -> {"username", "password", "friends": set(), "pending_from": set(), "last_read": {}}
username_index = {}  # username(lower) -> user_id
messages = {}  # f"{min_id}_{max_id}" -> [{"from", "text", "ts"}]
messages_lock = threading.Lock()
online_sessions = {}  # user_id(str) -> active client socket
online_lock = threading.Lock()
relay_advertise_host = DEFAULT_HOST
relay_advertise_port = DEFAULT_RELAY_PORT


def main():
    import argparse
    parser = argparse.ArgumentParser(description="bevy-mon-proto social server")
    parser.add_argument("--host", default=DEFAULT_HOST)
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    parser.add_argument(
        "--no-relay",
        action="store_true",
        help="不随社交服自动启动 PVP relay server",
    )
    parser.add_argument(
        "--relay-bind-host",
        default=None,
        help="relay 监听地址；默认继承 --host",
    )
    parser.add_argument(
        "--relay-host",
        default=None,
        help="下发给客户端的 relay 地址主机；默认继承 --host，0.0.0.0 时客户端会继承社交服地址",
    )
    parser.add_argument("--relay-port", type=int, default=DEFAULT_RELAY_PORT)
    args = parser.parse_args()

    global relay_advertise_host, relay_advertise_port
    relay_advertise_host = args.relay_host or args.host
    relay_advertise_port = args.relay_port

    if not args.no_relay:
        start_relay_server(args.relay_bind_host or args.host, args.relay_port)

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.bind((args.host, args.port))
        listener.listen()
        print(f"social server listening on {args.host}:{args.port}", flush=True)
        print(f"social server advertises relay {relay_address()}", flush=True)
        while True:
            conn, addr = listener.accept()
            t = threading.Thread(target=handle_client, args=(conn, addr), daemon=True)
            t.start()


def start_relay_server(host, port):
    def run():
        try:
            import relay_server
            relay_server.serve(host, port)
        except Exception as exc:
            print(f"[relay error] {exc}", flush=True)

    t = threading.Thread(target=run, daemon=True)
    t.start()
    return t


def relay_address():
    return f"{relay_advertise_host}:{relay_advertise_port}"


def with_relay_config(resp):
    resp["relay_address"] = relay_address()
    return resp


def handle_client(conn, addr):
    print(f"[connected] {addr}", flush=True)
    session_user_id = None
    try:
        while True:
            msg = recv_message(conn)
            if msg is None:
                print(f"[disconnect] {addr} (no message)", flush=True)
                break
            req = json.loads(msg.decode("utf-8"))
            print(f"[recv] {addr}: {req.get('type', '?')} | {json.dumps(req, ensure_ascii=False)}", flush=True)
            resp = handle_request(req, session_user_id)
            if "session_user_id" in resp:
                next_session_user_id = resp["session_user_id"]
                if next_session_user_id is not None:
                    next_session_user_id = str(next_session_user_id)
                    with online_lock:
                        if session_user_id is not None and session_user_id != next_session_user_id:
                            if online_sessions.get(session_user_id) is conn:
                                online_sessions.pop(session_user_id, None)
                        online_sessions[next_session_user_id] = conn
                    session_user_id = next_session_user_id
            send_message(conn, json.dumps(resp).encode("utf-8"))
            print(f"[sent] {addr}: {resp.get('type', '?')}", flush=True)
    except Exception as exc:
        print(f"[error] {addr}: {exc}", flush=True)
    finally:
        if session_user_id is not None:
            with online_lock:
                if online_sessions.get(session_user_id) is conn:
                    online_sessions.pop(session_user_id, None)
        conn.close()


def handle_request(req, session_user_id):
    t = req.get("type", "")
    if t == "register":
        return do_register(req.get("username", ""), req.get("password", ""))
    if t == "login":
        return do_login(req.get("username", ""), req.get("password", ""), session_user_id)
    if session_user_id is None:
        return {"type": "error", "message": "未登录"}
    if t == "get_profile":
        return do_get_profile(session_user_id)
    if t == "update_username":
        return do_update_username(session_user_id, req.get("new_name", ""))
    if t == "search_users":
        return do_search_users(session_user_id, req.get("query", ""))
    if t == "add_friend":
        return do_add_friend(session_user_id, req.get("user_id", ""))
    if t == "accept_friend":
        return do_accept_friend(session_user_id, req.get("user_id", ""))
    if t == "get_friends":
        return do_get_friends(session_user_id)
    if t == "get_pending_requests":
        return do_get_pending_requests(session_user_id)
    if t == "send_message":
        return do_send_message(session_user_id, req.get("to_user_id", ""), req.get("text", ""))
    if t == "get_messages":
        return do_get_messages(
            session_user_id,
            req.get("from_user_id", req.get("user_id", "")),
            bool(req.get("mark_read", True)),
        )
    if t == "get_updates":
        return do_get_updates(session_user_id)
    return {"type": "error", "message": f"未知命令: {t}"}


def do_register(username, password):
    global next_user_id
    username = username.strip()
    if not username or not password:
        return {"type": "error", "message": "用户名和密码不能为空"}
    with users_lock:
        key = username.lower()
        if key in username_index:
            return {"type": "error", "message": "用户名已存在，请直接登录或换一个用户名"}
        uid = str(next_user_id)
        next_user_id += 1
        users[uid] = {
            "username": username,
            "password": password,
            "friends": set(),
            "pending_from": set(),
            "last_read": {},
        }
        username_index[key] = uid
    print(f"register: {username} -> id={uid}", flush=True)
    return with_relay_config({"type": "register_ok", "user_id": uid, "username": username, "session_user_id": uid})


def do_login(username, password, current_session_user_id=None):
    with users_lock:
        key = username.strip().lower()
        uid = username_index.get(key)
        if uid is None:
            return {"type": "error", "message": "用户不存在，请先注册账号"}
        u = users[uid]
        if u["password"] != password:
            return {"type": "error", "message": "密码错误"}
    with online_lock:
        existing_conn = online_sessions.get(uid)
        if existing_conn is not None and uid != current_session_user_id:
            return {"type": "error", "message": "该账号已在其他客户端登录，请先退出后再登录"}
    print(f"login: {username} -> id={uid}", flush=True)
    return with_relay_config({"type": "login_ok", "user_id": uid, "username": u["username"], "session_user_id": uid})


def do_get_profile(uid):
    with users_lock:
        u = users.get(uid)
        if u is None:
            return {"type": "error", "message": "用户不存在"}
        return with_relay_config({"type": "profile", "user_id": uid, "username": u["username"]})


def do_update_username(uid, new_name):
    new_name = new_name.strip()
    if not new_name:
        return {"type": "error", "message": "用户名不能为空"}
    with users_lock:
        key = new_name.lower()
        if key in username_index and username_index[key] != uid:
            return {"type": "error", "message": "用户名已被占用"}
        u = users.get(uid)
        if u is None:
            return {"type": "error", "message": "用户不存在"}
        old_key = u["username"].lower()
        username_index.pop(old_key, None)
        u["username"] = new_name
        username_index[key] = uid
    return {"type": "username_updated", "username": new_name}


def do_search_users(uid, query):
    query = query.strip().lower()
    results = []
    with users_lock:
        for other_uid, u in users.items():
            if other_uid == uid:
                continue
            if query and query in u["username"].lower():
                is_friend = other_uid in users[uid]["friends"]
                results.append({"user_id": other_uid, "username": u["username"], "is_friend": is_friend})
    return {"type": "search_results", "users": results}


def do_add_friend(uid, target_id):
    with users_lock:
        u = users.get(uid)
        target = users.get(target_id)
        if u is None or target is None:
            return {"type": "error", "message": "用户不存在"}
        if target_id == uid:
            return {"type": "error", "message": "不能添加自己"}
        if target_id in u["friends"]:
            return {"type": "error", "message": "已经是好友了"}
        target["pending_from"].add(uid)
    return {"type": "friend_request_sent"}


def do_accept_friend(uid, target_id):
    with users_lock:
        u = users.get(uid)
        target = users.get(target_id)
        if u is None or target is None:
            return {"type": "error", "message": "用户不存在"}
        if target_id not in u["pending_from"]:
            return {"type": "error", "message": "没有该好友请求"}
        u["pending_from"].discard(target_id)
        u["friends"].add(target_id)
        target["friends"].add(uid)
    return {"type": "friend_accepted", "user_id": target_id}


def do_get_friends(uid):
    with users_lock:
        u = users.get(uid)
        if u is None:
            return {"type": "error", "message": "用户不存在"}
        friends = []
        for fid in u["friends"]:
            fu = users.get(fid)
            if fu:
                friends.append({"user_id": fid, "username": fu["username"]})
    return {"type": "friends_list", "friends": friends}


def do_get_pending_requests(uid):
    with users_lock:
        u = users.get(uid)
        if u is None:
            return {"type": "error", "message": "用户不存在"}
        requests = []
        for from_id in u["pending_from"]:
            fu = users.get(from_id)
            if fu:
                requests.append({"user_id": from_id, "username": fu["username"]})
    return {"type": "pending_requests", "requests": requests}


def do_send_message(uid, to_id, text):
    if not text.strip():
        return {"type": "error", "message": "消息不能为空"}
    with users_lock:
        u = users.get(uid)
        if u is None:
            return {"type": "error", "message": "用户不存在"}
        if to_id not in u["friends"]:
            return {"type": "error", "message": "只能给好友发消息"}
    key = msg_key(uid, to_id)
    with messages_lock:
        messages.setdefault(key, []).append({
            "from": uid,
            "text": text,
            "ts": time.time(),
        })
    print(f"[msg] {uid}->{to_id} key={key} text={text!r}", flush=True)
    return {"type": "message_sent"}


def do_get_messages(uid, from_id, mark_read=True):
    key = msg_key(uid, from_id)
    with messages_lock:
        msgs = messages.get(key, [])
        result = [{"from": m["from"], "text": m["text"], "ts": m["ts"]} for m in msgs]
    if mark_read:
        with users_lock:
            u = users.get(uid)
            if u is not None:
                u.setdefault("last_read", {})[from_id] = time.time()
    return {"type": "messages", "from_user_id": from_id, "messages": result}


def do_get_updates(uid):
    with users_lock:
        u = users.get(uid)
        if u is None:
            return {"type": "error", "message": "用户不存在"}
        requests = [
            {"user_id": fid, "username": users[fid]["username"]}
            for fid in u["pending_from"] if fid in users
        ]
        last_read = u.get("last_read", {})
        friends = []
        for fid in u["friends"]:
            if fid not in users:
                continue
            # 计算未读消息数
            key = msg_key(uid, fid)
            with messages_lock:
                msgs = messages.get(key, [])
                lr = last_read.get(fid, 0)
                unread = sum(1 for m in msgs if m["from"] != uid and m["ts"] > lr)
            with online_lock:
                is_online = fid in online_sessions
            friends.append({
                "user_id": fid,
                "username": users[fid]["username"],
                "unread": unread,
                "online": is_online,
            })
    return with_relay_config({"type": "updates", "pending_requests": requests, "friends": friends})


def msg_key(a, b):
    lo, hi = sorted([a, b])
    return f"{lo}_{hi}"


def recv_message(conn):
    header = recv_exact(conn, 4)
    if header is None:
        return None
    length = struct.unpack(">I", header)[0]
    if length > MAX_FRAME_LEN:
        return None
    return recv_exact(conn, length)


def recv_exact(conn, size):
    chunks = []
    remaining = size
    while remaining > 0:
        chunk = conn.recv(remaining)
        if not chunk:
            return None
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def send_message(conn, data):
    conn.sendall(struct.pack(">I", len(data)) + data)


if __name__ == "__main__":
    main()
