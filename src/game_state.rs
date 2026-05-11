use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// 顶层游戏状态：队伍选择、战斗中或结算页。
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    Lobby,
    Map,  // 新增地图状态
    MonsterDex,
    PvpLobby,
    TeamSelection,
    Battle,
    Result,
}

/// 战斗子状态：用于驱动 1v1 回合流程。
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default, Serialize, Deserialize)]
pub enum BattlePhase {
    #[default]
    Init,
    /// 每回合开始：抽牌、叠加 AP、重置出牌权标记。
    RoundStart,
    /// 玩家出牌/出招阶段（可多次连续行动，直到 AP 为 0 或手动结束）。
    PlayerTurn,
    /// 敌方行动阶段（AI 连续行动直到 AP 为 0）。
    EnemyTurn,
    /// 本回合结束判定与换人/胜负切换。
    CheckEnd,
    /// 等待死亡动画播放完成，再执行换人或进入结算。
    DeathResolve,
}
