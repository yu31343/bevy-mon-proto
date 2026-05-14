#!/usr/bin/env python3
import argparse
import secrets
import selectors
import socket
import threading
import time
from dataclasses import dataclass
from queue import Queue, Empty

DEFAULT_HOST = "0.0.0.0"
DEFAULT_PORT = 42043
RELAY_PROTOCOL_VERSION = "1"
MAX_FRAME_LEN = 64 * 1024
MAX_WAITING_ROOMS = 1024
ROOM_WAIT_TIMEOUT = 10 * 60
ROOM_CODE_LEN = 4
ROOM_CODE_CHARS = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"


@dataclass
class WaitingRoom:
    client_queue: Queue
    created_at: float


rooms = {}
rooms_lock = threading.Lock()


def main():
    parser = argparse.ArgumentParser(description="bevy-mon-proto TCP relay server")
    parser.add_argument("--host", default=DEFAULT_HOST)
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    args = parser.parse_args()

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.bind((args.host, args.port))
        listener.listen()
        print(f"relay listening on {args.host}:{args.port}", flush=True)
        while True:
            conn, addr = listener.accept()
            thread = threading.Thread(target=handle_connection, args=(conn, addr), daemon=True)
            thread.start()


def handle_connection(conn, addr):
    try:
        line = read_control_line(conn)
        parts = line.split(" ", 3)
        if len(parts) < 3 or parts[0] != "RELAY" or parts[1] != RELAY_PROTOCOL_VERSION:
            write_control_line(conn, "ERR 中继协议版本不一致")
            return
        command = parts[2]
        if command == "CREATE":
            handle_create_room(conn, addr)
        elif command == "JOIN" and len(parts) == 4:
            handle_join_room(conn, normalize_room_code(parts[3]), addr)
        else:
            write_control_line(conn, "ERR 无效中继命令")
    except Exception as exc:
        print(f"connection {addr} failed: {exc}", flush=True)
        close_socket(conn)


def handle_create_room(host_conn, addr):
    client_queue = Queue(maxsize=1)
    room_code = insert_waiting_room(client_queue)
    if room_code is None:
        write_control_line(host_conn, "ERR 等待房间数量已满")
        close_socket(host_conn)
        return

    print(f"room {room_code} created by {addr}", flush=True)
    try:
        write_control_line(host_conn, f"ROOM {room_code}")
        write_control_line(host_conn, f"WAIT {room_code}")
        client_conn = wait_for_client_or_host_close(host_conn, client_queue)
    except Exception as exc:
        remove_room(room_code)
        try:
            write_control_line(host_conn, f"ERR {exc}")
        except OSError:
            pass
        close_socket(host_conn)
        print(f"room {room_code} closed before peer joined: {exc}", flush=True)
        return

    try:
        write_control_line(host_conn, "PEER")
        write_control_line(client_conn, "PEER")
    except OSError as exc:
        close_socket(host_conn)
        close_socket(client_conn)
        print(f"room {room_code} peer notify failed: {exc}", flush=True)
        return

    print(f"room {room_code} bridged", flush=True)
    bridge(room_code, host_conn, client_conn)


def handle_join_room(client_conn, room_code, addr):
    with rooms_lock:
        clean_expired_rooms_locked()
        room = rooms.pop(room_code, None)
    if room is None:
        write_control_line(client_conn, "ERR 房间不存在或已关闭")
        close_socket(client_conn)
        return
    try:
        room.client_queue.put_nowait(client_conn)
        print(f"peer {addr} joined room {room_code}", flush=True)
    except Exception:
        write_control_line(client_conn, "ERR 房间已关闭")
        close_socket(client_conn)


def insert_waiting_room(client_queue):
    with rooms_lock:
        clean_expired_rooms_locked()
        if len(rooms) >= MAX_WAITING_ROOMS:
            return None
        for _ in range(128):
            room_code = generate_room_code()
            if room_code not in rooms:
                rooms[room_code] = WaitingRoom(client_queue=client_queue, created_at=time.monotonic())
                return room_code
    return None


def remove_room(room_code):
    with rooms_lock:
        rooms.pop(room_code, None)


def clean_expired_rooms_locked():
    now = time.monotonic()
    expired = [code for code, room in rooms.items() if now - room.created_at >= ROOM_WAIT_TIMEOUT]
    for code in expired:
        rooms.pop(code, None)


def wait_for_client_or_host_close(host_conn, client_queue):
    deadline = time.monotonic() + ROOM_WAIT_TIMEOUT
    host_conn.setblocking(False)
    selector = selectors.DefaultSelector()
    selector.register(host_conn, selectors.EVENT_READ)
    try:
        while time.monotonic() < deadline:
            try:
                return client_queue.get_nowait()
            except Empty:
                pass
            events = selector.select(timeout=0.1)
            if events:
                data = host_conn.recv(1, socket.MSG_PEEK)
                if not data:
                    raise RuntimeError("建房方已断开，房间已关闭")
                raise RuntimeError("建房方在配对前发送了无效数据")
        raise RuntimeError("等待对手超时，房间已关闭")
    finally:
        selector.unregister(host_conn)
        host_conn.setblocking(True)


def bridge(room_code, host_conn, client_conn):
    stop_event = threading.Event()
    first = threading.Thread(
        target=copy_frames,
        args=(room_code, "host", host_conn, client_conn, stop_event),
        daemon=True,
    )
    second = threading.Thread(
        target=copy_frames,
        args=(room_code, "client", client_conn, host_conn, stop_event),
        daemon=True,
    )
    first.start()
    second.start()
    first.join()
    second.join()
    close_socket(host_conn)
    close_socket(client_conn)
    print(f"room {room_code} closed", flush=True)


def copy_frames(room_code, source_name, reader, writer, stop_event):
    try:
        while not stop_event.is_set():
            header = recv_exact(reader, 4)
            if header is None:
                return
            frame_len = int.from_bytes(header, "big")
            if frame_len > MAX_FRAME_LEN:
                print(f"room {room_code} {source_name} sent oversized frame", flush=True)
                return
            payload = recv_exact(reader, frame_len)
            if payload is None:
                return
            writer.sendall(header + payload)
    except OSError as exc:
        print(f"room {room_code} {source_name} relay ended: {exc}", flush=True)
    finally:
        stop_event.set()
        close_socket(writer)


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


def read_control_line(conn):
    data = bytearray()
    while True:
        chunk = conn.recv(1)
        if not chunk:
            raise RuntimeError("连接已关闭")
        if chunk == b"\n":
            return data.decode("utf-8").strip()
        data.extend(chunk)
        if len(data) > 1024:
            raise RuntimeError("控制消息过长")


def write_control_line(conn, line):
    conn.sendall((f"RELAY {line}\n").encode("utf-8"))


def normalize_room_code(room_code):
    return room_code.strip().upper()


def generate_room_code():
    return "".join(secrets.choice(ROOM_CODE_CHARS) for _ in range(ROOM_CODE_LEN))


def close_socket(conn):
    try:
        conn.shutdown(socket.SHUT_RDWR)
    except OSError:
        pass
    try:
        conn.close()
    except OSError:
        pass


if __name__ == "__main__":
    main()
