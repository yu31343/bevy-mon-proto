use bevy::prelude::*;

use crate::{
    battle::{
        ActionPoints, BattleEvent, BattleLog, BattleResult, Combatant, ElementAura, Hand,
        InBattle, PendingBoosts, Shield, Side, SkillCount, SkillList, Stats, TurnContext,
    },
    data::{BattleDbs, CardEffect},
    game_state::{BattlePhase, GameState},
};

use super::{abort_battle, apply_effect, monster_skill_ap_cost};

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
        ai_state.0 = 1.55;
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
        let e_skills_arr = e_skills_m.0;
        let e_skill_count = e_skill_count_m.0;

        let enemy_hp_pct = if e_max_hp > 0 {
            (e_hp.max(0) * 100) / e_max_hp
        } else {
            0
        };

        // 先决定是否打牌：优先补“对接下一次技能”的 PendingBoost。
        let mut played_card = false;
        if pending_boosts.enemy.next_attack_bonus == 0 && action_points.enemy >= 2 {
            let can_attack0 = e_skill_count > 0 && action_points.enemy >= monster_skill_ap_cost(0);
            let can_attack1 = e_skill_count > 1 && action_points.enemy >= monster_skill_ap_cost(1);
            if can_attack0 || can_attack1 {
                if let Some((idx, _)) = hand.enemy.iter().enumerate().find(|(_, cid)| {
                    dbs.cards.get(cid).is_some_and(|c| {
                        matches!(c.effect, CardEffect::NextAttackBoost { .. })
                            && c.cost_ap <= action_points.enemy
                    })
                }) {
                    let card_id = hand.enemy.remove(idx);
                    if let Some(card) = dbs.cards.get(&card_id) {
                        action_points.enemy -= card.cost_ap;
                        if let CardEffect::NextAttackBoost { amount } = card.effect {
                            pending_boosts.enemy.next_attack_bonus += amount;
                            event_writer.write(BattleEvent::CardUsed {
                                side: Side::Enemy,
                                card_name: card.name.to_string(),
                            });
                            played_card = true;
                        }
                    }
                }
            }
        }

        if !played_card
            && pending_boosts.enemy.next_heal_bonus == 0
            && enemy_hp_pct < 40
            && action_points.enemy >= 2
            && e_skill_count > 3
            && action_points.enemy >= monster_skill_ap_cost(3)
        {
            if let Some((idx, _)) = hand.enemy.iter().enumerate().find(|(_, cid)| {
                dbs.cards.get(cid).is_some_and(|c| {
                    matches!(c.effect, CardEffect::NextHealBoost { .. })
                        && c.cost_ap <= action_points.enemy
                })
            }) {
                let card_id = hand.enemy.remove(idx);
                if let Some(card) = dbs.cards.get(&card_id) {
                    action_points.enemy -= card.cost_ap;
                    if let CardEffect::NextHealBoost { amount } = card.effect {
                        pending_boosts.enemy.next_heal_bonus += amount;
                        event_writer.write(BattleEvent::CardUsed {
                            side: Side::Enemy,
                            card_name: card.name.to_string(),
                        });
                        played_card = true;
                    }
                }
            }
        }

        if !played_card
            && pending_boosts.enemy.next_shield_bonus == 0
            && action_points.enemy >= 2
            && e_skill_count > 2
            && action_points.enemy >= monster_skill_ap_cost(2)
        {
            if let Some((idx, _)) = hand.enemy.iter().enumerate().find(|(_, cid)| {
                dbs.cards.get(cid).is_some_and(|c| {
                    matches!(c.effect, CardEffect::NextShieldBoost { .. })
                        && c.cost_ap <= action_points.enemy
                })
            }) {
                let card_id = hand.enemy.remove(idx);
                if let Some(card) = dbs.cards.get(&card_id) {
                    action_points.enemy -= card.cost_ap;
                    if let CardEffect::NextShieldBoost { amount } = card.effect {
                        pending_boosts.enemy.next_shield_bonus += amount;
                        event_writer.write(BattleEvent::CardUsed {
                            side: Side::Enemy,
                            card_name: card.name.to_string(),
                        });
                        played_card = true;
                    }
                }
            }
        }

        if played_card {
            acted_this_update = true;
            break;
        }

        // 否则优先使用精灵技能（根据 HP 简单选择）。
        let mut chosen_slot: Option<usize> = None;

        if e_skill_count > 3 && enemy_hp_pct < 40 && action_points.enemy >= monster_skill_ap_cost(3) {
            chosen_slot = Some(3);
        } else if e_skill_count > 2
            && action_points.enemy >= monster_skill_ap_cost(2)
            && e_shield_value <= 0
        {
            chosen_slot = Some(2);
        } else if e_skill_count > 1 && action_points.enemy >= monster_skill_ap_cost(1) {
            chosen_slot = Some(1);
        } else if e_skill_count > 0 && action_points.enemy >= monster_skill_ap_cost(0) {
            chosen_slot = Some(0);
        }

        if let Some(slot) = chosen_slot {
            let skill_id = e_skills_arr[slot];
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
        ai_state.0 = 2.55; //AI 每次行动后冷却约 2.5 秒，给玩家反应时间（UI 更新、动画等）。
        return;
    }

    ai_state.0 = 0.0;
    ai_state.1 = false;
    turn_ctx.enemy_ended = true;
    next_phase.set(BattlePhase::CheckEnd);
}
