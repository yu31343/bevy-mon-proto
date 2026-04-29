use bevy::prelude::*;
use rand::seq::SliceRandom;

use crate::{
    battle::BattleControlMode,
    data::{BattleRules, MonsterPool, TeamSelections},
    game_state::GameState,
    pvp::{PvpConnection, PvpTeamState, submit_local_team},
    team_selection::{
        BackToLobbyButton, BackToLobbyButtonText, ConfirmSelectionButton,
        ConfirmSelectionButtonText, MonsterCardButton, MonsterCardSelectionIndicator,
        SelectionCountText, SelectionEntryMode, SelectionInstructionsText, SelectionOrderText,
        SelectionStage, SelectionState, SelectionTitleText,
    },
    ui::battle::theme::UiTheme,
};

/// System to handle monster card button clicks.
pub fn button_select_monster_system(
    mut interaction_query: Query<
        (&Interaction, &MonsterCardButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut selection_state: ResMut<SelectionState>,
    rules: Res<BattleRules>,
) {
    for (interaction, button) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            selection_state.toggle(button.monster_index, rules.max_team_size);
        }
    }
}

/// System to handle confirm button click.
pub fn button_confirm_selection_system(
    mut interaction_query: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<Button>,
            With<ConfirmSelectionButton>,
        ),
    >,
    mut selection_state: ResMut<SelectionState>,
    entry_mode: Res<SelectionEntryMode>,
    monster_pool: Res<MonsterPool>,
    rules: Res<BattleRules>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    pvp_connection: Option<Res<PvpConnection>>,
    mut pvp_team_state: Option<ResMut<PvpTeamState>>,
) {
    for interaction in &mut interaction_query {
        let selected_count = selection_state.selected_indices.len();
        if *interaction != Interaction::Pressed
            || !selection_state.can_confirm()
            || selected_count > rules.max_team_size
        {
            continue;
        }

        match *entry_mode {
            SelectionEntryMode::VsAi => {
                let enemy_indices =
                    generate_ai_selection(monster_pool.monsters.len(), selected_count);

                println!("=== 队伍选择 ===");
                print!("玩家选择: ");
                for (i, &idx) in selection_state.selected_indices.iter().enumerate() {
                    if i > 0 {
                        print!(", ");
                    }
                    print!("{}", monster_pool.monsters[idx].name);
                }
                println!();
                print!("AI选择: ");
                for (i, &idx) in enemy_indices.iter().enumerate() {
                    if i > 0 {
                        print!(", ");
                    }
                    print!("{}", monster_pool.monsters[idx].name);
                }
                println!();
                println!("================");

                commands.insert_resource(BattleControlMode::PlayerVsAi);
                commands.insert_resource(TeamSelections {
                    player_indices: selection_state.selected_indices.clone(),
                    enemy_indices,
                });
                next_state.set(GameState::Battle);
            }
            SelectionEntryMode::Debug if selection_state.stage == SelectionStage::Player => {
                selection_state.player_indices = selection_state.selected_indices.clone();
                selection_state.selected_indices.clear();
                selection_state.stage = SelectionStage::Enemy;
            }
            SelectionEntryMode::Pvp => {
                let Some(connection) = pvp_connection.as_ref() else {
                    continue;
                };
                let Some(team_state) = pvp_team_state.as_mut() else {
                    continue;
                };
                println!("=== PVP 队伍选择 ===");
                print!("我方选择: ");
                for (i, &idx) in selection_state.selected_indices.iter().enumerate() {
                    if i > 0 {
                        print!(", ");
                    }
                    print!("{}", monster_pool.monsters[idx].name);
                }
                println!();
                println!("等待对方队伍...");
                println!("================");
                submit_local_team(
                    connection,
                    team_state,
                    selection_state.selected_indices.clone(),
                );
            }
            SelectionEntryMode::Debug => {
                println!("=== 调试模式队伍选择 ===");
                print!("我方选择: ");
                for (i, &idx) in selection_state.player_indices.iter().enumerate() {
                    if i > 0 {
                        print!(", ");
                    }
                    print!("{}", monster_pool.monsters[idx].name);
                }
                println!();
                print!("敌方选择: ");
                for (i, &idx) in selection_state.selected_indices.iter().enumerate() {
                    if i > 0 {
                        print!(", ");
                    }
                    print!("{}", monster_pool.monsters[idx].name);
                }
                println!();
                println!("====================");

                commands.insert_resource(BattleControlMode::DebugPlayerControlsBoth);
                commands.insert_resource(TeamSelections {
                    player_indices: selection_state.player_indices.clone(),
                    enemy_indices: selection_state.selected_indices.clone(),
                });
                next_state.set(GameState::Battle);
            }
        }
    }
}

/// System to handle return-to-lobby button click.
pub fn button_back_to_lobby_system(
    mut interaction_query: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<BackToLobbyButton>),
    >,
    entry_mode: Res<SelectionEntryMode>,
    mut selection_state: ResMut<SelectionState>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if *entry_mode == SelectionEntryMode::Debug
            && selection_state.stage == SelectionStage::Enemy
        {
            selection_state.stage = SelectionStage::Player;
            selection_state.selected_indices = selection_state.player_indices.clone();
            selection_state.player_indices.clear();
            continue;
        }
        next_state.set(GameState::Lobby);
    }
}

/// System to update selection UI based on current state.
pub fn update_selection_ui_system(
    selection_state: Res<SelectionState>,
    entry_mode: Res<SelectionEntryMode>,
    rules: Res<BattleRules>,
    theme: Res<UiTheme>,
    mut text_queries: ParamSet<(
        Query<&mut Text, With<SelectionTitleText>>,
        Query<&mut Text, With<SelectionInstructionsText>>,
        Query<&mut Text, With<SelectionCountText>>,
        Query<&mut Text, With<ConfirmSelectionButtonText>>,
        Query<&mut Text, With<BackToLobbyButtonText>>,
        Query<(&SelectionOrderText, &mut Text), Without<SelectionCountText>>,
    )>,
    mut indicator_query: Query<(&MonsterCardSelectionIndicator, &mut Visibility)>,
    mut card_query: Query<
        (
            &Interaction,
            &MonsterCardButton,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<Button>,
    >,
    mut confirm_button_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            With<Button>,
            With<ConfirmSelectionButton>,
            Without<MonsterCardButton>,
        ),
    >,
) {
    let selecting_enemy =
        *entry_mode == SelectionEntryMode::Debug && selection_state.stage == SelectionStage::Enemy;
    let title = if selecting_enemy {
        "选择敌方队伍"
    } else {
        "选择我方队伍"
    };
    let instructions = if selecting_enemy {
        format!("选择 1-{} 个精灵组成敌方队伍", rules.max_team_size)
    } else {
        format!("选择 1-{} 个精灵组成我方队伍", rules.max_team_size)
    };
    let confirm_label = match (*entry_mode, selection_state.stage) {
        (SelectionEntryMode::VsAi, _) => "确认选择",
        (SelectionEntryMode::Pvp, _) => "确认并等待对方",
        (SelectionEntryMode::Debug, SelectionStage::Player) => "下一步",
        (SelectionEntryMode::Debug, SelectionStage::Enemy) => "开始调试对战",
    };
    let back_label = if selecting_enemy {
        "返回上一步"
    } else {
        "返回大厅"
    };

    for mut text in &mut text_queries.p0() {
        **text = title.to_string();
    }
    for mut text in &mut text_queries.p1() {
        **text = instructions.clone();
    }
    for mut text in &mut text_queries.p3() {
        **text = confirm_label.to_string();
    }
    for mut text in &mut text_queries.p4() {
        **text = back_label.to_string();
    }
    for mut text in &mut text_queries.p2() {
        **text = format!(
            "已选择: {} / {}",
            selection_state.selected_indices.len(),
            rules.max_team_size
        );
    }

    // Update selection indicators visibility and order numbers
    for (indicator, mut visibility) in &mut indicator_query {
        if selection_state
            .selected_indices
            .iter()
            .any(|&idx| idx == indicator.monster_index)
        {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }

    // Update order text to show 1/2/3 based on selection order
    for (order_text, mut text) in &mut text_queries.p5() {
        if let Some(position) = selection_state
            .selected_indices
            .iter()
            .position(|&idx| idx == order_text.monster_index)
        {
            **text = format!("{}", position + 1);
        }
    }

    // Update monster card colors (immediate response to selection state)
    for (interaction, button, mut bg, mut border) in &mut card_query {
        let is_selected = selection_state.is_selected(button.monster_index);

        // Apply selection colors immediately, regardless of interaction state
        if is_selected {
            *bg = BackgroundColor(Color::srgb(0.2, 0.4, 0.6));
            *border = BorderColor::all(Color::srgb(0.3, 0.6, 0.9));
        } else {
            // Only apply default colors when not hovering/pressing
            if *interaction == Interaction::None {
                *bg = BackgroundColor(theme.button_idle);
                *border = BorderColor::all(theme.button_border_idle);
            }
        }
    }

    // Update confirm button state
    for (interaction, mut bg, mut border) in &mut confirm_button_query {
        if *interaction == Interaction::None {
            if selection_state.can_confirm()
                && selection_state.selected_indices.len() <= rules.max_team_size
            {
                *bg = BackgroundColor(theme.button_idle);
                *border = BorderColor::all(theme.button_border_idle);
            } else {
                *bg = BackgroundColor(Color::srgb(0.3, 0.3, 0.3));
                *border = BorderColor::all(Color::srgb(0.4, 0.4, 0.4));
            }
        }
    }
}

/// Generate AI team selection (same count as player, no duplicates within AI team).
fn generate_ai_selection(pool_size: usize, count: usize) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..pool_size).collect();
    indices.shuffle(&mut rand::thread_rng());
    indices.into_iter().take(count).collect()
}

/// System to clear selection state when entering TeamSelection state.
pub fn clear_selection_state(mut selection_state: ResMut<SelectionState>) {
    selection_state.reset();
}
