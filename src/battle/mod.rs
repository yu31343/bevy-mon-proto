mod components;
mod events;
mod systems;

use bevy::prelude::*;

use crate::game_state::{BattlePhase, GameState};

pub use components::*;
pub use events::*;
pub(crate) use systems::SideEndTickParams;

/// 战斗插件：注册战斗资源、事件与各阶段系统。
pub struct BattlePlugin;

impl Plugin for BattlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TurnContext>()
            .init_resource::<BattleLog>()
            .init_resource::<StructuredBattleLog>()
            .init_resource::<ReplayEventLog>()
            .init_resource::<ActionTrace>()
            .init_resource::<BattleResult>()
            .init_resource::<PendingKoResolution>()
            .init_resource::<TurnCount>()
            .init_resource::<ActionPoints>()
            .init_resource::<RoundOrder>()
            .init_resource::<AccuracyRng>()
            .init_resource::<Hand>()
            .init_resource::<PendingBoosts>()
            .init_resource::<SelectedCard>()
            .add_message::<BattleEvent>()
            .add_message::<BattleTraceEvent>()
            .add_message::<BattleStateEvent>()
            .add_message::<BattleLifecycleEvent>()
            .add_message::<BattleFormulaEvent>()
            .add_message::<BattleStatusEvent>()
            .add_systems(
                Update,
                systems::init_battle_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::Init))),
            )
            .add_systems(
                Update,
                systems::round_start_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::RoundStart))),
            )
            .add_systems(
                Update,
                systems::player_turn_input_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn))),
            );
        app.add_systems(
            Update,
            systems::enemy_turn_ai_system
                .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::EnemyTurn))),
        );

        app.add_systems(
            Update,
            systems::check_end_system
                .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::CheckEnd))),
        )
        .add_systems(
            Update,
            systems::resolve_ko_system
                .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::DeathResolve))),
        )
        .add_systems(
            Update,
            systems::restart_from_result_system.run_if(in_state(GameState::Result)),
        )
        .add_systems(Update, systems::consume_battle_events_system);
    }
}
