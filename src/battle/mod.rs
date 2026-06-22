pub(crate) mod ai;
mod components;
mod events;
mod systems;

use bevy::prelude::*;

use crate::game_state::{BattlePhase, GameState};

pub use components::*;
pub use events::*;
pub(crate) use systems::{SideEndTickParams, player_action_cooldown_ready};

/// 战斗插件：注册战斗资源、事件与各阶段系统。
pub struct BattlePlugin;

impl Plugin for BattlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TurnContext>()
            .init_resource::<BattleActionCooldown>()
            .init_resource::<BattleLog>()
            .init_resource::<StructuredBattleLog>()
            .init_resource::<ReplayEventLog>()
            .init_resource::<ActionTrace>()
            .init_resource::<BattleResult>()
            .init_resource::<PendingBattleResultAction>()
            .init_resource::<BattleResultNotice>()
            .init_resource::<PendingKoResolution>()
            .init_resource::<TurnCount>()
            .init_resource::<RoundTransition>()
            .init_resource::<ActionPoints>()
            .init_resource::<RoundOrder>()
            .init_resource::<AccuracyRng>()
            .init_resource::<Hand>()
            .init_resource::<CardPiles>()
            .init_resource::<PendingBoosts>()
            .init_resource::<CardTurnMemory>()
            .init_resource::<BattleControlMode>()
            .init_resource::<UiControlSide>()
            .init_resource::<SelectedCards>()
            .add_message::<BattleEvent>()
            .add_message::<BattleTraceEvent>()
            .add_message::<BattleStateEvent>()
            .add_message::<BattleLifecycleEvent>()
            .add_message::<BattleFormulaEvent>()
            .add_message::<BattleStatusEvent>()
            .add_systems(
                Update,
                systems::tick_battle_action_cooldown_system.run_if(in_state(GameState::Battle)),
            )
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
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn)))
                    .run_if(systems::player_action_cooldown_ready),
            )
            .add_systems(
                Update,
                systems::hand_discard_phase_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::Discard)))
                    .run_if(systems::discard_action_cooldown_ready),
            );
        app.add_systems(
            Update,
            (
                systems::enemy_turn_ai_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::EnemyTurn)))
                    .run_if(systems::enemy_action_cooldown_ready),
                systems::enemy_turn_input_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::EnemyTurn)))
                    .run_if(systems::enemy_action_cooldown_ready),
                systems::sync_ui_control_side_system.run_if(in_state(GameState::Battle)),
            ),
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
        .add_systems(
            Update,
            (
                systems::card_trigger_event_system,
                systems::start_battle_action_cooldown_system,
                systems::consume_battle_events_system,
            )
                .chain()
                .after(systems::player_turn_input_system)
                .after(systems::hand_discard_phase_system)
                .after(systems::enemy_turn_ai_system)
                .after(systems::enemy_turn_input_system),
        );
    }
}
