use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{Mutex, mpsc},
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
    ui::battle::theme::UiTheme,
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
    #[serde(default)]
    online: bool,
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
    online: bool,
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
                send_request(
                    &conn,
                    &ReqSendMessage {
                        req_type: "send_message",
                        to_user_id: &target,
                        text: &text,
                    },
                );
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
                send_request(
                    &conn,
                    &ReqSimple {
                        req_type: "get_updates",
                    },
                );
                // 轮询一个好友的消息（检测对战邀请）
                if !home.friends.is_empty() {
                    if home.msg_poll_index >= home.friends.len() {
                        home.msg_poll_index = 0;
                    }
                    let fid = home.friends[home.msg_poll_index].user_id.clone();
                    home.msg_poll_index += 1;
                    send_request(
                        &conn,
                        &ReqWithId {
                            req_type: "get_messages",
                            user_id: &fid,
                        },
                    );
                }
            }
        }
        GameState::OnlineChat => {
            if !chat.target_user_id.is_empty() {
                chat.refresh_timer += time.delta_secs();
                if chat.refresh_timer > 1.0 {
                    chat.refresh_timer = 0.0;
                    let from = chat.target_user_id.clone();
                    send_request(
                        &conn,
                        &ReqWithId {
                            req_type: "get_messages",
                            user_id: &from,
                        },
                    );
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
                if resp.resp_type == "register_ok" {
                    "注册"
                } else {
                    "登录"
                },
                conn.username
            );
            login_input.connecting = false;
            home.new_username = conn.username.clone();
            next_state.set(GameState::OnlineHome);
            send_request(
                conn,
                &ReqSimple {
                    req_type: "get_updates",
                },
            );
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
            send_request(
                conn,
                &ReqSimple {
                    req_type: "get_updates",
                },
            );
        }
        "friends_list" | "updates" => {
            if let Some(friends) = resp.friends {
                home.friends = friends
                    .into_iter()
                    .map(|f| FriendEntry {
                        user_id: f.user_id,
                        username: f.username,
                        unread: f.unread,
                        online: f.online,
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
                        online: false,
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
                    online: false,
                })
                .collect();
        }
        "message_sent" => {
            chat.input.clear();
            chat.refresh_timer = 0.0;
            chat.info = "发送成功，等待同步最新消息".into();
            let from = chat.target_user_id.clone();
            send_request(
                conn,
                &ReqWithId {
                    req_type: "get_messages",
                    user_id: &from,
                },
            );
        }
        "messages" => {
            let target = resp
                .from_user_id
                .clone()
                .unwrap_or_else(|| chat.target_user_id.clone());
            let messages = resp.messages.take().unwrap_or_default();
            let my_id = conn.user_id.clone().unwrap_or_default();

            // 检查特殊消息（对战邀请/接受/拒绝）
            for m in &messages {
                if m.from == my_id {
                    continue; // 忽略自己发的
                }
                if let Some(room_code) = m.text.strip_prefix("BATTLE_INVITE:") {
                    if invite.incoming.is_none() && !invite.rejected_room_codes.contains(room_code)
                    {
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
                    if invite.waiting_for_accept
                        && invite.pending_host_target.as_deref() == Some(&target)
                    {
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
                        invite.reject_notification = Some(format!(
                            "{} 拒绝了对战邀请",
                            home.friends
                                .iter()
                                .find(|f| f.user_id == target)
                                .map(|f| f.username.clone())
                                .unwrap_or_else(|| target.clone())
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

fn egui_color(color: Color) -> egui::Color32 {
    let c = color.to_srgba();
    egui::Color32::from_rgba_unmultiplied(
        (c.red.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.green.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.blue.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

fn apply_online_egui_style(ctx: &egui::Context, theme: &UiTheme) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(egui_color(theme.text_primary));
    style.visuals.panel_fill = egui_color(theme.bg_bottom);
    style.visuals.window_fill = egui_color(theme.panel);
    style.visuals.extreme_bg_color = egui_color(theme.bg_bottom);
    style.visuals.faint_bg_color = egui_color(theme.lacquer_panel);
    style.visuals.selection.bg_fill = egui_color(theme.gold_dim);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, egui_color(theme.gold_bright));

    let widgets = &mut style.visuals.widgets;
    widgets.noninteractive.bg_fill = egui_color(theme.lacquer_panel);
    widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, egui_color(theme.border_panel));
    widgets.inactive.bg_fill = egui_color(theme.button_idle);
    widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui_color(theme.button_border_idle));
    widgets.hovered.bg_fill = egui_color(theme.button_hover);
    widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui_color(theme.button_border_hover));
    widgets.active.bg_fill = egui_color(theme.button_pressed);
    widgets.active.bg_stroke = egui::Stroke::new(1.0, egui_color(theme.button_border_pressed));
    widgets.open.bg_fill = egui_color(theme.button_hover);
    widgets.open.bg_stroke = egui::Stroke::new(1.0, egui_color(theme.gold));

    ctx.set_style(style);
}

fn online_panel_frame(ui: &egui::Ui, theme: &UiTheme) -> egui::Frame {
    egui::Frame::group(ui.style())
        .fill(egui_color(theme.lacquer_panel))
        .stroke(egui::Stroke::new(1.0, egui_color(theme.border_panel)))
        .corner_radius(egui::CornerRadius::same(16))
        .inner_margin(egui::Margin::same(16))
}

fn online_card_frame(ui: &egui::Ui, theme: &UiTheme) -> egui::Frame {
    egui::Frame::group(ui.style())
        .fill(egui_color(theme.panel))
        .stroke(egui::Stroke::new(1.0, egui_color(theme.gold_divider)))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::same(12))
}

fn section_title(ui: &mut egui::Ui, text: &str, theme: &UiTheme) {
    ui.label(
        egui::RichText::new(text)
            .size(20.0)
            .strong()
            .color(egui_color(theme.gold_bright)),
    );
}

fn online_button(ui: &mut egui::Ui, label: &str, theme: &UiTheme) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(egui_color(theme.text_primary)))
            .min_size(egui::vec2(86.0, 32.0)),
    )
}

fn status_text_color(message: &str, theme: &UiTheme) -> egui::Color32 {
    if message.contains("失败")
        || message.contains("断开")
        || message.contains("错误")
        || message.contains("拒绝")
        || message.contains("不能为空")
        || message.contains("请输入")
        || message.contains("格式")
    {
        egui_color(theme.status_debuff)
    } else if message.contains("成功") || message.contains("已") {
        egui_color(theme.status_buff)
    } else if message.contains("等待") || message.contains("正在") || message.contains("请求")
    {
        egui_color(theme.gold_bright)
    } else {
        egui_color(theme.text_secondary)
    }
}

fn show_status_message(ui: &mut egui::Ui, message: &str, theme: &UiTheme) {
    if !message.is_empty() {
        ui.label(
            egui::RichText::new(message)
                .color(status_text_color(message, theme))
                .strong(),
        );
    }
}

fn show_invite_feedback_windows(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    conn: &OnlineConnection,
    invite: &mut OnlineBattleInvite,
    pvp_conn: &mut PvpConnection,
    lobby_input: &mut crate::pvp::PvpLobbyInput,
    entry_mode: &mut SelectionEntryMode,
    next_state: &mut NextState<GameState>,
) {
    if invite.waiting_for_accept {
        egui::Window::new("对战邀请已发送")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                online_panel_frame(ui, theme).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        section_title(ui, "等待对方回应", theme);
                        ui.add_space(6.0);
                        ui.spinner();
                        show_status_message(ui, "邀请已发送，对方同意后将自动进入配队", theme);
                        ui.add_space(10.0);
                        if online_button(ui, "取消邀请", theme).clicked() {
                            invite.waiting_for_accept = false;
                            invite.pending_host_target = None;
                            invite.room_code = None;
                        }
                    });
                });
            });
    }

    if let Some(inv) = invite.incoming.clone() {
        egui::Window::new("收到对战邀请")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                online_panel_frame(ui, theme).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        section_title(ui, "好友对战邀请", theme);
                        ui.label(
                            egui::RichText::new(format!("{} 邀请你对战！", inv.from_username))
                                .size(18.0)
                                .strong()
                                .color(egui_color(theme.gold_bright)),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if online_button(ui, "同意", theme).clicked() {
                                let relay_addr = "127.0.0.1:42043".to_string();
                                pvp::start_relay_client(
                                    pvp_conn,
                                    relay_addr,
                                    inv.room_code.clone(),
                                );
                                invite.rejected_room_codes.insert(inv.room_code.clone());
                                send_request(
                                    conn,
                                    &ReqSendMessage {
                                        req_type: "send_message",
                                        to_user_id: &inv.from_user_id,
                                        text: "BATTLE_ACCEPT",
                                    },
                                );
                                *entry_mode = SelectionEntryMode::Pvp;
                                lobby_input.screen = crate::pvp::PvpLobbyScreen::RelayJoinRoom;
                                invite.incoming = None;
                                next_state.set(GameState::PvpLobby);
                            }
                            if online_button(ui, "拒绝", theme).clicked() {
                                invite.rejected_room_codes.insert(inv.room_code.clone());
                                let reject_text = format!("BATTLE_REJECT:{}", inv.room_code);
                                send_request(
                                    conn,
                                    &ReqSendMessage {
                                        req_type: "send_message",
                                        to_user_id: &inv.from_user_id,
                                        text: &reject_text,
                                    },
                                );
                                invite.incoming = None;
                            }
                        });
                    });
                });
            });
    }

    if let Some(msg) = invite.reject_notification.clone() {
        egui::Window::new("对战邀请被拒绝")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                online_panel_frame(ui, theme).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        show_status_message(ui, &msg, theme);
                        ui.add_space(8.0);
                        if online_button(ui, "确定", theme).clicked() {
                            invite.reject_notification = None;
                        }
                    });
                });
            });
    }
}

fn online_login_egui_system(
    mut contexts: EguiContexts,
    mut conn: ResMut<OnlineConnection>,
    mut input: ResMut<OnlineLoginInput>,
    mut font_flag: ResMut<EguiFontRegistered>,
    theme: Res<UiTheme>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    register_egui_font(ctx, &mut font_flag);
    apply_online_egui_style(ctx, &theme);

    if input.server_address.is_empty() {
        input.server_address = format!("127.0.0.1:{SOCIAL_PORT}");
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(88.0);
            ui.label(
                egui::RichText::new("联网系统")
                    .size(40.0)
                    .strong()
                    .color(egui_color(theme.text_primary)),
            );
            ui.label(
                egui::RichText::new("登录账号后可添加好友、聊天，并一键发起对战邀请")
                    .size(16.0)
                    .color(egui_color(theme.text_secondary)),
            );
            ui.add_space(18.0);

            online_panel_frame(ui, &theme).show(ui, |ui| {
                ui.set_width(460.0);
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut input.is_register, false, "登录");
                        ui.selectable_value(&mut input.is_register, true, "注册");
                    });
                });
                ui.add_space(12.0);

                egui::Grid::new("login_grid")
                    .num_columns(2)
                    .spacing([12.0, 10.0])
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("服务器").color(egui_color(theme.text_secondary)),
                        );
                        ui.text_edit_singleline(&mut input.server_address);
                        ui.end_row();
                        ui.label(
                            egui::RichText::new("用户名").color(egui_color(theme.text_secondary)),
                        );
                        ui.text_edit_singleline(&mut input.username);
                        ui.end_row();
                        ui.label(
                            egui::RichText::new("密码").color(egui_color(theme.text_secondary)),
                        );
                        ui.add(egui::TextEdit::singleline(&mut input.password).password(true));
                        ui.end_row();
                    });
                ui.add_space(12.0);

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
                        ui.horizontal(|ui| {
                            ui.spinner();
                            show_status_message(ui, "正在连接服务器...", &theme);
                        });
                    }
                } else if online_button(
                    ui,
                    if input.is_register {
                        "注册"
                    } else {
                        "登录"
                    },
                    &theme,
                )
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

                ui.add_space(6.0);
                show_status_message(ui, &input.info, &theme);
            });

            ui.add_space(16.0);
            if online_button(ui, "返回大厅", &theme).clicked() {
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
    mut lobby_input: ResMut<crate::pvp::PvpLobbyInput>,
    mut entry_mode: ResMut<SelectionEntryMode>,
    mut font_flag: ResMut<EguiFontRegistered>,
    theme: Res<UiTheme>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    register_egui_font(ctx, &mut font_flag);
    apply_online_egui_style(ctx, &theme);

    // 发起方：对方同意后进入选精灵
    if invite.accepted_by_target {
        invite.accepted_by_target = false;
        invite.waiting_for_accept = false;
        invite.pending_host_target = None;
        invite.room_code = None;
        *entry_mode = SelectionEntryMode::Pvp;
        lobby_input.screen = crate::pvp::PvpLobbyScreen::RelayHostRoom;
        next_state.set(GameState::PvpLobby);
        return;
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            if online_button(ui, "← 返回大厅", &theme).clicked() {
                conn.stop();
                conn.user_id = None;
                next_state.set(GameState::Lobby);
            }
            if online_button(ui, "退出登录", &theme).clicked() {
                conn.stop();
                conn.user_id = None;
                next_state.set(GameState::OnlineLogin);
            }
            if online_button(ui, "刷新", &theme).clicked() {
                send_request(
                    &conn,
                    &ReqSimple {
                        req_type: "get_updates",
                    },
                );
                home.info = "正在刷新好友与消息...".into();
            }
            show_status_message(ui, &home.info, &theme);
        });
        ui.add_space(12.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_min_width(760.0);
                ui.label(
                    egui::RichText::new("用户主页")
                        .size(34.0)
                        .strong()
                        .color(egui_color(theme.text_primary)),
                );
                ui.label(
                    egui::RichText::new("管理资料、好友、聊天和一键对战邀请")
                        .color(egui_color(theme.text_secondary)),
                );
                ui.add_space(12.0);

                online_panel_frame(ui, &theme).show(ui, |ui| {
                    section_title(ui, "个人信息", &theme);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("ID：{}", conn.user_id.as_deref().unwrap_or("未知")));
                        ui.separator();
                        ui.label(format!("用户名：{}", conn.username));
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("修改用户名")
                                .color(egui_color(theme.text_secondary)),
                        );
                        ui.text_edit_singleline(&mut home.new_username);
                        if online_button(ui, "保存", &theme).clicked() {
                            send_request(
                                &conn,
                                &ReqUpdateUsername {
                                    req_type: "update_username",
                                    new_name: &home.new_username,
                                },
                            );
                            home.info = "正在提交用户名...".into();
                        }
                    });
                });

                ui.add_space(12.0);
                ui.columns(2, |columns| {
                    online_card_frame(&columns[0], &theme).show(&mut columns[0], |ui| {
                        section_title(ui, "搜索用户", &theme);
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut home.search_query);
                            if online_button(ui, "搜索", &theme).clicked() {
                                send_request(
                                    &conn,
                                    &ReqSearch {
                                        req_type: "search_users",
                                        query: &home.search_query,
                                    },
                                );
                                home.info = "正在搜索用户...".into();
                            }
                        });
                        ui.add_space(6.0);
                        if home.search_results.is_empty() {
                            ui.label(
                                egui::RichText::new("输入用户名关键词来查找玩家")
                                    .color(egui_color(theme.text_muted)),
                            );
                        }
                        for entry in home.search_results.clone() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new(&entry.username).strong());
                                if entry.is_friend {
                                    ui.label(
                                        egui::RichText::new("已是好友")
                                            .color(egui_color(theme.status_buff)),
                                    );
                                } else if online_button(ui, "添加好友", &theme).clicked() {
                                    send_request(
                                        &conn,
                                        &ReqWithId {
                                            req_type: "add_friend",
                                            user_id: &entry.user_id,
                                        },
                                    );
                                    home.info = format!("已向 {} 发送好友请求", entry.username);
                                }
                            });
                        }
                    });

                    online_card_frame(&columns[1], &theme).show(&mut columns[1], |ui| {
                        section_title(ui, "好友请求", &theme);
                        if home.pending_requests.is_empty() {
                            ui.label(
                                egui::RichText::new("暂无好友请求")
                                    .color(egui_color(theme.text_muted)),
                            );
                        }
                        for req in home.pending_requests.clone() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new(&req.username).strong());
                                if online_button(ui, "接受", &theme).clicked() {
                                    send_request(
                                        &conn,
                                        &ReqWithId {
                                            req_type: "accept_friend",
                                            user_id: &req.user_id,
                                        },
                                    );
                                    home.info = format!("正在接受 {} 的好友请求", req.username);
                                }
                            });
                        }
                    });
                });

                ui.add_space(12.0);
                online_panel_frame(ui, &theme).show(ui, |ui| {
                    section_title(ui, "好友列表", &theme);
                    if home.friends.is_empty() {
                        ui.label(
                            egui::RichText::new("还没有好友，去搜索添加吧")
                                .color(egui_color(theme.text_muted)),
                        );
                    }
                    for friend in home.friends.clone() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(&friend.username).strong());
                            let state_text = if friend.online {
                                "● 在线"
                            } else {
                                "● 离线"
                            };
                            let state_color = if friend.online {
                                theme.status_buff
                            } else {
                                theme.text_muted
                            };
                            ui.label(
                                egui::RichText::new(state_text).color(egui_color(state_color)),
                            );
                            if friend.unread > 0 {
                                ui.label(
                                    egui::RichText::new(format!("{} 条新消息", friend.unread))
                                        .color(egui_color(theme.status_debuff))
                                        .strong(),
                                );
                            }
                            if online_button(ui, "聊天", &theme).clicked() {
                                home.pending_chat = Some(friend.clone());
                            }
                            if friend.online {
                                if online_button(ui, "邀请对战", &theme).clicked() {
                                    // 通过 relay 创建房间
                                    let relay_addr = "127.0.0.1:42043".to_string();
                                    pvp::start_relay_host(&mut pvp_conn, relay_addr);
                                    invite.pending_host_target = Some(friend.user_id.clone());
                                    invite.room_code = None;
                                    *entry_mode = SelectionEntryMode::Pvp;
                                    lobby_input.screen = crate::pvp::PvpLobbyScreen::RelayHostRoom;
                                    home.info =
                                        format!("正在向 {} 发起对战邀请...", friend.username);
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new("离线不可邀请")
                                        .color(egui_color(theme.text_muted)),
                                );
                            }
                        });
                        ui.separator();
                    }
                });

                ui.add_space(8.0);
            });
        });

        show_invite_feedback_windows(
            ui,
            &theme,
            &conn,
            &mut invite,
            &mut pvp_conn,
            &mut lobby_input,
            &mut entry_mode,
            &mut next_state,
        );
    });
}

fn online_chat_egui_system(
    mut contexts: EguiContexts,
    conn: ResMut<OnlineConnection>,
    mut chat: ResMut<OnlineChatState>,
    mut invite: ResMut<OnlineBattleInvite>,
    mut pvp_conn: ResMut<PvpConnection>,
    mut lobby_input: ResMut<crate::pvp::PvpLobbyInput>,
    mut entry_mode: ResMut<SelectionEntryMode>,
    mut font_flag: ResMut<EguiFontRegistered>,
    theme: Res<UiTheme>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    register_egui_font(ctx, &mut font_flag);
    apply_online_egui_style(ctx, &theme);

    // 发起方：对方同意后进入选精灵
    if invite.accepted_by_target {
        invite.accepted_by_target = false;
        invite.waiting_for_accept = false;
        invite.pending_host_target = None;
        invite.room_code = None;
        *entry_mode = SelectionEntryMode::Pvp;
        lobby_input.screen = crate::pvp::PvpLobbyScreen::RelayHostRoom;
        next_state.set(GameState::PvpLobby);
        return;
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("与 {} 聊天", chat.target_username))
                    .size(30.0)
                    .strong()
                    .color(egui_color(theme.text_primary)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if online_button(ui, "返回", &theme).clicked() {
                    next_state.set(GameState::OnlineHome);
                }
            });
        });
        ui.add_space(10.0);

        online_panel_frame(ui, &theme).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .stick_to_bottom(true)
                .max_height(460.0)
                .show(ui, |ui| {
                    if chat.messages.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("暂无聊天记录，发送一句问候吧")
                                    .color(egui_color(theme.text_muted)),
                            );
                        });
                    }
                    for msg in &chat.messages {
                        if msg.text.starts_with("BATTLE_INVITE:")
                            || msg.text == "BATTLE_ACCEPT"
                            || msg.text.starts_with("BATTLE_REJECT:")
                        {
                            continue; // 隐藏协议消息
                        }
                        let speaker = if msg.from_me {
                            "我"
                        } else {
                            &chat.target_username
                        };
                        let bubble_color = if msg.from_me {
                            theme.button_hover
                        } else {
                            theme.panel
                        };
                        let text_color = if msg.from_me {
                            theme.text_primary
                        } else {
                            theme.text_secondary
                        };
                        let layout = if msg.from_me {
                            egui::Layout::right_to_left(egui::Align::Min)
                        } else {
                            egui::Layout::left_to_right(egui::Align::Min)
                        };
                        ui.with_layout(layout, |ui| {
                            egui::Frame::group(ui.style())
                                .fill(egui_color(bubble_color))
                                .stroke(egui::Stroke::new(1.0, egui_color(theme.gold_divider)))
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::same(10))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(speaker)
                                            .strong()
                                            .color(egui_color(theme.gold_bright)),
                                    );
                                    ui.label(
                                        egui::RichText::new(&msg.text)
                                            .color(egui_color(text_color)),
                                    );
                                });
                        });
                        ui.add_space(6.0);
                    }
                });
        });

        ui.add_space(10.0);
        online_card_frame(ui, &theme).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [ui.available_width() - 190.0, 32.0],
                    egui::TextEdit::singleline(&mut chat.input)
                        .hint_text("输入消息，回车或点击发送"),
                );
                let send_clicked = online_button(ui, "发送", &theme).clicked()
                    || ui.input(|input| input.key_pressed(egui::Key::Enter));
                let input_to_send = chat.input.clone();
                if send_clicked && !input_to_send.trim().is_empty() {
                    chat.messages.push(ChatEntry {
                        from_me: true,
                        text: input_to_send.clone(),
                    });
                    send_request(
                        &conn,
                        &ReqSendMessage {
                            req_type: "send_message",
                            to_user_id: &chat.target_user_id,
                            text: &input_to_send,
                        },
                    );
                    chat.input.clear();
                    chat.refresh_timer = 0.0;
                    chat.info = "消息已发送，等待服务器同步".into();
                }
                if online_button(ui, "刷新", &theme).clicked() {
                    let from = chat.target_user_id.clone();
                    send_request(
                        &conn,
                        &ReqWithId {
                            req_type: "get_messages",
                            user_id: &from,
                        },
                    );
                    chat.info = "正在刷新聊天记录...".into();
                }
            });
            show_status_message(ui, &chat.info, &theme);
        });

        show_invite_feedback_windows(
            ui,
            &theme,
            &conn,
            &mut invite,
            &mut pvp_conn,
            &mut lobby_input,
            &mut entry_mode,
            &mut next_state,
        );
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
