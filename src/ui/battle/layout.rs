use bevy::prelude::*;
use std::fs;

use super::{
    components::*,
    resources::UiFontHandle,
    theme::UiTheme,
};

use crate::battle::Side;

pub(crate) fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

pub(crate) fn load_cjk_font_system(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let candidates = [
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/msyh.ttf",
        "C:/Windows/Fonts/simhei.ttf",
        "C:/Windows/Fonts/simsun.ttc",
        "C:/Windows/Fonts/simkai.ttf",
    ];

    for path in candidates {
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let Ok(font) = Font::try_from_bytes(bytes) else {
            continue;
        };
        let handle = fonts.add(font);
        commands.insert_resource(UiFontHandle(handle));
        return;
    }
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
                height: Val::Px(14.0),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(radius_hp),
                border: border_1,
                ..default()
            },
            BackgroundColor(theme.hp_track),
            BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.35)),
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
                    padding: UiRect::axes(Val::Px(1.0), Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(fill),
                fill_marker,
            ))
            .with_children(|fill_ent| {
                fill_ent.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(36.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
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
                height: Val::Px(10.0),
                margin: UiRect::top(Val::Px(4.0)),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                border: border_1,
                ..default()
            },
            BackgroundColor(theme.shield_track),
            BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.3)),
            Visibility::Hidden,
            track_marker,
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(fill),
                fill_marker,
            ));
        });
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
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(96.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(8.0),
                align_items: AlignItems::Stretch,
                ..default()
            },
        ))
        .with_children(|row| {
            for idx in 0..4 {
                row.spawn((
                    Button,
                    Node {
                        width: Val::Percent(48.0),
                        min_height: Val::Px(64.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
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
                                width: Val::Px(28.0),
                                height: Val::Px(28.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            ImageNode::solid_color(Color::srgba(0.3, 0.45, 0.55, 0.9)),
                        ))
                        .with_children(|icon_box| {
                            icon_box.spawn((
                                Text::new("?"),
                                icon_font.clone(),
                                TextColor(Color::WHITE),
                                SkillButtonIconText { index: idx },
                            ));
                        });

                    button
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(2.0),
                                ..default()
                            },
                        ))
                        .with_children(|column| {
                            column.spawn((
                                Text::new(format!("技能 {}", idx + 1)),
                                body_font.clone(),
                                TextColor(Color::WHITE),
                                SkillButtonText { index: idx },
                            ));
                            column.spawn((
                                Text::new("类型：--"),
                                meta_font.clone(),
                                TextColor(Color::srgb(0.70, 0.82, 0.92)),
                                SkillButtonMetaText { index: idx },
                            ));
                        });
                });
            }
        });
}

fn spawn_skill_row_enemy(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    border_1: UiRect,
    radius_button: Val,
    body_font: TextFont,
    meta_font: TextFont,
    icon_font: TextFont,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(96.0),
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(8.0),
                align_items: AlignItems::Stretch,
                ..default()
            },
        ))
        .with_children(|row| {
            for idx in 0..4 {
                row.spawn((
                    Node {
                        width: Val::Percent(49.0),
                        min_height: Val::Px(64.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_button),
                        ..default()
                    },
                    BackgroundColor(theme.enemy_card_bg),
                    BorderColor::all(theme.enemy_card_border),
                    theme.button_shadow(),
                    SkillSlotId {
                        side: Side::Enemy,
                        index: idx,
                    },
                ))
                .with_children(|card| {
                    card.spawn((
                        Node {
                            width: Val::Px(28.0),
                            height: Val::Px(28.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        ImageNode::solid_color(Color::srgba(0.45, 0.28, 0.32, 0.92)),
                    ))
                    .with_children(|icon_box| {
                        icon_box.spawn((
                            Text::new("?"),
                            icon_font.clone(),
                            TextColor(Color::srgb(0.95, 0.85, 0.88)),
                            EnemySkillIconText { index: idx },
                        ));
                    });

                    card.spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            ..default()
                        },
                    ))
                    .with_children(|column| {
                        column.spawn((
                            Text::new(format!("技能 {}", idx + 1)),
                            body_font.clone(),
                            TextColor(Color::WHITE),
                            EnemySkillText { index: idx },
                        ));
                        column.spawn((
                            Text::new("类型：--"),
                            meta_font.clone(),
                            TextColor(Color::srgb(0.75, 0.78, 0.88)),
                            EnemySkillMetaText { index: idx },
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
) {
    let radius_panel = theme.radius_panel;
    let radius_button = theme.radius_button;
    let radius_hp = theme.radius_hp;
    let border_1 = UiRect::all(Val::Px(1.0));

    let title_font = super::helpers::make_text_font(21.0, ui_font.as_deref());
    let body_font = super::helpers::make_text_font(17.0, ui_font.as_deref());
    let meta_font = super::helpers::make_text_font(13.0, ui_font.as_deref());
    let result_font = super::helpers::make_text_font(38.0, ui_font.as_deref());
    let icon_font = super::helpers::make_text_font(14.0, ui_font.as_deref());

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
            // Top layer (Top Bar absolute to top)
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(4.0),
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
                    TextColor(Color::srgb(0.95, 0.98, 1.0)),
                    theme.title_text_shadow(),
                    BattlePhaseText,
                ));
                bar.spawn((
                    Text::new("操作提示：按 1-4 使用精灵技能，按 5/6/7 切换我方队伍1/2/3，按 Z/X/C/V/B 使用手牌，按 F 弃牌换 AP，按 E 结束回合，按 R 重新开始"),
                    meta_font.clone(),
                    TextColor(Color::srgb(0.80, 0.90, 0.95)),
                    TextShadow {
                        offset: Vec2::new(1.0, 1.0),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.45),
                    },
                    BattleHintText,
                ));
                bar.spawn((
                    Text::new("AP：Player 0 / Enemy 0"),
                    body_font.clone(),
                    TextColor(Color::srgb(0.95, 0.98, 1.0)),
                    theme.title_text_shadow(),
                    ActionPointsText,
                ));
                bar.spawn((
                    Text::new("行为：等待操作"),
                    meta_font.clone(),
                    TextColor(Color::srgb(0.90, 0.93, 0.98)),
                    TextShadow {
                        offset: Vec2::new(1.0, 1.0),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.45),
                    },
                    BattleActionText,
                ));
            });

            // Middle layer: Battle Arena
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(15.0), // Start below top bar
                    width: Val::Percent(100.0),
                    height: Val::Percent(60.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(40.0), Val::Px(20.0)),
                    ..default()
                },
            ))
            .with_children(|arena| {
                // Player side block
                arena.spawn((
                    Node {
                        width: Val::Percent(45.0),
                        max_width: Val::Px(450.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_panel),
                        ..default()
                    },
                    BackgroundColor(theme.panel),
                    BorderColor::all(theme.border_panel),
                    theme.panel_shadow(),
                ))
                .with_children(|player_zone| {
                    player_zone.spawn((
                        Text::new("我方"),
                        title_font.clone(),
                        TextColor(Color::srgb(0.75, 0.92, 1.0)),
                        theme.title_text_shadow(),
                    ));
                    player_zone.spawn((
                        Text::new("玩家：..."),
                        body_font.clone(),
                        TextColor(Color::WHITE),
                        theme.title_text_shadow(),
                        PlayerStatsText,
                    ));
                    spawn_hp_bar(
                        player_zone,
                        &theme,
                        border_1,
                        radius_hp,
                        theme.hp_fill_player,
                        PlayerHpBarFill,
                    );
                    spawn_shield_bar(
                        player_zone,
                        &theme,
                        border_1,
                        radius_hp,
                        theme.shield_fill_player,
                        PlayerShieldBarFill,
                        PlayerShieldBarTrack,
                    );
                    // 队伍切换按钮
                    player_zone
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                min_height: Val::Px(96.0),
                                max_height: Val::Px(124.0),
                                overflow: Overflow {
                                    x: OverflowAxis::Clip,
                                    y: OverflowAxis::Scroll,
                                },
                                padding: UiRect::top(Val::Px(6.0)),
                                ..default()
                            },
                        ))
                        .with_children(|scroll| {
                            scroll.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(8.0),
                                    row_gap: Val::Px(8.0),
                                    flex_wrap: FlexWrap::Wrap,
                                    ..default()
                                },
                            ))
                            .with_children(|row| {
                                for idx in 0..3 {
                                    row.spawn((
                                        Button,
                                        Node {
                                            width: Val::Percent(48.0),
                                            min_height: Val::Px(58.0),
                                            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                                            border: border_1,
                                            border_radius: BorderRadius::all(radius_button),
                                            flex_direction: FlexDirection::Column,
                                            row_gap: Val::Px(3.0),
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
                                            meta_font.clone(),
                                            TextColor(Color::WHITE),
                                            TeamMemberButtonText { index: idx },
                                        ));
                                        p.spawn((
                                            Text::new("附着: 无"),
                                            TextFont::from_font_size(12.0),
                                            TextColor(Color::srgb(0.78, 0.88, 0.98)),
                                            TeamMemberAuraText { index: idx },
                                        ));
                                        p.spawn((
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
                                                TeamMemberHpBarFill { index: idx },
                                            ));
                                        });
                                        p.spawn((
                                            Node {
                                                width: Val::Percent(100.0),
                                                height: Val::Px(6.0),
                                                overflow: Overflow::clip(),
                                                border_radius: BorderRadius::all(Val::Px(3.0)),
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
                                                    border_radius: BorderRadius::all(Val::Px(3.0)),
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

                // Enemy side block
                arena.spawn((
                    Node {
                        width: Val::Percent(45.0),
                        max_width: Val::Px(450.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_panel),
                        ..default()
                    },
                    BackgroundColor(theme.panel),
                    BorderColor::all(theme.border_panel),
                    theme.panel_shadow(),
                ))
                .with_children(|enemy_zone| {
                    enemy_zone.spawn((
                        Text::new("敌方"),
                        title_font.clone(),
                        TextColor(Color::srgb(1.0, 0.75, 0.78)),
                        theme.title_text_shadow(),
                    ));
                    enemy_zone.spawn((
                        Text::new("敌方：..."),
                        body_font.clone(),
                        TextColor(Color::WHITE),
                        theme.title_text_shadow(),
                        EnemyStatsText,
                    ));
                    spawn_hp_bar(
                        enemy_zone,
                        &theme,
                        border_1,
                        radius_hp,
                        theme.hp_fill_enemy,
                        EnemyHpBarFill,
                    );
                    spawn_shield_bar(
                        enemy_zone,
                        &theme,
                        border_1,
                        radius_hp,
                        theme.shield_fill_enemy,
                        EnemyShieldBarFill,
                        EnemyShieldBarTrack,
                    );
                    spawn_skill_row_enemy(
                        enemy_zone,
                        &theme,
                        border_1,
                        radius_button,
                        body_font.clone(),
                        meta_font.clone(),
                        icon_font.clone(),
                    );
                    enemy_zone
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                min_height: Val::Px(96.0),
                                max_height: Val::Px(124.0),
                                overflow: Overflow {
                                    x: OverflowAxis::Clip,
                                    y: OverflowAxis::Scroll,
                                },
                                padding: UiRect::top(Val::Px(6.0)),
                                ..default()
                            },
                        ))
                        .with_children(|scroll| {
                            scroll.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(8.0),
                                    row_gap: Val::Px(8.0),
                                    flex_wrap: FlexWrap::Wrap,
                                    ..default()
                                },
                            ))
                            .with_children(|row| {
                                for idx in 0..3 {
                                    row.spawn((
                                        Node {
                                            width: Val::Percent(48.0),
                                            min_height: Val::Px(58.0),
                                            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                                            border: border_1,
                                            border_radius: BorderRadius::all(radius_button),
                                            flex_direction: FlexDirection::Column,
                                            row_gap: Val::Px(3.0),
                                            ..default()
                                        },
                                        BackgroundColor(theme.enemy_card_bg),
                                        BorderColor::all(theme.enemy_card_border),
                                        theme.button_shadow(),
                                        EnemyTeamMemberButton { index: idx },
                                    ))
                                    .with_children(|p| {
                                        p.spawn((
                                            Text::new(format!("队伍{}", idx + 1)),
                                            meta_font.clone(),
                                            TextColor(Color::WHITE),
                                            EnemyTeamMemberButtonText { index: idx },
                                        ));
                                        p.spawn((
                                            Text::new("附着: 无"),
                                            TextFont::from_font_size(12.0),
                                            TextColor(Color::srgb(0.90, 0.84, 0.88)),
                                            EnemyTeamMemberAuraText { index: idx },
                                        ));
                                        p.spawn((
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
                                                EnemyTeamMemberHpBarFill { index: idx },
                                            ));
                                        });
                                        p.spawn((
                                            Node {
                                                width: Val::Percent(100.0),
                                                height: Val::Px(6.0),
                                                overflow: Overflow::clip(),
                                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                                ..default()
                                            },
                                            BackgroundColor(theme.shield_track),
                                            Visibility::Hidden,
                                            EnemyTeamMemberShieldBarTrack { index: idx },
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
                                                EnemyTeamMemberShieldBarFill { index: idx },
                                            ));
                                        });
                                    });
                                }
                            });
                        });
                });
            });

            // Bottom layer: Hand Cards
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::FlexEnd,
                    padding: UiRect::bottom(Val::Px(24.0)),
                    column_gap: Val::Px(12.0),
                    ..default()
                },
            ))
            .with_children(|hand_container| {
                for idx in 0..5 {
                    hand_container.spawn((
                        Button,
                        Node {
                            width: Val::Px(130.0),
                            height: Val::Px(180.0),
                            padding: UiRect::all(Val::Px(8.0)),
                            border: border_1,
                            border_radius: BorderRadius::all(radius_button),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            ..default()
                        },
                        BackgroundColor(theme.button_idle),
                        BorderColor::all(theme.button_border_idle),
                        theme.button_shadow(),
                        PlayerCardButton { index: idx },
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Text::new(super::helpers::card_hotkey_label(idx)),
                            icon_font.clone(),
                            TextColor(Color::srgb(0.90, 0.95, 1.0)),
                            PlayerCardHotkeyText { index: idx },
                        ));
                        card.spawn((
                            Text::new("—"),
                            body_font.clone(),
                            TextColor(Color::WHITE),
                            PlayerCardNameText { index: idx },
                        ));
                        card.spawn((
                            Text::new("AP—"),
                            meta_font.clone(),
                            TextColor(Color::srgb(0.78, 0.88, 0.98)),
                            PlayerCardCostText { index: idx },
                        ));
                        card.spawn((
                            Text::new(""),
                            meta_font.clone(),
                            TextColor(Color::srgb(0.86, 0.92, 0.98)),
                            Visibility::Hidden,
                            PlayerCardDescText { index: idx },
                        ));
                    });
                }
            });

            // Left-Bottom Layer: Control panel
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    bottom: Val::Px(20.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    ..default()
                },
            ))
            .with_children(|controls| {
                controls.spawn((
                    Button,
                    Node {
                        width: Val::Px(160.0),
                        min_height: Val::Px(48.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
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
                        TextColor(Color::WHITE),
                    ));
                });

                controls.spawn((
                    Button,
                    Node {
                        width: Val::Px(160.0),
                        min_height: Val::Px(48.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
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
                        TextColor(Color::WHITE),
                    ));
                });
            });

            // Right-Bottom Layer: Skill panel
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(20.0),
                    bottom: Val::Px(20.0),
                    width: Val::Px(360.0),
                    ..default()
                },
            ))
            .with_children(|skills| {
                spawn_skill_row_player(
                    skills,
                    &theme,
                    border_1,
                    radius_button,
                    body_font.clone(),
                    meta_font.clone(),
                    icon_font.clone(),
                );
            });

            // Central absolute Results Text
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Percent(45.0),
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

