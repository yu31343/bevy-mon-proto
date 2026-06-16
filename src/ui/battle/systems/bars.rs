use bevy::prelude::*;

use crate::battle::{Combatant, ElementAura, EnemyTeam, InBattle, PlayerTeam, Shield, Stats};

use super::super::{components::*, theme::UiTheme};

const HP_ANIM_SECONDS: f32 = 0.38;
const HP_CHANGE_EPSILON: f32 = 0.05;

#[derive(Clone, Copy, Default)]
struct HpBarFx {
    initialized: bool,
    from_pct: f32,
    displayed_pct: f32,
    target_pct: f32,
    elapsed: f32,
    direction: HpChangeDirection,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum HpChangeDirection {
    #[default]
    None,
    Damage,
    Heal,
}

#[derive(Default)]
pub(crate) struct HpBarsFxState {
    player: HpBarFx,
    enemy: HpBarFx,
}

pub(crate) fn update_battle_bars_system(
    time: Res<Time>,
    mut hp_fx: Local<HpBarsFxState>,
    mut fills: ParamSet<(
        Query<(&mut Node, &mut BackgroundColor), With<PlayerHpBarFill>>,
        Query<(&mut Node, &mut BackgroundColor), With<EnemyHpBarFill>>,
        Query<&mut Node, With<PlayerShieldBarFill>>,
        Query<&mut Node, With<EnemyShieldBarFill>>,
    )>,
    mut shield_tracks: ParamSet<(
        Query<&mut Visibility, With<PlayerShieldBarTrack>>,
        Query<&mut Visibility, With<EnemyShieldBarTrack>>,
    )>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<(&Combatant, &Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
    theme: Res<UiTheme>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_hp_pct = super::super::helpers::active_hp_percent(&player_team.0, &combat_query);
    let enemy_hp_pct = super::super::helpers::active_hp_percent(&enemy_team.0, &combat_query);

    if let Ok((mut node, mut color)) = fills.p0().single_mut() {
        apply_hp_bar_fx(
            &mut node,
            &mut color,
            &mut hp_fx.player,
            player_hp_pct,
            theme.hp_fill_player,
            time.delta_secs(),
        );
    }
    if let Ok((mut node, mut color)) = fills.p1().single_mut() {
        apply_hp_bar_fx(
            &mut node,
            &mut color,
            &mut hp_fx.enemy,
            enemy_hp_pct,
            theme.hp_fill_enemy,
            time.delta_secs(),
        );
    }

    let player_shield_pct =
        super::super::helpers::active_shield_percent(&player_team.0, &combat_query);
    let enemy_shield_pct =
        super::super::helpers::active_shield_percent(&enemy_team.0, &combat_query);
    if let Ok(mut node) = fills.p2().single_mut() {
        node.width = Val::Percent(player_shield_pct);
    }
    if let Ok(mut node) = fills.p3().single_mut() {
        node.width = Val::Percent(enemy_shield_pct);
    }

    if let Ok(mut vis) = shield_tracks.p0().single_mut() {
        *vis = Visibility::Visible;
    }
    if let Ok(mut vis) = shield_tracks.p1().single_mut() {
        *vis = Visibility::Visible;
    }
}

fn apply_hp_bar_fx(
    node: &mut Node,
    color: &mut BackgroundColor,
    fx: &mut HpBarFx,
    target_pct: f32,
    base_color: Color,
    delta_secs: f32,
) {
    let target_pct = target_pct.clamp(0.0, 100.0);

    if !fx.initialized {
        fx.initialized = true;
        fx.from_pct = target_pct;
        fx.displayed_pct = target_pct;
        fx.target_pct = target_pct;
    }

    if (target_pct - fx.target_pct).abs() > HP_CHANGE_EPSILON {
        fx.from_pct = fx.displayed_pct;
        fx.target_pct = target_pct;
        fx.elapsed = 0.0;
        fx.direction = if target_pct < fx.from_pct {
            HpChangeDirection::Damage
        } else {
            HpChangeDirection::Heal
        };
    }

    if fx.direction != HpChangeDirection::None {
        fx.elapsed = (fx.elapsed + delta_secs).min(HP_ANIM_SECONDS);
        let progress = (fx.elapsed / HP_ANIM_SECONDS).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - progress).powi(3);
        fx.displayed_pct = fx.from_pct + (fx.target_pct - fx.from_pct) * eased;

        *color = BackgroundColor(match fx.direction {
            HpChangeDirection::Damage => Color::srgb(1.0, 0.28, 0.18),
            HpChangeDirection::Heal => Color::srgb(0.50, 1.0, 0.52),
            HpChangeDirection::None => base_color,
        });

        if progress >= 1.0 {
            fx.displayed_pct = fx.target_pct;
            fx.direction = HpChangeDirection::None;
            *color = BackgroundColor(base_color);
        }
    } else {
        fx.displayed_pct = target_pct;
        *color = BackgroundColor(base_color);
    }

    node.width = Val::Percent(fx.displayed_pct);
}
