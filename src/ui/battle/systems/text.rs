use bevy::prelude::*;

use super::super::{components::*, resources::UiFontHandle, theme::UiTheme};
use crate::{
    battle::{
        BattleEvent, Combatant, ElementAura, EnemyTeam, InBattle, PlayerTeam, Shield, SkillCount,
        SkillList, Stats, StatusBoard, Team,
    },
    data::{BattleDbs, BattleFormulaRules, ElementType, StatusCategory},
    game_state::BattlePhase,
};

fn element_icon_path(element: ElementType) -> &'static str {
    match element {
        ElementType::Fire => "images/icons/elements/fire.png",
        ElementType::Water => "images/icons/elements/water.png",
        ElementType::Grass => "images/icons/elements/grass.png",
        ElementType::Light => "images/icons/elements/light.png",
        ElementType::Dark => "images/icons/elements/dark.png",
        ElementType::Thunder => "images/icons/elements/thunder.png",
        ElementType::Wind => "images/icons/elements/wind.png",
    }
}

type ActiveCombatantRef<'a> = (
    &'a Combatant,
    &'a Stats,
    &'a Name,
    &'a Shield,
    &'a ElementAura,
    &'a StatusBoard,
);

fn active_combatant_data<'a>(
    team: &Team,
    query: &'a Query<
        (
            &Combatant,
            &Stats,
            &Name,
            &Shield,
            &ElementAura,
            &StatusBoard,
        ),
        With<InBattle>,
    >,
) -> Option<ActiveCombatantRef<'a>> {
    let entity = team.active_combatant()?;
    query.get(entity).ok()
}

fn active_summary(active: Option<ActiveCombatantRef<'_>>) -> String {
    let Some((combatant, _, _, _, _, _)) = active else {
        return "无".to_string();
    };
    super::super::helpers::element_name(combatant.element).to_string()
}

fn stat_value(stage: i32, current: i32) -> String {
    format!("{}{}", super::super::helpers::stage_prefix(stage), current)
}

pub(crate) fn update_phase_text_system(
    battle_phase: Res<State<BattlePhase>>,
    mut text_q: Query<&mut Text, With<BattlePhaseText>>,
) {
    if let Ok(mut text) = text_q.single_mut() {
        text.0 = format!(
            "战斗阶段：{}",
            super::super::helpers::phase_label(*battle_phase.get())
        );
    }
}

pub(crate) fn update_element_icon_system(
    asset_server: Res<AssetServer>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<(&Combatant,), With<InBattle>>,
    mut icons: Query<(
        &mut ImageNode,
        Option<&PlayerElementIcon>,
        Option<&EnemyElementIcon>,
    )>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_element = player_team
        .0
        .active_combatant()
        .and_then(|entity| combat_query.get(entity).ok())
        .map(|(combatant,)| combatant.element);
    let enemy_element = enemy_team
        .0
        .active_combatant()
        .and_then(|entity| combat_query.get(entity).ok())
        .map(|(combatant,)| combatant.element);

    for (mut image, is_player, is_enemy) in &mut icons {
        let element = if is_player.is_some() {
            player_element
        } else if is_enemy.is_some() {
            enemy_element
        } else {
            None
        };

        if let Some(element) = element {
            image.image = asset_server.load(element_icon_path(element));
            image.color = Color::WHITE;
        } else {
            image.color = Color::NONE;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_active_panel_text_system(
    mut text_sets: ParamSet<(
        Query<
            (
                &mut Text,
                Option<&PlayerStatsText>,
                Option<&EnemyStatsText>,
                Option<&PlayerNameText>,
                Option<&EnemyNameText>,
            ),
            Without<BattlePhaseText>,
        >,
        Query<
            (
                &mut Text,
                Option<&PlayerHpValueText>,
                Option<&PlayerHpStatText>,
                Option<&EnemyHpValueText>,
                Option<&EnemyHpStatText>,
                Option<&PlayerShieldValueText>,
                Option<&EnemyShieldValueText>,
                Option<&PlayerAtkText>,
                Option<&EnemyAtkText>,
                Option<&PlayerDefText>,
                Option<&EnemyDefText>,
                Option<&PlayerAccText>,
                Option<&EnemyAccText>,
                Option<&PlayerSpdText>,
                Option<&EnemySpdText>,
            ),
            Without<BattlePhaseText>,
        >,
    )>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<
        (
            &Combatant,
            &Stats,
            &Name,
            &Shield,
            &ElementAura,
            &StatusBoard,
        ),
        With<InBattle>,
    >,
    formula_rules: Res<BattleFormulaRules>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_active = active_combatant_data(&player_team.0, &combat_query);
    let enemy_active = active_combatant_data(&enemy_team.0, &combat_query);
    let player_summary = active_summary(player_active);
    let enemy_summary = active_summary(enemy_active);

    for (mut text, is_player_summary, is_enemy_summary, is_player_name, is_enemy_name) in
        &mut text_sets.p0()
    {
        if is_player_name.is_some() {
            text.0 = if let Some((_, _, name, _, _, _)) = player_active {
                format!("我方：{}", name)
            } else {
                "我方：无在场精灵".to_string()
            };
            continue;
        }
        if is_enemy_name.is_some() {
            text.0 = if let Some((_, _, name, _, _, _)) = enemy_active {
                format!("敌方：{}", name)
            } else {
                "敌方：无在场精灵".to_string()
            };
            continue;
        }
        if is_player_summary.is_some() {
            text.0 = player_summary.clone();
            continue;
        }
        if is_enemy_summary.is_some() {
            text.0 = enemy_summary.clone();
        }
    }

    for (
        mut text,
        is_player_hp,
        is_player_hp_stat,
        is_enemy_hp,
        is_enemy_hp_stat,
        is_player_shield,
        is_enemy_shield,
        is_player_atk,
        is_enemy_atk,
        is_player_def,
        is_enemy_def,
        is_player_acc,
        is_enemy_acc,
        is_player_spd,
        is_enemy_spd,
    ) in &mut text_sets.p1()
    {
        if is_player_hp.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                format!("{}/{}", stats.hp.max(0), stats.max_hp)
            } else {
                "0/0".to_string()
            };
            continue;
        }
        if is_player_hp_stat.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                stats.max_hp.to_string()
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_hp.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                format!("{}/{}", stats.hp.max(0), stats.max_hp)
            } else {
                "0/0".to_string()
            };
            continue;
        }
        if is_enemy_hp_stat.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                stats.max_hp.to_string()
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_player_shield.is_some() {
            text.0 = if let Some((_, _, _, shield, _, _)) = player_active {
                shield.0.max(0).to_string()
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_shield.is_some() {
            text.0 = if let Some((_, _, _, shield, _, _)) = enemy_active {
                shield.0.max(0).to_string()
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_player_atk.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                stat_value(
                    stats.atk_stage,
                    super::super::helpers::effective_atk_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_atk.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                stat_value(
                    stats.atk_stage,
                    super::super::helpers::effective_atk_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_player_def.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                stat_value(
                    stats.def_stage,
                    super::super::helpers::effective_def_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_def.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                stat_value(
                    stats.def_stage,
                    super::super::helpers::effective_def_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_player_acc.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                stat_value(
                    stats.acc_stage,
                    super::super::helpers::effective_acc_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_acc.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                stat_value(
                    stats.acc_stage,
                    super::super::helpers::effective_acc_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_player_spd.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                stat_value(
                    stats.spd_stage,
                    super::super::helpers::effective_spd_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_spd.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                stat_value(
                    stats.spd_stage,
                    super::super::helpers::effective_spd_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_active_panel_tokens_system(
    mut commands: Commands,
    aura_lines: Query<
        (
            Entity,
            Option<&Children>,
            Option<&PlayerAuraLine>,
            Option<&EnemyAuraLine>,
        ),
        Or<(With<PlayerAuraLine>, With<EnemyAuraLine>)>,
    >,
    status_lines: Query<
        (
            Entity,
            Option<&Children>,
            Option<&PlayerStatusLine>,
            Option<&EnemyStatusLine>,
        ),
        Or<(With<PlayerStatusLine>, With<EnemyStatusLine>)>,
    >,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<
        (
            &Combatant,
            &Stats,
            &Name,
            &Shield,
            &ElementAura,
            &StatusBoard,
        ),
        With<InBattle>,
    >,
    theme: Res<UiTheme>,
    ui_font: Option<Res<UiFontHandle>>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_active = active_combatant_data(&player_team.0, &combat_query);
    let enemy_active = active_combatant_data(&enemy_team.0, &combat_query);
    let info_font = super::super::helpers::make_text_font(13.0, ui_font.as_deref());

    for (entity, children, is_player_aura, is_enemy_aura) in &aura_lines {
        let active = if is_player_aura.is_some() {
            player_active
        } else if is_enemy_aura.is_some() {
            enemy_active
        } else {
            None
        };
        let items = if let Some((_, _, _, _, aura, _)) = active {
            aura.elements()
                .iter()
                .map(|element| {
                    (
                        super::super::helpers::element_name(*element).to_string(),
                        super::super::helpers::element_color(*element, &theme),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        super::super::helpers::replace_debug_tokens(
            &mut commands,
            entity,
            children,
            &info_font,
            &items,
            DebugAuraToken,
        );
    }

    for (entity, children, is_player_status, is_enemy_status) in &status_lines {
        let active = if is_player_status.is_some() {
            player_active
        } else if is_enemy_status.is_some() {
            enemy_active
        } else {
            None
        };
        let items = if let Some((_, _, _, _, _, statuses)) = active {
            let labels: Vec<_> = statuses
                .entries
                .iter()
                .filter(|entry| entry.category != StatusCategory::Aura)
                .map(|entry| {
                    (
                        entry.name.clone(),
                        super::super::helpers::status_color(entry, &theme),
                    )
                })
                .collect();
            if labels.is_empty() {
                vec![("无".to_string(), theme.text_muted)]
            } else {
                labels
            }
        } else {
            vec![("无".to_string(), theme.text_muted)]
        };
        super::super::helpers::replace_debug_tokens(
            &mut commands,
            entity,
            children,
            &info_font,
            &items,
            DebugStatusToken,
        );
    }
}

pub(crate) fn update_skill_text_system(
    battle_phase: Res<State<BattlePhase>>,
    ui_control_side: Res<crate::battle::UiControlSide>,
    mut text_q: Query<
        (
            &mut Text,
            Option<&mut TextColor>,
            Option<&mut TextShadow>,
            Option<&TurnBannerText>,
            Option<&SkillButtonText>,
            Option<&SkillButtonMetaText>,
            Option<&SkillButtonIconText>,
            Option<&EnemySkillText>,
            Option<&EnemySkillMetaText>,
            Option<&EnemySkillIconText>,
        ),
        (
            Without<ActionPointsText>,
            Without<ResultText>,
            Without<BattleHintText>,
            Without<TeamMemberButtonText>,
            Without<TeamMemberAuraText>,
            Without<PlayerCardHotkeyText>,
            Without<PlayerCardNameText>,
            Without<PlayerCardCostText>,
            Without<PlayerCardDescText>,
            Without<BattleActionText>,
        ),
    >,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    skill_query: Query<(&SkillList, &SkillCount), With<InBattle>>,
    skill_db: Res<BattleDbs>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let mut player_skills = None;
    if let Some(entity) = player_team.0.active_combatant() {
        if let Ok((skills, count)) = skill_query.get(entity) {
            player_skills = Some((skills.0, count.0));
        }
    }

    let mut enemy_skills = None;
    if let Some(entity) = enemy_team.0.active_combatant() {
        if let Ok((skills, count)) = skill_query.get(entity) {
            enemy_skills = Some((skills.0, count.0));
        }
    }

    for (
        mut text,
        text_color,
        text_shadow,
        is_turn_banner,
        skill_button_text,
        skill_button_meta_text,
        skill_icon_text,
        enemy_skill_text,
        enemy_skill_meta,
        enemy_skill_icon,
    ) in &mut text_q
    {
        if is_turn_banner.is_some() {
            let (banner_text, outline_color) = match *battle_phase.get() {
                BattlePhase::PlayerTurn => ("你的回合", Color::srgba(0.05, 0.24, 1.0, 0.95)),
                BattlePhase::EnemyTurn => ("敌方回合", Color::srgba(1.0, 0.05, 0.04, 0.95)),
                BattlePhase::Discard => ("弃牌阶段", Color::srgba(0.05, 0.24, 1.0, 0.95)),
                _ => ("", Color::srgba(0.0, 0.0, 0.0, 0.0)),
            };
            text.0 = banner_text.to_string();
            if let Some(mut text_color) = text_color {
                text_color.0 = Color::WHITE;
            }
            if let Some(mut text_shadow) = text_shadow {
                text_shadow.offset = Vec2::new(3.0, 3.0);
                text_shadow.color = outline_color;
            }
            continue;
        }
        let control_skills = match ui_control_side.0 {
            crate::battle::Side::Player => player_skills,
            crate::battle::Side::Enemy => enemy_skills,
        };
        if let (Some(button), Some((skills, count))) = (skill_button_text, control_skills) {
            if button.index >= count {
                text.0 = format!("{}号: 未配置", button.index + 1);
            } else {
                let skill_id = skills[button.index];
                text.0 = format!(
                    "{}号: {}",
                    button.index + 1,
                    super::super::helpers::skill_name(skill_id, &skill_db)
                );
            }
            continue;
        }
        if let (Some(meta), Some((skills, count))) = (skill_button_meta_text, control_skills) {
            if meta.index >= count {
                text.0 = "AP消耗：--".to_string();
            } else {
                let skill_id = skills[meta.index];
                text.0 = format!(
                    "{}\nAP消耗：{}\n{}",
                    super::super::helpers::skill_meta(skill_id, &skill_db),
                    super::super::helpers::monster_skill_ap_cost_ui(skill_id, &skill_db),
                    super::super::helpers::skill_summary(skill_id, &skill_db)
                );
            }
            continue;
        }
        if let (Some(icon), Some((_skills, count))) = (skill_icon_text, control_skills) {
            text.0 = if icon.index >= count {
                "-".to_string()
            } else {
                (icon.index + 1).to_string()
            };
            continue;
        }
        if let (Some(button), Some((skills, count))) = (enemy_skill_text, enemy_skills) {
            if button.index >= count {
                text.0 = format!("{}号: 未配置", button.index + 1);
            } else {
                let skill_id = skills[button.index];
                text.0 = format!(
                    "{}号: {}",
                    button.index + 1,
                    super::super::helpers::skill_name(skill_id, &skill_db)
                );
            }
            continue;
        }
        if let (Some(meta), Some((skills, count))) = (enemy_skill_meta, enemy_skills) {
            if meta.index >= count {
                text.0 = "类型：--".to_string();
            } else {
                let skill_id = skills[meta.index];
                text.0 = super::super::helpers::skill_meta(skill_id, &skill_db);
            }
            continue;
        }
        if let (Some(icon), Some((_skills, count))) = (enemy_skill_icon, enemy_skills) {
            text.0 = if icon.index >= count {
                "-".to_string()
            } else {
                (icon.index + 1).to_string()
            };
        }
    }
}

pub(crate) fn update_action_points_text_system(
    action_points: Res<crate::battle::ActionPoints>,
    mut text_q: Query<&mut Text, With<ActionPointsText>>,
) {
    if let Ok(mut text) = text_q.single_mut() {
        text.0 = format!(
            "AP：Player {} / Enemy {}",
            action_points.player, action_points.enemy
        );
    }
}

pub(crate) fn update_battle_action_text_system(
    mut events: MessageReader<BattleEvent>,
    mut text_q: Query<&mut Text, With<BattleActionText>>,
) {
    let Ok(mut text) = text_q.single_mut() else {
        return;
    };

    for event in events.read() {
        let line = match event {
            BattleEvent::SkillUsed {
                side, skill_name, ..
            } => {
                let owner = if *side == crate::battle::Side::Player {
                    "我方"
                } else {
                    "对方"
                };
                format!("行为：{}发动{}", owner, skill_name)
            }
            BattleEvent::CardUsed { side, card_name } => {
                let owner = if *side == crate::battle::Side::Player {
                    "我方"
                } else {
                    "对方"
                };
                format!("行为：{}使用技能牌{}", owner, card_name)
            }
            BattleEvent::CardDiscarded { side, card_name } => {
                let owner = if *side == crate::battle::Side::Player {
                    "我方"
                } else {
                    "对方"
                };
                format!("行为：{}弃置{}", owner, card_name)
            }
            BattleEvent::Switched { side, name } => {
                let owner = if *side == crate::battle::Side::Player {
                    "我方"
                } else {
                    "对方"
                };
                format!("行为：{}换上{}", owner, name)
            }
            BattleEvent::CombatantFainted { side, name, .. } => {
                let owner = if *side == crate::battle::Side::Player {
                    "我方"
                } else {
                    "对方"
                };
                format!("行为：{}{}倒下", owner, name)
            }
            _ => continue,
        };
        text.0 = line;
    }
}

pub(crate) fn update_result_ui_system(
    mut result_text_q: Query<&mut Text, With<ResultText>>,
    battle_result: Res<crate::battle::BattleResult>,
) {
    if let Ok(mut result_text) = result_text_q.single_mut() {
        if battle_result.message.is_empty() {
            result_text.0.clear();
            return;
        }

        let mut lines = vec![battle_result.message.clone()];
        if let Some(status) = &battle_result.export_status {
            lines.push(status.clone());
        }
        lines.push("按 L 导出 replay/action log。".to_string());
        result_text.0 = lines.join("\n");
    }
}
