use bevy::prelude::*;

#[derive(Resource, Clone)]
pub(crate) struct UiTheme {
    pub bg_top: Color,
    pub bg_bottom: Color,
    pub top_bar_start: Color,
    pub top_bar_end: Color,
    pub panel: Color,
    pub button_idle: Color,
    pub button_hover: Color,
    pub button_pressed: Color,
    pub enemy_card_bg: Color,
    pub enemy_card_border: Color,
    pub hp_track: Color,
    pub hp_fill_player: Color,
    pub hp_fill_enemy: Color,
    pub shield_track: Color,
    pub shield_fill_player: Color,
    pub shield_fill_enemy: Color,
    pub border_panel: Color,
    pub border_top_bar: Color,
    pub button_border_idle: Color,
    pub button_border_hover: Color,
    pub button_border_pressed: Color,
    pub radius_panel: Val,
    pub radius_button: Val,
    pub radius_hp: Val,
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
    pub fn root_background(&self) -> BackgroundGradient {
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::percent(self.bg_top, 0.0),
            ColorStop::percent(self.bg_bottom, 100.0),
        ]))
    }

    pub fn top_bar_background(&self) -> BackgroundGradient {
        BackgroundGradient::from(LinearGradient::to_right(vec![
            ColorStop::percent(self.top_bar_start, 0.0),
            ColorStop::percent(self.top_bar_end, 100.0),
        ]))
    }

    pub fn panel_shadow(&self) -> BoxShadow {
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.48),
            Val::Px(0.0),
            Val::Px(5.0),
            Val::Px(0.0),
            Val::Px(14.0),
        )
    }

    pub fn button_shadow(&self) -> BoxShadow {
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.38),
            Val::Px(0.0),
            Val::Px(3.0),
            Val::Px(0.0),
            Val::Px(10.0),
        )
    }

    pub fn title_text_shadow(&self) -> TextShadow {
        TextShadow {
            offset: Vec2::new(1.5, 1.5),
            color: Color::srgba(0.0, 0.0, 0.0, 0.72),
        }
    }

    pub fn result_text_shadow(&self) -> TextShadow {
        TextShadow {
            offset: Vec2::new(2.0, 2.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.78),
        }
    }
}

