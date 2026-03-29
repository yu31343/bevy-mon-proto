use bevy::prelude::*;

/// 顶层游戏状态：战斗中或结算页。
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    Battle,
    Result,
}

/// 战斗子状态：用于驱动 1v1 回合流程。
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum BattlePhase {
    #[default]
    Init,
    PlayerCommand,
    EnemyCommand,
    Resolve,
    CheckEnd,
}
