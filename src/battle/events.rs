use bevy::prelude::*;

use crate::data::ElementType;

use super::Side;

/// 战斗事件：逻辑层发出，日志/UI 统一消费。
#[derive(Message, Debug, Clone)]
pub enum BattleEvent {
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
        /// 技能在 4 格栏中的索引（用于 UI 高亮）。
        slot: usize,
    },
    DamageDealt {
        source: Side,
        target: Side,
        amount: i32,
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
        from: Option<ElementType>,
        to: ElementType,
        effectiveness: f32,
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
}
