use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    battle::{
        ActionPoints, ActionTrace, BattleEvent, BattleFormulaEvent, BattleLog, BattleResult,
        BattleStatusEvent, Combatant, ElementAura, Hand, InBattle, PendingBoosts, RoundOrder,
        Shield, Side, SkillCount, SkillList, Stats, StructuredBattleLog, TurnAction, TurnContext,
        TurnCount, next_phase_after_side_end, note_action_phase, push_named_action_trace,
        push_turn_action_trace,
    },
    data::{BattleDbs, CardEffect, SkillCategory, SkillDef, SkillEffect, SkillId},
    game_state::{BattlePhase, GameState},
};

use super::{
    SkillExecutionMode, SkillTargetMode, WindSpreadTarget, abort_battle, apply_effect,
    apply_self_effect, apply_wind_effect, process_side_end_statuses, skill_execution_mode,
    skill_target_mode,
};

const ENEMY_AI_INITIAL_DELAY: f32 = 0.35;
const ENEMY_AI_ACTION_DELAY: f32 = 1.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnemyAiSkillKind {
    Attack,
    Heal,
    Shield,
    Debuff,
}

#[derive(Debug, Clone)]
struct EnemyAiContext {
    enemy_hp: i32,
    enemy_max_hp: i32,
    enemy_shield: i32,
    enemy_atk: i32,
    enemy_has_aura: bool,
    enemy_has_cleansable_debuff: bool,
    player_def: i32,
    player_hp: i32,
    player_shield: i32,
    target_element: crate::data::ElementType,
    target_attached_auras: [Option<crate::data::ElementType>; 2],
    target_status_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct ScoredEnemySkill {
    slot: usize,
    skill_id: SkillId,
    score: f32,
    kind: EnemyAiSkillKind,
}

fn enemy_hp_ratio(ctx: &EnemyAiContext) -> f32 {
    if ctx.enemy_max_hp <= 0 {
        0.0
    } else {
        ctx.enemy_hp.max(0) as f32 / ctx.enemy_max_hp as f32
    }
}

fn primary_effect(effect: &SkillEffect) -> Option<&SkillEffect> {
    match effect {
        SkillEffect::Sequence { effects } => effects.first().and_then(primary_effect),
        SkillEffect::Conditional { .. } => None,
        _ => Some(effect),
    }
}

fn primary_attack_power(effect: &SkillEffect) -> Option<i32> {
    match primary_effect(effect)? {
        SkillEffect::Attack { power, .. } => Some(*power),
        _ => None,
    }
}

fn condition_matches_precast(
    condition: &crate::data::SkillCondition,
    ctx: &EnemyAiContext,
) -> bool {
    match condition {
        crate::data::SkillCondition::TargetHadAura { element } => ctx
            .target_attached_auras
            .iter()
            .flatten()
            .any(|aura| aura == element),
        crate::data::SkillCondition::TargetHadStatus { status_id } => {
            ctx.target_status_ids.iter().any(|id| id == status_id)
        }
        crate::data::SkillCondition::TargetHadNoShield => ctx.player_shield <= 0,
        crate::data::SkillCondition::Any { conditions } => conditions
            .iter()
            .any(|nested| condition_matches_precast(nested, ctx)),
        crate::data::SkillCondition::All { conditions } => conditions
            .iter()
            .all(|nested| condition_matches_precast(nested, ctx)),
        crate::data::SkillCondition::LastReactionName { .. }
        | crate::data::SkillCondition::LastWindSpreadSucceeded
        | crate::data::SkillCondition::LastWindSpreadFailed
        | crate::data::SkillCondition::LastCleanseSucceeded
        | crate::data::SkillCondition::LastCleanseFailed
        | crate::data::SkillCondition::LastTargetFainted => false,
    }
}

fn conditional_bonus_score(effect: &SkillEffect, ctx: &EnemyAiContext) -> f32 {
    match effect {
        SkillEffect::Conditional { branches } => branches
            .iter()
            .filter(|branch| condition_matches_precast(&branch.condition, ctx))
            .map(|branch| match branch.effect.as_ref() {
                SkillEffect::DealFixedDamage { amount, .. } => *amount as f32,
                SkillEffect::DealStatDifferenceDamage { .. } => 10.0,
                SkillEffect::ModifyStages { .. }
                | SkillEffect::ApplyStatus { .. }
                | SkillEffect::Dispel { .. } => 8.0,
                SkillEffect::Heal { amount } => (*amount as f32) * 0.6,
                SkillEffect::Shield { amount } => (*amount as f32) * 0.4,
                _ => 0.0,
            })
            .sum(),
        _ => 0.0,
    }
}

fn cleanse_value(
    prefer_aura: bool,
    fallback_to_debuff: bool,
    amount: usize,
    ctx: &EnemyAiContext,
) -> f32 {
    let mut value = 0.0;
    let mut remaining = amount;
    let mut aura_available = ctx.enemy_has_aura;
    let mut debuff_available = ctx.enemy_has_cleansable_debuff;

    while remaining > 0 {
        if prefer_aura && aura_available {
            value += 18.0;
            aura_available = false;
            remaining -= 1;
            continue;
        }
        if fallback_to_debuff && debuff_available {
            value += 14.0;
            debuff_available = false;
            remaining -= 1;
            continue;
        }
        break;
    }

    value
}

fn estimate_attack_value(skill: &SkillDef, ctx: &EnemyAiContext, dbs: &BattleDbs) -> f32 {
    let Some(power) = primary_attack_power(&skill.effect) else {
        return 0.0;
    };

    let raw = (power + ctx.enemy_atk - ctx.player_def).max(1) as f32;
    let effectiveness = if let Some(skill_element) = skill.element {
        let defender_element = if ctx.player_shield > 0 {
            ctx.target_element
        } else {
            ctx.target_attached_auras
                .iter()
                .flatten()
                .copied()
                .next()
                .unwrap_or(ctx.target_element)
        };
        dbs.elements
            .get_effectiveness(skill_element, defender_element)
    } else {
        1.0
    };

    let theoretical_damage = (raw * effectiveness).max(1.0);
    let hp_damage = (theoretical_damage - ctx.player_shield.max(0) as f32).max(0.0);
    let mut score = hp_damage;

    if effectiveness > 1.0 {
        score += 18.0 + (effectiveness - 1.0) * 22.0;
    } else if effectiveness < 1.0 {
        score -= 16.0 + (1.0 - effectiveness) * 24.0;
    }

    if hp_damage >= ctx.player_hp.max(0) as f32 {
        score += 28.0;
    }

    score
}

fn score_enemy_skill(
    slot: usize,
    skill_id: SkillId,
    skill: &SkillDef,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
) -> ScoredEnemySkill {
    let hp_ratio = enemy_hp_ratio(ctx);

    match &skill.effect {
        SkillEffect::Attack { .. } => {
            let mut score = estimate_attack_value(skill, ctx, dbs);
            score += 10.0;
            if hp_ratio >= 0.7 {
                score += 12.0;
            } else if hp_ratio <= 0.35 {
                score -= 2.0;
            }
            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind: EnemyAiSkillKind::Attack,
            }
        }
        SkillEffect::Heal { amount } => {
            let missing_hp = (ctx.enemy_max_hp - ctx.enemy_hp).max(0) as f32;
            let effective_heal = (*amount as f32).min(missing_hp);
            let urgency = 1.0 - hp_ratio;
            let mut score = effective_heal * (0.5 + urgency * 1.8);

            if hp_ratio <= 0.25 {
                score += 35.0;
            } else if hp_ratio <= 0.4 {
                score += 18.0;
            } else if hp_ratio >= 0.8 {
                score -= 24.0;
            }

            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind: EnemyAiSkillKind::Heal,
            }
        }
        SkillEffect::Shield { amount } => {
            let mut score = *amount as f32 + 5.0;
            if ctx.enemy_shield <= 0 {
                score += 2.0;
            }
            if hp_ratio <= 0.35 {
                score += 3.0;
            }
            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind: EnemyAiSkillKind::Shield,
            }
        }
        SkillEffect::ApplyStatus { .. }
        | SkillEffect::ModifyStages { .. }
        | SkillEffect::Cleanse { .. }
        | SkillEffect::Dispel { .. }
        | SkillEffect::DealFixedDamage { .. }
        | SkillEffect::DealStatDifferenceDamage { .. }
        | SkillEffect::Conditional { .. } => {
            let mut score = 40.0 + (1.0 - hp_ratio) * 8.0;
            if skill.category == SkillCategory::EnemyDebuff {
                score += 10.0;
            }
            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind: EnemyAiSkillKind::Debuff,
            }
        }
        SkillEffect::Sequence { effects } => {
            let contains_attack = effects
                .iter()
                .any(|effect| matches!(effect, SkillEffect::Attack { .. }));
            let contains_heal = effects
                .iter()
                .any(|effect| matches!(effect, SkillEffect::Heal { .. }));
            let contains_shield = effects
                .iter()
                .any(|effect| matches!(effect, SkillEffect::Shield { .. }));
            let contains_debuff = effects.iter().any(|effect| {
                matches!(
                    effect,
                    SkillEffect::ApplyStatus { .. }
                        | SkillEffect::ModifyStages { .. }
                        | SkillEffect::Cleanse { .. }
                        | SkillEffect::Dispel { .. }
                        | SkillEffect::DealFixedDamage { .. }
                        | SkillEffect::Conditional { .. }
                )
            });
            let conditional_bonus = effects
                .iter()
                .map(|effect| conditional_bonus_score(effect, ctx))
                .sum::<f32>();
            let (score, kind) = match primary_effect(&skill.effect) {
                Some(SkillEffect::Attack { .. }) => (
                    estimate_attack_value(skill, ctx, dbs) + 14.0 + conditional_bonus,
                    EnemyAiSkillKind::Attack,
                ),
                Some(SkillEffect::Heal { amount }) => {
                    let missing_hp = (ctx.enemy_max_hp - ctx.enemy_hp).max(0) as f32;
                    let effective_heal = (*amount as f32).min(missing_hp);
                    let urgency = 1.0 - hp_ratio;
                    let mut score = effective_heal * (0.5 + urgency * 1.8);
                    if hp_ratio <= 0.25 {
                        score += 35.0;
                    } else if hp_ratio <= 0.4 {
                        score += 18.0;
                    } else if hp_ratio >= 0.8 {
                        score -= 24.0;
                    }
                    (score, EnemyAiSkillKind::Heal)
                }
                Some(SkillEffect::Shield { amount }) => {
                    let mut score = *amount as f32 + 5.0;
                    if ctx.enemy_shield <= 0 {
                        score += 2.0;
                    }
                    if hp_ratio <= 0.35 {
                        score += 3.0;
                    }
                    (score, EnemyAiSkillKind::Shield)
                }
                Some(SkillEffect::Cleanse {
                    prefer_aura,
                    fallback_to_debuff,
                    amount,
                }) => {
                    let score = cleanse_value(*prefer_aura, *fallback_to_debuff, *amount, ctx)
                        + 18.0
                        + (1.0 - hp_ratio) * 10.0;
                    (score, EnemyAiSkillKind::Heal)
                }
                Some(SkillEffect::ApplyStatus { .. })
                | Some(SkillEffect::ModifyStages { .. })
                | Some(SkillEffect::Dispel { .. })
                | Some(SkillEffect::DealFixedDamage { .. })
                | Some(SkillEffect::DealStatDifferenceDamage { .. })
                | Some(SkillEffect::Conditional { .. }) => {
                    let mut score = 40.0 + (1.0 - hp_ratio) * 8.0;
                    if skill.category == SkillCategory::EnemyDebuff {
                        score += 10.0;
                    }
                    (score, EnemyAiSkillKind::Debuff)
                }
                Some(SkillEffect::Sequence { .. }) | None => {
                    if contains_attack {
                        (
                            estimate_attack_value(skill, ctx, dbs) + 14.0,
                            EnemyAiSkillKind::Attack,
                        )
                    } else if contains_heal {
                        (32.0 + (1.0 - hp_ratio) * 28.0, EnemyAiSkillKind::Heal)
                    } else if contains_shield {
                        (30.0 + (1.0 - hp_ratio) * 16.0, EnemyAiSkillKind::Shield)
                    } else if contains_debuff {
                        (48.0, EnemyAiSkillKind::Debuff)
                    } else {
                        (20.0, EnemyAiSkillKind::Debuff)
                    }
                }
            };
            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind,
            }
        }
    }
}

fn choose_enemy_skill(
    skill_ids: &[SkillId; 4],
    skill_count: usize,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
) -> Option<ScoredEnemySkill> {
    let mut best: Option<ScoredEnemySkill> = None;

    for (slot, skill_id) in skill_ids.iter().copied().take(skill_count).enumerate() {
        let Some(skill) = dbs.skills.get(&skill_id) else {
            continue;
        };
        let cost = skill.cost_ap;
        if current_ap < cost {
            continue;
        }

        let scored = score_enemy_skill(slot, skill_id, skill, ctx, dbs);
        match best {
            Some(current_best)
                if scored.score < current_best.score
                    || (scored.score == current_best.score && scored.slot >= current_best.slot) => {
            }
            _ => best = Some(scored),
        }
    }

    best
}

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
    dbs: Res<'w, BattleDbs>,
    formula_rules: Res<'w, crate::data::BattleFormulaRules>,
    accuracy_rng: ResMut<'w, crate::battle::AccuracyRng>,
    player_team: Res<'w, crate::battle::PlayerTeam>,
    enemy_team: Res<'w, crate::battle::EnemyTeam>,
    battle_log: ResMut<'w, BattleLog>,
    battle_result: ResMut<'w, BattleResult>,
    next_game_state: ResMut<'w, NextState<GameState>>,
}

fn finalize_enemy_turn(
    e_entity: Entity,
    enemy_team: &crate::battle::EnemyTeam,
    turn_ctx: &mut TurnContext,
    round_order: &RoundOrder,
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
    ai_state: &mut Local<(f32, bool)>,
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
                    event_writer: &mut writers.event_writer,
                    formula_writer: &mut writers.formula_writer,
                    status_writer: &mut writers.status_writer,
                    structured_log: &mut logs.structured_log,
                },
            );
        }
    }
    turn_ctx.enemy_ended = true;
    ai_state.0 = 0.0;
    ai_state.1 = false;
    if let Ok((_, _, stats, _, _, _, _, _, _)) = exec_query.get(e_entity) {
        if stats.hp <= 0 {
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }
    }
    next_phase.set(next_phase_after_side_end(round_order, Side::Enemy));
}

fn try_play_boost_card_for_skill(
    chosen_skill: ScoredEnemySkill,
    hand: &mut Hand,
    action_points: &mut ActionPoints,
    pending_boosts: &mut PendingBoosts,
    dbs: &BattleDbs,
    event_writer: &mut MessageWriter<BattleEvent>,
) -> bool {
    let Some(skill) = dbs.skills.get(&chosen_skill.skill_id) else {
        return false;
    };
    let skill_cost = skill.cost_ap;
    let desired_card = match chosen_skill.kind {
        EnemyAiSkillKind::Attack if pending_boosts.enemy.next_attack_bonus == 0 => {
            Some(CardEffect::NextAttackBoost { amount: 0 })
        }
        EnemyAiSkillKind::Heal if pending_boosts.enemy.next_heal_bonus == 0 => {
            Some(CardEffect::NextHealBoost { amount: 0 })
        }
        EnemyAiSkillKind::Shield if pending_boosts.enemy.next_shield_bonus == 0 => {
            Some(CardEffect::NextShieldBoost { amount: 0 })
        }
        EnemyAiSkillKind::Debuff if pending_boosts.enemy.next_attack_bonus == 0 => {
            Some(CardEffect::NextAttackBoost { amount: 0 })
        }
        _ => None,
    };

    let Some(desired_card) = desired_card else {
        return false;
    };

    let Some((idx, _)) = hand.enemy.iter().enumerate().find(|(_, cid)| {
        dbs.cards.get(cid).is_some_and(|card| {
            std::mem::discriminant(&card.effect) == std::mem::discriminant(&desired_card)
                && card.cost_ap <= action_points.enemy - skill_cost
        })
    }) else {
        return false;
    };

    let card_id = hand.enemy.remove(idx);
    let Some(card) = dbs.cards.get(&card_id) else {
        return false;
    };

    action_points.enemy -= card.cost_ap;
    match card.effect {
        CardEffect::NextAttackBoost { amount } => pending_boosts.enemy.next_attack_bonus += amount,
        CardEffect::NextHealBoost { amount } => pending_boosts.enemy.next_heal_bonus += amount,
        CardEffect::NextShieldBoost { amount } => pending_boosts.enemy.next_shield_bonus += amount,
        CardEffect::GainAp { amount } => action_points.enemy += amount,
    }
    event_writer.write(BattleEvent::CardUsed {
        side: Side::Enemy,
        card_name: card.name.to_string(),
    });
    true
}

pub fn enemy_turn_ai_system(
    time: Res<Time>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut runtime: EnemyTurnRuntime,
    mut logs: EnemyTurnLogs,
    mut writers: EnemyTurnEventWriters,
    mut ai_state: Local<(f32, bool)>,
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
    let round_order = &runtime.round_order;
    let action_points = &mut runtime.action_points;
    let hand = &mut runtime.hand;
    let pending_boosts = &mut runtime.pending_boosts;
    let dbs = &runtime.dbs;
    let formula_rules = &runtime.formula_rules;
    let accuracy_rng = &mut runtime.accuracy_rng;
    let player_team = &runtime.player_team;
    let enemy_team = &runtime.enemy_team;
    let battle_log = &mut runtime.battle_log;
    let battle_result = &mut runtime.battle_result;
    let next_game_state = &mut runtime.next_game_state;

    if ai_state.0 > 0.0 {
        ai_state.0 = (ai_state.0 - time.delta_secs()).max(0.0);
        return;
    }

    if turn_ctx.enemy_ended {
        ai_state.0 = 0.0;
        ai_state.1 = false;
        return;
    }

    if !ai_state.1 {
        ai_state.1 = true;
        ai_state.0 = ENEMY_AI_INITIAL_DELAY;
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
            turn_ctx.enemy_ended = true;
            ai_state.0 = 0.0;
            ai_state.1 = false;
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }

        let Ok(
            [
                (
                    _,
                    _e_combatant,
                    mut e_stats_m,
                    e_skills_m,
                    e_skill_count_m,
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
            break;
        };

        let p_element = p_combatant.element; // Copy
        let e_hp = e_stats_m.hp;
        let e_max_hp = e_stats_m.max_hp;
        let e_shield_value = e_shield_m.0;
        let e_atk = e_stats_m.atk;
        let e_skills_arr = e_skills_m.0;
        let e_skill_count = e_skill_count_m.0;
        let p_hp = p_stats_m.hp;
        let p_def = p_stats_m.def;
        let p_shield_value = p_shield_m.0;
        let p_attached_aura = p_aura_m.slots;
        let p_status_ids = p_statuses_m
            .entries
            .iter()
            .map(|entry| entry.id.clone())
            .collect();
        let enemy_has_aura = e_aura_m.primary().is_some();
        let enemy_has_cleansable_debuff = e_statuses_m.entries.iter().any(|entry| {
            matches!(
                entry.category,
                crate::data::StatusCategory::Debuff | crate::data::StatusCategory::Special
            )
        });

        let ai_ctx = EnemyAiContext {
            enemy_hp: e_hp,
            enemy_max_hp: e_max_hp,
            enemy_shield: e_shield_value,
            enemy_atk: e_atk,
            enemy_has_aura,
            enemy_has_cleansable_debuff,
            player_def: p_def,
            player_hp: p_hp,
            player_shield: p_shield_value,
            target_element: p_element,
            target_attached_auras: p_attached_aura,
            target_status_ids: p_status_ids,
        };

        let chosen_skill = choose_enemy_skill(
            &e_skills_arr,
            e_skill_count,
            action_points.enemy,
            &dbs,
            &ai_ctx,
        );

        let mut played_card = false;
        if let Some(chosen_skill) = chosen_skill {
            played_card = try_play_boost_card_for_skill(
                chosen_skill,
                hand,
                action_points,
                pending_boosts,
                &dbs,
                &mut writers.event_writer,
            );
        }

        if played_card {
            if let Some(chosen_skill) = chosen_skill {
                note_action_phase(
                    &mut logs.structured_log,
                    logs.turn_count.0,
                    Side::Enemy,
                    "敌方使用卡牌",
                    format!(
                        "为技能槽位{}预先使用增益卡；当前AP={}",
                        chosen_skill.slot, action_points.enemy
                    ),
                );
                push_named_action_trace(
                    &mut logs.action_trace,
                    logs.turn_count.0,
                    Side::Enemy,
                    "use_card",
                    format!(
                        "为技能槽位{}预先使用增益卡；当前AP={}",
                        chosen_skill.slot, action_points.enemy
                    ),
                );
            }
            acted_this_update = true;
            break;
        }

        if let Some(chosen_skill) = choose_enemy_skill(
            &e_skills_arr,
            e_skill_count,
            action_points.enemy,
            &dbs,
            &ai_ctx,
        ) {
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
                turn_ctx.enemy_ended = true;
                ai_state.0 = 0.0;
                ai_state.1 = false;
                next_phase.set(BattlePhase::CheckEnd);
                return;
            }
            acted_this_update = true;
            break;
        } else {
            // 没有可用技能：弃牌换 AP（或直接结束）
            if !hand.enemy.is_empty() {
                let card_id = hand.enemy.remove(0);
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
        &formula_rules,
        &mut logs,
        &mut writers,
        &mut next_phase,
        &mut exec_query,
        &mut ai_state,
    );
}
