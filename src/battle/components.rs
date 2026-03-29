use std::collections::VecDeque;

use bevy::prelude::*;

use crate::data::SkillId;

/// 阵营标记：玩家或敌方。
#[derive(Component, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Side {
    Player,
    Enemy,
}

/// 战斗体组件：仅用于区分阵营。
#[derive(Component, Debug, Clone, Copy)]
pub struct Combatant {
    pub side: Side,
}

/// 战斗属性组件。
#[derive(Component, Debug, Clone, Copy)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
}

/// 技能栏：固定 4 个技能槽位，便于输入映射。
#[derive(Component, Debug, Clone, Copy)]
pub struct SkillList(pub [SkillId; 4]);

/// 护盾值：优先于生命值扣减。
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Shield(pub i32);

/// 战斗内实体标签，方便统一清理。
#[derive(Component, Debug, Clone, Copy)]
pub struct InBattle;

/// 当前回合双方已选技能。
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct TurnContext {
    pub player_skill: Option<SkillId>,
    pub enemy_skill: Option<SkillId>,
}

/// 战斗日志缓存（用于控制台与 UI 展示）。
#[derive(Resource, Debug, Default)]
pub struct BattleLog(pub VecDeque<String>);

/// 结算页面显示文本。
#[derive(Resource, Debug, Default)]
pub struct BattleResult {
    pub message: String,
}
