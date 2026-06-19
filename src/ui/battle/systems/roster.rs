use bevy::prelude::*;

use crate::battle::{
    ElementAura, EnemyTeam, InBattle, PlayerTeam, Shield, Side, Stats, StatusBoard, UiControlSide,
};
use crate::data::BattleFormulaRules;

use super::super::components::*;
use super::super::theme::UiTheme;

const BENCH_SHIELD_BAR_WIDTH: f32 = 92.0;
const SWITCH_SHIELD_BAR_WIDTH: f32 = 220.0;

fn shield_pct(stats: &Stats, shield: &Shield) -> f32 {
    if stats.max_hp > 0 {
        ((shield.0.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

fn shield_value_text(value: i32) -> String {
    if value > 0 {
        value.to_string()
    } else {
        String::new()
    }
}

fn apply_bench_shield_track(node: &mut Node, visibility: &mut Visibility, pct: f32) {
    let has_shield = pct > 0.0;
    node.width = Val::Px(if has_shield {
        BENCH_SHIELD_BAR_WIDTH * pct / 100.0
    } else {
        0.0
    });
    node.display = if has_shield {
        Display::Flex
    } else {
        Display::None
    };
    *visibility = if has_shield {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

fn apply_switch_shield_track(node: &mut Node, visibility: &mut Visibility, pct: f32) {
    let has_shield = pct > 0.0;
    node.width = Val::Px(if has_shield {
        SWITCH_SHIELD_BAR_WIDTH * pct / 100.0
    } else {
        0.0
    });
    node.display = if has_shield {
        Display::Flex
    } else {
        Display::None
    };
    *visibility = if has_shield {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

fn effective_stat_value(
    stats: &Stats,
    stat: StatStageModifierKind,
    formula_rules: &BattleFormulaRules,
) -> i32 {
    match stat {
        StatStageModifierKind::Atk => {
            super::super::helpers::effective_atk_value(stats, formula_rules)
        }
        StatStageModifierKind::Def => {
            super::super::helpers::effective_def_value(stats, formula_rules)
        }
        StatStageModifierKind::Acc => {
            super::super::helpers::effective_acc_value(stats, formula_rules)
        }
        StatStageModifierKind::Spd => {
            super::super::helpers::effective_spd_value(stats, formula_rules)
        }
    }
}

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
            Option<&TeamMemberNameText>,
            Option<&TeamMemberHpValueText>,
            Option<&TeamMemberShieldValueText>,
            Option<&TeamMemberAtkText>,
            Option<&TeamMemberDefText>,
            Option<&TeamMemberAccText>,
            Option<&TeamMemberSpdText>,
            Option<&PlayerBenchNameText>,
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
        Query<
            (&PlayerBenchShieldBarTrack, &mut Node, &mut Visibility),
            Without<TeamMemberShieldBarTrack>,
        >,
        Query<(
            &PlayerBenchCard,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        )>,
        Query<
            (&TeamMemberShieldBarTrack, &mut Node, &mut Visibility),
            Without<PlayerBenchShieldBarTrack>,
        >,
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
        switch_name,
        switch_hp,
        switch_shield,
        switch_atk,
        switch_def,
        switch_acc,
        switch_spd,
        bench_name,
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

        if let Some(meta) = switch_name {
            if let Some(entity) = get_controlled_entity(meta.index) {
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

        if let Some(meta) = switch_hp {
            if let Some(entity) = get_controlled_entity(meta.index) {
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

        if let Some(meta) = switch_shield {
            if let Some(entity) = get_controlled_entity(meta.index) {
                if let Ok((_, _, shield, _, _)) = combat_query.get(entity) {
                    text.0 = shield_value_text(shield.0);
                } else {
                    text.0 = String::new();
                }
            } else {
                text.0 = String::new();
            }
            continue;
        }

        let switch_stat = switch_atk
            .map(|meta| (meta.index, StatStageModifierKind::Atk))
            .or_else(|| switch_def.map(|meta| (meta.index, StatStageModifierKind::Def)))
            .or_else(|| switch_acc.map(|meta| (meta.index, StatStageModifierKind::Acc)))
            .or_else(|| switch_spd.map(|meta| (meta.index, StatStageModifierKind::Spd)));
        if let Some((index, stat)) = switch_stat {
            if let Some(entity) = get_controlled_entity(index) {
                if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                    text.0 = effective_stat_value(stats, stat, &formula_rules).to_string();
                } else {
                    text.0 = "0".to_string();
                }
            } else {
                text.0 = "0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_name {
            if let Some(entity) = get_player_entity(meta.index) {
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
                    text.0 = shield_value_text(shield.0);
                } else {
                    text.0 = String::new();
                }
            } else {
                text.0 = String::new();
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
                node.width = Val::Percent(shield_pct(stats, shield));
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

    for (meta, mut node, mut visibility) in &mut nodes.p4() {
        let pct = get_player_entity(meta.index)
            .and_then(|entity| combat_query.get(entity).ok())
            .map(|(stats, _, shield, _, _)| shield_pct(stats, shield))
            .unwrap_or(0.0);
        apply_bench_shield_track(&mut node, &mut visibility, pct);
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
            Interaction::Hovered => BackgroundColor(Color::srgba(1.0, 0.82, 0.35, 0.08)),
            Interaction::Pressed => BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.18)),
            Interaction::None => BackgroundColor(Color::NONE),
        };
        *border = match *interaction {
            Interaction::Hovered => BorderColor::all(theme.gold_bright),
            Interaction::Pressed => BorderColor::all(theme.gold_dim),
            Interaction::None => BorderColor::all(theme.gold),
        };
    }

    for (meta, interaction, mut node, mut bg) in &mut nodes.p5() {
        let exists = get_player_entity(meta.index).is_some();
        let active = meta.index == player_team.active_index;
        node.display = if !exists || active {
            Display::None
        } else {
            Display::Flex
        };
        // 无边框：仅用半透明底色区分悬停/按下，去掉原本的“黑方框”。
        *bg = match *interaction {
            Interaction::Hovered => BackgroundColor(super::super::layout::BENCH_BG_HOVER),
            Interaction::Pressed => BackgroundColor(super::super::layout::BENCH_BG_PRESSED),
            Interaction::None => BackgroundColor(super::super::layout::BENCH_BG),
        };
    }

    for (meta, mut node, mut visibility) in &mut nodes.p6() {
        let pct = get_controlled_entity(meta.index)
            .and_then(|entity| combat_query.get(entity).ok())
            .map(|(stats, _, shield, _, _)| shield_pct(stats, shield))
            .unwrap_or(0.0);
        apply_switch_shield_track(&mut node, &mut visibility, pct);
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
        Query<
            (&EnemyBenchShieldBarTrack, &mut Node, &mut Visibility),
            Without<EnemyTeamMemberShieldBarTrack>,
        >,
        Query<(&EnemyBenchCard, &mut Node)>,
    )>,
    mut team_member_shield_tracks: Query<
        (&EnemyTeamMemberShieldBarTrack, &mut Visibility),
        Without<EnemyBenchShieldBarTrack>,
    >,
    theme: Res<UiTheme>,
) {
    let Some(enemy_team) = enemy_team else {
        return;
    };

    let get_entity = |index: usize| enemy_team.0.combatants.get(index).copied();

    for (mut text, overlay_name, overlay_aura, bench_name, bench_hp, bench_shield) in &mut text_q {
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
                    text.0 = shield_value_text(shield.0);
                } else {
                    text.0 = String::new();
                }
            } else {
                text.0 = String::new();
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
                node.width = Val::Percent(shield_pct(stats, shield));
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

    for (meta, mut node, mut visibility) in &mut nodes.p4() {
        let pct = get_entity(meta.index)
            .and_then(|entity| combat_query.get(entity).ok())
            .map(|(stats, _, shield, _, _)| shield_pct(stats, shield))
            .unwrap_or(0.0);
        apply_bench_shield_track(&mut node, &mut visibility, pct);
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

    for (meta, mut vis) in &mut team_member_shield_tracks {
        let has_shield = get_entity(meta.index)
            .and_then(|entity| combat_query.get(entity).ok())
            .map(|(_, _, shield, _, _)| shield.0 > 0)
            .unwrap_or(false);
        *vis = if has_shield {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
