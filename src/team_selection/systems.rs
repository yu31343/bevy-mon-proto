use bevy::prelude::*;
use rand::seq::SliceRandom;

use crate::{
    data::{BattleRules, MonsterPool, TeamSelections},
    game_state::GameState,
    team_selection::{
        BackToLobbyButton, ConfirmSelectionButton, MonsterCardButton,
        MonsterCardSelectionIndicator, SelectionCountText, SelectionOrderText, SelectionState,
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
    selection_state: Res<SelectionState>,
    monster_pool: Res<MonsterPool>,
    rules: Res<BattleRules>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in &mut interaction_query {
        let selected_count = selection_state.selected_indices.len();
        if *interaction == Interaction::Pressed
            && selection_state.can_confirm()
            && selected_count <= rules.max_team_size
        {
            // Generate AI selection (same count as player, without duplicates)
            let enemy_indices = generate_ai_selection(monster_pool.monsters.len(), selected_count);

            // Log player selections
            println!("=== 队伍选择 ===");
            print!("玩家选择: ");
            for (i, &idx) in selection_state.selected_indices.iter().enumerate() {
                if i > 0 {
                    print!(", ");
                }
                print!("{}", monster_pool.monsters[idx].name);
            }
            println!();

            // Log AI selections
            print!("AI选择: ");
            for (i, &idx) in enemy_indices.iter().enumerate() {
                if i > 0 {
                    print!(", ");
                }
                print!("{}", monster_pool.monsters[idx].name);
            }
            println!();
            println!("================");

            // Insert TeamSelections resource
            commands.insert_resource(TeamSelections {
                player_indices: selection_state.selected_indices.clone(),
                enemy_indices,
            });

            // Transition to Battle state
            next_state.set(GameState::Battle);
        }
    }
}

/// System to handle return-to-lobby button click.
pub fn button_back_to_lobby_system(
    mut interaction_query: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<BackToLobbyButton>),
    >,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::Lobby);
        }
    }
}

/// System to update selection UI based on current state.
pub fn update_selection_ui_system(
    selection_state: Res<SelectionState>,
    rules: Res<BattleRules>,
    theme: Res<UiTheme>,
    mut count_text_query: Query<&mut Text, With<SelectionCountText>>,
    mut indicator_query: Query<(&MonsterCardSelectionIndicator, &mut Visibility)>,
    mut order_text_query: Query<(&SelectionOrderText, &mut Text), Without<SelectionCountText>>,
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
    // Update selection count text
    for mut text in &mut count_text_query {
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
    for (order_text, mut text) in &mut order_text_query {
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
    selection_state.selected_indices.clear();
}
