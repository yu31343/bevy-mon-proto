use std::{
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
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use serde::{Deserialize, Serialize};

use crate::{
    battle::{
        ActionPoints, BattleControlMode, BattleEvent, BattleLog, BattleResult, EnemyTeam, Hand,
        InBattle, PendingBoosts, PlayerTeam, PvpTurnOrder, RoundOrder, SelectedCards, Side, Stats,
        StatusBoard, TurnAction, TurnContext, TurnCount, push_battle_line, transfer_status_by_id,
    },
    data::{BattleDbs, CardDef, CardEffect, CardId, MonsterPool, SkillDef, TeamSelections},
    game_state::{BattlePhase, GameState},
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
            .init_resource::<PvpIncomingSnapshots>()
            .add_systems(Update, pvp_poll_network_system)
            .add_systems(
                Update,
                setup_pvp_lobby_ui.run_if(
                    in_state(GameState::PvpLobby)
                        .and(resource_exists::<crate::ui::battle::theme::UiTheme>),
                ),
            )
            .add_systems(
                Update,
                pvp_lobby_button_system.run_if(in_state(GameState::PvpLobby)),
            )
            .add_systems(
                Update,
                pvp_lobby_button_visual_system.run_if(in_state(GameState::PvpLobby)),
            )
            .add_systems(
                Update,
                update_pvp_lobby_ui_system.run_if(in_state(GameState::PvpLobby)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                pvp_address_text_box_system.run_if(in_state(GameState::PvpLobby)),
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
            .add_systems(Update, pvp_send_host_snapshot_system)
            .add_systems(Update, pvp_apply_host_snapshot_system)
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
    pub screen: PvpLobbyScreen,
    egui_font_registered: bool,
}

impl Default for PvpLobbyInput {
    fn default() -> Self {
        Self {
            address: format!("127.0.0.1:{DEFAULT_PORT}"),
            info: "选择建房或加入，开始联机对战。".to_string(),
            screen: PvpLobbyScreen::Menu,
            egui_font_registered: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PvpLobbyScreen {
    #[default]
    Menu,
    HostRoom,
    JoinAddress,
}

#[derive(Resource, Debug, Default)]
pub struct PvpTeamState {
    pub local_indices: Option<Vec<usize>>,
    pub remote_indices: Option<Vec<usize>>,
    pub battle_started: bool,
}

#[derive(Resource, Debug, Default)]
pub struct PvpIncomingIntents(pub Vec<BattleIntent>);

#[derive(Resource, Debug, Default)]
struct PvpIncomingSnapshots(Vec<PvpBattleSnapshot>);

#[derive(Component)]
pub struct PvpLobbyUiRoot;

#[derive(Component)]
pub struct PvpHostButton;

#[derive(Component)]
pub struct PvpJoinButton;

#[derive(Component)]
pub struct PvpConfirmJoinButton;

#[derive(Component)]
pub struct PvpBackButton;

#[derive(Component)]
pub struct PvpAddressText;

#[derive(Component)]
pub struct PvpStatusText;

#[derive(Component)]
pub struct PvpScreenTitleText;

#[derive(Component)]
pub struct PvpScreenHintText;

#[derive(Component)]
pub struct PvpRoomInfoText;

#[derive(Component)]
pub struct PvpLobbyButtonVisual;

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
    BattleSnapshot(PvpBattleSnapshot),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PvpBattleSnapshot {
    turn: u32,
    phase: BattlePhase,
    first_side: Side,
    player_active_index: usize,
    enemy_active_index: usize,
    player_hp: Vec<i32>,
    enemy_hp: Vec<i32>,
    player_ap: i32,
    enemy_ap: i32,
    player_hand: Vec<CardId>,
    enemy_hand: Vec<CardId>,
    player_defeated: bool,
    enemy_defeated: bool,
    result_message: Option<String>,
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
        self.stop_with_leave(None);
    }

    pub fn stop_with_leave(&mut self, reason: Option<String>) {
        if let Some(tx) = self.command_tx.take() {
            if let Some(reason) = reason {
                let _ = tx.send(NetCommand::Send(PvpMessage::Leave { reason }));
            }
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
    connection.seq = 0;
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
    connection.seq = 0;
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
    let mut parts = Vec::new();

    let mut skills = dbs.skills.values().collect::<Vec<_>>();
    skills.sort_by_key(|skill| format!("{:?}", skill.id));
    for skill in skills {
        parts.push(skill_hash_part(skill));
    }

    let mut cards = dbs.cards.values().collect::<Vec<_>>();
    cards.sort_by_key(|card| format!("{:?}", card.id));
    for card in cards {
        parts.push(card_hash_part(card));
    }

    for (index, monster) in monsters.monsters.iter().enumerate() {
        parts.push(format!("monster:{index}:{monster:?}"));
    }

    stable_hash(&parts.join("\n"))
}

fn skill_hash_part(skill: &SkillDef) -> String {
    format!(
        "skill:{:?}:{}:{:?}:{}:{:?}:{:?}:{:?}",
        skill.id,
        skill.name,
        skill.category,
        skill.cost_ap,
        skill.effect,
        skill.element,
        skill.base_accuracy
    )
}

fn card_hash_part(card: &CardDef) -> String {
    format!(
        "card:{:?}:{}:{}:{:?}",
        card.id, card.name, card.cost_ap, card.effect
    )
}

fn stable_hash(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn host_thread(command_rx: Receiver<NetCommand>, event_tx: Sender<NetEvent>) {
    let mut last_error = None;
    for offset in 0..MAX_PORT_ATTEMPTS {
        let port = DEFAULT_PORT.saturating_add(offset);
        match TcpListener::bind(("0.0.0.0", port)) {
            Ok(listener) => {
                let _ = listener.set_nonblocking(true);
                let _ = event_tx.send(NetEvent::Listening(port));
                loop {
                    if matches!(command_rx.try_recv(), Ok(NetCommand::Stop)) {
                        return;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => {
                            run_stream(stream, command_rx, event_tx);
                            return;
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(16));
                        }
                        Err(err) => {
                            let _ = event_tx.send(NetEvent::Failed(format!("接受连接失败：{err}")));
                            return;
                        }
                    }
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                last_error = Some(err.to_string());
            }
            Err(err) => {
                let _ = event_tx.send(NetEvent::Failed(format!("建房失败：{err}")));
                return;
            }
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
    let mut read_buffer = Vec::new();

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

        match read_messages(&mut stream, &mut read_buffer) {
            Ok((messages, received_bytes)) => {
                if received_bytes {
                    last_rx = Instant::now();
                }
                for message in messages {
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
            }
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
    let mut framed = Vec::with_capacity(4 + bytes.len());
    framed.extend_from_slice(&len.to_be_bytes());
    framed.extend_from_slice(bytes);
    write_all_nonblocking(stream, &framed)
}

fn write_all_nonblocking(stream: &mut TcpStream, bytes: &[u8]) -> std::io::Result<()> {
    let start = Instant::now();
    let mut written = 0;
    while written < bytes.len() {
        match stream.write(&bytes[written..]) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "连接已关闭",
                ));
            }
            Ok(count) => written += count,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() >= HEARTBEAT_TIMEOUT {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "发送超时",
                    ));
                }
                thread::sleep(Duration::from_millis(1));
            }
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn read_messages(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
) -> std::io::Result<(Vec<PvpMessage>, bool)> {
    let mut temp = [0_u8; 4096];
    let mut received_bytes = false;
    loop {
        match stream.read(&mut temp) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "对方已关闭连接",
                ));
            }
            Ok(count) => {
                received_bytes = true;
                buffer.extend_from_slice(&temp[..count]);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(err) => return Err(err),
        }
    }

    let mut messages = Vec::new();
    loop {
        if buffer.len() < 4 {
            break;
        }
        let len = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
        if len > 64 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "消息过长",
            ));
        }
        if buffer.len() < 4 + len {
            break;
        }
        let payload = buffer[4..4 + len].to_vec();
        buffer.drain(..4 + len);
        let text = String::from_utf8(payload)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
        let message = ron::from_str(&text)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
        messages.push(message);
    }
    Ok((messages, received_bytes))
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
        input.info = "选择建房或加入，开始联机对战。".to_string();
        input.screen = PvpLobbyScreen::Menu;
    }
}

fn clear_pending_error_on_exit(mut input: ResMut<PvpLobbyInput>) {
    input.info.clear();
}

fn reset_session_state(team_state: &mut PvpTeamState, incoming_intents: &mut PvpIncomingIntents) {
    *team_state = PvpTeamState::default();
    incoming_intents.0.clear();
}

fn cleanup_pvp_lobby_ui(mut commands: Commands, query: Query<Entity, With<PvpLobbyUiRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn setup_pvp_lobby_ui(
    mut commands: Commands,
    existing: Query<(), With<PvpLobbyUiRoot>>,
    theme: Res<crate::ui::battle::theme::UiTheme>,
    ui_font: Option<Res<crate::ui::battle::resources::UiFontHandle>>,
) {
    if !existing.is_empty() {
        return;
    }
    let title_font = make_text_font(42.0, ui_font.as_deref());
    let subtitle_font = make_text_font(18.0, ui_font.as_deref());
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
                row_gap: Val::Px(18.0),
                ..default()
            },
            theme.root_background(),
            PvpLobbyUiRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("联机对战"),
                title_font,
                TextColor(theme.text_primary),
                theme.title_text_shadow(),
            ));
            root.spawn((
                Text::new("选择建房或加入对方房间"),
                subtitle_font,
                TextColor(theme.text_secondary),
                PvpScreenTitleText,
            ));
            root.spawn((
                Node {
                    width: Val::Px(520.0),
                    min_height: Val::Px(220.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(14.0),
                    padding: UiRect::all(Val::Px(22.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_panel),
                    ..default()
                },
                BackgroundColor(theme.panel),
                BorderColor::all(theme.border_panel),
                theme.panel_shadow(),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new(""),
                    body_font.clone(),
                    TextColor(theme.text_primary),
                    PvpRoomInfoText,
                ));
                panel.spawn((
                    Text::new(""),
                    body_font.clone(),
                    TextColor(theme.text_primary),
                    PvpAddressText,
                ));
                panel.spawn((
                    Text::new(""),
                    small_font.clone(),
                    TextColor(theme.text_secondary),
                    PvpStatusText,
                ));
            });
            root.spawn((Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                ..default()
            },))
                .with_children(|row| {
                    spawn_pvp_button(row, "建房", body_font.clone(), PvpHostButton, &theme);
                    spawn_pvp_button(row, "加入", body_font.clone(), PvpJoinButton, &theme);
                    spawn_pvp_button(
                        row,
                        "确认加入",
                        body_font.clone(),
                        PvpConfirmJoinButton,
                        &theme,
                    );
                    spawn_pvp_button(row, "返回", body_font, PvpBackButton, &theme);
                });
            root.spawn((
                Text::new(""),
                small_font,
                TextColor(theme.text_muted),
                PvpScreenHintText,
            ));
        });
}

fn spawn_pvp_button<T: Component>(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    font: TextFont,
    marker: T,
    theme: &crate::ui::battle::theme::UiTheme,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(140.0),
                min_height: Val::Px(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(theme.radius_button),
                ..default()
            },
            BackgroundColor(theme.button_idle),
            BorderColor::all(theme.button_border_idle),
            theme.button_shadow(),
            marker,
            PvpLobbyButtonVisual,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label.to_string()),
                font,
                TextColor(theme.text_primary),
            ));
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
    mut confirm_join_buttons: Query<
        &Interaction,
        (Changed<Interaction>, With<PvpConfirmJoinButton>),
    >,
    mut back_buttons: Query<&Interaction, (Changed<Interaction>, With<PvpBackButton>)>,
    mut connection: ResMut<PvpConnection>,
    mut team_state: ResMut<PvpTeamState>,
    mut incoming_intents: ResMut<PvpIncomingIntents>,
    mut input: ResMut<PvpLobbyInput>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in &mut host_buttons {
        if *interaction == Interaction::Pressed && input.screen == PvpLobbyScreen::Menu {
            reset_session_state(&mut team_state, &mut incoming_intents);
            input.screen = PvpLobbyScreen::HostRoom;
            input.info = "正在建房...".to_string();
            start_host(&mut connection);
            return;
        }
    }
    for interaction in &mut join_buttons {
        if *interaction == Interaction::Pressed && input.screen == PvpLobbyScreen::Menu {
            input.screen = PvpLobbyScreen::JoinAddress;
            input.info = "输入对方 IP 或域名与端口，然后点击确认加入。".to_string();
            return;
        }
    }
    for interaction in &mut confirm_join_buttons {
        if *interaction == Interaction::Pressed && input.screen == PvpLobbyScreen::JoinAddress {
            submit_join_address(
                &mut input,
                &mut connection,
                &mut team_state,
                &mut incoming_intents,
            );
            return;
        }
    }
    for interaction in &mut back_buttons {
        if *interaction == Interaction::Pressed {
            if input.screen != PvpLobbyScreen::Menu {
                connection.stop_with_leave(Some("对方已返回联机菜单，房间已关闭。".to_string()));
                reset_session_state(&mut team_state, &mut incoming_intents);
                connection.status = PvpStatus::Idle;
                input.screen = PvpLobbyScreen::Menu;
                input.info = "选择建房或加入，开始联机对战。".to_string();
            } else {
                connection.stop_with_leave(Some("对方已返回大厅，联机已取消。".to_string()));
                reset_session_state(&mut team_state, &mut incoming_intents);
                connection.status = PvpStatus::Idle;
                next_state.set(GameState::Lobby);
            }
            return;
        }
    }
}

fn submit_join_address(
    input: &mut PvpLobbyInput,
    connection: &mut PvpConnection,
    team_state: &mut PvpTeamState,
    incoming_intents: &mut PvpIncomingIntents,
) {
    let address = input.address.trim().to_string();
    if valid_address_like(&address) {
        reset_session_state(team_state, incoming_intents);
        input.info = format!("正在连接 {address} ...");
        start_client(connection, address);
    } else {
        input.info = "地址格式应为 ip:端口 或 域名:端口。".to_string();
    }
}

fn update_pvp_lobby_ui_system(
    connection: Res<PvpConnection>,
    input: Res<PvpLobbyInput>,
    mut texts: ParamSet<(
        Query<&mut Text, With<PvpScreenTitleText>>,
        Query<&mut Text, With<PvpScreenHintText>>,
        Query<&mut Text, With<PvpRoomInfoText>>,
        Query<&mut Text, With<PvpAddressText>>,
        Query<&mut Text, With<PvpStatusText>>,
    )>,
) {
    let (title, hint, room, address_label) = match input.screen {
        PvpLobbyScreen::Menu => (
            "选择联机方式",
            "建房会显示房间地址；加入会进入地址输入界面。",
            format!("本机局域网 IP：{}", connection.local_ip),
            "等待选择".to_string(),
        ),
        PvpLobbyScreen::HostRoom => {
            let address = match connection.status {
                PvpStatus::Hosting { port } => format!("{}:{port}", connection.local_ip),
                _ => format!("{}:{DEFAULT_PORT}", connection.local_ip),
            };
            (
                "房间信息",
                "把房间地址发给对方；连接成功后会自动进入配队。",
                "已建房，等待对方加入".to_string(),
                address,
            )
        }
        PvpLobbyScreen::JoinAddress => (
            "加入房间",
            "输入示例：127.0.0.1:42043；输入框支持鼠标选中、光标移动和复制粘贴。",
            "对方地址".to_string(),
            "".to_string(),
        ),
    };
    for mut text in &mut texts.p0() {
        **text = title.to_string();
    }
    for mut text in &mut texts.p1() {
        **text = hint.to_string();
    }
    for mut text in &mut texts.p2() {
        **text = room.clone();
    }
    for mut text in &mut texts.p3() {
        **text = address_label.clone();
    }
    let status = match &connection.status {
        PvpStatus::Idle => input.info.clone(),
        PvpStatus::Hosting { port } => format!("端口：{port}。{}", input.info),
        PvpStatus::Connecting => input.info.clone(),
        PvpStatus::Connected => input.info.clone(),
        PvpStatus::Failed(reason) => reason.clone(),
        PvpStatus::Disconnected(reason) => reason.clone(),
    };
    for mut text in &mut texts.p4() {
        **text = status.clone();
    }
}

fn pvp_lobby_button_visual_system(
    theme: Res<crate::ui::battle::theme::UiTheme>,
    input: Res<PvpLobbyInput>,
    mut buttons: Query<
        (
            &Interaction,
            &mut Visibility,
            &mut BackgroundColor,
            &mut BorderColor,
            Option<&PvpHostButton>,
            Option<&PvpJoinButton>,
            Option<&PvpConfirmJoinButton>,
            Option<&PvpBackButton>,
        ),
        With<PvpLobbyButtonVisual>,
    >,
) {
    for (interaction, mut visibility, mut bg, mut border, host, join, confirm, _back) in
        &mut buttons
    {
        let hidden = match input.screen {
            PvpLobbyScreen::Menu => confirm.is_some(),
            PvpLobbyScreen::HostRoom => host.is_some() || join.is_some() || confirm.is_some(),
            PvpLobbyScreen::JoinAddress => host.is_some() || join.is_some(),
        };
        *visibility = if hidden {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        if hidden {
            continue;
        }
        *bg = match *interaction {
            Interaction::Pressed => BackgroundColor(theme.button_pressed),
            Interaction::Hovered => BackgroundColor(theme.button_hover),
            Interaction::None => BackgroundColor(theme.button_idle),
        };
        *border = match *interaction {
            Interaction::Pressed => BorderColor::all(theme.button_border_pressed),
            Interaction::Hovered => BorderColor::all(theme.button_border_hover),
            Interaction::None => BorderColor::all(theme.button_border_idle),
        };
    }
}

fn pvp_address_text_box_system(
    mut contexts: EguiContexts,
    mut input: ResMut<PvpLobbyInput>,
    mut connection: ResMut<PvpConnection>,
    mut team_state: ResMut<PvpTeamState>,
    mut incoming_intents: ResMut<PvpIncomingIntents>,
) -> Result {
    if input.screen != PvpLobbyScreen::JoinAddress {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    register_egui_cjk_font(ctx, &mut input);
    egui::Area::new(egui::Id::new("pvp_address_text_box"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 3.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let mut style = (*ctx.style()).clone();
            style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(20, 36, 61);
            style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(77, 140, 209, 180),
            );
            style.visuals.widgets.active.bg_stroke = egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(115, 184, 245, 230),
            );
            ui.set_style(style);
            ui.set_width(360.0);
            let response = ui.add(
                egui::TextEdit::singleline(&mut input.address)
                    .hint_text("ip/域名:端口")
                    .desired_width(360.0)
                    .font(egui::TextStyle::Heading),
            );
            response.request_focus();
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                submit_join_address(
                    &mut input,
                    &mut connection,
                    &mut team_state,
                    &mut incoming_intents,
                );
            }
        });
    Ok(())
}

fn register_egui_cjk_font(ctx: &egui::Context, input: &mut PvpLobbyInput) {
    if input.egui_font_registered {
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
    input.egui_font_registered = true;
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
    mut incoming_snapshots: ResMut<PvpIncomingSnapshots>,
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
                PvpMessage::BattleSnapshot(snapshot) => incoming_snapshots.0.push(snapshot),
                PvpMessage::Intent { intent, .. } => incoming_intents.0.push(intent),
                PvpMessage::Surrender => {
                    connection.status = PvpStatus::Disconnected("对方已撤退/战斗中止".to_string());
                }
                PvpMessage::Leave { reason } => {
                    input.info = reason.clone();
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
    commands.insert_resource(PvpTurnOrder {
        local_first: connection.role == Some(PvpRole::Host),
    });
    commands.insert_resource(TeamSelections {
        player_indices: local_indices,
        enemy_indices: remote_indices,
    });
    team_state.battle_started = true;
    next_state.set(GameState::Battle);
}

fn team_hp(team: &crate::battle::Team, query: &Query<&Stats, With<InBattle>>) -> Vec<i32> {
    team.combatants
        .iter()
        .map(|&entity| query.get(entity).map(|stats| stats.hp).unwrap_or(0))
        .collect()
}

fn apply_team_hp(
    team: &crate::battle::Team,
    hp_values: &[i32],
    query: &mut Query<&mut Stats, With<InBattle>>,
) {
    for (&entity, hp) in team.combatants.iter().zip(hp_values.iter().copied()) {
        if let Ok(mut stats) = query.get_mut(entity) {
            stats.hp = hp.clamp(0, stats.max_hp);
        }
    }
}

fn mirror_side(side: Side) -> Side {
    match side {
        Side::Player => Side::Enemy,
        Side::Enemy => Side::Player,
    }
}

fn mirror_result_message(message: &str) -> String {
    if message.starts_with("胜利！") {
        message.replacen("胜利！", "失败！", 1)
    } else if message.starts_with("失败！") {
        message.replacen("失败！", "胜利！", 1)
    } else {
        message.to_string()
    }
}

fn pvp_send_host_snapshot_system(
    connection: Res<PvpConnection>,
    battle_mode: Res<BattleControlMode>,
    game_state: Res<State<GameState>>,
    battle_phase: Res<State<BattlePhase>>,
    turn_count: Res<TurnCount>,
    round_order: Res<RoundOrder>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    hand: Res<Hand>,
    action_points: Res<ActionPoints>,
    battle_result: Res<BattleResult>,
    stats_query: Query<&Stats, With<InBattle>>,
) {
    if *battle_mode != BattleControlMode::PlayerVsRemote
        || connection.role != Some(PvpRole::Host)
        || !matches!(*game_state.get(), GameState::Battle | GameState::Result)
    {
        return;
    }
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };
    let player_hp = team_hp(&player_team.0, &stats_query);
    let enemy_hp = team_hp(&enemy_team.0, &stats_query);
    let host_player_defeated = !player_hp.iter().any(|hp| *hp > 0);
    let host_enemy_defeated = !enemy_hp.iter().any(|hp| *hp > 0);
    connection.send(PvpMessage::BattleSnapshot(PvpBattleSnapshot {
        turn: turn_count.0,
        phase: *battle_phase.get(),
        first_side: round_order.first,
        player_active_index: enemy_team.0.active_index,
        enemy_active_index: player_team.0.active_index,
        player_hp: enemy_hp,
        enemy_hp: player_hp,
        player_ap: action_points.enemy,
        enemy_ap: action_points.player,
        player_hand: hand.enemy.clone(),
        enemy_hand: hand.player.clone(),
        player_defeated: host_enemy_defeated,
        enemy_defeated: host_player_defeated,
        result_message: (!battle_result.message.is_empty())
            .then(|| mirror_result_message(&battle_result.message)),
    }));
}

fn pvp_apply_host_snapshot_system(
    mut incoming: ResMut<PvpIncomingSnapshots>,
    connection: Res<PvpConnection>,
    battle_mode: Res<BattleControlMode>,
    battle_phase: Res<State<BattlePhase>>,
    mut turn_count: ResMut<TurnCount>,
    mut round_order: ResMut<RoundOrder>,
    mut player_team: Option<ResMut<PlayerTeam>>,
    mut enemy_team: Option<ResMut<EnemyTeam>>,
    mut hand: ResMut<Hand>,
    mut action_points: ResMut<ActionPoints>,
    mut battle_result: ResMut<BattleResult>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut stats_query: Query<&mut Stats, With<InBattle>>,
) {
    if *battle_mode != BattleControlMode::PlayerVsRemote || connection.role != Some(PvpRole::Client)
    {
        incoming.0.clear();
        return;
    }
    let Some(snapshot) = incoming.0.pop() else {
        return;
    };
    incoming.0.clear();

    turn_count.0 = snapshot.turn;
    round_order.set_first(mirror_side(snapshot.first_side));
    *hand = Hand {
        player: snapshot.player_hand,
        enemy: snapshot.enemy_hand,
    };
    action_points.player = snapshot.player_ap;
    action_points.enemy = snapshot.enemy_ap;

    if let Some(player_team) = player_team.as_mut() {
        player_team.0.active_index = snapshot
            .player_active_index
            .min(player_team.0.combatants.len().saturating_sub(1));
        apply_team_hp(&player_team.0, &snapshot.player_hp, &mut stats_query);
    }
    if let Some(enemy_team) = enemy_team.as_mut() {
        enemy_team.0.active_index = snapshot
            .enemy_active_index
            .min(enemy_team.0.combatants.len().saturating_sub(1));
        apply_team_hp(&enemy_team.0, &snapshot.enemy_hp, &mut stats_query);
    }

    if let Some(message) = snapshot.result_message {
        battle_result.message = message;
        next_state.set(GameState::Result);
    } else if snapshot.player_defeated || snapshot.enemy_defeated {
        battle_result.message = if snapshot.player_defeated && snapshot.enemy_defeated {
            "平局！按 R 返回大厅。".to_string()
        } else if snapshot.enemy_defeated {
            "胜利！全歼敌方。按 R 返回大厅。".to_string()
        } else {
            "失败！队伍全灭。按 R 返回大厅。".to_string()
        };
        next_state.set(GameState::Result);
    } else {
        let mirrored_phase = host_phase_for_local_phase(snapshot.phase);
        if mirrored_phase != *battle_phase.get() {
            next_phase.set(mirrored_phase);
        }
    }
}

fn host_phase_for_local_phase(phase: BattlePhase) -> BattlePhase {
    match phase {
        BattlePhase::PlayerTurn => BattlePhase::EnemyTurn,
        BattlePhase::EnemyTurn => BattlePhase::PlayerTurn,
        other => other,
    }
}

fn pvp_apply_remote_intents_system(
    mut incoming: ResMut<PvpIncomingIntents>,
    connection: Res<PvpConnection>,
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    mut turn_ctx: ResMut<TurnContext>,
    mut selected: ResMut<SelectedCards>,
    mut hand: ResMut<Hand>,
    mut action_points: ResMut<crate::battle::ActionPoints>,
    mut pending_boosts: ResMut<PendingBoosts>,
    mut enemy_team: Option<ResMut<crate::battle::EnemyTeam>>,
    skill_query: Query<&crate::battle::SkillList, With<crate::battle::InBattle>>,
    mut combat_query: Query<(&mut Stats, &Name, &mut StatusBoard), With<crate::battle::InBattle>>,
    dbs: Res<BattleDbs>,
    mut event_writer: MessageWriter<BattleEvent>,
) {
    if *battle_mode != BattleControlMode::PlayerVsRemote
        || connection.role != Some(PvpRole::Host)
        || *battle_phase.get() != BattlePhase::EnemyTurn
    {
        return;
    }
    let Some(intent) = incoming.0.first().cloned() else {
        return;
    };
    incoming.0.remove(0);
    match intent {
        BattleIntent::UseSkill { slot } => {
            let Some(enemy_team) = enemy_team.as_ref() else {
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
            if let Some(enemy_team) = enemy_team.as_mut() {
                apply_remote_switch(
                    target_index,
                    enemy_team,
                    &mut action_points,
                    &mut combat_query,
                    &mut event_writer,
                );
            }
            selected.enemy.index = None;
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

pub fn pvp_host_first_side(
    player_spd: i32,
    enemy_spd: i32,
    local_is_host: bool,
    previous_first: Option<Side>,
) -> Side {
    let host_spd = if local_is_host { player_spd } else { enemy_spd };
    let client_spd = if local_is_host { enemy_spd } else { player_spd };
    let host_was_previous_first = previous_first
        .map(|side| side == Side::Player && local_is_host || side == Side::Enemy && !local_is_host);
    let host_first = if host_spd > client_spd {
        true
    } else if client_spd > host_spd {
        false
    } else {
        host_was_previous_first
            .map(|was_first| !was_first)
            .unwrap_or(true)
    };
    if host_first == local_is_host {
        Side::Player
    } else {
        Side::Enemy
    }
}

fn apply_remote_switch(
    target_index: usize,
    enemy_team: &mut crate::battle::EnemyTeam,
    action_points: &mut crate::battle::ActionPoints,
    combat_query: &mut Query<(&mut Stats, &Name, &mut StatusBoard), With<crate::battle::InBattle>>,
    event_writer: &mut MessageWriter<BattleEvent>,
) {
    if action_points.enemy < 1
        || target_index >= enemy_team.0.combatants.len()
        || target_index == enemy_team.0.active_index
    {
        return;
    }
    let current_entity = enemy_team.0.combatants[enemy_team.0.active_index];
    let target_entity = enemy_team.0.combatants[target_index];
    let Ok(
        [
            (mut current_stats, _, mut current_statuses),
            (target_stats, name, target_statuses),
        ],
    ) = combat_query.get_many_mut([current_entity, target_entity])
    else {
        return;
    };
    if target_stats.hp <= 0 {
        return;
    }

    transfer_status_by_id(
        &mut current_statuses,
        &mut current_stats,
        target_statuses.into_inner(),
        target_stats.into_inner(),
        "nature_regen",
    );
    action_points.enemy -= 1;
    enemy_team.0.active_index = target_index;
    event_writer.write(BattleEvent::Switched {
        side: Side::Enemy,
        name: name.to_string(),
    });
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
    let card_id = hand.enemy[card_index];
    let card_name = dbs
        .cards
        .get(&card_id)
        .map(|card| card.name.clone())
        .unwrap_or_else(|| format!("{card_id:?}"));
    if discard {
        hand.enemy.remove(card_index);
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
    hand.enemy.remove(card_index);
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
