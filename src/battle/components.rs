//! 战斗系统组件定义

use std::{collections::VecDeque, fmt};

use bevy::prelude::*;
use serde::Serialize;

use crate::{
    data::{
        AttributeType, CardId, ElementType, SkillId, StatusCategory, StatusDef, StatusTickTiming,
    },
    game_state::BattlePhase,
};

#[derive(Component, Debug, Clone, Copy, Eq, PartialEq, Serialize)]
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

#[derive(Debug, Clone)]
pub struct StatusStageModifier {
    pub attribute: AttributeType,
    pub amount: i32,
}

#[derive(Debug, Clone)]
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

    status_board.entries.retain(|entry| entry.remaining_turns > 0);
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

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct TurnContext {
    pub player_ended: bool,
    pub enemy_ended: bool,
    pub player_end_requested: bool,
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
}

impl Default for PendingKoResolution {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            player_switch_index: None,
            enemy_switch_index: None,
            player_defeated: false,
            enemy_defeated: false,
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

#[derive(Debug, Clone, Copy, Default)]
pub struct PendingBoost {
    pub next_attack_bonus: i32,
    pub next_shield_bonus: i32,
    pub next_heal_bonus: i32,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct PendingBoosts {
    pub player: PendingBoost,
    pub enemy: PendingBoost,
}

#[derive(Resource, Default, Clone, Copy)]
pub struct SelectedCard {
    pub index: Option<usize>,
    pub discard_armed: bool,
}

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
    println!("{line}");
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
    log.0.push_back(StructuredLogEntry {
        phase: phase.into(),
        summary: summary.into(),
        detail: detail.into(),
    });
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
}

pub fn reset_round_end_flags(turn_ctx: &mut TurnContext) {
    turn_ctx.player_ended = false;
    turn_ctx.enemy_ended = false;
    turn_ctx.player_end_requested = false;
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
    use crate::data::{SkillId, StatusCategory, StatusTickTiming};

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

        let outcomes = tick_statuses_for_timing(&mut status_board, StatusTickTiming::OwnerActionEnd);

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].remaining_turns, 1);
        assert!(!outcomes[0].expired);
        assert_eq!(status_board.entries.len(), 1);
        assert_eq!(status_board.entries[0].remaining_turns, 1);
    }

    #[test]
    fn round_end_decrement_skips_same_round_and_expires_next_full_round() {
        let mut stats = Stats {
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
        };
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
}
