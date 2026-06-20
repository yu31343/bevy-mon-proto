use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    battle::{
        ActionPoints, ActionTrace, BattleControlMode, BattleEvent, BattleFormulaEvent, BattleLog,
        BattleResult, BattleStatusEvent, Combatant, ElementAura, Hand, InBattle, PendingBoosts,
        PendingKoResolution, PendingTacticalDiscard, RoundOrder, SelectedCards, Shield, Side,
        SkillCount, SkillList, SkillUses, Stats, StructuredBattleLog, TurnAction, TurnContext,
        TurnCount, next_phase_after_side_end, note_action_phase, push_named_action_trace,
        push_turn_action_trace, transfer_status_by_id,
    },
    data::{BattleDbs, BattleRules},
    game_state::{BattlePhase, GameState},
    pvp,
    ui::battle::{components::BattleUiNotice, systems::HandFullEndTurnWarning},
};

use super::{
    SkillExecutionMode, SkillTargetMode, WindSpreadTarget, abort_battle, apply_effect,
    apply_self_effect, apply_wind_effect, process_side_end_statuses, skill_execution_mode,
    skill_target_mode,
};

#[derive(SystemParam)]
pub(crate) struct PlayerTurnLogs<'w> {
    turn_count: Res<'w, TurnCount>,
    structured_log: ResMut<'w, StructuredBattleLog>,
    action_trace: ResMut<'w, ActionTrace>,
}

#[derive(SystemParam)]
pub(crate) struct PlayerTurnEventWriters<'w> {
    event_writer: MessageWriter<'w, BattleEvent>,
    formula_writer: MessageWriter<'w, BattleFormulaEvent>,
    status_writer: MessageWriter<'w, BattleStatusEvent>,
    ui_notices: MessageWriter<'w, BattleUiNotice>,
}

#[derive(SystemParam)]
pub(crate) struct PlayerTurnRuntime<'w> {
    action_points: ResMut<'w, ActionPoints>,
    round_order: Res<'w, RoundOrder>,
    hand: ResMut<'w, Hand>,
    pending_boosts: ResMut<'w, PendingBoosts>,
    pending_ko: ResMut<'w, PendingKoResolution>,
    dbs: Res<'w, BattleDbs>,
    battle_rules: Res<'w, BattleRules>,
    formula_rules: Res<'w, crate::data::BattleFormulaRules>,
    accuracy_rng: ResMut<'w, crate::battle::AccuracyRng>,
    player_team: ResMut<'w, crate::battle::PlayerTeam>,
    enemy_team: Res<'w, crate::battle::EnemyTeam>,
    battle_log: ResMut<'w, BattleLog>,
    battle_result: ResMut<'w, BattleResult>,
    next_game_state: ResMut<'w, NextState<GameState>>,
    selected: ResMut<'w, SelectedCards>,
    hand_full_warning: ResMut<'w, HandFullEndTurnWarning>,
    battle_mode: Res<'w, BattleControlMode>,
    pvp_connection: Option<ResMut<'w, pvp::PvpConnection>>,
    pvp_pending_intent: Option<ResMut<'w, pvp::PvpPendingLocalIntent>>,
}

fn send_pvp_intent(
    battle_mode: &BattleControlMode,
    connection: &mut Option<ResMut<pvp::PvpConnection>>,
    pending_intent: &mut Option<ResMut<pvp::PvpPendingLocalIntent>>,
    intent: pvp::BattleIntent,
) -> bool {
    if *battle_mode != BattleControlMode::PlayerVsRemote {
        return false;
    }
    let Some(connection) = connection.as_mut() else {
        return false;
    };
    if connection.role != Some(pvp::PvpRole::Client) {
        return false;
    }
    if let Some(pending_intent) = pending_intent.as_mut() {
        pvp::send_local_intent(connection, pending_intent, intent);
    }
    true
}

fn predict_local_card_discard(
    cards: &mut Vec<crate::data::CardId>,
    ap: &mut i32,
    card_index: usize,
) {
    if card_index < cards.len() {
        cards.remove(card_index);
        *ap += 1;
    }
}

fn predict_local_card_use(
    cards: &mut Vec<crate::data::CardId>,
    ap: &mut i32,
    card_index: usize,
    cost_ap: i32,
) {
    if card_index < cards.len() {
        cards.remove(card_index);
        *ap -= cost_ap;
    }
}

fn predict_local_skill_use(
    action_points: &mut ActionPoints,
    skill_uses_q: &mut Query<&mut SkillUses, With<InBattle>>,
    entity: Entity,
    slot: usize,
    cost_ap: i32,
) {
    action_points.player -= cost_ap;
    if let Ok(mut uses) = skill_uses_q.get_mut(entity) {
        if let Some(remaining) = uses.0.get_mut(slot) {
            *remaining = remaining.saturating_sub(1);
        }
    }
}

fn finalize_player_turn(
    p_entity: Entity,
    player_team: &crate::battle::PlayerTeam,
    turn_ctx: &mut TurnContext,
    round_order: &RoundOrder,
    pending_boosts: &mut PendingBoosts,
    action_points: &mut ActionPoints,
    hand: &Hand,
    battle_rules: &BattleRules,
    commands: &mut Commands,
    formula_rules: &crate::data::BattleFormulaRules,
    logs: &mut PlayerTurnLogs,
    writers: &mut PlayerTurnEventWriters,
    next_phase: &mut ResMut<NextState<BattlePhase>>,
    query: &mut Query<
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
    let team_entities = player_team.0.combatants.clone();
    for entity in team_entities {
        if let Ok((_, _, mut stats, _, _, mut shield, mut statuses, mut aura, _)) =
            query.get_mut(entity)
        {
            process_side_end_statuses(
                &mut stats,
                &mut shield,
                &mut aura,
                &mut statuses,
                crate::battle::SideEndTickParams {
                    side: Side::Player,
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
    super::clear_action_scoped_card_effects(Side::Player, pending_boosts);
    commands.insert_resource(crate::battle::PendingGuardCounterClear {
        acting_side: Side::Player,
    });
    turn_ctx.player_ended = true;
    if let Ok((_, _, stats, _, _, _, _, _, _)) = query.get(p_entity) {
        if stats.hp <= 0 {
            super::clamp_ap_to_max(Side::Player, battle_rules, action_points);
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }
    }
    super::enter_discard_phase_or_continue(
        Side::Player,
        next_phase_after_side_end(round_order, Side::Player),
        hand,
        battle_rules,
        action_points,
        commands,
        next_phase,
    );
}

pub fn player_turn_input_system(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    pending_tactical_discard: Option<Res<PendingTacticalDiscard>>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut runtime: PlayerTurnRuntime,
    mut logs: PlayerTurnLogs,
    mut writers: PlayerTurnEventWriters,
    mut query: Query<
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
    mut skill_uses_q: Query<&mut SkillUses, With<InBattle>>,
) {
    let action_points = &mut runtime.action_points;
    let round_order = &runtime.round_order;
    let hand = &mut runtime.hand;
    let pending_boosts = &mut runtime.pending_boosts;
    let pending_ko = &mut runtime.pending_ko;
    let dbs = &runtime.dbs;
    let battle_rules = &runtime.battle_rules;
    let formula_rules = &runtime.formula_rules;
    let accuracy_rng = &mut runtime.accuracy_rng;
    let player_team = &mut runtime.player_team;
    let enemy_team = &runtime.enemy_team;
    let battle_log = &mut runtime.battle_log;
    let battle_result = &mut runtime.battle_result;
    let next_game_state = &mut runtime.next_game_state;
    let selected = &mut runtime.selected;
    let hand_full_warning = &mut runtime.hand_full_warning;
    let battle_mode = &runtime.battle_mode;
    let pvp_connection = &mut runtime.pvp_connection;
    let pvp_pending_intent = &mut runtime.pvp_pending_intent;

    if pvp_pending_intent.as_ref().is_some_and(|pending| {
        pending.0.is_some() && **battle_mode == BattleControlMode::PlayerVsRemote
    }) {
        return;
    }

    if turn_ctx.player_ended {
        return;
    }
    if player_team.0.combatants.is_empty() || enemy_team.0.combatants.is_empty() {
        abort_battle(
            "战斗数据异常：队伍为空。",
            battle_log,
            battle_result,
            next_game_state,
        );
        return;
    }

    let Some(p_entity) = player_team.0.active_combatant() else {
        abort_battle(
            "玩家上场精灵无效。",
            battle_log,
            battle_result,
            next_game_state,
        );
        return;
    };
    let Some(e_entity) = enemy_team.0.active_combatant() else {
        abort_battle(
            "敌方上场精灵无效。",
            battle_log,
            battle_result,
            next_game_state,
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
        if target_index >= player_team.0.combatants.len()
            || target_index == player_team.0.active_index
        {
            return;
        }
        if action_points.player < 1 {
            writers.ui_notices.write(BattleUiNotice { text: "AP不足" });
            return;
        }
        {
            let current_entity = player_team.0.combatants[player_team.0.active_index];
            let target_entity = player_team.0.combatants[target_index];
            if let Ok(
                [
                    (_, _, mut current_stats, _, _, _, mut current_statuses, _, _),
                    (_, _, target_stats, _, _, _, target_statuses, _, name),
                ],
            ) = query.get_many_mut([current_entity, target_entity])
            {
                if target_stats.hp > 0 {
                    if send_pvp_intent(
                        battle_mode,
                        pvp_connection,
                        pvp_pending_intent,
                        pvp::BattleIntent::Switch { target_index },
                    ) {
                        transfer_status_by_id(
                            &mut current_statuses,
                            &mut current_stats,
                            target_statuses.into_inner(),
                            target_stats.into_inner(),
                            "nature_regen",
                        );
                        action_points.player -= 1;
                        player_team.0.active_index = target_index;
                        return;
                    }
                    transfer_status_by_id(
                        &mut current_statuses,
                        &mut current_stats,
                        target_statuses.into_inner(),
                        target_stats.into_inner(),
                        "nature_regen",
                    );
                    action_points.player -= 1;
                    player_team.0.active_index = target_index;
                    writers.event_writer.write(BattleEvent::Switched {
                        side: Side::Player,
                        name: name.to_string(),
                    });
                    note_action_phase(
                        &mut logs.structured_log,
                        logs.turn_count.0,
                        Side::Player,
                        "玩家换人",
                        format!("切换到 {}；玩家AP={}", name, action_points.player),
                    );
                    push_turn_action_trace(
                        &mut logs.action_trace,
                        logs.turn_count.0,
                        Side::Player,
                        TurnAction::Switch,
                        format!("切换到 {}；剩余AP={}", name, action_points.player),
                    );
                }
            }
        }
        return;
    }

    // 1) 来自 UI 的按钮选择（若存在则优先执行）。
    if let Some(TurnAction::Skill(skill_id)) = turn_ctx.player_action {
        // 找到按钮对应的技能槽位（用于计算 AP 消耗与 UI 闪白）。
        let Ok((_, _, _, skill_list, skill_count, _, _, _, _)) = query.get(p_entity) else {
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
        let Some(skill) = dbs.skills.get(&skill_id) else {
            turn_ctx.player_action = None;
            return;
        };
        let cost = skill.cost_ap;

        // 该技能本回合释放次数已耗尽：提示并清空，避免下一帧重复触发。
        if !skill_uses_q
            .get(p_entity)
            .is_ok_and(|uses| uses.has_remaining(slot))
        {
            writers.ui_notices.write(BattleUiNotice {
                text: "次数不足"
            });
            turn_ctx.player_action = None;
            return;
        }

        // 如果 AP 不够：不执行并清空，避免下一帧重复触发。
        if action_points.player < cost {
            writers.ui_notices.write(BattleUiNotice { text: "AP不足" });
            turn_ctx.player_action = None;
            return;
        }

        if send_pvp_intent(
            battle_mode,
            pvp_connection,
            pvp_pending_intent,
            pvp::BattleIntent::UseSkill { slot },
        ) {
            predict_local_skill_use(action_points, &mut skill_uses_q, p_entity, slot, cost);
            turn_ctx.player_action = None;
            return;
        }
        action_points.player -= cost;
        turn_ctx.player_action = None;
        if let Ok(mut uses) = skill_uses_q.get_mut(p_entity) {
            if let Some(remaining) = uses.0.get_mut(slot) {
                *remaining = remaining.saturating_sub(1);
            }
        }

        let Ok(
            [
                (
                    _,
                    _attacker_combatant,
                    mut p_stats,
                    _,
                    _,
                    mut p_shield,
                    mut p_statuses,
                    mut p_aura,
                    _,
                ),
                (_, e_combatant, mut e_stats, _, _, mut e_shield, mut e_statuses, mut e_aura, _),
            ],
        ) = query.get_many_mut([p_entity, e_entity])
        else {
            return;
        };

        writers.event_writer.write(BattleEvent::SkillUsed {
            side: Side::Player,
            skill_name: skill.name.clone(),
            slot,
        });
        note_action_phase(
            &mut logs.structured_log,
            logs.turn_count.0,
            Side::Player,
            "玩家使用技能",
            format!(
                "技能={}；槽位={}；消耗AP={}；剩余AP={}",
                skill.name, slot, cost, action_points.player
            ),
        );
        push_turn_action_trace(
            &mut logs.action_trace,
            logs.turn_count.0,
            Side::Player,
            TurnAction::Skill(skill_id),
            format!(
                "技能={}；槽位={}；剩余AP={}",
                skill.name, slot, action_points.player
            ),
        );

        if skill_execution_mode(skill) == SkillExecutionMode::WindSpread {
            let enemy_backs = enemy_team
                .0
                .combatants
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(index, entity)| {
                    (index != enemy_team.0.active_index && entity != e_entity).then_some(entity)
                })
                .collect::<Vec<_>>();

            match enemy_backs.as_slice() {
                [back_a, back_b, ..] => {
                    let Ok(
                        [
                            (
                                _,
                                _attacker_combatant,
                                mut p_stats,
                                _,
                                _,
                                mut p_shield,
                                mut p_statuses,
                                _p_aura,
                                _,
                            ),
                            (
                                _,
                                e_combatant,
                                mut e_stats,
                                _,
                                _,
                                mut e_shield,
                                mut e_statuses,
                                mut e_aura,
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
                    ) = query.get_many_mut([p_entity, e_entity, *back_a, *back_b])
                    else {
                        return;
                    };

                    apply_wind_effect(
                        skill,
                        Side::Player,
                        Side::Enemy,
                        &mut p_stats,
                        &mut p_shield,
                        &mut p_statuses,
                        WindSpreadTarget {
                            base_element: e_combatant.element,
                            stats: &mut e_stats,
                            shield: &mut e_shield,
                            aura: &mut e_aura,
                            statuses: &mut e_statuses,
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
                        battle_rules,
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
                                _attacker_combatant,
                                mut p_stats,
                                _,
                                _,
                                mut p_shield,
                                mut p_statuses,
                                _p_aura,
                                _,
                            ),
                            (
                                _,
                                e_combatant,
                                mut e_stats,
                                _,
                                _,
                                mut e_shield,
                                mut e_statuses,
                                mut e_aura,
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
                    ) = query.get_many_mut([p_entity, e_entity, *back_a])
                    else {
                        return;
                    };

                    apply_wind_effect(
                        skill,
                        Side::Player,
                        Side::Enemy,
                        &mut p_stats,
                        &mut p_shield,
                        &mut p_statuses,
                        WindSpreadTarget {
                            base_element: e_combatant.element,
                            stats: &mut e_stats,
                            shield: &mut e_shield,
                            aura: &mut e_aura,
                            statuses: &mut e_statuses,
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
                        battle_rules,
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
                    apply_wind_effect(
                        skill,
                        Side::Player,
                        Side::Enemy,
                        &mut p_stats,
                        &mut p_shield,
                        &mut p_statuses,
                        WindSpreadTarget {
                            base_element: e_combatant.element,
                            stats: &mut e_stats,
                            shield: &mut e_shield,
                            aura: &mut e_aura,
                            statuses: &mut e_statuses,
                        },
                        None,
                        None,
                        pending_boosts,
                        &formula_rules,
                        battle_rules,
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
                    Side::Player,
                    &mut p_stats,
                    &mut p_shield,
                    &mut p_aura,
                    &mut p_statuses,
                    pending_boosts,
                    &formula_rules,
                    battle_rules,
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
                    Side::Player,
                    Side::Enemy,
                    e_combatant.element,
                    &mut p_stats,
                    &mut p_shield,
                    &mut p_statuses,
                    &mut e_stats,
                    &mut e_shield,
                    &mut e_aura,
                    &mut e_statuses,
                    pending_boosts,
                    &formula_rules,
                    battle_rules,
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

        let should_go_check_end = query
            .get(p_entity)
            .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false)
            || query
                .get(e_entity)
                .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false);
        if should_go_check_end {
            pending_ko.resume_phase = Some(BattlePhase::PlayerTurn);
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }

        return;
    }

    if pending_tactical_discard
        .as_ref()
        .is_some_and(|pending| pending.side == Side::Player)
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
            if idx >= hand.player.len() {
                return;
            }
            if send_pvp_intent(
                battle_mode,
                pvp_connection,
                pvp_pending_intent,
                pvp::BattleIntent::DiscardCard { card_index: idx },
            ) {
                predict_local_card_discard(&mut hand.player, &mut action_points.player, idx);
                selected.player.index = None;
                selected.player.discard_armed = false;
                return;
            }
            let card_id = hand.player.remove(idx);
            action_points.player += 1;
            let card_name = dbs
                .cards
                .get(&card_id)
                .map(|c| c.name.to_string())
                .unwrap_or_else(|| format!("{card_id:?}"));
            writers.event_writer.write(BattleEvent::CardDiscarded {
                side: Side::Player,
                card_name: card_name.clone(),
            });
            note_action_phase(
                &mut logs.structured_log,
                logs.turn_count.0,
                Side::Player,
                "玩家战术整理",
                format!(
                    "弃置卡牌={}；获得AP=1；当前AP={}",
                    card_name, action_points.player
                ),
            );
            push_named_action_trace(
                &mut logs.action_trace,
                logs.turn_count.0,
                Side::Player,
                "tactical_discard",
                format!("弃置卡牌={}；当前AP={}", card_name, action_points.player),
            );
            selected.player.index = None;
            selected.player.discard_armed = false;
            return;
        }
        return;
    }

    // 1) 手动结束回合（优先级最高）
    if keyboard.just_pressed(KeyCode::KeyE) {
        if hand.player.len() > battle_rules.max_retained_hand {
            hand_full_warning.trigger_end_turn_blocked();
            return;
        }
        turn_ctx.player_end_requested = true;
    }

    if turn_ctx.player_end_requested {
        if send_pvp_intent(
            battle_mode,
            pvp_connection,
            pvp_pending_intent,
            pvp::BattleIntent::EndTurn,
        ) {
            turn_ctx.player_end_requested = false;
            return;
        }
        note_action_phase(
            &mut logs.structured_log,
            logs.turn_count.0,
            Side::Player,
            "玩家结束回合",
            format!("玩家主动结束回合；剩余AP={}", action_points.player),
        );
        push_named_action_trace(
            &mut logs.action_trace,
            logs.turn_count.0,
            Side::Player,
            "end_turn",
            format!("玩家主动结束回合；剩余AP={}", action_points.player),
        );
        turn_ctx.player_end_requested = false;
        finalize_player_turn(
            p_entity,
            player_team,
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
            &mut query,
        );
        return;
    }

    // 2) 弃牌：弃置选中的牌；若无选中则武装弃牌模式（需再选一张才弃置）
    if keyboard.just_pressed(KeyCode::KeyF) {
        if hand.player.is_empty() {
            return;
        }
        let Some(target_index) = selected.player.index.filter(|&i| i < hand.player.len()) else {
            // 无已选牌：切换武装状态（再按一次 F 取消）
            selected.player.discard_armed = !selected.player.discard_armed;
            return;
        };
        if send_pvp_intent(
            battle_mode,
            pvp_connection,
            pvp_pending_intent,
            pvp::BattleIntent::DiscardCard {
                card_index: target_index,
            },
        ) {
            predict_local_card_discard(&mut hand.player, &mut action_points.player, target_index);
            selected.player.index = None;
            selected.player.discard_armed = false;
            return;
        }
        let card_id = hand.player.remove(target_index);
        action_points.player += 1;
        let card_name = dbs
            .cards
            .get(&card_id)
            .map(|c| c.name.to_string())
            .unwrap_or_else(|| format!("{card_id:?}"));
        writers.event_writer.write(BattleEvent::CardDiscarded {
            side: Side::Player,
            card_name: card_name.clone(),
        });
        note_action_phase(
            &mut logs.structured_log,
            logs.turn_count.0,
            Side::Player,
            "玩家弃牌",
            format!(
                "弃置卡牌={}；获得AP=1；当前AP={}",
                card_name, action_points.player
            ),
        );
        push_named_action_trace(
            &mut logs.action_trace,
            logs.turn_count.0,
            Side::Player,
            "discard_card",
            format!("弃置卡牌={}；当前AP={}", card_name, action_points.player),
        );
        selected.player.index = None;
        selected.player.discard_armed = false;
        // 弃牌可能使 AP 从 0 变为正，这种情况不触发自动结束。
        return;
    }

    // 3) 出牌/选牌（手牌热键按 UI 标注；两步式：首按选中/弹出，再按出牌；弃牌武装时直接弃置）
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
            if idx >= hand.player.len() {
                return;
            }
            // 若已武装弃牌模式（由点击"弃牌"按钮触发），直接弃置该牌
            if selected.player.discard_armed {
                if send_pvp_intent(
                    battle_mode,
                    pvp_connection,
                    pvp_pending_intent,
                    pvp::BattleIntent::DiscardCard { card_index: idx },
                ) {
                    predict_local_card_discard(&mut hand.player, &mut action_points.player, idx);
                    selected.player.index = None;
                    selected.player.discard_armed = false;
                    return;
                }
                let card_id = hand.player.remove(idx);
                action_points.player += 1;
                let card_name = dbs
                    .cards
                    .get(&card_id)
                    .map(|c| c.name.to_string())
                    .unwrap_or_else(|| format!("{card_id:?}"));
                writers.event_writer.write(BattleEvent::CardDiscarded {
                    side: Side::Player,
                    card_name: card_name.clone(),
                });
                note_action_phase(
                    &mut logs.structured_log,
                    logs.turn_count.0,
                    Side::Player,
                    "玩家弃牌",
                    format!(
                        "弃置卡牌={}；获得AP=1；当前AP={}",
                        card_name, action_points.player
                    ),
                );
                push_named_action_trace(
                    &mut logs.action_trace,
                    logs.turn_count.0,
                    Side::Player,
                    "discard_card",
                    format!("弃置卡牌={}；当前AP={}", card_name, action_points.player),
                );
                selected.player.index = None;
                selected.player.discard_armed = false;
                return;
            }
            // 第一步：选中该牌（显示描述）
            if selected.player.index != Some(idx) {
                selected.player.index = Some(idx);
                return;
            }
            // 第二步：出牌
            let card_id = hand.player[idx];
            if let Some(card) = dbs.cards.get(&card_id) {
                if action_points.player < card.cost_ap {
                    writers.ui_notices.write(BattleUiNotice { text: "AP不足" });
                    return;
                }
                if send_pvp_intent(
                    battle_mode,
                    pvp_connection,
                    pvp_pending_intent,
                    pvp::BattleIntent::UseCard { card_index: idx },
                ) {
                    predict_local_card_use(
                        &mut hand.player,
                        &mut action_points.player,
                        idx,
                        card.cost_ap,
                    );
                    selected.player.index = None;
                    selected.player.discard_armed = false;
                    return;
                }
                hand.player.remove(idx);
                action_points.player -= card.cost_ap;

                let card_name = card.name.to_string();
                writers.event_writer.write(BattleEvent::CardUsed {
                    side: Side::Player,
                    card_name: card_name.clone(),
                });

                let effect_detail = "效果已排入卡牌结算".to_string();
                note_action_phase(
                    &mut logs.structured_log,
                    logs.turn_count.0,
                    Side::Player,
                    "玩家使用卡牌",
                    format!(
                        "卡牌={}；消耗AP={}；效果={}；当前AP={}",
                        card_name, card.cost_ap, effect_detail, action_points.player
                    ),
                );
                push_named_action_trace(
                    &mut logs.action_trace,
                    logs.turn_count.0,
                    Side::Player,
                    "use_card",
                    format!(
                        "卡牌={}；效果={}；当前AP={}",
                        card_name, effect_detail, action_points.player
                    ),
                );

                selected.player.index = None;
                selected.player.discard_armed = false;

                let should_go_check_end = query
                    .get(p_entity)
                    .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
                    .unwrap_or(false)
                    || query
                        .get(e_entity)
                        .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
                        .unwrap_or(false);
                if should_go_check_end {
                    pending_ko.resume_phase = Some(BattlePhase::PlayerTurn);
                    next_phase.set(BattlePhase::CheckEnd);
                    return;
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
    let Ok((_, _, _, skill_list, skill_count, _, _, _, _)) = query.get(p_entity) else {
        return;
    };
    if skill_slot >= skill_count.0 {
        return;
    }
    let skill_id = skill_list.0[skill_slot];
    let Some(skill) = dbs.skills.get(&skill_id) else {
        return;
    };
    let cost = skill.cost_ap;
    if !skill_uses_q
        .get(p_entity)
        .is_ok_and(|uses| uses.has_remaining(skill_slot))
    {
        writers.ui_notices.write(BattleUiNotice {
            text: "次数不足"
        });
        return;
    }
    if action_points.player < cost {
        writers.ui_notices.write(BattleUiNotice { text: "AP不足" });
        return;
    }

    if send_pvp_intent(
        battle_mode,
        pvp_connection,
        pvp_pending_intent,
        pvp::BattleIntent::UseSkill { slot: skill_slot },
    ) {
        predict_local_skill_use(action_points, &mut skill_uses_q, p_entity, skill_slot, cost);
        return;
    }
    action_points.player -= cost;
    if let Ok(mut uses) = skill_uses_q.get_mut(p_entity) {
        if let Some(remaining) = uses.0.get_mut(skill_slot) {
            *remaining = remaining.saturating_sub(1);
        }
    }

    // 事件：技能使用（用于 UI 闪白）。
    writers.event_writer.write(BattleEvent::SkillUsed {
        side: Side::Player,
        skill_name: skill.name.clone(),
        slot: skill_slot,
    });
    note_action_phase(
        &mut logs.structured_log,
        logs.turn_count.0,
        Side::Player,
        "玩家使用技能",
        format!(
            "技能={}；槽位={}；消耗AP={}；剩余AP={}",
            skill.name, skill_slot, cost, action_points.player
        ),
    );
    push_turn_action_trace(
        &mut logs.action_trace,
        logs.turn_count.0,
        Side::Player,
        TurnAction::Skill(skill_id),
        format!(
            "技能={}；槽位={}；剩余AP={}",
            skill.name, skill_slot, action_points.player
        ),
    );

    if skill_execution_mode(skill) == SkillExecutionMode::WindSpread {
        let enemy_backs = enemy_team
            .0
            .combatants
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, entity)| {
                (index != enemy_team.0.active_index && entity != e_entity).then_some(entity)
            })
            .collect::<Vec<_>>();

        match enemy_backs.as_slice() {
            [back_a, back_b, ..] => {
                let Ok(
                    [
                        (
                            _,
                            _attacker_combatant,
                            mut p_stats,
                            _,
                            _,
                            mut p_shield,
                            mut p_statuses,
                            _p_aura,
                            _,
                        ),
                        (
                            _,
                            e_combatant,
                            mut e_stats,
                            _,
                            _,
                            mut e_shield,
                            mut e_statuses,
                            mut e_aura,
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
                ) = query.get_many_mut([p_entity, e_entity, *back_a, *back_b])
                else {
                    return;
                };

                apply_wind_effect(
                    skill,
                    Side::Player,
                    Side::Enemy,
                    &mut p_stats,
                    &mut p_shield,
                    &mut p_statuses,
                    WindSpreadTarget {
                        base_element: e_combatant.element,
                        stats: &mut e_stats,
                        shield: &mut e_shield,
                        aura: &mut e_aura,
                        statuses: &mut e_statuses,
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
                    battle_rules,
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
                            _attacker_combatant,
                            mut p_stats,
                            _,
                            _,
                            mut p_shield,
                            mut p_statuses,
                            _p_aura,
                            _,
                        ),
                        (
                            _,
                            e_combatant,
                            mut e_stats,
                            _,
                            _,
                            mut e_shield,
                            mut e_statuses,
                            mut e_aura,
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
                ) = query.get_many_mut([p_entity, e_entity, *back_a])
                else {
                    return;
                };

                apply_wind_effect(
                    skill,
                    Side::Player,
                    Side::Enemy,
                    &mut p_stats,
                    &mut p_shield,
                    &mut p_statuses,
                    WindSpreadTarget {
                        base_element: e_combatant.element,
                        stats: &mut e_stats,
                        shield: &mut e_shield,
                        aura: &mut e_aura,
                        statuses: &mut e_statuses,
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
                    battle_rules,
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
                            _attacker_combatant,
                            mut p_stats,
                            _,
                            _,
                            mut p_shield,
                            mut p_statuses,
                            _p_aura,
                            _,
                        ),
                        (
                            _,
                            e_combatant,
                            mut e_stats,
                            _,
                            _,
                            mut e_shield,
                            mut e_statuses,
                            mut e_aura,
                            _,
                        ),
                    ],
                ) = query.get_many_mut([p_entity, e_entity])
                else {
                    return;
                };

                apply_wind_effect(
                    skill,
                    Side::Player,
                    Side::Enemy,
                    &mut p_stats,
                    &mut p_shield,
                    &mut p_statuses,
                    WindSpreadTarget {
                        base_element: e_combatant.element,
                        stats: &mut e_stats,
                        shield: &mut e_shield,
                        aura: &mut e_aura,
                        statuses: &mut e_statuses,
                    },
                    None,
                    None,
                    pending_boosts,
                    &formula_rules,
                    battle_rules,
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
                    mut p_stats,
                    _,
                    _,
                    mut p_shield,
                    mut p_statuses,
                    mut p_aura,
                    _,
                ),
                (_, e_combatant, mut e_stats, _, _, mut e_shield, mut e_statuses, mut e_aura, _),
            ],
        ) = query.get_many_mut([p_entity, e_entity])
        else {
            return;
        };

        match skill_target_mode(skill) {
            SkillTargetMode::SelfOnly => apply_self_effect(
                skill,
                Side::Player,
                &mut p_stats,
                &mut p_shield,
                &mut p_aura,
                &mut p_statuses,
                pending_boosts,
                &formula_rules,
                battle_rules,
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
                Side::Player,
                Side::Enemy,
                e_combatant.element,
                &mut p_stats,
                &mut p_shield,
                &mut p_statuses,
                &mut e_stats,
                &mut e_shield,
                &mut e_aura,
                &mut e_statuses,
                pending_boosts,
                &formula_rules,
                battle_rules,
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

    let should_go_check_end = query
        .get(p_entity)
        .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
        .unwrap_or(false)
        || query
            .get(e_entity)
            .map(|(_, _, s, _, _, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false);
    if should_go_check_end {
        pending_ko.resume_phase = Some(BattlePhase::PlayerTurn);
        next_phase.set(BattlePhase::CheckEnd);
        return;
    }
}
