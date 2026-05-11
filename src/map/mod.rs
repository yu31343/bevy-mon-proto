pub mod components;
pub mod systems;

use bevy::prelude::*;
use crate::game_state::GameState;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(OnEnter(GameState::Map), (
                systems::spawn_map,
                systems::spawn_character,
                systems::spawn_sprites,
                systems::setup_sprite_spine,
            ))
            .add_systems(Update, (
                systems::move_character,
                systems::click_sprites,
            ).run_if(in_state(GameState::Map)))
            .add_systems(OnExit(GameState::Map), systems::cleanup_map);
    }
}