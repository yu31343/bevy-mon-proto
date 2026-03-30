use std::collections::VecDeque;

use bevy::prelude::*;

use crate::data::{ElementType, SkillId};

/// 阵营标记：玩家或敌方。
#[derive(Component, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Side {
    Player,
    Enemy,
}

use std::fmt;

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let side_str = match self {
            Side::Player => "Player",
            Side::Enemy => "Enemy",
        };
        write!(f, "{}", side_str)
    }
}

/// 队伍信息
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

/// 玩家队伍资源
#[derive(Resource, Debug, Clone)]
pub struct PlayerTeam(pub Team);

/// 敌方队伍资源
#[derive(Resource, Debug, Clone)]
pub struct EnemyTeam(pub Team);

/// 战斗体组件：包含阵营和元素类型。
#[derive(Component, Debug, Clone, Copy)]
pub struct Combatant {
    pub side: Side,
    pub element: ElementType,
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

/// 表达一次行动（出招或换人）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnAction {
    Skill(SkillId),
    Switch,
}

/// 当前回合双方已选行动。
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct TurnContext {
    pub player_action: Option<TurnAction>,
    pub enemy_action: Option<TurnAction>,
}

/// 战斗日志缓存（用于控制台与 UI 展示）。
#[derive(Resource, Debug, Default)]
pub struct BattleLog(pub VecDeque<String>);

/// 战斗日志最大条数（与 `push_battle_line` 一致）。
pub const BATTLE_LOG_LIMIT: usize = 10;

/// 写入一条战斗日志：打印到终端并加入 `BattleLog`。
pub fn push_battle_line(log: &mut BattleLog, line: impl Into<String>) {
    let line = line.into();
    println!("{line}");
    log.0.push_back(line);
    while log.0.len() > BATTLE_LOG_LIMIT {
        log.0.pop_front();
    }
}

/// 结算页面显示文本。
#[derive(Resource, Debug, Default)]
pub struct BattleResult {
    pub message: String,
}

/// 回合计数器。
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct TurnCount(pub u32);
