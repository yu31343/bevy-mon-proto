//! 战斗 UI：上敌方 / 下玩家，血条与护盾条，技能格占位图与特效。
mod fx;

use bevy::prelude::*;
use std::fs;

use crate::{
    battle::{
        ActionPoints, BattleResult, Combatant, EnemyTeam, Hand, InBattle, PlayerTeam, Shield, Side,
        SkillList, Stats, Team, TurnContext, BattleEvent,
    },
    data::{CardDb, CardId, ElementType, SkillDb, SkillEffect, SkillId},
    game_state::{BattlePhase, GameState},
};

use fx::{process_battle_fx_events, tick_fx_lifetimes, tick_screen_flashes, tick_skill_flash_timer, SkillFlashTimer};

/// 根画布（飘字与闪屏的父节点）。
#[derive(Component)]
pub(crate) struct BattleUiRoot;

/// 技能格所属阵营与槽位（玩家按钮与敌方卡共用）。
#[derive(Component, Clone, Copy)]
pub(crate) struct SkillSlotId {
    pub side: Side,
    pub index: usize,
}

#[derive(Component)]
struct PlayerStatsText;
#[derive(Component)]
struct EnemyStatsText;
#[derive(Component)]
struct ResultText;
#[derive(Component)]
struct BattlePhaseText;

#[derive(Component)]
struct ActionPointsText;

#[derive(Component)]
struct PlayerHandCardText {
    index: usize,
}

#[derive(Component)]
struct DiscardButton;

#[derive(Component)]
struct EndTurnButton;

#[derive(Component)]
struct SkillButton {
    index: usize,
}
#[derive(Component)]
struct SkillButtonText {
    index: usize,
}
#[derive(Component)]
struct SkillButtonMetaText {
    index: usize,
}
#[derive(Component)]
struct SkillButtonIconText {
    index: usize,
}

#[derive(Component)]
struct EnemySkillText {
    index: usize,
}
#[derive(Component)]
struct EnemySkillMetaText {
    index: usize,
}
#[derive(Component)]
struct EnemySkillIconText {
    index: usize,
}

#[derive(Component)]
struct PlayerHpBarFill;
#[derive(Component)]
struct EnemyHpBarFill;

#[derive(Component)]
struct PlayerShieldBarTrack;
#[derive(Component)]
struct EnemyShieldBarTrack;

#[derive(Component)]
struct PlayerShieldBarFill;
#[derive(Component)]
struct EnemyShieldBarFill;

pub struct UiPlugin;

#[derive(Resource, Clone)]
pub(crate) struct UiFontHandle(Handle<Font>);

// #region agent log
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

fn agent_debug_log(
    hypothesis_id: &str,
    location: &str,
    message: &str,
    run_id: &str,
) {
    // Debug logs are best-effort; if writing fails, ignore to avoid hiding the original panic.
    let _ = (|| {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("debug-648aa6.log");
        let mut f = OpenOptions::new().create(true).append(true).open(path)?;
        let line = format!(
            "{{\"sessionId\":\"648aa6\",\"runId\":\"{}\",\"hypothesisId\":\"{}\",\"location\":\"{}\",\"message\":\"{}\",\"data\":{{}},\"timestamp\":{}}}\n",
            run_id, hypothesis_id, location, message, ts
        );
        f.write_all(line.as_bytes())?;
        f.flush()?;
        Ok::<(), std::io::Error>(())
    })();
}
// #endregion

#[derive(Resource, Clone)]
struct UiTheme {
    bg_top: Color,
    bg_bottom: Color,
    top_bar_start: Color,
    top_bar_end: Color,
    panel: Color,
    button_idle: Color,
    button_hover: Color,
    button_pressed: Color,
    enemy_card_bg: Color,
    enemy_card_border: Color,
    hp_track: Color,
    hp_fill_player: Color,
    hp_fill_enemy: Color,
    shield_track: Color,
    shield_fill_player: Color,
    shield_fill_enemy: Color,
    border_panel: Color,
    border_top_bar: Color,
    button_border_idle: Color,
    button_border_hover: Color,
    button_border_pressed: Color,
    radius_panel: Val,
    radius_button: Val,
    radius_hp: Val,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self {
            bg_top: Color::srgb(0.09, 0.12, 0.18),
            bg_bottom: Color::srgb(0.05, 0.07, 0.11),
            top_bar_start: Color::srgba(0.14, 0.26, 0.38, 0.92),
            top_bar_end: Color::srgba(0.10, 0.18, 0.28, 0.88),
            panel: Color::srgb(0.12, 0.16, 0.22),
            button_idle: Color::srgb(0.18, 0.26, 0.34),
            button_hover: Color::srgb(0.23, 0.33, 0.43),
            button_pressed: Color::srgb(0.12, 0.22, 0.30),
            enemy_card_bg: Color::srgb(0.14, 0.20, 0.28),
            enemy_card_border: Color::srgba(0.5, 0.4, 0.45, 0.55),
            hp_track: Color::srgb(0.16, 0.16, 0.19),
            hp_fill_player: Color::srgb(0.17, 0.73, 0.45),
            hp_fill_enemy: Color::srgb(0.89, 0.31, 0.33),
            shield_track: Color::srgb(0.14, 0.18, 0.22),
            shield_fill_player: Color::srgb(0.35, 0.75, 0.95),
            shield_fill_enemy: Color::srgb(0.55, 0.45, 0.95),
            border_panel: Color::srgba(0.42, 0.58, 0.72, 0.45),
            border_top_bar: Color::srgba(0.55, 0.72, 0.88, 0.42),
            button_border_idle: Color::srgba(0.45, 0.58, 0.70, 0.55),
            button_border_hover: Color::srgba(0.58, 0.72, 0.88, 0.65),
            button_border_pressed: Color::srgba(0.32, 0.44, 0.55, 0.55),
            radius_panel: Val::Px(10.0),
            radius_button: Val::Px(8.0),
            radius_hp: Val::Px(7.0),
        }
    }
}

impl UiTheme {
    fn root_background(&self) -> BackgroundGradient {
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::percent(self.bg_top, 0.0),
            ColorStop::percent(self.bg_bottom, 100.0),
        ]))
    }

    fn top_bar_background(&self) -> BackgroundGradient {
        BackgroundGradient::from(LinearGradient::to_right(vec![
            ColorStop::percent(self.top_bar_start, 0.0),
            ColorStop::percent(self.top_bar_end, 100.0),
        ]))
    }

    fn panel_shadow(&self) -> BoxShadow {
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.48),
            Val::Px(0.0),
            Val::Px(5.0),
            Val::Px(0.0),
            Val::Px(14.0),
        )
    }

    fn button_shadow(&self) -> BoxShadow {
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.38),
            Val::Px(0.0),
            Val::Px(3.0),
            Val::Px(0.0),
            Val::Px(10.0),
        )
    }

    fn title_text_shadow(&self) -> TextShadow {
        TextShadow {
            offset: Vec2::new(1.5, 1.5),
            color: Color::srgba(0.0, 0.0, 0.0, 0.72),
        }
    }

    fn result_text_shadow(&self) -> TextShadow {
        TextShadow {
            offset: Vec2::new(2.0, 2.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.78),
        }
    }
}

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiTheme>()
            .add_systems(Startup, (spawn_camera, load_cjk_font_system, setup_ui_system).chain())

            // #region agent log
            .add_systems(
                Update,
                (
                    button_select_skill_system
                        .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn))),
                    button_discard_system
                        .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn))),
                ),
            );

        // #endregion

        // #region agent log
        agent_debug_log(
            "H2",
            "src/ui/mod.rs",
            "about_to_register_update_player_hand_text_system",
            "instrumentation-pre",
        );
        // #endregion

        app.add_systems(
            Update,
            update_player_hand_text_system.run_if(in_state(GameState::Battle)),
        );

        // #region agent log
        agent_debug_log(
            "H3",
            "src/ui/mod.rs",
            "about_to_register_update_battle_text_system",
            "instrumentation-pre",
        );
        // #endregion

        app.add_systems(
            Update,
            update_battle_text_system.run_if(in_state(GameState::Battle)),
        );

        app.add_systems(
            Update,
            (
                button_end_turn_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn))),
                button_visual_state_system.run_if(in_state(GameState::Battle)),
                update_battle_bars_system.run_if(in_state(GameState::Battle)),
                update_action_points_text_system.run_if(in_state(GameState::Battle)),
                process_battle_fx_events,
                tick_skill_flash_timer.after(process_battle_fx_events),
                tick_screen_flashes,
                tick_fx_lifetimes,
            ),
        );

        app.add_systems(Update, update_result_ui_system.run_if(in_state(GameState::Result)));
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn load_cjk_font_system(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
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
                min_height: Val::Px(120.0),
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
                    Button,
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
                min_height: Val::Px(120.0),
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

fn setup_ui_system(mut commands: Commands, theme: Res<UiTheme>, ui_font: Option<Res<UiFontHandle>>) {
    let radius_panel = theme.radius_panel;
    let radius_button = theme.radius_button;
    let radius_hp = theme.radius_hp;
    let border_1 = UiRect::all(Val::Px(1.0));

    let title_font = make_text_font(24.0, ui_font.as_deref());
    let body_font = make_text_font(19.0, ui_font.as_deref());
    let meta_font = make_text_font(14.0, ui_font.as_deref());
    let result_font = make_text_font(28.0, ui_font.as_deref());
    let icon_font = make_text_font(16.0, ui_font.as_deref());

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexStart,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(14.0)),
                ..default()
            },
            theme.root_background(),
            BattleUiRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    border: border_1,
                    border_radius: BorderRadius::all(radius_panel),
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
                    Text::new("操作提示：按 1-4 使用精灵技能，按 Z/X/C/V/B 使用手牌，按 F 弃牌换 AP，按 E 结束回合，按 R 重新开始"),
                    meta_font.clone(),
                    TextColor(Color::srgb(0.80, 0.90, 0.95)),
                    TextShadow {
                        offset: Vec2::new(1.0, 1.0),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.45),
                    },
                ));

                bar.spawn((
                    Text::new("AP：Player 0 / Enemy 0"),
                    body_font.clone(),
                    TextColor(Color::srgb(0.95, 0.98, 1.0)),
                    theme.title_text_shadow(),
                    ActionPointsText,
                ));

                bar.spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(38.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_button),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.button_idle),
                    BorderColor::all(theme.button_border_idle),
                    theme.button_shadow(),
                    DiscardButton,
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new("弃牌（F）"),
                        body_font.clone(),
                        TextColor(Color::WHITE),
                    ));
                });

                bar.spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(38.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                        border: border_1,
                        border_radius: BorderRadius::all(radius_button),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.button_idle),
                    BorderColor::all(theme.button_border_idle),
                    theme.button_shadow(),
                    EndTurnButton,
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new("结束回合（E）"),
                        body_font.clone(),
                        TextColor(Color::WHITE),
                    ));
                });
            });

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(200.0),
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
            });

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(200.0),
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
                spawn_skill_row_player(
                    player_zone,
                    &theme,
                    border_1,
                    radius_button,
                    body_font.clone(),
                    meta_font.clone(),
                    icon_font.clone(),
                );

                player_zone.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        padding: UiRect::top(Val::Px(6.0)),
                        ..default()
                    },
                ))
                .with_children(|hand_node| {
                    hand_node.spawn((
                        Text::new("手牌："),
                        meta_font.clone(),
                        TextColor(Color::srgb(0.75, 0.92, 1.0)),
                    ));
                    for idx in 0..5 {
                        hand_node.spawn((
                            Text::new(format!("{}: —", idx + 1)),
                            body_font.clone(),
                            TextColor(Color::WHITE),
                            PlayerHandCardText { index: idx },
                        ));
                    }
                });
            });

            root.spawn((
                Text::new(""),
                result_font,
                TextColor(Color::srgb(1.0, 0.82, 0.35)),
                theme.result_text_shadow(),
                ResultText,
            ));
        });
}

fn button_select_skill_system(
    mut interaction_query: Query<(&Interaction, &SkillButton), (Changed<Interaction>, With<Button>)>,
    mut turn_ctx: ResMut<TurnContext>,
    action_points: Res<ActionPoints>,
    player_team: Option<Res<PlayerTeam>>,
    query: Query<&SkillList, With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    if let Some(player_team) = player_team {
        if let Some(active_entity) = player_team.0.active_combatant() {
            let Ok(skills) = query.get(active_entity) else {
                return;
            };
            let skills = skills.0;

            for (interaction, button) in &mut interaction_query {
                if *interaction == Interaction::Pressed {
                    let cost = monster_skill_ap_cost_ui(button.index);
                    if action_points.player < cost {
                        continue;
                    }
                    turn_ctx.player_action = Some(crate::battle::TurnAction::Skill(skills[button.index]));
                    // 保持在玩家回合内，由 `player_turn_input_system` 按 AP 规则执行。
                    next_phase.set(BattlePhase::PlayerTurn);
                }
            }
        }
    }
}

fn button_visual_state_system(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            Changed<Interaction>,
            With<SkillButton>,
            Without<SkillFlashTimer>,
        ),
    >,
    theme: Res<UiTheme>,
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

fn format_team_roster(
    header: &str,
    team: &Team,
    query: &Query<(&Combatant, &Stats, &Name, &Shield), With<InBattle>>,
) -> String {
    let total = team.combatants.len();
    let alive = team
        .combatants
        .iter()
        .filter(|&&e| query.get(e).map(|(_, stats, _, _)| stats.hp > 0).unwrap_or(false))
        .count();

    let mut lines = vec![format!("{}（存活 {}/{}）", header, alive, total)];

    for (i, &entity) in team.combatants.iter().enumerate() {
        let Ok((combatant, stats, name, shield)) = query.get(entity) else {
            continue;
        };
        let slot = if i == team.active_index { "[场上]" } else { "[替补]" };
        let shield_str = if shield.0 > 0 {
            format!(" 护盾: {}", shield.0)
        } else {
            String::new()
        };
        let faint = if stats.hp <= 0 { " 倒下" } else { "" };
        lines.push(format!(
            "{} [{}] {} [{}] 生命: {}/{}{}{}",
            slot,
            combatant.side,
            name,
            element_name(combatant.element),
            stats.hp,
            stats.max_hp,
            shield_str,
            faint,
        ));
    }

    lines.join("\n")
}

/// 仅更新战斗相关 `Text`，避免与 `Node` 宽度更新在同一系统内触发 B0001（同一 UI 实体常同时有 `Text` 与 `Node`）。
#[allow(clippy::too_many_arguments)]
fn update_battle_text_system(
    mut text_q: Query<
        (
            &mut Text,
            Option<&BattlePhaseText>,
            Option<&PlayerStatsText>,
            Option<&EnemyStatsText>,
            Option<&ResultText>,
            Option<&SkillButtonText>,
            Option<&SkillButtonMetaText>,
            Option<&SkillButtonIconText>,
            Option<&EnemySkillText>,
            Option<&EnemySkillMetaText>,
            Option<&EnemySkillIconText>,
        ),
        (
            Without<ActionPointsText>,
            Without<PlayerHandCardText>,
            Without<ResultText>,
        ),
    >,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<(&Combatant, &Stats, &Name, &Shield), With<InBattle>>,
    skill_query: Query<&SkillList, With<InBattle>>,
    skill_db: Res<SkillDb>,
    battle_phase: Res<State<BattlePhase>>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_line = format_team_roster("玩家队伍", &player_team.0, &combat_query);
    let enemy_line = format_team_roster("敌方队伍", &enemy_team.0, &combat_query);

    let mut player_skills = None;
    if let Some(p_entity) = player_team.0.active_combatant() {
        if let Ok(skills) = skill_query.get(p_entity) {
            player_skills = Some(skills.0);
        }
    }

    let mut enemy_skills = None;
    if let Some(e_entity) = enemy_team.0.active_combatant() {
        if let Ok(skills) = skill_query.get(e_entity) {
            enemy_skills = Some(skills.0);
        }
    }

    for (
        mut text,
        is_phase,
        is_player,
        is_enemy,
        is_result,
        skill_button_text,
        skill_button_meta_text,
        skill_icon_text,
        enemy_skill_text,
        enemy_skill_meta,
        enemy_skill_icon,
    ) in &mut text_q
    {
        if is_result.is_some() {
            text.0.clear();
            continue;
        }
        if is_phase.is_some() {
            text.0 = format!("战斗阶段：{}", phase_label(*battle_phase.get()));
            continue;
        }
        if is_player.is_some() {
            text.0 = player_line.clone();
            continue;
        }
        if is_enemy.is_some() {
            text.0 = enemy_line.clone();
            continue;
        }
        if let (Some(button), Some(skills)) = (skill_button_text, player_skills) {
            let skill_id = skills[button.index];
            text.0 = format!("{}号: {}", button.index + 1, skill_name(skill_id, &skill_db));
            continue;
        }
        if let (Some(meta), Some(skills)) = (skill_button_meta_text, player_skills) {
            let skill_id = skills[meta.index];
            text.0 = format!(
                "{} AP消耗：{}",
                skill_meta(skill_id, &skill_db),
                monster_skill_ap_cost_ui(meta.index)
            );
            continue;
        }
        if let (Some(icon), Some(_skills)) = (skill_icon_text, player_skills) {
            text.0 = (icon.index + 1).to_string();
            continue;
        }
        if let (Some(button), Some(skills)) = (enemy_skill_text, enemy_skills) {
            let skill_id = skills[button.index];
            text.0 = format!("{}号: {}", button.index + 1, skill_name(skill_id, &skill_db));
            continue;
        }
        if let (Some(meta), Some(skills)) = (enemy_skill_meta, enemy_skills) {
            let skill_id = skills[meta.index];
            text.0 = skill_meta(skill_id, &skill_db);
            continue;
        }
        if let (Some(icon), Some(_skills)) = (enemy_skill_icon, enemy_skills) {
            text.0 = (icon.index + 1).to_string();
        }
    }
}

fn update_battle_bars_system(
    mut fills: ParamSet<(
        Query<&mut Node, With<PlayerHpBarFill>>,
        Query<&mut Node, With<EnemyHpBarFill>>,
        Query<&mut Node, With<PlayerShieldBarFill>>,
        Query<&mut Node, With<EnemyShieldBarFill>>,
    )>,
    mut tracks: ParamSet<(
        Query<&mut Visibility, With<PlayerShieldBarTrack>>,
        Query<&mut Visibility, With<EnemyShieldBarTrack>>,
    )>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<(&Combatant, &Stats, &Name, &Shield), With<InBattle>>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_hp_pct = active_hp_percent(&player_team.0, &combat_query);
    let enemy_hp_pct = active_hp_percent(&enemy_team.0, &combat_query);

    if let Ok(mut node) = fills.p0().single_mut() {
        node.width = Val::Percent(player_hp_pct);
    }
    if let Ok(mut node) = fills.p1().single_mut() {
        node.width = Val::Percent(enemy_hp_pct);
    }

    let player_shield_pct = active_shield_percent(&player_team.0, &combat_query);
    let enemy_shield_pct = active_shield_percent(&enemy_team.0, &combat_query);

    if let Ok(mut node) = fills.p2().single_mut() {
        node.width = Val::Percent(player_shield_pct);
    }
    if let Ok(mut node) = fills.p3().single_mut() {
        node.width = Val::Percent(enemy_shield_pct);
    }

    let player_has_shield = active_shield(&player_team.0, &combat_query) > 0;
    let enemy_has_shield = active_shield(&enemy_team.0, &combat_query) > 0;

    if let Ok(mut vis) = tracks.p0().single_mut() {
        *vis = if player_has_shield {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut vis) = tracks.p1().single_mut() {
        *vis = if enemy_has_shield {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn update_action_points_text_system(
    action_points: Res<ActionPoints>,
    mut text_q: Query<&mut Text, With<ActionPointsText>>,
) {
    if let Ok(mut text) = text_q.single_mut() {
        text.0 = format!(
            "AP：Player {} / Enemy {}",
            action_points.player, action_points.enemy
        );
    }
}

fn update_player_hand_text_system(
    hand: Res<Hand>,
    card_db: Res<CardDb>,
    mut text_q: Query<(&PlayerHandCardText, &mut Text)>,
) {
    for (meta, mut text) in &mut text_q {
        if meta.index < hand.player.len() {
            let cid = hand.player[meta.index];
            if let Some(card) = card_db.0.get(&cid) {
                text.0 = format!("{}: {}（AP{}）", meta.index + 1, card.name, card.cost_ap);
            } else {
                text.0 = format!("{}: —", meta.index + 1);
            }
        } else {
            text.0 = format!("{}: —", meta.index + 1);
        }
    }
}

fn button_discard_system(
    mut interaction_query: Query<(&Interaction, &DiscardButton), (Changed<Interaction>, With<Button>)>,
    mut turn_ctx: ResMut<TurnContext>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    card_db: Res<CardDb>,
    mut event_writer: MessageWriter<BattleEvent>,
) {
    for (interaction, _) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            if hand.player.is_empty() {
                continue;
            }
            let card_id = hand.player.remove(0);
            action_points.player += 1;

            let card_name = card_db
                .0
                .get(&card_id)
                .map(|c| c.name.to_string())
                .unwrap_or_else(|| format!("{card_id:?}"));

            event_writer.write(BattleEvent::CardDiscarded {
                side: Side::Player,
                card_name,
            });

            // 清理一次性动作，避免 UI 点击后的残留。
            turn_ctx.player_action = None;
            break;
        }
    }
}

fn button_end_turn_system(
    mut interaction_query: Query<(&Interaction, &EndTurnButton), (Changed<Interaction>, With<Button>)>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    for (interaction, _) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            turn_ctx.player_ended = true;
            turn_ctx.player_action = None;
            next_phase.set(BattlePhase::EnemyTurn);
            break;
        }
    }
}

fn update_result_ui_system(
    mut result_text_q: Query<&mut Text, With<ResultText>>,
    battle_result: Res<BattleResult>,
) {
    if let Ok(mut result_text) = result_text_q.single_mut() {
        result_text.0 = battle_result.message.clone();
    }
}

fn skill_name(skill_id: SkillId, db: &SkillDb) -> String {
    db.0
        .get(&skill_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("{skill_id:?}"))
}

fn skill_meta(skill_id: SkillId, db: &SkillDb) -> String {
    let Some(skill) = db.0.get(&skill_id) else {
        return "类型：未知".to_string();
    };
    match &skill.effect {
        SkillEffect::Attack { .. } => {
            let element_text = match skill.element {
                Some(ElementType::Water) => "·水系",
                Some(ElementType::Fire) => "·火系",
                Some(ElementType::Grass) => "·草系",
                Some(ElementType::Light) => "·光系",
                Some(ElementType::Dark) => "·暗系",
                Some(ElementType::Thunder) => "·雷系",
                Some(ElementType::Wind) => "·风系",
                None => "",
            };
            format!("类型：攻击{}", element_text)
        }
        SkillEffect::Heal { .. } => "类型：治疗".to_string(),
        SkillEffect::Shield { .. } => "类型：护盾".to_string(),
    }
}

fn monster_skill_ap_cost_ui(slot: usize) -> i32 {
    match slot {
        0 => 2,
        1 => 3,
        2 => 1,
        3 => 1,
        _ => 999,
    }
}

fn phase_label(phase: BattlePhase) -> &'static str {
    match phase {
        BattlePhase::Init => "初始化",
        BattlePhase::RoundStart => "回合开始",
        BattlePhase::PlayerTurn => "玩家回合",
        BattlePhase::EnemyTurn => "敌方回合",
        BattlePhase::CheckEnd => "胜负判定",
    }
}

fn active_hp_percent(team: &Team, query: &Query<(&Combatant, &Stats, &Name, &Shield), With<InBattle>>) -> f32 {
    let Some(entity) = team.active_combatant() else {
        return 0.0;
    };
    let Ok((_, stats, _, _)) = query.get(entity) else {
        return 0.0;
    };
    if stats.max_hp <= 0 {
        return 0.0;
    }
    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
}

fn active_shield(team: &Team, query: &Query<(&Combatant, &Stats, &Name, &Shield), With<InBattle>>) -> i32 {
    let Some(entity) = team.active_combatant() else {
        return 0;
    };
    let Ok((_, _, _, shield)) = query.get(entity) else {
        return 0;
    };
    shield.0.max(0)
}

fn active_shield_percent(team: &Team, query: &Query<(&Combatant, &Stats, &Name, &Shield), With<InBattle>>) -> f32 {
    let Some(entity) = team.active_combatant() else {
        return 0.0;
    };
    let Ok((_, stats, _, shield)) = query.get(entity) else {
        return 0.0;
    };
    if stats.max_hp <= 0 {
        return 0.0;
    }
    ((shield.0.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
}

fn element_name(element: ElementType) -> &'static str {
    match element {
        ElementType::Water => "水",
        ElementType::Fire => "火",
        ElementType::Grass => "草",
        ElementType::Light => "光",
        ElementType::Dark => "暗",
        ElementType::Thunder => "雷",
        ElementType::Wind => "风",
    }
}

fn make_text_font(size: f32, ui_font: Option<&UiFontHandle>) -> TextFont {
    let mut text_font = TextFont::from_font_size(size);
    if let Some(font) = ui_font {
        text_font.font = font.0.clone();
    }
    text_font
}
