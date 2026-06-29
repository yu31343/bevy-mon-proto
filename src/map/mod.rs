pub mod components;
pub mod systems;

use crate::game_state::GameState;
use crate::map::components::{CurrentMap, MapNameBannerTimer};
use crate::ui::battle::{resources::UiFontHandle, theme::UiTheme};
use bevy::prelude::*;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentMap>()
            .init_resource::<MapNameBannerTimer>()
            .add_systems(
                OnEnter(GameState::Map),
                (
                    systems::spawn_map,
                    systems::spawn_character,
                    systems::spawn_sprites,
                    systems::setup_sprite_spine,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                systems::setup_map_ui.run_if(
                    in_state(GameState::Map)
                        .and(resource_exists::<UiTheme>)
                        .and(resource_exists::<UiFontHandle>),
                ),
            )
            .add_systems(
                Update,
                (
                    systems::move_character,
                    systems::click_sprites,
                    systems::map_enter_lobby_button_system,
                    systems::map_navigation_button_system,
                    systems::update_map_background_system,
                    systems::map_button_visual_system,
                    systems::map_name_banner_system,
                )
                    .chain()
                    .run_if(in_state(GameState::Map)),
            )
            .add_systems(OnExit(GameState::Map), systems::cleanup_map);
    }
}
