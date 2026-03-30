mod components;
mod events;
mod systems;

use bevy::prelude::*;

use crate::game_state::{BattlePhase, GameState};

pub use components::*;
pub use events::*;

// #region agent log
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

fn agent_debug_log(
    hypothesis_id: &str,
    location: &str,
    message: &str,
    run_id: &str,
) {
    // Debug logs are best-effort; if writing fails, ignore to avoid hiding the original panic.
    let _ = (|| {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let id = format!("log_{}_{}", ts, hypothesis_id);
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("debug-648aa6.log");
        let mut f = OpenOptions::new().create(true).append(true).open(path)?;
        // All strings here are static and do not contain quotes/backslashes.
        let line = format!(
            "{{\"sessionId\":\"648aa6\",\"runId\":\"{}\",\"hypothesisId\":\"{}\",\"location\":\"{}\",\"message\":\"{}\",\"data\":{{}},\"timestamp\":{}}}\n",
            run_id, hypothesis_id, location, message, ts
        );
        f.write_all(line.as_bytes())?;
        f.flush()?;
        Ok::<(), std::io::Error>(())
    })();
}
// #endregion

/// 战斗插件：注册战斗资源、事件与各阶段系统。
pub struct BattlePlugin;

impl Plugin for BattlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TurnContext>()
            .init_resource::<BattleLog>()
            .init_resource::<BattleResult>()
            .init_resource::<TurnCount>()
            .init_resource::<ActionPoints>()
            .init_resource::<Hand>()
            .init_resource::<PendingBoosts>()
            .add_message::<BattleEvent>()
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

        // #region agent log
        agent_debug_log(
            "H1",
            "src/battle/mod.rs",
            "about_to_register_enemy_turn_ai_system",
            "instrumentation-pre",
        );
        // #endregion

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
        .add_systems(Update, systems::restart_from_result_system.run_if(in_state(GameState::Result)))
        .add_systems(Update, systems::consume_battle_events_system);
    }
}
