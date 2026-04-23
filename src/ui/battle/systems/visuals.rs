use bevy::prelude::*;

use super::super::components::{
    DiscardButton, EndTurnButton, PlayerCardButton, SkillButton, SwitchCancelButton,
    SwitchMonsterButton, TeamMemberButton,
};
use super::super::fx::ButtonClickFlash;
use super::super::theme::UiTheme;
use crate::battle::SelectedCard;

/// 弃牌武装状态的高亮颜色（琥珀色，与蓝白点击闪光明显区分）
const DISCARD_ARMED_COLOR: Color = Color::srgba(0.85, 0.60, 0.10, 0.88);
const DISCARD_ARMED_BORDER: Color = Color::srgba(0.90, 0.72, 0.18, 0.85);

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
        ),
    >,
    mut action_buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            Changed<Interaction>,
            Or<(With<EndTurnButton>, With<DiscardButton>)>,
            Without<ButtonClickFlash>,
            Without<SkillButton>,
            Without<PlayerCardButton>,
            Without<SwitchMonsterButton>,
            Without<SwitchCancelButton>,
            Without<TeamMemberButton>,
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
        ),
    >,
    theme: Res<UiTheme>,
) {
    let apply = |interaction: &Interaction, bg: &mut BackgroundColor, border: &mut BorderColor, theme: &UiTheme| {
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

/// 维护弃牌按钮的持久高亮状态。
///
/// 优先级（由高到低）：
///   1. `ButtonClickFlash`（点击瞬间蓝白闪光，通过 `Without` 过滤自动跳过）
///   2. Hover / Pressed（`button_visual_state_system` 处理，需在其之后运行）
///   3. 武装高亮（`discard_armed = true` 且 `Interaction::None`）
///   4. 空闲色（`discard_armed = false` 且 `Interaction::None`）
///
/// 每帧轮询，确保 Hover → Un-hover 后武装色能正确恢复。
pub(crate) fn update_discard_armed_visual_system(
    selected: Res<SelectedCard>,
    mut q: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (With<DiscardButton>, Without<ButtonClickFlash>),
    >,
    theme: Res<UiTheme>,
) {
    for (interaction, mut bg, mut border) in &mut q {
        // 仅在未悬停/未按下时介入，Hover/Press 状态交给 button_visual_state_system
        if *interaction == Interaction::None {
            if selected.discard_armed {
                *bg = BackgroundColor(DISCARD_ARMED_COLOR);
                *border = BorderColor::all(DISCARD_ARMED_BORDER);
            } else {
                *bg = BackgroundColor(theme.button_idle);
                *border = BorderColor::all(theme.button_border_idle);
            }
        }
    }
}

