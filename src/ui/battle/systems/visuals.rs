use bevy::prelude::*;

use super::super::components::{
    ActionDialButton, ActionDialHighlight, BattleHintButton, BattleHintCloseButton, DiscardButton,
    EndTurnButton, HandFullHintRoot, HandFullHintText, PlayerCardButton, ReserveInfoButton,
    ReserveInfoCloseButton, RetreatButton, SkillButton, StatIconButton, StatIconTooltip,
    SwitchCancelButton, SwitchMonsterButton, TeamMemberButton,
};
use super::super::fx::ButtonClickFlash;
use super::super::theme::UiTheme;
use super::buttons::HandFullEndTurnWarning;
use crate::battle::{SelectedCards, Side, UiControlSide};

/// 弃牌武装状态的高亮颜色（琥珀色，与蓝白点击闪光明显区分）
const DISCARD_ARMED_COLOR: Color = Color::srgba(0.85, 0.60, 0.10, 0.88);
const DISCARD_ARMED_BORDER: Color = Color::srgba(0.90, 0.72, 0.18, 0.85);
const HAND_FULL_BORDER: Color = Color::srgba(1.0, 0.08, 0.08, 0.95);
const HAND_FULL_HINT_DURATION: f32 = 3.0;
const HAND_FULL_HINT_FADE: f32 = 0.65;

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
                With<ReserveInfoButton>,
                With<ReserveInfoCloseButton>,
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
            Or<(
                With<SwitchMonsterButton>,
                With<SwitchCancelButton>,
                With<TeamMemberButton>,
            )>,
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
    let apply = |interaction: &Interaction,
                 bg: &mut BackgroundColor,
                 border: &mut BorderColor,
                 theme: &UiTheme| {
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
    };
    for (interaction, mut bg, mut border) in &mut skill_buttons {
        apply(interaction, &mut bg, &mut border, &theme);
    }
    for (interaction, mut bg, mut border) in &mut action_buttons {
        apply(interaction, &mut bg, &mut border, &theme);
    }
    for (interaction, mut bg, mut border) in &mut card_buttons {
        apply(interaction, &mut bg, &mut border, &theme);
    }
    for (interaction, mut bg, mut border) in &mut switch_buttons {
        apply(interaction, &mut bg, &mut border, &theme);
    }
}

pub(crate) fn stat_icon_tooltip_system(
    mut queries: ParamSet<(
        Query<(&Interaction, &Children), (Changed<Interaction>, With<StatIconButton>)>,
        Query<&mut Node, With<StatIconTooltip>>,
    )>,
) {
    let mut updates = Vec::new();
    for (interaction, children) in &queries.p0() {
        let display = match *interaction {
            Interaction::None => Display::None,
            Interaction::Hovered | Interaction::Pressed => Display::Flex,
        };
        for child in children.iter() {
            updates.push((child, display));
        }
    }

    let mut tooltips = queries.p1();
    for (child, display) in updates {
        if let Ok(mut node) = tooltips.get_mut(child) {
            node.display = display;
        }
    }
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
    mut hint_roots: Query<&mut Node, With<HandFullHintRoot>>,
    mut hint_texts: Query<(&mut TextColor, &mut TextShadow), With<HandFullHintText>>,
) {
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
    for (mut text_color, mut shadow) in &mut hint_texts {
        text_color.0 = Color::srgba(1.0, 0.94, 0.86, alpha);
        shadow.offset = Vec2::splat(blur * 4.0);
        shadow.color = Color::srgba(1.0, 0.18, 0.18, blur * 0.85);
    }
}

pub(crate) fn update_discard_armed_visual_system(
    selected: Res<SelectedCards>,
    ui_control_side: Res<UiControlSide>,
    mut q: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            With<DiscardButton>,
            Without<ButtonClickFlash>,
            Without<ActionDialButton>,
        ),
    >,
    theme: Res<UiTheme>,
) {
    let discard_armed = match ui_control_side.0 {
        Side::Player => selected.player.discard_armed,
        Side::Enemy => selected.enemy.discard_armed,
    };

    for (interaction, mut bg, mut border) in &mut q {
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
}
