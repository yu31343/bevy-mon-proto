use bevy::prelude::*;

use crate::battle::{Combatant, EnemyTeam, ElementAura, InBattle, PlayerTeam, Shield, Stats};

use super::super::components::*;

pub(crate) fn update_battle_bars_system(
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
    combat_query: Query<(&Combatant, &Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_hp_pct = crate::ui::active_hp_percent(&player_team.0, &combat_query);
    let enemy_hp_pct = crate::ui::active_hp_percent(&enemy_team.0, &combat_query);

    if let Ok(mut node) = fills.p0().single_mut() {
        node.width = Val::Percent(player_hp_pct);
    }
    if let Ok(mut node) = fills.p1().single_mut() {
        node.width = Val::Percent(enemy_hp_pct);
    }

    let player_shield_pct = crate::ui::active_shield_percent(&player_team.0, &combat_query);
    let enemy_shield_pct = crate::ui::active_shield_percent(&enemy_team.0, &combat_query);

    if let Ok(mut node) = fills.p2().single_mut() {
        node.width = Val::Percent(player_shield_pct);
    }
    if let Ok(mut node) = fills.p3().single_mut() {
        node.width = Val::Percent(enemy_shield_pct);
    }

    let player_has_shield = crate::ui::active_shield(&player_team.0, &combat_query) > 0;
    let enemy_has_shield = crate::ui::active_shield(&enemy_team.0, &combat_query) > 0;

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

