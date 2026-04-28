use bevy::prelude::*;

use crate::{
    data::{BattleRules, MonsterPool},
    team_selection::{
        BackToLobbyButton, ConfirmSelectionButton, MonsterCardButton,
        MonsterCardSelectionIndicator, SelectionCountText, SelectionOrderText, SelectionUiRoot,
    },
    ui::battle::{resources::UiFontHandle, theme::UiTheme},
};

/// Setup the team selection UI (runs once when resources are ready).
pub fn setup_selection_ui(
    mut commands: Commands,
    theme: Res<UiTheme>,
    monster_pool: Res<MonsterPool>,
    rules: Res<BattleRules>,
    ui_font: Option<Res<UiFontHandle>>,
    existing_ui: Query<(), With<SelectionUiRoot>>,
) {
    // Only spawn UI once
    if !existing_ui.is_empty() {
        return;
    }

    println!(
        "正在创建队伍选择界面... 可选精灵数: {}",
        monster_pool.monsters.len()
    );

    let font_handle = ui_font.as_ref().map(|f| f.0.clone());
    // Root container
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                padding: UiRect::all(Val::Px(40.0)),
                ..default()
            },
            theme.root_background(),
            SelectionUiRoot,
        ))
        .with_children(|root| {
            // Title
            root.spawn((
                Text::new("选择你的队伍"),
                make_text_font(32.0, font_handle.as_ref()),
                TextColor(Color::WHITE),
                TextShadow {
                    offset: Vec2::new(2.0, 2.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.8),
                },
                Node {
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
            ));

            // Instructions
            root.spawn((
                Text::new(format!("选择 1-{} 个精灵组成你的队伍", rules.max_team_size)),
                make_text_font(16.0, font_handle.as_ref()),
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
                Node {
                    margin: UiRect::bottom(Val::Px(15.0)),
                    ..default()
                },
            ));

            // Selection count
            root.spawn((
                Text::new(format!("已选择: 0 / {}", rules.max_team_size)),
                make_text_font(20.0, font_handle.as_ref()),
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                Node {
                    margin: UiRect::bottom(Val::Px(25.0)),
                    ..default()
                },
                SelectionCountText,
            ));

            // Scrollable container for monster cards
            root.spawn((
                Node {
                    width: Val::Percent(90.0),
                    max_width: Val::Px(1000.0),
                    height: Val::Px(500.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::FlexStart,
                    row_gap: Val::Px(20.0),
                    column_gap: Val::Px(20.0),
                    overflow: Overflow::scroll_y(),
                    padding: UiRect::all(Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.2)),
            ))
            .with_children(|grid| {
                // Spawn a card for each monster
                for (index, monster) in monster_pool.monsters.iter().enumerate() {
                    spawn_monster_card(grid, &theme, index, monster, font_handle.as_ref());
                }
            });

            root.spawn((Node {
                margin: UiRect::top(Val::Px(25.0)),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                ..default()
            },))
                .with_children(|row| {
                    row.spawn((
                        Button,
                        Node {
                            width: Val::Px(200.0),
                            height: Val::Px(50.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(theme.radius_button),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                        BorderColor::all(Color::srgb(0.4, 0.4, 0.4)),
                        theme.button_shadow(),
                        ConfirmSelectionButton,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("确认选择"),
                            make_text_font(18.0, font_handle.as_ref()),
                            TextColor(Color::WHITE),
                        ));
                    });

                    row.spawn((
                        Button,
                        Node {
                            width: Val::Px(200.0),
                            height: Val::Px(50.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(theme.radius_button),
                            ..default()
                        },
                        BackgroundColor(theme.button_idle),
                        BorderColor::all(theme.button_border_idle),
                        theme.button_shadow(),
                        BackToLobbyButton,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("返回大厅"),
                            make_text_font(18.0, font_handle.as_ref()),
                            TextColor(Color::WHITE),
                        ));
                    });
                });
        });
}

/// Spawn a single monster card.
fn spawn_monster_card(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    index: usize,
    monster: &crate::data::MonsterPrototype,
    font_handle: Option<&Handle<Font>>,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(220.0),
                height: Val::Px(180.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(15.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(theme.radius_button),
                ..default()
            },
            BackgroundColor(theme.button_idle),
            BorderColor::all(theme.button_border_idle),
            theme.button_shadow(),
            MonsterCardButton {
                monster_index: index,
            },
        ))
        .with_children(|card| {
            // Selection indicator (checkmark, hidden by default)
            card.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(10.0),
                    right: Val::Px(10.0),
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(16.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.7, 0.3)),
                BorderColor::all(Color::srgb(0.3, 0.9, 0.4)),
                Visibility::Hidden,
                MonsterCardSelectionIndicator {
                    monster_index: index,
                },
            ))
            .with_children(|indicator| {
                indicator.spawn((
                    Text::new("1"),
                    make_text_font(20.0, font_handle),
                    TextColor(Color::WHITE),
                    SelectionOrderText {
                        monster_index: index,
                    },
                ));
            });

            // Monster name
            card.spawn((
                Text::new(&monster.name),
                make_text_font(18.0, font_handle),
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
            ));

            // Element
            card.spawn((
                Text::new(format!("元素: {:?}", monster.element)),
                make_text_font(14.0, font_handle),
                TextColor(Color::srgb(0.9, 0.9, 0.6)),
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
            ));

            // Stats
            card.spawn((
                Text::new(format!(
                    "HP:{} ATK:{} DEF:{} SPD:{}",
                    monster.stats.hp, monster.stats.atk, monster.stats.def, monster.stats.spd
                )),
                make_text_font(13.0, font_handle),
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
            ));
        });
}

/// Cleanup the team selection UI.
pub fn cleanup_selection_ui(mut commands: Commands, query: Query<Entity, With<SelectionUiRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

/// Helper function to create TextFont with optional CJK font.
fn make_text_font(size: f32, font_handle: Option<&Handle<Font>>) -> TextFont {
    if let Some(handle) = font_handle {
        TextFont {
            font: handle.clone(),
            font_size: size,
            ..default()
        }
    } else {
        TextFont {
            font_size: size,
            ..default()
        }
    }
}
