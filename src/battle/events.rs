use bevy::prelude::*;

use super::Side;

/// 战斗事件：逻辑层发出，日志/UI 统一消费。
#[derive(Message, Debug, Clone)]
pub enum BattleEvent {
    TurnStarted(u32),
    SkillUsed { side: Side, skill_name: String },
    DamageDealt { source: Side, target: Side, amount: i32 },
    Healed { side: Side, amount: i32 },
    ShieldGained { side: Side, amount: i32 },
    CombatantFainted { side: Side },
}
