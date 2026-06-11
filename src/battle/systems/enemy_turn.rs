use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    battle::{
        ActionPoints, ActionTrace, BattleControlMode, BattleEvent, BattleFormulaEvent, BattleLog,
        BattleResult, BattleStatusEvent, Combatant, ElementAura, Hand, InBattle, PendingBoosts,
        PendingKoResolution, PendingTacticalDiscard, RoundOrder, Shield, Side, SkillCount,
        SkillList, Stats, StructuredBattleLog, TurnAction, TurnContext, TurnCount,
        next_phase_after_side_end, note_action_phase, push_named_action_trace,
        push_turn_action_trace, transfer_status_by_id,
    },
    console_log::{ConsoleLogCategory, log as console_log, log_enabled},
    data::{
        AiDifficulty, AiPlayerInfoVisibility, BattleDbs, BattleRules, CardDef, CardEffect,
        EnemyAiConfig, StatusCategory,
    },
    game_state::{BattlePhase, GameState},
};

use super::{
    SkillExecutionMode, SkillTargetMode, WindSpreadTarget, abort_battle, apply_effect,
    apply_self_effect, apply_wind_effect, process_side_end_statuses, skill_execution_mode,
    skill_target_mode,
};

use crate::battle::ai::{
    EnemyAiContext, EnemyPlannedAction, EnemySwitchCandidate, ScoredEnemySkill, best_action_value,
    build_player_threat_context, choose_enemy_discard_card, choose_enemy_plan_candidates,
    choose_enemy_skill_with_threat, enemy_skill_candidate_report, enemy_switch_candidate_report,
    score_card_for_skill,
};

const ENEMY_AI_INITIAL_DELAY: f32 = 0.35;
const ENEMY_AI_ACTION_DELAY: f32 = 1.25;

#[derive(SystemParam)]
pub(crate) struct EnemyTurnLogs<'w> {
    turn_count: Res<'w, TurnCount>,
    structured_log: ResMut<'w, StructuredBattleLog>,
    action_trace: ResMut<'w, ActionTrace>,
}

#[derive(SystemParam)]
pub(crate) struct EnemyTurnEventWriters<'w> {
    event_writer: MessageWriter<'w, BattleEvent>,
    formula_writer: MessageWriter<'w, BattleFormulaEvent>,
    status_writer: MessageWriter<'w, BattleStatusEvent>,
}

#[derive(SystemParam)]
pub(crate) struct EnemyTurnRuntime<'w> {
    round_order: Res<'w, RoundOrder>,
    action_points: ResMut<'w, ActionPoints>,
    hand: ResMut<'w, Hand>,
    pending_boosts: ResMut<'w, PendingBoosts>,
    pending_ko: ResMut<'w, PendingKoResolution>,
    dbs: Res<'w, BattleDbs>,
    battle_rules: Res<'w, BattleRules>,
    formula_rules: Res<'w, crate::data::BattleFormulaRules>,
    accuracy_rng: ResMut<'w, crate::battle::AccuracyRng>,
    player_team: Res<'w, crate::battle::PlayerTeam>,
    enemy_team: ResMut<'w, crate::battle::EnemyTeam>,
    battle_log: ResMut<'w, BattleLog>,
    battle_result: ResMut<'w, BattleResult>,
    next_game_state: ResMut<'w, NextState<GameState>>,
    ai_config: Res<'w, EnemyAiConfig>,
}

fn finalize_enemy_turn(
    e_entity: Entity,
    enemy_team: &crate::battle::EnemyTeam,
    turn_ctx: &mut TurnContext,
    round_order: &RoundOrder,
    pending_boosts: &mut PendingBoosts,
    action_points: &mut ActionPoints,
    hand: &Hand,
    battle_rules: &BattleRules,
    commands: &mut Commands,
    formula_rules: &crate::data::BattleFormulaRules,
    logs: &mut EnemyTurnLogs,
    writers: &mut EnemyTurnEventWriters,
    next_phase: &mut ResMut<NextState<BattlePhase>>,
    exec_query: &mut Query<
        (
            Entity,
            &Combatant,
            &mut Stats,
            &SkillList,
            &SkillCount,
            &mut Shield,
            &mut crate::battle::StatusBoard,
            &mut ElementAura,
            &Name,
        ),
        With<InBattle>,
    >,
    ai_state: &mut (f32, bool, bool),
) {
    let team_entities = enemy_team.0.combatants.clone();
    for entity in team_entities {
        if let Ok((_, _, mut stats, _, _, mut shield, mut statuses, mut aura, _)) =
            exec_query.get_mut(entity)
        {
            process_side_end_statuses(
                &mut stats,
                &mut shield,
                &mut aura,
                &mut statuses,
                crate::battle::SideEndTickParams {
                    side: Side::Enemy,
                    round: logs.turn_count.0,
                    formula_rules,
                    pending_boosts: Some(&mut *pending_boosts),
                    event_writer: &mut writers.event_writer,
                    formula_writer: &mut writers.formula_writer,
                    status_writer: &mut writers.status_writer,
                    structured_log: &mut logs.structured_log,
                },
            );
        }
    }
    super::clear_action_scoped_card_effects(Side::Enemy, pending_boosts);
    commands.insert_resource(crate::battle::PendingGuardCounterClear {
        acting_side: Side::Enemy,
    });
    turn_ctx.enemy_ended = true;
    ai_state.0 = 0.0;
    ai_state.1 = false;
    ai_state.2 = false;
    if let Ok((_, _, stats, _, _, _, _, _, _)) = exec_query.get(e_entity) {
        if stats.hp <= 0 {
            super::clamp_ap_to_max(Side::Enemy, battle_rules, action_points);
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }
    }
    super::enter_discard_phase_or_continue(
        Side::Enemy,
        next_phase_after_side_end(round_order, Side::Enemy),
        hand,
        battle_rules,
        action_points,
        commands,
        next_phase,
    );
}

fn skill_card_pending_slot_available(card: &CardDef, pending_boosts: &PendingBoosts) -> bool {
    let pending = &pending_boosts.enemy;
    match &card.effect {
        CardEffect::NextAttackBoost { .. } => pending.next_attack_bonus == 0,
        CardEffect::NextShieldBoost { .. } => pending.next_shield_bonus == 0,
        CardEffect::NextHealBoost { .. } => pending.next_heal_bonus == 0,
        CardEffect::NextElementAttachmentGainAp { .. } => {
            pending.next_element_attachment_ap.is_none()
        }
        CardEffect::NextReactionFixedDamage { .. } => pending.next_reaction_fixed_damage.is_none(),
        CardEffect::NextWindSpreadDamage { .. } => pending.next_wind_spread_damage.is_none(),
        CardEffect::NextAuraAttackDraw { .. } => pending.next_aura_attack_draw.is_none(),
        CardEffect::NextSkillCostDraw { .. } => pending.next_skill_cost_draw.is_none(),
        CardEffect::DrawIfKnockedOutThisTurn { .. } => pending.next_knockout_draw.is_none(),
        _ => true,
    }
}

fn try_play_boost_card_for_skill(
    chosen_skill: ScoredEnemySkill,
    hand: &mut Hand,
    action_points: &mut ActionPoints,
    pending_boosts: &PendingBoosts,
    dbs: &BattleDbs,
    ai_ctx: &EnemyAiContext,
    weights: &crate::data::EnemyAiWeights,
    event_writer: &mut MessageWriter<BattleEvent>,
) -> bool {
    let Some(skill) = dbs.skills.get(&chosen_skill.skill_id) else {
        return false;
    };
    let skill_cost = skill.cost_ap;
    let available_ap_after_skill = action_points.enemy - skill_cost;
    let Some((idx, _score)) = hand
        .enemy
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(idx, card_id)| {
            let card = dbs.cards.get(&card_id)?;
            skill_card_pending_slot_available(card, pending_boosts)
                .then(|| {
                    score_card_for_skill(
                        card,
                        chosen_skill.kind,
                        skill,
                        ai_ctx,
                        dbs,
                        available_ap_after_skill,
                        weights,
                    )
                    .map(|score| (idx, score))
                })
                .flatten()
        })
        .max_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.0.cmp(&a.0))
        })
    else {
        return false;
    };

    let card_id = hand.enemy.remove(idx);
    let Some(card) = dbs.cards.get(&card_id) else {
        return false;
    };

    action_points.enemy -= card.cost_ap;
    event_writer.write(BattleEvent::CardUsed {
        side: Side::Enemy,
        card_name: card.name.to_string(),
    });
    true
}

pub fn enemy_turn_input_system(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    pending_tactical_discard: Option<Res<PendingTacticalDiscard>>,
    battle_mode: Res<BattleControlMode>,
    mut selected: ResMut<crate::battle::SelectedCards>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut runtime: EnemyTurnRuntime,
    mut logs: EnemyTurnLogs,
    mut writers: EnemyTurnEventWriters,
    mut exec_query: Query<
        (
            Entity,
            &Combatant,
            &mut Stats,
            &SkillList,
            &SkillCount,
            &mut Shield,
            &mut crate::battle::StatusBoard,
            &mut ElementAura,
            &Name,
        ),
        With<InBattle>,
    >,
) {
    if !matches!(
        *battle_mode,
        BattleControlMode::DebugPlayerControlsBoth | BattleControlMode::PlayerVsRemote
    ) {
        return;
    }
    let remote_controlled = *battle_mode == BattleControlMode::PlayerVsRemote;

    let round_order = &runtime.round_order;
    let action_points = &mut runtime.action_points;
    let hand = &mut runtime.hand;
    let pending_boosts = &mut runtime.pending_boosts;
    let pending_ko = &mut runtime.pending_ko;
    let dbs = &runtime.dbs;
    let battle_rules = &runtime.battle_rules;
    let formula_rules = &runtime.formula_rules;
    let accuracy_rng = &mut runtime.accuracy_rng;
    let player_team = &runtime.player_team;
    let enemy_team = &mut runtime.enemy_team;
    let battle_log = &mut runtime.battle_log;
    let battle_result = &mut runtime.battle_result;
    let next_game_state = &mut runtime.next_game_state;

    if turn_ctx.enemy_ended {
        return;
    }

    let Some(p_entity) = player_team.0.active_combatant() else {
        abort_battle(
            "调试模式：玩家上场精灵无效。",
            battle_log,
            battle_result,
            next_game_state,
        );
        return;
    };
    let Some(e_entity) = enemy_team.0.active_combatant() else {
        abort_battle(
            "调试模式：敌方上场精灵无效。",
            battle_log,
            battle_result,
            next_game_state,
        );
        return;
    };

    let switch_target = if keyboard.just_pressed(KeyCode::Digit5) {
        Some(0_usize)
    } else if keyboard.just_pressed(KeyCode::Digit6) {
        Some(1_usize)
    } else if keyboard.just_pressed(KeyCode::Digit7) {
        Some(2_usize)
    } else {
        None
    };

    if !remote_controlled && let Some(target_index) = switch_target {
        if action_points.enemy >= 1
            && target_index < enemy_team.0.combatants.len()
            && target_index != enemy_team.0.active_index
        {
            let current_entity = enemy_team.0.combatants[enemy_team.0.active_index];
            let target_entity = enemy_team.0.combatants[target_index];
            if let Ok(
                [
                    (_, _, mut current_stats, _, _, _, mut current_statuses, _, _),
                    (_, _, target_stats, _, _, _, target_statuses, _, name),
                ],
            ) = exec_query.get_many_mut([current_entity, target_entity])
            {
                if target_stats.hp > 0 {
                    transfer_status_by_id(
                        &mut current_statuses,
                        &mut current_stats,
                        target_statuses.into_inner(),
                        target_stats.into_inner(),
                        "nature_regen",
                    );
                    action_points.enemy -= 1;
                    enemy_team.0.active_index = target_index;
                    writers.event_writer.write(BattleEvent::Switched {
                        side: Side::Enemy,
                        name: name.to_string(),
                    });
                    note_action_phase(
                        &mut logs.structured_log,
                        logs.turn_count.0,
                        Side::Enemy,
                        "敌方换人",
                        format!("切换到 {}；敌方AP={}", name, action_points.enemy),
                    );
                    push_turn_action_trace(
                        &mut logs.action_trace,
                        logs.turn_count.0,
                        Side::Enemy,
                        TurnAction::Switch,
                        format!("切换到 {}；剩余AP={}", name, action_points.enemy),
                    );
                }
            }
        }
        return;
    }

    if !remote_controlled
        && pending_tactical_discard
            .as_ref()
            .is_some_and(|pending| pending.side == Side::Enemy)
    {
        for (key, idx) in [
            (KeyCode::KeyZ, 0_usize),
            (KeyCode::KeyX, 1_usize),
            (KeyCode::KeyC, 2_usize),
            (KeyCode::KeyV, 3_usize),
            (KeyCode::KeyB, 4_usize),
            (KeyCode::KeyN, 5_usize),
            (KeyCode::KeyA, 6_usize),
            (KeyCode::KeyS, 7_usize),
            (KeyCode::KeyD, 8_usize),
            (KeyCode::KeyG, 9_usize),
            (KeyCode::KeyH, 10_usize),
            (KeyCode::KeyJ, 11_usize),
            (KeyCode::KeyK, 12_usize),
            (KeyCode::KeyL, 13_usize),
            (KeyCode::KeyU, 14_usize),
            (KeyCode::KeyI, 15_usize),
            (KeyCode::KeyO, 16_usize),
            (KeyCode::KeyP, 17_usize),
        ] {
            if !keyboard.just_pressed(key) {
                continue;
            }
            if idx >= hand.enemy.len() {
                return;
            }
            let card_id = hand.enemy.remove(idx);
            action_points.enemy += 1;
            let card_name = dbs
                .cards
                .get(&card_id)
                .map(|c| c.name.to_string())
                .unwrap_or_else(|| format!("{card_id:?}"));
            writers.event_writer.write(BattleEvent::CardDiscarded {
                side: Side::Enemy,
                card_name: card_name.clone(),
            });
            note_action_phase(
                &mut logs.structured_log,
                logs.turn_count.0,
                Side::Enemy,
                "敌方战术整理",
                format!(
                    "弃置卡牌={}；获得AP=1；当前AP={}",
                    card_name, action_points.enemy
                ),
            );
            push_named_action_trace(
                &mut logs.action_trace,
                logs.turn_count.0,
                Side::Enemy,
                "tactical_discard",
                format!("弃置卡牌={}；当前AP={}", card_name, action_points.enemy),
            );
            selected.enemy.index = None;
            selected.enemy.discard_armed = false;
            return;
        }
        return;
    }

    if !remote_controlled && keyboard.just_pressed(KeyCode::KeyF) {
        if hand.enemy.is_empty() {
            return;
        }
        let Some(target_index) = selected.enemy.index.filter(|&i| i < hand.enemy.len()) else {
            selected.enemy.discard_armed = !selected.enemy.discard_armed;
            return;
        };
        let card_id = hand.enemy.remove(target_index);
        action_points.enemy += 1;
        let card_name = dbs
            .cards
            .get(&card_id)
            .map(|c| c.name.to_string())
            .unwrap_or_else(|| format!("{card_id:?}"));
        writers.event_writer.write(BattleEvent::CardDiscarded {
            side: Side::Enemy,
            card_name: card_name.clone(),
        });
        note_action_phase(
            &mut logs.structured_log,
            logs.turn_count.0,
            Side::Enemy,
            "敌方弃牌",
            format!(
                "弃置卡牌={}；获得AP=1；当前AP={}",
                card_name, action_points.enemy
            ),
        );
        push_named_action_trace(
            &mut logs.action_trace,
            logs.turn_count.0,
            Side::Enemy,
            "discard_card",
            format!("弃置卡牌={}；当前AP={}", card_name, action_points.enemy),
        );
        selected.enemy.index = None;
        selected.enemy.discard_armed = false;
        return;
    }

    if let Some(TurnAction::Skill(skill_id)) = turn_ctx.enemy_action {
        let Ok((_, _, _, skill_list, skill_count, _, _, _, _)) = exec_query.get(e_entity) else {
            turn_ctx.enemy_action = None;
            return;
        };
        let Some(slot) = skill_list
            .0
            .iter()
            .take(skill_count.0)
            .position(|&s| s == skill_id)
        else {
            turn_ctx.enemy_action = None;
            return;
        };
        let Some(skill) = dbs.skills.get(&skill_id) else {
            turn_ctx.enemy_action = None;
            return;
        };
        let cost = skill.cost_ap;
        if action_points.enemy < cost {
            turn_ctx.enemy_action = None;
            return;
        }
        action_points.enemy -= cost;
        turn_ctx.enemy_action = None;

        writers.event_writer.write(BattleEvent::SkillUsed {
            side: Side::Enemy,
            skill_name: skill.name.clone(),
            slot,
        });
        note_action_phase(
            &mut logs.structured_log,
            logs.turn_count.0,
            Side::Enemy,
            "敌方使用技能",
            format!(
                "技能={}；槽位={}；消耗AP={}；剩余AP={}",
                skill.name, slot, cost, action_points.enemy
            ),
        );
        push_turn_action_trace(
            &mut logs.action_trace,
            logs.turn_count.0,
            Side::Enemy,
            TurnAction::Skill(skill_id),
            format!(
                "技能={}；槽位={}；剩余AP={}",
                skill.name, slot, action_points.enemy
            ),
        );

        if skill_execution_mode(skill) == SkillExecutionMode::WindSpread {
            let player_backs = player_team
                .0
                .combatants
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(index, entity)| {
                    (index != player_team.0.active_index && entity != p_entity).then_some(entity)
                })
                .collect::<Vec<_>>();
            match player_backs.as_slice() {
                [back_a, back_b, ..] => {
                    let Ok(
                        [
                            (
                                _,
                                _e_combatant,
                                mut e_stats_m,
                                _,
                                _,
                                mut e_shield_m,
                                mut e_statuses_m,
                                _e_aura_m,
                                _,
                            ),
                            (
                                _,
                                p_combatant,
                                mut p_stats_m,
                                _,
                                _,
                                mut p_shield_m,
                                mut p_statuses_m,
                                mut p_aura_m,
                                _,
                            ),
                            (
                                _,
                                back_a_combatant,
                                mut back_a_stats,
                                _,
                                _,
                                mut back_a_shield,
                                mut back_a_statuses,
                                mut back_a_aura,
                                _,
                            ),
                            (
                                _,
                                back_b_combatant,
                                mut back_b_stats,
                                _,
                                _,
                                mut back_b_shield,
                                mut back_b_statuses,
                                mut back_b_aura,
                                _,
                            ),
                        ],
                    ) = exec_query.get_many_mut([e_entity, p_entity, *back_a, *back_b])
                    else {
                        return;
                    };
                    apply_wind_effect(
                        skill,
                        Side::Enemy,
                        Side::Player,
                        &mut e_stats_m,
                        &mut e_shield_m,
                        &mut e_statuses_m,
                        WindSpreadTarget {
                            base_element: p_combatant.element,
                            stats: &mut p_stats_m,
                            shield: &mut p_shield_m,
                            aura: &mut p_aura_m,
                            statuses: &mut p_statuses_m,
                        },
                        Some(WindSpreadTarget {
                            base_element: back_a_combatant.element,
                            stats: &mut back_a_stats,
                            shield: &mut back_a_shield,
                            aura: &mut back_a_aura,
                            statuses: &mut back_a_statuses,
                        }),
                        Some(WindSpreadTarget {
                            base_element: back_b_combatant.element,
                            stats: &mut back_b_stats,
                            shield: &mut back_b_shield,
                            aura: &mut back_b_aura,
                            statuses: &mut back_b_statuses,
                        }),
                        pending_boosts,
                        &formula_rules,
                        accuracy_rng,
                        &dbs.elements,
                        &dbs.statuses,
                        &dbs.reactions,
                        &mut writers.event_writer,
                        Some(&mut writers.formula_writer),
                        Some(&mut writers.status_writer),
                        Some(&mut logs.structured_log),
                        Some(logs.turn_count.0),
                        None,
                    );
                }
                [back_a] => {
                    let Ok(
                        [
                            (
                                _,
                                _e_combatant,
                                mut e_stats_m,
                                _,
                                _,
                                mut e_shield_m,
                                mut e_statuses_m,
                                _e_aura_m,
                                _,
                            ),
                            (
                                _,
                                p_combatant,
                                mut p_stats_m,
                                _,
                                _,
                                mut p_shield_m,
                                mut p_statuses_m,
                                mut p_aura_m,
                                _,
                            ),
                            (
                                _,
                                back_a_combatant,
                                mut back_a_stats,
                                _,
                                _,
                                mut back_a_shield,
                                mut back_a_statuses,
                                mut back_a_aura,
                                _,
                            ),
                        ],
                    ) = exec_query.get_many_mut([e_entity, p_entity, *back_a])
                    else {
                        return;
                    };
                    apply_wind_effect(
                        skill,
                        Side::Enemy,
                        Side::Player,
                        &mut e_stats_m,
                        &mut e_shield_m,
                        &mut e_statuses_m,
                        WindSpreadTarget {
                            base_element: p_combatant.element,
                            stats: &mut p_stats_m,
                            shield: &mut p_shield_m,
                            aura: &mut p_aura_m,
                            statuses: &mut p_statuses_m,
                        },
                        Some(WindSpreadTarget {
                            base_element: back_a_combatant.element,
                            stats: &mut back_a_stats,
                            shield: &mut back_a_shield,
                            aura: &mut back_a_aura,
                            statuses: &mut back_a_statuses,
                        }),
                        None,
                        pending_boosts,
                        &formula_rules,
                        accuracy_rng,
                        &dbs.elements,
                        &dbs.statuses,
                        &dbs.reactions,
                        &mut writers.event_writer,
                        Some(&mut writers.formula_writer),
                        Some(&mut writers.status_writer),
                        Some(&mut logs.structured_log),
                        Some(logs.turn_count.0),
                        None,
                    );
                }
                _ => {
                    let Ok(
                        [
                            (
                                _,
                                _e_combatant,
                                mut e_stats_m,
                                _,
                                _,
                                mut e_shield_m,
                                mut e_statuses_m,
                                _e_aura_m,
                                _,
                            ),
                            (
                                _,
                                p_combatant,
                                mut p_stats_m,
                                _,
                                _,
                                mut p_shield_m,
                                mut p_statuses_m,
                                mut p_aura_m,
                                _,
                            ),
                        ],
                    ) = exec_query.get_many_mut([e_entity, p_entity])
                    else {
                        return;
                    };
                    apply_wind_effect(
                        skill,
                        Side::Enemy,
                        Side::Player,
                        &mut e_stats_m,
                        &mut e_shield_m,
                        &mut e_statuses_m,
                        WindSpreadTarget {
                            base_element: p_combatant.element,
                            stats: &mut p_stats_m,
                            shield: &mut p_shield_m,
                            aura: &mut p_aura_m,
                            statuses: &mut p_statuses_m,
                        },
                        None,
                        None,
                        pending_boosts,
                        &formula_rules,
                        accuracy_rng,
                        &dbs.elements,
                        &dbs.statuses,
                        &dbs.reactions,
                        &mut writers.event_writer,
                        Some(&mut writers.formula_writer),
                        Some(&mut writers.status_writer),
                        Some(&mut logs.structured_log),
                        Some(logs.turn_count.0),
                        None,
                    );
                }
            }
        } else {
            let Ok(
                [
                    (
                        _,
                        _attacker_combatant,
                        mut e_stats_m,
                        _,
                        _,
                        mut e_shield_m,
                        mut e_statuses_m,
                        mut e_aura_m,
                        _,
                    ),
                    (
                        _,
                        p_combatant,
                        mut p_stats_m,
                        _,
                        _,
                        mut p_shield_m,
                        mut p_statuses_m,
                        mut p_aura_m,
                        _,
                    ),
                ],
            ) = exec_query.get_many_mut([e_entity, p_entity])
            else {
                return;
            };
            match skill_target_mode(skill) {
                SkillTargetMode::SelfOnly => apply_self_effect(
                    skill,
                    Side::Enemy,
                    &mut e_stats_m,
                    &mut e_shield_m,
                    &mut e_aura_m,
                    &mut e_statuses_m,
                    pending_boosts,
                    &formula_rules,
                    accuracy_rng,
                    &dbs.elements,
                    &dbs.statuses,
                    &dbs.reactions,
                    &mut writers.event_writer,
                    Some(&mut writers.formula_writer),
                    Some(&mut writers.status_writer),
                    Some(&mut logs.structured_log),
                    Some(logs.turn_count.0),
                ),
                SkillTargetMode::Opponent => apply_effect(
                    skill,
                    Side::Enemy,
                    Side::Player,
                    p_combatant.element,
                    &mut e_stats_m,
                    &mut e_shield_m,
                    &mut e_statuses_m,
                    &mut p_stats_m,
                    &mut p_shield_m,
                    &mut p_aura_m,
                    &mut p_statuses_m,
                    pending_boosts,
                    &formula_rules,
                    accuracy_rng,
                    &dbs.elements,
                    &dbs.statuses,
                    &dbs.reactions,
                    &mut writers.event_writer,
                    Some(&mut writers.formula_writer),
                    Some(&mut writers.status_writer),
                    Some(&mut logs.structured_log),
                    Some(logs.turn_count.0),
                ),
            };
        }

        let should_go_check_end = exec_query
            .get(p_entity)
            .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false)
            || exec_query
                .get(e_entity)
                .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false);
        if should_go_check_end {
            pending_ko.resume_phase = Some(BattlePhase::EnemyTurn);
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }
        return;
    }

    if !remote_controlled {
        for (key, idx) in [
            (KeyCode::KeyZ, 0_usize),
            (KeyCode::KeyX, 1_usize),
            (KeyCode::KeyC, 2_usize),
            (KeyCode::KeyV, 3_usize),
            (KeyCode::KeyB, 4_usize),
            (KeyCode::KeyN, 5_usize),
            (KeyCode::KeyA, 6_usize),
            (KeyCode::KeyS, 7_usize),
            (KeyCode::KeyD, 8_usize),
            (KeyCode::KeyG, 9_usize),
            (KeyCode::KeyH, 10_usize),
            (KeyCode::KeyJ, 11_usize),
            (KeyCode::KeyK, 12_usize),
            (KeyCode::KeyL, 13_usize),
            (KeyCode::KeyU, 14_usize),
            (KeyCode::KeyI, 15_usize),
            (KeyCode::KeyO, 16_usize),
            (KeyCode::KeyP, 17_usize),
        ] {
            if keyboard.just_pressed(key) {
                if idx >= hand.enemy.len() {
                    return;
                }
                if selected.enemy.discard_armed {
                    let card_id = hand.enemy.remove(idx);
                    action_points.enemy += 1;
                    let card_name = dbs
                        .cards
                        .get(&card_id)
                        .map(|c| c.name.to_string())
                        .unwrap_or_else(|| format!("{card_id:?}"));
                    writers.event_writer.write(BattleEvent::CardDiscarded {
                        side: Side::Enemy,
                        card_name: card_name.clone(),
                    });
                    note_action_phase(
                        &mut logs.structured_log,
                        logs.turn_count.0,
                        Side::Enemy,
                        "敌方弃牌",
                        format!(
                            "弃置卡牌={}；获得AP=1；当前AP={}",
                            card_name, action_points.enemy
                        ),
                    );
                    push_named_action_trace(
                        &mut logs.action_trace,
                        logs.turn_count.0,
                        Side::Enemy,
                        "discard_card",
                        format!("弃置卡牌={}；当前AP={}", card_name, action_points.enemy),
                    );
                    selected.enemy.index = None;
                    selected.enemy.discard_armed = false;
                    return;
                }
                if selected.enemy.index != Some(idx) {
                    selected.enemy.index = Some(idx);
                    return;
                }
                let card_id = hand.enemy[idx];
                if let Some(card) = dbs.cards.get(&card_id) {
                    if action_points.enemy >= card.cost_ap {
                        hand.enemy.remove(idx);
                        action_points.enemy -= card.cost_ap;

                        let card_name = card.name.to_string();
                        writers.event_writer.write(BattleEvent::CardUsed {
                            side: Side::Enemy,
                            card_name: card_name.clone(),
                        });

                        let effect_detail = "效果已排入卡牌结算".to_string();
                        note_action_phase(
                            &mut logs.structured_log,
                            logs.turn_count.0,
                            Side::Enemy,
                            "敌方使用卡牌",
                            format!(
                                "卡牌={}；消耗AP={}；效果={}；当前AP={}",
                                card_name, card.cost_ap, effect_detail, action_points.enemy
                            ),
                        );
                        push_named_action_trace(
                            &mut logs.action_trace,
                            logs.turn_count.0,
                            Side::Enemy,
                            "use_card",
                            format!(
                                "卡牌={}；效果={}；当前AP={}",
                                card_name, effect_detail, action_points.enemy
                            ),
                        );

                        selected.enemy.index = None;
                        selected.enemy.discard_armed = false;

                        let should_go_check_end = exec_query
                            .get(p_entity)
                            .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
                            .unwrap_or(false)
                            || exec_query
                                .get(e_entity)
                                .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
                                .unwrap_or(false);
                        if should_go_check_end {
                            pending_ko.resume_phase = Some(BattlePhase::EnemyTurn);
                            next_phase.set(BattlePhase::CheckEnd);
                            return;
                        }
                    }
                }
                return;
            }
        }

        let skill_slot = if keyboard.just_pressed(KeyCode::Digit1) {
            Some(0_usize)
        } else if keyboard.just_pressed(KeyCode::Digit2) {
            Some(1_usize)
        } else if keyboard.just_pressed(KeyCode::Digit3) {
            Some(2_usize)
        } else if keyboard.just_pressed(KeyCode::Digit4) {
            Some(3_usize)
        } else {
            None
        };

        if let Some(skill_slot) = skill_slot {
            let Ok((_, _, _, skill_list, skill_count, _, _, _, _)) = exec_query.get(e_entity)
            else {
                return;
            };
            if skill_slot >= skill_count.0 {
                return;
            }
            let skill_id = skill_list.0[skill_slot];
            turn_ctx.enemy_action = Some(TurnAction::Skill(skill_id));
            next_phase.set(BattlePhase::EnemyTurn);
            return;
        }

        if keyboard.just_pressed(KeyCode::KeyE) {
            turn_ctx.enemy_end_requested = true;
        }
    }
    if turn_ctx.enemy_end_requested {
        note_action_phase(
            &mut logs.structured_log,
            logs.turn_count.0,
            Side::Enemy,
            "敌方结束回合",
            format!("敌方主动结束回合；剩余AP={}", action_points.enemy),
        );
        push_named_action_trace(
            &mut logs.action_trace,
            logs.turn_count.0,
            Side::Enemy,
            "end_turn",
            format!("敌方主动结束回合；剩余AP={}", action_points.enemy),
        );
        turn_ctx.enemy_end_requested = false;
        let mut ai_state = (0.0_f32, false, false);
        finalize_enemy_turn(
            e_entity,
            enemy_team,
            &mut turn_ctx,
            &round_order,
            pending_boosts,
            action_points,
            hand,
            battle_rules,
            &mut commands,
            &formula_rules,
            &mut logs,
            &mut writers,
            &mut next_phase,
            &mut exec_query,
            &mut ai_state,
        );
    }
}

pub fn enemy_turn_ai_system(
    mut commands: Commands,
    time: Res<Time>,
    pending_tactical_discard: Option<Res<PendingTacticalDiscard>>,
    battle_mode: Res<BattleControlMode>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut runtime: EnemyTurnRuntime,
    mut logs: EnemyTurnLogs,
    mut writers: EnemyTurnEventWriters,
    mut ai_state: Local<(f32, bool, bool)>,
    mut exec_query: Query<
        (
            Entity,
            &Combatant,
            &mut Stats,
            &SkillList,
            &SkillCount,
            &mut Shield,
            &mut crate::battle::StatusBoard,
            &mut ElementAura,
            &Name,
        ),
        With<InBattle>,
    >,
) {
    if matches!(
        *battle_mode,
        BattleControlMode::DebugPlayerControlsBoth | BattleControlMode::PlayerVsRemote
    ) {
        ai_state.0 = 0.0;
        ai_state.1 = false;
        ai_state.2 = false;
        return;
    }

    let round_order = &runtime.round_order;
    let action_points = &mut runtime.action_points;
    let hand = &mut runtime.hand;
    let pending_boosts = &mut runtime.pending_boosts;
    let pending_ko = &mut runtime.pending_ko;
    let dbs = &runtime.dbs;
    let battle_rules = &runtime.battle_rules;
    let formula_rules = &runtime.formula_rules;
    let accuracy_rng = &mut runtime.accuracy_rng;
    let player_team = &runtime.player_team;
    let enemy_team = &mut runtime.enemy_team;
    let battle_log = &mut runtime.battle_log;
    let battle_result = &mut runtime.battle_result;
    let next_game_state = &mut runtime.next_game_state;
    let ai_config = &runtime.ai_config;

    if ai_state.0 > 0.0 {
        ai_state.0 = (ai_state.0 - time.delta_secs()).max(0.0);
        return;
    }

    if turn_ctx.enemy_ended {
        ai_state.0 = 0.0;
        ai_state.1 = false;
        ai_state.2 = false;
        return;
    }

    if !ai_state.1 {
        ai_state.1 = true;
        ai_state.0 = ENEMY_AI_INITIAL_DELAY;
        return;
    }

    if pending_tactical_discard
        .as_ref()
        .is_some_and(|pending| pending.side == Side::Enemy)
    {
        if hand.enemy.is_empty() {
            commands.remove_resource::<PendingTacticalDiscard>();
            return;
        }
        let discard_choice = choose_enemy_discard_card(&hand.enemy, &dbs, &ai_config.weights);
        let discard_score = discard_choice.as_ref().map(|choice| choice.score);
        let card_id = hand
            .enemy
            .remove(discard_choice.map(|choice| choice.index).unwrap_or(0));
        action_points.enemy += 1;
        let card_name = dbs
            .cards
            .get(&card_id)
            .map(|c| c.name.to_string())
            .unwrap_or_else(|| format!("{card_id:?}"));
        writers.event_writer.write(BattleEvent::CardDiscarded {
            side: Side::Enemy,
            card_name: card_name.clone(),
        });
        note_action_phase(
            &mut logs.structured_log,
            logs.turn_count.0,
            Side::Enemy,
            "敌方战术整理",
            format!(
                "弃置卡牌={}；保留评分={:.2}；获得AP=1；当前AP={}",
                card_name,
                discard_score.unwrap_or(0.0),
                action_points.enemy
            ),
        );
        push_named_action_trace(
            &mut logs.action_trace,
            logs.turn_count.0,
            Side::Enemy,
            "tactical_discard",
            format!(
                "弃置卡牌={}；保留评分={:.2}；当前AP={}",
                card_name,
                discard_score.unwrap_or(0.0),
                action_points.enemy
            ),
        );
        ai_state.0 = ENEMY_AI_ACTION_DELAY;
        return;
    }

    let Some(p_entity) = player_team.0.active_combatant() else {
        abort_battle(
            "敌方 AI：玩家上场精灵无效。",
            battle_log,
            battle_result,
            next_game_state,
        );
        return;
    };
    let Some(e_entity) = enemy_team.0.active_combatant() else {
        abort_battle(
            "敌方 AI：敌方上场精灵无效。",
            battle_log,
            battle_result,
            next_game_state,
        );
        return;
    };

    if action_points.enemy <= 0 {
        console_log(
            ConsoleLogCategory::Ai,
            format!("[round {}][enemy] 结束回合：无可用 AP", logs.turn_count.0),
        );
        note_action_phase(
            &mut logs.structured_log,
            logs.turn_count.0,
            Side::Enemy,
            "敌方结束回合",
            format!("敌方无可用AP；剩余AP={}", action_points.enemy),
        );
        push_named_action_trace(
            &mut logs.action_trace,
            logs.turn_count.0,
            Side::Enemy,
            "end_turn",
            format!("敌方无可用AP；剩余AP={}", action_points.enemy),
        );
        finalize_enemy_turn(
            e_entity,
            enemy_team,
            &mut turn_ctx,
            &round_order,
            pending_boosts,
            action_points,
            hand,
            battle_rules,
            &mut commands,
            &formula_rules,
            &mut logs,
            &mut writers,
            &mut next_phase,
            &mut exec_query,
            &mut ai_state,
        );
        return;
    }

    let mut acted_this_update = false;
    while action_points.enemy > 0 {
        let should_go_check_end = exec_query
            .get(p_entity)
            .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false)
            || exec_query
                .get(e_entity)
                .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false);
        if should_go_check_end {
            pending_ko.resume_phase = Some(BattlePhase::EnemyTurn);
            ai_state.0 = 0.0;
            ai_state.1 = false;
            ai_state.2 = false;
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }

        let Ok((_, p_combatant, p_stats, p_skills, p_skill_count, p_shield, p_statuses, p_aura, _)) =
            exec_query.get(p_entity)
        else {
            break;
        };
        let Ok((_, e_combatant, e_stats, e_skills, e_skill_count, e_shield, e_statuses, e_aura, _)) =
            exec_query.get(e_entity)
        else {
            break;
        };

        let p_element = p_combatant.element;
        let p_atk = p_stats.atk;
        let p_skills_arr = p_skills.0;
        let p_skill_count = p_skill_count.0;
        let e_hp = e_stats.hp;
        let e_max_hp = e_stats.max_hp;
        let e_shield_value = e_shield.0;
        let e_atk = e_stats.atk;
        let e_def = e_stats.def;
        let e_skills_arr = e_skills.0;
        let e_skill_count = e_skill_count.0;
        let p_hp = p_stats.hp;
        let p_def = p_stats.def;
        let p_shield_value = p_shield.0;
        let p_attached_aura = p_aura.slots;
        let p_status_ids = p_statuses
            .entries
            .iter()
            .map(|entry| entry.id.clone())
            .collect();
        let enemy_has_aura = e_aura.primary().is_some();
        let enemy_has_cleansable_debuff = e_statuses.entries.iter().any(|entry| {
            matches!(
                entry.category,
                StatusCategory::Debuff | StatusCategory::Special
            )
        });

        let ai_ctx = EnemyAiContext {
            enemy_hp: e_hp,
            enemy_max_hp: e_max_hp,
            enemy_shield: e_shield_value,
            enemy_atk: e_atk,
            enemy_def: e_def,
            enemy_atk_stage: e_stats.atk_stage,
            enemy_def_stage: e_stats.def_stage,
            enemy_spd_stage: e_stats.spd_stage,
            enemy_acc_stage: e_stats.acc_stage,
            enemy_element: e_combatant.element,
            enemy_attached_auras: e_aura.slots,
            enemy_status_ids: e_statuses
                .entries
                .iter()
                .map(|entry| entry.id.clone())
                .collect(),
            enemy_has_aura,
            enemy_has_cleansable_debuff,
            player_atk_stage: p_stats.atk_stage,
            player_def_stage: p_stats.def_stage,
            player_spd_stage: p_stats.spd_stage,
            player_acc_stage: p_stats.acc_stage,
            player_def: p_def,
            player_hp: p_hp,
            player_shield: p_shield_value,
            target_element: p_element,
            target_attached_auras: p_attached_aura,
            target_status_ids: p_status_ids,
        };
        let player_info_visible = !matches!(
            ai_config.player_info_visibility,
            AiPlayerInfoVisibility::None
        );
        let high_difficulty = matches!(
            ai_config.difficulty,
            AiDifficulty::Hard | AiDifficulty::Expert
        );
        let include_player_hand = matches!(
            ai_config.player_info_visibility,
            AiPlayerInfoVisibility::Full
        );
        let player_threat = player_info_visible.then(|| {
            build_player_threat_context(
                p_skills_arr,
                p_skill_count,
                p_atk,
                action_points.player,
                &hand.player,
                high_difficulty && include_player_hand,
                &dbs,
            )
        });
        let advanced_player_threat = high_difficulty.then_some(player_threat).flatten();

        if log_enabled(ConsoleLogCategory::AiDetail) {
            console_log(
                ConsoleLogCategory::AiDetail,
                format!(
                    "[round {}][enemy] AI配置：难度={:?}；搜索深度={}；候选数={}；换人阈值={:.2}；权重={:?}",
                    logs.turn_count.0,
                    ai_config.difficulty,
                    ai_config.search_depth,
                    ai_config.top_candidates,
                    ai_config.switch_score_threshold,
                    ai_config.weights
                ),
            );
            if let Some(threat) = advanced_player_threat {
                console_log(
                    ConsoleLogCategory::AiDetail,
                    format!(
                        "[round {}][enemy] 高难度玩家威胁：玩家AP={}；预测AP={}；攻击加成={}；固定伤害加成={}；防守潜力={:.2}",
                        logs.turn_count.0,
                        threat.current_ap,
                        threat.projected_ap,
                        threat.attack_bonus,
                        threat.fixed_damage_bonus,
                        threat.defensive_value
                    ),
                );
            }
            console_log(
                ConsoleLogCategory::AiDetail,
                format!(
                    "[round {}][enemy] 技能候选列表：{}",
                    logs.turn_count.0,
                    enemy_skill_candidate_report(
                        &e_skills_arr,
                        e_skill_count,
                        action_points.enemy,
                        &dbs,
                        &ai_ctx,
                        &ai_config.weights
                    )
                ),
            );
        }
        let chosen_skill = choose_enemy_skill_with_threat(
            &e_skills_arr,
            e_skill_count,
            action_points.enemy,
            &dbs,
            &ai_ctx,
            &ai_config.weights,
            advanced_player_threat.as_ref(),
        );
        if let Some(candidate) = chosen_skill
            && let Some(skill) = dbs.skills.get(&candidate.skill_id)
        {
            console_log(
                ConsoleLogCategory::AiDetail,
                format!(
                    "[round {}][enemy] 技能候选最优：{}；槽位={}；类型={:?}；得分={:.2}；当前AP={}；敌方HP={}/{}；玩家HP={}；玩家护盾={}",
                    logs.turn_count.0,
                    skill.name,
                    candidate.slot,
                    candidate.kind,
                    candidate.score,
                    action_points.enemy,
                    e_hp,
                    e_max_hp,
                    p_hp,
                    p_shield_value
                ),
            );
        }
        let current_switch_candidate = EnemySwitchCandidate {
            index: enemy_team.0.active_index,
            hp: e_hp,
            max_hp: e_max_hp,
            shield: e_shield_value,
            atk: e_atk,
            def: e_def,
            atk_stage: e_stats.atk_stage,
            def_stage: e_stats.def_stage,
            spd_stage: e_stats.spd_stage,
            acc_stage: e_stats.acc_stage,
            element: e_combatant.element,
            attached_auras: e_aura.slots,
            skill_ids: e_skills_arr,
            skill_count: e_skill_count,
            status_ids: e_statuses
                .entries
                .iter()
                .map(|entry| entry.id.clone())
                .collect(),
            has_aura: enemy_has_aura,
            has_cleansable_debuff: enemy_has_cleansable_debuff,
        };
        let current_best_action = best_action_value(
            &current_switch_candidate,
            action_points.enemy,
            &dbs,
            &ai_ctx,
            &ai_config.weights,
        );
        let enemy_switch_candidates = enemy_team
            .0
            .combatants
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, entity)| {
                let Ok((_, combatant, stats, skills, skill_count, shield, statuses, aura, _)) =
                    exec_query.get(entity)
                else {
                    return None;
                };
                Some(EnemySwitchCandidate {
                    index,
                    hp: stats.hp,
                    max_hp: stats.max_hp,
                    shield: shield.0,
                    atk: stats.atk,
                    def: stats.def,
                    atk_stage: stats.atk_stage,
                    def_stage: stats.def_stage,
                    spd_stage: stats.spd_stage,
                    acc_stage: stats.acc_stage,
                    element: combatant.element,
                    attached_auras: aura.slots,
                    skill_ids: skills.0,
                    skill_count: skill_count.0,
                    status_ids: statuses
                        .entries
                        .iter()
                        .map(|entry| entry.id.clone())
                        .collect(),
                    has_aura: aura.primary().is_some(),
                    has_cleansable_debuff: statuses.entries.iter().any(|entry| {
                        matches!(
                            entry.category,
                            StatusCategory::Debuff | StatusCategory::Special
                        )
                    }),
                })
            })
            .collect::<Vec<_>>();
        let switch_context = Some((
            &current_switch_candidate,
            enemy_switch_candidates.as_slice(),
            current_best_action,
            ai_state.2,
            ai_config.switch_score_threshold,
            player_threat,
        ));
        if log_enabled(ConsoleLogCategory::AiDetail) {
            console_log(
                ConsoleLogCategory::AiDetail,
                format!(
                    "[round {}][enemy] 换人候选列表：{}",
                    logs.turn_count.0,
                    enemy_switch_candidate_report(
                        &current_switch_candidate,
                        &enemy_switch_candidates,
                        current_best_action,
                        action_points.enemy,
                        &dbs,
                        &ai_ctx,
                        ai_state.2,
                        ai_config.switch_score_threshold,
                        &ai_config.weights,
                        player_threat.as_ref()
                    )
                ),
            );
        }
        let plan_candidates = choose_enemy_plan_candidates(
            &hand.enemy,
            action_points.enemy,
            &e_skills_arr,
            e_skill_count,
            &dbs,
            &ai_ctx,
            switch_context,
            &ai_config.weights,
            advanced_player_threat.as_ref(),
            ai_config.search_depth,
            ai_config.top_candidates,
        );
        let planned_action = plan_candidates.first().cloned();
        if log_enabled(ConsoleLogCategory::AiDetail) {
            console_log(
                ConsoleLogCategory::AiDetail,
                format!(
                    "[round {}][enemy] Top行动规划：{}",
                    logs.turn_count.0,
                    plan_candidates
                        .iter()
                        .enumerate()
                        .map(|(index, plan)| format!(
                            "#{} {}；总分={:.2}",
                            index + 1,
                            plan.summary,
                            plan.score
                        ))
                        .collect::<Vec<_>>()
                        .join(" | ")
                ),
            );
        }

        let planned_card_skill = match planned_action.as_ref().map(|plan| &plan.action) {
            Some(EnemyPlannedAction::UseCardForSkill { card_index, skill }) => {
                let _planned_card_index = *card_index;
                Some(*skill)
            }
            _ => None,
        };
        let mut played_card = false;
        if let Some(chosen_skill) = planned_card_skill {
            played_card = try_play_boost_card_for_skill(
                chosen_skill,
                hand,
                action_points,
                pending_boosts,
                &dbs,
                &ai_ctx,
                &ai_config.weights,
                &mut writers.event_writer,
            );
        }

        if played_card {
            if let Some(chosen_skill) = planned_card_skill {
                console_log(
                    ConsoleLogCategory::Ai,
                    format!(
                        "[round {}][enemy] 先使用辅助卡辅助技能槽位{}；当前AP={}",
                        logs.turn_count.0, chosen_skill.slot, action_points.enemy
                    ),
                );
                note_action_phase(
                    &mut logs.structured_log,
                    logs.turn_count.0,
                    Side::Enemy,
                    "敌方使用卡牌",
                    format!(
                        "为技能槽位{}预先使用辅助卡；当前AP={}",
                        chosen_skill.slot, action_points.enemy
                    ),
                );
                push_named_action_trace(
                    &mut logs.action_trace,
                    logs.turn_count.0,
                    Side::Enemy,
                    "use_card",
                    format!(
                        "为技能槽位{}预先使用辅助卡；当前AP={}",
                        chosen_skill.slot, action_points.enemy
                    ),
                );
            }
            acted_this_update = true;
            break;
        }

        if let Some(EnemyPlannedAction::Switch(chosen_switch)) =
            planned_action.as_ref().map(|plan| &plan.action)
        {
            let current_entity = enemy_team.0.combatants[enemy_team.0.active_index];
            let target_entity = enemy_team.0.combatants[chosen_switch.index];
            let Ok(
                [
                    (_, _, mut current_stats, _, _, _, mut current_statuses, _, _),
                    (_, _, target_stats, _, _, _, target_statuses, _, name),
                ],
            ) = exec_query.get_many_mut([current_entity, target_entity])
            else {
                break;
            };
            if target_stats.hp > 0 {
                transfer_status_by_id(
                    &mut current_statuses,
                    &mut current_stats,
                    target_statuses.into_inner(),
                    target_stats.into_inner(),
                    "nature_regen",
                );
                action_points.enemy -= 1;
                enemy_team.0.active_index = chosen_switch.index;
                writers.event_writer.write(BattleEvent::Switched {
                    side: Side::Enemy,
                    name: name.to_string(),
                });
                console_log(
                    ConsoleLogCategory::Ai,
                    format!(
                        "[round {}][enemy] 选择换人：{}；得分={:.2}；当前AP={}",
                        logs.turn_count.0, name, chosen_switch.score, action_points.enemy
                    ),
                );
                note_action_phase(
                    &mut logs.structured_log,
                    logs.turn_count.0,
                    Side::Enemy,
                    "敌方主动换人",
                    format!(
                        "切换到 {}；换人评分={:.2}；敌方AP={}",
                        name, chosen_switch.score, action_points.enemy
                    ),
                );
                push_turn_action_trace(
                    &mut logs.action_trace,
                    logs.turn_count.0,
                    Side::Enemy,
                    TurnAction::Switch,
                    format!(
                        "切换到 {}；换人评分={:.2}；剩余AP={}",
                        name, chosen_switch.score, action_points.enemy
                    ),
                );
                ai_state.2 = true;
                acted_this_update = true;
                break;
            }
        }

        let planned_skill = match planned_action.as_ref().map(|plan| &plan.action) {
            Some(EnemyPlannedAction::UseSkill(skill)) => Some(*skill),
            _ => None,
        };
        if let Some(chosen_skill) = planned_skill {
            let slot = chosen_skill.slot;
            let skill_id = chosen_skill.skill_id;
            let Some(skill) = dbs.skills.get(&skill_id) else {
                // 没技能直接跳过
                break;
            };
            let cost = skill.cost_ap;
            if action_points.enemy < cost {
                break;
            }

            action_points.enemy -= cost;

            // 技能使用事件
            writers.event_writer.write(BattleEvent::SkillUsed {
                side: Side::Enemy,
                skill_name: skill.name.clone(),
                slot,
            });
            console_log(
                ConsoleLogCategory::Ai,
                format!(
                    "[round {}][enemy] 选择技能：{}；槽位={}；得分={:.2}；AP {} -> {}",
                    logs.turn_count.0,
                    skill.name,
                    slot,
                    chosen_skill.score,
                    action_points.enemy + cost,
                    action_points.enemy
                ),
            );
            note_action_phase(
                &mut logs.structured_log,
                logs.turn_count.0,
                Side::Enemy,
                "敌方使用技能",
                format!(
                    "技能={}；槽位={}；得分={:.2}；消耗AP={}；剩余AP={}",
                    skill.name, slot, chosen_skill.score, cost, action_points.enemy
                ),
            );
            push_turn_action_trace(
                &mut logs.action_trace,
                logs.turn_count.0,
                Side::Enemy,
                TurnAction::Skill(skill_id),
                format!(
                    "技能={}；槽位={}；得分={:.2}；剩余AP={}",
                    skill.name, slot, chosen_skill.score, action_points.enemy
                ),
            );

            if skill_execution_mode(skill) == SkillExecutionMode::WindSpread {
                let player_backs = player_team
                    .0
                    .combatants
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(index, entity)| {
                        (index != player_team.0.active_index && entity != p_entity)
                            .then_some(entity)
                    })
                    .collect::<Vec<_>>();

                match player_backs.as_slice() {
                    [back_a, back_b, ..] => {
                        let Ok(
                            [
                                (
                                    _,
                                    _e_combatant,
                                    mut e_stats_m,
                                    _,
                                    _,
                                    mut e_shield_m,
                                    mut e_statuses_m,
                                    _e_aura_m,
                                    _,
                                ),
                                (
                                    _,
                                    p_combatant,
                                    mut p_stats_m,
                                    _,
                                    _,
                                    mut p_shield_m,
                                    mut p_statuses_m,
                                    mut p_aura_m,
                                    _,
                                ),
                                (
                                    _,
                                    back_a_combatant,
                                    mut back_a_stats,
                                    _,
                                    _,
                                    mut back_a_shield,
                                    mut back_a_statuses,
                                    mut back_a_aura,
                                    _,
                                ),
                                (
                                    _,
                                    back_b_combatant,
                                    mut back_b_stats,
                                    _,
                                    _,
                                    mut back_b_shield,
                                    mut back_b_statuses,
                                    mut back_b_aura,
                                    _,
                                ),
                            ],
                        ) = exec_query.get_many_mut([e_entity, p_entity, *back_a, *back_b])
                        else {
                            break;
                        };

                        apply_wind_effect(
                            skill,
                            Side::Enemy,
                            Side::Player,
                            &mut e_stats_m,
                            &mut e_shield_m,
                            &mut e_statuses_m,
                            WindSpreadTarget {
                                base_element: p_combatant.element,
                                stats: &mut p_stats_m,
                                shield: &mut p_shield_m,
                                aura: &mut p_aura_m,
                                statuses: &mut p_statuses_m,
                            },
                            Some(WindSpreadTarget {
                                base_element: back_a_combatant.element,
                                stats: &mut back_a_stats,
                                shield: &mut back_a_shield,
                                aura: &mut back_a_aura,
                                statuses: &mut back_a_statuses,
                            }),
                            Some(WindSpreadTarget {
                                base_element: back_b_combatant.element,
                                stats: &mut back_b_stats,
                                shield: &mut back_b_shield,
                                aura: &mut back_b_aura,
                                statuses: &mut back_b_statuses,
                            }),
                            pending_boosts,
                            &formula_rules,
                            accuracy_rng,
                            &dbs.elements,
                            &dbs.statuses,
                            &dbs.reactions,
                            &mut writers.event_writer,
                            Some(&mut writers.formula_writer),
                            Some(&mut writers.status_writer),
                            Some(&mut logs.structured_log),
                            Some(logs.turn_count.0),
                            None,
                        );
                    }
                    [back_a] => {
                        let Ok(
                            [
                                (
                                    _,
                                    _e_combatant,
                                    mut e_stats_m,
                                    _,
                                    _,
                                    mut e_shield_m,
                                    mut e_statuses_m,
                                    _e_aura_m,
                                    _,
                                ),
                                (
                                    _,
                                    p_combatant,
                                    mut p_stats_m,
                                    _,
                                    _,
                                    mut p_shield_m,
                                    mut p_statuses_m,
                                    mut p_aura_m,
                                    _,
                                ),
                                (
                                    _,
                                    back_a_combatant,
                                    mut back_a_stats,
                                    _,
                                    _,
                                    mut back_a_shield,
                                    mut back_a_statuses,
                                    mut back_a_aura,
                                    _,
                                ),
                            ],
                        ) = exec_query.get_many_mut([e_entity, p_entity, *back_a])
                        else {
                            break;
                        };

                        apply_wind_effect(
                            skill,
                            Side::Enemy,
                            Side::Player,
                            &mut e_stats_m,
                            &mut e_shield_m,
                            &mut e_statuses_m,
                            WindSpreadTarget {
                                base_element: p_combatant.element,
                                stats: &mut p_stats_m,
                                shield: &mut p_shield_m,
                                aura: &mut p_aura_m,
                                statuses: &mut p_statuses_m,
                            },
                            Some(WindSpreadTarget {
                                base_element: back_a_combatant.element,
                                stats: &mut back_a_stats,
                                shield: &mut back_a_shield,
                                aura: &mut back_a_aura,
                                statuses: &mut back_a_statuses,
                            }),
                            None,
                            pending_boosts,
                            &formula_rules,
                            accuracy_rng,
                            &dbs.elements,
                            &dbs.statuses,
                            &dbs.reactions,
                            &mut writers.event_writer,
                            Some(&mut writers.formula_writer),
                            Some(&mut writers.status_writer),
                            Some(&mut logs.structured_log),
                            Some(logs.turn_count.0),
                            None,
                        );
                    }
                    _ => {
                        let Ok(
                            [
                                (
                                    _,
                                    _e_combatant,
                                    mut e_stats_m,
                                    _,
                                    _,
                                    mut e_shield_m,
                                    mut e_statuses_m,
                                    _e_aura_m,
                                    _,
                                ),
                                (
                                    _,
                                    p_combatant,
                                    mut p_stats_m,
                                    _,
                                    _,
                                    mut p_shield_m,
                                    mut p_statuses_m,
                                    mut p_aura_m,
                                    _,
                                ),
                            ],
                        ) = exec_query.get_many_mut([e_entity, p_entity])
                        else {
                            break;
                        };

                        apply_wind_effect(
                            skill,
                            Side::Enemy,
                            Side::Player,
                            &mut e_stats_m,
                            &mut e_shield_m,
                            &mut e_statuses_m,
                            WindSpreadTarget {
                                base_element: p_combatant.element,
                                stats: &mut p_stats_m,
                                shield: &mut p_shield_m,
                                aura: &mut p_aura_m,
                                statuses: &mut p_statuses_m,
                            },
                            None,
                            None,
                            pending_boosts,
                            &formula_rules,
                            accuracy_rng,
                            &dbs.elements,
                            &dbs.statuses,
                            &dbs.reactions,
                            &mut writers.event_writer,
                            Some(&mut writers.formula_writer),
                            Some(&mut writers.status_writer),
                            Some(&mut logs.structured_log),
                            Some(logs.turn_count.0),
                            None,
                        );
                    }
                }
            } else {
                let Ok(
                    [
                        (
                            _,
                            _attacker_combatant,
                            mut e_stats_m,
                            _,
                            _,
                            mut e_shield_m,
                            mut e_statuses_m,
                            mut e_aura_m,
                            _,
                        ),
                        (
                            _,
                            _p_combatant,
                            mut p_stats_m,
                            _,
                            _,
                            mut p_shield_m,
                            mut p_statuses_m,
                            mut p_aura_m,
                            _,
                        ),
                    ],
                ) = exec_query.get_many_mut([e_entity, p_entity])
                else {
                    break;
                };
                match skill_target_mode(skill) {
                    SkillTargetMode::SelfOnly => apply_self_effect(
                        skill,
                        Side::Enemy,
                        &mut e_stats_m,
                        &mut e_shield_m,
                        &mut e_aura_m,
                        &mut e_statuses_m,
                        pending_boosts,
                        &formula_rules,
                        accuracy_rng,
                        &dbs.elements,
                        &dbs.statuses,
                        &dbs.reactions,
                        &mut writers.event_writer,
                        Some(&mut writers.formula_writer),
                        Some(&mut writers.status_writer),
                        Some(&mut logs.structured_log),
                        Some(logs.turn_count.0),
                    ),
                    SkillTargetMode::Opponent => apply_effect(
                        skill,
                        Side::Enemy,
                        Side::Player,
                        p_element,
                        &mut e_stats_m,
                        &mut e_shield_m,
                        &mut e_statuses_m,
                        &mut p_stats_m,
                        &mut p_shield_m,
                        &mut p_aura_m,
                        &mut p_statuses_m,
                        pending_boosts,
                        &formula_rules,
                        accuracy_rng,
                        &dbs.elements,
                        &dbs.statuses,
                        &dbs.reactions,
                        &mut writers.event_writer,
                        Some(&mut writers.formula_writer),
                        Some(&mut writers.status_writer),
                        Some(&mut logs.structured_log),
                        Some(logs.turn_count.0),
                    ),
                };
            }

            let should_go_check_end = exec_query
                .get(p_entity)
                .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false)
                || exec_query
                    .get(e_entity)
                    .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
                    .unwrap_or(false);
            if should_go_check_end {
                pending_ko.resume_phase = Some(BattlePhase::EnemyTurn);
                ai_state.0 = 0.0;
                ai_state.1 = false;
                ai_state.2 = false;
                next_phase.set(BattlePhase::CheckEnd);
                return;
            }
            acted_this_update = true;
            break;
        } else {
            // 规划器未选择技能时，按规划使用资源卡或弃牌换 AP。
            let planned_immediate_card = match planned_action.as_ref().map(|plan| &plan.action) {
                Some(EnemyPlannedAction::ImmediateCard(card)) => Some(card.clone()),
                _ => None,
            };
            if let Some(immediate_card) = planned_immediate_card {
                let card_id = hand.enemy.remove(immediate_card.index);
                let (card_name, card_cost) = dbs
                    .cards
                    .get(&card_id)
                    .map(|card| (card.name.to_string(), card.cost_ap))
                    .unwrap_or_else(|| (format!("{card_id:?}"), 0));
                action_points.enemy -= card_cost;
                writers.event_writer.write(BattleEvent::CardUsed {
                    side: Side::Enemy,
                    card_name: card_name.clone(),
                });
                console_log(
                    ConsoleLogCategory::Ai,
                    format!(
                        "[round {}][enemy] 无可用技能，先使用资源卡：{}；卡牌评分={:.2}；AP {} -> {}",
                        logs.turn_count.0,
                        card_name,
                        immediate_card.score,
                        action_points.enemy + card_cost,
                        action_points.enemy
                    ),
                );
                note_action_phase(
                    &mut logs.structured_log,
                    logs.turn_count.0,
                    Side::Enemy,
                    "敌方使用卡牌",
                    format!(
                        "卡牌={}；评分={:.2}；消耗AP={}；当前AP={}",
                        card_name, immediate_card.score, card_cost, action_points.enemy
                    ),
                );
                push_named_action_trace(
                    &mut logs.action_trace,
                    logs.turn_count.0,
                    Side::Enemy,
                    "use_card",
                    format!(
                        "卡牌={}；评分={:.2}；当前AP={}",
                        card_name, immediate_card.score, action_points.enemy
                    ),
                );
                acted_this_update = true;
                break;
            }

            if !hand.enemy.is_empty() {
                let discard_choice = match planned_action.as_ref().map(|plan| &plan.action) {
                    Some(EnemyPlannedAction::Discard(discard)) => Some(discard.clone()),
                    _ => None,
                };
                if discard_choice.is_none() {
                    break;
                }
                let discard_index = discard_choice
                    .as_ref()
                    .map(|choice| choice.index)
                    .unwrap_or(0);
                let keep_score = discard_choice
                    .as_ref()
                    .map(|choice| choice.keep_score)
                    .unwrap_or(0.0);
                let followup_score = discard_choice
                    .as_ref()
                    .map(|choice| choice.followup_score)
                    .unwrap_or(0.0);
                let followup = discard_choice
                    .as_ref()
                    .map(|choice| format!("{:?}", choice.followup))
                    .unwrap_or_else(|| "None".to_string());
                let sequence_score = discard_choice
                    .as_ref()
                    .map(|choice| choice.score)
                    .unwrap_or(0.0);
                let card_id = hand.enemy.remove(discard_index);
                action_points.enemy += 1;
                let card_name = dbs
                    .cards
                    .get(&card_id)
                    .map(|c| c.name.to_string())
                    .unwrap_or_else(|| format!("{card_id:?}"));
                writers.event_writer.write(BattleEvent::CardDiscarded {
                    side: Side::Enemy,
                    card_name: card_name.clone(),
                });
                note_action_phase(
                    &mut logs.structured_log,
                    logs.turn_count.0,
                    Side::Enemy,
                    "敌方弃牌",
                    format!(
                        "弃置卡牌={}；序列评分={:.2}；保留评分={:.2}；后续={}; 后续评分={:.2}；获得AP=1；当前AP={}",
                        card_name,
                        sequence_score,
                        keep_score,
                        followup,
                        followup_score,
                        action_points.enemy
                    ),
                );
                push_named_action_trace(
                    &mut logs.action_trace,
                    logs.turn_count.0,
                    Side::Enemy,
                    "discard_card",
                    format!(
                        "弃置卡牌={}；序列评分={:.2}；保留评分={:.2}；后续={}; 后续评分={:.2}；当前AP={}",
                        card_name,
                        sequence_score,
                        keep_score,
                        followup,
                        followup_score,
                        action_points.enemy
                    ),
                );
                acted_this_update = true;
                break;
            } else {
                break;
            }
        }
    }

    if acted_this_update && action_points.enemy > 0 {
        ai_state.0 = ENEMY_AI_ACTION_DELAY; // 敌方每两个操作之间间隔 0.75 秒。
        return;
    }

    note_action_phase(
        &mut logs.structured_log,
        logs.turn_count.0,
        Side::Enemy,
        "敌方结束回合",
        format!("敌方行动完毕；剩余AP={}", action_points.enemy),
    );
    push_named_action_trace(
        &mut logs.action_trace,
        logs.turn_count.0,
        Side::Enemy,
        "end_turn",
        format!("敌方行动完毕；剩余AP={}", action_points.enemy),
    );
    finalize_enemy_turn(
        e_entity,
        enemy_team,
        &mut turn_ctx,
        &round_order,
        pending_boosts,
        action_points,
        hand,
        battle_rules,
        &mut commands,
        &formula_rules,
        &mut logs,
        &mut writers,
        &mut next_phase,
        &mut exec_query,
        &mut ai_state,
    );
}
