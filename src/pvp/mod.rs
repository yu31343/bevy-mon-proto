use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs, UdpSocket},
    sync::{
        Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    battle::{
        BattleControlMode, BattleEvent, BattleLog, BattleResult, Hand, PendingBoosts,
        SelectedCards, Side, TurnAction, TurnContext, push_battle_line,
    },
    data::{BattleDbs, CardEffect, MonsterPool, TeamSelections},
    game_state::GameState,
};

const DEFAULT_PORT: u16 = 42043;
const MAX_PORT_ATTEMPTS: u16 = 32;
const PROTOCOL_VERSION: u32 = 1;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(8);

pub struct PvpPlugin;

impl Plugin for PvpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PvpConnection>()
            .init_resource::<PvpLobbyInput>()
            .init_resource::<PvpTeamState>()
            .init_resource::<PvpIncomingIntents>()
            .add_systems(Update, pvp_poll_network_system)
            .add_systems(
                Update,
                setup_pvp_lobby_ui.run_if(in_state(GameState::PvpLobby)),
            )
            .add_systems(
                Update,
                pvp_lobby_button_system.run_if(in_state(GameState::PvpLobby)),
            )
            .add_systems(
                Update,
                update_pvp_lobby_ui_system.run_if(in_state(GameState::PvpLobby)),
            )
            .add_systems(
                Update,
                pvp_connection_input_system.run_if(in_state(GameState::PvpLobby)),
            )
            .add_systems(
                Update,
                pvp_handle_connected_system.run_if(in_state(GameState::PvpLobby)),
            )
            .add_systems(
                Update,
                pvp_apply_remote_team_system.run_if(in_state(GameState::TeamSelection)),
            )
            .add_systems(
                Update,
                pvp_apply_remote_intents_system.run_if(in_state(GameState::Battle)),
            )
            .add_systems(Update, pvp_handle_battle_disconnect_system)
            .add_systems(OnEnter(GameState::PvpLobby), reset_pvp_lobby_ui_state)
            .add_systems(OnExit(GameState::PvpLobby), cleanup_pvp_lobby_ui)
            .add_systems(OnExit(GameState::PvpLobby), clear_pending_error_on_exit);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PvpRole {
    Host,
    Client,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PvpStatus {
    Idle,
    Hosting { port: u16 },
    Connecting,
    Connected,
    Failed(String),
    Disconnected(String),
}

#[derive(Resource, Debug)]
pub struct PvpConnection {
    pub role: Option<PvpRole>,
    pub status: PvpStatus,
    pub local_ip: String,
    command_tx: Option<Sender<NetCommand>>,
    event_rx: Option<Mutex<Receiver<NetEvent>>>,
    pub seq: u32,
    pub protocol_ready: bool,
    pub remote_data_hash: Option<String>,
}

impl Default for PvpConnection {
    fn default() -> Self {
        Self {
            role: None,
            status: PvpStatus::Idle,
            local_ip: local_lan_ip(),
            command_tx: None,
            event_rx: None,
            seq: 0,
            protocol_ready: false,
            remote_data_hash: None,
        }
    }
}

#[derive(Resource, Debug)]
pub struct PvpLobbyInput {
    pub address: String,
    pub info: String,
}

impl Default for PvpLobbyInput {
    fn default() -> Self {
        Self {
            address: format!("127.0.0.1:{DEFAULT_PORT}"),
            info: "输入对方地址，或点击建房等待连接。".to_string(),
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct PvpTeamState {
    pub local_indices: Option<Vec<usize>>,
    pub remote_indices: Option<Vec<usize>>,
    pub battle_started: bool,
}

#[derive(Resource, Debug, Default)]
pub struct PvpIncomingIntents(pub Vec<BattleIntent>);

#[derive(Component)]
pub struct PvpLobbyUiRoot;

#[derive(Component)]
pub struct PvpHostButton;

#[derive(Component)]
pub struct PvpJoinButton;

#[derive(Component)]
pub struct PvpBackButton;

#[derive(Component)]
pub struct PvpAddressText;

#[derive(Component)]
pub struct PvpStatusText;

#[derive(Debug)]
enum NetCommand {
    Send(PvpMessage),
    Stop,
}

#[derive(Debug)]
enum NetEvent {
    Listening(u16),
    Connected,
    Message(PvpMessage),
    Failed(String),
    Disconnected(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum PvpMessage {
    Hello {
        protocol_version: u32,
        data_hash: String,
    },
    HelloAck {
        accepted: bool,
        reason: Option<String>,
    },
    TeamSelected {
        monster_indices: Vec<usize>,
    },
    BattleReady {
        seed: u64,
    },
    Intent {
        seq: u32,
        intent: BattleIntent,
    },
    Surrender,
    Leave {
        reason: String,
    },
    Ping {
        nonce: u64,
    },
    Pong {
        nonce: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleIntent {
    UseSkill { slot: usize },
    Switch { target_index: usize },
    UseCard { card_index: usize },
    DiscardCard { card_index: usize },
    EndTurn,
}

impl PvpConnection {
    pub fn is_connected(&self) -> bool {
        matches!(self.status, PvpStatus::Connected)
    }

    fn send(&self, message: PvpMessage) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(NetCommand::Send(message));
        }
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.command_tx.take() {
            let _ = tx.send(NetCommand::Stop);
        }
        self.event_rx = None;
        self.role = None;
        self.protocol_ready = false;
        self.remote_data_hash = None;
    }
}

pub fn start_host(connection: &mut PvpConnection) {
    connection.stop();
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    thread::spawn(move || host_thread(command_rx, event_tx));
    connection.role = Some(PvpRole::Host);
    connection.status = PvpStatus::Hosting { port: DEFAULT_PORT };
    connection.command_tx = Some(command_tx);
    connection.event_rx = Some(Mutex::new(event_rx));
    connection.protocol_ready = false;
    connection.remote_data_hash = None;
}

pub fn start_client(connection: &mut PvpConnection, address: String) {
    connection.stop();
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    thread::spawn(move || client_thread(address, command_rx, event_tx));
    connection.role = Some(PvpRole::Client);
    connection.status = PvpStatus::Connecting;
    connection.command_tx = Some(command_tx);
    connection.event_rx = Some(Mutex::new(event_rx));
    connection.protocol_ready = false;
    connection.remote_data_hash = None;
}

pub fn submit_local_team(
    connection: &PvpConnection,
    team_state: &mut PvpTeamState,
    indices: Vec<usize>,
) {
    team_state.local_indices = Some(indices.clone());
    connection.send(PvpMessage::TeamSelected {
        monster_indices: indices,
    });
}

pub fn send_intent(connection: &mut PvpConnection, intent: BattleIntent) {
    if !connection.is_connected() {
        return;
    }
    connection.seq = connection.seq.wrapping_add(1);
    connection.send(PvpMessage::Intent {
        seq: connection.seq,
        intent,
    });
}

pub fn surrender(connection: &PvpConnection) {
    connection.send(PvpMessage::Surrender);
}

pub fn data_hash(dbs: &BattleDbs, monsters: &MonsterPool) -> String {
    let mut hasher = DefaultHasher::new();
    format!("{:?}", dbs.skills.keys().collect::<Vec<_>>()).hash(&mut hasher);
    format!("{:?}", dbs.cards.keys().collect::<Vec<_>>()).hash(&mut hasher);
    format!("{:?}", monsters.monsters).hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn host_thread(command_rx: Receiver<NetCommand>, event_tx: Sender<NetEvent>) {
    let mut last_error = None;
    for offset in 0..MAX_PORT_ATTEMPTS {
        let port = DEFAULT_PORT.saturating_add(offset);
        match TcpListener::bind(("0.0.0.0", port)) {
            Ok(listener) => {
                let _ = event_tx.send(NetEvent::Listening(port));
                match listener.accept() {
                    Ok((stream, _)) => run_stream(stream, command_rx, event_tx),
                    Err(err) => {
                        let _ = event_tx.send(NetEvent::Failed(format!("接受连接失败：{err}")));
                    }
                }
                return;
            }
            Err(err) => last_error = Some(err.to_string()),
        }
    }
    let reason = last_error.unwrap_or_else(|| "没有可用端口".to_string());
    let _ = event_tx.send(NetEvent::Failed(format!("建房失败：{reason}")));
}

fn client_thread(address: String, command_rx: Receiver<NetCommand>, event_tx: Sender<NetEvent>) {
    let addrs = match address.to_socket_addrs() {
        Ok(addrs) => addrs.collect::<Vec<_>>(),
        Err(err) => {
            let _ = event_tx.send(NetEvent::Failed(format!("地址解析失败：{err}")));
            return;
        }
    };
    if addrs.is_empty() {
        let _ = event_tx.send(NetEvent::Failed("地址解析失败：没有可用地址".to_string()));
        return;
    }
    let mut last_error = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
            Ok(stream) => {
                run_stream(stream, command_rx, event_tx);
                return;
            }
            Err(err) => last_error = Some(err.to_string()),
        }
    }
    let reason = last_error.unwrap_or_else(|| "连接失败".to_string());
    let _ = event_tx.send(NetEvent::Failed(format!("连接失败：{reason}")));
}

fn run_stream(mut stream: TcpStream, command_rx: Receiver<NetCommand>, event_tx: Sender<NetEvent>) {
    let _ = stream.set_nonblocking(true);
    let _ = stream.set_nodelay(true);
    let _ = event_tx.send(NetEvent::Connected);
    let mut last_rx = Instant::now();
    let mut last_ping = Instant::now();
    let mut nonce = 0_u64;

    loop {
        while let Ok(command) = command_rx.try_recv() {
            match command {
                NetCommand::Send(message) => {
                    if let Err(err) = write_message(&mut stream, &message) {
                        let _ = event_tx.send(NetEvent::Disconnected(format!("发送失败：{err}")));
                        return;
                    }
                }
                NetCommand::Stop => {
                    let _ = write_message(
                        &mut stream,
                        &PvpMessage::Leave {
                            reason: "本方已离开".to_string(),
                        },
                    );
                    return;
                }
            }
        }

        match read_message(&mut stream) {
            Ok(Some(message)) => {
                last_rx = Instant::now();
                match message {
                    PvpMessage::Ping { nonce } => {
                        let _ = write_message(&mut stream, &PvpMessage::Pong { nonce });
                    }
                    PvpMessage::Pong { .. } => {}
                    other => {
                        let _ = event_tx.send(NetEvent::Message(other));
                    }
                }
            }
            Ok(None) => {}
            Err(err) => {
                let _ = event_tx.send(NetEvent::Disconnected(format!("连接断开：{err}")));
                return;
            }
        }

        if last_ping.elapsed() >= HEARTBEAT_INTERVAL {
            nonce = nonce.wrapping_add(1);
            if let Err(err) = write_message(&mut stream, &PvpMessage::Ping { nonce }) {
                let _ = event_tx.send(NetEvent::Disconnected(format!("心跳失败：{err}")));
                return;
            }
            last_ping = Instant::now();
        }
        if last_rx.elapsed() >= HEARTBEAT_TIMEOUT {
            let _ = event_tx.send(NetEvent::Disconnected("连接超时".to_string()));
            return;
        }
        thread::sleep(Duration::from_millis(16));
    }
}

fn write_message(stream: &mut TcpStream, message: &PvpMessage) -> std::io::Result<()> {
    let payload = ron::to_string(message)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
    let bytes = payload.as_bytes();
    let len = u32::try_from(bytes.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "消息过长"))?;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(bytes)?;
    Ok(())
}

fn read_message(stream: &mut TcpStream) -> std::io::Result<Option<PvpMessage>> {
    let mut len_buf = [0_u8; 4];
    match stream.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "对方已关闭连接",
            ));
        }
        Err(err) => return Err(err),
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 64 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "消息过长",
        ));
    }
    let mut payload = vec![0_u8; len];
    stream.read_exact(&mut payload)?;
    let text = String::from_utf8(payload)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
    ron::from_str(&text)
        .map(Some)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))
}

fn local_lan_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn reset_pvp_lobby_ui_state(
    mut input: ResMut<PvpLobbyInput>,
    mut connection: ResMut<PvpConnection>,
) {
    connection.local_ip = local_lan_ip();
    if matches!(connection.status, PvpStatus::Idle) {
        input.info = "输入对方地址，或点击建房等待连接。".to_string();
    }
}

fn clear_pending_error_on_exit(mut input: ResMut<PvpLobbyInput>) {
    input.info.clear();
}

fn cleanup_pvp_lobby_ui(mut commands: Commands, query: Query<Entity, With<PvpLobbyUiRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn setup_pvp_lobby_ui(
    mut commands: Commands,
    existing: Query<(), With<PvpLobbyUiRoot>>,
    ui_font: Option<Res<crate::ui::battle::resources::UiFontHandle>>,
) {
    if !existing.is_empty() {
        return;
    }
    let title_font = make_text_font(42.0, ui_font.as_deref());
    let body_font = make_text_font(20.0, ui_font.as_deref());
    let small_font = make_text_font(16.0, ui_font.as_deref());
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.08, 0.13)),
            PvpLobbyUiRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("联机对战"),
                title_font,
                TextColor(Color::srgb(0.95, 0.96, 1.0)),
            ));
            root.spawn((
                Text::new("地址："),
                body_font.clone(),
                TextColor(Color::srgb(0.85, 0.88, 0.94)),
                PvpAddressText,
            ));
            root.spawn((
                Text::new("状态"),
                small_font.clone(),
                TextColor(Color::srgb(0.70, 0.75, 0.84)),
                PvpStatusText,
            ));
            spawn_pvp_button(root, "建房", body_font.clone(), PvpHostButton);
            spawn_pvp_button(root, "加入", body_font.clone(), PvpJoinButton);
            spawn_pvp_button(root, "返回大厅", body_font, PvpBackButton);
            root.spawn((
                Text::new("可直接键入地址；Enter 等同于加入。"),
                small_font,
                TextColor(Color::srgb(0.55, 0.60, 0.70)),
            ));
        });
}

fn spawn_pvp_button<T: Component>(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    font: TextFont,
    marker: T,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(280.0),
                min_height: Val::Px(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.16, 0.20, 0.30)),
            BorderColor::all(Color::srgb(0.32, 0.38, 0.50)),
            marker,
        ))
        .with_children(|button| {
            button.spawn((Text::new(label.to_string()), font, TextColor(Color::WHITE)));
        });
}

fn make_text_font(
    size: f32,
    ui_font: Option<&crate::ui::battle::resources::UiFontHandle>,
) -> TextFont {
    if let Some(ui_font) = ui_font {
        TextFont {
            font: ui_font.0.clone(),
            font_size: size,
            ..default()
        }
    } else {
        TextFont {
            font_size: size,
            ..default()
        }
    }
}

fn pvp_lobby_button_system(
    mut host_buttons: Query<&Interaction, (Changed<Interaction>, With<PvpHostButton>)>,
    mut join_buttons: Query<&Interaction, (Changed<Interaction>, With<PvpJoinButton>)>,
    mut back_buttons: Query<&Interaction, (Changed<Interaction>, With<PvpBackButton>)>,
    mut connection: ResMut<PvpConnection>,
    mut input: ResMut<PvpLobbyInput>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in &mut host_buttons {
        if *interaction == Interaction::Pressed {
            input.info = "正在建房...".to_string();
            start_host(&mut connection);
            return;
        }
    }
    for interaction in &mut join_buttons {
        if *interaction == Interaction::Pressed {
            let address = input.address.trim().to_string();
            if valid_address_like(&address) {
                input.info = format!("正在连接 {address} ...");
                start_client(&mut connection, address);
            } else {
                input.info = "地址格式应为 ip:端口 或 域名:端口。".to_string();
            }
            return;
        }
    }
    for interaction in &mut back_buttons {
        if *interaction == Interaction::Pressed {
            connection.stop();
            connection.status = PvpStatus::Idle;
            next_state.set(GameState::Lobby);
            return;
        }
    }
}

fn update_pvp_lobby_ui_system(
    connection: Res<PvpConnection>,
    input: Res<PvpLobbyInput>,
    mut address_text: Query<&mut Text, (With<PvpAddressText>, Without<PvpStatusText>)>,
    mut status_text: Query<&mut Text, (With<PvpStatusText>, Without<PvpAddressText>)>,
) {
    for mut text in &mut address_text {
        **text = format!("对方地址：{}", input.address);
    }
    let status = match &connection.status {
        PvpStatus::Idle => format!("本机局域网 IP：{}。{}", connection.local_ip, input.info),
        PvpStatus::Hosting { port } => format!(
            "本机局域网 IP：{}，端口：{}。{}",
            connection.local_ip, port, input.info
        ),
        PvpStatus::Connecting => input.info.clone(),
        PvpStatus::Connected => input.info.clone(),
        PvpStatus::Failed(reason) => reason.clone(),
        PvpStatus::Disconnected(reason) => reason.clone(),
    };
    for mut text in &mut status_text {
        **text = status.clone();
    }
}

fn pvp_connection_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut input: ResMut<PvpLobbyInput>,
    mut connection: ResMut<PvpConnection>,
) {
    if keyboard.just_pressed(KeyCode::Backspace) {
        input.address.pop();
    }
    if keyboard.just_pressed(KeyCode::Space) {
        input.address.push(' ');
    }
    for (key, ch) in address_key_map() {
        if keyboard.just_pressed(key) {
            input.address.push(ch);
        }
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        let address = input.address.trim().to_string();
        if valid_address_like(&address) {
            input.info = format!("正在连接 {address} ...");
            start_client(&mut connection, address);
        } else {
            input.info = "地址格式应为 ip:端口 或 域名:端口。".to_string();
        }
    }
}

fn address_key_map() -> [(KeyCode, char); 39] {
    [
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
        (KeyCode::Numpad0, '0'),
        (KeyCode::Numpad1, '1'),
        (KeyCode::Numpad2, '2'),
        (KeyCode::Numpad3, '3'),
        (KeyCode::Numpad4, '4'),
        (KeyCode::Numpad5, '5'),
        (KeyCode::Numpad6, '6'),
        (KeyCode::Numpad7, '7'),
        (KeyCode::Numpad8, '8'),
        (KeyCode::Numpad9, '9'),
        (KeyCode::Period, '.'),
        (KeyCode::NumpadDecimal, '.'),
        (KeyCode::Minus, '-'),
        (KeyCode::KeyA, 'a'),
        (KeyCode::KeyB, 'b'),
        (KeyCode::KeyC, 'c'),
        (KeyCode::KeyD, 'd'),
        (KeyCode::KeyE, 'e'),
        (KeyCode::KeyF, 'f'),
        (KeyCode::KeyG, 'g'),
        (KeyCode::KeyH, 'h'),
        (KeyCode::KeyI, 'i'),
        (KeyCode::KeyJ, 'j'),
        (KeyCode::KeyK, 'k'),
        (KeyCode::KeyL, 'l'),
        (KeyCode::KeyM, 'm'),
        (KeyCode::KeyN, 'n'),
        (KeyCode::KeyO, 'o'),
        (KeyCode::KeyP, 'p'),
    ]
}

fn valid_address_like(address: &str) -> bool {
    let Some((host, port)) = address.rsplit_once(':') else {
        return false;
    };
    !host.trim().is_empty() && port.parse::<u16>().is_ok()
}

fn pvp_poll_network_system(
    mut connection: ResMut<PvpConnection>,
    mut team_state: ResMut<PvpTeamState>,
    mut incoming_intents: ResMut<PvpIncomingIntents>,
    mut input: ResMut<PvpLobbyInput>,
    dbs: Option<Res<BattleDbs>>,
    monsters: Option<Res<MonsterPool>>,
) {
    let mut events = Vec::new();
    if let Some(rx) = &connection.event_rx {
        let Ok(rx) = rx.lock() else {
            return;
        };
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
    }
    for event in events {
        match event {
            NetEvent::Listening(port) => {
                connection.status = PvpStatus::Hosting { port };
                input.info = format!("建房成功：{}:{port}，等待对方加入。", connection.local_ip);
            }
            NetEvent::Connected => {
                connection.status = PvpStatus::Connected;
                input.info = "已连接，正在握手。".to_string();
                if let (Some(dbs), Some(monsters)) = (dbs.as_ref(), monsters.as_ref()) {
                    connection.send(PvpMessage::Hello {
                        protocol_version: PROTOCOL_VERSION,
                        data_hash: data_hash(dbs, monsters),
                    });
                }
            }
            NetEvent::Message(message) => match message {
                PvpMessage::Hello {
                    protocol_version,
                    data_hash: remote_hash,
                } => {
                    let local_hash = dbs
                        .as_ref()
                        .zip(monsters.as_ref())
                        .map(|(dbs, monsters)| data_hash(dbs, monsters))
                        .unwrap_or_default();
                    let accepted =
                        protocol_version == PROTOCOL_VERSION && remote_hash == local_hash;
                    let reason = if protocol_version != PROTOCOL_VERSION {
                        Some("协议版本不一致".to_string())
                    } else if remote_hash != local_hash {
                        Some("双方数据版本不一致".to_string())
                    } else {
                        None
                    };
                    connection.remote_data_hash = Some(remote_hash);
                    connection.protocol_ready = accepted;
                    connection.send(PvpMessage::HelloAck {
                        accepted,
                        reason: reason.clone(),
                    });
                    if let Some(reason) = reason {
                        input.info = reason;
                    } else {
                        input.info = "握手完成，请选择队伍。".to_string();
                    }
                }
                PvpMessage::HelloAck { accepted, reason } => {
                    connection.protocol_ready = accepted;
                    if accepted {
                        input.info = "握手完成，请选择队伍。".to_string();
                    } else {
                        input.info = reason.unwrap_or_else(|| "连接被拒绝".to_string());
                        connection.status = PvpStatus::Failed(input.info.clone());
                    }
                }
                PvpMessage::TeamSelected { monster_indices } => {
                    team_state.remote_indices = Some(monster_indices);
                }
                PvpMessage::BattleReady { .. } => {}
                PvpMessage::Intent { intent, .. } => incoming_intents.0.push(intent),
                PvpMessage::Surrender => {
                    connection.status = PvpStatus::Disconnected("对方已撤退/战斗中止".to_string());
                }
                PvpMessage::Leave { reason } => {
                    connection.status = PvpStatus::Disconnected(reason);
                }
                PvpMessage::Ping { .. } | PvpMessage::Pong { .. } => {}
            },
            NetEvent::Failed(reason) => {
                input.info = reason.clone();
                connection.status = PvpStatus::Failed(reason);
            }
            NetEvent::Disconnected(reason) => {
                input.info = reason.clone();
                connection.status = PvpStatus::Disconnected(reason);
            }
        }
    }
}

fn pvp_handle_connected_system(
    connection: Res<PvpConnection>,
    mut next_state: ResMut<NextState<GameState>>,
    mut entry_mode: ResMut<crate::team_selection::SelectionEntryMode>,
) {
    if connection.is_connected() && connection.protocol_ready {
        *entry_mode = crate::team_selection::SelectionEntryMode::Pvp;
        next_state.set(GameState::TeamSelection);
    }
}

fn pvp_apply_remote_team_system(
    mut commands: Commands,
    mut team_state: ResMut<PvpTeamState>,
    connection: Res<PvpConnection>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if team_state.battle_started || !connection.protocol_ready {
        return;
    }
    let (Some(local_indices), Some(remote_indices)) = (
        team_state.local_indices.clone(),
        team_state.remote_indices.clone(),
    ) else {
        return;
    };
    commands.insert_resource(BattleControlMode::PlayerVsRemote);
    commands.insert_resource(TeamSelections {
        player_indices: local_indices,
        enemy_indices: remote_indices,
    });
    team_state.battle_started = true;
    next_state.set(GameState::Battle);
}

fn pvp_apply_remote_intents_system(
    mut incoming: ResMut<PvpIncomingIntents>,
    battle_mode: Res<BattleControlMode>,
    mut turn_ctx: ResMut<TurnContext>,
    mut selected: ResMut<SelectedCards>,
    mut hand: ResMut<Hand>,
    mut action_points: ResMut<crate::battle::ActionPoints>,
    mut pending_boosts: ResMut<PendingBoosts>,
    enemy_team: Option<Res<crate::battle::EnemyTeam>>,
    skill_query: Query<&crate::battle::SkillList, With<crate::battle::InBattle>>,
    dbs: Res<BattleDbs>,
    mut event_writer: MessageWriter<BattleEvent>,
) {
    if *battle_mode != BattleControlMode::PlayerVsRemote {
        return;
    }
    let Some(intent) = incoming.0.first().cloned() else {
        return;
    };
    incoming.0.remove(0);
    match intent {
        BattleIntent::UseSkill { slot } => {
            let Some(enemy_team) = enemy_team else {
                return;
            };
            let Some(entity) = enemy_team.0.active_combatant() else {
                return;
            };
            let Ok(skill_list) = skill_query.get(entity) else {
                return;
            };
            let Some(skill_id) = skill_list.0.get(slot).copied() else {
                return;
            };
            turn_ctx.enemy_action = Some(TurnAction::Skill(skill_id));
        }
        BattleIntent::Switch { target_index } => {
            selected.enemy.index = Some(target_index);
        }
        BattleIntent::UseCard { card_index } => {
            apply_remote_card(
                card_index,
                false,
                &mut hand,
                &mut action_points,
                &mut pending_boosts,
                &dbs,
                &mut event_writer,
            );
        }
        BattleIntent::DiscardCard { card_index } => {
            apply_remote_card(
                card_index,
                true,
                &mut hand,
                &mut action_points,
                &mut pending_boosts,
                &dbs,
                &mut event_writer,
            );
        }
        BattleIntent::EndTurn => {
            turn_ctx.enemy_end_requested = true;
        }
    }
}

fn apply_remote_card(
    card_index: usize,
    discard: bool,
    hand: &mut Hand,
    action_points: &mut crate::battle::ActionPoints,
    pending_boosts: &mut PendingBoosts,
    dbs: &BattleDbs,
    event_writer: &mut MessageWriter<BattleEvent>,
) {
    if card_index >= hand.enemy.len() {
        return;
    }
    let card_id = hand.enemy.remove(card_index);
    let card_name = dbs
        .cards
        .get(&card_id)
        .map(|card| card.name.clone())
        .unwrap_or_else(|| format!("{card_id:?}"));
    if discard {
        action_points.enemy += 1;
        event_writer.write(BattleEvent::CardDiscarded {
            side: Side::Enemy,
            card_name,
        });
        return;
    }
    let Some(card) = dbs.cards.get(&card_id) else {
        return;
    };
    if action_points.enemy < card.cost_ap {
        return;
    }
    action_points.enemy -= card.cost_ap;
    match card.effect {
        CardEffect::GainAp { amount } => action_points.enemy += amount,
        CardEffect::NextAttackBoost { amount } => pending_boosts.enemy.next_attack_bonus += amount,
        CardEffect::NextShieldBoost { amount } => pending_boosts.enemy.next_shield_bonus += amount,
        CardEffect::NextHealBoost { amount } => pending_boosts.enemy.next_heal_bonus += amount,
    }
    event_writer.write(BattleEvent::CardUsed {
        side: Side::Enemy,
        card_name,
    });
}

fn pvp_handle_battle_disconnect_system(
    mut connection: ResMut<PvpConnection>,
    battle_mode: Res<BattleControlMode>,
    game_state: Res<State<GameState>>,
    mut battle_log: ResMut<BattleLog>,
    mut battle_result: ResMut<BattleResult>,
    mut next_state: ResMut<NextState<GameState>>,
    mut event_writer: MessageWriter<BattleEvent>,
) {
    if *game_state.get() != GameState::Battle || *battle_mode != BattleControlMode::PlayerVsRemote {
        return;
    }
    let reason = match &connection.status {
        PvpStatus::Disconnected(reason) => Some(reason.clone()),
        PvpStatus::Failed(reason) if connection.role.is_some() => Some(reason.clone()),
        _ => None,
    };
    let Some(reason) = reason else {
        return;
    };
    event_writer.write(BattleEvent::NetworkInterrupted {
        reason: reason.clone(),
    });
    let message = format!("联机中断：{reason}");
    battle_result.message = message.clone();
    push_battle_line(&mut battle_log, message);
    connection.stop();
    connection.status = PvpStatus::Idle;
    next_state.set(GameState::Result);
}
