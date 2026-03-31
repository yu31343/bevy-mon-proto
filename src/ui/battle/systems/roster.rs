use bevy::prelude::*;

use crate::battle::{EnemyTeam, ElementAura, InBattle, PlayerTeam, Shield, Stats};

use super::super::components::*;
use super::super::theme::UiTheme;

pub(crate) fn update_player_roster_ui_system(
    player_team: Option<Res<PlayerTeam>>,
    combat_query: Query<(&Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
    mut roster_text_q: Query<
        (
            &mut Text,
            Option<&TeamMemberButtonText>,
            Option<&TeamMemberAuraText>,
        ),
        (Without<ActionPointsText>, Without<PlayerCardNameText>),
    >,
    mut nodes: ParamSet<(
        Query<(&TeamMemberHpBarFill, &mut Node)>,
        Query<(&TeamMemberShieldBarFill, &mut Node)>,
        Query<(&TeamMemberButton, &Interaction, &mut Node, &mut BackgroundColor, &mut BorderColor)>,
    )>,
    mut shield_track_q: Query<(&TeamMemberShieldBarTrack, &mut Visibility)>,
    theme: Res<UiTheme>,
) {
    let Some(player_team) = player_team else {
        return;
    };

    let get_entity = |index: usize| player_team.0.combatants.get(index).copied();

    for (mut text, maybe_name, maybe_aura) in &mut roster_text_q {
        if let Some(meta) = maybe_name {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((stats, name, _, _)) = combat_query.get(entity) {
                    let dead = if stats.hp <= 0 { " (倒下)" } else { "" };
                    text.0 = format!("{}键：{}{}", meta.index + 5, name, dead);
                } else {
                    text.0 = format!("{}键：队伍{}", meta.index + 5, meta.index + 1);
                }
            } else {
                text.0 = format!("{}键：队伍{}", meta.index + 5, meta.index + 1);
            }
            continue;
        }
        if let Some(meta) = maybe_aura {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((_, _, _, aura)) = combat_query.get(entity) {
                    text.0 = format!(
                        "附着: {}",
                        super::super::helpers::aura_label(aura.attached)
                    );
                } else {
                    text.0 = "附着: 无".to_string();
                }
            } else {
                text.0 = "附着: 无".to_string();
            }
            continue;
        }
    }

    for (meta, mut node) in &mut nodes.p0() {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((stats, _, _, _)) = combat_query.get(entity) {
                let hp_pct = if stats.max_hp > 0 {
                    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(hp_pct);
            }
        }
    }

    for (meta, mut node) in &mut nodes.p1() {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((stats, _, shield, _)) = combat_query.get(entity) {
                let shield_pct = if stats.max_hp > 0 {
                    ((shield.0.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(shield_pct);
            }
        }
    }

    for (meta, interaction, mut node, mut bg, mut border) in &mut nodes.p2() {
        let active = meta.index == player_team.0.active_index;
        node.display = if active { Display::None } else { Display::Flex };
        node.min_height = Val::Px(58.0);
        *bg = match *interaction {
            Interaction::Hovered => BackgroundColor(theme.button_hover),
            Interaction::Pressed => BackgroundColor(theme.button_pressed),
            Interaction::None => BackgroundColor(theme.button_idle),
        };
        *border = match *interaction {
            Interaction::Hovered => BorderColor::all(theme.button_border_hover),
            Interaction::Pressed => BorderColor::all(theme.button_border_pressed),
            Interaction::None => BorderColor::all(theme.button_border_idle),
        };
    }

    for (meta, mut vis) in &mut shield_track_q {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((_, _, shield, _)) = combat_query.get(entity) {
                *vis = if shield.0 > 0 {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

pub(crate) fn update_enemy_roster_ui_system(
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<(&Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
    mut roster_text_q: Query<
        (
            &mut Text,
            Option<&EnemyTeamMemberButtonText>,
            Option<&EnemyTeamMemberAuraText>,
        ),
        (Without<ActionPointsText>, Without<PlayerCardNameText>),
    >,
    mut nodes: ParamSet<(
        Query<(&EnemyTeamMemberHpBarFill, &mut Node)>,
        Query<(&EnemyTeamMemberShieldBarFill, &mut Node)>,
        Query<(
            &EnemyTeamMemberButton,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        )>,
    )>,
    mut shield_track_q: Query<(&EnemyTeamMemberShieldBarTrack, &mut Visibility)>,
    theme: Res<UiTheme>,
) {
    let Some(enemy_team) = enemy_team else {
        return;
    };

    let get_entity = |index: usize| enemy_team.0.combatants.get(index).copied();

    for (mut text, maybe_name, maybe_aura) in &mut roster_text_q {
        if let Some(meta) = maybe_name {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((stats, name, _, _)) = combat_query.get(entity) {
                    let dead = if stats.hp <= 0 { " (倒下)" } else { "" };
                    text.0 = format!("{}{}", name, dead);
                } else {
                    text.0 = format!("队伍{}", meta.index + 1);
                }
            } else {
                text.0 = format!("队伍{}", meta.index + 1);
            }
            continue;
        }
        if let Some(meta) = maybe_aura {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((_, _, _, aura)) = combat_query.get(entity) {
                    text.0 = format!(
                        "附着: {}",
                        super::super::helpers::aura_label(aura.attached)
                    );
                } else {
                    text.0 = "附着: 无".to_string();
                }
            } else {
                text.0 = "附着: 无".to_string();
            }
            continue;
        }
    }

    for (meta, mut node) in &mut nodes.p0() {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((stats, _, _, _)) = combat_query.get(entity) {
                let hp_pct = if stats.max_hp > 0 {
                    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(hp_pct);
            }
        }
    }

    for (meta, mut node) in &mut nodes.p1() {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((stats, _, shield, _)) = combat_query.get(entity) {
                let shield_pct = if stats.max_hp > 0 {
                    ((shield.0.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(shield_pct);
            }
        }
    }

    for (meta, mut node, mut bg, mut border) in &mut nodes.p2() {
        let active = meta.index == enemy_team.0.active_index;
        node.display = if active { Display::None } else { Display::Flex };
        node.min_height = Val::Px(58.0);
        if active {
            *bg = BackgroundColor(theme.button_hover);
            *border = BorderColor::all(theme.button_border_hover);
        } else {
            *bg = BackgroundColor(theme.enemy_card_bg);
            *border = BorderColor::all(theme.enemy_card_border);
        }
    }

    for (meta, mut vis) in &mut shield_track_q {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((_, _, shield, _)) = combat_query.get(entity) {
                *vis = if shield.0 > 0 {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

