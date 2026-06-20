use bevy::prelude::*;

use super::super::components::{
    ActionDialButton, ActionDialHighlight, BattleHintButton, BattleHintCloseButton, BattleUiNotice,
    DiscardButton, EndTurnButton, HandFullHintRoot, HandFullHintText, PlayerCardButton,
    RetreatButton, RetreatConfirmCancelButton, RetreatConfirmProceedButton, SkillButton,
    SwitchCancelButton, SwitchMonsterButton, TeamMemberButton,
};
use super::super::fx::{ButtonClickFlash, SkillFlashTimer};
use super::super::theme::UiTheme;
use super::buttons::HandFullEndTurnWarning;
use crate::battle::{BattleActionCooldown, BattleControlMode, SelectedCards, Side, UiControlSide};
use crate::game_state::BattlePhase;

/// 弃牌武装状态的高亮颜色（琥珀色，与蓝白点击闪光明显区分）
const DISCARD_ARMED_COLOR: Color = Color::srgba(0.85, 0.60, 0.10, 0.88);
const DISCARD_ARMED_BORDER: Color = Color::srgba(0.90, 0.72, 0.18, 0.85);
const HAND_FULL_BORDER: Color = Color::srgba(1.0, 0.08, 0.08, 0.95);
const HAND_FULL_HINT_DURATION: f32 = 3.0;
const HAND_FULL_HINT_FADE: f32 = 0.65;
const ACTION_COOLDOWN_LOCKED_BG: Color = Color::srgba(0.18, 0.13, 0.065, 1.0);
const ACTION_COOLDOWN_LOCKED_CARD_BG: Color = Color::srgba(0.52, 0.47, 0.37, 1.0);
const ACTION_COOLDOWN_LOCKED_BORDER: Color = Color::srgba(0.34, 0.26, 0.13, 0.75);
const ACTION_COOLDOWN_DIAL_HIGHLIGHT: Color = Color::srgba(0.52, 0.42, 0.20, 0.18);

/// 战斗常规按钮（技能/结束/撤退/提示/换人）的暖色漆器底，与描金协调。
const BTN_IDLE_BG: Color = Color::srgba(0.34, 0.245, 0.115, 1.0);
const BTN_HOVER_BG: Color = Color::srgba(0.45, 0.33, 0.16, 1.0);
const BTN_PRESSED_BG: Color = Color::srgba(0.22, 0.155, 0.075, 1.0);

pub(crate) fn apply_regular_button_style(
    interaction: &Interaction,
    bg: &mut BackgroundColor,
    border: &mut BorderColor,
    theme: &UiTheme,
) {
    *bg = match *interaction {
        Interaction::Pressed => BackgroundColor(BTN_PRESSED_BG),
        Interaction::Hovered => BackgroundColor(BTN_HOVER_BG),
        Interaction::None => BackgroundColor(BTN_IDLE_BG),
    };
    *border = match *interaction {
        Interaction::Pressed => BorderColor::all(theme.gold),
        Interaction::Hovered => BorderColor::all(theme.gold_bright),
        Interaction::None => BorderColor::all(theme.gold_dim),
    };
}

fn apply_card_button_style(
    interaction: &Interaction,
    bg: &mut BackgroundColor,
    border: &mut BorderColor,
    theme: &UiTheme,
) {
    *bg = match *interaction {
        Interaction::Pressed => BackgroundColor(theme.parchment_edge),
        Interaction::Hovered => BackgroundColor(theme.parchment_bright),
        Interaction::None => BackgroundColor(theme.card_bg),
    };
    *border = match *interaction {
        Interaction::Pressed => BorderColor::all(theme.gold_dim),
        Interaction::Hovered => BorderColor::all(theme.gold_bright),
        Interaction::None => BorderColor::all(theme.card_border),
    };
}

pub(crate) fn button_visual_state_system(
    mut skill_buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            Changed<Interaction>,
            With<SkillButton>,
            Without<crate::ui::battle::fx::SkillFlashTimer>,
            Without<ButtonClickFlash>,
            Without<EndTurnButton>,
            Without<DiscardButton>,
            Without<PlayerCardButton>,
            Without<SwitchMonsterButton>,
            Without<SwitchCancelButton>,
            Without<TeamMemberButton>,
            Without<ActionDialButton>,
        ),
    >,
    mut action_buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            Changed<Interaction>,
            Or<(
                With<EndTurnButton>,
                With<DiscardButton>,
                With<RetreatButton>,
                With<RetreatConfirmCancelButton>,
                With<RetreatConfirmProceedButton>,
                With<BattleHintButton>,
                With<BattleHintCloseButton>,
            )>,
            Without<ButtonClickFlash>,
            Without<SkillButton>,
            Without<PlayerCardButton>,
            Without<SwitchMonsterButton>,
            Without<SwitchCancelButton>,
            Without<TeamMemberButton>,
            Without<ActionDialButton>,
        ),
    >,
    mut card_buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            Changed<Interaction>,
            With<PlayerCardButton>,
            Without<ButtonClickFlash>,
            Without<SkillButton>,
            Without<EndTurnButton>,
            Without<DiscardButton>,
            Without<SwitchMonsterButton>,
            Without<SwitchCancelButton>,
            Without<TeamMemberButton>,
            Without<ActionDialButton>,
        ),
    >,
    mut switch_buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            Changed<Interaction>,
            Or<(With<SwitchMonsterButton>, With<SwitchCancelButton>)>,
            Without<ButtonClickFlash>,
            Without<SkillButton>,
            Without<EndTurnButton>,
            Without<DiscardButton>,
            Without<PlayerCardButton>,
            Without<ActionDialButton>,
        ),
    >,
    theme: Res<UiTheme>,
) {
    for (interaction, mut bg, mut border) in &mut skill_buttons {
        apply_regular_button_style(interaction, &mut bg, &mut border, &theme);
    }
    for (interaction, mut bg, mut border) in &mut action_buttons {
        apply_regular_button_style(interaction, &mut bg, &mut border, &theme);
    }
    for (interaction, mut bg, mut border) in &mut card_buttons {
        apply_card_button_style(interaction, &mut bg, &mut border, &theme);
    }
    for (interaction, mut bg, mut border) in &mut switch_buttons {
        apply_regular_button_style(interaction, &mut bg, &mut border, &theme);
    }
}

pub(crate) fn battle_action_cooldown_visual_system(
    cooldown: Res<BattleActionCooldown>,
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    theme: Res<UiTheme>,
    mut was_locked: Local<bool>,
    mut queries: ParamSet<(
        Query<
            (
                &Interaction,
                &mut BackgroundColor,
                &mut BorderColor,
                Has<PlayerCardButton>,
            ),
            (
                Or<(
                    With<SkillButton>,
                    With<PlayerCardButton>,
                    With<TeamMemberButton>,
                    With<EndTurnButton>,
                )>,
                Without<ActionDialButton>,
                Without<ActionDialHighlight>,
                Without<ButtonClickFlash>,
                Without<SkillFlashTimer>,
            ),
        >,
        Query<
            (&Children, Has<DiscardButton>, Has<EndTurnButton>),
            (With<ActionDialButton>, Without<ButtonClickFlash>),
        >,
        Query<&mut BackgroundColor, With<ActionDialHighlight>>,
    )>,
) {
    // 本地玩家按钮变灰（锁定）的两个条件：
    //   1. 对方回合（PVP/AI 下非我方回合）——与 AI 模式一致的“非我方回合按钮变色”。
    //      Debug 双控模式下玩家可控制敌方，此时敌方回合不视为对方回合，不变灰。
    //   2. 本地玩家自己刚用完技能，处于 2 秒行动冷却。
    // 对方使用技能只会启动对方侧冷却，不会让本地按钮变灰。
    let opponent_turn = *battle_phase.get() == BattlePhase::EnemyTurn
        && *battle_mode != BattleControlMode::DebugPlayerControlsBoth;
    let locked = opponent_turn || !cooldown.ready(Side::Player);
    let should_restore = *was_locked && !locked;
    if !locked && !should_restore {
        return;
    }

    for (interaction, mut bg, mut border, is_card) in &mut queries.p0() {
        if locked {
            // 手牌正常是羊皮纸，锁定时变暗化的灰羊皮纸；技能/按钮变暗化暖褐。
            *bg = if is_card {
                BackgroundColor(ACTION_COOLDOWN_LOCKED_CARD_BG)
            } else {
                BackgroundColor(ACTION_COOLDOWN_LOCKED_BG)
            };
            *border = BorderColor::all(ACTION_COOLDOWN_LOCKED_BORDER);
        } else if is_card {
            apply_card_button_style(interaction, &mut bg, &mut border, &theme);
        } else {
            apply_regular_button_style(interaction, &mut bg, &mut border, &theme);
        }
    }

    if !locked {
        *was_locked = false;
        return;
    }

    let mut highlight_updates = Vec::new();
    for (children, is_discard, is_end_turn) in &queries.p1() {
        if !is_discard && !is_end_turn {
            continue;
        }
        for child in children.iter() {
            highlight_updates.push(child);
        }
    }

    let mut highlights = queries.p2();
    for child in highlight_updates {
        if let Ok(mut highlight_bg) = highlights.get_mut(child) {
            *highlight_bg = BackgroundColor(ACTION_COOLDOWN_DIAL_HIGHLIGHT);
        }
    }
    *was_locked = true;
}

pub(crate) fn action_dial_visual_state_system(
    warning: Res<HandFullEndTurnWarning>,
    mut queries: ParamSet<(
        Query<
            (
                &Interaction,
                &Children,
                &mut BackgroundColor,
                &mut BorderColor,
                Has<DiscardButton>,
            ),
            (
                With<ActionDialButton>,
                Without<ActionDialHighlight>,
                Without<ButtonClickFlash>,
            ),
        >,
        Query<&mut BackgroundColor, (With<ActionDialHighlight>, Without<ActionDialButton>)>,
    )>,
) {
    let warning_active = !warning.border_timer.is_finished();
    let mut highlight_updates = Vec::new();
    for (interaction, children, mut bg, mut border, is_discard) in &mut queries.p0() {
        *bg = BackgroundColor(Color::NONE);
        *border = if warning_active && is_discard {
            BorderColor::all(HAND_FULL_BORDER)
        } else {
            BorderColor::all(Color::NONE)
        };
        let highlight_color = match *interaction {
            Interaction::Pressed => Color::srgba(0.72, 0.88, 1.0, 0.34),
            Interaction::Hovered => Color::srgba(0.72, 0.88, 1.0, 0.18),
            Interaction::None => Color::NONE,
        };
        for child in children.iter() {
            highlight_updates.push((child, highlight_color));
        }
    }

    let mut highlights = queries.p1();
    for (child, highlight_color) in highlight_updates {
        if let Ok(mut highlight_bg) = highlights.get_mut(child) {
            *highlight_bg = BackgroundColor(highlight_color);
        }
    }
}

/// 维护弃牌按钮的持久高亮状态。
///
/// 优先级（由高到低）：
///   1. `ButtonClickFlash`（点击瞬间蓝白闪光，通过 `Without` 过滤自动跳过）
///   2. Hover / Pressed（`button_visual_state_system` 处理，需在其之后运行）
///   3. 武装高亮（`discard_armed = true` 且 `Interaction::None`）
///   4. 空闲色（`discard_armed = false` 且 `Interaction::None`）
///
/// 每帧轮询，确保 Hover → Un-hover 后武装色能正确恢复。
pub(crate) fn hand_full_warning_visual_system(
    time: Res<Time>,
    mut warning: ResMut<HandFullEndTurnWarning>,
    mut notices: MessageReader<BattleUiNotice>,
    mut hint_roots: Query<&mut Node, With<HandFullHintRoot>>,
    mut hint_texts: Query<(&mut Text, &mut TextColor, &mut TextShadow), With<HandFullHintText>>,
) {
    for notice in notices.read() {
        warning.hint_text = notice.text;
        warning.hint_timer = Timer::from_seconds(HAND_FULL_HINT_DURATION, TimerMode::Once);
    }

    warning.border_timer.tick(time.delta());
    warning.hint_timer.tick(time.delta());

    let elapsed = warning.hint_timer.elapsed_secs();
    let alpha = if warning.hint_timer.is_finished() {
        0.0
    } else if elapsed < HAND_FULL_HINT_FADE {
        elapsed / HAND_FULL_HINT_FADE
    } else {
        let remaining = HAND_FULL_HINT_DURATION - elapsed;
        if remaining < HAND_FULL_HINT_FADE {
            remaining.max(0.0) / HAND_FULL_HINT_FADE
        } else {
            1.0
        }
    };

    for mut node in &mut hint_roots {
        node.display = if alpha > 0.0 {
            Display::Flex
        } else {
            Display::None
        };
    }

    let blur = 1.0 - alpha;
    for (mut text, mut text_color, mut shadow) in &mut hint_texts {
        text.0 = warning.hint_text.to_string();
        text_color.0 = Color::srgba(1.0, 0.94, 0.86, alpha);
        shadow.offset = Vec2::splat(blur * 4.0);
        shadow.color = Color::srgba(1.0, 0.18, 0.18, blur * 0.85);
    }
}

pub(crate) fn update_discard_armed_visual_system(
    selected: Res<SelectedCards>,
    ui_control_side: Res<UiControlSide>,
    mut queries: ParamSet<(
        Query<
            (&Interaction, &mut BackgroundColor, &mut BorderColor),
            (
                With<DiscardButton>,
                Without<ButtonClickFlash>,
                Without<ActionDialButton>,
            ),
        >,
        Query<
            (&Interaction, &Children),
            (
                With<DiscardButton>,
                With<ActionDialButton>,
                Without<ButtonClickFlash>,
            ),
        >,
        Query<&mut BackgroundColor, With<ActionDialHighlight>>,
    )>,
    theme: Res<UiTheme>,
) {
    let discard_armed = match ui_control_side.0 {
        Side::Player => selected.player.discard_armed,
        Side::Enemy => selected.enemy.discard_armed,
    };

    for (interaction, mut bg, mut border) in &mut queries.p0() {
        // 仅在未悬停/未按下时介入，Hover/Press 状态交给 button_visual_state_system
        if *interaction == Interaction::None {
            if discard_armed {
                *bg = BackgroundColor(DISCARD_ARMED_COLOR);
                *border = BorderColor::all(DISCARD_ARMED_BORDER);
            } else {
                *bg = BackgroundColor(theme.button_idle);
                *border = BorderColor::all(theme.button_border_idle);
            }
        }
    }

    let mut highlight_updates = Vec::new();
    for (interaction, children) in &queries.p1() {
        // 圆盘按钮的颜色显示在 ActionDialHighlight 子节点上。
        if *interaction == Interaction::None {
            let color = if discard_armed {
                DISCARD_ARMED_COLOR
            } else {
                Color::NONE
            };
            for child in children.iter() {
                highlight_updates.push((child, color));
            }
        }
    }

    let mut highlights = queries.p2();
    for (child, color) in highlight_updates {
        if let Ok(mut highlight_bg) = highlights.get_mut(child) {
            *highlight_bg = BackgroundColor(color);
        }
    }
}
