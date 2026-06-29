use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{mpsc, Mutex},
    thread,
    time::Duration,
};

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use serde::{Deserialize, Serialize};

use crate::{
    game_state::GameState,
    pvp::{self, PvpConnection, PvpStatus},
    team_selection::SelectionEntryMode,
};

const SOCIAL_PORT: u16 = 42044;
const MAX_FRAME_LEN: usize = 64 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub struct OnlinePlugin;

impl Plugin for OnlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OnlineConnection>()
            .init_resource::<OnlineLoginInput>()
            .init_resource::<OnlineHomeState>()
            .init_resource::<OnlineChatState>()
            .init_resource::<OnlineBattleInvite>()
            .init_resource::<EguiFontRegistered>()
            .add_systems(OnExit(GameState::OnlineLogin), cleanup_online_login)
            .add_systems(OnExit(GameState::OnlineHome), cleanup_online_home)
            .add_systems(OnExit(GameState::OnlineChat), cleanup_online_chat)
            .add_systems(
                Update,
                online_poll_system.run_if(
                    in_state(GameState::OnlineLogin)
                        .or(in_state(GameState::OnlineHome))
                        .or(in_state(GameState::OnlineChat)),
                ),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (
                    online_login_egui_system.run_if(in_state(GameState::OnlineLogin)),
                    online_home_egui_system.run_if(in_state(GameState::OnlineHome)),
                    online_chat_egui_system.run_if(in_state(GameState::OnlineChat)),
                ),
            );
    }
}

// ==================== 连接管理 ====================

#[derive(Resource, Default)]
pub struct OnlineConnection {
    command_tx: Option<mpsc::Sender<OnlineCommand>>,
    event_rx: Option<Mutex<mpsc::Receiver<OnlineEvent>>>,
    pub user_id: Option<String>,
    pub username: String,
}

#[derive(Debug)]
enum OnlineCommand {
    Send(String),
    Stop,
}

#[derive(Debug, Clone)]
enum OnlineEvent {
    Message(String),
    Failed(String),
    Disconnected,
}

impl OnlineConnection {
    pub fn connect(&mut self, address: &str) {
        self.stop();
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        let addr = address.to_string();
        thread::spawn(move || online_thread(addr, cmd_rx, evt_tx));
        self.command_tx = Some(cmd_tx);
        self.event_rx = Some(Mutex::new(evt_rx));
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.command_tx.take() {
            let _ = tx.send(OnlineCommand::Stop);
        }
        self.event_rx = None;
    }

    pub fn is_connected(&self) -> bool {
        self.command_tx.is_some()
    }

    pub fn send(&self, json: String) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(OnlineCommand::Send(json));
        }
    }
}

fn online_thread(
    address: String,
    cmd_rx: mpsc::Receiver<OnlineCommand>,
    evt_tx: mpsc::Sender<OnlineEvent>,
) {
    let mut stream = match TcpStream::connect(&address) {
        Ok(s) => s,
        Err(e) => {
            let _ = evt_tx.send(OnlineEvent::Failed(format!("连接失败：{e}")));
            return;
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));

    loop {
        // 处理发送命令
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                OnlineCommand::Send(json) => {
                    let data = json.into_bytes();
                    let len = (data.len() as u32).to_be_bytes();
                    if stream.write_all(&len).is_err() || stream.write_all(&data).is_err() {
                        let _ = evt_tx.send(OnlineEvent::Disconnected);
                        return;
                    }
                }
                OnlineCommand::Stop => return,
            }
        }

        // 尝试接收消息（阻塞式 read，靠 50ms 超时退出）
        let mut header = [0u8; 4];
        match stream.read_exact(&mut header) {
            Ok(()) => {
                let len = u32::from_be_bytes(header) as usize;
                if len == 0 || len > MAX_FRAME_LEN {
                    let _ = evt_tx.send(OnlineEvent::Failed("消息格式错误".into()));
                    return;
                }
                let mut payload = vec![0u8; len];
                match stream.read_exact(&mut payload) {
                    Ok(()) => {
                        let text = String::from_utf8_lossy(&payload).into_owned();
                        let _ = evt_tx.send(OnlineEvent::Message(text));
                    }
                    Err(_) => {
                        let _ = evt_tx.send(OnlineEvent::Disconnected);
                        return;
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => {
                let _ = evt_tx.send(OnlineEvent::Disconnected);
                return;
            }
        }

        thread::sleep(POLL_INTERVAL);
    }
}

// ==================== 资源 ====================

#[derive(Resource, Default)]
struct OnlineLoginInput {
    username: String,
    password: String,
    server_address: String,
    info: String,
    is_register: bool,
    connecting: bool,
}

#[derive(Resource, Default)]
struct OnlineHomeState {
    search_query: String,
    search_results: Vec<SearchResultEntry>,
    friends: Vec<FriendEntry>,
    pending_requests: Vec<FriendEntry>,
    new_username: String,
    info: String,
    refresh_timer: f32,
    /// 轮询好友消息的索引
    msg_poll_index: usize,
    /// 待进入聊天的目标（用于从按钮点击传递到状态切换）
    pending_chat: Option<FriendEntry>,
}

#[derive(Resource, Default)]
struct OnlineChatState {
    target_user_id: String,
    target_username: String,
    messages: Vec<ChatEntry>,
    input: String,
    info: String,
    refresh_timer: f32,
}

/// 对战邀请状态
#[derive(Resource, Default)]
struct OnlineBattleInvite {
    /// 作为发起方，待发送的邀请目标 user_id
    pending_host_target: Option<String>,
    /// 作为发起方，已生成的房间码
    room_code: Option<String>,
    /// 作为发起方，是否正在等待对方同意
    waiting_for_accept: bool,
    /// 作为发起方，对方已同意（待进入选精灵）
    accepted_by_target: bool,
    /// 作为发起方，收到的拒绝通知
    reject_notification: Option<String>,
    /// 作为接收方，收到的对战邀请
    incoming: Option<IncomingInvite>,
    /// 已拒绝的房间码（防止重复弹出）
    rejected_room_codes: std::collections::HashSet<String>,
}

#[derive(Clone)]
struct IncomingInvite {
    from_user_id: String,
    from_username: String,
    room_code: String,
}

#[derive(Resource, Default)]
struct EguiFontRegistered(bool);

#[derive(Clone, Serialize, Deserialize)]
struct SearchUserResult {
    user_id: String,
    username: String,
    is_friend: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct FriendListEntry {
    user_id: String,
    username: String,
    #[serde(default)]
    unread: usize,
}

#[derive(Clone)]
struct ChatEntry {
    from_me: bool,
    text: String,
}

#[derive(Clone)]
struct FriendEntry {
    user_id: String,
    username: String,
    unread: usize,
}

#[derive(Clone)]
struct SearchResultEntry {
    user_id: String,
    username: String,
    is_friend: bool,
}

// ==================== 协议请求 ====================

fn send_request(conn: &OnlineConnection, req: &impl Serialize) {
    if let Ok(json) = serde_json::to_string(req) {
        conn.send(json);
    }
}

#[derive(Serialize)]
struct ReqRegister<'a> {
    #[serde(rename = "type")]
    req_type: &'a str,
    username: &'a str,
    password: &'a str,
}

#[derive(Serialize)]
struct ReqLogin<'a> {
    #[serde(rename = "type")]
    req_type: &'a str,
    username: &'a str,
    password: &'a str,
}

#[derive(Serialize)]
struct ReqSimple<'a> {
    #[serde(rename = "type")]
    req_type: &'a str,
}

#[derive(Serialize)]
struct ReqUpdateUsername<'a> {
    #[serde(rename = "type")]
    req_type: &'a str,
    new_name: &'a str,
}

#[derive(Serialize)]
struct ReqSearch<'a> {
    #[serde(rename = "type")]
    req_type: &'a str,
    query: &'a str,
}

#[derive(Serialize)]
struct ReqWithId<'a> {
    #[serde(rename = "type")]
    req_type: &'a str,
    user_id: &'a str,
}

#[derive(Serialize)]
struct ReqSendMessage<'a> {
    #[serde(rename = "type")]
    req_type: &'a str,
    to_user_id: &'a str,
    text: &'a str,
}

#[derive(Deserialize)]
struct ServerResponse {
    #[serde(rename = "type")]
    resp_type: String,
    user_id: Option<String>,
    username: Option<String>,
    message: Option<String>,
    users: Option<Vec<SearchUserResult>>,
    friends: Option<Vec<FriendListEntry>>,
    #[serde(alias = "pending_requests")]
    requests: Option<Vec<FriendListEntry>>,
    messages: Option<Vec<ServerMessage>>,
    from_user_id: Option<String>,
}

#[derive(Deserialize)]
struct ServerMessage {
    from: String,
    text: String,
    #[allow(dead_code)]
    ts: Option<f64>,
}

// ==================== 网络轮询 ====================

fn online_poll_system(
    time: Res<Time>,
    mut conn: ResMut<OnlineConnection>,
    mut login_input: ResMut<OnlineLoginInput>,
    mut home: ResMut<OnlineHomeState>,
    mut chat: ResMut<OnlineChatState>,
    mut invite: ResMut<OnlineBattleInvite>,
    pvp_conn: Res<PvpConnection>,
    current_state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // 消费网络事件
    let mut events = Vec::new();
    if let Some(rx) = &conn.event_rx {
        if let Ok(rx) = rx.lock() {
            while let Ok(evt) = rx.try_recv() {
                events.push(evt);
            }
        }
    }
    for evt in events {
        match evt {
            OnlineEvent::Message(json) => {
                handle_server_response(
                    &json,
                    &mut conn,
                    &mut login_input,
                    &mut home,
                    &mut chat,
                    &mut invite,
                    &mut next_state,
                );
            }
            OnlineEvent::Failed(msg) => {
                login_input.info = msg;
                login_input.connecting = false;
            }
            OnlineEvent::Disconnected => {
                login_input.info = "连接已断开".into();
                login_input.connecting = false;
            }
        }
    }

    // 发送对战邀请（发起方：当房间码就绪时）
    if let Some(target) = invite.pending_host_target.clone() {
        if let PvpStatus::WaitingRelayPeer { room_code } = &pvp_conn.status {
            if invite.room_code.as_deref() != Some(room_code.as_str()) {
                invite.room_code = Some(room_code.clone());
                invite.waiting_for_accept = true;
                let text = format!("BATTLE_INVITE:{room_code}");
                send_request(&conn, &ReqSendMessage {
                    req_type: "send_message",
                    to_user_id: &target,
                    text: &text,
                });
            }
        }
    }

    // 处理从主页点击聊天进入聊天状态
    if let Some(target) = home.pending_chat.take() {
        chat.target_user_id = target.user_id;
        chat.target_username = target.username;
        chat.messages.clear();
        chat.input.clear();
        chat.info.clear();
        chat.refresh_timer = 0.0;
        next_state.set(GameState::OnlineChat);
        return;
    }

    // 定期刷新
    match current_state.get() {
        GameState::OnlineHome => {
            home.refresh_timer += time.delta_secs();
            if home.refresh_timer > 2.0 {
                home.refresh_timer = 0.0;
                send_request(&conn, &ReqSimple { req_type: "get_updates" });
                // 轮询一个好友的消息（检测对战邀请）
                if !home.friends.is_empty() {
                    if home.msg_poll_index >= home.friends.len() {
                        home.msg_poll_index = 0;
                    }
                    let fid = home.friends[home.msg_poll_index].user_id.clone();
                    home.msg_poll_index += 1;
                    send_request(&conn, &ReqWithId { req_type: "get_messages", user_id: &fid });
                }
            }
        }
        GameState::OnlineChat => {
            if !chat.target_user_id.is_empty() {
                chat.refresh_timer += time.delta_secs();
                if chat.refresh_timer > 1.0 {
                    chat.refresh_timer = 0.0;
                    let from = chat.target_user_id.clone();
                    send_request(&conn, &ReqWithId { req_type: "get_messages", user_id: &from });
                }
            }
        }
        _ => {}
    }
}

fn handle_server_response(
    json: &str,
    conn: &mut OnlineConnection,
    login_input: &mut OnlineLoginInput,
    home: &mut OnlineHomeState,
    chat: &mut OnlineChatState,
    invite: &mut OnlineBattleInvite,
    next_state: &mut NextState<GameState>,
) {
    let Ok(mut resp) = serde_json::from_str::<ServerResponse>(json) else {
        return;
    };
    match resp.resp_type.as_str() {
        "register_ok" | "login_ok" => {
            conn.user_id = resp.user_id.clone();
            conn.username = resp.username.clone().unwrap_or_default();
            login_input.info = format!(
                "{}成功！欢迎，{}",
                if resp.resp_type == "register_ok" { "注册" } else { "登录" },
                conn.username
            );
            login_input.connecting = false;
            home.new_username = conn.username.clone();
            next_state.set(GameState::OnlineHome);
            send_request(conn, &ReqSimple { req_type: "get_updates" });
        }
        "profile" => {
            conn.username = resp.username.clone().unwrap_or_default();
        }
        "username_updated" => {
            conn.username = resp.username.clone().unwrap_or_default();
            home.new_username = conn.username.clone();
            home.info = "用户名修改成功".into();
        }
        "search_results" => {
            home.search_results = resp
                .users
                .unwrap_or_default()
                .into_iter()
                .map(|u| SearchResultEntry {
                    user_id: u.user_id,
                    username: u.username,
                    is_friend: u.is_friend,
                })
                .collect();
        }
        "friend_request_sent" => {
            home.info = "好友请求已发送".into();
        }
        "friend_accepted" => {
            home.info = "已添加好友".into();
            send_request(conn, &ReqSimple { req_type: "get_updates" });
        }
        "friends_list" | "updates" => {
            if let Some(friends) = resp.friends {
                home.friends = friends
                    .into_iter()
                    .map(|f| FriendEntry {
                        user_id: f.user_id,
                        username: f.username,
                        unread: f.unread,
                    })
                    .collect();
            }
            if let Some(requests) = resp.requests {
                home.pending_requests = requests
                    .into_iter()
                    .map(|r| FriendEntry {
                        user_id: r.user_id,
                        username: r.username,
                        unread: 0,
                    })
                    .collect();
            }
        }
        "pending_requests" => {
            home.pending_requests = resp
                .requests
                .unwrap_or_default()
                .into_iter()
                .map(|r| FriendEntry {
                    user_id: r.user_id,
                    username: r.username,
                    unread: 0,
                })
                .collect();
        }
        "message_sent" => {
            chat.input.clear();
            chat.refresh_timer = 0.0;
            chat.info = "发送成功，等待同步最新消息".into();
            let from = chat.target_user_id.clone();
            send_request(conn, &ReqWithId { req_type: "get_messages", user_id: &from });
        }
        "messages" => {
            let target = resp.from_user_id.clone().unwrap_or_else(|| chat.target_user_id.clone());
            let messages = resp.messages.take().unwrap_or_default();
            let my_id = conn.user_id.clone().unwrap_or_default();

            // 检查特殊消息（对战邀请/接受/拒绝）
            for m in &messages {
                if m.from == my_id {
                    continue; // 忽略自己发的
                }
                if let Some(room_code) = m.text.strip_prefix("BATTLE_INVITE:") {
                    if invite.incoming.is_none() && !invite.rejected_room_codes.contains(room_code) {
                        let username = home
                            .friends
                            .iter()
                            .find(|f| f.user_id == target)
                            .map(|f| f.username.clone())
                            .unwrap_or_else(|| target.clone());
                        invite.incoming = Some(IncomingInvite {
                            from_user_id: target.clone(),
                            from_username: username,
                            room_code: room_code.to_string(),
                        });
                    }
                } else if m.text == "BATTLE_ACCEPT" {
                    if invite.waiting_for_accept && invite.pending_host_target.as_deref() == Some(&target) {
                        invite.accepted_by_target = true;
                    }
                } else if let Some(rejected_code) = m.text.strip_prefix("BATTLE_REJECT:") {
                    // 只处理当前邀请的拒绝（带房间码匹配）
                    if invite.waiting_for_accept
                        && invite.pending_host_target.as_deref() == Some(&target)
                        && invite.room_code.as_deref() == Some(rejected_code)
                    {
                        invite.waiting_for_accept = false;
                        invite.pending_host_target = None;
                        invite.room_code = None;
                        invite.reject_notification = Some(format!("{} 拒绝了对战邀请",
                            home.friends.iter().find(|f| f.user_id == target).map(|f| f.username.clone()).unwrap_or_else(|| target.clone())
                        ));
                    }
                }
            }

            // 更新聊天记录（仅在聊天页且目标匹配时）
            if target == chat.target_user_id {
                chat.messages = messages
                    .into_iter()
                    .map(|m| ChatEntry {
                        from_me: m.from == my_id,
                        text: m.text,
                    })
                    .collect();
                chat.info = format!("聊天同步: {} 条消息", chat.messages.len());
            }
        }
        "error" => {
            let msg = resp.message.unwrap_or("未知错误".into());
            if login_input.connecting {
                login_input.info = msg;
                login_input.connecting = false;
            } else if !home.info.is_empty() || home.refresh_timer > 0.0 {
                home.info = msg;
            } else {
                chat.info = msg;
            }
        }
        _ => {}
    }
}

// ==================== egui UI ====================

fn register_egui_font(ctx: &egui::Context, registered: &mut EguiFontRegistered) {
    if registered.0 {
        return;
    }
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "cjk".to_string(),
        egui::FontData::from_static(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/embedded_ui_font.bin"
        )))
        .into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "cjk".to_string());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "cjk".to_string());
    ctx.set_fonts(fonts);
    registered.0 = true;
}

fn online_login_egui_system(
    mut contexts: EguiContexts,
    mut conn: ResMut<OnlineConnection>,
    mut input: ResMut<OnlineLoginInput>,
    mut font_flag: ResMut<EguiFontRegistered>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    register_egui_font(ctx, &mut font_flag);

    if input.server_address.is_empty() {
        input.server_address = format!("127.0.0.1:{SOCIAL_PORT}");
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(120.0);
            ui.heading("联网系统");
            ui.add_space(16.0);

            ui.horizontal(|ui| {
                ui.radio_value(&mut input.is_register, false, "登录");
                ui.radio_value(&mut input.is_register, true, "注册");
            });
            ui.add_space(8.0);

            egui::Grid::new("login_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("服务器地址:");
                    ui.text_edit_singleline(&mut input.server_address);
                    ui.end_row();
                    ui.label("用户名:");
                    ui.text_edit_singleline(&mut input.username);
                    ui.end_row();
                    ui.label("密码:");
                    ui.text_edit_singleline(&mut input.password);
                    ui.end_row();
                });
            ui.add_space(8.0);

            if input.connecting {
                // 连接建立后自动发送请求
                if conn.is_connected() {
                    let req = if input.is_register {
                        serde_json::to_string(&ReqRegister {
                            req_type: "register",
                            username: &input.username,
                            password: &input.password,
                        })
                    } else {
                        serde_json::to_string(&ReqLogin {
                            req_type: "login",
                            username: &input.username,
                            password: &input.password,
                        })
                    };
                    if let Ok(json) = req {
                        conn.send(json);
                        input.info = "请求中...".into();
                    }
                    input.connecting = false;
                } else {
                    ui.label("正在连接...");
                }
            } else if ui
                .button(if input.is_register { "注册" } else { "登录" })
                .clicked()
            {
                if input.username.trim().is_empty() || input.password.is_empty() {
                    input.info = "用户名和密码不能为空".into();
                } else if !conn.is_connected() {
                    conn.connect(&input.server_address);
                    input.connecting = true;
                    input.info = "正在连接服务器...".into();
                } else {
                    let req = if input.is_register {
                        serde_json::to_string(&ReqRegister {
                            req_type: "register",
                            username: &input.username,
                            password: &input.password,
                        })
                    } else {
                        serde_json::to_string(&ReqLogin {
                            req_type: "login",
                            username: &input.username,
                            password: &input.password,
                        })
                    };
                    if let Ok(json) = req {
                        conn.send(json);
                        input.info = "请求中...".into();
                    }
                }
            }

            ui.add_space(4.0);
            if !input.info.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_BLUE, &input.info);
            }

            ui.add_space(16.0);
            if ui.button("返回大厅").clicked() {
                conn.stop();
                conn.user_id = None;
                next_state.set(GameState::Lobby);
            }
        });
    });
}

fn online_home_egui_system(
    mut contexts: EguiContexts,
    mut conn: ResMut<OnlineConnection>,
    mut home: ResMut<OnlineHomeState>,
    mut invite: ResMut<OnlineBattleInvite>,
    mut pvp_conn: ResMut<PvpConnection>,
    mut entry_mode: ResMut<SelectionEntryMode>,
    mut font_flag: ResMut<EguiFontRegistered>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    register_egui_font(ctx, &mut font_flag);

    // 发起方：对方同意后进入选精灵
    if invite.accepted_by_target {
        invite.accepted_by_target = false;
        invite.waiting_for_accept = false;
        invite.pending_host_target = None;
        invite.room_code = None;
        *entry_mode = SelectionEntryMode::Pvp;
        next_state.set(GameState::PvpLobby);
        return;
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        // 顶部：返回大厅按钮
        ui.horizontal(|ui| {
            if ui.button("← 返回大厅").clicked() {
                conn.stop();
                conn.user_id = None;
                next_state.set(GameState::Lobby);
            }
            if ui.button("退出登录").clicked() {
                conn.stop();
                conn.user_id = None;
                next_state.set(GameState::OnlineLogin);
            }
            if ui.button("刷新").clicked() {
                send_request(&conn, &ReqSimple { req_type: "get_updates" });
                home.info.clear();
            }
            if !home.info.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_GREEN, &home.info);
            }
        });
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_min_width(420.0);
                ui.heading("用户主页");
                ui.label(format!(
                    "ID: {}",
                    conn.user_id.as_deref().unwrap_or("未知")
                ));
                ui.label(format!("用户名: {}", conn.username));

                ui.add_space(8.0);
                ui.label("修改用户名:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut home.new_username);
                    if ui.button("修改").clicked() {
                        send_request(&conn, &ReqUpdateUsername {
                            req_type: "update_username",
                            new_name: &home.new_username,
                        });
                    }
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(4.0);

                // 搜索用户
                ui.heading("搜索用户");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut home.search_query);
                    if ui.button("搜索").clicked() {
                        send_request(&conn, &ReqSearch {
                            req_type: "search_users",
                            query: &home.search_query,
                        });
                    }
                });
                ui.add_space(4.0);
                for entry in home.search_results.clone() {
                    ui.horizontal(|ui| {
                        ui.label(&entry.username);
                        if entry.is_friend {
                            ui.label("(已是好友)");
                        } else if ui.button("添加好友").clicked() {
                            send_request(&conn, &ReqWithId {
                                req_type: "add_friend",
                                user_id: &entry.user_id,
                            });
                        }
                    });
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(4.0);

                // 好友请求
                ui.heading("好友请求");
                if home.pending_requests.is_empty() {
                    ui.label("暂无好友请求");
                }
                for req in home.pending_requests.clone() {
                    ui.horizontal(|ui| {
                        ui.label(&req.username);
                        if ui.button("接受").clicked() {
                            send_request(&conn, &ReqWithId {
                                req_type: "accept_friend",
                                user_id: &req.user_id,
                            });
                        }
                    });
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // 好友列表
                ui.heading("好友列表");
                if home.friends.is_empty() {
                    ui.label("还没有好友，去搜索添加吧");
                }
                for friend in home.friends.clone() {
                    ui.horizontal(|ui| {
                        ui.label(&friend.username);
                        if friend.unread > 0 {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 80, 80),
                                format!("({}条新消息)", friend.unread),
                            );
                        }
                        if ui.button("聊天").clicked() {
                            home.pending_chat = Some(friend.clone());
                        }
                        if ui.button("邀请对战").clicked() {
                            // 通过 relay 创建房间
                            let relay_addr = "127.0.0.1:42043".to_string();
                            pvp::start_relay_host(&mut pvp_conn, relay_addr);
                            invite.pending_host_target = Some(friend.user_id.clone());
                            invite.room_code = None;
                            *entry_mode = SelectionEntryMode::Pvp;
                        }
                    });
                }

                ui.add_space(8.0);
            });
        }); // close ScrollArea

        // === 弹窗区域 ===

        // 等待对方同意
        if invite.waiting_for_accept {
            egui::Window::new("对战邀请已发送")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label("等待对方同意...");
                        ui.add_space(8.0);
                        ui.spinner();
                        ui.add_space(8.0);
                        if ui.button("取消邀请").clicked() {
                            invite.waiting_for_accept = false;
                            invite.pending_host_target = None;
                            invite.room_code = None;
                        }
                    });
                });
        }

        // 被邀请方弹窗
        if let Some(inv) = invite.incoming.clone() {
            egui::Window::new("收到对战邀请")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 200, 100),
                            format!("{} 邀请你对战！", inv.from_username),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button("同意").clicked() {
                                let relay_addr = "127.0.0.1:42043".to_string();
                                pvp::start_relay_client(&mut pvp_conn, relay_addr, inv.room_code.clone());
                                invite.rejected_room_codes.insert(inv.room_code.clone());
                                send_request(&conn, &ReqSendMessage {
                                    req_type: "send_message",
                                    to_user_id: &inv.from_user_id,
                                    text: "BATTLE_ACCEPT",
                                });
                                *entry_mode = SelectionEntryMode::Pvp;
                                invite.incoming = None;
                                next_state.set(GameState::PvpLobby);
                            }
                            if ui.button("拒绝").clicked() {
                                invite.rejected_room_codes.insert(inv.room_code.clone());
                                let reject_text = format!("BATTLE_REJECT:{}", inv.room_code);
                                send_request(&conn, &ReqSendMessage {
                                    req_type: "send_message",
                                    to_user_id: &inv.from_user_id,
                                    text: &reject_text,
                                });
                                invite.incoming = None;
                            }
                        });
                    });
                });
        }

        // 拒绝通知
        if let Some(msg) = invite.reject_notification.clone() {
            egui::Window::new("对战邀请被拒绝")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.colored_label(egui::Color32::LIGHT_RED, &msg);
                        ui.add_space(8.0);
                        if ui.button("确定").clicked() {
                            invite.reject_notification = None;
                        }
                    });
                });
        }
    });
}

fn online_chat_egui_system(
    mut contexts: EguiContexts,
    conn: ResMut<OnlineConnection>,
    mut chat: ResMut<OnlineChatState>,
    mut invite: ResMut<OnlineBattleInvite>,
    mut pvp_conn: ResMut<PvpConnection>,
    mut entry_mode: ResMut<SelectionEntryMode>,
    mut font_flag: ResMut<EguiFontRegistered>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    register_egui_font(ctx, &mut font_flag);

    // 发起方：对方同意后进入选精灵
    if invite.accepted_by_target {
        invite.accepted_by_target = false;
        invite.waiting_for_accept = false;
        invite.pending_host_target = None;
        invite.room_code = None;
        *entry_mode = SelectionEntryMode::Pvp;
        next_state.set(GameState::PvpLobby);
        return;
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading(format!("与 {} 聊天", chat.target_username));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("返回").clicked() {
                    next_state.set(GameState::OnlineHome);
                }
            });
        });

        ui.separator();

        // 聊天记录
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for msg in &chat.messages {
                    let color = if msg.from_me {
                        egui::Color32::LIGHT_BLUE
                    } else {
                        egui::Color32::WHITE
                    };
                    let label = if msg.text.starts_with("BATTLE_INVITE:")
                        || msg.text == "BATTLE_ACCEPT"
                        || msg.text.starts_with("BATTLE_REJECT:")
                    {
                        continue; // 隐藏协议消息
                    } else {
                        &msg.text
                    };
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{}:",
                            if msg.from_me { "我" } else { &chat.target_username }
                        ));
                        ui.colored_label(color, label);
                    });
                }
            });

        ui.separator();

        // 输入框
        ui.horizontal(|ui| {
            let send_clicked = ui.button("发送").clicked();
            let input_to_send = chat.input.clone();
            if send_clicked && !input_to_send.trim().is_empty() {
                chat.messages.push(ChatEntry { from_me: true, text: input_to_send.clone() });
                send_request(&conn, &ReqSendMessage {
                    req_type: "send_message",
                    to_user_id: &chat.target_user_id,
                    text: &input_to_send,
                });
                chat.input.clear();
                chat.refresh_timer = 0.0;
                chat.info.clear();
            }
            ui.text_edit_singleline(&mut chat.input);
            if ui.button("刷新").clicked() {
                let from = chat.target_user_id.clone();
                send_request(&conn, &ReqWithId { req_type: "get_messages", user_id: &from });
            }
        });

        if !chat.info.is_empty() {
            ui.colored_label(egui::Color32::LIGHT_BLUE, &chat.info);
        }

        // === 弹窗区域 ===

        // 等待对方同意
        if invite.waiting_for_accept {
            egui::Window::new("对战邀请已发送")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label("等待对方同意...");
                        ui.add_space(8.0);
                        ui.spinner();
                        ui.add_space(8.0);
                        if ui.button("取消邀请").clicked() {
                            invite.waiting_for_accept = false;
                            invite.pending_host_target = None;
                            invite.room_code = None;
                        }
                    });
                });
        }

        // 被邀请方弹窗
        if let Some(inv) = invite.incoming.clone() {
            egui::Window::new("收到对战邀请")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 200, 100),
                            format!("{} 邀请你对战！", inv.from_username),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button("同意").clicked() {
                                let relay_addr = "127.0.0.1:42043".to_string();
                                pvp::start_relay_client(&mut pvp_conn, relay_addr, inv.room_code.clone());
                                invite.rejected_room_codes.insert(inv.room_code.clone());
                                send_request(&conn, &ReqSendMessage {
                                    req_type: "send_message",
                                    to_user_id: &inv.from_user_id,
                                    text: "BATTLE_ACCEPT",
                                });
                                *entry_mode = SelectionEntryMode::Pvp;
                                invite.incoming = None;
                                next_state.set(GameState::PvpLobby);
                            }
                            if ui.button("拒绝").clicked() {
                                invite.rejected_room_codes.insert(inv.room_code.clone());
                                let reject_text = format!("BATTLE_REJECT:{}", inv.room_code);
                                send_request(&conn, &ReqSendMessage {
                                    req_type: "send_message",
                                    to_user_id: &inv.from_user_id,
                                    text: &reject_text,
                                });
                                invite.incoming = None;
                            }
                        });
                    });
                });
        }

        // 拒绝通知
        if let Some(msg) = invite.reject_notification.clone() {
            egui::Window::new("对战邀请被拒绝")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.colored_label(egui::Color32::LIGHT_RED, &msg);
                        ui.add_space(8.0);
                        if ui.button("确定").clicked() {
                            invite.reject_notification = None;
                        }
                    });
                });
        }
    });
}

// ==================== 清理 ====================

fn cleanup_online_login(mut input: ResMut<OnlineLoginInput>) {
    input.info.clear();
    input.connecting = false;
}

fn cleanup_online_home(mut home: ResMut<OnlineHomeState>) {
    home.info.clear();
}

fn cleanup_online_chat(mut chat: ResMut<OnlineChatState>) {
    chat.info.clear();
    chat.input.clear();
}
