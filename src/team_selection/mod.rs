mod components;
mod layout;
mod systems;

use bevy::prelude::*;

use crate::game_state::GameState;

pub use components::*;
pub use layout::*;
pub use systems::*;

pub struct TeamSelectionPlugin;

impl Plugin for TeamSelectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectionState>()
            .add_systems(
                Update,
                setup_selection_ui.run_if(
                    in_state(GameState::TeamSelection)
                        .and(resource_exists::<crate::data::MonsterPool>)
                        .and(resource_exists::<crate::ui::battle::theme::UiTheme>),
                ),
            )
            .add_systems(OnEnter(GameState::TeamSelection), clear_selection_state)
            .add_systems(OnExit(GameState::TeamSelection), cleanup_selection_ui)
            .add_systems(
                Update,
                (
                    button_select_monster_system,
                    button_confirm_selection_system,
                    button_back_to_lobby_system,
                    update_selection_ui_system,
                )
                    .run_if(in_state(GameState::TeamSelection)),
            );
    }
}
