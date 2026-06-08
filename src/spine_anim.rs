mod components;
mod config;
mod systems;

use bevy::prelude::*;
use bevy_spine::SpinePlugin;

use crate::game_state::GameState;

pub struct SpineAnimPlugin;

impl Plugin for SpineAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpinePlugin)
            .add_systems(Startup, systems::load_monster_skeletons)
            .add_systems(
                Update,
                systems::spawn_monster_ui_visuals.run_if(in_state(GameState::Battle)),
            )
            .add_systems(
                Update,
                (
                    systems::react_to_battle_events,
                    systems::sync_cursed_chain_vfx,
                    systems::sync_active_visibility_and_facing,
                    systems::handle_spine_animation_complete,
                    systems::handle_vfx_animation_complete,
                    systems::log_spine_ui_ready_events,
                    systems::log_spine_loader_failures,
                )
                    .run_if(in_state(GameState::Battle)),
            )
            .add_systems(
                Update,
                systems::tick_death_fade
                    .run_if(in_state(GameState::Battle).or(in_state(GameState::Result))),
            );
    }
}
