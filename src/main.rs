//! 程序入口：注册插件与状态机，启动 Bevy App。
mod battle;
mod data;
mod game_state;
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
            team_selection::TeamSelectionPlugin,
            battle::BattlePlugin,
        ))
        .run();
}
