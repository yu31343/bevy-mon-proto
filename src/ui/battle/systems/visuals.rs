use bevy::prelude::*;

use super::super::components::{DiscardButton, EndTurnButton, PlayerCardButton, SkillButton};
use super::super::fx::ButtonClickFlash;
use super::super::theme::UiTheme;

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
}

