use bevy::prelude::*;

use super::{components::*, resources::UiFontHandle, theme::UiTheme};

use crate::battle::Side;

const BATTLE_BACKGROUND_IMAGE: &str = "images/icons/background/bg1.png";
const BATTLE_BACKGROUND_SIZE: Vec2 = Vec2::new(1920.0, 1080.0);
const ACTIVE_STAT_ROW_TOP_OFFSET: f32 = -16.0;
const ACTIVE_INFO_MASK_TOP: f32 = 40.0; //距离屏幕顶部的距离
const ACTIVE_INFO_MASK_SIDE: f32 = 14.0; //距离屏幕边缘的距离
const ACTIVE_INFO_MASK_WIDTH: f32 = 318.0; //蒙版宽度
const ACTIVE_INFO_MASK_HEIGHT: f32 = 232.0; //蒙版高度

// 待机位精灵小面板：去掉黑方框，参照主精灵面板做的紧凑、敌我对称版本。
const BENCH_CARD_WIDTH: f32 = 168.0; //单个待机面板宽度
const BENCH_BAR_WIDTH: f32 = 92.0; //血量/护盾条宽度
// 半透明背景代替原本不透明的“黑方框”，敌我两侧使用同一底色保持对称。
pub(crate) const BENCH_BG: Color = Color::srgba(0.05, 0.07, 0.11, 0.42);
pub(crate) const BENCH_BG_HOVER: Color = Color::srgba(0.20, 0.32, 0.48, 0.55);
pub(crate) const BENCH_BG_PRESSED: Color = Color::srgba(0.16, 0.26, 0.40, 0.66);

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
    fill_justify: JustifyContent,
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
                justify_content: fill_justify,
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
                display: Display::None,
                width: Val::Px(0.0),
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
                    width: Val::Percent(100.0),
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

fn outlined_stat_text_shadow() -> TextShadow {
    TextShadow {
        offset: Vec2::new(1.0, 1.0),
        color: Color::WHITE,
    }
}

fn spawn_stat_value(
    parent: &mut ChildSpawnerCommands,
    _theme: &UiTheme,
    font: TextFont,
    label: &'static str,
    value_marker: impl Component,
    stage_markers: Option<(StatStageModifierBadge, StatStageModifierText)>,
) {
    parent
        .spawn((Node {
            width: Val::Px(44.0),
            height: Val::Px(84.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(2.0),
            ..default()
        },))
        .with_children(|root| {
            root.spawn((
                Node {
                    min_width: Val::Px(34.0),
                    min_height: Val::Px(18.0),
                    padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.95, 0.68, 0.10)),
            ))
            .with_children(|badge| {
                badge.spawn((Text::new(label), font.clone(), TextColor(Color::WHITE)));
            });
            root.spawn((
                Node {
                    min_width: Val::Px(34.0),
                    min_height: Val::Px(18.0),
                    padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.95, 0.68, 0.10)),
            ))
            .with_children(|badge| {
                badge.spawn((
                    Text::new("0"),
                    font.clone(),
                    TextColor(Color::WHITE),
                    value_marker,
                ));
            });

            let stage_slot = Node {
                min_width: Val::Px(34.0),
                min_height: Val::Px(18.0),
                padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            };
            if let Some((badge_marker, text_marker)) = stage_markers {
                root.spawn((stage_slot, BackgroundColor(Color::NONE), badge_marker))
                    .with_children(|badge| {
                        badge.spawn((Text::new(""), font, TextColor(Color::WHITE), text_marker));
                    });
            } else {
                root.spawn((stage_slot, BackgroundColor(Color::NONE)));
            }
        });
}

fn spawn_small_bench_card(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    _border_1: UiRect,
    radius_button: Val,
    title_font: TextFont,
    meta_font: TextFont,
    _hotkey_font: TextFont,
    index: usize,
    side: Side,
) {
    // 待机位面板：去掉黑方框与阴影，仅保留半透明底，参照主精灵面板展示关键信息
    // （名称+附着、血量条、护盾条、状态）。敌我两侧镜像对齐以保持对称。
    match side {
        Side::Player => {
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(BENCH_CARD_WIDTH),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                        border_radius: BorderRadius::all(radius_button),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexStart,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                    BackgroundColor(BENCH_BG),
                    PlayerBenchCard { index },
                ))
                .with_children(|card| {
                    // 名称（附着信息以紧凑文字附在名称后）
                    card.spawn((
                        Text::new("队伍"),
                        title_font.clone(),
                        TextColor(theme.accent_player),
                        theme.title_text_shadow(),
                        PlayerBenchNameText { index },
                    ));
                    // 血量条 + 数值
                    card.spawn((Node {
                        width: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },))
                        .with_children(|row| {
                            row.spawn((
                                Node {
                                    width: Val::Px(BENCH_BAR_WIDTH),
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
                            row.spawn((
                                Text::new("0/0"),
                                meta_font.clone(),
                                TextColor(Color::BLACK),
                                outlined_stat_text_shadow(),
                                PlayerBenchHpValueText { index },
                            ));
                        });
                    // 护盾条 + 数值（无护盾时条隐藏）
                    card.spawn((Node {
                        width: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },))
                        .with_children(|row| {
                            row.spawn((
                                Node {
                                    display: Display::None,
                                    width: Val::Px(0.0),
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
                                ));
                            });
                            row.spawn((
                                Text::new("0"),
                                meta_font.clone(),
                                TextColor(Color::BLACK),
                                outlined_stat_text_shadow(),
                                PlayerBenchShieldValueText { index },
                            ));
                        });
                    // 状态（图标，与主面板一致；无状态时不显示）
                    spawn_colored_debug_tokens(
                        card,
                        theme,
                        meta_font.clone(),
                        meta_font.clone(),
                        "",
                        Val::Percent(100.0),
                        FlexWrap::Wrap,
                        FlexDirection::Row,
                        Val::Px(4.0),
                        PlayerBenchStatusLine { index },
                        DebugStatusToken,
                        &[],
                    );
                });
        }
        Side::Enemy => {
            parent
                .spawn((
                    Node {
                        width: Val::Px(BENCH_CARD_WIDTH),
                        margin: UiRect::left(Val::Auto),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                        border_radius: BorderRadius::all(radius_button),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexEnd,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                    BackgroundColor(BENCH_BG),
                    EnemyBenchCard { index },
                ))
                .with_children(|card| {
                    // 名称（右对齐，镜像玩家面板）
                    card.spawn((
                        Text::new("队伍"),
                        title_font.clone(),
                        TextColor(theme.accent_enemy),
                        theme.title_text_shadow(),
                        EnemyBenchNameText { index },
                    ));
                    // 血量条 + 数值（数值在左、条在右，镜像玩家）
                    card.spawn((Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::FlexEnd,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },))
                        .with_children(|row| {
                            row.spawn((
                                Text::new("0/0"),
                                meta_font.clone(),
                                TextColor(Color::BLACK),
                                outlined_stat_text_shadow(),
                                EnemyBenchHpValueText { index },
                            ));
                            row.spawn((
                                Node {
                                    width: Val::Px(BENCH_BAR_WIDTH),
                                    height: Val::Px(8.0),
                                    overflow: Overflow::clip(),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                    justify_content: JustifyContent::FlexEnd,
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
                        });
                    // 护盾条 + 数值（镜像）
                    card.spawn((Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::FlexEnd,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },))
                        .with_children(|row| {
                            row.spawn((
                                Text::new("0"),
                                meta_font.clone(),
                                TextColor(Color::BLACK),
                                outlined_stat_text_shadow(),
                                EnemyBenchShieldValueText { index },
                            ));
                            row.spawn((
                                Node {
                                    display: Display::None,
                                    width: Val::Px(0.0),
                                    height: Val::Px(6.0),
                                    overflow: Overflow::clip(),
                                    border_radius: BorderRadius::all(Val::Px(3.0)),
                                    justify_content: JustifyContent::FlexEnd,
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
                                ));
                            });
                        });
                    // 状态（图标，右对齐镜像；无状态时不显示）
                    spawn_colored_debug_tokens(
                        card,
                        theme,
                        meta_font.clone(),
                        meta_font.clone(),
                        "",
                        Val::Percent(100.0),
                        FlexWrap::Wrap,
                        FlexDirection::RowReverse,
                        Val::Px(4.0),
                        EnemyBenchStatusLine { index },
                        DebugStatusToken,
                        &[],
                    );
                });
        }
    }
}

fn spawn_colored_debug_tokens(
    parent: &mut ChildSpawnerCommands,
    _theme: &UiTheme,
    label_font: TextFont,
    value_font: TextFont,
    label: &str,
    line_width: Val,
    line_wrap: FlexWrap,
    line_direction: FlexDirection,
    column_gap: Val,
    line_marker: impl Component,
    token_marker: impl Component + Clone,
    initial_tokens: &[(&str, Color)],
) {
    parent
        .spawn((
            Node {
                width: line_width,
                flex_direction: line_direction,
                flex_wrap: line_wrap,
                column_gap,
                row_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                ..default()
            },
            line_marker,
        ))
        .with_children(|line| {
            line.spawn((
                Text::new(label),
                label_font,
                TextColor(Color::BLACK),
                outlined_stat_text_shadow(),
            ));
            for (text, color) in initial_tokens {
                line.spawn((
                    Text::new(*text),
                    value_font.clone(),
                    TextColor(*color),
                    token_marker.clone(),
                ));
            }
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
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Stretch,
            ..default()
        },))
        .with_children(|row| {
            for idx in 0..4 {
                row.spawn((
                    Button,
                    Node {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        min_width: Val::Px(0.0),
                        height: Val::Px(136.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                        align_items: AlignItems::FlexStart,
                        column_gap: Val::Px(6.0),
                        overflow: Overflow::clip(),
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
    asset_server: Res<AssetServer>,
    ui_font: Option<Res<UiFontHandle>>,
    retreat_confirm: Option<ResMut<super::systems::RetreatConfirmState>>,
    hint_overlay_state: Option<ResMut<super::systems::BattleHintOverlayState>>,
    existing_ui: Query<(), (With<BattleUiRoot>, Without<BattleUiCleanupPending>)>,
) {
    if !existing_ui.is_empty() {
        return;
    }
    if let Some(mut retreat_confirm) = retreat_confirm {
        retreat_confirm.armed = false;
    }
    if let Some(mut hint_overlay_state) = hint_overlay_state {
        hint_overlay_state.open = false;
    }
    let radius_panel = theme.radius_panel;
    let radius_button = theme.radius_button;
    let radius_hp = theme.radius_hp;
    let radius_card = theme.radius_card;
    let border_1 = UiRect::all(Val::Px(1.0));

    let title_font = super::helpers::make_text_font(20.0, ui_font.as_deref());
    let body_font = super::helpers::make_text_font(16.0, ui_font.as_deref());
    let meta_font = super::helpers::make_text_font(17.0, ui_font.as_deref());
    let result_font = super::helpers::make_text_font(40.0, ui_font.as_deref());
    let icon_font = super::helpers::make_text_font(14.0, ui_font.as_deref());
    let small_font = super::helpers::make_text_font(11.0, ui_font.as_deref());
    let skill_header_font = super::helpers::make_text_font(17.0, ui_font.as_deref());
    let skill_body_font = super::helpers::make_text_font(14.0, ui_font.as_deref());
    let skill_meta_font = super::helpers::make_text_font(12.0, ui_font.as_deref());
    let skill_icon_font = super::helpers::make_text_font(12.0, ui_font.as_deref());

    commands.spawn((
        Sprite {
            image: asset_server.load(BATTLE_BACKGROUND_IMAGE),
            custom_size: Some(BATTLE_BACKGROUND_SIZE),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -100.0),
        BattleBackgroundSprite,
    ));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::NONE),
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
            });

            // === Player Info: Top-Left ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(ACTIVE_INFO_MASK_TOP),
                    left: Val::Px(ACTIVE_INFO_MASK_SIDE),
                    width: Val::Px(ACTIVE_INFO_MASK_WIDTH),
                    height: Val::Px(ACTIVE_INFO_MASK_HEIGHT),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.12, 0.05, 0.08, 0.76)), //玩家信息蒙版颜色
            ));

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(52.0),
                    left: Val::Px(20.0),
                    width: Val::Px(400.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                PlayerInfoPanel,
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(34.0),
                        padding: UiRect::axes(Val::Px(2.0), Val::Px(2.0)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|name_box| {
                    name_box.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            ..default()
                        },
                        PlayerNameAuraRow,
                    ))
                    .with_children(|row| {
                        row.spawn((Node {
                            width: Val::Px(30.0),
                            height: Val::Px(30.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },))
                        .with_children(|attr_box| {
                            attr_box.spawn((
                                Node {
                                    width: Val::Px(28.0),
                                    height: Val::Px(28.0),
                                    overflow: Overflow::clip(),
                                    border_radius: BorderRadius::all(Val::Px(14.0)),
                                    ..default()
                                },
                                ImageNode::new(asset_server.load("images/icons/elements/water.png")),
                                PlayerElementIcon,
                            ));
                        });
                        row.spawn((
                            Text::new("..."),
                            title_font.clone(),
                            TextColor(theme.accent_player),
                            theme.title_text_shadow(),
                            PlayerNameText,
                        ));
                    });
                });
            });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(92.0),
                    left: Val::Px(20.0),
                    width: Val::Px(400.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(0.0), Val::Px(6.0)),
                    ..default()
                },
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    row.spawn((Node { width: Val::Px(220.0), ..default() },))
                        .with_children(|bar_wrap| {
                            spawn_hp_bar(
                                bar_wrap,
                                &theme,
                                border_1,
                                radius_hp,
                                JustifyContent::FlexStart,
                                theme.hp_fill_player,
                                PlayerHpBarFill,
                            );
                        });
                    row.spawn((
                        Text::new("0/0"),
                        meta_font.clone(),
                        TextColor(Color::BLACK),
                        outlined_stat_text_shadow(),
                        PlayerHpValueText,
                    ));
                    spawn_colored_debug_tokens(
                        row,
                        &theme,
                        meta_font.clone(),
                        meta_font.clone(),
                        "",
                        Val::Px(96.0),
                        FlexWrap::Wrap,
                        FlexDirection::Row,
                        Val::Px(4.0),
                        PlayerAuraLine,
                        DebugAuraToken,
                        &[],
                    );
                });
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    spawn_shield_bar(
                        row,
                        &theme,
                        border_1,
                        radius_hp,
                        theme.shield_fill_player,
                        PlayerShieldBarFill,
                        PlayerShieldBarTrack,
                    );
                    row.spawn((
                        Text::new("0"),
                        meta_font.clone(),
                        TextColor(Color::BLACK),
                        outlined_stat_text_shadow(),
                        PlayerShieldValueText,
                    ));
                });
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(24.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(4.0),
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    },
                ))
                .with_children(|row| {
                    spawn_colored_debug_tokens(
                        row,
                        &theme,
                        meta_font.clone(),
                        meta_font.clone(),
                        "",
                        Val::Px(300.0),
                        FlexWrap::Wrap,
                        FlexDirection::Row,
                        Val::Px(4.0),
                        PlayerStatusLine,
                        DebugStatusToken,
                        &[],
                    );
                });
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(30.0),
                        top: Val::Px(ACTIVE_STAT_ROW_TOP_OFFSET),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexStart,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        row_gap: Val::Px(4.0),
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    },
                ))
                .with_children(|row| {
                    spawn_stat_value(
                        row,
                        &theme,
                        meta_font.clone(),
                        "生命",
                        PlayerHpStatText,
                        None,
                    );
                    spawn_stat_value(
                        row,
                        &theme,
                        meta_font.clone(),
                        "攻击",
                        PlayerAtkText,
                        Some((
                            StatStageModifierBadge { side: Side::Player, stat: StatStageModifierKind::Atk },
                            StatStageModifierText { side: Side::Player, stat: StatStageModifierKind::Atk },
                        )),
                    );
                    spawn_stat_value(
                        row,
                        &theme,
                        meta_font.clone(),
                        "防御",
                        PlayerDefText,
                        Some((
                            StatStageModifierBadge { side: Side::Player, stat: StatStageModifierKind::Def },
                            StatStageModifierText { side: Side::Player, stat: StatStageModifierKind::Def },
                        )),
                    );
                    spawn_stat_value(
                        row,
                        &theme,
                        meta_font.clone(),
                        "命中",
                        PlayerAccText,
                        Some((
                            StatStageModifierBadge { side: Side::Player, stat: StatStageModifierKind::Acc },
                            StatStageModifierText { side: Side::Player, stat: StatStageModifierKind::Acc },
                        )),
                    );
                    spawn_stat_value(
                        row,
                        &theme,
                        meta_font.clone(),
                        "速度",
                        PlayerSpdText,
                        Some((
                            StatStageModifierBadge { side: Side::Player, stat: StatStageModifierKind::Spd },
                            StatStageModifierText { side: Side::Player, stat: StatStageModifierKind::Spd },
                        )),
                    );
                });
                panel
                    .spawn((Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    },))
                    .with_children(|column| {
                        for idx in 0..3 {
                            spawn_small_bench_card(
                                column,
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
            });


            // === Enemy Info: Top-Right (mirrors player info) ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(ACTIVE_INFO_MASK_TOP),
                    right: Val::Px(ACTIVE_INFO_MASK_SIDE),
                    width: Val::Px(ACTIVE_INFO_MASK_WIDTH),
                    height: Val::Px(ACTIVE_INFO_MASK_HEIGHT),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.05, 0.08, 0.76)), //敌方信息蒙版颜色
            ));

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(52.0),
                    right: Val::Px(20.0),
                    width: Val::Px(400.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                EnemyInfoPanel,
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(34.0),
                        padding: UiRect::axes(Val::Px(2.0), Val::Px(2.0)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|name_box| {
                    name_box.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::FlexEnd,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            ..default()
                        },
                        EnemyNameAuraRow,
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new("..."),
                            title_font.clone(),
                            TextColor(theme.accent_enemy),
                            theme.title_text_shadow(),
                            EnemyNameText,
                        ));
                        row.spawn((Node {
                            width: Val::Px(30.0),
                            height: Val::Px(30.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },))
                        .with_children(|attr_box| {
                            attr_box.spawn((
                                Node {
                                    width: Val::Px(28.0),
                                    height: Val::Px(28.0),
                                    overflow: Overflow::clip(),
                                    border_radius: BorderRadius::all(Val::Px(14.0)),
                                    ..default()
                                },
                                ImageNode::new(asset_server.load("images/icons/elements/fire.png")),
                                EnemyElementIcon,
                            ));
                        });
                    });
                });
            });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(92.0),
                    right: Val::Px(20.0),
                    width: Val::Px(400.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(0.0), Val::Px(6.0)),
                    ..default()
                },
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    spawn_colored_debug_tokens(
                        row,
                        &theme,
                        meta_font.clone(),
                        meta_font.clone(),
                        "",
                        Val::Px(96.0),
                        FlexWrap::Wrap,
                        FlexDirection::RowReverse,
                        Val::Px(4.0),
                        EnemyAuraLine,
                        DebugAuraToken,
                        &[],
                    );
                    row.spawn((
                        Text::new("0/0"),
                        meta_font.clone(),
                        TextColor(Color::BLACK),
                        outlined_stat_text_shadow(),
                        EnemyHpValueText,
                    ));
                    row.spawn((Node { width: Val::Px(220.0), ..default() },))
                        .with_children(|bar_wrap| {
                            spawn_hp_bar(
                                bar_wrap,
                                &theme,
                                border_1,
                                radius_hp,
                                JustifyContent::FlexEnd,
                                theme.hp_fill_enemy,
                                EnemyHpBarFill,
                            );
                        });
                });
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new("0"),
                        meta_font.clone(),
                        TextColor(Color::BLACK),
                        outlined_stat_text_shadow(),
                        EnemyShieldValueText,
                    ));
                    spawn_shield_bar(
                        row,
                        &theme,
                        border_1,
                        radius_hp,
                        theme.shield_fill_enemy,
                        EnemyShieldBarFill,
                        EnemyShieldBarTrack,
                    );
                });
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(24.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(4.0),
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    },
                ))
                .with_children(|row| {
                    spawn_colored_debug_tokens(
                        row,
                        &theme,
                        meta_font.clone(),
                        meta_font.clone(),
                        "",
                        Val::Px(300.0),
                        FlexWrap::Wrap,
                        FlexDirection::RowReverse,
                        Val::Px(4.0),
                        EnemyStatusLine,
                        DebugStatusToken,
                        &[],
                    );
                });
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(30.0),
                        top: Val::Px(ACTIVE_STAT_ROW_TOP_OFFSET),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        row_gap: Val::Px(4.0),
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    },
                ))
                .with_children(|row| {
                    spawn_stat_value(
                        row,
                        &theme,
                        meta_font.clone(),
                        "生命",
                        EnemyHpStatText,
                        None,
                    );
                    spawn_stat_value(
                        row,
                        &theme,
                        meta_font.clone(),
                        "攻击",
                        EnemyAtkText,
                        Some((
                            StatStageModifierBadge { side: Side::Enemy, stat: StatStageModifierKind::Atk },
                            StatStageModifierText { side: Side::Enemy, stat: StatStageModifierKind::Atk },
                        )),
                    );
                    spawn_stat_value(
                        row,
                        &theme,
                        meta_font.clone(),
                        "防御",
                        EnemyDefText,
                        Some((
                            StatStageModifierBadge { side: Side::Enemy, stat: StatStageModifierKind::Def },
                            StatStageModifierText { side: Side::Enemy, stat: StatStageModifierKind::Def },
                        )),
                    );
                    spawn_stat_value(
                        row,
                        &theme,
                        meta_font.clone(),
                        "命中",
                        EnemyAccText,
                        Some((
                            StatStageModifierBadge { side: Side::Enemy, stat: StatStageModifierKind::Acc },
                            StatStageModifierText { side: Side::Enemy, stat: StatStageModifierKind::Acc },
                        )),
                    );
                    spawn_stat_value(
                        row,
                        &theme,
                        meta_font.clone(),
                        "速度",
                        EnemySpdText,
                        Some((
                            StatStageModifierBadge { side: Side::Enemy, stat: StatStageModifierKind::Spd },
                            StatStageModifierText { side: Side::Enemy, stat: StatStageModifierKind::Spd },
                        )),
                    );
                });
                panel
                    .spawn((Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    },))
                    .with_children(|column| {
                        for idx in 0..3 {
                            spawn_small_bench_card(
                                column,
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
            });


            // === Hint Button: Center ===
            root.spawn((
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(52.0),
                    left: Val::Percent(45.0),
                    width: Val::Percent(10.0),
                    min_height: Val::Px(30.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: border_1,
                    border_radius: BorderRadius::all(radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                BattleHintButton,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("操作提示"),
                    meta_font.clone(),
                    TextColor(theme.text_primary),
                    TextShadow {
                        offset: Vec2::new(1.0, 1.0),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.35),
                    },
                ));
            });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(92.0),
                    left: Val::Percent(28.0),
                    width: Val::Percent(44.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    border: border_1,
                    border_radius: BorderRadius::all(radius_panel),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.94)),
                BorderColor::all(theme.border_panel),
                theme.panel_shadow(),
                Visibility::Hidden,
                ZIndex(75),
                BattleHintOverlayRoot,
            ))
            .with_children(|hint_panel| {
                hint_panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(10.0),
                            ..default()
                        },
                    ))
                    .with_children(|header| {
                        header.spawn((
                            Text::new("操作提示"),
                            title_font.clone(),
                            TextColor(theme.text_primary),
                            theme.title_text_shadow(),
                        ));
                        header
                            .spawn((
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                                    border: border_1,
                                    border_radius: BorderRadius::all(radius_button),
                                    ..default()
                                },
                                BackgroundColor(theme.button_idle),
                                BorderColor::all(theme.button_border_idle),
                                theme.button_shadow(),
                                BattleHintCloseButton,
                            ))
                            .with_children(|button| {
                                button.spawn((
                                    Text::new("关闭"),
                                    small_font.clone(),
                                    TextColor(theme.text_primary),
                                ));
                            });
                    });
                hint_panel.spawn((
                    Text::new("按 1-4 使用精灵技能\n按 Q 打开/关闭换人面板\n按 5/6/7 切换我方队伍 1/2/3\n按 Z/X/C/V/B 使用手牌\n按 F 弃牌换 AP\n按 E 结束回合\n按 R 重新开始"),
                    meta_font.clone(),
                    TextColor(theme.text_muted),
                    TextShadow {
                        offset: Vec2::new(1.0, 1.0),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.35),
                    },
                    BattleHintText,
                ));
            });

            // === Center Turn Banner ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Percent(28.0),
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(14.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|banner| {
                banner.spawn((
                    Text::new(""),
                    super::helpers::make_text_font(46.0, ui_font.as_deref()),
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.88)),
                    TextShadow {
                        offset: Vec2::new(2.0, 2.0),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.60),
                    },
                    TurnBannerText,
                ));
                banner.spawn((
                    Text::new(""),
                    super::helpers::make_text_font(24.0, ui_font.as_deref()),
                    TextColor(theme.text_primary),
                    TextShadow {
                        offset: Vec2::new(2.0, 2.0),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.65),
                    },
                    Visibility::Hidden,
                    BattleActionText { remaining: 0.0 },
                ));
            });

            // === Right Action Dial: circular visual + transparent click quadrants ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(20.0),
                    bottom: Val::Px(24.0),
                    width: Val::Px(164.0),
                    height: Val::Px(164.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(82.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.panel_shadow(),
                ZIndex(60),
            ))
            .with_children(|dial| {
                dial.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(81.0),
                        top: Val::Px(0.0),
                        width: Val::Px(2.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(theme.button_border_idle),
                ));
                dial.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(81.0),
                        width: Val::Percent(100.0),
                        height: Val::Px(2.0),
                        ..default()
                    },
                    BackgroundColor(theme.button_border_idle),
                ));

                dial.spawn((
                    Button,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(82.0),
                        height: Val::Px(82.0),
                        border: UiRect::all(Val::Px(3.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                    ActionDialButton,
                    DiscardButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            width: Val::Px(164.0),
                            height: Val::Px(164.0),
                            border_radius: BorderRadius::all(Val::Px(82.0)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        ActionDialHighlight,
                    ));
                    btn.spawn((
                        Text::new("弃牌\nF"),
                        body_font.clone(),
                        TextColor(theme.text_primary),
                    ));
                });

                dial.spawn((
                    Button,
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(82.0),
                        height: Val::Px(82.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                    ActionDialButton,
                    EndTurnButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(0.0),
                            top: Val::Px(0.0),
                            width: Val::Px(164.0),
                            height: Val::Px(164.0),
                            border_radius: BorderRadius::all(Val::Px(82.0)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        ActionDialHighlight,
                    ));
                    btn.spawn((
                        Text::new("结束\nE"),
                        body_font.clone(),
                        TextColor(theme.text_primary),
                    ));
                });

                dial.spawn((
                    Button,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(82.0),
                        height: Val::Px(82.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                    ActionDialButton,
                    RetreatButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            width: Val::Px(164.0),
                            height: Val::Px(164.0),
                            border_radius: BorderRadius::all(Val::Px(82.0)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        ActionDialHighlight,
                    ));
                    btn.spawn((
                        Text::new("撤退"),
                        body_font.clone(),
                        TextColor(theme.text_primary),
                        RetreatButtonText,
                    ));
                });

                dial.spawn((
                    Button,
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(82.0),
                        height: Val::Px(82.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                    ActionDialButton,
                    SwitchMonsterButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            width: Val::Px(164.0),
                            height: Val::Px(164.0),
                            border_radius: BorderRadius::all(Val::Px(82.0)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        ActionDialHighlight,
                    ));
                    btn.spawn((
                        Text::new("换精灵\nQ"),
                        body_font.clone(),
                        TextColor(theme.accent_player),
                    ));
                });
            });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(184.0),
                    right: Val::Px(184.0),
                    bottom: Val::Px(430.0),
                    height: Val::Px(54.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    display: Display::None,
                    ..default()
                },
                HandFullHintRoot,
                ZIndex(80),
            ))
            .with_children(|hint| {
                hint.spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(26.0), Val::Px(10.0)),
                        border: border_1,
                        border_radius: BorderRadius::all(Val::Px(14.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.05, 0.10, 0.86)),
                    BorderColor::all(Color::srgba(1.0, 0.24, 0.24, 0.78)),
                    theme.panel_shadow(),
                ))
                .with_children(|box_root| {
                    box_root.spawn((
                        Text::new("手牌大于4张，请弃牌至4张"),
                        body_font.clone(),
                        TextColor(Color::NONE),
                        TextShadow {
                            offset: Vec2::ZERO,
                            color: Color::NONE,
                        },
                        HandFullHintText,
                    ));
                });
            });

            // === Bottom Hand Row (above) ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(184.0),
                    right: Val::Px(16.0),
                    bottom: Val::Px(92.0),
                    height: Val::Px(330.0),
                    ..default()
                },
                HandCardsRoot,
                ZIndex(10),
            ))
            .with_children(|hand_container| {
                for idx in 0..18 {
                    hand_container
                        .spawn((
                            Button,
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Px(130.0),
                                height: Val::Px(180.0),
                                padding: UiRect::px(10.0, 10.0, 12.0, 10.0),
                                border: border_1,
                                border_radius: BorderRadius::all(radius_card),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                ..default()
                            },
                            BackgroundColor(theme.card_bg),
                            BorderColor::all(theme.card_border),
                            theme.card_shadow(),
                            ZIndex(10),
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
                                PlayerCardHotkeyBadge { index: idx },
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

            // === Bottom Skill Region (front/overlay) ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(184.0),
                    right: Val::Px(200.0),
                    bottom: Val::Px(16.0),
                    min_height: Val::Px(196.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Stretch,
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                SkillPanelRoot,
                ZIndex(20),
            ))
            .with_children(|skill_region| {
                skill_region
                    .spawn((
                        Node {
                            flex_grow: 1.0,
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
                        skill_section
                            .spawn((
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
