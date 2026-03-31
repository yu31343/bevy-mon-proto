use bevy::prelude::*;

use super::super::components::SkillButton;
use super::super::theme::UiTheme;

pub(crate) fn button_visual_state_system(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            Changed<Interaction>,
            With<SkillButton>,
            Without<crate::ui::battle::fx::SkillFlashTimer>,
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

