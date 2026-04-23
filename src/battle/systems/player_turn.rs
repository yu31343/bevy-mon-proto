use bevy::prelude::*;

use crate::{
    battle::{
        ActionPoints, BattleEvent, BattleLog, BattleResult, Combatant, ElementAura, Hand,
        InBattle, PendingBoosts, SelectedCard, Shield, Side, SkillCount, SkillList, Stats,
        TurnAction, TurnContext,
    },
    data::{BattleDbs, CardEffect},
    game_state::{BattlePhase, GameState},
};

use super::{abort_battle, apply_effect, monster_skill_ap_cost};

pub fn player_turn_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    mut pending_boosts: ResMut<PendingBoosts>,
    dbs: Res<BattleDbs>,
    mut player_team: ResMut<crate::battle::PlayerTeam>,
    enemy_team: Res<crate::battle::EnemyTeam>,
    mut battle_log: ResMut<BattleLog>,
    mut battle_result: ResMut<BattleResult>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut selected: ResMut<SelectedCard>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut query: Query<
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
    if turn_ctx.player_ended {
        return;
    }
    if player_team.0.combatants.is_empty() || enemy_team.0.combatants.is_empty() {
        abort_battle(
            "战斗数据异常：队伍为空。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    }

    let Some(p_entity) = player_team.0.active_combatant() else {
        abort_battle(
            "玩家上场精灵无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    let Some(e_entity) = enemy_team.0.active_combatant() else {
        abort_battle(
            "敌方上场精灵无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    // 0) 回合内切换精灵（消耗 1 AP）：5/6/7 -> 队伍第 1/2/3 只
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
        if action_points.player >= 1
            && target_index < player_team.0.combatants.len()
            && target_index != player_team.0.active_index
        {
            let target_entity = player_team.0.combatants[target_index];
            if let Ok((_, _, stats, _, _, _, _, name)) = query.get(target_entity) {
                if stats.hp > 0 {
                    action_points.player -= 1;
                    player_team.0.active_index = target_index;
                    event_writer.write(BattleEvent::Switched {
                        side: Side::Player,
                        name: name.to_string(),
                    });
                }
            }
        }
        return;
    }

    // 1) 来自 UI 的按钮选择（若存在则优先执行）。
    if let Some(TurnAction::Skill(skill_id)) = turn_ctx.player_action {
        // 找到按钮对应的技能槽位（用于计算 AP 消耗与 UI 闪白）。
        let Ok((_, _, _, skill_list, skill_count, _, _, _)) = query.get(p_entity) else {
            turn_ctx.player_action = None;
            return;
        };
        let Some(slot) = skill_list
            .0
            .iter()
            .take(skill_count.0)
            .position(|&s| s == skill_id)
        else {
            turn_ctx.player_action = None;
            return;
        };
        let cost = monster_skill_ap_cost(slot);
        let Some(skill) = dbs.skills.get(&skill_id) else {
            turn_ctx.player_action = None;
            return;
        };

        // 如果 AP 不够：不执行并清空，避免下一帧重复触发。
        if action_points.player < cost {
            turn_ctx.player_action = None;
            return;
        }

        action_points.player -= cost;
        turn_ctx.player_action = None;

        let Ok(
            [
                (_, _attacker_combatant, mut p_stats, _, _, mut p_shield, mut _p_aura, _),
                (_, e_combatant, mut e_stats, _, _, mut e_shield, mut e_aura, _),
            ],
        ) = query.get_many_mut([p_entity, e_entity])
        else {
            return;
        };

        event_writer.write(BattleEvent::SkillUsed {
            side: Side::Player,
            skill_name: skill.name.clone(),
            slot,
        });

        apply_effect(
            &skill.effect,
            skill.element,
            Side::Player,
            Side::Enemy,
            e_combatant.element,
            &mut p_stats,
            &mut p_shield,
            &mut e_stats,
            &mut e_shield,
            &mut e_aura,
            &mut pending_boosts,
            &dbs.elements,
            &mut event_writer,
        );

        let should_go_check_end = query
            .get(p_entity)
            .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false)
            || query
                .get(e_entity)
                .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false);
        if should_go_check_end {
            turn_ctx.player_ended = true;
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }

        return;
    }

    // 1) 手动结束回合（优先级最高）
    if keyboard.just_pressed(KeyCode::KeyE) {
        turn_ctx.player_ended = true;
        next_phase.set(BattlePhase::EnemyTurn);
        return;
    }

    // 2) 弃牌：弃置选中的牌；若无选中则武装弃牌模式（需再选一张才弃置）
    if keyboard.just_pressed(KeyCode::KeyF) {
        if hand.player.is_empty() {
            return;
        }
        let Some(target_index) = selected.index.filter(|&i| i < hand.player.len()) else {
            // 无已选牌：切换武装状态（再按一次 F 取消）
            selected.discard_armed = !selected.discard_armed;
            return;
        };
        let card_id = hand.player.remove(target_index);
        action_points.player += 1;
        let card_name = dbs
            .cards
            .get(&card_id)
            .map(|c| c.name.to_string())
            .unwrap_or_else(|| format!("{card_id:?}"));
        event_writer.write(BattleEvent::CardDiscarded {
            side: Side::Player,
            card_name,
        });
        selected.index = None;
        selected.discard_armed = false;
        // 弃牌可能使 AP 从 0 变为正，这种情况不触发自动结束。
        return;
    }

    // 3) 出牌/选牌（手牌热键：Z X C V B；两步式：首按选中，再按出牌；弃牌武装时直接弃置）
    for (key, idx) in [
        (KeyCode::KeyZ, 0_usize),
        (KeyCode::KeyX, 1_usize),
        (KeyCode::KeyC, 2_usize),
        (KeyCode::KeyV, 3_usize),
        (KeyCode::KeyB, 4_usize),
    ] {
        if keyboard.just_pressed(key) {
            if idx >= hand.player.len() {
                return;
            }
            // 若已武装弃牌模式（由点击"弃牌"按钮触发），直接弃置该牌
            if selected.discard_armed {
                let card_id = hand.player.remove(idx);
                action_points.player += 1;
                let card_name = dbs
                    .cards
                    .get(&card_id)
                    .map(|c| c.name.to_string())
                    .unwrap_or_else(|| format!("{card_id:?}"));
                event_writer.write(BattleEvent::CardDiscarded {
                    side: Side::Player,
                    card_name,
                });
                selected.index = None;
                selected.discard_armed = false;
                return;
            }
            // 第一步：选中该牌（显示描述）
            if selected.index != Some(idx) {
                selected.index = Some(idx);
                return;
            }
            // 第二步：出牌
            let card_id = hand.player[idx];
            if let Some(card) = dbs.cards.get(&card_id) {
                if action_points.player >= card.cost_ap {
                    hand.player.remove(idx);
                    action_points.player -= card.cost_ap;

                    let card_name = card.name.to_string();
                    event_writer.write(BattleEvent::CardUsed {
                        side: Side::Player,
                        card_name,
                    });

                    match card.effect {
                        CardEffect::GainAp { amount } => {
                            action_points.player += amount;
                        }
                        CardEffect::NextAttackBoost { amount } => {
                            pending_boosts.player.next_attack_bonus = amount;
                        }
                        CardEffect::NextShieldBoost { amount } => {
                            pending_boosts.player.next_shield_bonus = amount;
                        }
                        CardEffect::NextHealBoost { amount } => {
                            pending_boosts.player.next_heal_bonus = amount;
                        }
                    }

                    selected.index = None;
                    selected.discard_armed = false;

                    let should_go_check_end = query
                        .get(p_entity)
                        .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
                        .unwrap_or(false)
                        || query
                            .get(e_entity)
                            .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
                            .unwrap_or(false);
                    if should_go_check_end {
                        turn_ctx.player_ended = true;
                        next_phase.set(BattlePhase::CheckEnd);
                        return;
                    }
                }
            }
            return;
        }
    }

    // 4) 使用精灵技能（1-4 -> slot 0-3），需要消耗 AP
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

    let Some(skill_slot) = skill_slot else { return };

    // 读取玩家技能栏（slot 与按钮一一对应）。
    let Ok((_, _, _, skill_list, skill_count, _, _, _)) = query.get(p_entity) else {
        return;
    };
    if skill_slot >= skill_count.0 {
        return;
    }
    let skill_id = skill_list.0[skill_slot];
    let cost = monster_skill_ap_cost(skill_slot);
    if action_points.player < cost {
        return;
    }

    let Some(skill) = dbs.skills.get(&skill_id) else {
        return;
    };

    action_points.player -= cost;

    // 执行技能：玩家为攻击方/施术方。
    let Ok(
        [
            (_, _attacker_combatant, mut p_stats, _, _, mut p_shield, mut _p_aura, _),
            (_, e_combatant, mut e_stats, _, _, mut e_shield, mut e_aura, _),
        ],
    ) = query.get_many_mut([p_entity, e_entity])
    else {
        return;
    };

    // 事件：技能使用（用于 UI 闪白）。
    event_writer.write(BattleEvent::SkillUsed {
        side: Side::Player,
        skill_name: skill.name.clone(),
        slot: skill_slot,
    });

    apply_effect(
        &skill.effect,
        skill.element,
        Side::Player,
        Side::Enemy,
        e_combatant.element,
        &mut p_stats,
        &mut p_shield,
        &mut e_stats,
        &mut e_shield,
        &mut e_aura,
        &mut pending_boosts,
        &dbs.elements,
        &mut event_writer,
    );

    let should_go_check_end = query
        .get(p_entity)
        .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
        .unwrap_or(false)
        || query
            .get(e_entity)
            .map(|(_, _, s, _, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false);
    if should_go_check_end {
        turn_ctx.player_ended = true;
        next_phase.set(BattlePhase::CheckEnd);
        return;
    }

}
