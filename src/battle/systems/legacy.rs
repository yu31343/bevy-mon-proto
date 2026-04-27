use std::collections::HashMap;

use bevy::prelude::*;

use crate::{
    battle::{
        AccuracyRng, BattleEvent, BattleLog, BattleResult, Combatant, ElementAura, InBattle,
        PendingBoosts, Shield, Side, SkillList, Stats, TurnAction, TurnContext, TurnCount,
    },
    data::{BattleDbs, BattleFormulaRules, SkillEffect},
    game_state::{BattlePhase, GameState},
};

use super::{abort_battle, apply_effect};

#[allow(dead_code)]
fn card_hotkey_to_index(key: KeyCode) -> Option<usize> {
    match key {
        KeyCode::KeyZ => Some(0),
        KeyCode::KeyX => Some(1),
        KeyCode::KeyC => Some(2),
        KeyCode::KeyV => Some(3),
        KeyCode::KeyB => Some(4),
        _ => None,
    }
}

#[allow(dead_code)]
fn player_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut player_team: ResMut<crate::battle::PlayerTeam>,
    mut battle_log: ResMut<BattleLog>,
    mut battle_result: ResMut<BattleResult>,
    query: Query<(&Combatant, &SkillList, &Stats), With<InBattle>>,
) {
    if player_team.0.combatants.is_empty() {
        abort_battle(
            "玩家队伍为空，无法继续战斗。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyQ) {
        // 尝试切换到下一个活着的成员
        let current = player_team.0.active_index;
        for offset in 1..player_team.0.combatants.len() {
            let next_idx = (current + offset) % player_team.0.combatants.len();
            let entity = player_team.0.combatants[next_idx];
            if let Ok((_, _, stats)) = query.get(entity) {
                if stats.hp > 0 {
                    player_team.0.active_index = next_idx;
                    turn_ctx.player_action = Some(TurnAction::Switch);
                    next_phase.set(BattlePhase::EnemyTurn);
                    return;
                }
            }
        }
    }

    let Some(active_entity) = player_team.0.active_combatant() else {
        abort_battle(
            "玩家当前上场成员无效，战斗已中断。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    let Ok((_, skill_list, _)) = query.get(active_entity) else {
        abort_battle(
            "玩家当前成员数据丢失，战斗已中断。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    let skills = skill_list.0;

    let selected = if keyboard.just_pressed(KeyCode::Digit1) {
        Some(skills[0])
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        Some(skills[1])
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        Some(skills[2])
    } else if keyboard.just_pressed(KeyCode::Digit4) {
        Some(skills[3])
    } else if keyboard.just_pressed(KeyCode::Space) {
        Some(skills[0])
    } else {
        None
    };

    if let Some(skill) = selected {
        turn_ctx.player_action = Some(TurnAction::Skill(skill));
        next_phase.set(BattlePhase::EnemyTurn);
    }
}

#[allow(dead_code)]
fn enemy_choose_skill_system(
    mut turn_ctx: ResMut<TurnContext>,
    dbs: Res<BattleDbs>,
    player_team: Res<crate::battle::PlayerTeam>,
    enemy_team: Res<crate::battle::EnemyTeam>,
    mut battle_log: ResMut<BattleLog>,
    mut battle_result: ResMut<BattleResult>,
    mut next_game_state: ResMut<NextState<GameState>>,
    query: Query<(&Combatant, &Stats, &SkillList), With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    let Some(p_entity) = player_team.0.active_combatant() else {
        abort_battle(
            "敌方选招失败：玩家上场成员无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    let Some(e_entity) = enemy_team.0.active_combatant() else {
        abort_battle(
            "敌方选招失败：敌方上场成员无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    let Ok((player_combatant, player_stats, _)) = query.get(p_entity) else {
        abort_battle(
            "敌方选招失败：玩家成员数据缺失。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    let Ok((enemy_combatant, enemy_stats, enemy_skills)) = query.get(e_entity) else {
        abort_battle(
            "敌方选招失败：敌方成员数据缺失。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    let low_hp = enemy_stats.max_hp > 0 && enemy_stats.hp * 100 < enemy_stats.max_hp * 30;
    let under_counter = dbs
        .elements
        .get_effectiveness(player_combatant.element, enemy_combatant.element)
        > 1.0;

    let mut heal_candidates = Vec::new();
    let mut best_scored: Option<(crate::data::SkillId, i32)> = None;

    for skill_id in enemy_skills.0 {
        let Some(skill) = dbs.skills.get(&skill_id) else {
            continue;
        };

        let score = match &skill.effect {
            SkillEffect::Attack { power, .. } => {
                let base_damage = *power + enemy_stats.atk - player_stats.def;
                let effectiveness = if let Some(skill_element) = skill.element {
                    dbs.elements
                        .get_effectiveness(skill_element, player_combatant.element)
                } else {
                    1.0
                };
                (base_damage as f32 * effectiveness).max(1.0) as i32
            }
            SkillEffect::Heal { amount } => {
                heal_candidates.push((skill_id, *amount));
                *amount / 2
            }
            SkillEffect::Shield { amount } => {
                let mut shield_score = *amount;
                if under_counter {
                    shield_score += 50;
                }
                shield_score
            }
            SkillEffect::ApplyStatus { .. } => 90,
            SkillEffect::ModifyStages { .. } => 85,
            SkillEffect::Cleanse { .. } => 88,
            SkillEffect::Dispel { .. } => 89,
            SkillEffect::DealFixedDamage { .. } => 92,
            SkillEffect::DealStatDifferenceDamage { .. } => 93,
            SkillEffect::Conditional { .. } => 94,
            SkillEffect::Sequence { .. } => 95,
        };

        match best_scored {
            Some((_, best_score)) if score <= best_score => {}
            _ => best_scored = Some((skill_id, score)),
        }
    }

    let selected = if low_hp {
        heal_candidates
            .into_iter()
            .max_by_key(|(_, amount)| *amount)
            .map(|(id, _)| id)
            .or_else(|| best_scored.map(|(id, _)| id))
    } else {
        best_scored.map(|(id, _)| id)
    };

    let fallback = enemy_skills
        .0
        .iter()
        .copied()
        .find(|sid| dbs.skills.contains_key(sid));

    let Some(final_skill) = selected.or(fallback) else {
        abort_battle(
            "敌方选招失败：无可用技能。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    turn_ctx.enemy_action = Some(TurnAction::Skill(final_skill));
    next_phase.set(BattlePhase::CheckEnd);
}

#[allow(dead_code)]
fn resolve_turn_system(
    mut query: Query<
        (
            Entity,
            &Combatant,
            &mut Stats,
            &SkillList,
            &mut Shield,
            &mut crate::battle::StatusBoard,
            &mut ElementAura,
            &Name,
        ),
        With<InBattle>,
    >,
    mut turn_ctx: ResMut<TurnContext>,
    dbs: Res<BattleDbs>,
    player_team: Res<crate::battle::PlayerTeam>,
    enemy_team: Res<crate::battle::EnemyTeam>,
    mut battle_log: ResMut<BattleLog>,
    mut battle_result: ResMut<BattleResult>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut turn_count: ResMut<TurnCount>,
    mut pending_boosts: ResMut<PendingBoosts>,
    formula_rules: Res<BattleFormulaRules>,
    mut accuracy_rng: ResMut<AccuracyRng>,
) {
    let Some(player_action) = turn_ctx.player_action else {
        return;
    };
    let Some(enemy_action) = turn_ctx.enemy_action else {
        return;
    };

    let Some(player_entity) = player_team.0.active_combatant() else {
        abort_battle(
            "回合结算失败：玩家上场成员无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    let Some(enemy_entity) = enemy_team.0.active_combatant() else {
        abort_battle(
            "回合结算失败：敌方上场成员无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    let Ok((_, _, p_stats, _, _, _, _, _)) = query.get(player_entity) else {
        abort_battle(
            "回合结算失败：玩家成员数据缺失。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    let Ok((_, _, e_stats, _, _, _, _, _)) = query.get(enemy_entity) else {
        abort_battle(
            "回合结算失败：敌方成员数据缺失。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    let player_spd = p_stats.spd;
    let enemy_spd = e_stats.spd;

    turn_count.0 += 1;
    event_writer.write(BattleEvent::TurnStarted(turn_count.0));

    let mut order = vec![
        (
            Side::Player,
            player_action,
            action_priority(&player_action, &dbs.skills),
            player_spd,
            0_u8,
        ),
        (
            Side::Enemy,
            enemy_action,
            action_priority(&enemy_action, &dbs.skills),
            enemy_spd,
            1_u8,
        ),
    ];
    order.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| b.3.cmp(&a.3))
            .then_with(|| a.4.cmp(&b.4))
    });

    for (side, action, _, _, _) in order {
        let active_side_entity = if side == Side::Player {
            player_entity
        } else {
            enemy_entity
        };

        if let TurnAction::Switch = action {
            if let Ok((_, _, _, _, _, _, _, name)) = query.get(active_side_entity) {
                event_writer.write(BattleEvent::Switched {
                    side,
                    name: name.to_string(),
                });
            }
            continue;
        }

        let skill_id = match action {
            TurnAction::Skill(id) => id,
            TurnAction::Switch => continue,
        };

        let skill_slot = query
            .get(active_side_entity)
            .map(|(_, _, _, sl, _, _, _, _)| sl.0.iter().position(|&s| s == skill_id).unwrap_or(0))
            .unwrap_or(0);

        let Ok(
            [
                (_, a_combatant, mut a_stats, _, mut a_shield, mut a_statuses, mut a_aura, _),
                (_, t_combatant, mut t_stats, _, mut t_shield, mut t_statuses, mut t_aura, _),
            ],
        ) = query.get_many_mut([player_entity, enemy_entity])
        else {
            abort_battle(
                "回合结算失败：无法同时访问双方成员。",
                &mut battle_log,
                &mut battle_result,
                &mut next_game_state,
            );
            return;
        };

        let actor_is_player_slot = a_combatant.side == side;
        let attacker_dead = if actor_is_player_slot {
            a_stats.hp <= 0
        } else {
            t_stats.hp <= 0
        };
        if attacker_dead {
            continue;
        }

        let Some(skill) = dbs.skills.get(&skill_id) else {
            continue;
        };

        let attacker_side = if actor_is_player_slot {
            a_combatant.side
        } else {
            t_combatant.side
        };
        let target_side = if actor_is_player_slot {
            t_combatant.side
        } else {
            a_combatant.side
        };

        event_writer.write(BattleEvent::SkillUsed {
            side: attacker_side,
            skill_name: skill.name.clone(),
            slot: skill_slot,
        });

        if actor_is_player_slot {
            apply_effect(
                skill,
                attacker_side,
                target_side,
                t_combatant.element,
                &mut a_stats,
                &mut a_shield,
                &mut a_statuses,
                &mut t_stats,
                &mut t_shield,
                &mut t_aura,
                &mut t_statuses,
                &mut pending_boosts,
                &formula_rules,
                &mut accuracy_rng,
                &dbs.elements,
                &dbs.statuses,
                &dbs.reactions,
                &mut event_writer,
                None,
                None,
                None,
                None,
            );
        } else {
            apply_effect(
                skill,
                attacker_side,
                target_side,
                a_combatant.element,
                &mut t_stats,
                &mut t_shield,
                &mut t_statuses,
                &mut a_stats,
                &mut a_shield,
                &mut a_aura,
                &mut a_statuses,
                &mut pending_boosts,
                &formula_rules,
                &mut accuracy_rng,
                &dbs.elements,
                &dbs.statuses,
                &dbs.reactions,
                &mut event_writer,
                None,
                None,
                None,
                None,
            );
        }
    }

    turn_ctx.player_action = None;
    turn_ctx.enemy_action = None;
    next_phase.set(BattlePhase::CheckEnd);
}

#[allow(dead_code)]
fn action_priority(
    action: &TurnAction,
    skills: &HashMap<crate::data::SkillId, crate::data::SkillDef>,
) -> i32 {
    match action {
        TurnAction::Switch => 300,
        TurnAction::Skill(skill_id) => {
            let Some(skill) = skills.get(skill_id) else {
                return 0;
            };
            match &skill.effect {
                SkillEffect::Shield { .. } | SkillEffect::Heal { .. } => 200,
                SkillEffect::ApplyStatus { .. }
                | SkillEffect::ModifyStages { .. }
                | SkillEffect::Cleanse { .. }
                | SkillEffect::Dispel { .. } => 150,
                SkillEffect::DealFixedDamage { .. }
                | SkillEffect::DealStatDifferenceDamage { .. }
                | SkillEffect::Conditional { .. } => 165,
                SkillEffect::Sequence { .. } => 175,
                SkillEffect::Attack { .. } => 100,
            }
        }
    }
}
