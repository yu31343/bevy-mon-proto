mod components;
mod events;
mod systems;

use bevy::prelude::*;

use crate::game_state::{BattlePhase, GameState};

pub use components::*;
pub use events::*;

/// 战斗插件：注册战斗资源、事件与各阶段系统。
pub struct BattlePlugin;

impl Plugin for BattlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TurnContext>()
            .init_resource::<BattleLog>()
            .init_resource::<BattleResult>()
            .add_message::<BattleEvent>()
            .add_systems(
                Update,
                (
                    systems::init_battle_system.run_if(in_state(GameState::Battle).and(in_state(BattlePhase::Init))),
                    systems::player_input_system
                        .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerCommand))),
                    systems::enemy_choose_skill_system
                        .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::EnemyCommand))),
                    systems::resolve_turn_system
                        .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::Resolve))),
                    systems::check_end_system
                        .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::CheckEnd))),
                    systems::restart_from_result_system.run_if(in_state(GameState::Result)),
                    systems::consume_battle_events_system,
                ),
            );
    }
}
