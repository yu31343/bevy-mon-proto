use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    battle::{
        ActionPoints, ActionTrace, BattleControlMode, BattleEvent, BattleFormulaEvent, BattleLog,
        BattleResult, BattleStatusEvent, Combatant, ElementAura, Hand, InBattle, PendingBoosts,
        RoundOrder, Shield, Side, SkillCount, SkillList, Stats, StructuredBattleLog, TurnAction,
        TurnContext, TurnCount, next_phase_after_side_end, note_action_phase,
        push_named_action_trace, push_turn_action_trace, transfer_status_by_id,
    },
    data::{
        BattleDbs, CardEffect, ElementType, SkillCategory, SkillDef, SkillEffect, SkillId,
        StatusCategory,
    },
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

#[derive(Debug, Clone)]
struct EnemySwitchCandidate {
    index: usize,
    hp: i32,
    max_hp: i32,
    shield: i32,
    atk: i32,
    element: ElementType,
    skill_ids: [SkillId; 4],
    skill_count: usize,
    status_ids: Vec<String>,
    has_aura: bool,
    has_cleansable_debuff: bool,
}

#[derive(Debug, Clone)]
struct ScoredEnemySwitch {
    index: usize,
    score: f32,
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
                SkillEffect::Dispel { status_ids, .. } => dispel_value(status_ids, ctx),
                SkillEffect::ModifyStages { .. } | SkillEffect::ApplyStatus { .. } => 8.0,
                SkillEffect::Heal { amount } => (*amount as f32) * 0.6,
                SkillEffect::Shield { amount } => (*amount as f32) * 0.4,
                _ => 0.0,
            })
            .sum(),
        _ => 0.0,
    }
}

fn dispel_value(status_ids: &[String], ctx: &EnemyAiContext) -> f32 {
    let removed_count = status_ids
        .iter()
        .map(|status_id| {
            if matches!(
                status_id.as_str(),
                "stage_shift_buff" | "stage_shift_debuff"
            ) {
                ctx.target_status_ids
                    .iter()
                    .filter(|id| *id == status_id || id.starts_with(&format!("{status_id}_")))
                    .count()
            } else if ctx.target_status_ids.iter().any(|id| id == status_id) {
                1
            } else {
                0
            }
        })
        .sum::<usize>();

    removed_count as f32 * 12.0
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
    estimate_attack_value_with_atk(skill, ctx.enemy_atk, ctx, dbs)
}

fn estimate_attack_value_with_atk(
    skill: &SkillDef,
    attacker_atk: i32,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
) -> f32 {
    let Some(power) = primary_attack_power(&skill.effect) else {
        return 0.0;
    };

    let raw = (power + attacker_atk - ctx.player_def).max(1) as f32;
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
            let followup_bonus = effects
                .iter()
                .skip(1)
                .map(|effect| match effect {
                    SkillEffect::Dispel { status_ids, .. } => dispel_value(status_ids, ctx),
                    _ => 0.0,
                })
                .sum::<f32>();
            let (score, kind) = match primary_effect(&skill.effect) {
                Some(SkillEffect::Attack { .. }) => (
                    estimate_attack_value(skill, ctx, dbs)
                        + 14.0
                        + conditional_bonus
                        + followup_bonus,
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
                Some(SkillEffect::Dispel { status_ids, .. }) => (
                    24.0 + dispel_value(status_ids, ctx),
                    EnemyAiSkillKind::Debuff,
                ),
                Some(SkillEffect::ApplyStatus { .. })
                | Some(SkillEffect::ModifyStages { .. })
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

fn best_attack_value(
    skill_ids: &[SkillId; 4],
    skill_count: usize,
    attacker_atk: i32,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
) -> f32 {
    skill_ids
        .iter()
        .copied()
        .take(skill_count)
        .filter_map(|skill_id| dbs.skills.get(&skill_id))
        .filter(|skill| current_ap >= skill.cost_ap)
        .map(|skill| estimate_attack_value_with_atk(skill, attacker_atk, ctx, dbs))
        .fold(0.0, f32::max)
}

fn status_pressure(status_ids: &[String], has_aura: bool, has_cleansable_debuff: bool) -> f32 {
    let damaging_statuses = status_ids
        .iter()
        .filter(|id| {
            matches!(
                id.as_str(),
                "burning"
                    | "seeded"
                    | "conduct_from_thunder"
                    | "conduct_from_water"
                    | "burning_from_fire"
                    | "burning_from_grass"
            )
        })
        .count() as f32;

    damaging_statuses * 8.0
        + if has_aura { 4.0 } else { 0.0 }
        + if has_cleansable_debuff { 6.0 } else { 0.0 }
}

fn score_switch_candidate(
    current: &EnemySwitchCandidate,
    candidate: &EnemySwitchCandidate,
    current_best_attack: f32,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
) -> f32 {
    if candidate.hp <= 0 {
        return f32::MIN;
    }

    let current_hp_ratio = if current.max_hp <= 0 {
        0.0
    } else {
        current.hp.max(0) as f32 / current.max_hp as f32
    };
    let candidate_hp_ratio = if candidate.max_hp <= 0 {
        0.0
    } else {
        candidate.hp.max(0) as f32 / candidate.max_hp as f32
    };
    let current_defense = dbs
        .elements
        .get_effectiveness(ctx.target_element, current.element);
    let candidate_defense = dbs
        .elements
        .get_effectiveness(ctx.target_element, candidate.element);
    let defensive_gain = (current_defense - candidate_defense) * 34.0;
    let health_gain = (candidate_hp_ratio - current_hp_ratio) * 30.0;
    let shield_gain = (candidate.shield - current.shield) as f32 * 0.35;
    let pressure_relief = status_pressure(
        &current.status_ids,
        current.has_aura,
        current.has_cleansable_debuff,
    ) - status_pressure(
        &candidate.status_ids,
        candidate.has_aura,
        candidate.has_cleansable_debuff,
    );
    let candidate_attack = best_attack_value(
        &candidate.skill_ids,
        candidate.skill_count,
        candidate.atk,
        current_ap - 1,
        dbs,
        ctx,
    );
    let attack_gain = (candidate_attack - current_best_attack) * 0.5;
    let danger_bonus = if current_hp_ratio <= 0.25 { 20.0 } else { 0.0 };

    defensive_gain + health_gain + shield_gain + pressure_relief + attack_gain + danger_bonus - 12.0
}

fn choose_enemy_switch(
    current: &EnemySwitchCandidate,
    candidates: &[EnemySwitchCandidate],
    current_best_attack: f32,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    already_switched: bool,
) -> Option<ScoredEnemySwitch> {
    if already_switched {
        return None;
    }
    if current_ap < 2 {
        return None;
    }

    candidates
        .iter()
        .filter(|candidate| candidate.index != current.index && candidate.hp > 0)
        .map(|candidate| ScoredEnemySwitch {
            index: candidate.index,
            score: score_switch_candidate(
                current,
                candidate,
                current_best_attack,
                current_ap,
                dbs,
                ctx,
            ),
        })
        .filter(|candidate| candidate.score >= 18.0)
        .max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.index.cmp(&a.index))
        })
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
    enemy_team: ResMut<'w, crate::battle::EnemyTeam>,
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
    ai_state.2 = false;
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

pub fn enemy_turn_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
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
    if *battle_mode != BattleControlMode::DebugPlayerControlsBoth {
        return;
    }

    let round_order = &runtime.round_order;
    let action_points = &mut runtime.action_points;
    let hand = &mut runtime.hand;
    let pending_boosts = &mut runtime.pending_boosts;
    let dbs = &runtime.dbs;
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

    if let Some(target_index) = switch_target {
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

    if keyboard.just_pressed(KeyCode::KeyF) {
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
            turn_ctx.enemy_ended = true;
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }
        return;
    }

    for (key, idx) in [
        (KeyCode::KeyZ, 0_usize),
        (KeyCode::KeyX, 1_usize),
        (KeyCode::KeyC, 2_usize),
        (KeyCode::KeyV, 3_usize),
        (KeyCode::KeyB, 4_usize),
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

                    let effect_detail = match card.effect {
                        CardEffect::GainAp { amount } => {
                            action_points.enemy += amount;
                            format!("获得AP={amount}")
                        }
                        CardEffect::NextAttackBoost { amount } => {
                            pending_boosts.enemy.next_attack_bonus = amount;
                            format!("下次攻击加成={amount}")
                        }
                        CardEffect::NextShieldBoost { amount } => {
                            pending_boosts.enemy.next_shield_bonus = amount;
                            format!("下次护盾加成={amount}")
                        }
                        CardEffect::NextHealBoost { amount } => {
                            pending_boosts.enemy.next_heal_bonus = amount;
                            format!("下次治疗加成={amount}")
                        }
                    };
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
                        turn_ctx.enemy_ended = true;
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
        let Ok((_, _, _, skill_list, skill_count, _, _, _, _)) = exec_query.get(e_entity) else {
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
    if turn_ctx.enemy_end_requested || action_points.enemy <= 0 {
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
            &formula_rules,
            &mut logs,
            &mut writers,
            &mut next_phase,
            &mut exec_query,
            &mut ai_state,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{EffectTarget, ElementDb, ElementType, ReactionDb, StatusDb};
    use std::collections::HashMap;

    fn test_ai_context(target_status_ids: Vec<String>) -> EnemyAiContext {
        EnemyAiContext {
            enemy_hp: 20,
            enemy_max_hp: 20,
            enemy_shield: 0,
            enemy_atk: 5,
            enemy_has_aura: false,
            enemy_has_cleansable_debuff: false,
            player_def: 5,
            player_hp: 20,
            player_shield: 0,
            target_element: ElementType::Dark,
            target_attached_auras: [None, None],
            target_status_ids,
        }
    }

    fn test_dbs(skill: SkillDef) -> BattleDbs {
        BattleDbs {
            skills: HashMap::from([(skill.id, skill)]),
            cards: HashMap::new(),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        }
    }

    fn test_switch_dbs() -> BattleDbs {
        let attack = SkillDef {
            id: SkillId::WaterBlade,
            name: "水刃".to_string(),
            category: SkillCategory::ElementAttack,
            cost_ap: 1,
            effect: SkillEffect::Attack {
                power: 12,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: Some(ElementType::Water),
            base_accuracy: None,
        };
        BattleDbs {
            skills: HashMap::from([(attack.id, attack)]),
            cards: HashMap::new(),
            elements: ElementDb::from_default_config(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        }
    }

    fn test_switch_candidate(
        index: usize,
        hp: i32,
        max_hp: i32,
        shield: i32,
        element: ElementType,
    ) -> EnemySwitchCandidate {
        EnemySwitchCandidate {
            index,
            hp,
            max_hp,
            shield,
            atk: 8,
            element,
            skill_ids: [SkillId::WaterBlade; 4],
            skill_count: 1,
            status_ids: Vec::new(),
            has_aura: false,
            has_cleansable_debuff: false,
        }
    }

    #[test]
    fn enemy_switch_prefers_healthy_resistant_candidate_when_current_is_low() {
        let dbs = test_switch_dbs();
        let mut ctx = test_ai_context(Vec::new());
        ctx.target_element = ElementType::Water;
        let current = test_switch_candidate(0, 5, 40, 0, ElementType::Fire);
        let candidate = test_switch_candidate(1, 34, 40, 0, ElementType::Grass);

        let chosen = choose_enemy_switch(
            &current,
            &[current.clone(), candidate],
            0.0,
            3,
            &dbs,
            &ctx,
            false,
        )
        .expect("low HP and bad matchup should make switching valuable");

        assert_eq!(chosen.index, 1);
        assert!(chosen.score >= 18.0);
    }

    #[test]
    fn enemy_switch_requires_ap_after_switch_and_only_once_per_turn() {
        let dbs = test_switch_dbs();
        let mut ctx = test_ai_context(Vec::new());
        ctx.target_element = ElementType::Water;
        let current = test_switch_candidate(0, 5, 40, 0, ElementType::Fire);
        let candidate = test_switch_candidate(1, 34, 40, 0, ElementType::Grass);
        let candidates = [current.clone(), candidate];

        assert!(choose_enemy_switch(&current, &candidates, 0.0, 1, &dbs, &ctx, false).is_none());
        assert!(choose_enemy_switch(&current, &candidates, 0.0, 3, &dbs, &ctx, true).is_none());
    }

    #[test]
    fn enemy_switch_ignores_defeated_candidates() {
        let dbs = test_switch_dbs();
        let mut ctx = test_ai_context(Vec::new());
        ctx.target_element = ElementType::Water;
        let current = test_switch_candidate(0, 5, 40, 0, ElementType::Fire);
        let defeated = test_switch_candidate(1, 0, 40, 0, ElementType::Grass);

        assert!(
            choose_enemy_switch(
                &current,
                &[current.clone(), defeated],
                0.0,
                3,
                &dbs,
                &ctx,
                false
            )
            .is_none()
        );
    }

    #[test]
    fn dispel_score_uses_target_stage_shift_prefix_matches() {
        let skill = SkillDef {
            id: SkillId::SacredJudgment,
            name: "圣辉裁决".to_string(),
            category: SkillCategory::SpecialAttack,
            cost_ap: 4,
            effect: SkillEffect::Sequence {
                effects: vec![
                    SkillEffect::Attack {
                        power: 1,
                        lifesteal_ratio: None,
                        ignore_shield: true,
                    },
                    SkillEffect::Dispel {
                        status_ids: vec!["stage_shift_buff".to_string()],
                        target: EffectTarget::Opponent,
                    },
                ],
            },
            element: Some(ElementType::Light),
            base_accuracy: None,
        };
        let dbs = test_dbs(skill.clone());
        let without_buff = test_ai_context(vec!["cursed".to_string()]);
        let with_buff = test_ai_context(vec![
            "stage_shift_buff_atk_1".to_string(),
            "stage_shift_buff_def_2".to_string(),
            "stage_shift_debuff_acc_1".to_string(),
        ]);

        let score_without_buff =
            score_enemy_skill(0, SkillId::SacredJudgment, &skill, &without_buff, &dbs).score;
        let score_with_buff =
            score_enemy_skill(0, SkillId::SacredJudgment, &skill, &with_buff, &dbs).score;

        assert_eq!(
            dispel_value(&["stage_shift_buff".to_string()], &with_buff),
            24.0
        );
        assert_eq!(
            dispel_value(&["stage_shift_buff".to_string()], &without_buff),
            0.0
        );
        assert!(score_with_buff > score_without_buff);
    }
}

pub fn enemy_turn_ai_system(
    time: Res<Time>,
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
    if *battle_mode == BattleControlMode::DebugPlayerControlsBoth {
        ai_state.0 = 0.0;
        ai_state.1 = false;
        ai_state.2 = false;
        return;
    }

    let round_order = &runtime.round_order;
    let action_points = &mut runtime.action_points;
    let hand = &mut runtime.hand;
    let pending_boosts = &mut runtime.pending_boosts;
    let dbs = &runtime.dbs;
    let formula_rules = &runtime.formula_rules;
    let accuracy_rng = &mut runtime.accuracy_rng;
    let player_team = &runtime.player_team;
    let enemy_team = &mut runtime.enemy_team;
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
        ai_state.2 = false;
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
            ai_state.2 = false;
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }

        let Ok((_, p_combatant, p_stats, _, _, p_shield, p_statuses, p_aura, _)) =
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
        let e_hp = e_stats.hp;
        let e_max_hp = e_stats.max_hp;
        let e_shield_value = e_shield.0;
        let e_atk = e_stats.atk;
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
        let current_best_attack = best_attack_value(
            &e_skills_arr,
            e_skill_count,
            e_atk,
            action_points.enemy,
            &dbs,
            &ai_ctx,
        );
        let current_switch_candidate = EnemySwitchCandidate {
            index: enemy_team.0.active_index,
            hp: e_hp,
            max_hp: e_max_hp,
            shield: e_shield_value,
            atk: e_atk,
            element: e_combatant.element,
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
                    element: combatant.element,
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

        if let Some(chosen_switch) = choose_enemy_switch(
            &current_switch_candidate,
            &enemy_switch_candidates,
            current_best_attack,
            action_points.enemy,
            &dbs,
            &ai_ctx,
            ai_state.2,
        ) {
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
                turn_ctx.enemy_ended = true;
                ai_state.0 = 0.0;
                ai_state.1 = false;
                ai_state.2 = false;
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
