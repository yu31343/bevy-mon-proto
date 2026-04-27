use bevy::prelude::*;

use super::{components::*, resources::UiFontHandle, theme::UiTheme};

use crate::battle::Side;

pub(crate) fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

pub(crate) fn load_cjk_font_system(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    static EMBEDDED_UI_FONT: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/embedded_ui_font.bin"));

    let font = Font::try_from_bytes(EMBEDDED_UI_FONT.to_vec())
        .expect("embedded UI font should be valid and loadable");
    let handle = fonts.add(font);
    commands.insert_resource(UiFontHandle(handle));
}

fn spawn_hp_bar(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    border_1: UiRect,
    radius_hp: Val,
    fill: Color,
    fill_marker: impl Component,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(radius_hp),
                border: border_1,
                ..default()
            },
            BackgroundColor(theme.hp_track),
            BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.45)),
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Stretch,
                    border_radius: BorderRadius::all(radius_hp),
                    padding: UiRect::axes(Val::Px(2.0), Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(fill),
                fill_marker,
            ))
            .with_children(|fill_ent| {
                fill_ent.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(40.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.18)),
                ));
            });
        });
}

fn spawn_shield_bar(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    border_1: UiRect,
    _radius_hp: Val,
    fill: Color,
    fill_marker: impl Component,
    track_marker: impl Component,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(12.0),
                margin: UiRect::top(Val::Px(5.0)),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                border: border_1,
                ..default()
            },
            BackgroundColor(theme.shield_track),
            BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.30)),
            track_marker,
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(fill),
                fill_marker,
            ))
            .with_children(|fill_ent| {
                fill_ent.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(45.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15)),
                ));
            });
        });
}

fn spawn_small_bench_card(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    border_1: UiRect,
    radius_button: Val,
    title_font: TextFont,
    meta_font: TextFont,
    hotkey_font: TextFont,
    index: usize,
    side: Side,
) {
    let header_text = if matches!(side, Side::Player) {
        format!("键位 {}", index + 5)
    } else {
        "键位 -".to_string()
    };

    let card_node = Node {
        flex_grow: 1.0,
        min_width: Val::Px(104.0),
        min_height: Val::Px(82.0),
        padding: UiRect::all(Val::Px(8.0)),
        border: border_1,
        border_radius: BorderRadius::all(radius_button),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(5.0),
        ..default()
    };

    match side {
        Side::Player => {
            parent
                .spawn((
                    card_node,
                    BackgroundColor(theme.button_idle),
                    BorderColor::all(theme.button_border_idle),
                    theme.button_shadow(),
                    PlayerBenchCard { index },
                ))
                .with_children(|card| {
                    card.spawn((Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },))
                        .with_children(|header| {
                            header.spawn((
                                Text::new("待机位"),
                                title_font.clone(),
                                TextColor(theme.text_primary),
                            ));
                            header.spawn((
                                Text::new(header_text),
                                hotkey_font.clone(),
                                TextColor(theme.accent_player),
                            ));
                        });
                    card.spawn((
                        Text::new("队伍"),
                        title_font.clone(),
                        TextColor(theme.text_secondary),
                        PlayerBenchNameText { index },
                    ));
                    card.spawn((
                        Text::new("附着: 无"),
                        meta_font.clone(),
                        TextColor(theme.text_muted),
                        PlayerBenchAuraText { index },
                    ));
                    card.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(8.0),
                            overflow: Overflow::clip(),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(theme.hp_track),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(theme.hp_fill_player),
                            PlayerBenchHpBarFill { index },
                        ));
                    });
                    card.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(6.0),
                            overflow: Overflow::clip(),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(theme.shield_track),
                        Visibility::Hidden,
                        PlayerBenchShieldBarTrack { index },
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(theme.shield_fill_player),
                            PlayerBenchShieldBarFill { index },
                        ));
                    });
                });
        }
        Side::Enemy => {
            parent
                .spawn((
                    card_node,
                    BackgroundColor(theme.enemy_card_bg),
                    BorderColor::all(theme.enemy_card_border),
                    theme.button_shadow(),
                    EnemyBenchCard { index },
                ))
                .with_children(|card| {
                    card.spawn((Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },))
                        .with_children(|header| {
                            header.spawn((
                                Text::new("待机位"),
                                title_font.clone(),
                                TextColor(theme.text_primary),
                            ));
                            header.spawn((
                                Text::new(header_text),
                                hotkey_font.clone(),
                                TextColor(theme.accent_enemy),
                            ));
                        });
                    card.spawn((
                        Text::new("队伍"),
                        title_font.clone(),
                        TextColor(theme.text_secondary),
                        EnemyBenchNameText { index },
                    ));
                    card.spawn((
                        Text::new("附着: 无"),
                        meta_font.clone(),
                        TextColor(theme.text_muted),
                        EnemyBenchAuraText { index },
                    ));
                    card.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(8.0),
                            overflow: Overflow::clip(),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(theme.hp_track),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(theme.hp_fill_enemy),
                            EnemyBenchHpBarFill { index },
                        ));
                    });
                    card.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(6.0),
                            overflow: Overflow::clip(),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(theme.shield_track),
                        Visibility::Hidden,
                        EnemyBenchShieldBarTrack { index },
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(theme.shield_fill_enemy),
                            EnemyBenchShieldBarFill { index },
                        ));
                    });
                });
        }
    }
}

fn spawn_skill_row_player(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    border_1: UiRect,
    radius_button: Val,
    body_font: TextFont,
    meta_font: TextFont,
    icon_font: TextFont,
) {
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            align_items: AlignItems::Stretch,
            ..default()
        },))
        .with_children(|row| {
            for idx in 0..4 {
                row.spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(78.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                        align_items: AlignItems::FlexStart,
                        column_gap: Val::Px(6.0),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_button),
                        ..default()
                    },
                    BackgroundColor(theme.button_idle),
                    BorderColor::all(theme.button_border_idle),
                    theme.button_shadow(),
                    SkillButton { index: idx },
                    SkillSlotId {
                        side: Side::Player,
                        index: idx,
                    },
                ))
                .with_children(|button| {
                    button
                        .spawn((
                            Node {
                                width: Val::Px(26.0),
                                height: Val::Px(26.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(Val::Px(6.0)),
                                ..default()
                            },
                            ImageNode::solid_color(Color::srgba(0.22, 0.38, 0.52, 0.90)),
                        ))
                        .with_children(|icon_box| {
                            icon_box.spawn((
                                Text::new("?"),
                                icon_font.clone(),
                                TextColor(Color::srgb(0.85, 0.95, 1.0)),
                                SkillButtonIconText { index: idx },
                            ));
                        });

                    button
                        .spawn((Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(1.0),
                            flex_grow: 1.0,
                            ..default()
                        },))
                        .with_children(|column| {
                            column.spawn((
                                Text::new(format!("技能 {}", idx + 1)),
                                body_font.clone(),
                                TextColor(theme.text_primary),
                                SkillButtonText { index: idx },
                            ));
                            column.spawn((
                                Text::new("类型：--"),
                                meta_font.clone(),
                                TextColor(theme.text_secondary),
                                SkillButtonMetaText { index: idx },
                            ));
                        });
                });
            }
        });
}

pub(crate) fn setup_ui_system(
    mut commands: Commands,
    theme: Res<UiTheme>,
    ui_font: Option<Res<UiFontHandle>>,
    existing_ui: Query<(), With<BattleUiRoot>>,
) {
    if !existing_ui.is_empty() {
        return;
    }
    let radius_panel = theme.radius_panel;
    let radius_button = theme.radius_button;
    let radius_hp = theme.radius_hp;
    let radius_card = theme.radius_card;
    let border_1 = UiRect::all(Val::Px(1.0));

    let title_font = super::helpers::make_text_font(20.0, ui_font.as_deref());
    let body_font = super::helpers::make_text_font(16.0, ui_font.as_deref());
    let meta_font = super::helpers::make_text_font(13.0, ui_font.as_deref());
    let result_font = super::helpers::make_text_font(40.0, ui_font.as_deref());
    let icon_font = super::helpers::make_text_font(14.0, ui_font.as_deref());
    let small_font = super::helpers::make_text_font(11.0, ui_font.as_deref());
    let skill_header_font = super::helpers::make_text_font(17.0, ui_font.as_deref());
    let skill_body_font = super::helpers::make_text_font(14.0, ui_font.as_deref());
    let skill_meta_font = super::helpers::make_text_font(12.0, ui_font.as_deref());
    let skill_icon_font = super::helpers::make_text_font(12.0, ui_font.as_deref());

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            theme.root_background(),
            BattleUiRoot,
        ))
        .with_children(|root| {
            // === Top Bar ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    padding: UiRect::px(16.0, 16.0, 8.0, 10.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(24.0),
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                theme.top_bar_background(),
                BorderColor::all(theme.border_top_bar),
                theme.panel_shadow(),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Text::new("战斗阶段：准备中"),
                    body_font.clone(),
                    TextColor(theme.text_primary),
                    theme.title_text_shadow(),
                    BattlePhaseText,
                ));
                bar.spawn((
                    Text::new("AP：Player 0 / Enemy 0"),
                    body_font.clone(),
                    TextColor(theme.text_primary),
                    theme.title_text_shadow(),
                    ActionPointsText,
                ));
                bar.spawn((
                    Text::new("行为：等待操作"),
                    meta_font.clone(),
                    TextColor(theme.text_secondary),
                    TextShadow {
                        offset: Vec2::new(1.0, 1.0),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.40),
                    },
                    BattleActionText,
                ));
            });

            // === Player Info: Top-Left ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(52.0),
                    left: Val::Px(20.0),
                    width: Val::Px(320.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    border: border_1,
                    border_radius: BorderRadius::all(radius_panel),
                    ..default()
                },
                BackgroundColor(theme.panel),
                BorderColor::all(theme.border_panel),
                theme.panel_shadow(),
                PlayerInfoPanel,
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::px(0.0, 0.0, 4.0, 6.0),
                        border: UiRect::bottom(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(theme.divider),
                ))
                .with_children(|header| {
                    header.spawn((
                        Text::new("⚔ ..."),
                        title_font.clone(),
                        TextColor(theme.accent_player),
                        theme.title_text_shadow(),
                        PlayerNameText,
                    ));
                });
                panel.spawn((
                    Text::new("玩家：..."),
                    body_font.clone(),
                    TextColor(theme.text_primary),
                    theme.title_text_shadow(),
                    PlayerStatsText,
                ));
                spawn_hp_bar(
                    panel,
                    &theme,
                    border_1,
                    radius_hp,
                    theme.hp_fill_player,
                    PlayerHpBarFill,
                );
                spawn_shield_bar(
                    panel,
                    &theme,
                    border_1,
                    radius_hp,
                    theme.shield_fill_player,
                    PlayerShieldBarFill,
                    PlayerShieldBarTrack,
                );
            });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(236.0),
                    left: Val::Px(20.0),
                    width: Val::Px(320.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    ..default()
                },
            ))
            .with_children(|row| {
                for idx in 0..3 {
                    spawn_small_bench_card(
                        row,
                        &theme,
                        border_1,
                        radius_button,
                        meta_font.clone(),
                        small_font.clone(),
                        small_font.clone(),
                        idx,
                        Side::Player,
                    );
                }
            });


            // === Enemy Info: Top-Right (only name + HP + shield, no skills/team) ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(52.0),
                    right: Val::Px(20.0),
                    width: Val::Px(320.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    border: border_1,
                    border_radius: BorderRadius::all(radius_panel),
                    ..default()
                },
                BackgroundColor(theme.enemy_card_bg),
                BorderColor::all(theme.enemy_card_border),
                theme.panel_shadow(),
                EnemyInfoPanel,
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::px(0.0, 0.0, 4.0, 6.0),
                        border: UiRect::bottom(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(theme.divider),
                ))
                .with_children(|header| {
                    header.spawn((
                        Text::new("💀 ..."),
                        title_font.clone(),
                        TextColor(theme.accent_enemy),
                        theme.title_text_shadow(),
                        EnemyNameText,
                    ));
                });
                panel.spawn((
                    Text::new("敌方：..."),
                    body_font.clone(),
                    TextColor(theme.text_primary),
                    theme.title_text_shadow(),
                    EnemyStatsText,
                ));
                spawn_hp_bar(
                    panel,
                    &theme,
                    border_1,
                    radius_hp,
                    theme.hp_fill_enemy,
                    EnemyHpBarFill,
                );
                spawn_shield_bar(
                    panel,
                    &theme,
                    border_1,
                    radius_hp,
                    theme.shield_fill_enemy,
                    EnemyShieldBarFill,
                    EnemyShieldBarTrack,
                );
            });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(236.0),
                    right: Val::Px(20.0),
                    width: Val::Px(320.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    ..default()
                },
            ))
            .with_children(|row| {
                for idx in 0..3 {
                    spawn_small_bench_card(
                        row,
                        &theme,
                        border_1,
                        radius_button,
                        meta_font.clone(),
                        small_font.clone(),
                        small_font.clone(),
                        idx,
                        Side::Enemy,
                    );
                }
            });


            // === Hint Text: Center ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(52.0),
                    left: Val::Percent(30.0),
                    width: Val::Percent(40.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|hint_area| {
                hint_area.spawn((
                    Text::new("操作提示：按 1-4 使用精灵技能，按 Q 打开/关闭换人面板，按 5/6/7 切换我方队伍1/2/3，按 Z/X/C/V/B 使用手牌，按 F 弃牌换 AP，按 E 结束回合，按 R 重新开始"),
                    small_font.clone(),
                    TextColor(theme.text_muted),
                    TextShadow {
                        offset: Vec2::new(1.0, 1.0),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.35),
                    },
                    BattleHintText,
                ));
            });


            // === Bottom-Left: Control Buttons ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    bottom: Val::Px(16.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
            ))
            .with_children(|controls| {
                controls.spawn((
                    Button,
                    Node {
                        width: Val::Px(150.0),
                        min_height: Val::Px(46.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_button),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.button_idle),
                    BorderColor::all(theme.button_border_idle),
                    theme.button_shadow(),
                    DiscardButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("弃牌（F）"),
                        body_font.clone(),
                        TextColor(theme.text_primary),
                    ));
                });

                controls.spawn((
                    Button,
                    Node {
                        width: Val::Px(150.0),
                        min_height: Val::Px(46.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_button),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.button_idle),
                    BorderColor::all(theme.button_border_idle),
                    theme.button_shadow(),
                    EndTurnButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("结束回合（E）"),
                        body_font.clone(),
                        TextColor(theme.text_primary),
                    ));
                });
            });

            // === Bottom-Center: Hand Cards (Radial Fan Layout) ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.0),
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    height: Val::Px(350.0),
                    ..default()
                },
                HandCardsRoot,
            ))
            .with_children(|hand_container| {
                let card_w = 130.0_f32;
                let card_h = 180.0_f32;
                let total_cards = 5_f32;
                let radius = 260.0_f32;
                let half_spread = 50.0_f32;

                for idx in 0..5 {
                    let i = idx as f32;
                    let t = if total_cards > 1.0 {
                        (i / (total_cards - 1.0)) * 2.0 - 1.0
                    } else {
                        0.0
                    };
                    let angle_deg = t * half_spread;
                    let angle_rad = angle_deg.to_radians();

                    let cx = radius * angle_rad.sin();
                    let cy = radius * angle_rad.cos();

                    let z = idx as i32;

                    hand_container.spawn((
                        Button,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(50.0),
                            bottom: Val::Px(cy - card_h / 2.0),
                            width: Val::Px(card_w),
                            height: Val::Px(card_h),
                            margin: UiRect::left(Val::Px(cx - card_w / 2.0)),
                            padding: UiRect::px(10.0, 10.0, 12.0, 10.0),
                            border: border_1,
                            border_radius: BorderRadius::all(radius_card),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            ..default()
                        },
                        UiTransform::from_rotation(Rot2::radians(angle_rad)),
                        BackgroundColor(theme.card_bg),
                        BorderColor::all(theme.card_border),
                        theme.card_shadow(),
                        ZIndex(z),
                        PlayerCardButton { index: idx },
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Node {
                                width: Val::Px(24.0),
                                height: Val::Px(24.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(Val::Px(5.0)),
                                ..default()
                            },
                            ImageNode::solid_color(Color::srgba(0.18, 0.32, 0.48, 0.85)),
                        ))
                        .with_children(|hotkey_box| {
                            hotkey_box.spawn((
                                Text::new(super::helpers::card_hotkey_label(idx)),
                                icon_font.clone(),
                                TextColor(theme.accent_player),
                                PlayerCardHotkeyText { index: idx },
                            ));
                        });
                        card.spawn((
                            Text::new("—"),
                            body_font.clone(),
                            TextColor(theme.text_primary),
                            PlayerCardNameText { index: idx },
                        ));
                        card.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(1.0),
                                ..default()
                            },
                            BackgroundColor(theme.divider),
                        ));
                        card.spawn((
                            Text::new("AP—"),
                            meta_font.clone(),
                            TextColor(theme.text_secondary),
                            PlayerCardCostText { index: idx },
                        ));
                        card.spawn((
                            Text::new(""),
                            small_font.clone(),
                            TextColor(theme.text_muted),
                            Visibility::Hidden,
                            PlayerCardDescText { index: idx },
                        ));
                    });
                }
            });

            // === Bottom-Right: Switch Button + Skills ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(16.0),
                    bottom: Val::Px(16.0),
                    width: Val::Px(400.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                SkillPanelRoot,
            ))
            .with_children(|right_panel| {
                // Switch Monster Button
                right_panel.spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(42.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_button),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(theme.button_idle),
                    BorderColor::all(theme.button_border_idle),
                    theme.button_shadow(),
                    SwitchMonsterButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("换精灵（Q）"),
                        skill_body_font.clone(),
                        TextColor(theme.accent_player),
                    ));
                });

                // Skill Section
                right_panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_panel),
                        ..default()
                    },
                    BackgroundColor(theme.panel),
                    BorderColor::all(theme.border_panel),
                    theme.panel_shadow(),
                ))
                .with_children(|skill_section| {
                    skill_section.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::px(0.0, 0.0, 3.0, 5.0),
                            border: UiRect::bottom(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(theme.divider),
                    ))
                    .with_children(|header| {
                        header.spawn((
                            Text::new("技能"),
                            skill_header_font.clone(),
                            TextColor(theme.accent_player),
                            theme.title_text_shadow(),
                        ));
                    });
                    spawn_skill_row_player(
                        skill_section,
                        &theme,
                        border_1,
                        radius_button,
                        skill_body_font.clone(),
                        skill_meta_font.clone(),
                        skill_icon_font.clone(),
                    );
                });
            });

            // === Switch Monster Overlay (hidden by default) ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::Center,
                    padding: UiRect::px(40.0, 40.0, 40.0, 60.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.70)),
                SwitchOverlayRoot,
            ))
            .with_children(|overlay| {
                overlay.spawn((
                    Node {
                        width: Val::Px(600.0),
                        padding: UiRect::all(Val::Px(20.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(16.0),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_panel),
                        ..default()
                    },
                    BackgroundColor(theme.panel),
                    BorderColor::all(theme.border_panel),
                    theme.panel_shadow(),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::px(0.0, 0.0, 6.0, 8.0),
                            border: UiRect::bottom(Val::Px(1.0)),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(theme.divider),
                    ))
                    .with_children(|header| {
                        header.spawn((
                            Text::new("选择精灵"),
                            title_font.clone(),
                            TextColor(theme.accent_player),
                            theme.title_text_shadow(),
                        ));
                        header.spawn((
                            Button,
                            Node {
                                min_width: Val::Px(80.0),
                                min_height: Val::Px(36.0),
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                                border: border_1,
                                border_radius: BorderRadius::all(radius_button),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(theme.button_idle),
                            BorderColor::all(theme.button_border_idle),
                            theme.button_shadow(),
                            SwitchCancelButton,
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("取消"),
                                meta_font.clone(),
                                TextColor(theme.text_primary),
                            ));
                        });
                    });

                    panel.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(12.0),
                            row_gap: Val::Px(12.0),
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        for idx in 0..3 {
                            row.spawn((
                                Button,
                                Node {
                                    flex_grow: 1.0,
                                    min_height: Val::Px(100.0),
                                    padding: UiRect::all(Val::Px(12.0)),
                                    border: border_1,
                                    border_radius: BorderRadius::all(radius_button),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(6.0),
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(theme.button_idle),
                                BorderColor::all(theme.button_border_idle),
                                theme.button_shadow(),
                                TeamMemberButton { index: idx },
                            ))
                            .with_children(|p| {
                                p.spawn((
                                    Text::new(format!("{}键：队伍{}", idx + 5, idx + 1)),
                                    body_font.clone(),
                                    TextColor(theme.text_primary),
                                    TeamMemberButtonText { index: idx },
                                ));
                                p.spawn((
                                    Text::new("附着: 无"),
                                    meta_font.clone(),
                                    TextColor(theme.text_muted),
                                    TeamMemberAuraText { index: idx },
                                ));
                                p.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(10.0),
                                        overflow: Overflow::clip(),
                                        border_radius: BorderRadius::all(Val::Px(5.0)),
                                        ..default()
                                    },
                                    BackgroundColor(theme.hp_track),
                                ))
                                .with_children(|bar| {
                                    bar.spawn((
                                        Node {
                                            width: Val::Percent(100.0),
                                            height: Val::Percent(100.0),
                                            border_radius: BorderRadius::all(Val::Px(5.0)),
                                            ..default()
                                        },
                                        BackgroundColor(theme.hp_fill_player),
                                        TeamMemberHpBarFill { index: idx },
                                    ));
                                });
                                p.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(7.0),
                                        overflow: Overflow::clip(),
                                        border_radius: BorderRadius::all(Val::Px(3.5)),
                                        ..default()
                                    },
                                    BackgroundColor(theme.shield_track),
                                    Visibility::Hidden,
                                    TeamMemberShieldBarTrack { index: idx },
                                ))
                                .with_children(|bar| {
                                    bar.spawn((
                                        Node {
                                            width: Val::Percent(100.0),
                                            height: Val::Percent(100.0),
                                            border_radius: BorderRadius::all(Val::Px(3.5)),
                                            ..default()
                                        },
                                        BackgroundColor(theme.shield_fill_player),
                                        TeamMemberShieldBarFill { index: idx },
                                    ));
                                });
                            });
                        }
                    });
                });
            });

            // === Central: Result Text ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Percent(42.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ))
            .with_children(|res_node| {
                res_node.spawn((
                    Text::new(""),
                    result_font,
                    TextColor(Color::srgb(1.0, 0.82, 0.35)),
                    theme.result_text_shadow(),
                    ResultText,
                ));
            });
        });
}
