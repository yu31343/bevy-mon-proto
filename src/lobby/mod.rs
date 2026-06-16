use bevy::prelude::*;

use crate::{
    data::{BattleDbs, MapBattleContext, MonsterPool},
    game_state::GameState,
    map::components::CurrentMap,
    team_selection::SelectionEntryMode,
    ui::battle::{
        helpers::{monster_skill_ap_cost_ui, skill_meta, skill_name, skill_summary},
        resources::UiFontHandle,
        theme::UiTheme,
    },
};

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MonsterDexScrollOffset>()
            .init_resource::<MonsterDexDetailState>()
            .add_systems(OnExit(GameState::Lobby), cleanup_lobby_ui)
            .add_systems(
                Update,
                setup_lobby_ui.run_if(
                    in_state(GameState::Lobby)
                        .and(resource_exists::<UiTheme>)
                        .and(resource_exists::<UiFontHandle>),
                ),
            )
            .add_systems(
                Update,
                lobby_button_system.run_if(in_state(GameState::Lobby)),
            )
            .add_systems(
                Update,
                lobby_button_visual_system.run_if(in_state(GameState::Lobby)),
            )
            .add_systems(OnExit(GameState::MonsterDex), cleanup_monster_dex_ui)
            .add_systems(
                Update,
                setup_monster_dex_ui.run_if(
                    in_state(GameState::MonsterDex)
                        .and(resource_exists::<UiTheme>)
                        .and(resource_exists::<UiFontHandle>)
                        .and(resource_exists::<MonsterPool>),
                ),
            )
            .add_systems(
                Update,
                (
                    dex_back_button_system,
                    monster_dex_entry_click_system,
                    monster_dex_detail_close_system,
                    monster_dex_detail_visibility_system,
                    monster_dex_detail_update_system,
                    monster_dex_mouse_wheel_scroll_system,
                    monster_dex_scrollbar_update_system,
                )
                    .run_if(in_state(GameState::MonsterDex)),
            );
    }
}

#[derive(Component)]
struct LobbyUiRoot;

#[derive(Component)]
struct StartBattleButton;

#[derive(Component)]
struct MapButton; // 新增

#[derive(Component)]
struct OpenDexButton;

#[derive(Component)]
struct PvpBattleButton;

#[derive(Component)]
struct DebugBattleButton;

#[derive(Component)]
struct MonsterDexUiRoot;

#[derive(Component)]
struct DexBackButton;

#[derive(Component)]
struct MonsterDexListContent;

#[derive(Component)]
struct MonsterDexScrollbarTrack;

#[derive(Component)]
struct MonsterDexScrollbarThumb;

#[derive(Resource, Default)]
struct MonsterDexScrollOffset(f32);

#[derive(Resource, Default)]
struct MonsterDexDetailState {
    selected: Option<usize>,
    open: bool,
}

#[derive(Component)]
struct MonsterDexEntryButton {
    index: usize,
}

#[derive(Component)]
struct MonsterDexDetailOverlayRoot;

#[derive(Component)]
struct MonsterDexDetailBackdrop;

#[derive(Component)]
struct MonsterDexDetailCloseButton;

#[derive(Component)]
struct MonsterDexDetailNameText;

#[derive(Component)]
struct MonsterDexDetailStatsText;

#[derive(Component)]
struct MonsterDexDetailSkillsText;

fn setup_lobby_ui(
    mut commands: Commands,
    theme: Res<UiTheme>,
    ui_font: Option<Res<UiFontHandle>>,
    existing_ui: Query<(), With<LobbyUiRoot>>,
) {
    if !existing_ui.is_empty() {
        return;
    }

    let title_font = make_text_font(48.0, ui_font.as_deref());
    let body_font = make_text_font(22.0, ui_font.as_deref());
    let hint_font = make_text_font(16.0, ui_font.as_deref());

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
            theme.root_background(),
            LobbyUiRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("大厅"),
                title_font,
                TextColor(Color::srgb(0.95, 0.96, 1.0)),
                theme.title_text_shadow(),
            ));

            root.spawn((
                Text::new("选择你要进行的内容"),
                hint_font,
                TextColor(theme.text_secondary),
            ));

            root.spawn((
                Button,
                Node {
                    width: Val::Px(280.0),
                    min_height: Val::Px(58.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                StartBattleButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("与电脑对战"),
                    body_font.clone(),
                    TextColor(theme.text_primary),
                ));
            });

            root.spawn((
                Button,
                Node {
                    width: Val::Px(280.0),
                    min_height: Val::Px(58.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                MapButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("进入地图"),
                    body_font.clone(),
                    TextColor(theme.text_primary),
                ));
            });

            root.spawn((
                Button,
                Node {
                    width: Val::Px(280.0),
                    min_height: Val::Px(58.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                PvpBattleButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("联机对战"),
                    body_font.clone(),
                    TextColor(theme.text_primary),
                ));
            });

            root.spawn((
                Button,
                Node {
                    width: Val::Px(280.0),
                    min_height: Val::Px(58.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                OpenDexButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("查看精灵图鉴"),
                    body_font.clone(),
                    TextColor(theme.text_primary),
                ));
            });

            root.spawn((
                Button,
                Node {
                    width: Val::Px(280.0),
                    min_height: Val::Px(58.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                DebugBattleButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("调试模式"),
                    body_font,
                    TextColor(theme.text_primary),
                ));
            });
        });
}

fn cleanup_lobby_ui(mut commands: Commands, query: Query<Entity, With<LobbyUiRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn lobby_button_system(
    mut next_state: ResMut<NextState<GameState>>,
    mut entry_mode: ResMut<SelectionEntryMode>,
    mut map_battle_context: ResMut<MapBattleContext>,
    mut current_map: ResMut<CurrentMap>,
    mut battle_buttons: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<StartBattleButton>),
    >,
    mut dex_buttons: Query<&Interaction, (Changed<Interaction>, With<Button>, With<OpenDexButton>)>,
    mut pvp_buttons: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<PvpBattleButton>),
    >,
    mut debug_buttons: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<DebugBattleButton>),
    >,
    mut map_buttons: Query<&Interaction, (Changed<Interaction>, With<Button>, With<MapButton>)>, // 新增
) {
    for interaction in &mut battle_buttons {
        if *interaction == Interaction::Pressed {
            *map_battle_context = MapBattleContext::default();
            *entry_mode = SelectionEntryMode::VsAi;
            next_state.set(GameState::TeamSelection);
            return;
        }
    }

    for interaction in &mut pvp_buttons {
        if *interaction == Interaction::Pressed {
            *map_battle_context = MapBattleContext::default();
            *entry_mode = SelectionEntryMode::Pvp;
            next_state.set(GameState::PvpLobby);
            return;
        }
    }

    for interaction in &mut debug_buttons {
        if *interaction == Interaction::Pressed {
            *map_battle_context = MapBattleContext::default();
            *entry_mode = SelectionEntryMode::Debug;
            next_state.set(GameState::TeamSelection);
            return;
        }
    }

    for interaction in &mut dex_buttons {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::MonsterDex);
            return;
        }
    }

    for interaction in &mut map_buttons {
        // 新增
        if *interaction == Interaction::Pressed {
            *map_battle_context = MapBattleContext::default();
            *current_map = CurrentMap::Map1;
            next_state.set(GameState::Map);
            return;
        }
    }
}

fn lobby_button_visual_system(
    theme: Res<UiTheme>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            Changed<Interaction>,
            With<Button>,
            Or<(
                With<StartBattleButton>,
                With<MapButton>, // 新增
                With<PvpBattleButton>,
                With<OpenDexButton>,
                With<DebugBattleButton>,
            )>,
        ),
    >,
) {
    for (interaction, mut bg, mut border) in &mut buttons {
        *bg = match *interaction {
            Interaction::Pressed => BackgroundColor(theme.button_pressed),
            Interaction::Hovered => BackgroundColor(theme.button_hover),
            Interaction::None => BackgroundColor(theme.button_idle),
        };
        *border = match *interaction {
            Interaction::Pressed => BorderColor::all(theme.button_border_pressed),
            Interaction::Hovered => BorderColor::all(theme.button_border_hover),
            Interaction::None => BorderColor::all(theme.button_border_idle),
        };
    }
}

fn setup_monster_dex_ui(
    mut commands: Commands,
    theme: Res<UiTheme>,
    monster_pool: Res<MonsterPool>,
    mut scroll_offset: ResMut<MonsterDexScrollOffset>,
    mut detail_state: ResMut<MonsterDexDetailState>,
    ui_font: Option<Res<UiFontHandle>>,
    existing_ui: Query<(), With<MonsterDexUiRoot>>,
) {
    if !existing_ui.is_empty() {
        return;
    }
    scroll_offset.0 = 0.0;
    detail_state.selected = None;
    detail_state.open = false;

    let title_font = make_text_font(36.0, ui_font.as_deref());
    let body_font = make_text_font(18.0, ui_font.as_deref());
    let small_font = make_text_font(14.0, ui_font.as_deref());

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(28.0)),
                row_gap: Val::Px(12.0),
                ..default()
            },
            theme.root_background(),
            MonsterDexUiRoot,
        ))
        .with_children(|root| {
            root.spawn((Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },))
                .with_children(|top| {
                    top.spawn((
                        Text::new("精灵图鉴"),
                        title_font.clone(),
                        TextColor(Color::srgb(0.96, 0.97, 1.0)),
                        theme.title_text_shadow(),
                    ));

                    top.spawn((
                        Button,
                        Node {
                            min_width: Val::Px(120.0),
                            min_height: Val::Px(42.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(theme.radius_button),
                            ..default()
                        },
                        BackgroundColor(theme.button_idle),
                        BorderColor::all(theme.button_border_idle),
                        theme.button_shadow(),
                        DexBackButton,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("返回大厅"),
                            body_font.clone(),
                            TextColor(theme.text_primary),
                        ));
                    });
                });

            root.spawn((Node {
                width: Val::Percent(100.0),
                height: Val::Percent(86.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                ..default()
            },))
                .with_children(|row| {
                    row.spawn((
                        Node {
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            width: Val::Auto,
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::clip_y(),
                            padding: UiRect::all(Val::Px(12.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(theme.radius_panel),
                            ..default()
                        },
                        BackgroundColor(theme.panel),
                        BorderColor::all(theme.border_panel),
                    ))
                    .with_children(|viewport| {
                        viewport
                            .spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(10.0),
                                    ..default()
                                },
                                MonsterDexListContent,
                            ))
                            .with_children(|list| {
                                for (index, monster) in monster_pool.monsters.iter().enumerate() {
                                    list.spawn((
                                        Button,
                                        Node {
                                            width: Val::Percent(100.0),
                                            flex_direction: FlexDirection::Column,
                                            row_gap: Val::Px(4.0),
                                            padding: UiRect::all(Val::Px(10.0)),
                                            border: UiRect::all(Val::Px(1.0)),
                                            border_radius: BorderRadius::all(Val::Px(8.0)),
                                            ..default()
                                        },
                                        BackgroundColor(theme.enemy_card_bg),
                                        BorderColor::all(theme.enemy_card_border),
                                        MonsterDexEntryButton { index },
                                    ))
                                    .with_children(|card| {
                                        card.spawn((
                                            Text::new(format!(
                                                "{} ({:?})",
                                                monster.name, monster.element
                                            )),
                                            body_font.clone(),
                                            TextColor(theme.text_primary),
                                        ));
                                        card.spawn((
                                            Text::new(format!(
                                                "HP {}  ATK {}  DEF {}  SPD {}  ACC {}",
                                                monster.stats.hp,
                                                monster.stats.atk,
                                                monster.stats.def,
                                                monster.stats.spd,
                                                monster.stats.acc
                                            )),
                                            small_font.clone(),
                                            TextColor(theme.text_secondary),
                                        ));
                                        card.spawn((
                                            Text::new(format!(
                                                "技能数：{}（点击查看详情）",
                                                monster.skills.len()
                                            )),
                                            small_font.clone(),
                                            TextColor(theme.text_muted),
                                        ));
                                    });
                                }
                            });
                    });

                    row.spawn((
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(2.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(12.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.15, 0.18, 0.28, 1.0)),
                        BorderColor::all(Color::srgba(0.25, 0.28, 0.38, 1.0)),
                        MonsterDexScrollbarTrack,
                    ))
                    .with_children(|track| {
                        track.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(60.0),
                                border_radius: BorderRadius::all(Val::Px(10.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.6, 0.65, 0.75, 1.0)),
                            MonsterDexScrollbarThumb,
                        ));
                    });
                });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::None,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                MonsterDexDetailOverlayRoot,
            ))
            .with_children(|overlay| {
                overlay.spawn((
                    Button,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(0.0),
                        left: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.68)),
                    BorderColor::all(Color::NONE),
                    MonsterDexDetailBackdrop,
                ));

                overlay
                    .spawn((
                        Node {
                            width: Val::Px(760.0),
                            max_height: Val::Percent(82.0),
                            padding: UiRect::all(Val::Px(18.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(10.0),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(theme.radius_panel),
                            overflow: Overflow::clip_y(),
                            ..default()
                        },
                        BackgroundColor(theme.panel),
                        BorderColor::all(theme.border_panel),
                        theme.panel_shadow(),
                    ))
                    .with_children(|panel| {
                        panel
                            .spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    justify_content: JustifyContent::SpaceBetween,
                                    align_items: AlignItems::Center,
                                    border: UiRect::bottom(Val::Px(1.0)),
                                    padding: UiRect::bottom(Val::Px(8.0)),
                                    ..default()
                                },
                                BorderColor::all(theme.divider),
                            ))
                            .with_children(|header| {
                                header.spawn((
                                    Text::new("精灵详情"),
                                    body_font.clone(),
                                    TextColor(theme.text_primary),
                                    MonsterDexDetailNameText,
                                ));

                                header
                                    .spawn((
                                        Button,
                                        Node {
                                            min_width: Val::Px(80.0),
                                            min_height: Val::Px(34.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(1.0)),
                                            border_radius: BorderRadius::all(theme.radius_button),
                                            ..default()
                                        },
                                        BackgroundColor(theme.button_idle),
                                        BorderColor::all(theme.button_border_idle),
                                        theme.button_shadow(),
                                        MonsterDexDetailCloseButton,
                                    ))
                                    .with_children(|btn| {
                                        btn.spawn((
                                            Text::new("关闭"),
                                            small_font.clone(),
                                            TextColor(theme.text_primary),
                                        ));
                                    });
                            });

                        panel.spawn((
                            Text::new(""),
                            small_font.clone(),
                            TextColor(theme.text_secondary),
                            MonsterDexDetailStatsText,
                        ));

                        panel.spawn((
                            Text::new(""),
                            small_font.clone(),
                            TextColor(theme.text_primary),
                            MonsterDexDetailSkillsText,
                        ));
                    });
            });
        });
}

fn cleanup_monster_dex_ui(mut commands: Commands, query: Query<Entity, With<MonsterDexUiRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn dex_back_button_system(
    mut next_state: ResMut<NextState<GameState>>,
    mut back_buttons: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<DexBackButton>),
    >,
) {
    for interaction in &mut back_buttons {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::Lobby);
            return;
        }
    }
}

fn monster_dex_entry_click_system(
    mut detail_state: ResMut<MonsterDexDetailState>,
    mut entries: Query<
        (&Interaction, &MonsterDexEntryButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, entry) in &mut entries {
        if *interaction == Interaction::Pressed {
            detail_state.selected = Some(entry.index);
            detail_state.open = true;
            return;
        }
    }
}

fn monster_dex_detail_close_system(
    mut detail_state: ResMut<MonsterDexDetailState>,
    mut close_buttons: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<Button>,
            With<MonsterDexDetailCloseButton>,
        ),
    >,
    mut backdrops: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<Button>,
            With<MonsterDexDetailBackdrop>,
        ),
    >,
) {
    for interaction in &mut close_buttons {
        if *interaction == Interaction::Pressed {
            detail_state.open = false;
            return;
        }
    }

    for interaction in &mut backdrops {
        if *interaction == Interaction::Pressed {
            detail_state.open = false;
            return;
        }
    }
}

fn monster_dex_detail_visibility_system(
    detail_state: Res<MonsterDexDetailState>,
    mut overlays: Query<&mut Node, With<MonsterDexDetailOverlayRoot>>,
) {
    for mut node in &mut overlays {
        node.display = if detail_state.open {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn monster_dex_detail_update_system(
    detail_state: Res<MonsterDexDetailState>,
    monster_pool: Res<MonsterPool>,
    battle_dbs: Res<BattleDbs>,
    mut detail_texts: Query<(
        &mut Text,
        Option<&MonsterDexDetailNameText>,
        Option<&MonsterDexDetailStatsText>,
        Option<&MonsterDexDetailSkillsText>,
    )>,
) {
    if !detail_state.is_changed() {
        return;
    }

    let Some(index) = detail_state.selected else {
        return;
    };
    let Some(monster) = monster_pool.monsters.get(index) else {
        return;
    };

    let name_value = format!("{}（{:?}）", monster.name, monster.element);
    let stat_value = format!(
        "HP {}  ATK {}  DEF {}  SPD {}  ACC {}",
        monster.stats.hp,
        monster.stats.atk,
        monster.stats.def,
        monster.stats.spd,
        monster.stats.acc
    );

    let mut lines = Vec::with_capacity(monster.skills.len() * 4 + 1);
    for (slot, skill_id) in monster.skills.iter().copied().enumerate() {
        lines.push(format!(
            "{}号：{}",
            slot + 1,
            skill_name(skill_id, &battle_dbs)
        ));
        lines.push(skill_meta(skill_id, &battle_dbs));
        lines.push(format!(
            "AP消耗：{}",
            monster_skill_ap_cost_ui(skill_id, &battle_dbs)
        ));
        lines.push(skill_summary(skill_id, &battle_dbs));
        lines.push(String::new());
    }
    if lines.is_empty() {
        lines.push("该精灵暂无技能配置".to_string());
    }
    let skills_value = lines.join("\n");

    for (mut text, is_name, is_stats, is_skills) in &mut detail_texts {
        if is_name.is_some() {
            text.0 = name_value.clone();
        } else if is_stats.is_some() {
            text.0 = stat_value.clone();
        } else if is_skills.is_some() {
            text.0 = skills_value.clone();
        }
    }
}

fn monster_dex_mouse_wheel_scroll_system(
    mut wheel_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mut scroll_offset: ResMut<MonsterDexScrollOffset>,
    monster_pool: Res<MonsterPool>,
    mut list_query: Query<
        &mut Node,
        (
            With<MonsterDexListContent>,
            Without<MonsterDexScrollbarThumb>,
        ),
    >,
    mut thumb_query: Query<
        &mut Node,
        (
            With<MonsterDexScrollbarThumb>,
            Without<MonsterDexListContent>,
        ),
    >,
) {
    let mut wheel_y = 0.0_f32;
    for event in wheel_events.read() {
        wheel_y += event.y;
    }
    if wheel_y.abs() <= f32::EPSILON {
        return;
    }

    let content_estimated_height = monster_pool.monsters.len() as f32 * 100.0;
    let viewport_estimated_height = 600.0_f32;
    let max_offset = (content_estimated_height - viewport_estimated_height).max(0.0);
    scroll_offset.0 = (scroll_offset.0 - wheel_y * 36.0).clamp(0.0, max_offset);

    for mut node in list_query.iter_mut() {
        node.margin.top = Val::Px(-scroll_offset.0);
    }

    if max_offset > 0.0 {
        let scroll_ratio = scroll_offset.0 / max_offset;
        let track_height = 600.0;
        let thumb_height = 60.0;
        let max_thumb_offset = track_height - thumb_height;
        let thumb_offset = scroll_ratio * max_thumb_offset;

        for mut thumb_node in thumb_query.iter_mut() {
            thumb_node.margin.top = Val::Px(thumb_offset);
        }
    }
}

fn monster_dex_scrollbar_update_system(
    scroll_offset: Res<MonsterDexScrollOffset>,
    monster_pool: Res<MonsterPool>,
    mut thumb_query: Query<
        &mut Node,
        (
            With<MonsterDexScrollbarThumb>,
            Without<MonsterDexListContent>,
        ),
    >,
) {
    let content_estimated_height = monster_pool.monsters.len() as f32 * 100.0;
    let viewport_estimated_height = 600.0_f32;
    let max_offset = (content_estimated_height - viewport_estimated_height).max(0.0);

    if max_offset > 0.0 {
        let scroll_ratio = scroll_offset.0 / max_offset;
        let track_height = 600.0;
        let thumb_height = 60.0;
        let max_thumb_offset = track_height - thumb_height;
        let thumb_offset = scroll_ratio * max_thumb_offset;

        for mut thumb_node in thumb_query.iter_mut() {
            thumb_node.margin.top = Val::Px(thumb_offset);
        }
    }
}

fn make_text_font(size: f32, ui_font: Option<&UiFontHandle>) -> TextFont {
    if let Some(ui_font) = ui_font {
        TextFont {
            font: ui_font.0.clone(),
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
