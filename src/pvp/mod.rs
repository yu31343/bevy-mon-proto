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

use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use serde::{Deserialize, Serialize};

use crate::{
    battle::{
        ActionPoints, BattleControlMode, BattleEvent, BattleResult, BattleResultAction,
        BattleResultNotice, BattleShuffleSeed, DamageType, ElementAura, EnemyTeam, Hand, InBattle,
        PendingBattleResultAction, PendingBoosts, PendingHandDiscard, PendingKoResolution,
        PlayerTeam, PvpTurnOrder, RoundOrder, SelectedCards, Shield, Side, Stats, StatusBoard,
        StatusInstance, TurnAction, TurnContext, TurnCount, new_battle_shuffle_seed,
        recalculate_stage_modifiers, transfer_status_by_id,
    },
    console_log::{ConsoleLogCategory, log as console_log},
    data::{
        BattleDbs, BattleFormulaRules, BattleRules, CardDeck, CardDef, CardId, MonsterPool,
        SkillDef, TeamSelections,
    },
    game_state::{BattlePhase, GameState},
};

const DEFAULT_PORT: u16 = 42043;
const MAX_PORT_ATTEMPTS: u16 = 32;
const PROTOCOL_VERSION: u32 = 5;
const RELAY_PROTOCOL_VERSION: u32 = 1;
const MAX_FRAME_LEN: usize = 64 * 1024;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(8);
const NETWORK_IDLE_SLEEP: Duration = Duration::from_millis(1);
const PVP_SNAPSHOT_INTERVAL: Duration = Duration::from_millis(50);

pub struct PvpPlugin;

impl Plugin for PvpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PvpConnection>()
            .init_resource::<PvpLobbyInput>()
            .init_resource::<PvpTeamState>()
            .init_resource::<PvpRematchState>()
            .init_resource::<PvpIncomingIntents>()
            .init_resource::<PvpIncomingSnapshots>()
            .init_resource::<PvpIncomingFeedbacks>()
            .init_resource::<PvpPendingLocalIntent>()
            .init_resource::<PvpLastRemoteIntentSeq>()
            .init_resource::<PvpSnapshotSync>()
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
            .add_systems(Update, pvp_forward_host_battle_events_system)
            .add_systems(Update, pvp_send_host_snapshot_system)
            .add_systems(Update, pvp_apply_host_snapshot_system)
            .add_systems(Update, pvp_handle_battle_disconnect_system)
            .add_systems(
                Update,
                pvp_result_rematch_system.run_if(in_state(GameState::Result)),
            )
            .add_systems(OnEnter(GameState::Lobby), reset_pvp_state_on_lobby)
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
    ConnectingRelay,
    WaitingRelayPeer { room_code: String },
    JoiningRelayRoom { room_code: String },
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
    /// 最近一次心跳测得的往返延迟（毫秒）；未连接或尚无读数时为 None。
    pub latency_ms: Option<u32>,
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
            latency_ms: None,
        }
    }
}

#[derive(Resource, Debug)]
pub struct PvpLobbyInput {
    pub address: String,
    pub relay_address: String,
    pub room_code: String,
    pub info: String,
    pub screen: PvpLobbyScreen,
    egui_font_registered: bool,
}

impl Default for PvpLobbyInput {
    fn default() -> Self {
        Self {
            address: format!("127.0.0.1:{DEFAULT_PORT}"),
            relay_address: format!("127.0.0.1:{DEFAULT_PORT}"),
            room_code: String::new(),
            info: "选择局域网联机或服务器联机。".to_string(),
            screen: PvpLobbyScreen::Menu,
            egui_font_registered: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PvpLobbyScreen {
    #[default]
    Menu,
    DirectMenu,
    RelayMenu,
    HostRoom,
    JoinAddress,
    RelayHostRoom,
    RelayJoinRoom,
}

#[derive(Resource, Debug, Default)]
pub struct PvpTeamState {
    pub local_indices: Option<Vec<usize>>,
    pub remote_indices: Option<Vec<usize>>,
    pub battle_started: bool,
    pub battle_seed: Option<u64>,
    pub session_id: u64,
}

#[derive(Resource, Debug, Default)]
pub struct PvpRematchState {
    pub incoming_request: bool,
    pub outgoing_request: bool,
    pub accepted: bool,
}

#[derive(Resource, Debug, Default)]
pub struct PvpIncomingIntents(pub Vec<(u32, BattleIntent)>);

#[derive(Resource, Debug, Default)]
struct PvpIncomingSnapshots(Vec<PvpBattleSnapshot>);

#[derive(Resource, Debug, Default)]
struct PvpIncomingFeedbacks(Vec<PvpBattleFeedback>);

#[derive(Resource, Debug, Default)]
pub struct PvpPendingLocalIntent(pub Option<u32>);

#[derive(Resource, Debug, Default)]
pub struct PvpLastRemoteIntentSeq(pub u32);

#[derive(Resource, Debug)]
struct PvpSnapshotSync {
    interval: Timer,
    last_sent: Option<PvpBattleSnapshot>,
}

impl Default for PvpSnapshotSync {
    fn default() -> Self {
        Self {
            interval: Timer::new(PVP_SNAPSHOT_INTERVAL, TimerMode::Repeating),
            last_sent: None,
        }
    }
}

impl PvpSnapshotSync {
    fn reset(&mut self) {
        self.interval.reset();
        self.last_sent = None;
    }
}

#[derive(Component)]
pub struct PvpLobbyUiRoot;

#[derive(Component)]
pub struct PvpHostButton;

#[derive(Component)]
pub struct PvpJoinButton;

#[derive(Component)]
pub struct PvpRelayHostButton;

#[derive(Component)]
pub struct PvpRelayJoinButton;

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
    Stop { reason: Option<String> },
}

#[derive(Debug)]
enum NetEvent {
    Listening(u16),
    RelayRoomCreated(String),
    RelayWaitingPeer(String),
    Connected,
    Message(PvpMessage),
    Latency(u32),
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
        session_id: u64,
        monster_indices: Vec<usize>,
    },
    BattleReady {
        session_id: u64,
        seed: u64,
    },
    BattleSnapshot(Box<PvpBattleSnapshot>),
    BattleFeedback(PvpBattleFeedback),
    Intent {
        seq: u32,
        intent: BattleIntent,
    },
    RematchRequest,
    RematchAccepted,
    RematchRejected,
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
enum PvpBattleFeedback {
    TurnStarted(u32),
    CardUsed {
        side: Side,
        card_name: String,
    },
    CardDiscarded {
        side: Side,
        card_name: String,
    },
    SkillUsed {
        side: Side,
        skill_name: String,
        slot: usize,
    },
    DamageDealt {
        source: Side,
        target: Side,
        amount: i32,
        damage_type: DamageType,
    },
    AttackMissed {
        source: Side,
        target: Side,
    },
    ShieldAbsorbed {
        side: Side,
        amount: i32,
    },
    Healed {
        side: Side,
        amount: i32,
    },
    ShieldGained {
        side: Side,
        amount: i32,
    },
    Switched {
        side: Side,
        name: String,
    },
    ReactionTriggered {
        source: Side,
        target: Side,
        reaction_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PvpPendingDiscardSnapshot {
    side: Side,
    next_phase: BattlePhase,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PvpBattleSnapshot {
    session_id: u64,
    battle_seed: u64,
    turn: u32,
    phase: BattlePhase,
    first_side: Side,
    player_active_index: usize,
    enemy_active_index: usize,
    player_hp: Vec<i32>,
    enemy_hp: Vec<i32>,
    player_shields: Vec<i32>,
    enemy_shields: Vec<i32>,
    player_auras: Vec<[Option<crate::data::ElementType>; 2]>,
    enemy_auras: Vec<[Option<crate::data::ElementType>; 2]>,
    player_statuses: Vec<Vec<StatusInstance>>,
    enemy_statuses: Vec<Vec<StatusInstance>>,
    player_skill_uses: Vec<[u8; 4]>,
    enemy_skill_uses: Vec<[u8; 4]>,
    player_ap: i32,
    enemy_ap: i32,
    player_hand: Vec<CardId>,
    enemy_hand: Vec<CardId>,
    pending_discard: Option<PvpPendingDiscardSnapshot>,
    acknowledged_intent_seq: u32,
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
            let _ = tx.send(NetCommand::Stop { reason });
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
    console_log(
        ConsoleLogCategory::Pvp,
        "[host] 开始局域网建房，等待监听端口...",
    );
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
    console_log(
        ConsoleLogCategory::Pvp,
        format!("[client] 正在连接局域网对局：{address}"),
    );
    thread::spawn(move || client_thread(address, command_rx, event_tx));
    connection.role = Some(PvpRole::Client);
    connection.status = PvpStatus::Connecting;
    connection.command_tx = Some(command_tx);
    connection.event_rx = Some(Mutex::new(event_rx));
    connection.seq = 0;
    connection.protocol_ready = false;
    connection.remote_data_hash = None;
}

pub fn start_relay_host(connection: &mut PvpConnection, relay_address: String) {
    connection.stop();
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    console_log(
        ConsoleLogCategory::Pvp,
        format!("[relay-host] 正在连接中继服务器：{relay_address}"),
    );
    thread::spawn(move || relay_host_thread(relay_address, command_rx, event_tx));
    connection.role = Some(PvpRole::Host);
    connection.status = PvpStatus::ConnectingRelay;
    connection.command_tx = Some(command_tx);
    connection.event_rx = Some(Mutex::new(event_rx));
    connection.seq = 0;
    connection.protocol_ready = false;
    connection.remote_data_hash = None;
}

pub fn start_relay_client(
    connection: &mut PvpConnection,
    relay_address: String,
    room_code: String,
) {
    connection.stop();
    let room_code = normalize_room_code(&room_code);
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    let thread_room_code = room_code.clone();
    console_log(
        ConsoleLogCategory::Pvp,
        format!(
            "[relay-client] 正在加入中继房间：{} @ {}",
            room_code, relay_address
        ),
    );
    thread::spawn(move || {
        relay_client_thread(relay_address, thread_room_code, command_rx, event_tx)
    });
    connection.role = Some(PvpRole::Client);
    connection.status = PvpStatus::JoiningRelayRoom { room_code };
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
        session_id: team_state.session_id,
        monster_indices: indices,
    });
}

pub fn send_intent(connection: &mut PvpConnection, intent: BattleIntent) {
    if !connection.is_connected() {
        return;
    }
    connection.seq = connection.seq.wrapping_add(1);
    console_log(
        ConsoleLogCategory::PvpDetail,
        format!("[intent-send] seq={} intent={:?}", connection.seq, intent),
    );
    connection.send(PvpMessage::Intent {
        seq: connection.seq,
        intent,
    });
}

pub fn send_local_intent(
    connection: &mut PvpConnection,
    pending: &mut PvpPendingLocalIntent,
    intent: BattleIntent,
) {
    send_intent(connection, intent);
    pending.0 = Some(connection.seq);
}

pub fn surrender(connection: &PvpConnection) {
    connection.send(PvpMessage::Surrender);
}

fn request_rematch(connection: &PvpConnection) {
    connection.send(PvpMessage::RematchRequest);
}

fn accept_rematch(connection: &PvpConnection) {
    connection.send(PvpMessage::RematchAccepted);
}

fn reject_rematch(connection: &PvpConnection) {
    connection.send(PvpMessage::RematchRejected);
}

pub fn data_hash(
    dbs: &BattleDbs,
    monsters: &MonsterPool,
    rules: &BattleRules,
    formulas: &BattleFormulaRules,
    deck: &CardDeck,
) -> String {
    let mut parts = Vec::new();

    parts.push(rules_hash_part(rules));
    parts.push(formulas_hash_part(formulas));

    let mut elements = dbs.elements.entries();
    elements.sort_by_key(|(attacker, defender, _)| format!("{attacker:?}:{defender:?}"));
    for (attacker, defender, multiplier) in elements {
        parts.push(format!("element:{attacker:?}:{defender:?}:{multiplier}"));
    }

    let mut statuses = dbs.statuses.statuses.values().collect::<Vec<_>>();
    statuses.sort_by_key(|status| status.id.as_str());
    for status in statuses {
        parts.push(format!("status:{status:?}"));
    }

    let mut reactions = dbs.reactions.reactions.iter().collect::<Vec<_>>();
    reactions.sort_by_key(|reaction| reaction.id.as_str());
    for reaction in reactions {
        parts.push(format!("reaction:{reaction:?}"));
    }

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

    parts.push(format!("deck:{:?}", deck.0));

    stable_hash(&parts.join("\n"))
}

fn rules_hash_part(rules: &BattleRules) -> String {
    format!(
        "rules:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        rules.max_team_size,
        rules.initial_cards,
        rules.cards_per_round,
        rules.ap_per_round,
        rules.max_ap,
        rules.max_retained_hand,
        rules.discard_ap_gain,
        rules.max_shield_hp_ratio,
        rules.default_skill_uses_per_turn
    )
}

fn formulas_hash_part(formulas: &BattleFormulaRules) -> String {
    format!(
        "formulas:{}:{}:{}:{}:{}:{}",
        formulas.attribute_stage_bounds.min,
        formulas.attribute_stage_bounds.max,
        formulas.accuracy.min,
        formulas.accuracy.max,
        formulas.accuracy.stage_step,
        formulas.damage.min_damage
    )
}

fn skill_hash_part(skill: &SkillDef) -> String {
    format!(
        "skill:{:?}:{}:{:?}:{}:{:?}:{:?}:{:?}:{:?}",
        skill.id,
        skill.name,
        skill.category,
        skill.cost_ap,
        skill.effect,
        skill.element,
        skill.base_accuracy,
        skill.uses_per_turn
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
                    if matches!(command_rx.try_recv(), Ok(NetCommand::Stop { .. })) {
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
    match connect_to_address(&address) {
        Ok(stream) => run_stream(stream, command_rx, event_tx),
        Err(reason) => {
            let _ = event_tx.send(NetEvent::Failed(reason));
        }
    }
}

fn relay_host_thread(
    relay_address: String,
    command_rx: Receiver<NetCommand>,
    event_tx: Sender<NetEvent>,
) {
    let mut stream = match connect_to_address(&relay_address) {
        Ok(stream) => stream,
        Err(reason) => {
            let _ = event_tx.send(NetEvent::Failed(format!("中继服务器连接失败：{reason}")));
            return;
        }
    };
    if let Err(err) = write_relay_command(&mut stream, "CREATE") {
        let _ = event_tx.send(NetEvent::Failed(format!("中继建房失败：{err}")));
        return;
    }
    run_relay_setup(stream, command_rx, event_tx);
}

fn relay_client_thread(
    relay_address: String,
    room_code: String,
    command_rx: Receiver<NetCommand>,
    event_tx: Sender<NetEvent>,
) {
    let mut stream = match connect_to_address(&relay_address) {
        Ok(stream) => stream,
        Err(reason) => {
            let _ = event_tx.send(NetEvent::Failed(format!("中继服务器连接失败：{reason}")));
            return;
        }
    };
    if let Err(err) = write_relay_command(&mut stream, &format!("JOIN {room_code}")) {
        let _ = event_tx.send(NetEvent::Failed(format!("中继加入失败：{err}")));
        return;
    }
    run_relay_setup(stream, command_rx, event_tx);
}

fn connect_to_address(address: &str) -> Result<TcpStream, String> {
    let addrs = address
        .to_socket_addrs()
        .map_err(|err| format!("地址解析失败：{err}"))?
        .collect::<Vec<_>>();
    if addrs.is_empty() {
        return Err("地址解析失败：没有可用地址".to_string());
    }
    let mut last_error = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
            Ok(stream) => return Ok(stream),
            Err(err) => last_error = Some(err.to_string()),
        }
    }
    Err(format!(
        "连接失败：{}",
        last_error.unwrap_or_else(|| "连接失败".to_string())
    ))
}

fn run_relay_setup(
    mut stream: TcpStream,
    command_rx: Receiver<NetCommand>,
    event_tx: Sender<NetEvent>,
) {
    let _ = stream.set_nonblocking(true);
    let mut read_buffer = Vec::new();
    loop {
        if matches!(command_rx.try_recv(), Ok(NetCommand::Stop { .. })) {
            return;
        }
        match read_relay_lines(&mut stream, &mut read_buffer) {
            Ok(lines) => {
                for line in lines {
                    match parse_relay_line(&line) {
                        Ok(RelayLine::RoomCreated(room_code)) => {
                            let _ = event_tx.send(NetEvent::RelayRoomCreated(room_code));
                        }
                        Ok(RelayLine::WaitingForPeer(room_code)) => {
                            let _ = event_tx.send(NetEvent::RelayWaitingPeer(room_code));
                        }
                        Ok(RelayLine::PeerConnected) => {
                            run_stream(stream, command_rx, event_tx);
                            return;
                        }
                        Ok(RelayLine::Error(reason)) => {
                            let _ = event_tx.send(NetEvent::Failed(reason));
                            return;
                        }
                        Err(reason) => {
                            let _ = event_tx.send(NetEvent::Failed(reason));
                            return;
                        }
                    }
                }
            }
            Err(err) => {
                let _ = event_tx.send(NetEvent::Failed(format!("中继握手失败：{err}")));
                return;
            }
        }
        thread::sleep(Duration::from_millis(16));
    }
}

fn run_stream(mut stream: TcpStream, command_rx: Receiver<NetCommand>, event_tx: Sender<NetEvent>) {
    let _ = stream.set_nonblocking(true);
    let _ = stream.set_nodelay(true);
    let _ = event_tx.send(NetEvent::Connected);
    let mut last_rx = Instant::now();
    // 让首个心跳 Ping 立即发出，以便尽快得到一次延迟读数。
    let mut last_ping = Instant::now()
        .checked_sub(HEARTBEAT_INTERVAL)
        .unwrap_or_else(Instant::now);
    let mut nonce = 0_u64;
    // 记录最近一次发出的 Ping（nonce 与发送时刻），用于在收到对应 Pong 时计算 RTT。
    let mut pending_ping: Option<(u64, Instant)> = None;
    let mut read_buffer = Vec::new();

    loop {
        let mut did_work = false;
        while let Ok(command) = command_rx.try_recv() {
            did_work = true;
            match command {
                NetCommand::Send(message) => {
                    if let Err(err) = write_message(&mut stream, &message) {
                        let _ = event_tx.send(NetEvent::Disconnected(format!("发送失败：{err}")));
                        return;
                    }
                }
                NetCommand::Stop { reason } => {
                    if let Some(reason) = reason {
                        let _ = write_message(&mut stream, &PvpMessage::Leave { reason });
                    }
                    return;
                }
            }
        }

        match read_messages(&mut stream, &mut read_buffer) {
            Ok((messages, received_bytes)) => {
                if received_bytes {
                    did_work = true;
                    last_rx = Instant::now();
                }
                for message in messages {
                    match message {
                        PvpMessage::Ping { nonce } => {
                            let _ = write_message(&mut stream, &PvpMessage::Pong { nonce });
                        }
                        PvpMessage::Pong { nonce } => {
                            if let Some((sent_nonce, sent_at)) = pending_ping {
                                if sent_nonce == nonce {
                                    let ms = sent_at.elapsed().as_millis().min(u128::from(u32::MAX))
                                        as u32;
                                    let _ = event_tx.send(NetEvent::Latency(ms));
                                    pending_ping = None;
                                }
                            }
                        }
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
            pending_ping = Some((nonce, last_ping));
            did_work = true;
        }
        if last_rx.elapsed() >= HEARTBEAT_TIMEOUT {
            let _ = event_tx.send(NetEvent::Disconnected("连接超时".to_string()));
            return;
        }
        if did_work {
            thread::yield_now();
        } else {
            thread::sleep(NETWORK_IDLE_SLEEP);
        }
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

enum RelayLine {
    RoomCreated(String),
    WaitingForPeer(String),
    PeerConnected,
    Error(String),
}

fn write_relay_command(stream: &mut TcpStream, command: &str) -> std::io::Result<()> {
    stream.write_all(format!("RELAY {RELAY_PROTOCOL_VERSION} {command}\n").as_bytes())
}

fn read_relay_lines(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> std::io::Result<Vec<String>> {
    read_available_bytes(stream, buffer)?;
    let mut lines = Vec::new();
    while let Some(newline_index) = buffer.iter().position(|byte| *byte == b'\n') {
        let line_bytes = buffer.drain(..=newline_index).collect::<Vec<_>>();
        let line = String::from_utf8(line_bytes)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
        lines.push(line.trim().to_string());
    }
    Ok(lines)
}

fn parse_relay_line(line: &str) -> Result<RelayLine, String> {
    let mut parts = line.splitn(3, ' ');
    let tag = parts.next().unwrap_or_default();
    let code = parts.next().unwrap_or_default();
    let value = parts.next().unwrap_or_default().trim().to_string();
    if tag != "RELAY" {
        return Err("中继服务器返回了无效响应".to_string());
    }
    Ok(match code {
        "ROOM" => RelayLine::RoomCreated(value),
        "WAIT" => RelayLine::WaitingForPeer(value),
        "PEER" => RelayLine::PeerConnected,
        "ERR" => RelayLine::Error(value),
        _ => return Err("中继服务器返回了未知响应".to_string()),
    })
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
    let received_bytes = read_available_bytes(stream, buffer)?;
    let mut messages = Vec::new();
    loop {
        if buffer.len() < 4 {
            break;
        }
        let len = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
        if len > MAX_FRAME_LEN {
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

fn read_available_bytes(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> std::io::Result<bool> {
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
    Ok(received_bytes)
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
        reset_lobby_input(&mut input);
    }
}

fn reset_pvp_state_on_lobby(
    mut connection: ResMut<PvpConnection>,
    mut input: ResMut<PvpLobbyInput>,
    mut team_state: ResMut<PvpTeamState>,
    mut rematch_state: ResMut<PvpRematchState>,
    mut incoming_intents: ResMut<PvpIncomingIntents>,
    mut incoming_snapshots: ResMut<PvpIncomingSnapshots>,
    mut incoming_feedbacks: ResMut<PvpIncomingFeedbacks>,
    mut pending_local_intent: ResMut<PvpPendingLocalIntent>,
    mut last_remote_intent_seq: ResMut<PvpLastRemoteIntentSeq>,
    mut snapshot_sync: ResMut<PvpSnapshotSync>,
) {
    connection.stop();
    connection.status = PvpStatus::Idle;
    connection.seq = 0;
    connection.latency_ms = None;
    connection.local_ip = local_lan_ip();
    reset_lobby_input(&mut input);
    reset_session_state(&mut team_state, &mut incoming_intents);
    *rematch_state = PvpRematchState::default();
    incoming_snapshots.0.clear();
    incoming_feedbacks.0.clear();
    *pending_local_intent = PvpPendingLocalIntent::default();
    *last_remote_intent_seq = PvpLastRemoteIntentSeq::default();
    snapshot_sync.reset();
}

fn clear_pending_error_on_exit(mut input: ResMut<PvpLobbyInput>) {
    input.info.clear();
}

fn reset_lobby_input(input: &mut PvpLobbyInput) {
    input.info = "选择局域网联机或服务器联机。".to_string();
    input.screen = PvpLobbyScreen::Menu;
    input.room_code.clear();
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
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            },))
                .with_children(|buttons| {
                    buttons
                        .spawn((Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(12.0),
                            ..default()
                        },))
                        .with_children(|row| {
                            spawn_pvp_button(
                                row,
                                "局域网联机",
                                body_font.clone(),
                                PvpHostButton,
                                &theme,
                            );
                            spawn_pvp_button(
                                row,
                                "服务器联机",
                                body_font.clone(),
                                PvpRelayHostButton,
                                &theme,
                            );
                        });
                    buttons
                        .spawn((Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(12.0),
                            ..default()
                        },))
                        .with_children(|row| {
                            spawn_pvp_button(row, "建房", body_font.clone(), PvpJoinButton, &theme);
                            spawn_pvp_button(
                                row,
                                "加入",
                                body_font.clone(),
                                PvpRelayJoinButton,
                                &theme,
                            );
                        });
                    buttons
                        .spawn((Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(12.0),
                            ..default()
                        },))
                        .with_children(|row| {
                            spawn_pvp_button(
                                row,
                                "确认",
                                body_font.clone(),
                                PvpConfirmJoinButton,
                                &theme,
                            );
                            spawn_pvp_button(row, "返回", body_font, PvpBackButton, &theme);
                        });
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
    mut relay_host_buttons: Query<&Interaction, (Changed<Interaction>, With<PvpRelayHostButton>)>,
    mut relay_join_buttons: Query<&Interaction, (Changed<Interaction>, With<PvpRelayJoinButton>)>,
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
            input.screen = PvpLobbyScreen::DirectMenu;
            input.info = "选择局域网建房或加入。".to_string();
            return;
        }
    }
    for interaction in &mut relay_host_buttons {
        if *interaction == Interaction::Pressed && input.screen == PvpLobbyScreen::Menu {
            input.screen = PvpLobbyScreen::RelayMenu;
            input.room_code.clear();
            input.info = "选择服务器建房或加入。".to_string();
            return;
        }
    }
    for interaction in &mut join_buttons {
        if *interaction == Interaction::Pressed {
            match input.screen {
                PvpLobbyScreen::DirectMenu => {
                    reset_session_state(&mut team_state, &mut incoming_intents);
                    input.screen = PvpLobbyScreen::HostRoom;
                    input.info = "正在局域网建房...".to_string();
                    start_host(&mut connection);
                    return;
                }
                PvpLobbyScreen::RelayMenu => {
                    input.screen = PvpLobbyScreen::RelayHostRoom;
                    input.room_code.clear();
                    input.info = "输入服务器地址，然后点击确认。".to_string();
                    return;
                }
                _ => {}
            }
        }
    }
    for interaction in &mut relay_join_buttons {
        if *interaction == Interaction::Pressed {
            match input.screen {
                PvpLobbyScreen::DirectMenu => {
                    input.screen = PvpLobbyScreen::JoinAddress;
                    input.info = "输入对方 IP 或域名与端口，然后点击确认。".to_string();
                    return;
                }
                PvpLobbyScreen::RelayMenu => {
                    input.screen = PvpLobbyScreen::RelayJoinRoom;
                    input.room_code.clear();
                    input.info = "输入服务器地址和房间码，然后点击确认。".to_string();
                    return;
                }
                _ => {}
            }
        }
    }
    for interaction in &mut confirm_join_buttons {
        if *interaction == Interaction::Pressed {
            match input.screen {
                PvpLobbyScreen::JoinAddress => {
                    submit_join_address(
                        &mut input,
                        &mut connection,
                        &mut team_state,
                        &mut incoming_intents,
                    );
                    return;
                }
                PvpLobbyScreen::RelayHostRoom => {
                    submit_relay_host(
                        &mut input,
                        &mut connection,
                        &mut team_state,
                        &mut incoming_intents,
                    );
                    return;
                }
                PvpLobbyScreen::RelayJoinRoom => {
                    submit_relay_join(
                        &mut input,
                        &mut connection,
                        &mut team_state,
                        &mut incoming_intents,
                    );
                    return;
                }
                _ => {}
            }
        }
    }
    for interaction in &mut back_buttons {
        if *interaction == Interaction::Pressed {
            if matches!(
                input.screen,
                PvpLobbyScreen::DirectMenu | PvpLobbyScreen::RelayMenu
            ) {
                input.screen = PvpLobbyScreen::Menu;
                input.room_code.clear();
                input.info = "选择局域网联机或服务器联机。".to_string();
            } else if input.screen != PvpLobbyScreen::Menu {
                connection.stop_with_leave(Some("对方已返回联机菜单，房间已关闭。".to_string()));
                reset_session_state(&mut team_state, &mut incoming_intents);
                connection.status = PvpStatus::Idle;
                match input.screen {
                    PvpLobbyScreen::HostRoom | PvpLobbyScreen::JoinAddress => {
                        input.screen = PvpLobbyScreen::DirectMenu;
                        input.info = "选择局域网建房或加入。".to_string();
                    }
                    PvpLobbyScreen::RelayHostRoom | PvpLobbyScreen::RelayJoinRoom => {
                        input.screen = PvpLobbyScreen::RelayMenu;
                        input.room_code.clear();
                        input.info = "选择服务器建房或加入。".to_string();
                    }
                    _ => {}
                }
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
        input.info = format!("正在加入局域网房间 {address} ...");
        start_client(connection, address);
    } else {
        input.info = "地址格式应为 ip:端口 或 域名:端口。".to_string();
    }
}

fn submit_relay_host(
    input: &mut PvpLobbyInput,
    connection: &mut PvpConnection,
    team_state: &mut PvpTeamState,
    incoming_intents: &mut PvpIncomingIntents,
) {
    let relay_address = input.relay_address.trim().to_string();
    if valid_address_like(&relay_address) {
        reset_session_state(team_state, incoming_intents);
        input.room_code.clear();
        input.info = format!("正在连接服务器 {relay_address} ...");
        start_relay_host(connection, relay_address);
    } else {
        input.info = "服务器地址格式应为 ip:端口 或 域名:端口。".to_string();
    }
}

fn submit_relay_join(
    input: &mut PvpLobbyInput,
    connection: &mut PvpConnection,
    team_state: &mut PvpTeamState,
    incoming_intents: &mut PvpIncomingIntents,
) {
    let relay_address = input.relay_address.trim().to_string();
    let room_code = normalize_room_code(&input.room_code);
    if !valid_address_like(&relay_address) {
        input.info = "服务器地址格式应为 ip:端口 或 域名:端口。".to_string();
        return;
    }
    if room_code.is_empty() {
        input.info = "请输入房间码。".to_string();
        return;
    }
    reset_session_state(team_state, incoming_intents);
    input.room_code = room_code.clone();
    input.info = format!("正在通过服务器加入房间 {room_code} ...");
    start_relay_client(connection, relay_address, room_code);
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
            "局域网联机适合同一网络或端口映射；服务器联机适合双方都连接公网服务器。",
            format!("本机局域网 IP：{}", connection.local_ip),
            "等待选择".to_string(),
        ),
        PvpLobbyScreen::DirectMenu => (
            "局域网联机",
            "建房会监听本机端口；加入需要输入对方给你的地址。",
            format!("本机局域网 IP：{}", connection.local_ip),
            "选择建房或加入".to_string(),
        ),
        PvpLobbyScreen::RelayMenu => (
            "服务器联机",
            "双方连接同一个中继服务器；建房后用房间码加入。",
            "通过服务器匹配房间".to_string(),
            "选择建房或加入".to_string(),
        ),
        PvpLobbyScreen::HostRoom => {
            let address = match connection.status {
                PvpStatus::Hosting { port } => format!("{}:{port}", connection.local_ip),
                _ => format!("{}:{DEFAULT_PORT}", connection.local_ip),
            };
            (
                "局域网房间",
                "把局域网地址发给对方；连接成功后会自动进入配队。",
                "局域网建房，等待对方加入".to_string(),
                address,
            )
        }
        PvpLobbyScreen::JoinAddress => (
            "局域网加入",
            "输入示例：127.0.0.1:42043；输入框支持鼠标选中、光标移动和复制粘贴。",
            "对方地址".to_string(),
            "".to_string(),
        ),
        PvpLobbyScreen::RelayHostRoom => {
            let room = if input.room_code.is_empty() {
                "服务器建房：等待生成房间码".to_string()
            } else {
                format!("服务器房间码：{}", input.room_code)
            };
            (
                "服务器建房",
                "输入服务器地址；建房后把房间码发给对方。",
                room,
                "服务器地址".to_string(),
            )
        }
        PvpLobbyScreen::RelayJoinRoom => (
            "服务器加入",
            "输入同一个服务器地址和对方给你的房间码。",
            "服务器房间".to_string(),
            "服务器地址 / 房间码".to_string(),
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
        PvpStatus::ConnectingRelay => input.info.clone(),
        PvpStatus::WaitingRelayPeer { room_code } => {
            format!("房间码：{room_code}。{}", input.info)
        }
        PvpStatus::JoiningRelayRoom { room_code } => {
            format!("正在加入房间 {room_code}。{}", input.info)
        }
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
            Option<&PvpRelayHostButton>,
            Option<&PvpRelayJoinButton>,
            Option<&PvpConfirmJoinButton>,
            Option<&PvpBackButton>,
        ),
        With<PvpLobbyButtonVisual>,
    >,
) {
    for (
        interaction,
        mut visibility,
        mut bg,
        mut border,
        host,
        join,
        relay_host,
        relay_join,
        confirm,
        _back,
    ) in &mut buttons
    {
        let mode_button = host.is_some() || relay_host.is_some();
        let room_action_button = join.is_some() || relay_join.is_some();
        let hidden = match input.screen {
            PvpLobbyScreen::Menu => room_action_button || confirm.is_some(),
            PvpLobbyScreen::DirectMenu | PvpLobbyScreen::RelayMenu => {
                mode_button || confirm.is_some()
            }
            PvpLobbyScreen::HostRoom => mode_button || room_action_button || confirm.is_some(),
            PvpLobbyScreen::JoinAddress
            | PvpLobbyScreen::RelayHostRoom
            | PvpLobbyScreen::RelayJoinRoom => mode_button || room_action_button,
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
    mut last_input_screen: Local<Option<PvpLobbyScreen>>,
) -> Result {
    if !matches!(
        input.screen,
        PvpLobbyScreen::JoinAddress | PvpLobbyScreen::RelayHostRoom | PvpLobbyScreen::RelayJoinRoom
    ) {
        *last_input_screen = None;
        return Ok(());
    }
    let screen = input.screen;
    let should_request_initial_focus = *last_input_screen != Some(screen);
    *last_input_screen = Some(screen);

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
            let enter_pressed = match screen {
                PvpLobbyScreen::JoinAddress => {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut input.address)
                            .hint_text("对方 ip/域名:端口")
                            .desired_width(360.0)
                            .font(egui::TextStyle::Heading),
                    );
                    if should_request_initial_focus {
                        response.request_focus();
                    }
                    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))
                }
                PvpLobbyScreen::RelayHostRoom => {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut input.relay_address)
                            .hint_text("服务器 ip/域名:端口")
                            .desired_width(360.0)
                            .font(egui::TextStyle::Heading),
                    );
                    if should_request_initial_focus {
                        response.request_focus();
                    }
                    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))
                }
                PvpLobbyScreen::RelayJoinRoom => {
                    let address_response = ui.add(
                        egui::TextEdit::singleline(&mut input.relay_address)
                            .hint_text("服务器 ip/域名:端口")
                            .desired_width(360.0)
                            .font(egui::TextStyle::Heading),
                    );
                    ui.add_space(8.0);
                    let room_response = ui.add(
                        egui::TextEdit::singleline(&mut input.room_code)
                            .hint_text("房间码")
                            .desired_width(360.0)
                            .font(egui::TextStyle::Heading),
                    );
                    if should_request_initial_focus && input.room_code.is_empty() {
                        room_response.request_focus();
                    }
                    (address_response.lost_focus() || room_response.lost_focus())
                        && ui.input(|input| input.key_pressed(egui::Key::Enter))
                }
                _ => false,
            };
            if enter_pressed {
                match input.screen {
                    PvpLobbyScreen::JoinAddress => submit_join_address(
                        &mut input,
                        &mut connection,
                        &mut team_state,
                        &mut incoming_intents,
                    ),
                    PvpLobbyScreen::RelayHostRoom => submit_relay_host(
                        &mut input,
                        &mut connection,
                        &mut team_state,
                        &mut incoming_intents,
                    ),
                    PvpLobbyScreen::RelayJoinRoom => submit_relay_join(
                        &mut input,
                        &mut connection,
                        &mut team_state,
                        &mut incoming_intents,
                    ),
                    _ => {}
                }
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

fn normalize_room_code(room_code: &str) -> String {
    room_code.trim().to_ascii_uppercase()
}

fn pvp_poll_network_system(
    mut connection: ResMut<PvpConnection>,
    mut team_state: ResMut<PvpTeamState>,
    mut rematch_state: ResMut<PvpRematchState>,
    mut incoming_intents: ResMut<PvpIncomingIntents>,
    mut incoming_snapshots: ResMut<PvpIncomingSnapshots>,
    mut incoming_feedbacks: ResMut<PvpIncomingFeedbacks>,
    mut input: ResMut<PvpLobbyInput>,
    game_state: Res<State<GameState>>,
    mut result_notice: ResMut<BattleResultNotice>,
    dbs: Option<Res<BattleDbs>>,
    monsters: Option<Res<MonsterPool>>,
    rules: Option<Res<BattleRules>>,
    formulas: Option<Res<BattleFormulaRules>>,
    deck: Option<Res<CardDeck>>,
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
                input.info = format!(
                    "局域网建房成功：{}:{port}，等待对方加入。",
                    connection.local_ip
                );
                console_log(
                    ConsoleLogCategory::Pvp,
                    format!("[host] 局域网建房成功：{}:{port}", connection.local_ip),
                );
            }
            NetEvent::RelayRoomCreated(room_code) | NetEvent::RelayWaitingPeer(room_code) => {
                input.room_code = room_code.clone();
                connection.status = PvpStatus::WaitingRelayPeer {
                    room_code: room_code.clone(),
                };
                input.info = format!("服务器房间码：{room_code}，等待对方加入。");
                console_log(
                    ConsoleLogCategory::Pvp,
                    format!("[relay-host] 房间就绪：{room_code}，等待对方加入"),
                );
            }
            NetEvent::Connected => {
                connection.status = PvpStatus::Connected;
                connection.latency_ms = None;
                input.info = "已连接，正在握手。".to_string();
                console_log(ConsoleLogCategory::Pvp, "[connect] 已连接，开始协议握手");
                if let (Some(dbs), Some(monsters), Some(rules), Some(formulas), Some(deck)) = (
                    dbs.as_ref(),
                    monsters.as_ref(),
                    rules.as_ref(),
                    formulas.as_ref(),
                    deck.as_ref(),
                ) {
                    let hash = data_hash(dbs, monsters, rules, formulas, deck);
                    console_log(
                        ConsoleLogCategory::PvpDetail,
                        format!(
                            "[handshake-send] protocol={} data_hash={hash}",
                            PROTOCOL_VERSION
                        ),
                    );
                    connection.send(PvpMessage::Hello {
                        protocol_version: PROTOCOL_VERSION,
                        data_hash: hash,
                    });
                }
            }
            NetEvent::Message(message) => match message {
                PvpMessage::Hello {
                    protocol_version,
                    data_hash: remote_hash,
                } => {
                    let local_hash = match (
                        dbs.as_ref(),
                        monsters.as_ref(),
                        rules.as_ref(),
                        formulas.as_ref(),
                        deck.as_ref(),
                    ) {
                        (Some(dbs), Some(monsters), Some(rules), Some(formulas), Some(deck)) => {
                            data_hash(dbs, monsters, rules, formulas, deck)
                        }
                        _ => String::new(),
                    };
                    let accepted =
                        protocol_version == PROTOCOL_VERSION && remote_hash == local_hash;
                    let reason = if protocol_version != PROTOCOL_VERSION {
                        Some("协议版本不一致".to_string())
                    } else if remote_hash != local_hash {
                        Some("双方数据版本不一致".to_string())
                    } else {
                        None
                    };
                    console_log(
                        ConsoleLogCategory::PvpDetail,
                        format!(
                            "[handshake-recv] protocol={} data_hash={} accepted={}",
                            protocol_version, remote_hash, accepted
                        ),
                    );
                    connection.remote_data_hash = Some(remote_hash);
                    connection.protocol_ready = accepted;
                    connection.send(PvpMessage::HelloAck {
                        accepted,
                        reason: reason.clone(),
                    });
                    if let Some(reason) = reason {
                        input.info = reason.clone();
                        console_log(
                            ConsoleLogCategory::Pvp,
                            format!("[handshake] 失败：{reason}"),
                        );
                    } else {
                        input.info = "握手完成，请选择队伍。".to_string();
                        console_log(ConsoleLogCategory::Pvp, "[handshake] 成功，请选择队伍");
                    }
                }
                PvpMessage::HelloAck { accepted, reason } => {
                    connection.protocol_ready = accepted;
                    if accepted {
                        input.info = "握手完成，请选择队伍。".to_string();
                        console_log(ConsoleLogCategory::Pvp, "[handshake] 成功，请选择队伍");
                    } else {
                        input.info = reason.unwrap_or_else(|| "连接被拒绝".to_string());
                        console_log(
                            ConsoleLogCategory::Pvp,
                            format!("[handshake] 被拒绝：{}", input.info),
                        );
                        connection.status = PvpStatus::Failed(input.info.clone());
                    }
                }
                PvpMessage::TeamSelected {
                    session_id,
                    monster_indices,
                } => {
                    if session_id == team_state.session_id {
                        team_state.remote_indices = Some(monster_indices);
                    } else {
                        console_log(
                            ConsoleLogCategory::PvpDetail,
                            format!(
                                "[team-ignore] stale session_id={} current={}",
                                session_id, team_state.session_id
                            ),
                        );
                    }
                }
                PvpMessage::BattleReady { session_id, seed } => {
                    if session_id == team_state.session_id {
                        team_state.battle_seed = Some(seed);
                    } else {
                        console_log(
                            ConsoleLogCategory::PvpDetail,
                            format!(
                                "[ready-ignore] stale session_id={} current={}",
                                session_id, team_state.session_id
                            ),
                        );
                    }
                }
                PvpMessage::BattleSnapshot(snapshot) => {
                    console_log(
                        ConsoleLogCategory::PvpDetail,
                        format!(
                            "[snapshot-recv] round={} phase={:?} result={}",
                            snapshot.turn,
                            snapshot.phase,
                            snapshot.result_message.as_deref().unwrap_or("<none>")
                        ),
                    );
                    incoming_snapshots.0.push(*snapshot)
                }
                PvpMessage::BattleFeedback(feedback) => {
                    console_log(
                        ConsoleLogCategory::PvpDetail,
                        format!("[feedback-recv] {:?}", feedback),
                    );
                    incoming_feedbacks.0.push(feedback)
                }
                PvpMessage::Intent { seq, intent } => {
                    console_log(
                        ConsoleLogCategory::PvpDetail,
                        format!("[intent-recv] seq={} intent={:?}", seq, intent),
                    );
                    incoming_intents.0.push((seq, intent))
                }
                PvpMessage::RematchRequest => {
                    if *game_state.get() == GameState::Result && connection.is_connected() {
                        rematch_state.incoming_request = true;
                        console_log(ConsoleLogCategory::Pvp, "[rematch] 收到再来一局邀请");
                    }
                }
                PvpMessage::RematchAccepted => {
                    rematch_state.accepted = true;
                    rematch_state.incoming_request = false;
                    rematch_state.outgoing_request = false;
                    console_log(ConsoleLogCategory::Pvp, "[rematch] 对方接受再来一局");
                }
                PvpMessage::RematchRejected => {
                    rematch_state.incoming_request = false;
                    rematch_state.outgoing_request = false;
                    result_notice.text = "对方拒绝了你的邀请".to_string();
                    result_notice.remaining = 2.5;
                    console_log(ConsoleLogCategory::Pvp, "[rematch] 对方拒绝再来一局");
                }
                PvpMessage::Surrender => {
                    let reason = "对方已撤退/战斗中止".to_string();
                    input.info = reason.clone();
                    connection.status = PvpStatus::Disconnected(reason);
                }
                PvpMessage::Leave { reason } => {
                    input.info = reason.clone();
                    connection.status = PvpStatus::Disconnected(reason);
                }
                PvpMessage::Ping { .. } | PvpMessage::Pong { .. } => {}
            },
            NetEvent::Latency(ms) => {
                connection.latency_ms = Some(ms);
            }
            NetEvent::Failed(reason) => {
                console_log(ConsoleLogCategory::Pvp, format!("[error] {reason}"));
                input.info = reason.clone();
                connection.latency_ms = None;
                connection.status = PvpStatus::Failed(reason);
            }
            NetEvent::Disconnected(reason) => {
                if !matches!(connection.status, PvpStatus::Disconnected(_)) {
                    console_log(ConsoleLogCategory::Pvp, format!("[disconnect] {reason}"));
                    input.info = reason.clone();
                    connection.latency_ms = None;
                    connection.status = PvpStatus::Disconnected(reason);
                }
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
    mut incoming_snapshots: ResMut<PvpIncomingSnapshots>,
    mut incoming_feedbacks: ResMut<PvpIncomingFeedbacks>,
    mut pending_local_intent: ResMut<PvpPendingLocalIntent>,
    mut last_remote_intent_seq: ResMut<PvpLastRemoteIntentSeq>,
    mut snapshot_sync: ResMut<PvpSnapshotSync>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
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
    let Some(seed) = pvp_battle_seed(&connection, &mut team_state) else {
        return;
    };
    commands.insert_resource(BattleShuffleSeed(seed));
    commands.insert_resource(BattleControlMode::PlayerVsRemote);
    commands.insert_resource(PvpTurnOrder {
        local_first: connection.role == Some(PvpRole::Host),
    });
    commands.insert_resource(TeamSelections {
        player_indices: local_indices,
        enemy_indices: remote_indices,
    });
    incoming_snapshots.0.clear();
    incoming_feedbacks.0.clear();
    *pending_local_intent = PvpPendingLocalIntent::default();
    *last_remote_intent_seq = PvpLastRemoteIntentSeq::default();
    snapshot_sync.reset();
    team_state.battle_started = true;
    // 重置战斗阶段到 Init，确保 init_battle_system 会重新初始化战斗实体与状态。
    // 否则上一局结束时遗留的 BattlePhase（如 DeathResolve）会让再来一局直接回到上一局结算界面。
    next_phase.set(BattlePhase::Init);
    next_state.set(GameState::Battle);
}

fn pvp_battle_seed(connection: &PvpConnection, team_state: &mut PvpTeamState) -> Option<u64> {
    match connection.role {
        Some(PvpRole::Host) => {
            let seed = team_state
                .battle_seed
                .unwrap_or_else(new_battle_shuffle_seed);
            team_state.battle_seed = Some(seed);
            connection.send(PvpMessage::BattleReady {
                session_id: team_state.session_id,
                seed,
            });
            Some(seed)
        }
        Some(PvpRole::Client) => team_state.battle_seed,
        None => None,
    }
}

fn team_hp(team: &crate::battle::Team, query: &Query<&Stats, With<InBattle>>) -> Vec<i32> {
    team.combatants
        .iter()
        .map(|&entity| query.get(entity).map(|stats| stats.hp).unwrap_or(0))
        .collect()
}

fn team_shields(team: &crate::battle::Team, query: &Query<&Shield, With<InBattle>>) -> Vec<i32> {
    team.combatants
        .iter()
        .map(|&entity| query.get(entity).map(|shield| shield.0).unwrap_or(0))
        .collect()
}

fn team_auras(
    team: &crate::battle::Team,
    query: &Query<&ElementAura, With<InBattle>>,
) -> Vec<[Option<crate::data::ElementType>; 2]> {
    team.combatants
        .iter()
        .map(|&entity| query.get(entity).map(|aura| aura.slots).unwrap_or_default())
        .collect()
}

fn team_statuses(
    team: &crate::battle::Team,
    query: &Query<&StatusBoard, With<InBattle>>,
) -> Vec<Vec<StatusInstance>> {
    team.combatants
        .iter()
        .map(|&entity| {
            query
                .get(entity)
                .map(|statuses| statuses.entries.clone())
                .unwrap_or_default()
        })
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

fn team_skill_uses(
    team: &crate::battle::Team,
    query: &Query<&crate::battle::SkillUses, With<InBattle>>,
) -> Vec<[u8; 4]> {
    team.combatants
        .iter()
        .map(|&entity| query.get(entity).map(|uses| uses.0).unwrap_or([0; 4]))
        .collect()
}

fn apply_team_skill_uses(
    team: &crate::battle::Team,
    uses_values: &[[u8; 4]],
    query: &mut Query<&mut crate::battle::SkillUses, With<InBattle>>,
) {
    for (&entity, uses) in team.combatants.iter().zip(uses_values.iter().copied()) {
        if let Ok(mut skill_uses) = query.get_mut(entity) {
            skill_uses.0 = uses;
        }
    }
}

fn apply_team_shields(
    team: &crate::battle::Team,
    shield_values: &[i32],
    max_shield_hp_ratio: f32,
    stats_query: &mut Query<&mut Stats, With<InBattle>>,
    shield_query: &mut Query<&mut Shield, With<InBattle>>,
) {
    for (&entity, shield) in team.combatants.iter().zip(shield_values.iter().copied()) {
        let max_hp = stats_query
            .get_mut(entity)
            .map(|stats| stats.max_hp)
            .unwrap_or_default();
        if let Ok(mut shield_value) = shield_query.get_mut(entity) {
            shield_value.set_capped(shield, max_hp, max_shield_hp_ratio);
        }
    }
}

fn apply_team_auras(
    team: &crate::battle::Team,
    aura_values: &[[Option<crate::data::ElementType>; 2]],
    query: &mut Query<&mut ElementAura, With<InBattle>>,
) {
    for (&entity, slots) in team.combatants.iter().zip(aura_values.iter().copied()) {
        if let Ok(mut aura) = query.get_mut(entity) {
            aura.slots = slots;
        }
    }
}

fn apply_team_statuses(
    team: &crate::battle::Team,
    status_values: &[Vec<StatusInstance>],
    stats_query: &mut Query<&mut Stats, With<InBattle>>,
    status_query: &mut Query<&mut StatusBoard, With<InBattle>>,
) {
    for (&entity, entries) in team.combatants.iter().zip(status_values.iter()) {
        let Ok(mut statuses) = status_query.get_mut(entity) else {
            continue;
        };
        statuses.entries = entries.clone();
        if let Ok(mut stats) = stats_query.get_mut(entity) {
            recalculate_stage_modifiers(&mut stats, &statuses);
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
        "失败！队伍全灭。按 R 返回大厅。".to_string()
    } else if message.starts_with("失败！") {
        "胜利！全歼敌方。按 R 返回大厅。".to_string()
    } else {
        message.to_string()
    }
}

fn pvp_forward_host_battle_events_system(
    connection: Res<PvpConnection>,
    battle_mode: Res<BattleControlMode>,
    mut events: MessageReader<BattleEvent>,
) {
    if *battle_mode != BattleControlMode::PlayerVsRemote || connection.role != Some(PvpRole::Host) {
        events.clear();
        return;
    }

    for event in events.read() {
        if let Some(feedback) = battle_event_to_pvp_feedback(event) {
            console_log(
                ConsoleLogCategory::PvpDetail,
                format!("[feedback-send] {:?}", feedback),
            );
            connection.send(PvpMessage::BattleFeedback(feedback));
        }
    }
}

fn battle_event_to_pvp_feedback(event: &BattleEvent) -> Option<PvpBattleFeedback> {
    Some(match event {
        BattleEvent::TurnStarted(turn) => PvpBattleFeedback::TurnStarted(*turn),
        BattleEvent::CardUsed { side, card_name } => PvpBattleFeedback::CardUsed {
            side: *side,
            card_name: card_name.clone(),
        },
        BattleEvent::CardDiscarded { side, card_name } => PvpBattleFeedback::CardDiscarded {
            side: *side,
            card_name: card_name.clone(),
        },
        BattleEvent::SkillUsed {
            side,
            skill_name,
            slot,
        } => PvpBattleFeedback::SkillUsed {
            side: *side,
            skill_name: skill_name.clone(),
            slot: *slot,
        },
        BattleEvent::DamageDealt {
            source,
            target,
            amount,
            damage_type,
        } => PvpBattleFeedback::DamageDealt {
            source: *source,
            target: *target,
            amount: *amount,
            damage_type: *damage_type,
        },
        BattleEvent::AttackMissed { source, target } => PvpBattleFeedback::AttackMissed {
            source: *source,
            target: *target,
        },
        BattleEvent::ShieldAbsorbed { side, amount } => PvpBattleFeedback::ShieldAbsorbed {
            side: *side,
            amount: *amount,
        },
        BattleEvent::Healed { side, amount } => PvpBattleFeedback::Healed {
            side: *side,
            amount: *amount,
        },
        BattleEvent::ShieldGained { side, amount } => PvpBattleFeedback::ShieldGained {
            side: *side,
            amount: *amount,
        },
        BattleEvent::Switched { side, name } => PvpBattleFeedback::Switched {
            side: *side,
            name: name.clone(),
        },
        BattleEvent::ReactionTriggered {
            source,
            target,
            reaction_name,
        } => PvpBattleFeedback::ReactionTriggered {
            source: *source,
            target: *target,
            reaction_name: reaction_name.clone(),
        },
        _ => return None,
    })
}

fn should_replay_pvp_feedback_on_client(feedback: &PvpBattleFeedback) -> bool {
    !matches!(feedback, PvpBattleFeedback::TurnStarted(_))
}

fn pvp_feedback_to_battle_event(feedback: PvpBattleFeedback) -> BattleEvent {
    match feedback {
        PvpBattleFeedback::TurnStarted(turn) => BattleEvent::TurnStarted(turn),
        PvpBattleFeedback::CardUsed { side, card_name } => BattleEvent::CardUsed {
            side: mirror_side(side),
            card_name,
        },
        PvpBattleFeedback::CardDiscarded { side, card_name } => BattleEvent::CardDiscarded {
            side: mirror_side(side),
            card_name,
        },
        PvpBattleFeedback::SkillUsed {
            side,
            skill_name,
            slot,
        } => BattleEvent::SkillUsed {
            side: mirror_side(side),
            skill_name,
            slot,
        },
        PvpBattleFeedback::DamageDealt {
            source,
            target,
            amount,
            damage_type,
        } => BattleEvent::DamageDealt {
            source: mirror_side(source),
            target: mirror_side(target),
            amount,
            damage_type,
        },
        PvpBattleFeedback::AttackMissed { source, target } => BattleEvent::AttackMissed {
            source: mirror_side(source),
            target: mirror_side(target),
        },
        PvpBattleFeedback::ShieldAbsorbed { side, amount } => BattleEvent::ShieldAbsorbed {
            side: mirror_side(side),
            amount,
        },
        PvpBattleFeedback::Healed { side, amount } => BattleEvent::Healed {
            side: mirror_side(side),
            amount,
        },
        PvpBattleFeedback::ShieldGained { side, amount } => BattleEvent::ShieldGained {
            side: mirror_side(side),
            amount,
        },
        PvpBattleFeedback::Switched { side, name } => BattleEvent::Switched {
            side: mirror_side(side),
            name,
        },
        PvpBattleFeedback::ReactionTriggered {
            source,
            target,
            reaction_name,
        } => BattleEvent::ReactionTriggered {
            source: mirror_side(source),
            target: mirror_side(target),
            reaction_name,
        },
    }
}

#[derive(SystemParam)]
struct PvpSendSnapshotResources<'w> {
    last_remote_intent_seq: Res<'w, PvpLastRemoteIntentSeq>,
    time: Res<'w, Time>,
    snapshot_sync: ResMut<'w, PvpSnapshotSync>,
    game_state: Res<'w, State<GameState>>,
    battle_phase: Res<'w, State<BattlePhase>>,
    turn_count: Res<'w, TurnCount>,
    round_order: Res<'w, RoundOrder>,
    player_team: Option<Res<'w, PlayerTeam>>,
    enemy_team: Option<Res<'w, EnemyTeam>>,
    hand: Res<'w, Hand>,
    action_points: Res<'w, ActionPoints>,
    battle_result: Res<'w, BattleResult>,
    pending_discard: Option<Res<'w, PendingHandDiscard>>,
    shuffle_seed: Option<Res<'w, BattleShuffleSeed>>,
    team_state: Res<'w, PvpTeamState>,
}

fn pvp_send_host_snapshot_system(
    connection: Res<PvpConnection>,
    battle_mode: Res<BattleControlMode>,
    mut runtime: PvpSendSnapshotResources,
    stats_query: Query<&Stats, With<InBattle>>,
    shield_query: Query<&Shield, With<InBattle>>,
    aura_query: Query<&ElementAura, With<InBattle>>,
    status_query: Query<&StatusBoard, With<InBattle>>,
    skill_uses_query: Query<&crate::battle::SkillUses, With<InBattle>>,
) {
    if *battle_mode != BattleControlMode::PlayerVsRemote
        || connection.role != Some(PvpRole::Host)
        || !matches!(
            *runtime.game_state.get(),
            GameState::Battle | GameState::Result
        )
    {
        return;
    }
    let (Some(player_team), Some(enemy_team)) =
        (runtime.player_team.as_ref(), runtime.enemy_team.as_ref())
    else {
        return;
    };
    let player_hp = team_hp(&player_team.0, &stats_query);
    let enemy_hp = team_hp(&enemy_team.0, &stats_query);
    let player_shields = team_shields(&player_team.0, &shield_query);
    let enemy_shields = team_shields(&enemy_team.0, &shield_query);
    let player_auras = team_auras(&player_team.0, &aura_query);
    let enemy_auras = team_auras(&enemy_team.0, &aura_query);
    let player_statuses = team_statuses(&player_team.0, &status_query);
    let enemy_statuses = team_statuses(&enemy_team.0, &status_query);
    let player_skill_uses = team_skill_uses(&player_team.0, &skill_uses_query);
    let enemy_skill_uses = team_skill_uses(&enemy_team.0, &skill_uses_query);
    let host_player_defeated = !player_hp.iter().any(|hp| *hp > 0);
    let host_enemy_defeated = !enemy_hp.iter().any(|hp| *hp > 0);
    let mirrored_result = (!runtime.battle_result.message.is_empty())
        .then(|| mirror_result_message(&runtime.battle_result.message));
    let snapshot = PvpBattleSnapshot {
        session_id: runtime.team_state.session_id,
        battle_seed: runtime
            .shuffle_seed
            .as_ref()
            .map(|seed| seed.0)
            .unwrap_or(0),
        turn: runtime.turn_count.0,
        phase: *runtime.battle_phase.get(),
        first_side: runtime.round_order.first,
        player_active_index: enemy_team.0.active_index,
        enemy_active_index: player_team.0.active_index,
        player_hp: enemy_hp,
        enemy_hp: player_hp,
        player_shields: enemy_shields,
        enemy_shields: player_shields,
        player_auras: enemy_auras,
        enemy_auras: player_auras,
        player_statuses: enemy_statuses,
        enemy_statuses: player_statuses,
        player_skill_uses: enemy_skill_uses,
        enemy_skill_uses: player_skill_uses,
        player_ap: runtime.action_points.enemy,
        enemy_ap: runtime.action_points.player,
        player_hand: runtime.hand.enemy.clone(),
        enemy_hand: runtime.hand.player.clone(),
        pending_discard: runtime.pending_discard.as_ref().map(|pending| {
            PvpPendingDiscardSnapshot {
                side: pending.side,
                next_phase: pending.next_phase,
            }
        }),
        acknowledged_intent_seq: runtime.last_remote_intent_seq.0,
        player_defeated: host_enemy_defeated,
        enemy_defeated: host_player_defeated,
        result_message: mirrored_result,
    };

    let changed = runtime.snapshot_sync.last_sent.as_ref() != Some(&snapshot);
    let interval_elapsed = matches!(*runtime.game_state.get(), GameState::Battle)
        && runtime
            .snapshot_sync
            .interval
            .tick(runtime.time.delta())
            .just_finished();
    if !changed && !interval_elapsed {
        return;
    }
    if changed {
        runtime.snapshot_sync.interval.reset();
    }

    console_log(
        ConsoleLogCategory::PvpDetail,
        format!(
            "[snapshot-send] round={} phase={:?} ack_seq={} 我方AP={} 敌方AP={} 我方手牌={} 敌方手牌={} result={}",
            snapshot.turn,
            snapshot.phase,
            snapshot.acknowledged_intent_seq,
            snapshot.enemy_ap,
            snapshot.player_ap,
            snapshot.enemy_hand.len(),
            snapshot.player_hand.len(),
            snapshot.result_message.as_deref().unwrap_or("<none>")
        ),
    );
    runtime.snapshot_sync.last_sent = Some(snapshot.clone());
    connection.send(PvpMessage::BattleSnapshot(Box::new(snapshot)));
}

#[derive(SystemParam)]
struct PvpApplySnapshotResources<'w> {
    incoming: ResMut<'w, PvpIncomingSnapshots>,
    incoming_feedbacks: ResMut<'w, PvpIncomingFeedbacks>,
    pending_local_intent: ResMut<'w, PvpPendingLocalIntent>,
    turn_count: ResMut<'w, TurnCount>,
    round_order: ResMut<'w, RoundOrder>,
    player_team: Option<ResMut<'w, PlayerTeam>>,
    enemy_team: Option<ResMut<'w, EnemyTeam>>,
    hand: ResMut<'w, Hand>,
    action_points: ResMut<'w, ActionPoints>,
    battle_rules: Res<'w, BattleRules>,
    battle_result: ResMut<'w, BattleResult>,
    pending_ko: ResMut<'w, PendingKoResolution>,
    next_phase: ResMut<'w, NextState<BattlePhase>>,
    next_state: ResMut<'w, NextState<GameState>>,
}

fn pvp_apply_host_snapshot_system(
    mut commands: Commands,
    mut runtime: PvpApplySnapshotResources,
    connection: Res<PvpConnection>,
    game_state: Res<State<GameState>>,
    team_state: Res<PvpTeamState>,
    shuffle_seed: Option<Res<BattleShuffleSeed>>,
    battle_mode: Res<BattleControlMode>,
    battle_phase: Res<State<BattlePhase>>,
    mut stats_query: Query<&mut Stats, With<InBattle>>,
    mut shield_query: Query<&mut Shield, With<InBattle>>,
    mut aura_query: Query<&mut ElementAura, With<InBattle>>,
    mut status_query: Query<&mut StatusBoard, With<InBattle>>,
    mut skill_uses_query: Query<&mut crate::battle::SkillUses, With<InBattle>>,
    mut event_writer: MessageWriter<BattleEvent>,
) {
    if *game_state.get() != GameState::Battle
        || *battle_mode != BattleControlMode::PlayerVsRemote
        || connection.role != Some(PvpRole::Client)
    {
        runtime.incoming.0.clear();
        runtime.incoming_feedbacks.0.clear();
        return;
    }
    let Some(snapshot) = runtime.incoming.0.pop() else {
        return;
    };
    runtime.incoming.0.clear();
    if snapshot.session_id != team_state.session_id {
        console_log(
            ConsoleLogCategory::PvpDetail,
            format!(
                "[snapshot-ignore] stale session_id={} current={}",
                snapshot.session_id, team_state.session_id
            ),
        );
        runtime.incoming_feedbacks.0.clear();
        return;
    }
    if shuffle_seed
        .as_ref()
        .is_some_and(|seed| snapshot.battle_seed != seed.0)
    {
        console_log(
            ConsoleLogCategory::PvpDetail,
            format!(
                "[snapshot-ignore] stale battle_seed={} current={}",
                snapshot.battle_seed,
                shuffle_seed.as_ref().map(|seed| seed.0).unwrap_or(0)
            ),
        );
        runtime.incoming_feedbacks.0.clear();
        return;
    }
    console_log(
        ConsoleLogCategory::PvpDetail,
        format!(
            "[snapshot-apply] round={} host_phase={:?} local_phase={:?} ack_seq={} 我方AP={} 敌方AP={} 我方手牌={} 敌方手牌={} feedbacks={} result={}",
            snapshot.turn,
            snapshot.phase,
            host_phase_for_local_phase(snapshot.phase),
            snapshot.acknowledged_intent_seq,
            snapshot.player_ap,
            snapshot.enemy_ap,
            snapshot.player_hand.len(),
            snapshot.enemy_hand.len(),
            runtime.incoming_feedbacks.0.len(),
            snapshot.result_message.as_deref().unwrap_or("<none>")
        ),
    );

    runtime.turn_count.0 = snapshot.turn;
    runtime
        .round_order
        .set_first(mirror_side(snapshot.first_side));
    *runtime.hand = Hand {
        player: snapshot.player_hand,
        enemy: snapshot.enemy_hand,
    };
    if let Some(pending) = snapshot.pending_discard {
        commands.insert_resource(PendingHandDiscard {
            side: mirror_side(pending.side),
            next_phase: host_phase_for_local_phase(pending.next_phase),
        });
    } else {
        commands.remove_resource::<PendingHandDiscard>();
    }
    runtime.action_points.player = snapshot.player_ap;
    runtime.action_points.enemy = snapshot.enemy_ap;
    if runtime
        .pending_local_intent
        .0
        .is_some_and(|seq| snapshot.acknowledged_intent_seq >= seq)
    {
        runtime.pending_local_intent.0 = None;
    }
    runtime.pending_ko.timer.reset();
    runtime.pending_ko.player_switch_index = None;
    runtime.pending_ko.enemy_switch_index = None;
    runtime.pending_ko.player_defeated = snapshot.player_defeated;
    runtime.pending_ko.enemy_defeated = snapshot.enemy_defeated;
    runtime.pending_ko.resume_phase = None;

    if let Some(player_team) = runtime.player_team.as_mut() {
        player_team.0.active_index = snapshot
            .player_active_index
            .min(player_team.0.combatants.len().saturating_sub(1));
        apply_team_hp(&player_team.0, &snapshot.player_hp, &mut stats_query);
        apply_team_shields(
            &player_team.0,
            &snapshot.player_shields,
            runtime.battle_rules.max_shield_hp_ratio,
            &mut stats_query,
            &mut shield_query,
        );
        apply_team_auras(&player_team.0, &snapshot.player_auras, &mut aura_query);
        apply_team_statuses(
            &player_team.0,
            &snapshot.player_statuses,
            &mut stats_query,
            &mut status_query,
        );
        apply_team_skill_uses(
            &player_team.0,
            &snapshot.player_skill_uses,
            &mut skill_uses_query,
        );
    }
    if let Some(enemy_team) = runtime.enemy_team.as_mut() {
        enemy_team.0.active_index = snapshot
            .enemy_active_index
            .min(enemy_team.0.combatants.len().saturating_sub(1));
        apply_team_hp(&enemy_team.0, &snapshot.enemy_hp, &mut stats_query);
        apply_team_shields(
            &enemy_team.0,
            &snapshot.enemy_shields,
            runtime.battle_rules.max_shield_hp_ratio,
            &mut stats_query,
            &mut shield_query,
        );
        apply_team_auras(&enemy_team.0, &snapshot.enemy_auras, &mut aura_query);
        apply_team_statuses(
            &enemy_team.0,
            &snapshot.enemy_statuses,
            &mut stats_query,
            &mut status_query,
        );
        apply_team_skill_uses(
            &enemy_team.0,
            &snapshot.enemy_skill_uses,
            &mut skill_uses_query,
        );
    }

    for feedback in runtime.incoming_feedbacks.0.drain(..) {
        if should_replay_pvp_feedback_on_client(&feedback) {
            console_log(
                ConsoleLogCategory::PvpDetail,
                format!("[feedback-apply] {:?}", feedback),
            );
            event_writer.write(pvp_feedback_to_battle_event(feedback));
        }
    }

    if let Some(message) = snapshot.result_message {
        runtime.battle_result.message = message;
        runtime.next_state.set(GameState::Result);
    } else {
        let mirrored_phase = host_phase_for_local_phase(snapshot.phase);
        if mirrored_phase != *battle_phase.get() {
            runtime.next_phase.set(mirrored_phase);
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
    mut last_remote_intent_seq: ResMut<PvpLastRemoteIntentSeq>,
    connection: Res<PvpConnection>,
    battle_phase: Res<State<BattlePhase>>,
    pending_discard: Option<Res<PendingHandDiscard>>,
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
    let remote_discard_phase = *battle_phase.get() == BattlePhase::Discard
        && pending_discard
            .as_ref()
            .is_some_and(|pending| pending.side == Side::Enemy);
    if *battle_mode != BattleControlMode::PlayerVsRemote
        || connection.role != Some(PvpRole::Host)
        || (*battle_phase.get() != BattlePhase::EnemyTurn && !remote_discard_phase)
    {
        return;
    }
    let Some((seq, intent)) = incoming.0.first().cloned() else {
        return;
    };
    incoming.0.remove(0);
    last_remote_intent_seq.0 = last_remote_intent_seq.0.max(seq);
    console_log(
        ConsoleLogCategory::PvpDetail,
        format!("[intent-apply] seq={} intent={:?}", seq, intent),
    );
    if remote_discard_phase && !matches!(intent, BattleIntent::DiscardCard { .. }) {
        console_log(
            ConsoleLogCategory::PvpDetail,
            format!(
                "[intent-ignore] discard phase only accepts discard intents: {:?}",
                intent
            ),
        );
        return;
    }
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
    let _ = pending_boosts;
    event_writer.write(BattleEvent::CardUsed {
        side: Side::Enemy,
        card_name,
    });
}

fn pvp_handle_battle_disconnect_system(
    mut connection: ResMut<PvpConnection>,
    battle_mode: Res<BattleControlMode>,
    game_state: Res<State<GameState>>,
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
    let message = format!("联机中断：{reason}");
    event_writer.write(BattleEvent::NetworkInterrupted {
        reason: reason.clone(),
    });
    battle_result.message = message;
    connection.stop();
    connection.status = PvpStatus::Idle;
    next_state.set(GameState::Result);
}

fn pvp_result_rematch_system(
    mut pending_action: ResMut<PendingBattleResultAction>,
    mut rematch_state: ResMut<PvpRematchState>,
    connection: Res<PvpConnection>,
    battle_mode: Res<BattleControlMode>,
    mut team_state: ResMut<PvpTeamState>,
    mut incoming_intents: ResMut<PvpIncomingIntents>,
    mut incoming_snapshots: ResMut<PvpIncomingSnapshots>,
    mut incoming_feedbacks: ResMut<PvpIncomingFeedbacks>,
    mut pending_local_intent: ResMut<PvpPendingLocalIntent>,
    mut last_remote_intent_seq: ResMut<PvpLastRemoteIntentSeq>,
    mut snapshot_sync: ResMut<PvpSnapshotSync>,
    mut selection_state: ResMut<crate::team_selection::SelectionState>,
    mut entry_mode: ResMut<crate::team_selection::SelectionEntryMode>,
    mut battle_result: ResMut<BattleResult>,
    mut result_notice: ResMut<BattleResultNotice>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if *battle_mode != BattleControlMode::PlayerVsRemote {
        return;
    }

    if rematch_state.accepted {
        rematch_state.accepted = false;
        enter_pvp_rematch_selection(
            &mut team_state,
            &mut incoming_intents,
            &mut incoming_snapshots,
            &mut incoming_feedbacks,
            &mut pending_local_intent,
            &mut last_remote_intent_seq,
            &mut snapshot_sync,
            &mut selection_state,
            &mut entry_mode,
            &mut battle_result,
            &mut next_state,
        );
        return;
    }

    let Some(action) = pending_action.0 else {
        return;
    };
    match action {
        BattleResultAction::Rematch => {
            if connection.is_connected() {
                request_rematch(&connection);
                rematch_state.outgoing_request = true;
                result_notice.text = "已邀请对方再来一局".to_string();
                result_notice.remaining = 2.0;
            } else {
                rematch_state.outgoing_request = false;
                result_notice.text = "对方已离开房间，无法再来一局。".to_string();
                result_notice.remaining = 2.5;
            }
            pending_action.0 = None;
        }
        BattleResultAction::AcceptRematch => {
            if connection.is_connected() {
                accept_rematch(&connection);
            }
            rematch_state.incoming_request = false;
            pending_action.0 = None;
            enter_pvp_rematch_selection(
                &mut team_state,
                &mut incoming_intents,
                &mut incoming_snapshots,
                &mut incoming_feedbacks,
                &mut pending_local_intent,
                &mut last_remote_intent_seq,
                &mut snapshot_sync,
                &mut selection_state,
                &mut entry_mode,
                &mut battle_result,
                &mut next_state,
            );
        }
        BattleResultAction::RejectRematch => {
            if connection.is_connected() {
                reject_rematch(&connection);
            }
            rematch_state.incoming_request = false;
            result_notice.text = "已拒绝对方邀请".to_string();
            result_notice.remaining = 2.0;
            pending_action.0 = None;
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn enter_pvp_rematch_selection(
    team_state: &mut PvpTeamState,
    incoming_intents: &mut PvpIncomingIntents,
    incoming_snapshots: &mut PvpIncomingSnapshots,
    incoming_feedbacks: &mut PvpIncomingFeedbacks,
    pending_local_intent: &mut PvpPendingLocalIntent,
    last_remote_intent_seq: &mut PvpLastRemoteIntentSeq,
    snapshot_sync: &mut PvpSnapshotSync,
    selection_state: &mut crate::team_selection::SelectionState,
    entry_mode: &mut crate::team_selection::SelectionEntryMode,
    battle_result: &mut BattleResult,
    next_state: &mut NextState<GameState>,
) {
    *team_state = PvpTeamState::default();
    incoming_intents.0.clear();
    incoming_snapshots.0.clear();
    incoming_feedbacks.0.clear();
    *pending_local_intent = PvpPendingLocalIntent::default();
    *last_remote_intent_seq = PvpLastRemoteIntentSeq::default();
    snapshot_sync.reset();
    selection_state.reset();
    *entry_mode = crate::team_selection::SelectionEntryMode::Pvp;
    battle_result.message.clear();
    battle_result.export_status = None;
    next_state.set(GameState::TeamSelection);
}
