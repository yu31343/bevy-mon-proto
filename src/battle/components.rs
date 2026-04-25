//! 战斗系统组件定义
//! 
//! 定义游戏中与战斗相关的所有组件、资源和辅助结构
//! 采用 Bevy 的 ECS 架构，将游戏对象分解为可组合的组件

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::data::{CardId, ElementType, SkillId};

/// 阵营标记：区分玩家和敌方
/// 
/// 用于标记实体的阵营归属，在战斗逻辑和UI显示中使用
#[derive(Component, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Side {
    /// 玩家阵营
    Player,
    /// 敌方阵营
    Enemy,
}

use std::fmt;

/// 为 Side 实现 Display trait，便于打印和日志输出
impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let side_str = match self {
            Side::Player => "Player",
            Side::Enemy => "Enemy",
        };
        write!(f, "{}", side_str)
    }
}

/// 队伍信息结构体
/// 
/// 管理一个队伍的精灵实体和当前活跃精灵
#[derive(Debug, Clone)]
pub struct Team {
    /// 队伍中的精灵实体列表
    pub combatants: Vec<Entity>,
    /// 当前活跃精灵的索引
    pub active_index: usize,
}

impl Team {
    /// 获取当前活跃的精灵实体
    /// 
    /// # Returns
    /// - `Some(Entity)`: 如果有活跃精灵
    /// - `None`: 如果队伍为空或索引无效
    pub fn active_combatant(&self) -> Option<Entity> {
        self.combatants.get(self.active_index).copied()
    }
}

/// 玩家队伍资源
/// 
/// 作为全局资源存储玩家的队伍信息
#[derive(Resource, Debug, Clone)]
pub struct PlayerTeam(pub Team);

/// 敌方队伍资源
/// 
/// 作为全局资源存储敌方的队伍信息
#[derive(Resource, Debug, Clone)]
pub struct EnemyTeam(pub Team);

/// 战斗体组件：标记战斗实体并存储阵营和元素类型
/// 
/// 用于识别战斗中的精灵，处理元素克制关系
#[derive(Component, Debug, Clone, Copy)]
pub struct Combatant {
    /// 所属阵营
    pub side: Side,
    /// 元素类型
    pub element: ElementType,
}

/// 战斗属性组件：存储精灵的基本属性
/// 
/// 包含生命值、攻击力、防御力和速度等核心属性
#[derive(Component, Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Stats {
    /// 当前生命值
    pub hp: i32,
    /// 最大生命值
    pub max_hp: i32,
    /// 攻击力
    pub atk: i32,
    /// 防御力
    pub def: i32,
    /// 速度（影响行动顺序）
    pub spd: i32,
}

/// 技能栏组件：存储精灵的技能列表
/// 
/// 固定4个技能槽位，对应键盘1-4键，便于输入映射
#[derive(Component, Debug, Clone, Copy)]
pub struct SkillList(pub [SkillId; 4]);

/// 技能栏实际可用槽位数量（1..=4）。
#[derive(Component, Debug, Clone, Copy)]
pub struct SkillCount(pub usize);

/// 护盾值：优先于生命值扣减。
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Shield(pub i32);

/// 元素附着组件：存储精灵的元素附着状态
/// 
/// 用于“元素附着/克制反应”与“盾免疫元素”的规则结算
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ElementAura {
    /// 当前附着的元素类型
    pub attached: Option<ElementType>,
}

/// 战斗内实体标签组件
/// 
/// 标记战斗中的实体，方便战斗结束时统一清理
#[derive(Component, Debug, Clone, Copy)]
pub struct InBattle;

/// 行动类型枚举：表示一次战斗行动
/// 
/// 用于记录和处理战斗中的行动选择
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TurnAction {
    /// 使用技能（包含技能ID）
    Skill(SkillId),
    /// 切换精灵
    Switch,
}

/// 回合上下文资源：存储当前回合的出牌权状态
/// 
/// 跟踪双方的行动状态和已选行动
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct TurnContext {
    /// 玩家是否已结束行动
    pub player_ended: bool,
    /// 敌方是否已结束行动
    pub enemy_ended: bool,
    /// 兼容旧原型：存储本回合玩家已选的“单次行动”（出招/换人）
    /// 新原型不会再依赖此字段，但保留以减少一次性重构成本
    pub player_action: Option<TurnAction>,
    /// 兼容旧原型：存储本回合敌方已选的“单次行动”（出招/换人）
    /// 新原型不会再依赖此字段，但保留以减少一次性重构成本
    pub enemy_action: Option<TurnAction>,
}

/// 战斗日志资源：存储战斗过程中的日志信息
/// 
/// 用于控制台输出和UI展示，限制最大条数以保持性能
#[derive(Resource, Debug, Default)]
pub struct BattleLog(pub VecDeque<String>);

/// 战斗日志最大条数
/// 
/// 与 `push_battle_line` 函数配合使用，确保日志数量不超过限制
pub const BATTLE_LOG_LIMIT: usize = 10;

/// 写入一条战斗日志
/// 
/// 将日志打印到终端并加入 `BattleLog` 资源，自动保持日志数量不超过限制
/// 
/// # Parameters
/// - `log`: 战斗日志资源的可变引用
/// - `line`: 要添加的日志内容
pub fn push_battle_line(log: &mut BattleLog, line: impl Into<String>) {
    let line = line.into();
    // 打印到终端
    println!("{line}");
    // 添加到日志队列
    log.0.push_back(line);
    // 保持日志数量不超过限制
    while log.0.len() > BATTLE_LOG_LIMIT {
        log.0.pop_front();
    }
}

/// 战斗结果资源：存储战斗结束后的结果信息
/// 
/// 用于在结算页面显示战斗结果
#[derive(Resource, Debug, Default)]
pub struct BattleResult {
    /// 结果消息文本
    pub message: String,
}

/// 死亡结算：等待动画播放完成后再换人/出结果。
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

/// 回合计数器资源：记录战斗的回合数
/// 
/// 用于游戏逻辑和UI显示
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct TurnCount(pub u32);

/// 行动点池（可跨回合继承；回合开始时按 BattleRules 叠加）。
#[derive(Resource, Debug, Clone)]
pub struct ActionPoints {
    /// 玩家的行动点
    pub player: i32,
    /// 敌方的行动点
    pub enemy: i32,
}

/// 为 ActionPoints 实现 Default trait
impl Default for ActionPoints {
    /// 创建默认的行动点池（初始为0）
    fn default() -> Self {
        Self {
            player: 0,
            enemy: 0,
        }
    }
}

/// 手牌资源：存储双方的手牌
/// 
/// 每回合从卡组抽取固定数量，出牌/弃牌会减少手牌
#[derive(Resource, Debug, Clone, Default)]
pub struct Hand {
    /// 玩家的手牌
    pub player: Vec<CardId>,
    /// 敌方的手牌
    pub enemy: Vec<CardId>,
}

/// 待命增益结构体：存储下次技能的增益效果
/// 
/// 在下一次对应类型的精灵技能结算时生效并清空
#[derive(Debug, Clone, Copy, Default)]
pub struct PendingBoost {
    /// 下次攻击的伤害增益
    pub next_attack_bonus: i32,
    /// 下次护盾的效果增益
    pub next_shield_bonus: i32,
    /// 下次治疗的效果增益
    pub next_heal_bonus: i32,
}

/// 双方的待命增益资源
/// 
/// 存储玩家和敌方的待命增益效果
#[derive(Resource, Debug, Clone, Default)]
pub struct PendingBoosts {
    /// 玩家的待命增益
    pub player: PendingBoost,
    /// 敌方的待命增益
    pub enemy: PendingBoost,
}

/// 当前选中的手牌索引资源
/// 
/// 用于两步式出牌/弃牌逻辑，键鼠共享状态
#[derive(Resource, Default, Clone, Copy)]
pub struct SelectedCard {
    /// 当前选中的手牌索引
    pub index: Option<usize>,
    /// 是否处于弃牌模式
    pub discard_armed: bool,
}
