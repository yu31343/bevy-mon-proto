use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::{CardId, ElementType, SkillId};

use super::{Side, TurnAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageType {
    Direct,
    Fixed,
}

/// 战斗事件：逻辑层发出，日志/UI 统一消费。
#[derive(Message, Debug, Clone)]
pub enum BattleEvent {
    TurnStarted(u32),
    CardUsed {
        side: Side,
        card_id: CardId,
        card_name: String,
        cost_ap: i32,
    },
    CardDiscarded {
        side: Side,
        card_id: CardId,
        card_name: String,
        ap_gain: i32,
    },
    CardsDrawn {
        side: Side,
        count: usize,
    },
    SkillUsed {
        side: Side,
        skill_id: SkillId,
        skill_name: String,
        /// 技能在 4 格栏中的索引（用于 UI 高亮）。
        slot: usize,
        cost_ap: i32,
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
    ElementAuraApplied {
        side: Side,
        from: Vec<ElementType>,
        to: Vec<ElementType>,
        effectiveness: f32,
    },
    ReactionTriggered {
        source: Side,
        target: Side,
        reaction_id: String,
        reaction_name: String,
    },
    WindSpreadTriggered {
        source: Side,
        target: Side,
        element: ElementType,
    },
    WindSpreadSkipped {
        source: Side,
        target: Side,
        aura: Vec<ElementType>,
    },
    CombatantFainted {
        owner: Entity,
        side: Side,
        name: String,
    },
    Switched {
        side: Side,
        name: String,
    },
    NetworkInterrupted {
        reason: String,
    },
}

#[allow(dead_code)]
#[derive(Message, Debug, Clone)]
pub struct BattleTraceEvent {
    pub round: u32,
    pub side: Side,
    pub action: String,
    pub detail: String,
}

#[allow(dead_code)]
#[derive(Message, Debug, Clone)]
pub struct BattleStateEvent {
    pub round: u32,
    pub subject: Side,
    pub summary: String,
    pub detail: String,
}

#[derive(Message, Debug, Clone)]
pub struct BattleLifecycleEvent {
    pub phase: String,
    pub summary: String,
    pub detail: String,
}

#[allow(dead_code)]
#[derive(Message, Debug, Clone)]
pub struct BattleFormulaEvent {
    pub round: u32,
    pub source: Side,
    pub target: Side,
    pub action: String,
    pub detail: String,
}

#[allow(dead_code)]
#[derive(Message, Debug, Clone)]
pub struct BattleStatusEvent {
    pub round: u32,
    pub subject: Side,
    pub status_id: String,
    pub action: String,
    pub detail: String,
}

#[allow(dead_code)]
pub fn trace_from_turn_action(
    round: u32,
    side: Side,
    action: TurnAction,
    detail: impl Into<String>,
) -> BattleTraceEvent {
    let action = match action {
        TurnAction::Skill(skill_id) => format!("skill:{skill_id:?}"),
        TurnAction::Switch => "switch".to_string(),
    };
    BattleTraceEvent {
        round,
        side,
        action,
        detail: detail.into(),
    }
}

#[allow(dead_code)]
pub fn trace_named(
    round: u32,
    side: Side,
    action: impl Into<String>,
    detail: impl Into<String>,
) -> BattleTraceEvent {
    BattleTraceEvent {
        round,
        side,
        action: action.into(),
        detail: detail.into(),
    }
}

#[allow(dead_code)]
pub fn lifecycle_event(
    phase: impl Into<String>,
    summary: impl Into<String>,
    detail: impl Into<String>,
) -> BattleLifecycleEvent {
    BattleLifecycleEvent {
        phase: phase.into(),
        summary: summary.into(),
        detail: detail.into(),
    }
}

#[allow(dead_code)]
pub fn state_event(
    round: u32,
    subject: Side,
    summary: impl Into<String>,
    detail: impl Into<String>,
) -> BattleStateEvent {
    BattleStateEvent {
        round,
        subject,
        summary: summary.into(),
        detail: detail.into(),
    }
}

pub fn formula_event(
    round: u32,
    source: Side,
    target: Side,
    action: impl Into<String>,
    detail: impl Into<String>,
) -> BattleFormulaEvent {
    BattleFormulaEvent {
        round,
        source,
        target,
        action: action.into(),
        detail: detail.into(),
    }
}

pub fn status_event(
    round: u32,
    subject: Side,
    status_id: impl Into<String>,
    action: impl Into<String>,
    detail: impl Into<String>,
) -> BattleStatusEvent {
    BattleStatusEvent {
        round,
        subject,
        status_id: status_id.into(),
        action: action.into(),
        detail: detail.into(),
    }
}

#[allow(dead_code)]
pub fn lifecycle_phase_name(phase: &str) -> String {
    phase.to_string()
}

#[allow(dead_code)]
pub fn trace_action_name(action: &str) -> String {
    action.to_string()
}

#[allow(dead_code)]
pub fn trace_detail_text(detail: &str) -> String {
    detail.to_string()
}

#[allow(dead_code)]
pub fn side_label(side: Side) -> &'static str {
    match side {
        Side::Player => "player",
        Side::Enemy => "enemy",
    }
}

#[allow(dead_code)]
pub fn round_phase_label(round: u32, side: Side) -> String {
    format!("round-{round}-{}", side_label(side))
}

#[allow(dead_code)]
pub fn round_label(round: u32) -> String {
    format!("round-{round}")
}

#[allow(dead_code)]
pub fn trace_summary(event: &BattleTraceEvent) -> String {
    format!(
        "r{} {} {}",
        event.round,
        side_label(event.side),
        event.action
    )
}

#[allow(dead_code)]
pub fn state_summary(event: &BattleStateEvent) -> String {
    format!(
        "r{} {} {}",
        event.round,
        side_label(event.subject),
        event.summary
    )
}

#[allow(dead_code)]
pub fn formula_summary(event: &BattleFormulaEvent) -> String {
    format!(
        "r{} {}->{} {}",
        event.round,
        side_label(event.source),
        side_label(event.target),
        event.action
    )
}

#[allow(dead_code)]
pub fn status_summary(event: &BattleStatusEvent) -> String {
    format!(
        "r{} {} {} {}",
        event.round,
        side_label(event.subject),
        event.status_id,
        event.action
    )
}

#[allow(dead_code)]
pub fn lifecycle_summary(event: &BattleLifecycleEvent) -> String {
    format!("{} {}", event.phase, event.summary)
}
