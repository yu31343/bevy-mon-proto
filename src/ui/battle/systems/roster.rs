use bevy::prelude::*;

use crate::{
    battle::{
        ElementAura, EnemyTeam, InBattle, PlayerTeam, Shield, Side, Stats, StatusBoard,
        UiControlSide,
    },
    data::BattleFormulaRules,
};

use super::super::components::*;
use super::super::theme::UiTheme;

pub(crate) fn update_player_roster_ui_system(
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    ui_control_side: Res<UiControlSide>,
    combat_query: Query<(&Stats, &Name, &Shield, &ElementAura, &StatusBoard), With<InBattle>>,
    mut text_q: Query<
        (
            &mut Text,
            Option<&TeamMemberButtonText>,
            Option<&TeamMemberAuraText>,
            Option<&PlayerBenchNameText>,
            Option<&PlayerBenchStatsText>,
            Option<&PlayerBenchAuraText>,
            Option<&PlayerBenchHpValueText>,
            Option<&PlayerBenchShieldValueText>,
        ),
        (Without<ActionPointsText>, Without<PlayerCardNameText>),
    >,
    mut nodes: ParamSet<(
        Query<(&TeamMemberHpBarFill, &mut Node)>,
        Query<(&TeamMemberShieldBarFill, &mut Node)>,
        Query<(
            &TeamMemberButton,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        )>,
        Query<(&PlayerBenchHpBarFill, &mut Node)>,
        Query<(&PlayerBenchShieldBarFill, &mut Node)>,
        Query<(
            &PlayerBenchCard,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        )>,
    )>,
    mut visibilities: ParamSet<(
        Query<(&TeamMemberShieldBarTrack, &mut Visibility)>,
        Query<(&PlayerBenchShieldBarTrack, &mut Visibility)>,
    )>,
    theme: Res<UiTheme>,
    formula_rules: Res<BattleFormulaRules>,
) {
    let player_team = player_team.map(|team| team.0.clone());
    let enemy_team = enemy_team.map(|team| team.0.clone());
    let controlled_team = match ui_control_side.0 {
        Side::Player => player_team.clone(),
        Side::Enemy => enemy_team,
    };
    let Some(controlled_team) = controlled_team else {
        return;
    };
    let Some(player_team) = player_team else {
        return;
    };

    let get_controlled_entity = |index: usize| controlled_team.combatants.get(index).copied();
    let get_player_entity = |index: usize| player_team.combatants.get(index).copied();

    for (
        mut text,
        overlay_name,
        overlay_aura,
        bench_name,
        bench_stats,
        bench_aura,
        bench_hp,
        bench_shield,
    ) in &mut text_q
    {
        if let Some(meta) = overlay_name {
            if let Some(entity) = get_controlled_entity(meta.index) {
                if let Ok((stats, name, _, aura, _)) = combat_query.get(entity) {
                    let dead = if stats.hp <= 0 { " (倒下)" } else { "" };
                    let aura_text = super::super::helpers::compact_aura_label(&aura.elements());
                    text.0 = if aura_text.is_empty() {
                        format!("{}键：{}{}", meta.index + 5, name, dead)
                    } else {
                        format!("{}键：{} {}{}", meta.index + 5, name, aura_text, dead)
                    };
                } else {
                    text.0 = format!("{}键：队伍{}", meta.index + 5, meta.index + 1);
                }
            } else {
                text.0 = format!("{}键：队伍{}", meta.index + 5, meta.index + 1);
            }
            continue;
        }

        if let Some(meta) = overlay_aura {
            if let Some(entity) = get_controlled_entity(meta.index) {
                if let Ok((stats, _, _, _, statuses)) = combat_query.get(entity) {
                    text.0 = super::super::helpers::status_stage_label(stats, statuses);
                } else {
                    text.0 = "状态: 无 | 阶段: Atk+0 Def+0 Spd+0 Acc+0".to_string();
                }
            } else {
                text.0 = "状态: 无 | 阶段: Atk+0 Def+0 Spd+0 Acc+0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_name {
            if let Some(entity) = get_player_entity(meta.index) {
                if let Ok((stats, name, _, aura, _)) = combat_query.get(entity) {
                    let dead = if stats.hp <= 0 { " (倒下)" } else { "" };
                    let aura_text = super::super::helpers::compact_aura_label(&aura.elements());
                    text.0 = if aura_text.is_empty() {
                        format!("{}{}", name, dead)
                    } else {
                        format!("{} {}{}", name, aura_text, dead)
                    };
                } else {
                    text.0 = format!("队伍{}", meta.index + 1);
                }
            } else {
                text.0 = format!("队伍{}", meta.index + 1);
            }
            continue;
        }

        if let Some(meta) = bench_stats {
            if let Some(entity) = get_player_entity(meta.index) {
                if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                    text.0 = format!(
                        "Atk: {}{} Def: {}{} Acc: {}{} Spd: {}{}",
                        super::super::helpers::stage_prefix(stats.atk_stage),
                        super::super::helpers::effective_atk_value(stats, &formula_rules),
                        super::super::helpers::stage_prefix(stats.def_stage),
                        super::super::helpers::effective_def_value(stats, &formula_rules),
                        super::super::helpers::stage_prefix(stats.acc_stage),
                        super::super::helpers::effective_acc_value(stats, &formula_rules),
                        super::super::helpers::stage_prefix(stats.spd_stage),
                        super::super::helpers::effective_spd_value(stats, &formula_rules),
                    );
                } else {
                    text.0 = "Atk: 0 Def: 0 Acc: 0 Spd: 0".to_string();
                }
            } else {
                text.0 = "Atk: 0 Def: 0 Acc: 0 Spd: 0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_aura {
            if let Some(entity) = get_player_entity(meta.index) {
                if let Ok((_, _, _, _, statuses)) = combat_query.get(entity) {
                    text.0 = format!("状态：{}", super::super::helpers::status_label(statuses));
                } else {
                    text.0 = "状态：无".to_string();
                }
            } else {
                text.0 = "状态：无".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_hp {
            if let Some(entity) = get_player_entity(meta.index) {
                if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                    text.0 = format!("{}/{}", stats.hp.max(0), stats.max_hp);
                } else {
                    text.0 = "0/0".to_string();
                }
            } else {
                text.0 = "0/0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_shield {
            if let Some(entity) = get_player_entity(meta.index) {
                if let Ok((_, _, shield, _, _)) = combat_query.get(entity) {
                    text.0 = shield.0.max(0).to_string();
                } else {
                    text.0 = "0".to_string();
                }
            } else {
                text.0 = "0".to_string();
            }
        }
    }

    for (meta, mut node) in &mut nodes.p0() {
        if let Some(entity) = get_controlled_entity(meta.index) {
            if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
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
        if let Some(entity) = get_controlled_entity(meta.index) {
            if let Ok((stats, _, shield, _, _)) = combat_query.get(entity) {
                let shield_pct = if stats.max_hp > 0 {
                    ((shield.0.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(shield_pct);
            }
        }
    }

    for (meta, mut node) in &mut nodes.p3() {
        if let Some(entity) = get_player_entity(meta.index) {
            if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                let hp_pct = if stats.max_hp > 0 {
                    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(hp_pct);
            }
        }
    }

    for (meta, mut node) in &mut nodes.p4() {
        if let Some(entity) = get_player_entity(meta.index) {
            if let Ok((stats, _, shield, _, _)) = combat_query.get(entity) {
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
        let exists = get_controlled_entity(meta.index).is_some();
        let active = meta.index == controlled_team.active_index;
        node.display = if !exists || active {
            Display::None
        } else {
            Display::Flex
        };
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

    for (meta, interaction, mut node, mut bg, mut border) in &mut nodes.p5() {
        let exists = get_player_entity(meta.index).is_some();
        let active = meta.index == player_team.active_index;
        node.display = if !exists || active {
            Display::None
        } else {
            Display::Flex
        };
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

    for (meta, mut vis) in &mut visibilities.p0() {
        *vis = if get_controlled_entity(meta.index).is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (meta, mut vis) in &mut visibilities.p1() {
        *vis = if get_player_entity(meta.index).is_some() && meta.index != player_team.active_index
        {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(crate) fn update_enemy_roster_ui_system(
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<(&Stats, &Name, &Shield, &ElementAura, &StatusBoard), With<InBattle>>,
    mut text_q: Query<
        (
            &mut Text,
            Option<&EnemyTeamMemberButtonText>,
            Option<&EnemyTeamMemberAuraText>,
            Option<&EnemyBenchNameText>,
            Option<&EnemyBenchStatsText>,
            Option<&EnemyBenchAuraText>,
            Option<&EnemyBenchHpValueText>,
            Option<&EnemyBenchShieldValueText>,
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
        Query<(&EnemyBenchHpBarFill, &mut Node)>,
        Query<(&EnemyBenchShieldBarFill, &mut Node)>,
        Query<(&EnemyBenchCard, &mut Node)>,
    )>,
    mut visibilities: ParamSet<(
        Query<(&EnemyTeamMemberShieldBarTrack, &mut Visibility)>,
        Query<(&EnemyBenchShieldBarTrack, &mut Visibility)>,
    )>,
    theme: Res<UiTheme>,
    formula_rules: Res<BattleFormulaRules>,
) {
    let Some(enemy_team) = enemy_team else {
        return;
    };

    let get_entity = |index: usize| enemy_team.0.combatants.get(index).copied();

    for (
        mut text,
        overlay_name,
        overlay_aura,
        bench_name,
        bench_stats,
        bench_aura,
        bench_hp,
        bench_shield,
    ) in &mut text_q
    {
        if let Some(meta) = overlay_name {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((stats, name, _, _, _)) = combat_query.get(entity) {
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

        if let Some(meta) = overlay_aura {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((stats, _, _, aura, statuses)) = combat_query.get(entity) {
                    text.0 = super::super::helpers::aura_status_stage_label(
                        stats,
                        &aura.elements(),
                        statuses,
                    );
                } else {
                    text.0 = "附着: 无 | 状态: 无 | 阶段: Atk+0 Def+0 Spd+0 Acc+0".to_string();
                }
            } else {
                text.0 = "附着: 无 | 状态: 无 | 阶段: Atk+0 Def+0 Spd+0 Acc+0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_name {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((stats, name, _, aura, _)) = combat_query.get(entity) {
                    let dead = if stats.hp <= 0 { " (倒下)" } else { "" };
                    let aura_text = super::super::helpers::compact_aura_label(&aura.elements());
                    text.0 = if aura_text.is_empty() {
                        format!("{}{}", name, dead)
                    } else {
                        format!("{} {}{}", name, aura_text, dead)
                    };
                } else {
                    text.0 = format!("队伍{}", meta.index + 1);
                }
            } else {
                text.0 = format!("队伍{}", meta.index + 1);
            }
            continue;
        }

        if let Some(meta) = bench_stats {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                    text.0 = format!(
                        "Atk: {}{} Def: {}{} Acc: {}{} Spd: {}{}",
                        super::super::helpers::stage_prefix(stats.atk_stage),
                        super::super::helpers::effective_atk_value(stats, &formula_rules),
                        super::super::helpers::stage_prefix(stats.def_stage),
                        super::super::helpers::effective_def_value(stats, &formula_rules),
                        super::super::helpers::stage_prefix(stats.acc_stage),
                        super::super::helpers::effective_acc_value(stats, &formula_rules),
                        super::super::helpers::stage_prefix(stats.spd_stage),
                        super::super::helpers::effective_spd_value(stats, &formula_rules),
                    );
                } else {
                    text.0 = "Atk: 0 Def: 0 Acc: 0 Spd: 0".to_string();
                }
            } else {
                text.0 = "Atk: 0 Def: 0 Acc: 0 Spd: 0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_aura {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((_, _, _, _, statuses)) = combat_query.get(entity) {
                    text.0 = format!("状态：{}", super::super::helpers::status_label(statuses));
                } else {
                    text.0 = "状态：无".to_string();
                }
            } else {
                text.0 = "状态：无".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_hp {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                    text.0 = format!("{}/{}", stats.hp.max(0), stats.max_hp);
                } else {
                    text.0 = "0/0".to_string();
                }
            } else {
                text.0 = "0/0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_shield {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((_, _, shield, _, _)) = combat_query.get(entity) {
                    text.0 = shield.0.max(0).to_string();
                } else {
                    text.0 = "0".to_string();
                }
            } else {
                text.0 = "0".to_string();
            }
        }
    }

    for (meta, mut node) in &mut nodes.p0() {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
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
            if let Ok((stats, _, shield, _, _)) = combat_query.get(entity) {
                let shield_pct = if stats.max_hp > 0 {
                    ((shield.0.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(shield_pct);
            }
        }
    }

    for (meta, mut node) in &mut nodes.p3() {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                let hp_pct = if stats.max_hp > 0 {
                    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(hp_pct);
            }
        }
    }

    for (meta, mut node) in &mut nodes.p4() {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((stats, _, shield, _, _)) = combat_query.get(entity) {
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
        let exists = get_entity(meta.index).is_some();
        let active = meta.index == enemy_team.0.active_index;
        node.display = if !exists || active {
            Display::None
        } else {
            Display::Flex
        };
        node.min_height = Val::Px(58.0);
        if active {
            *bg = BackgroundColor(theme.button_hover);
            *border = BorderColor::all(theme.button_border_hover);
        } else {
            *bg = BackgroundColor(theme.enemy_card_bg);
            *border = BorderColor::all(theme.enemy_card_border);
        }
    }

    for (meta, mut node) in &mut nodes.p5() {
        let exists = get_entity(meta.index).is_some();
        let active = meta.index == enemy_team.0.active_index;
        node.display = if !exists || active {
            Display::None
        } else {
            Display::Flex
        };
    }

    for (meta, mut vis) in &mut visibilities.p0() {
        *vis = if get_entity(meta.index).is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (meta, mut vis) in &mut visibilities.p1() {
        *vis = if get_entity(meta.index).is_some() && meta.index != enemy_team.0.active_index {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
