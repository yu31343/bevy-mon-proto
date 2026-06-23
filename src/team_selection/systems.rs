use bevy::prelude::*;
use rand::seq::SliceRandom;

use crate::{
    battle::BattleControlMode,
    console_log::{ConsoleLogCategory, log as console_log},
    data::{
        BattleRules, EnemyAiPolicyMode, EnemyAiPresets, MapBattleContext, MonsterPool,
        TeamSelections,
    },
    game_state::GameState,
    pvp::{PvpConnection, PvpIncomingIntents, PvpStatus, PvpTeamState, submit_local_team},
    team_selection::{
        AiDifficultyButton, AiDifficultyButtonText, AiDifficultySelectorRoot,
        AiDifficultySummaryText, AiPolicyButton, AiPolicyButtonText, AiPolicySummaryText,
        BackToLobbyButton, BackToLobbyButtonText, ConfirmSelectionButton,
        ConfirmSelectionButtonText, MonsterCardButton, MonsterCardSelectionIndicator,
        SelectedAiDifficulty, SelectedAiPolicy, SelectionCountText, SelectionEntryMode,
        SelectionInstructionsText, SelectionOrderText, SelectionStage, SelectionState,
        SelectionTitleText, ai_difficulty_description, ai_difficulty_label, ai_policy_label,
    },
    ui::battle::theme::UiTheme,
};

fn monster_names(indices: &[usize], monster_pool: &MonsterPool) -> String {
    let names = indices
        .iter()
        .filter_map(|&idx| monster_pool.monsters.get(idx))
        .map(|monster| monster.name.as_str())
        .collect::<Vec<_>>();
    if names.is_empty() {
        "无".to_string()
    } else {
        names.join(" / ")
    }
}

/// System to handle monster card button clicks.
pub fn button_select_monster_system(
    mut interaction_query: Query<
        (&Interaction, &MonsterCardButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut selection_state: ResMut<SelectionState>,
    entry_mode: Res<SelectionEntryMode>,
    pvp_team_state: Option<Res<PvpTeamState>>,
    rules: Res<BattleRules>,
) {
    if *entry_mode == SelectionEntryMode::Pvp
        && pvp_team_state
            .as_ref()
            .is_some_and(|team_state| team_state.local_indices.is_some())
    {
        return;
    }
    for (interaction, button) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            selection_state.toggle(button.monster_index, rules.max_team_size);
        }
    }
}

pub fn button_select_ai_difficulty_system(
    mut interaction_query: Query<
        (&Interaction, &AiDifficultyButton),
        (Changed<Interaction>, With<Button>),
    >,
    entry_mode: Res<SelectionEntryMode>,
    mut selected_ai: ResMut<SelectedAiDifficulty>,
) {
    if *entry_mode != SelectionEntryMode::VsAi {
        return;
    }
    for (interaction, button) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            selected_ai.difficulty = button.difficulty;
        }
    }
}

pub fn button_select_ai_policy_system(
    mut interaction_query: Query<
        (&Interaction, &AiPolicyButton),
        (Changed<Interaction>, With<Button>),
    >,
    entry_mode: Res<SelectionEntryMode>,
    mut selected_policy: ResMut<SelectedAiPolicy>,
) {
    if *entry_mode != SelectionEntryMode::VsAi {
        return;
    }
    for (interaction, button) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            selected_policy.mode = button.mode;
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
    selected_ai: Res<SelectedAiDifficulty>,
    selected_policy: Res<SelectedAiPolicy>,
    ai_presets: Res<EnemyAiPresets>,
    mut map_battle_context: ResMut<MapBattleContext>,
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
        if *entry_mode == SelectionEntryMode::Pvp {
            if pvp_team_state
                .as_ref()
                .is_some_and(|team_state| team_state.local_indices.is_some())
            {
                continue;
            }
            if pvp_connection.as_ref().is_none_or(|connection| {
                matches!(
                    connection.status,
                    PvpStatus::Disconnected(_) | PvpStatus::Failed(_)
                )
            }) {
                continue;
            }
        }

        match *entry_mode {
            SelectionEntryMode::VsAi => {
                let enemy_indices = if let Some(idx) = map_battle_context.enemy_monster_index {
                    if idx < monster_pool.monsters.len() {
                        vec![idx]
                    } else {
                        generate_ai_selection(monster_pool.monsters.len(), selected_count)
                    }
                } else {
                    generate_ai_selection(monster_pool.monsters.len(), selected_count)
                };

                console_log(ConsoleLogCategory::Selection, "=== 队伍选择 ===");
                console_log(
                    ConsoleLogCategory::Selection,
                    format!(
                        "玩家选择：{}",
                        monster_names(&selection_state.selected_indices, &monster_pool)
                    ),
                );
                console_log(
                    ConsoleLogCategory::Selection,
                    format!("AI选择：{}", monster_names(&enemy_indices, &monster_pool)),
                );
                console_log(
                    ConsoleLogCategory::Selection,
                    format!("AI难度：{}", ai_difficulty_label(selected_ai.difficulty)),
                );
                console_log(
                    ConsoleLogCategory::Selection,
                    format!("AI策略：{}", ai_policy_label(selected_policy.mode)),
                );
                console_log(ConsoleLogCategory::Selection, "================");

                commands.insert_resource(selected_ai.config(&ai_presets, *selected_policy));
                commands.insert_resource(BattleControlMode::PlayerVsAi);
                commands.insert_resource(TeamSelections {
                    player_indices: selection_state.selected_indices.clone(),
                    enemy_indices,
                });
                map_battle_context.enemy_monster_index = None;
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
                if team_state.local_indices.is_some() {
                    continue;
                }
                console_log(ConsoleLogCategory::Selection, "=== PVP 队伍选择 ===");
                console_log(
                    ConsoleLogCategory::Selection,
                    format!(
                        "我方选择：{}",
                        monster_names(&selection_state.selected_indices, &monster_pool)
                    ),
                );
                console_log(ConsoleLogCategory::Selection, "等待对方队伍...");
                console_log(ConsoleLogCategory::Selection, "================");
                submit_local_team(
                    connection,
                    team_state,
                    selection_state.selected_indices.clone(),
                );
            }
            SelectionEntryMode::Debug => {
                console_log(ConsoleLogCategory::Selection, "=== 调试模式队伍选择 ===");
                console_log(
                    ConsoleLogCategory::Selection,
                    format!(
                        "我方选择：{}",
                        monster_names(&selection_state.player_indices, &monster_pool)
                    ),
                );
                console_log(
                    ConsoleLogCategory::Selection,
                    format!(
                        "敌方选择：{}",
                        monster_names(&selection_state.selected_indices, &monster_pool)
                    ),
                );
                console_log(ConsoleLogCategory::Selection, "====================");

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
    mut pvp_connection: Option<ResMut<PvpConnection>>,
    mut pvp_team_state: Option<ResMut<PvpTeamState>>,
    mut pvp_incoming_intents: Option<ResMut<PvpIncomingIntents>>,
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
        if *entry_mode == SelectionEntryMode::Pvp {
            if let Some(connection) = pvp_connection.as_mut() {
                connection.stop_with_leave(Some("对方已返回大厅，联机配队已取消。".to_string()));
                connection.status = PvpStatus::Idle;
            }
            if let Some(team_state) = pvp_team_state.as_mut() {
                **team_state = PvpTeamState::default();
            }
            if let Some(incoming_intents) = pvp_incoming_intents.as_mut() {
                incoming_intents.0.clear();
            }
        }
        next_state.set(GameState::Lobby);
    }
}

/// System to update selection UI based on current state.
pub fn update_selection_ui_system(
    selection_state: Res<SelectionState>,
    entry_mode: Res<SelectionEntryMode>,
    pvp_connection: Option<Res<PvpConnection>>,
    pvp_team_state: Option<Res<PvpTeamState>>,
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
    let pvp_locked = *entry_mode == SelectionEntryMode::Pvp
        && pvp_team_state
            .as_ref()
            .is_some_and(|team_state| team_state.local_indices.is_some());
    let pvp_waiting_or_disconnected = if *entry_mode == SelectionEntryMode::Pvp {
        pvp_connection
            .as_ref()
            .and_then(|connection| match &connection.status {
                PvpStatus::Disconnected(reason) | PvpStatus::Failed(reason) => Some(reason.clone()),
                _ => None,
            })
    } else {
        None
    };
    let title = if selecting_enemy {
        "选择敌方队伍"
    } else {
        "选择我方队伍"
    };
    let instructions = if let Some(reason) = pvp_waiting_or_disconnected.as_ref() {
        format!("联机已取消：{reason}。请返回大厅重新开始。")
    } else if pvp_locked {
        "已确认队伍，等待对方选择。确认后不可再修改精灵。".to_string()
    } else if selecting_enemy {
        format!("选择 1-{} 个精灵组成敌方队伍", rules.max_team_size)
    } else {
        format!("选择 1-{} 个精灵组成我方队伍", rules.max_team_size)
    };
    let confirm_label = match (*entry_mode, selection_state.stage) {
        (SelectionEntryMode::VsAi, _) => "确认选择",
        (SelectionEntryMode::Pvp, _) if pvp_locked => "等待对方中",
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
            *bg = if pvp_locked {
                BackgroundColor(Color::srgb(0.14, 0.28, 0.40))
            } else {
                BackgroundColor(Color::srgb(0.2, 0.4, 0.6))
            };
            *border = if pvp_locked {
                BorderColor::all(Color::srgb(0.22, 0.42, 0.58))
            } else {
                BorderColor::all(Color::srgb(0.3, 0.6, 0.9))
            };
        } else if pvp_locked {
            *bg = BackgroundColor(Color::srgb(0.08, 0.10, 0.14));
            *border = BorderColor::all(Color::srgb(0.18, 0.22, 0.28));
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
            if !pvp_locked
                && selection_state.can_confirm()
                && selection_state.selected_indices.len() <= rules.max_team_size
                && pvp_waiting_or_disconnected.is_none()
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

pub fn update_ai_difficulty_ui_system(
    entry_mode: Res<SelectionEntryMode>,
    selected_ai: Res<SelectedAiDifficulty>,
    selected_policy: Res<SelectedAiPolicy>,
    ai_presets: Res<EnemyAiPresets>,
    theme: Res<UiTheme>,
    mut root_query: Query<&mut Visibility, With<AiDifficultySelectorRoot>>,
    mut button_query: Query<
        (
            &Interaction,
            &AiDifficultyButton,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<Button>,
    >,
    mut button_text_query: Query<
        (&AiDifficultyButtonText, &mut Text),
        Without<AiDifficultySummaryText>,
    >,
    mut summary_query: Query<
        &mut Text,
        (
            With<AiDifficultySummaryText>,
            Without<AiDifficultyButtonText>,
            Without<AiPolicyButtonText>,
            Without<AiPolicySummaryText>,
        ),
    >,
    mut policy_button_query: Query<
        (
            &Interaction,
            &AiPolicyButton,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (With<Button>, Without<AiDifficultyButton>),
    >,
    mut policy_text_query: Query<
        (&AiPolicyButtonText, &mut Text),
        (
            Without<AiDifficultySummaryText>,
            Without<AiDifficultyButtonText>,
            Without<AiPolicySummaryText>,
        ),
    >,
    mut policy_summary_query: Query<
        &mut Text,
        (
            With<AiPolicySummaryText>,
            Without<AiDifficultySummaryText>,
            Without<AiDifficultyButtonText>,
        ),
    >,
) {
    let visible = *entry_mode == SelectionEntryMode::VsAi;
    for mut visibility in &mut root_query {
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !visible {
        return;
    }

    for (interaction, button, mut bg, mut border) in &mut button_query {
        let selected = button.difficulty == selected_ai.difficulty;
        if selected {
            *bg = BackgroundColor(Color::srgb(0.18, 0.42, 0.74));
            *border = BorderColor::all(Color::srgb(0.45, 0.72, 1.0));
        } else if *interaction == Interaction::None {
            *bg = BackgroundColor(theme.button_idle);
            *border = BorderColor::all(theme.button_border_idle);
        }
    }

    for (button_text, mut text) in &mut button_text_query {
        **text = if button_text.difficulty == selected_ai.difficulty {
            format!("✓ {}", ai_difficulty_label(button_text.difficulty))
        } else {
            ai_difficulty_label(button_text.difficulty).to_string()
        };
    }

    for mut text in &mut summary_query {
        let config = selected_ai.base_config(&ai_presets);
        **text = format!(
            "当前：{} — {} 深度={}；候选={}；信息={:?}",
            ai_difficulty_label(selected_ai.difficulty),
            ai_difficulty_description(selected_ai.difficulty),
            config.search_depth,
            config.top_candidates,
            config.player_info_visibility
        );
    }

    for (interaction, button, mut bg, mut border) in &mut policy_button_query {
        let selected = button.mode == selected_policy.mode;
        if selected {
            *bg = BackgroundColor(Color::srgb(0.18, 0.42, 0.74));
            *border = BorderColor::all(Color::srgb(0.45, 0.72, 1.0));
        } else if *interaction == Interaction::None {
            *bg = BackgroundColor(theme.button_idle);
            *border = BorderColor::all(theme.button_border_idle);
        }
    }

    for (button_text, mut text) in &mut policy_text_query {
        **text = if button_text.mode == selected_policy.mode {
            format!("✓ {}", ai_policy_label(button_text.mode))
        } else {
            ai_policy_label(button_text.mode).to_string()
        };
    }

    for mut text in &mut policy_summary_query {
        let mut config = selected_ai.base_config(&ai_presets);
        selected_policy.apply_to_config(&mut config);
        **text = format!(
            "策略：{}；样本={}；模型={}",
            ai_policy_label(selected_policy.mode),
            if config.export_decision_samples {
                "开"
            } else {
                "关"
            },
            config.policy_model_path.as_deref().unwrap_or("无")
        );
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

pub fn reset_selected_ai_difficulty_system(
    mut selected_ai: ResMut<SelectedAiDifficulty>,
    ai_presets: Res<EnemyAiPresets>,
) {
    selected_ai.difficulty = ai_presets.default_difficulty;
}

pub fn reset_selected_ai_policy_system(mut selected_policy: ResMut<SelectedAiPolicy>) {
    selected_policy.mode = EnemyAiPolicyMode::Heuristic;
}
