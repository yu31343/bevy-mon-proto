#![allow(
    clippy::collapsible_if,
    clippy::derivable_impls,
    clippy::if_same_then_else,
    clippy::items_after_test_module,
    clippy::manual_contains,
    clippy::needless_borrow,
    clippy::needless_option_as_deref,
    clippy::needless_return,
    clippy::never_loop,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

//! 程序入口：注册插件与状态机，启动 Bevy App。
mod battle;
mod data;
mod game_state;
mod lobby;
mod pvp;
mod spine_anim;
mod team_selection;
mod ui;

use bevy::prelude::*;
use game_state::{BattlePhase, GameState};

fn main() {
    // 只在入口组装应用，业务逻辑全部放在各自插件中。
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<GameState>()
        .init_state::<BattlePhase>()
        .add_plugins((
            data::DataPlugin,
            ui::UiPlugin,
            lobby::LobbyPlugin,
            pvp::PvpPlugin,
            team_selection::TeamSelectionPlugin,
            battle::BattlePlugin,
            spine_anim::SpineAnimPlugin,
        ))
        .run();
}
