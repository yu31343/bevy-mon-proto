use bevy::prelude::*;

use super::{components::*, resources::UiFontHandle, theme::UiTheme};

use crate::battle::Side;

const BATTLE_BACKGROUND_IMAGE: &str = "images/icons/background/bg1.png";
const BATTLE_BACKGROUND_SIZE: Vec2 = Vec2::new(1920.0, 1080.0);
const ACTIVE_INFO_MASK_TOP: f32 = 40.0; //距离屏幕顶部的距离
const ACTIVE_INFO_MASK_SIDE: f32 = 14.0; //距离屏幕边缘的距离
const ACTIVE_INFO_MASK_WIDTH: f32 = 318.0; //信息卡宽度
// 上场精灵大头像与压在其角上的元素图标尺寸。
const PORTRAIT_SIZE: f32 = 105.0; //上场大头像边长
const PORTRAIT_BADGE_SIZE: f32 = 28.0; //头像角上的元素图标边长
const BENCH_PORTRAIT_SIZE: f32 = 80.0; //待机位小头像边长（明显小于上场）

// 待机位精灵小面板：去掉黑方框，参照主精灵面板做的紧凑、敌我对称版本。
const BENCH_CARD_WIDTH: f32 = 220.0; //单个待机面板宽度（含左/右侧小头像）
const BENCH_BAR_WIDTH: f32 = 92.0; //血量/护盾条宽度
// 半透明背景代替原本不透明的“黑方框”，敌我两侧使用同一底色保持对称。
pub(crate) const BENCH_BG: Color = Color::srgba(0.26, 0.18, 0.085, 0.62);
pub(crate) const BENCH_BG_HOVER: Color = Color::srgba(0.38, 0.28, 0.13, 0.68);
pub(crate) const BENCH_BG_PRESSED: Color = Color::srgba(0.22, 0.15, 0.07, 0.74);
/// 阵亡待机位的暗化底色，与存活位的暖底拉开明显反差。
pub(crate) const BENCH_BG_DEAD: Color = Color::srgba(0.08, 0.075, 0.075, 0.66);
/// 阵亡待机位/换人项的冷灰描边，替代金色描边以传达“不可用”。
pub(crate) const DEAD_MEMBER_BORDER: Color = Color::srgba(0.34, 0.32, 0.32, 0.70);

/// AP 宝石 pip 的最大数量（与 BattleRules.max_ap 对齐；多余的 AP 仍由数字显示）。
const AP_GEM_COUNT: usize = 12;

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

fn bar_value_text_color() -> TextColor {
    TextColor(Color::srgb(0.96, 0.98, 1.0))
}

fn bar_value_text_shadow() -> TextShadow {
    TextShadow {
        offset: Vec2::new(1.0, 1.0),
        color: Color::srgba(0.0, 0.0, 0.0, 0.75),
    }
}

fn spawn_stat_chip_compact<ValueMarker, StageBadgeMarker, StageTextMarker>(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    font: TextFont,
    label: &'static str,
    value_marker: ValueMarker,
    stage_markers: Option<(StageBadgeMarker, StageTextMarker)>,
) where
    ValueMarker: Component,
    StageBadgeMarker: Component,
    StageTextMarker: Component,
{
    // 紧凑横向属性项：描金标签 + 羊皮纸数值牌 + 升降箭头位。
    parent
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(3.0),
            ..default()
        },))
        .with_children(|chip| {
            chip.spawn((
                Text::new(label),
                font.clone(),
                TextColor(theme.gold_bright),
                theme.title_text_shadow(),
            ));
            chip.spawn((
                Node {
                    min_width: Val::Px(24.0),
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                theme.parchment_panel(),
                BorderColor::all(theme.gold_dim),
            ))
            .with_children(|pill| {
                pill.spawn((
                    Text::new("0"),
                    font.clone(),
                    TextColor(theme.ink_primary),
                    value_marker,
                ));
            });
            if let Some((badge_marker, text_marker)) = stage_markers {
                chip.spawn((
                    Node {
                        min_width: Val::Px(14.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    badge_marker,
                ))
                .with_children(|badge| {
                    badge.spawn((Text::new(""), font, TextColor(Color::WHITE), text_marker));
                });
            }
        });
}

fn spawn_small_bench_card(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    asset_server: &AssetServer,
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
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(BENCH_BG),
                    PlayerBenchCard { index },
                ))
                .with_children(|bench| {
                    // 小头像（左）
                    bench.spawn((
                        Node {
                            width: Val::Px(BENCH_PORTRAIT_SIZE),
                            height: Val::Px(BENCH_PORTRAIT_SIZE),
                            flex_shrink: 0.0,
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        ImageNode::new(asset_server.load("images/icons/profile/ui/fire.png")),
                        BorderColor::all(theme.gold_dim),
                        PlayerBenchPortrait { index },
                    ));
                    // 内容列：名称 / 血量 / 护盾 / 状态
                    bench
                        .spawn((Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::FlexStart,
                            row_gap: Val::Px(3.0),
                            ..default()
                        },))
                        .with_children(|card| {
                            // 名称 + 附着图标
                            card.spawn((Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(4.0),
                                ..default()
                            },))
                                .with_children(|row| {
                                    row.spawn((
                                        Text::new("队伍"),
                                        title_font.clone(),
                                        TextColor(theme.accent_player),
                                        theme.title_text_shadow(),
                                        PlayerBenchNameText { index },
                                    ));
                                    spawn_colored_debug_tokens(
                                        row,
                                        theme,
                                        meta_font.clone(),
                                        meta_font.clone(),
                                        "",
                                        Val::Auto,
                                        FlexWrap::NoWrap,
                                        FlexDirection::Row,
                                        Val::Px(3.0),
                                        PlayerBenchAuraLine { index },
                                        DebugAuraToken,
                                        &[],
                                    );
                                });
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
                                        bar_value_text_color(),
                                        bar_value_text_shadow(),
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
                                        bar_value_text_color(),
                                        bar_value_text_shadow(),
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
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(BENCH_BG),
                    EnemyBenchCard { index },
                ))
                .with_children(|bench| {
                    // 内容列：名称 / 血量 / 护盾 / 状态（右对齐，镜像玩家）
                    bench
                        .spawn((Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::FlexEnd,
                            row_gap: Val::Px(3.0),
                            ..default()
                        },))
                        .with_children(|card| {
                            // 名称 + 附着图标（右对齐镜像）
                            card.spawn((Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::FlexEnd,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(4.0),
                                ..default()
                            },))
                                .with_children(|row| {
                                    spawn_colored_debug_tokens(
                                        row,
                                        theme,
                                        meta_font.clone(),
                                        meta_font.clone(),
                                        "",
                                        Val::Auto,
                                        FlexWrap::NoWrap,
                                        FlexDirection::RowReverse,
                                        Val::Px(3.0),
                                        EnemyBenchAuraLine { index },
                                        DebugAuraToken,
                                        &[],
                                    );
                                    row.spawn((
                                        Text::new("队伍"),
                                        title_font.clone(),
                                        TextColor(theme.accent_enemy),
                                        theme.title_text_shadow(),
                                        EnemyBenchNameText { index },
                                    ));
                                });
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
                                        bar_value_text_color(),
                                        bar_value_text_shadow(),
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
                                        bar_value_text_color(),
                                        bar_value_text_shadow(),
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
                    // 小头像（右）：头像水平翻转，镜像我方
                    bench.spawn((
                        Node {
                            width: Val::Px(BENCH_PORTRAIT_SIZE),
                            height: Val::Px(BENCH_PORTRAIT_SIZE),
                            flex_shrink: 0.0,
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        ImageNode {
                            flip_x: true,
                            ..ImageNode::new(asset_server.load("images/icons/profile/ui/fire.png"))
                        },
                        BorderColor::all(theme.gold_dim),
                        EnemyBenchPortrait { index },
                    ));
                });
        }
    }
}

fn spawn_switch_candidate_card(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    asset_server: &AssetServer,
    border_1: UiRect,
    radius_hp: Val,
    meta_font: TextFont,
    title_font: TextFont,
    index: usize,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(ACTIVE_INFO_MASK_WIDTH),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            theme.lacquer_panel_bg(),
            BackgroundColor(Color::NONE),
            BorderColor::all(theme.gold),
            theme.gold_frame_shadow(),
            TeamMemberButton { index },
        ))
        .with_children(|card| {
            card.spawn((Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },))
                .with_children(|header| {
                    header
                        .spawn((
                            Node {
                                width: Val::Px(PORTRAIT_SIZE),
                                height: Val::Px(PORTRAIT_SIZE),
                                flex_shrink: 0.0,
                                position_type: PositionType::Relative,
                                border: UiRect::all(Val::Px(2.0)),
                                border_radius: BorderRadius::all(Val::Px(12.0)),
                                ..default()
                            },
                            ImageNode::new(asset_server.load("images/icons/profile/ui/water.png")),
                            BorderColor::all(theme.gold),
                            TeamMemberPortrait { index },
                        ))
                        .with_children(|portrait| {
                            portrait.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(3.0),
                                    bottom: Val::Px(3.0),
                                    width: Val::Px(PORTRAIT_BADGE_SIZE),
                                    height: Val::Px(PORTRAIT_BADGE_SIZE),
                                    border: UiRect::all(Val::Px(1.5)),
                                    border_radius: BorderRadius::all(Val::Px(
                                        PORTRAIT_BADGE_SIZE / 2.0,
                                    )),
                                    ..default()
                                },
                                ImageNode::new(
                                    asset_server.load("images/icons/elements/water.png"),
                                ),
                                BorderColor::all(theme.gold_bright),
                                TeamMemberElementIcon { index },
                            ));
                        });

                    header
                        .spawn((Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Stretch,
                            row_gap: Val::Px(6.0),
                            ..default()
                        },))
                        .with_children(|col| {
                            col.spawn((Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(6.0),
                                ..default()
                            },))
                                .with_children(|row| {
                                    row.spawn((
                                        Text::new("..."),
                                        title_font.clone(),
                                        TextColor(theme.accent_player),
                                        theme.title_text_shadow(),
                                        TeamMemberNameText { index },
                                    ));
                                });

                            col.spawn((Node {
                                width: Val::Percent(100.0),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(8.0),
                                ..default()
                            },))
                                .with_children(|row| {
                                    row.spawn((Node {
                                        flex_grow: 1.0,
                                        ..default()
                                    },))
                                        .with_children(|bar_wrap| {
                                            spawn_hp_bar(
                                                bar_wrap,
                                                theme,
                                                border_1,
                                                radius_hp,
                                                JustifyContent::FlexStart,
                                                theme.hp_fill_player,
                                                TeamMemberHpBarFill { index },
                                            );
                                        });
                                    row.spawn((
                                        Text::new("0/0"),
                                        meta_font.clone(),
                                        bar_value_text_color(),
                                        bar_value_text_shadow(),
                                        TeamMemberHpValueText { index },
                                    ));
                                    spawn_colored_debug_tokens(
                                        row,
                                        theme,
                                        meta_font.clone(),
                                        meta_font.clone(),
                                        "",
                                        Val::Auto,
                                        FlexWrap::Wrap,
                                        FlexDirection::Row,
                                        Val::Px(4.0),
                                        TeamMemberAuraLine { index },
                                        DebugAuraToken,
                                        &[],
                                    );
                                });

                            col.spawn((Node {
                                width: Val::Percent(100.0),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(8.0),
                                ..default()
                            },))
                                .with_children(|row| {
                                    spawn_shield_bar(
                                        row,
                                        theme,
                                        border_1,
                                        radius_hp,
                                        theme.shield_fill_player,
                                        TeamMemberShieldBarFill { index },
                                        TeamMemberShieldBarTrack { index },
                                    );
                                    row.spawn((
                                        Text::new("0"),
                                        meta_font.clone(),
                                        bar_value_text_color(),
                                        bar_value_text_shadow(),
                                        TeamMemberShieldValueText { index },
                                    ));
                                });
                        });
                });

            card.spawn((Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(4.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },))
                .with_children(|row| {
                    spawn_colored_debug_tokens(
                        row,
                        theme,
                        meta_font.clone(),
                        meta_font.clone(),
                        "",
                        Val::Percent(100.0),
                        FlexWrap::Wrap,
                        FlexDirection::Row,
                        Val::Px(4.0),
                        TeamMemberStatusLine { index },
                        DebugStatusToken,
                        &[],
                    );
                });

            card.spawn((Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(4.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },))
                .with_children(|row| {
                    spawn_stat_chip_compact(
                        row,
                        theme,
                        meta_font.clone(),
                        "攻",
                        TeamMemberAtkText { index },
                        Some((
                            TeamMemberStatStageModifierBadge {
                                index,
                                stat: StatStageModifierKind::Atk,
                            },
                            TeamMemberStatStageModifierText {
                                index,
                                stat: StatStageModifierKind::Atk,
                            },
                        )),
                    );
                    spawn_stat_chip_compact(
                        row,
                        theme,
                        meta_font.clone(),
                        "防",
                        TeamMemberDefText { index },
                        Some((
                            TeamMemberStatStageModifierBadge {
                                index,
                                stat: StatStageModifierKind::Def,
                            },
                            TeamMemberStatStageModifierText {
                                index,
                                stat: StatStageModifierKind::Def,
                            },
                        )),
                    );
                    spawn_stat_chip_compact(
                        row,
                        theme,
                        meta_font.clone(),
                        "速",
                        TeamMemberSpdText { index },
                        Some((
                            TeamMemberStatStageModifierBadge {
                                index,
                                stat: StatStageModifierKind::Spd,
                            },
                            TeamMemberStatStageModifierText {
                                index,
                                stat: StatStageModifierKind::Spd,
                            },
                        )),
                    );
                    spawn_stat_chip_compact(
                        row,
                        theme,
                        meta_font,
                        "命",
                        TeamMemberAccText { index },
                        Some((
                            TeamMemberStatStageModifierBadge {
                                index,
                                stat: StatStageModifierKind::Acc,
                            },
                            TeamMemberStatStageModifierText {
                                index,
                                stat: StatStageModifierKind::Acc,
                            },
                        )),
                    );
                });
        });
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
    // 2×2 招式格：元素色类型片 + 名称/类型 + AP 费用宝石，靠 flex 填满技能区。
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        },))
        .with_children(|grid| {
            for row_idx in 0..2 {
                grid.spawn((Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Stretch,
                    ..default()
                },))
                    .with_children(|row| {
                        for col in 0..2 {
                            let idx = row_idx * 2 + col;
                            row.spawn((
                                Button,
                                Node {
                                    flex_grow: 1.0,
                                    flex_basis: Val::Px(0.0),
                                    min_width: Val::Px(0.0),
                                    min_height: Val::Px(70.0),
                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(8.0),
                                    overflow: Overflow::clip(),
                                    border: border_1,
                                    border_radius: BorderRadius::all(radius_button),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.34, 0.245, 0.115, 1.0)),
                                BorderColor::all(theme.gold_dim),
                                theme.button_shadow(),
                                SkillButton { index: idx },
                                SkillSlotId {
                                    side: Side::Player,
                                    index: idx,
                                },
                            ))
                            .with_children(|button| {
                                // 元素类型色片（含槽位号），颜色由 update_skill_text_system 动态着色
                                button
                                    .spawn((
                                        Node {
                                            width: Val::Px(32.0),
                                            height: Val::Px(32.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(1.0)),
                                            border_radius: BorderRadius::all(Val::Px(8.0)),
                                            ..default()
                                        },
                                        BackgroundColor(theme.gold_dim),
                                        BorderColor::all(theme.gold_bright),
                                        SkillTileTypeChip { index: idx },
                                    ))
                                    .with_children(|chip| {
                                        chip.spawn((
                                            Text::new("?"),
                                            icon_font.clone(),
                                            TextColor(Color::WHITE),
                                            TextShadow {
                                                offset: Vec2::new(1.0, 1.0),
                                                color: Color::srgba(0.0, 0.0, 0.0, 0.55),
                                            },
                                            SkillButtonIconText { index: idx },
                                        ));
                                    });

                                // 名称 + 类型/摘要
                                button
                                    .spawn((Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(2.0),
                                        flex_grow: 1.0,
                                        min_width: Val::Px(0.0),
                                        overflow: Overflow::clip(),
                                        ..default()
                                    },))
                                    .with_children(|column| {
                                        column.spawn((
                                            Text::new(format!("技能 {}", idx + 1)),
                                            body_font.clone(),
                                            TextColor(theme.text_primary),
                                            theme.title_text_shadow(),
                                            SkillButtonText { index: idx },
                                        ));
                                        column.spawn((
                                            Text::new("类型：--"),
                                            meta_font.clone(),
                                            TextColor(theme.text_secondary),
                                            SkillButtonMetaText { index: idx },
                                        ));
                                    });

                                // AP 费用宝石
                                button
                                    .spawn((
                                        Node {
                                            width: Val::Px(30.0),
                                            height: Val::Px(30.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(1.0)),
                                            border_radius: BorderRadius::all(Val::Px(15.0)),
                                            ..default()
                                        },
                                        theme.gold_gradient(),
                                        BorderColor::all(theme.gold_bright),
                                    ))
                                    .with_children(|gem| {
                                        gem.spawn((
                                            Text::new("—"),
                                            icon_font.clone(),
                                            TextColor(theme.ink_primary),
                                            SkillButtonCostText { index: idx },
                                        ));
                                    });
                            });
                        }
                    });
            }
        });
}

fn spawn_ap_gem_group(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    label_font: TextFont,
    count_font: TextFont,
    side: Side,
) {
    let (label, accent) = match side {
        Side::Player => ("我方", theme.accent_player),
        Side::Enemy => ("敌方", theme.accent_enemy),
    };
    parent
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        },))
        .with_children(|group| {
            group.spawn((
                Text::new(label),
                label_font,
                TextColor(accent),
                theme.title_text_shadow(),
            ));
            group
                .spawn((Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(3.0),
                    ..default()
                },))
                .with_children(|pips| {
                    for index in 0..AP_GEM_COUNT {
                        pips.spawn((
                            Node {
                                width: Val::Px(11.0),
                                height: Val::Px(11.0),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(6.0)),
                                ..default()
                            },
                            BackgroundColor(theme.ap_gem_empty),
                            BorderColor::all(theme.gold_dim),
                            ApGemPip { side, index },
                        ));
                    }
                });
            group.spawn((
                Text::new("0"),
                count_font,
                TextColor(theme.gold_bright),
                theme.title_text_shadow(),
                ApGemCountText { side },
            ));
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
    let card_desc_font = super::helpers::make_text_font(13.5, ui_font.as_deref());
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
            // === Background vignette / scrim (顶部与底部压暗，提升文字对比与仪式感) ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundGradient::from(LinearGradient::to_bottom(vec![
                    ColorStop::percent(Color::srgba(0.02, 0.02, 0.04, 0.58), 0.0),
                    ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.0), 24.0),
                    ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.0), 64.0),
                    ColorStop::percent(Color::srgba(0.02, 0.02, 0.04, 0.66), 100.0),
                ])),
                ZIndex(0),
                BattleVignette,
            ));

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
                theme.lacquer_panel_bg(),
                BorderColor::all(theme.gold),
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
                spawn_ap_gem_group(
                    bar,
                    &theme,
                    body_font.clone(),
                    meta_font.clone(),
                    Side::Player,
                );
                spawn_ap_gem_group(
                    bar,
                    &theme,
                    body_font.clone(),
                    meta_font.clone(),
                    Side::Enemy,
                );
            });

            // === Player Info: Top-Left (单一描金信息卡) ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(ACTIVE_INFO_MASK_TOP),
                    left: Val::Px(ACTIVE_INFO_MASK_SIDE),
                    width: Val::Px(ACTIVE_INFO_MASK_WIDTH),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                theme.info_panel_mask_bg(),
                BorderColor::all(theme.gold),
                theme.gold_frame_shadow(),
                ActivePortraitFrame { side: Side::Player },
                PlayerInfoPanel,
            ))
            .with_children(|card| {
                // 头部：大头像（左）+ 右列（名称 / 血量 / 护盾）
                card.spawn((Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    ..default()
                },))
                    .with_children(|header| {
                        // 大头像 + 元素图标（压在左下角，适当遮挡头像）
                        header
                            .spawn((
                                Node {
                                    width: Val::Px(PORTRAIT_SIZE),
                                    height: Val::Px(PORTRAIT_SIZE),
                                    flex_shrink: 0.0,
                                    position_type: PositionType::Relative,
                                    border: UiRect::all(Val::Px(2.0)),
                                    border_radius: BorderRadius::all(Val::Px(12.0)),
                                    ..default()
                                },
                                ImageNode::new(asset_server.load("images/icons/profile/ui/water.png")),
                                BorderColor::all(theme.gold),
                                PlayerPortraitImage,
                            ))
                            .with_children(|portrait| {
                                portrait.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(3.0),
                                        bottom: Val::Px(3.0),
                                        width: Val::Px(PORTRAIT_BADGE_SIZE),
                                        height: Val::Px(PORTRAIT_BADGE_SIZE),
                                        border: UiRect::all(Val::Px(1.5)),
                                        border_radius: BorderRadius::all(Val::Px(
                                            PORTRAIT_BADGE_SIZE / 2.0,
                                        )),
                                        ..default()
                                    },
                                    ImageNode::new(
                                        asset_server.load("images/icons/elements/water.png"),
                                    ),
                                    BorderColor::all(theme.gold_bright),
                                    PlayerElementIcon,
                                ));
                            });
                        // 右列：名称 / 血量 / 护盾
                        header
                            .spawn((Node {
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Stretch,
                                row_gap: Val::Px(6.0),
                                ..default()
                            },))
                            .with_children(|col| {
                                // 名称行
                                col.spawn((
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
                                    row.spawn((
                                        Text::new("..."),
                                        title_font.clone(),
                                        TextColor(theme.accent_player),
                                        theme.title_text_shadow(),
                                        PlayerNameText,
                                    ));
                                });
                                // 血量行：血条 + 数值 + 附着（紧跟血量，无标签）
                                col.spawn((Node {
                                    width: Val::Percent(100.0),
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(8.0),
                                    ..default()
                                },))
                                    .with_children(|row| {
                                        row.spawn((Node {
                                            flex_grow: 1.0,
                                            ..default()
                                        },))
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
                                            bar_value_text_color(),
                                            bar_value_text_shadow(),
                                            PlayerHpValueText,
                                        ));
                                        spawn_colored_debug_tokens(
                                            row,
                                            &theme,
                                            meta_font.clone(),
                                            meta_font.clone(),
                                            "",
                                            Val::Auto,
                                            FlexWrap::Wrap,
                                            FlexDirection::Row,
                                            Val::Px(4.0),
                                            PlayerAuraLine,
                                            DebugAuraToken,
                                            &[],
                                        );
                                    });
                                // 护盾行
                                col.spawn((Node {
                                    width: Val::Percent(100.0),
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(8.0),
                                    ..default()
                                },))
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
                                            bar_value_text_color(),
                                            bar_value_text_shadow(),
                                            PlayerShieldValueText,
                                        ));
                                    });
                            });
                    });
                // 状态行
                card.spawn((Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(22.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },))
                    .with_children(|row| {
                        spawn_colored_debug_tokens(
                            row,
                            &theme,
                            meta_font.clone(),
                            meta_font.clone(),
                            "",
                            Val::Percent(100.0),
                            FlexWrap::Wrap,
                            FlexDirection::Row,
                            Val::Px(4.0),
                            PlayerStatusLine,
                            DebugStatusToken,
                            &[],
                        );
                    });
                // 紧凑属性条：攻/防/速/命
                card.spawn((Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },))
                    .with_children(|row| {
                        spawn_stat_chip_compact(
                            row,
                            &theme,
                            meta_font.clone(),
                            "攻",
                            PlayerAtkText,
                            Some((
                                StatStageModifierBadge { side: Side::Player, stat: StatStageModifierKind::Atk },
                                StatStageModifierText { side: Side::Player, stat: StatStageModifierKind::Atk },
                            )),
                        );
                        spawn_stat_chip_compact(
                            row,
                            &theme,
                            meta_font.clone(),
                            "防",
                            PlayerDefText,
                            Some((
                                StatStageModifierBadge { side: Side::Player, stat: StatStageModifierKind::Def },
                                StatStageModifierText { side: Side::Player, stat: StatStageModifierKind::Def },
                            )),
                        );
                        spawn_stat_chip_compact(
                            row,
                            &theme,
                            meta_font.clone(),
                            "速",
                            PlayerSpdText,
                            Some((
                                StatStageModifierBadge { side: Side::Player, stat: StatStageModifierKind::Spd },
                                StatStageModifierText { side: Side::Player, stat: StatStageModifierKind::Spd },
                            )),
                        );
                        spawn_stat_chip_compact(
                            row,
                            &theme,
                            meta_font.clone(),
                            "命",
                            PlayerAccText,
                            Some((
                                StatStageModifierBadge { side: Side::Player, stat: StatStageModifierKind::Acc },
                                StatStageModifierText { side: Side::Player, stat: StatStageModifierKind::Acc },
                            )),
                        );
                    });
                // 待机位预览
                card.spawn((Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(5.0),
                    ..default()
                },))
                    .with_children(|column| {
                        for idx in 0..3 {
                            spawn_small_bench_card(
                                column,
                                &theme,
                                &asset_server,
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


            // === Enemy Info: Top-Right (单一描金信息卡，镜像我方) ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(ACTIVE_INFO_MASK_TOP),
                    right: Val::Px(ACTIVE_INFO_MASK_SIDE),
                    width: Val::Px(ACTIVE_INFO_MASK_WIDTH),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                theme.info_panel_mask_bg(),
                BorderColor::all(theme.gold),
                theme.gold_frame_shadow(),
                ActivePortraitFrame { side: Side::Enemy },
                EnemyInfoPanel,
            ))
            .with_children(|card| {
                // 头部：右列（名称 / 血量 / 护盾）+ 大头像（右），镜像我方
                card.spawn((Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    ..default()
                },))
                    .with_children(|header| {
                        // 右列：名称 / 血量 / 护盾（右对齐）
                        header
                            .spawn((Node {
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Stretch,
                                row_gap: Val::Px(6.0),
                                ..default()
                            },))
                            .with_children(|col| {
                                // 名称行（右对齐）
                                col.spawn((
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
                                });
                                // 血量行：附着 + 数值 + 血条（镜像我方）
                                col.spawn((Node {
                                    width: Val::Percent(100.0),
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(8.0),
                                    ..default()
                                },))
                                    .with_children(|row| {
                                        spawn_colored_debug_tokens(
                                            row,
                                            &theme,
                                            meta_font.clone(),
                                            meta_font.clone(),
                                            "",
                                            Val::Auto,
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
                                            bar_value_text_color(),
                                            bar_value_text_shadow(),
                                            EnemyHpValueText,
                                        ));
                                        row.spawn((Node {
                                            flex_grow: 1.0,
                                            ..default()
                                        },))
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
                                // 护盾行（镜像）
                                col.spawn((Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::FlexEnd,
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(8.0),
                                    ..default()
                                },))
                                    .with_children(|row| {
                                        row.spawn((
                                            Text::new("0"),
                                            meta_font.clone(),
                                            bar_value_text_color(),
                                            bar_value_text_shadow(),
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
                            });
                        // 大头像（右）+ 元素图标（压在右下角），头像水平翻转
                        header
                            .spawn((
                                Node {
                                    width: Val::Px(PORTRAIT_SIZE),
                                    height: Val::Px(PORTRAIT_SIZE),
                                    flex_shrink: 0.0,
                                    position_type: PositionType::Relative,
                                    border: UiRect::all(Val::Px(2.0)),
                                    border_radius: BorderRadius::all(Val::Px(12.0)),
                                    ..default()
                                },
                                ImageNode {
                                    flip_x: true,
                                    ..ImageNode::new(
                                        asset_server.load("images/icons/profile/ui/fire.png"),
                                    )
                                },
                                BorderColor::all(theme.gold),
                                EnemyPortraitImage,
                            ))
                            .with_children(|portrait| {
                                portrait.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        right: Val::Px(3.0),
                                        bottom: Val::Px(3.0),
                                        width: Val::Px(PORTRAIT_BADGE_SIZE),
                                        height: Val::Px(PORTRAIT_BADGE_SIZE),
                                        border: UiRect::all(Val::Px(1.5)),
                                        border_radius: BorderRadius::all(Val::Px(
                                            PORTRAIT_BADGE_SIZE / 2.0,
                                        )),
                                        ..default()
                                    },
                                    ImageNode::new(
                                        asset_server.load("images/icons/elements/fire.png"),
                                    ),
                                    BorderColor::all(theme.gold_bright),
                                    EnemyElementIcon,
                                ));
                            });
                    });
                // 状态行
                card.spawn((Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(22.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },))
                    .with_children(|row| {
                        spawn_colored_debug_tokens(
                            row,
                            &theme,
                            meta_font.clone(),
                            meta_font.clone(),
                            "",
                            Val::Percent(100.0),
                            FlexWrap::Wrap,
                            FlexDirection::RowReverse,
                            Val::Px(4.0),
                            EnemyStatusLine,
                            DebugStatusToken,
                            &[],
                        );
                    });
                // 紧凑属性条
                card.spawn((Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },))
                    .with_children(|row| {
                        spawn_stat_chip_compact(
                            row,
                            &theme,
                            meta_font.clone(),
                            "攻",
                            EnemyAtkText,
                            Some((
                                StatStageModifierBadge { side: Side::Enemy, stat: StatStageModifierKind::Atk },
                                StatStageModifierText { side: Side::Enemy, stat: StatStageModifierKind::Atk },
                            )),
                        );
                        spawn_stat_chip_compact(
                            row,
                            &theme,
                            meta_font.clone(),
                            "防",
                            EnemyDefText,
                            Some((
                                StatStageModifierBadge { side: Side::Enemy, stat: StatStageModifierKind::Def },
                                StatStageModifierText { side: Side::Enemy, stat: StatStageModifierKind::Def },
                            )),
                        );
                        spawn_stat_chip_compact(
                            row,
                            &theme,
                            meta_font.clone(),
                            "速",
                            EnemySpdText,
                            Some((
                                StatStageModifierBadge { side: Side::Enemy, stat: StatStageModifierKind::Spd },
                                StatStageModifierText { side: Side::Enemy, stat: StatStageModifierKind::Spd },
                            )),
                        );
                        spawn_stat_chip_compact(
                            row,
                            &theme,
                            meta_font.clone(),
                            "命",
                            EnemyAccText,
                            Some((
                                StatStageModifierBadge { side: Side::Enemy, stat: StatStageModifierKind::Acc },
                                StatStageModifierText { side: Side::Enemy, stat: StatStageModifierKind::Acc },
                            )),
                        );
                    });
                // 待机位预览
                card.spawn((Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(5.0),
                    ..default()
                },))
                    .with_children(|column| {
                        for idx in 0..3 {
                            spawn_small_bench_card(
                                column,
                                &theme,
                                &asset_server,
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
                theme.lacquer_panel_bg(),
                BorderColor::all(theme.gold),
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
                                BackgroundColor(Color::srgba(0.34, 0.245, 0.115, 1.0)),
                                BorderColor::all(theme.gold_dim),
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
                theme.lacquer_panel_bg(),
                BorderColor::all(theme.gold),
                theme.gold_frame_shadow(),
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
                    BackgroundColor(theme.gold_divider),
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
                    BackgroundColor(theme.gold_divider),
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
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    bottom: Val::Px(120.0),
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
                                padding: UiRect::px(8.0, 8.0, 8.0, 8.0),
                                border: UiRect::all(Val::Px(2.0)),
                                border_radius: BorderRadius::all(radius_card),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(5.0),
                                ..default()
                            },
                            BackgroundColor(theme.card_bg),
                            BorderColor::all(theme.card_border),
                            theme.card_shadow(),
                            ZIndex(10),
                            PlayerCardButton { index: idx },
                        ))
                        .with_children(|card| {
                            // 发光环：可出牌时由手牌系统点亮（独立覆盖层，不争用按钮 bg/border）。
                            // 加粗外环并外扩，使“可出/不可出”描边差异更醒目。
                            card.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: Val::Px(-4.0),
                                    left: Val::Px(-4.0),
                                    right: Val::Px(-4.0),
                                    bottom: Val::Px(-4.0),
                                    border: UiRect::all(Val::Px(3.0)),
                                    border_radius: BorderRadius::all(Val::Px(18.0)),
                                    ..default()
                                },
                                BorderColor::all(Color::NONE),
                                CardGlow { index: idx },
                            ));
                            // 顶行：费用宝石 + 类别色带 + 快捷键
                            card.spawn((Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(4.0),
                                ..default()
                            },))
                                .with_children(|top| {
                                    top.spawn((
                                        Node {
                                            width: Val::Px(26.0),
                                            height: Val::Px(26.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(1.0)),
                                            border_radius: BorderRadius::all(Val::Px(13.0)),
                                            ..default()
                                        },
                                        theme.gold_gradient(),
                                        BorderColor::all(theme.gold_bright),
                                    ))
                                    .with_children(|gem| {
                                        gem.spawn((
                                            Text::new("—"),
                                            body_font.clone(),
                                            TextColor(theme.ink_primary),
                                            PlayerCardCostText { index: idx },
                                        ));
                                    });
                                    top.spawn((
                                        Node {
                                            flex_grow: 1.0,
                                            min_width: Val::Px(0.0),
                                            height: Val::Px(20.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border_radius: BorderRadius::all(Val::Px(6.0)),
                                            overflow: Overflow::clip(),
                                            ..default()
                                        },
                                        BackgroundColor(theme.card_cat_resource),
                                        CardCategoryBand { index: idx },
                                    ))
                                    .with_children(|band| {
                                        band.spawn((
                                            Text::new(""),
                                            small_font.clone(),
                                            TextColor(Color::WHITE),
                                            TextShadow {
                                                offset: Vec2::new(1.0, 1.0),
                                                color: Color::srgba(0.0, 0.0, 0.0, 0.50),
                                            },
                                            CardCategoryLabel { index: idx },
                                        ));
                                    });
                                    top.spawn((
                                        Node {
                                            width: Val::Px(22.0),
                                            height: Val::Px(22.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border_radius: BorderRadius::all(Val::Px(5.0)),
                                            ..default()
                                        },
                                        ImageNode::solid_color(Color::srgba(0.14, 0.10, 0.06, 0.92)),
                                        PlayerCardHotkeyBadge { index: idx },
                                    ))
                                    .with_children(|hotkey_box| {
                                        hotkey_box.spawn((
                                            Text::new(super::helpers::card_hotkey_label(idx)),
                                            icon_font.clone(),
                                            TextColor(theme.gold_bright),
                                            PlayerCardHotkeyText { index: idx },
                                        ));
                                    });
                                });
                            card.spawn((
                                Text::new("—"),
                                body_font.clone(),
                                TextColor(theme.ink_primary),
                                PlayerCardNameText { index: idx },
                            ));
                            card.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(1.0),
                                    ..default()
                                },
                                BackgroundColor(theme.gold_divider),
                            ));
                            card.spawn((
                                Text::new(""),
                                card_desc_font.clone(),
                                TextColor(theme.ink_secondary),
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
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(radius_panel),
                            ..default()
                        },
                        theme.lacquer_panel_bg(),
                        BorderColor::all(theme.gold),
                        theme.gold_frame_shadow(),
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
                                    TextColor(theme.gold_bright),
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
                ZIndex(1000),
                SwitchOverlayRoot,
            ))
            .with_children(|overlay| {
                overlay.spawn((
                    Node {
                        width: Val::Px(1080.0),
                        padding: UiRect::all(Val::Px(20.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(16.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(radius_panel),
                        ..default()
                    },
                    theme.lacquer_panel_bg(),
                    BorderColor::all(theme.gold),
                    theme.gold_frame_shadow(),
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
                            column_gap: Val::Px(16.0),
                            row_gap: Val::Px(16.0),
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Stretch,
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        for idx in 0..3 {
                            spawn_switch_candidate_card(
                                row,
                                &theme,
                                &asset_server,
                                border_1,
                                radius_hp,
                                meta_font.clone(),
                                title_font.clone(),
                                idx,
                            );
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
