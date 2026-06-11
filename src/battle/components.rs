//! 战斗系统组件定义

use std::{
    collections::VecDeque,
    fmt,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    console_log::{ConsoleLogCategory, log as console_log, log_with_prefix},
    data::{
        AttributeStageBounds, AttributeType, CardId, ElementType, SkillId, StatusCategory,
        StatusDef, StatusTickTiming,
    },
    game_state::BattlePhase,
};

#[derive(Component, Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum Side {
    Player,
    Enemy,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let side_str = match self {
            Side::Player => "Player",
            Side::Enemy => "Enemy",
        };
        write!(f, "{}", side_str)
    }
}

#[derive(Debug, Clone)]
pub struct Team {
    pub combatants: Vec<Entity>,
    pub active_index: usize,
}

impl Team {
    pub fn active_combatant(&self) -> Option<Entity> {
        self.combatants.get(self.active_index).copied()
    }
}

#[derive(Resource, Debug, Clone)]
pub struct PlayerTeam(pub Team);

#[derive(Resource, Debug, Clone)]
pub struct EnemyTeam(pub Team);

#[derive(Component, Debug, Clone, Copy)]
pub struct Combatant {
    pub side: Side,
    pub element: ElementType,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
    pub acc: i32,
    pub atk_stage: i32,
    pub def_stage: i32,
    pub spd_stage: i32,
    pub acc_stage: i32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct SkillList(pub [SkillId; 4]);

#[derive(Component, Debug, Clone, Copy)]
pub struct SkillCount(pub usize);

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Shield(pub i32);

impl Shield {
    pub fn max_for_hp(max_hp: i32, max_hp_ratio: f32) -> i32 {
        ((max_hp.max(0) as f32) * max_hp_ratio.max(0.0)).floor() as i32
    }

    pub fn capped_value(value: i32, max_hp: i32, max_hp_ratio: f32) -> i32 {
        value.clamp(0, Self::max_for_hp(max_hp, max_hp_ratio))
    }

    pub fn set_capped(&mut self, value: i32, max_hp: i32, max_hp_ratio: f32) {
        self.0 = Self::capped_value(value, max_hp, max_hp_ratio);
    }

    pub fn gain_capped(&mut self, amount: i32, max_hp: i32, max_hp_ratio: f32) -> i32 {
        let max_shield = Self::max_for_hp(max_hp, max_hp_ratio);
        let before = Self::capped_value(self.0, max_hp, max_hp_ratio);
        let gained = amount.max(0).min(max_shield.saturating_sub(before));
        self.0 = before + gained;
        gained
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ElementAura {
    pub slots: [Option<ElementType>; 2],
}

impl ElementAura {
    pub fn elements(&self) -> Vec<ElementType> {
        self.slots.iter().flatten().copied().collect()
    }

    pub fn primary(&self) -> Option<ElementType> {
        self.slots.iter().flatten().copied().next()
    }

    pub fn contains(&self, element: ElementType) -> bool {
        self.slots.contains(&Some(element))
    }

    pub fn set_elements(&mut self, elements: &[ElementType]) {
        self.slots = [None, None];
        let mut next = 0;
        for element in elements {
            if next >= 2 || self.contains(*element) {
                continue;
            }
            self.slots[next] = Some(*element);
            next += 1;
        }
    }

    pub fn remove(&mut self, element: ElementType) -> bool {
        let before = self.elements();
        let after: Vec<_> = before
            .into_iter()
            .filter(|entry| *entry != element)
            .collect();
        let removed = after.len() < self.elements().len();
        if removed {
            self.set_elements(&after);
        }
        removed
    }

    #[allow(dead_code)]
    pub fn apply_attachment(&mut self, element: ElementType) {
        let mut current: Vec<_> = self
            .elements()
            .into_iter()
            .filter(|entry| {
                matches!(
                    entry,
                    ElementType::Fire
                        | ElementType::Water
                        | ElementType::Grass
                        | ElementType::Thunder
                )
            })
            .collect();
        if current.contains(&element) {
            self.set_elements(&current);
            return;
        }
        if current.len() >= 2 {
            current.remove(0);
        }
        current.push(element);
        self.set_elements(&current);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusStageModifier {
    pub attribute: AttributeType,
    pub amount: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInstance {
    pub id: String,
    pub name: String,
    pub category: StatusCategory,
    pub remaining_turns: i32,
    pub applied_round: u32,
    pub source_side: Option<Side>,
    pub tick_timing: Option<StatusTickTiming>,
    pub stage_modifiers: Vec<StatusStageModifier>,
    pub fixed_damage_on_tick: i32,
    pub heal_on_tick: i32,
    pub heal_taken_multiplier: Option<f32>,
    pub evade_charges: i32,
}

#[derive(Component, Debug, Clone, Default)]
pub struct StatusBoard {
    pub entries: Vec<StatusInstance>,
}

impl StatusBoard {
    pub fn heal_taken_multiplier(&self) -> f32 {
        self.entries
            .iter()
            .filter_map(|entry| entry.heal_taken_multiplier)
            .fold(1.0, |acc, multiplier| acc * multiplier)
    }

    pub fn try_consume_evade_charge(&mut self) -> Option<String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.evade_charges > 0)?;
        entry.evade_charges -= 1;
        let status_id = entry.id.clone();
        if entry.evade_charges <= 0 {
            self.entries.retain(|other| other.id != status_id);
        }
        Some(status_id)
    }
}

impl StatusInstance {
    pub fn from_def(def: &StatusDef, source_side: Option<Side>, applied_round: u32) -> Self {
        Self {
            id: def.id.clone(),
            name: def.name.clone(),
            category: def.category,
            remaining_turns: def.duration_turns,
            applied_round,
            source_side,
            tick_timing: def.tick_timing,
            stage_modifiers: def
                .stage_modifiers
                .iter()
                .map(|modifier| StatusStageModifier {
                    attribute: modifier.attribute,
                    amount: modifier.amount,
                })
                .collect(),
            fixed_damage_on_tick: def.fixed_damage_on_tick,
            heal_on_tick: def.heal_on_tick,
            heal_taken_multiplier: def.heal_taken_multiplier,
            evade_charges: def.evade_charges,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatusTickOutcome {
    pub status_id: String,
    pub status_name: String,
    pub fixed_damage: i32,
    pub heal_amount: i32,
    pub expired: bool,
    pub remaining_turns: i32,
}

pub fn recalculate_stage_modifiers(stats: &mut Stats, status_board: &StatusBoard) {
    let bounds = AttributeStageBounds::default();
    stats.atk_stage = 0;
    stats.def_stage = 0;
    stats.spd_stage = 0;
    stats.acc_stage = 0;

    for entry in &status_board.entries {
        for modifier in &entry.stage_modifiers {
            match modifier.attribute {
                AttributeType::Atk => stats.atk_stage += modifier.amount,
                AttributeType::Def => stats.def_stage += modifier.amount,
                AttributeType::Spd => stats.spd_stage += modifier.amount,
                AttributeType::Acc => stats.acc_stage += modifier.amount,
            }
        }
    }

    stats.atk_stage = stats.atk_stage.clamp(bounds.min, bounds.max);
    stats.def_stage = stats.def_stage.clamp(bounds.min, bounds.max);
    stats.spd_stage = stats.spd_stage.clamp(bounds.min, bounds.max);
    stats.acc_stage = stats.acc_stage.clamp(bounds.min, bounds.max);
}

pub fn upsert_status_instance(
    status_board: &mut StatusBoard,
    status: StatusInstance,
    stats: &mut Stats,
) -> bool {
    let mut refreshed = false;
    if let Some(existing) = status_board
        .entries
        .iter_mut()
        .find(|entry| entry.id == status.id)
    {
        existing.name = status.name;
        existing.category = status.category;
        existing.remaining_turns = status.remaining_turns;
        existing.applied_round = status.applied_round;
        existing.source_side = status.source_side;
        existing.tick_timing = status.tick_timing;
        existing.stage_modifiers = status.stage_modifiers;
        existing.fixed_damage_on_tick = status.fixed_damage_on_tick;
        existing.heal_on_tick = status.heal_on_tick;
        existing.heal_taken_multiplier = status.heal_taken_multiplier;
        existing.evade_charges = status.evade_charges;
        refreshed = true;
    } else {
        status_board.entries.push(status);
    }
    recalculate_stage_modifiers(stats, status_board);
    refreshed
}

pub fn apply_status_from_def(
    status_board: &mut StatusBoard,
    stats: &mut Stats,
    def: &StatusDef,
    source_side: Option<Side>,
    applied_round: u32,
) -> bool {
    upsert_status_instance(
        status_board,
        StatusInstance::from_def(def, source_side, applied_round),
        stats,
    )
}

pub fn remove_status_by_id(
    status_board: &mut StatusBoard,
    stats: &mut Stats,
    status_id: &str,
) -> bool {
    let original_len = status_board.entries.len();
    status_board.entries.retain(|entry| entry.id != status_id);
    let removed = status_board.entries.len() != original_len;
    if removed {
        recalculate_stage_modifiers(stats, status_board);
    }
    removed
}

pub fn transfer_status_by_id(
    source_statuses: &mut StatusBoard,
    source_stats: &mut Stats,
    target_statuses: &mut StatusBoard,
    target_stats: &mut Stats,
    status_id: &str,
) -> bool {
    let Some(index) = source_statuses
        .entries
        .iter()
        .position(|entry| entry.id == status_id)
    else {
        return false;
    };

    let status = source_statuses.entries.remove(index);
    recalculate_stage_modifiers(source_stats, source_statuses);
    upsert_status_instance(target_statuses, status, target_stats);
    true
}

pub fn tick_statuses_for_timing(
    status_board: &mut StatusBoard,
    timing: StatusTickTiming,
) -> Vec<StatusTickOutcome> {
    let mut outcomes = Vec::with_capacity(status_board.entries.len());

    for entry in &status_board.entries {
        if entry.tick_timing != Some(timing) {
            continue;
        }
        outcomes.push(StatusTickOutcome {
            status_id: entry.id.clone(),
            status_name: entry.name.clone(),
            fixed_damage: entry.fixed_damage_on_tick.max(0),
            heal_amount: entry.heal_on_tick.max(0),
            expired: false,
            remaining_turns: entry.remaining_turns.max(0),
        });
    }

    outcomes
}

pub fn decrement_status_durations_for_round(
    status_board: &mut StatusBoard,
    stats: &mut Stats,
    current_round: u32,
) -> Vec<StatusTickOutcome> {
    let mut outcomes = Vec::with_capacity(status_board.entries.len());

    for entry in &mut status_board.entries {
        if entry.applied_round >= current_round {
            outcomes.push(StatusTickOutcome {
                status_id: entry.id.clone(),
                status_name: entry.name.clone(),
                fixed_damage: 0,
                heal_amount: 0,
                expired: false,
                remaining_turns: entry.remaining_turns.max(0),
            });
            continue;
        }
        if entry.remaining_turns > 0 {
            entry.remaining_turns -= 1;
        }
        outcomes.push(StatusTickOutcome {
            status_id: entry.id.clone(),
            status_name: entry.name.clone(),
            fixed_damage: 0,
            heal_amount: 0,
            expired: entry.remaining_turns <= 0,
            remaining_turns: entry.remaining_turns.max(0),
        });
    }

    status_board
        .entries
        .retain(|entry| entry.remaining_turns > 0);
    recalculate_stage_modifiers(stats, status_board);
    outcomes
}

#[derive(Component, Debug, Clone, Copy)]
pub struct InBattle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnAction {
    Skill(SkillId),
    Switch,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BattleControlMode {
    #[default]
    PlayerVsAi,
    DebugPlayerControlsBoth,
    PlayerVsRemote,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PvpTurnOrder {
    pub local_first: bool,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct UiControlSide(pub Side);

impl Default for UiControlSide {
    fn default() -> Self {
        Self(Side::Player)
    }
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct TurnContext {
    pub player_ended: bool,
    pub enemy_ended: bool,
    pub player_end_requested: bool,
    pub enemy_end_requested: bool,
    pub player_action: Option<TurnAction>,
    pub enemy_action: Option<TurnAction>,
}

#[derive(Resource, Debug, Default)]
pub struct BattleLog(pub VecDeque<String>);

#[derive(Resource, Debug, Default)]
pub struct BattleResult {
    pub message: String,
    pub export_status: Option<String>,
}

#[derive(Resource, Debug, Clone)]
pub struct PendingKoResolution {
    pub timer: Timer,
    pub player_switch_index: Option<usize>,
    pub enemy_switch_index: Option<usize>,
    pub player_defeated: bool,
    pub enemy_defeated: bool,
    pub resume_phase: Option<BattlePhase>,
}

impl Default for PendingKoResolution {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            player_switch_index: None,
            enemy_switch_index: None,
            player_defeated: false,
            enemy_defeated: false,
            resume_phase: None,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct TurnCount(pub u32);

#[derive(Resource, Debug, Clone, Default)]
pub struct ActionPoints {
    pub player: i32,
    pub enemy: i32,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct RoundOrder {
    pub first: Side,
    pub second: Side,
    pub previous_first: Option<Side>,
}

impl Default for RoundOrder {
    fn default() -> Self {
        Self {
            first: Side::Player,
            second: Side::Enemy,
            previous_first: None,
        }
    }
}

impl RoundOrder {
    pub fn set_first(&mut self, side: Side) {
        self.first = side;
        self.second = opposite_side(side);
        self.previous_first = Some(side);
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct AccuracyRng {
    state: u64,
}

impl Default for AccuracyRng {
    fn default() -> Self {
        Self {
            state: 0xA5A5_1F2D_D3C4_B7E9,
        }
    }
}

impl AccuracyRng {
    pub fn reset(&mut self, seed: u64) {
        self.state = seed;
    }

    pub fn next_unit_f32(&mut self) -> f32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let value = (self.state >> 32) as u32;
        value as f32 / u32::MAX as f32
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct Hand {
    pub player: Vec<CardId>,
    pub enemy: Vec<CardId>,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct BattleShuffleSeed(pub u64);

#[derive(Resource, Debug, Clone, Default)]
pub struct CardPiles {
    pub draw: Vec<CardId>,
    pub discard: Vec<CardId>,
    pub shuffle_seed: u64,
    pub reshuffle_counter: u32,
}

impl CardPiles {
    pub fn from_deck(deck: &[CardId], seed: u64) -> Self {
        Self {
            draw: shuffled_cards(deck, seed),
            discard: Vec::new(),
            shuffle_seed: seed,
            reshuffle_counter: 0,
        }
    }

    pub fn push_discard(&mut self, _side: Side, card_id: CardId) {
        self.discard.push(card_id);
    }

    pub fn draw_one(&mut self, fallback_deck: &[CardId]) -> Option<CardId> {
        if self.draw.is_empty() {
            if !self.discard.is_empty() {
                console_log(
                    ConsoleLogCategory::CardsDetail,
                    format!(
                        "牌堆耗尽：将弃牌堆 {} 张重洗进入牌堆；重洗次数={}",
                        self.discard.len(),
                        self.reshuffle_counter.wrapping_add(1)
                    ),
                );
                self.draw = shuffled_cards(&self.discard, self.reshuffle_seed(1));
                self.discard.clear();
                self.reshuffle_counter = self.reshuffle_counter.wrapping_add(1);
            } else if !fallback_deck.is_empty() {
                console_log(
                    ConsoleLogCategory::CardsDetail,
                    format!(
                        "牌堆与弃牌堆为空：使用基础牌组 {} 张重建牌堆；重洗次数={}",
                        fallback_deck.len(),
                        self.reshuffle_counter.wrapping_add(1)
                    ),
                );
                self.draw = shuffled_cards(fallback_deck, self.reshuffle_seed(17));
                self.reshuffle_counter = self.reshuffle_counter.wrapping_add(1);
            }
        }
        self.draw.pop()
    }

    fn reshuffle_seed(&self, salt: u64) -> u64 {
        mix_seed(
            self.shuffle_seed
                ^ (self.reshuffle_counter as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ salt,
        )
    }
}

static BATTLE_SHUFFLE_SEED_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn new_battle_shuffle_seed() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let time_seed = (now as u64) ^ ((now >> 64) as u64);
    let counter = BATTLE_SHUFFLE_SEED_COUNTER
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1);
    mix_seed(time_seed ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

fn shuffled_cards(cards: &[CardId], seed: u64) -> Vec<CardId> {
    let mut result = cards.to_vec();
    if result.len() <= 1 {
        return result;
    }
    let mut state = mix_seed(seed);
    for i in (1..result.len()).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let j = (state as usize) % (i + 1);
        result.swap(i, j);
    }
    result
}

fn mix_seed(mut seed: u64) -> u64 {
    seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    seed = (seed ^ (seed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    seed = (seed ^ (seed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    seed ^ (seed >> 31)
}

#[derive(Debug, Clone, Default)]
pub struct PendingBoost {
    pub next_attack_bonus: i32,
    pub next_shield_bonus: i32,
    pub next_heal_bonus: i32,
    pub next_element_attachment_ap: Option<i32>,
    pub next_reaction_fixed_damage: Option<i32>,
    pub next_wind_spread_damage: Option<(i32, Vec<crate::data::ElementType>)>,
    pub next_aura_attack_draw: Option<(usize, String)>,
    pub next_skill_cost_draw: Option<(i32, usize, String)>,
    pub next_switch_draw: Option<(usize, String)>,
    pub next_knockout_draw: Option<(usize, String)>,
    pub shield_absorb_ap: Option<i32>,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct PendingBoosts {
    pub player: PendingBoost,
    pub enemy: PendingBoost,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SideTurnMemory {
    pub knocked_out_opponent_this_turn: bool,
    pub switched_this_turn: bool,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct CardTurnMemory {
    pub player: SideTurnMemory,
    pub enemy: SideTurnMemory,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct PendingHandDiscard {
    pub side: Side,
    pub next_phase: BattlePhase,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct PendingGuardCounterClear {
    pub acting_side: Side,
}

#[derive(Resource, Debug, Clone)]
pub struct PendingTacticalDiscard {
    pub side: Side,
    pub draw: usize,
    pub source_card: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SelectedCardState {
    pub index: Option<usize>,
    pub discard_armed: bool,
}

#[derive(Resource, Default, Clone, Copy)]
pub struct SelectedCards {
    pub player: SelectedCardState,
    pub enemy: SelectedCardState,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StructuredLogEntry {
    pub phase: String,
    pub summary: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayLogEntry {
    pub seq: u64,
    pub phase: String,
    pub summary: String,
    pub detail: String,
}

#[derive(Resource, Debug, Default)]
pub struct StructuredBattleLog(pub VecDeque<StructuredLogEntry>);

#[derive(Resource, Debug, Default)]
pub struct ReplayEventLog(pub Vec<ReplayLogEntry>);

#[derive(Debug, Clone, Serialize)]
pub struct ActionTraceEntry {
    pub seq: u64,
    pub round: u32,
    pub side: Side,
    pub action: String,
    pub detail: String,
}

#[derive(Resource, Debug, Default)]
pub struct ActionTrace(pub Vec<ActionTraceEntry>);

pub const BATTLE_LOG_LIMIT: usize = 30;

pub fn push_battle_line(log: &mut BattleLog, line: impl Into<String>) {
    let line = line.into();
    console_log(ConsoleLogCategory::Battle, &line);
    log.0.push_back(line);
    while log.0.len() > BATTLE_LOG_LIMIT {
        log.0.pop_front();
    }
}

pub fn push_structured_battle_line(
    log: &mut StructuredBattleLog,
    phase: impl Into<String>,
    summary: impl Into<String>,
    detail: impl Into<String>,
) {
    let entry = StructuredLogEntry {
        phase: phase.into(),
        summary: summary.into(),
        detail: detail.into(),
    };
    let should_print = !matches!(
        entry.phase.as_str(),
        phase if phase.starts_with("trace-")
            || phase.starts_with("state-")
            || phase.starts_with("formula-")
            || phase.starts_with("status-")
    );
    if should_print {
        log_with_prefix(
            ConsoleLogCategory::BattleDebug,
            format!("[{}]", entry.phase),
            format!("{}：{}", entry.summary, entry.detail),
        );
    }
    log.0.push_back(entry);
    while log.0.len() > BATTLE_LOG_LIMIT {
        log.0.pop_front();
    }
}

pub fn push_replay_log_entry(
    log: &mut ReplayEventLog,
    phase: impl Into<String>,
    summary: impl Into<String>,
    detail: impl Into<String>,
) {
    let seq = log.0.len() as u64 + 1;
    log.0.push(ReplayLogEntry {
        seq,
        phase: phase.into(),
        summary: summary.into(),
        detail: detail.into(),
    });
}

pub fn record_action_trace(trace: &mut ActionTrace, mut entry: ActionTraceEntry) {
    entry.seq = trace.0.len() as u64 + 1;
    log_with_prefix(
        ConsoleLogCategory::Trace,
        format!(
            "[seq {}][round {}][{}]",
            entry.seq,
            entry.round,
            side_phase_label(entry.side)
        ),
        format!("{}：{}", entry.action, entry.detail),
    );
    trace.0.push(entry);
}

pub fn clear_runtime_battle_logs(
    battle_log: &mut BattleLog,
    structured_log: &mut StructuredBattleLog,
    replay_log: &mut ReplayEventLog,
    action_trace: &mut ActionTrace,
) {
    battle_log.0.clear();
    structured_log.0.clear();
    replay_log.0.clear();
    action_trace.0.clear();
}

pub fn side_phase_label(side: Side) -> &'static str {
    match side {
        Side::Player => "player",
        Side::Enemy => "enemy",
    }
}

pub fn opposite_side(side: Side) -> Side {
    match side {
        Side::Player => Side::Enemy,
        Side::Enemy => Side::Player,
    }
}

pub fn battle_phase_for_side(side: Side) -> BattlePhase {
    match side {
        Side::Player => BattlePhase::PlayerTurn,
        Side::Enemy => BattlePhase::EnemyTurn,
    }
}

pub fn next_phase_after_side_end(order: &RoundOrder, side: Side) -> BattlePhase {
    if side == order.first {
        battle_phase_for_side(order.second)
    } else {
        BattlePhase::CheckEnd
    }
}

pub fn action_text(action: &TurnAction) -> String {
    match action {
        TurnAction::Skill(skill_id) => format!("skill:{skill_id:?}"),
        TurnAction::Switch => "switch".to_string(),
    }
}

pub fn push_turn_action_trace(
    trace: &mut ActionTrace,
    round: u32,
    side: Side,
    action: TurnAction,
    detail: impl Into<String>,
) {
    record_action_trace(
        trace,
        ActionTraceEntry {
            seq: 0,
            round,
            side,
            action: action_text(&action),
            detail: detail.into(),
        },
    );
}

pub fn push_named_action_trace(
    trace: &mut ActionTrace,
    round: u32,
    side: Side,
    action: impl Into<String>,
    detail: impl Into<String>,
) {
    record_action_trace(
        trace,
        ActionTraceEntry {
            seq: 0,
            round,
            side,
            action: action.into(),
            detail: detail.into(),
        },
    );
}

pub fn clear_turn_context(turn_ctx: &mut TurnContext) {
    turn_ctx.player_action = None;
    turn_ctx.enemy_action = None;
    turn_ctx.player_ended = false;
    turn_ctx.enemy_ended = false;
    turn_ctx.player_end_requested = false;
    turn_ctx.enemy_end_requested = false;
}

#[allow(dead_code)]
pub fn reset_round_end_flags(turn_ctx: &mut TurnContext) {
    turn_ctx.player_ended = false;
    turn_ctx.enemy_ended = false;
    turn_ctx.player_end_requested = false;
    turn_ctx.enemy_end_requested = false;
}

pub fn note_structured_phase(
    log: &mut StructuredBattleLog,
    phase: &str,
    summary: &str,
    detail: impl Into<String>,
) {
    push_structured_battle_line(log, phase, summary, detail);
}

pub fn note_action_phase(
    log: &mut StructuredBattleLog,
    round: u32,
    side: Side,
    summary: impl Into<String>,
    detail: impl Into<String>,
) {
    push_structured_battle_line(
        log,
        format!("round-{round}-{}", side_phase_label(side)),
        summary,
        detail,
    );
}

pub fn note_round_phase(log: &mut StructuredBattleLog, round: u32, detail: impl Into<String>) {
    push_structured_battle_line(log, format!("round-{round}"), "回合开始", detail);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{CardId, SkillId, StatusCategory, StatusTickTiming};

    fn test_stats() -> Stats {
        Stats {
            hp: 10,
            max_hp: 10,
            atk: 5,
            def: 5,
            spd: 5,
            acc: 100,
            atk_stage: 0,
            def_stage: 0,
            spd_stage: 0,
            acc_stage: 0,
        }
    }

    #[test]
    fn shield_gain_is_capped_at_half_max_hp() {
        let mut shield = Shield(8);

        let gained = shield.gain_capped(10, 21, 0.5);

        assert_eq!(gained, 2);
        assert_eq!(shield.0, 10);
    }

    #[test]
    fn shield_set_clamps_to_half_max_hp() {
        let mut shield = Shield(0);

        shield.set_capped(99, 20, 0.5);

        assert_eq!(shield.0, 10);
    }

    #[test]
    fn shared_card_pile_draws_from_one_deck() {
        let deck = [
            CardId::GainAp,
            CardId::NextAttackBoost,
            CardId::NextShieldBoost,
        ];
        let mut piles = CardPiles::from_deck(&deck, 42);
        let initial_draw = piles.draw.clone();

        let first = piles.draw_one(&deck);
        let second = piles.draw_one(&deck);

        assert_eq!(first, initial_draw.last().copied());
        assert_eq!(second, initial_draw.get(initial_draw.len() - 2).copied());
        assert_eq!(piles.draw.len(), deck.len() - 2);
    }

    #[test]
    fn shared_card_pile_keeps_one_discard_for_both_sides() {
        let deck = [
            CardId::GainAp,
            CardId::NextAttackBoost,
            CardId::NextShieldBoost,
        ];
        let mut piles = CardPiles::from_deck(&deck, 42);

        piles.push_discard(Side::Player, CardId::GainAp);
        piles.push_discard(Side::Enemy, CardId::NextAttackBoost);

        assert_eq!(piles.discard, vec![CardId::GainAp, CardId::NextAttackBoost]);
    }

    #[test]
    fn replay_log_assigns_monotonic_sequence_numbers() {
        let mut replay_log = ReplayEventLog::default();
        push_replay_log_entry(&mut replay_log, "battle-event", "BattleEvent", "first");
        push_replay_log_entry(
            &mut replay_log,
            "status-r1",
            "burning_aura:applied",
            "second",
        );

        assert_eq!(replay_log.0.len(), 2);
        assert_eq!(replay_log.0[0].seq, 1);
        assert_eq!(replay_log.0[1].seq, 2);
        assert_eq!(replay_log.0[0].phase, "battle-event");
        assert_eq!(replay_log.0[1].summary, "burning_aura:applied");
    }

    #[test]
    fn action_trace_assigns_monotonic_sequence_numbers() {
        let mut trace = ActionTrace::default();
        push_turn_action_trace(
            &mut trace,
            3,
            Side::Enemy,
            TurnAction::Skill(SkillId::ShadowWingAssassinate),
            "first action",
        );
        push_named_action_trace(&mut trace, 3, Side::Enemy, "end_turn", "second action");

        assert_eq!(trace.0.len(), 2);
        assert_eq!(trace.0[0].seq, 1);
        assert_eq!(trace.0[1].seq, 2);
        assert_eq!(trace.0[0].action, "skill:ShadowWingAssassinate");
        assert_eq!(trace.0[1].action, "end_turn");
    }

    #[test]
    fn clear_runtime_battle_logs_clears_all_runtime_logs() {
        let mut battle_log = BattleLog(VecDeque::from(["a".to_string()]));
        let mut structured_log = StructuredBattleLog(VecDeque::from([StructuredLogEntry {
            phase: "phase".to_string(),
            summary: "summary".to_string(),
            detail: "detail".to_string(),
        }]));
        let mut replay_log = ReplayEventLog(vec![ReplayLogEntry {
            seq: 1,
            phase: "battle-event".to_string(),
            summary: "BattleEvent".to_string(),
            detail: "detail".to_string(),
        }]);
        let mut action_trace = ActionTrace(vec![ActionTraceEntry {
            seq: 1,
            round: 1,
            side: Side::Player,
            action: "skill:FirePunch".to_string(),
            detail: "detail".to_string(),
        }]);

        clear_runtime_battle_logs(
            &mut battle_log,
            &mut structured_log,
            &mut replay_log,
            &mut action_trace,
        );

        assert!(battle_log.0.is_empty());
        assert!(structured_log.0.is_empty());
        assert!(replay_log.0.is_empty());
        assert!(action_trace.0.is_empty());
    }

    #[test]
    fn owner_action_end_tick_does_not_decrement_duration() {
        let mut status_board = StatusBoard {
            entries: vec![StatusInstance {
                id: "wind_evade".to_string(),
                name: "闪避".to_string(),
                category: StatusCategory::Buff,
                remaining_turns: 1,
                applied_round: 1,
                source_side: Some(Side::Player),
                tick_timing: Some(StatusTickTiming::OwnerActionEnd),
                stage_modifiers: vec![],
                fixed_damage_on_tick: 0,
                heal_on_tick: 0,
                heal_taken_multiplier: None,
                evade_charges: 1,
            }],
        };

        let outcomes =
            tick_statuses_for_timing(&mut status_board, StatusTickTiming::OwnerActionEnd);

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].remaining_turns, 1);
        assert!(!outcomes[0].expired);
        assert_eq!(status_board.entries.len(), 1);
        assert_eq!(status_board.entries[0].remaining_turns, 1);
    }

    #[test]
    fn round_end_decrement_skips_same_round_and_expires_next_full_round() {
        let mut stats = test_stats();
        let mut status_board = StatusBoard {
            entries: vec![StatusInstance {
                id: "wind_evade".to_string(),
                name: "闪避".to_string(),
                category: StatusCategory::Buff,
                remaining_turns: 1,
                applied_round: 1,
                source_side: Some(Side::Player),
                tick_timing: Some(StatusTickTiming::OwnerActionEnd),
                stage_modifiers: vec![],
                fixed_damage_on_tick: 0,
                heal_on_tick: 0,
                heal_taken_multiplier: None,
                evade_charges: 1,
            }],
        };

        let first_round = decrement_status_durations_for_round(&mut status_board, &mut stats, 1);
        assert_eq!(first_round.len(), 1);
        assert_eq!(first_round[0].remaining_turns, 1);
        assert!(!first_round[0].expired);
        assert_eq!(status_board.entries.len(), 1);

        let second_round = decrement_status_durations_for_round(&mut status_board, &mut stats, 2);
        assert_eq!(second_round.len(), 1);
        assert_eq!(second_round[0].remaining_turns, 0);
        assert!(second_round[0].expired);
        assert!(status_board.entries.is_empty());
    }

    #[test]
    fn recalculate_stage_modifiers_clamps_each_attribute_stage() {
        let mut stats = test_stats();
        let status_board = StatusBoard {
            entries: vec![
                StatusInstance {
                    id: "buff_1".to_string(),
                    name: "buff".to_string(),
                    category: StatusCategory::Buff,
                    remaining_turns: 1,
                    applied_round: 1,
                    source_side: Some(Side::Player),
                    tick_timing: Some(StatusTickTiming::OwnerActionEnd),
                    stage_modifiers: vec![
                        StatusStageModifier {
                            attribute: AttributeType::Atk,
                            amount: 4,
                        },
                        StatusStageModifier {
                            attribute: AttributeType::Atk,
                            amount: 4,
                        },
                        StatusStageModifier {
                            attribute: AttributeType::Def,
                            amount: -4,
                        },
                    ],
                    fixed_damage_on_tick: 0,
                    heal_on_tick: 0,
                    heal_taken_multiplier: None,
                    evade_charges: 0,
                },
                StatusInstance {
                    id: "debuff_1".to_string(),
                    name: "debuff".to_string(),
                    category: StatusCategory::Debuff,
                    remaining_turns: 1,
                    applied_round: 1,
                    source_side: Some(Side::Enemy),
                    tick_timing: Some(StatusTickTiming::OwnerActionEnd),
                    stage_modifiers: vec![
                        StatusStageModifier {
                            attribute: AttributeType::Def,
                            amount: -4,
                        },
                        StatusStageModifier {
                            attribute: AttributeType::Spd,
                            amount: 6,
                        },
                        StatusStageModifier {
                            attribute: AttributeType::Acc,
                            amount: -6,
                        },
                    ],
                    fixed_damage_on_tick: 0,
                    heal_on_tick: 0,
                    heal_taken_multiplier: None,
                    evade_charges: 0,
                },
            ],
        };

        recalculate_stage_modifiers(&mut stats, &status_board);

        assert_eq!(stats.atk_stage, 6);
        assert_eq!(stats.def_stage, -6);
        assert_eq!(stats.spd_stage, 6);
        assert_eq!(stats.acc_stage, -6);
    }

    #[test]
    fn transfer_status_moves_nature_regen_to_new_active_combatant() {
        let mut old_stats = test_stats();
        let mut new_stats = test_stats();
        let mut old_statuses = StatusBoard {
            entries: vec![StatusInstance {
                id: "nature_regen".to_string(),
                name: "自然治愈".to_string(),
                category: StatusCategory::Buff,
                remaining_turns: 2,
                applied_round: 1,
                source_side: Some(Side::Player),
                tick_timing: Some(StatusTickTiming::OwnerActionEnd),
                stage_modifiers: vec![],
                fixed_damage_on_tick: 0,
                heal_on_tick: 5,
                heal_taken_multiplier: None,
                evade_charges: 0,
            }],
        };
        let mut new_statuses = StatusBoard::default();

        assert!(transfer_status_by_id(
            &mut old_statuses,
            &mut old_stats,
            &mut new_statuses,
            &mut new_stats,
            "nature_regen",
        ));

        assert!(old_statuses.entries.is_empty());
        assert_eq!(new_statuses.entries.len(), 1);

        let old_outcomes =
            tick_statuses_for_timing(&mut old_statuses, StatusTickTiming::OwnerActionEnd);
        let new_outcomes =
            tick_statuses_for_timing(&mut new_statuses, StatusTickTiming::OwnerActionEnd);

        assert!(old_outcomes.is_empty());
        assert_eq!(new_outcomes.len(), 1);
        assert_eq!(new_outcomes[0].status_id, "nature_regen");
        assert_eq!(new_outcomes[0].heal_amount, 5);
    }
}
