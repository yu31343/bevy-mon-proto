use bevy::prelude::*;

use crate::{
    battle::{
        BattleEvent, Combatant, EnemyTeam, ElementAura, InBattle, PlayerTeam, Shield, SkillCount,
        SkillList, Stats, Team,
    },
    data::BattleDbs,
    game_state::BattlePhase,
};

use super::super::components::*;

fn format_active_summary(
    header: &str,
    team: &Team,
    query: &Query<(&Combatant, &Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
) -> String {
    let Some(entity) = team.active_combatant() else {
        return format!("{header}：无在场精灵");
    };
    let Ok((combatant, stats, name, shield, aura)) = query.get(entity) else {
        return format!("{header}：数据读取失败");
    };
    format!(
        "{}在场 [{}] {} | HP {}/{} | 护盾 {} | 附着 {}",
        header,
        combatant.side,
        name,
        stats.hp.max(0),
        stats.max_hp,
        shield.0.max(0),
        super::super::helpers::aura_label(aura.attached),
    )
}

/// 仅更新战斗相关 `Text`，避免与 `Node` 宽度更新在同一系统内触发 B0001
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_battle_text_system(
    mut text_q: Query<
        (
            &mut Text,
            Option<&BattlePhaseText>,
            Option<&PlayerStatsText>,
            Option<&EnemyStatsText>,
            Option<&ResultText>,
            Option<&SkillButtonText>,
            Option<&SkillButtonMetaText>,
            Option<&SkillButtonIconText>,
            Option<&EnemySkillText>,
            Option<&EnemySkillMetaText>,
            Option<&EnemySkillIconText>,
            Option<&PlayerNameText>,
            Option<&EnemyNameText>,
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
        ),
    >,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<(&Combatant, &Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
    skill_query: Query<(&SkillList, &SkillCount), With<InBattle>>,
    skill_db: Res<BattleDbs>,
    battle_phase: Res<State<BattlePhase>>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_line = format_active_summary("玩家", &player_team.0, &combat_query);
    let enemy_line = format_active_summary("敌方", &enemy_team.0, &combat_query);

    let mut player_skills = None;
    if let Some(p_entity) = player_team.0.active_combatant() {
        if let Ok((skills, count)) = skill_query.get(p_entity) {
            player_skills = Some((skills.0, count.0));
        }
    }

    let mut enemy_skills = None;
    if let Some(e_entity) = enemy_team.0.active_combatant() {
        if let Ok((skills, count)) = skill_query.get(e_entity) {
            enemy_skills = Some((skills.0, count.0));
        }
    }

    for (
        mut text,
        is_phase,
        is_player,
        is_enemy,
        is_result,
        skill_button_text,
        skill_button_meta_text,
        skill_icon_text,
        enemy_skill_text,
        enemy_skill_meta,
        enemy_skill_icon,
        is_player_name,
        is_enemy_name,
    ) in &mut text_q
    {
        if is_result.is_some() {
            text.0.clear();
            continue;
        }
        if is_phase.is_some() {
            text.0 = format!(
                "战斗阶段：{}",
                super::super::helpers::phase_label(*battle_phase.get())
            );
            continue;
        }
        if is_player_name.is_some() {
            if let Some(entity) = player_team.0.active_combatant() {
                if let Ok((_, _, name, _, _)) = combat_query.get(entity) {
                    text.0 = format!("我方：{}", name);
                }
            }
            continue;
        }
        if is_enemy_name.is_some() {
            if let Some(entity) = enemy_team.0.active_combatant() {
                if let Ok((_, _, name, _, _)) = combat_query.get(entity) {
                    text.0 = format!("敌方：{}", name);
                }
            }
            continue;
        }
        if is_player.is_some() {
            text.0 = player_line.clone();
            continue;
        }
        if is_enemy.is_some() {
            text.0 = enemy_line.clone();
            continue;
        }
        if let (Some(button), Some((skills, count))) = (skill_button_text, player_skills) {
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
        if let (Some(meta), Some((skills, count))) = (skill_button_meta_text, player_skills) {
            if meta.index >= count {
                text.0 = "AP消耗：--".to_string();
            } else {
                let skill_id = skills[meta.index];
                text.0 = format!(
                    "{} AP消耗：{}",
                    super::super::helpers::skill_meta(skill_id, &skill_db),
                    super::super::helpers::monster_skill_ap_cost_ui(meta.index)
                );
            }
            continue;
        }
        if let (Some(icon), Some((_skills, count))) = (skill_icon_text, player_skills) {
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
            BattleEvent::CombatantFainted { side, name } => {
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
        result_text.0 = battle_result.message.clone();
    }
}

