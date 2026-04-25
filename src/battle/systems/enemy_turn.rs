use bevy::prelude::*;

use crate::{
    battle::{
        ActionPoints, BattleEvent, BattleLog, BattleResult, Combatant, ElementAura, Hand,
        InBattle, PendingBoosts, Shield, Side, SkillCount, SkillList, Stats, TurnContext,
    },
    data::{BattleDbs, CardEffect, SkillDef, SkillEffect, SkillId},
    game_state::{BattlePhase, GameState},
};

use super::{abort_battle, apply_effect, monster_skill_ap_cost};

const ENEMY_AI_INITIAL_DELAY: f32 = 0.35;
const ENEMY_AI_ACTION_DELAY: f32 = 0.55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnemyAiSkillKind {
    Attack,
    Heal,
    Shield,
}

#[derive(Debug, Clone, Copy)]
struct EnemyAiContext {
    enemy_hp: i32,
    enemy_max_hp: i32,
    enemy_shield: i32,
    enemy_atk: i32,
    player_def: i32,
    player_hp: i32,
    player_shield: i32,
    target_element: crate::data::ElementType,
    target_attached_aura: Option<crate::data::ElementType>,
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

fn estimate_attack_value(skill: &SkillDef, ctx: &EnemyAiContext, dbs: &BattleDbs) -> f32 {
    let SkillEffect::Attack { power } = skill.effect else {
        return 0.0;
    };

    let raw = (power + ctx.enemy_atk - ctx.player_def).max(1) as f32;
    let effectiveness = if let Some(skill_element) = skill.element {
        let defender_element = if ctx.player_shield > 0 {
            ctx.target_element
        } else {
            ctx.target_attached_aura.unwrap_or(ctx.target_element)
        };
        dbs.elements.get_effectiveness(skill_element, defender_element)
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

    match skill.effect {
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
            let effective_heal = (amount as f32).min(missing_hp);
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
            let mut score = amount as f32 + 5.0;
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
        let cost = monster_skill_ap_cost(slot);
        if current_ap < cost {
            continue;
        }
        let Some(skill) = dbs.skills.get(&skill_id) else {
            continue;
        };

        let scored = score_enemy_skill(slot, skill_id, skill, ctx, dbs);
        match best {
            Some(current_best)
                if scored.score < current_best.score
                    || (scored.score == current_best.score && scored.slot >= current_best.slot) => {}
            _ => best = Some(scored),
        }
    }

    best
}

fn try_play_boost_card_for_skill(
    chosen_skill: ScoredEnemySkill,
    hand: &mut Hand,
    action_points: &mut ActionPoints,
    pending_boosts: &mut PendingBoosts,
    dbs: &BattleDbs,
    event_writer: &mut MessageWriter<BattleEvent>,
) -> bool {
    let skill_cost = monster_skill_ap_cost(chosen_skill.slot);
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
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    mut pending_boosts: ResMut<PendingBoosts>,
    dbs: Res<BattleDbs>,
    player_team: Res<crate::battle::PlayerTeam>,
    enemy_team: Res<crate::battle::EnemyTeam>,
    mut battle_log: ResMut<BattleLog>,
    mut battle_result: ResMut<BattleResult>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut ai_state: Local<(f32, bool)>,
    mut exec_query: Query<
        (
            Entity,
            &Combatant,
            &mut Stats,
            &SkillList,
            &SkillCount,
            &mut Shield,
            &mut ElementAura,
            &Name,
        ),
        With<InBattle>,
    >,
) {
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
    if action_points.enemy <= 0 {
        turn_ctx.enemy_ended = true;
        ai_state.0 = 0.0;
        ai_state.1 = false;
        next_phase.set(BattlePhase::CheckEnd);
        return;
    }

    let Some(p_entity) = player_team.0.active_combatant() else {
        abort_battle(
            "敌方 AI：玩家上场精灵无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    let Some(e_entity) = enemy_team.0.active_combatant() else {
        abort_battle(
            "敌方 AI：敌方上场精灵无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    let mut acted_this_update = false;
    while action_points.enemy > 0 {
        let should_go_check_end = exec_query
            .get(p_entity)
            .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false)
            || exec_query
                .get(e_entity)
                .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
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
                    _e_aura_m,
                    _,
                ),
                (_, p_combatant, mut p_stats_m, _, _, mut p_shield_m, mut p_aura_m, _),
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
        let p_attached_aura = p_aura_m.attached;

        let ai_ctx = EnemyAiContext {
            enemy_hp: e_hp,
            enemy_max_hp: e_max_hp,
            enemy_shield: e_shield_value,
            enemy_atk: e_atk,
            player_def: p_def,
            player_hp: p_hp,
            player_shield: p_shield_value,
            target_element: p_element,
            target_attached_aura: p_attached_aura,
        };

        let chosen_skill = choose_enemy_skill(&e_skills_arr, e_skill_count, action_points.enemy, &dbs, &ai_ctx);

        let mut played_card = false;
        if let Some(chosen_skill) = chosen_skill {
            played_card = try_play_boost_card_for_skill(
                chosen_skill,
                &mut hand,
                &mut action_points,
                &mut pending_boosts,
                &dbs,
                &mut event_writer,
            );
        }

        if played_card {
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
            let cost = monster_skill_ap_cost(slot);
            if action_points.enemy < cost {
                break;
            }

            action_points.enemy -= cost;

            // 技能使用事件
            event_writer.write(BattleEvent::SkillUsed {
                side: Side::Enemy,
                skill_name: skill.name.clone(),
                slot,
            });

            apply_effect(
                &skill.effect,
                skill.element,
                Side::Enemy,
                Side::Player,
                p_element,
                &mut e_stats_m,
                &mut e_shield_m,
                &mut p_stats_m,
                &mut p_shield_m,
                &mut p_aura_m,
                &mut pending_boosts,
                &dbs.elements,
                &mut event_writer,
            );

            let should_go_check_end = exec_query
                .get(p_entity)
                .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false)
                || exec_query
                    .get(e_entity)
                    .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
                    .unwrap_or(false);
            if should_go_check_end {
                turn_ctx.enemy_ended = true;
                ai_state.0 = 0.0;
                ai_state.1 = false;
                next_phase.set(BattlePhase::CheckEnd);
                return;
            }
            acted_this_update = true;
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
                event_writer.write(BattleEvent::CardDiscarded {
                    side: Side::Enemy,
                    card_name,
                });
                acted_this_update = true;
            } else {
                break;
            }
        }

        let should_go_check_end = exec_query
            .get(p_entity)
            .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false)
            || exec_query
                .get(e_entity)
                .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false);
        if should_go_check_end {
            turn_ctx.enemy_ended = true;
            ai_state.0 = 0.0;
            ai_state.1 = false;
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }

        if action_points.enemy <= 0 {
            break;
        }
    }

    if acted_this_update && action_points.enemy > 0 {
        ai_state.0 = ENEMY_AI_ACTION_DELAY; // 缩短敌方思考/行动间隔，保持节奏更紧凑。
        return;
    }

    ai_state.0 = 0.0;
    ai_state.1 = false;
    turn_ctx.enemy_ended = true;
    next_phase.set(BattlePhase::CheckEnd);
}
