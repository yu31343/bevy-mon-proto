use std::collections::VecDeque;

use bevy::prelude::*;

use crate::data::{CardId, ElementType, SkillId};

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
#[allow(dead_code)]
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

/// 技能栏实际可用槽位数量（1..=4）。
#[derive(Component, Debug, Clone, Copy)]
pub struct SkillCount(pub usize);

/// 护盾值：优先于生命值扣减。
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Shield(pub i32);

/// 元素附着（实时生效的元素 buff）。
/// - 用于“元素附着/克制反应”与“盾免疫元素”的规则结算。
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ElementAura {
    pub attached: Option<ElementType>,
}

/// 战斗内实体标签，方便统一清理。
#[derive(Component, Debug, Clone, Copy)]
pub struct InBattle;

/// 表达一次行动（出招或换人）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TurnAction {
    Skill(SkillId),
    Switch,
}

/// 当前回合的出牌权状态。
/// - `player_ended == true` 表示玩家已结束本方行动（无论是因为 AP 归零还是手动结束）。
/// - `enemy_ended == true` 表示敌方已结束本方行动。
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct TurnContext {
    pub player_ended: bool,
    pub enemy_ended: bool,
    /// 兼容旧原型：存储本回合双方已选的“单次行动”（出招/换人）。
    /// 新原型不会再依赖此字段，但保留以减少一次性重构成本。
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

/// 行动点池（可跨回合继承；回合开始时按 BattleRules 叠加）。
#[derive(Resource, Debug, Clone)]
pub struct ActionPoints {
    pub player: i32,
    pub enemy: i32,
}

impl Default for ActionPoints {
    fn default() -> Self {
        Self {
            player: 0,
            enemy: 0,
        }
    }
}

/// 手牌：每回合从卡组抽取固定数量，出牌/弃牌会减少手牌。
#[derive(Resource, Debug, Clone, Default)]
pub struct Hand {
    pub player: Vec<CardId>,
    pub enemy: Vec<CardId>,
}

/// 待命增益：在下一次对应类型的精灵技能结算时生效并清空。
#[derive(Debug, Clone, Copy, Default)]
pub struct PendingBoost {
    pub next_attack_bonus: i32,
    pub next_shield_bonus: i32,
    pub next_heal_bonus: i32,
}

/// 双方的待命增益。
#[derive(Resource, Debug, Clone, Default)]
pub struct PendingBoosts {
    pub player: PendingBoost,
    pub enemy: PendingBoost,
}

/// 当前选中的手牌索引（用于两步式出牌/弃牌逻辑，键鼠共享状态）。
#[derive(Resource, Default, Clone, Copy)]
pub struct SelectedCard {
    pub index: Option<usize>,
    pub discard_armed: bool,
}
