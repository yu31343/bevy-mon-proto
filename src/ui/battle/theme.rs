use bevy::prelude::*;

#[derive(Resource, Clone)]
pub(crate) struct UiTheme {
    pub bg_top: Color,
    pub bg_bottom: Color,
    pub top_bar_start: Color,
    pub top_bar_end: Color,
    pub panel: Color,
    #[allow(dead_code)]
    pub panel_inner: Color,
    pub button_idle: Color,
    pub button_hover: Color,
    pub button_pressed: Color,
    pub enemy_card_bg: Color,
    pub enemy_card_border: Color,
    pub card_bg: Color,
    pub card_border: Color,
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
    pub radius_card: Val,
    pub accent_player: Color,
    pub accent_enemy: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub divider: Color,
    pub aura_fire: Color,
    pub aura_water: Color,
    pub aura_grass: Color,
    pub aura_light: Color,
    pub aura_dark: Color,
    pub aura_thunder: Color,
    pub aura_wind: Color,
    pub status_burn: Color,
    pub status_scorch: Color,
    pub status_paralysis: Color,
    pub status_poison_like: Color,
    pub status_buff: Color,
    pub status_debuff: Color,
    pub status_special: Color,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self {
            bg_top: Color::srgb(0.06, 0.08, 0.14),
            bg_bottom: Color::srgb(0.02, 0.03, 0.08),
            top_bar_start: Color::srgba(0.07, 0.14, 0.26, 0.96),
            top_bar_end: Color::srgba(0.04, 0.08, 0.16, 0.92),
            panel: Color::srgb(0.07, 0.10, 0.17),
            panel_inner: Color::srgba(0.04, 0.06, 0.10, 0.60),
            button_idle: Color::srgb(0.10, 0.17, 0.27),
            button_hover: Color::srgb(0.15, 0.25, 0.38),
            button_pressed: Color::srgb(0.07, 0.12, 0.20),
            enemy_card_bg: Color::srgb(0.20, 0.09, 0.12),
            enemy_card_border: Color::srgba(0.75, 0.28, 0.35, 0.70),
            card_bg: Color::srgb(0.08, 0.14, 0.24),
            card_border: Color::srgba(0.30, 0.55, 0.82, 0.60),
            hp_track: Color::srgb(0.06, 0.08, 0.12),
            hp_fill_player: Color::srgb(0.06, 0.78, 0.52),
            hp_fill_enemy: Color::srgb(0.92, 0.18, 0.22),
            shield_track: Color::srgb(0.05, 0.08, 0.12),
            shield_fill_player: Color::srgb(0.16, 0.80, 0.96),
            shield_fill_enemy: Color::srgb(0.64, 0.34, 0.96),
            border_panel: Color::srgba(0.25, 0.48, 0.75, 0.50),
            border_top_bar: Color::srgba(0.38, 0.65, 0.92, 0.45),
            button_border_idle: Color::srgba(0.28, 0.50, 0.76, 0.60),
            button_border_hover: Color::srgba(0.45, 0.72, 0.96, 0.80),
            button_border_pressed: Color::srgba(0.18, 0.36, 0.55, 0.65),
            radius_panel: Val::Px(18.0),
            radius_button: Val::Px(12.0),
            radius_hp: Val::Px(10.0),
            radius_card: Val::Px(14.0),
            accent_player: Color::srgb(0.30, 0.75, 1.0),
            accent_enemy: Color::srgb(1.0, 0.45, 0.50),
            text_primary: Color::srgb(0.95, 0.97, 1.0),
            text_secondary: Color::srgb(0.72, 0.84, 0.95),
            text_muted: Color::srgb(0.50, 0.62, 0.78),
            divider: Color::srgba(0.25, 0.45, 0.70, 0.30),
            aura_fire: Color::srgb(1.0, 0.58, 0.22),
            aura_water: Color::srgb(0.42, 0.76, 1.0),
            aura_grass: Color::srgb(0.58, 0.90, 0.50),
            aura_light: Color::srgb(1.0, 0.93, 0.55),
            aura_dark: Color::srgb(0.58, 0.58, 0.64),
            aura_thunder: Color::srgb(0.82, 0.72, 0.98),
            aura_wind: Color::srgb(0.74, 0.96, 0.90),
            status_burn: Color::srgb(1.0, 0.36, 0.30),
            status_scorch: Color::srgb(1.0, 0.62, 0.56),
            status_paralysis: Color::srgb(0.98, 0.86, 0.28),
            status_poison_like: Color::srgb(0.64, 0.48, 0.92),
            status_buff: Color::srgb(0.48, 0.92, 0.62),
            status_debuff: Color::srgb(1.0, 0.52, 0.66),
            status_special: Color::srgb(0.76, 0.84, 1.0),
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
            Color::srgba(0.0, 0.0, 0.0, 0.60),
            Val::Px(0.0),
            Val::Px(10.0),
            Val::Px(0.0),
            Val::Px(24.0),
        )
    }

    pub fn button_shadow(&self) -> BoxShadow {
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.40),
            Val::Px(0.0),
            Val::Px(4.0),
            Val::Px(0.0),
            Val::Px(12.0),
        )
    }

    pub fn card_shadow(&self) -> BoxShadow {
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.55),
            Val::Px(0.0),
            Val::Px(6.0),
            Val::Px(0.0),
            Val::Px(18.0),
        )
    }

    pub fn title_text_shadow(&self) -> TextShadow {
        TextShadow {
            offset: Vec2::new(1.5, 1.5),
            color: Color::srgba(0.0, 0.0, 0.0, 0.75),
        }
    }

    pub fn result_text_shadow(&self) -> TextShadow {
        TextShadow {
            offset: Vec2::new(3.0, 3.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.85),
        }
    }
}
