use bevy::prelude::*;

use crate::{
    data::{BattleDataStatus, CardDb, CardDeck, CardEffect, SkillDb, SkillEffect, TeamSetup},
    game_state::{BattlePhase, GameState},
};

use super::{
    AP_PER_ROUND, ActionPoints, BattleEvent, BattleLog, BattleResult, CARDS_PER_ROUND, Combatant,
    ElementAura, Hand, InBattle, PendingBoosts, Shield, Side, SkillList, Stats, TurnAction,
    TurnContext, TurnCount, push_battle_line,
};

/// 初始化战斗：清理旧实体、生成双方单位并进入玩家指令阶段。
pub fn init_battle_system(
    mut commands: Commands,
    setup: Res<TeamSetup>,
    skill_db: Res<SkillDb>,
    data_status: Option<Res<BattleDataStatus>>,
    mut turn_ctx: ResMut<TurnContext>,
    mut battle_log: ResMut<BattleLog>,
    mut result: ResMut<BattleResult>,
    mut turn_count: ResMut<TurnCount>,
    cleanup_query: Query<Entity, With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    for entity in &cleanup_query {
        commands.entity(entity).despawn();
    }

    turn_ctx.player_action = None;
    turn_ctx.enemy_action = None;
    turn_ctx.player_ended = false;
    turn_ctx.enemy_ended = false;
    battle_log.0.clear();
    result.message.clear();
    turn_count.0 = 0;

    // 回合进度状态初始化（在每次“战斗重开”时重置）。
    commands.insert_resource(ActionPoints {
        player: 0,
        enemy: 0,
    });
    commands.insert_resource(Hand::default());
    commands.insert_resource(PendingBoosts::default());

    if let Some(status) = data_status {
        if let Some(reason) = &status.error {
            abort_battle(
                &format!("战斗初始化失败：{reason}"),
                &mut battle_log,
                &mut result,
                &mut next_game_state,
            );
            return;
        }
    }

    if setup.player.is_empty() || setup.enemy.is_empty() {
        abort_battle(
            "战斗初始化失败：队伍配置不能为空。",
            &mut battle_log,
            &mut result,
            &mut next_game_state,
        );
        return;
    }

    if skill_db.0.is_empty() {
        abort_battle(
            "战斗初始化失败：技能数据库为空。",
            &mut battle_log,
            &mut result,
            &mut next_game_state,
        );
        return;
    }

    for mon in setup.player.iter().chain(setup.enemy.iter()) {
        for sid in mon.skills {
            if !skill_db.0.contains_key(&sid) {
                abort_battle(
                    &format!("战斗初始化失败：{} 存在未定义技能 {:?}。", mon.name, sid),
                    &mut battle_log,
                    &mut result,
                    &mut next_game_state,
                );
                return;
            }
        }
    }

    let mut player_team = crate::battle::Team {
        combatants: vec![],
        active_index: 0,
    };

    let mut enemy_team = crate::battle::Team {
        combatants: vec![],
        active_index: 0,
    };

    for combatant in &setup.player {
        let entity = commands
            .spawn((
                Name::new(combatant.name.clone()),
                InBattle,
                Combatant {
                    side: Side::Player,
                    element: combatant.element,
                },
                Stats {
                    hp: combatant.stats.hp,
                    max_hp: combatant.stats.hp,
                    atk: combatant.stats.atk,
                    def: combatant.stats.def,
                    spd: combatant.stats.spd,
                },
                SkillList(combatant.skills),
                Shield::default(),
                ElementAura {
                    attached: match combatant.element {
                        crate::data::ElementType::Fire
                        | crate::data::ElementType::Light
                        | crate::data::ElementType::Dark => Some(combatant.element),
                        _ => None,
                    },
                },
            ))
            .id();
        player_team.combatants.push(entity);
    }

    for combatant in &setup.enemy {
        let entity = commands
            .spawn((
                Name::new(combatant.name.clone()),
                InBattle,
                Combatant {
                    side: Side::Enemy,
                    element: combatant.element,
                },
                Stats {
                    hp: combatant.stats.hp,
                    max_hp: combatant.stats.hp,
                    atk: combatant.stats.atk,
                    def: combatant.stats.def,
                    spd: combatant.stats.spd,
                },
                SkillList(combatant.skills),
                Shield::default(),
                ElementAura {
                    attached: match combatant.element {
                        crate::data::ElementType::Fire
                        | crate::data::ElementType::Light
                        | crate::data::ElementType::Dark => Some(combatant.element),
                        _ => None,
                    },
                },
            ))
            .id();
        enemy_team.combatants.push(entity);
    }

    if player_team.combatants.is_empty() || enemy_team.combatants.is_empty() {
        abort_battle(
            "战斗初始化失败：生成战斗实体后队伍为空。",
            &mut battle_log,
            &mut result,
            &mut next_game_state,
        );
        return;
    }

    commands.insert_resource(crate::battle::PlayerTeam(player_team));
    commands.insert_resource(crate::battle::EnemyTeam(enemy_team));

    push_battle_line(
        &mut battle_log,
        "战斗已开始：每回合开始抽取手牌并获得行动点；按 1-4 使用精灵技能，按 F 弃牌换 AP，按 E 结束回合。",
    );
    next_phase.set(BattlePhase::RoundStart);
}

fn monster_skill_ap_cost(slot: usize) -> i32 {
    match slot {
        0 => 2, // 普通攻击
        1 => 3, // 元素战技
        2 => 1, // 防御（护盾）
        3 => 1, // 治疗
        _ => 999,
    }
}

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

/// 回合开始：抽 5 张牌 + 给双方本回合 AP（“叠加 +6”，不清零继承）。
pub fn round_start_system(
    card_deck: Res<CardDeck>,
    card_db: Res<CardDb>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    mut turn_ctx: ResMut<TurnContext>,
    mut turn_count: ResMut<TurnCount>,
    mut pending_boosts: ResMut<PendingBoosts>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    if card_deck.0.is_empty() {
        return;
    }

    turn_count.0 += 1;
    event_writer.write(BattleEvent::TurnStarted(turn_count.0));

    // 每回合开始抽取手牌（重置本回合手牌）。
    hand.player.clear();
    hand.enemy.clear();
    let deck_len = card_deck.0.len();
    let seed = turn_count.0 as usize;
    for i in 0..CARDS_PER_ROUND {
        let idx = (seed * 7 + i * 3) % deck_len;
        hand.player.push(card_deck.0[idx]);
    }
    for i in 0..CARDS_PER_ROUND {
        let idx = (seed * 11 + i * 5) % deck_len;
        hand.enemy.push(card_deck.0[idx]);
    }

    let player_cards: Vec<String> = hand
        .player
        .iter()
        .map(|cid| {
            card_db
                .0
                .get(cid)
                .map(|c| c.name.to_string())
                .unwrap_or_else(|| format!("{cid:?}"))
        })
        .collect();
    let enemy_cards: Vec<String> = hand
        .enemy
        .iter()
        .map(|cid| {
            card_db
                .0
                .get(cid)
                .map(|c| c.name.to_string())
                .unwrap_or_else(|| format!("{cid:?}"))
        })
        .collect();
    println!("玩家抽到: {}", player_cards.join(" / "));
    println!("敌方抽到: {}", enemy_cards.join(" / "));

    // “回合开始时额外 +6”，并保留继承的剩余 AP。
    action_points.player += AP_PER_ROUND;
    action_points.enemy += AP_PER_ROUND;

    // 重置本回合出牌权标记。
    turn_ctx.player_ended = false;
    turn_ctx.enemy_ended = false;

    // PendingBoost 不在这里清空：它应该“用于下一次对应技能”，可以跨回合继承。
    let _ = &mut pending_boosts;

    next_phase.set(BattlePhase::PlayerTurn);
}

/// 玩家回合：可多次行动，直到 AP 为 0 或手动结束。
pub fn player_turn_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    mut pending_boosts: ResMut<PendingBoosts>,
    card_db: Res<CardDb>,
    skill_db: Res<SkillDb>,
    mut player_team: ResMut<crate::battle::PlayerTeam>,
    enemy_team: Res<crate::battle::EnemyTeam>,
    mut battle_log: ResMut<BattleLog>,
    mut battle_result: ResMut<BattleResult>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut query: Query<
        (
            Entity,
            &Combatant,
            &mut Stats,
            &SkillList,
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
    if action_points.player <= 0 {
        turn_ctx.player_ended = true;
        next_phase.set(BattlePhase::EnemyTurn);
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
            if let Ok((_, _, stats, _, _, _, name)) = query.get(target_entity) {
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
        let Ok((_, _, _, skill_list, _, _, _)) = query.get(p_entity) else {
            turn_ctx.player_action = None;
            return;
        };
        let slot = skill_list
            .0
            .iter()
            .position(|&s| s == skill_id)
            .unwrap_or(0);
        let cost = monster_skill_ap_cost(slot);
        let Some(skill) = skill_db.0.get(&skill_id) else {
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
                (_, attacker_combatant, mut p_stats, _, mut p_shield, mut _p_aura, _),
                (_, e_combatant, mut e_stats, _, mut e_shield, mut e_aura, _),
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
            &mut event_writer,
        );

        let should_go_check_end = query
            .get(p_entity)
            .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false)
            || query
                .get(e_entity)
                .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false);
        if should_go_check_end {
            turn_ctx.player_ended = true;
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }

        if action_points.player <= 0 {
            turn_ctx.player_ended = true;
            next_phase.set(BattlePhase::EnemyTurn);
        }
        return;
    }

    // 1) 手动结束回合（优先级最高）
    if keyboard.just_pressed(KeyCode::KeyE) {
        turn_ctx.player_ended = true;
        next_phase.set(BattlePhase::EnemyTurn);
        return;
    }

    // 2) 弃牌：弃置 1 张技能牌，获得 +1 AP
    if keyboard.just_pressed(KeyCode::KeyF) {
        if !hand.player.is_empty() {
            let card_id = hand.player.remove(0);
            action_points.player += 1;
            let card_name = card_db
                .0
                .get(&card_id)
                .map(|c| c.name.to_string())
                .unwrap_or_else(|| format!("{card_id:?}"));
            event_writer.write(BattleEvent::CardDiscarded {
                side: Side::Player,
                card_name,
            });
        }
        // 弃牌可能使 AP 从 0 变为正，这种情况不触发自动结束。
        return;
    }

    // 3) 出牌（手牌 5 张热键：Z X C V B）
    for (key, idx) in [
        (KeyCode::KeyZ, 0_usize),
        (KeyCode::KeyX, 1_usize),
        (KeyCode::KeyC, 2_usize),
        (KeyCode::KeyV, 3_usize),
        (KeyCode::KeyB, 4_usize),
    ] {
        if keyboard.just_pressed(key) {
            if idx < hand.player.len() {
                let card_id = hand.player[idx];
                if let Some(card) = card_db.0.get(&card_id) {
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
                                pending_boosts.player.next_attack_bonus += amount;
                            }
                            CardEffect::NextShieldBoost { amount } => {
                                pending_boosts.player.next_shield_bonus += amount;
                            }
                            CardEffect::NextHealBoost { amount } => {
                                pending_boosts.player.next_heal_bonus += amount;
                            }
                        }

                        let should_go_check_end = query
                            .get(p_entity)
                            .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
                            .unwrap_or(false)
                            || query
                                .get(e_entity)
                                .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
                                .unwrap_or(false);
                        if should_go_check_end {
                            turn_ctx.player_ended = true;
                            next_phase.set(BattlePhase::CheckEnd);
                            return;
                        }

                        if action_points.player <= 0 {
                            turn_ctx.player_ended = true;
                            next_phase.set(BattlePhase::EnemyTurn);
                        }
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
    let Ok((_, _, _, skill_list, _, _, _)) = query.get(p_entity) else {
        return;
    };
    let skill_id = skill_list.0[skill_slot];
    let cost = monster_skill_ap_cost(skill_slot);
    if action_points.player < cost {
        return;
    }

    let Some(skill) = skill_db.0.get(&skill_id) else {
        return;
    };

    action_points.player -= cost;

    // 执行技能：玩家为攻击方/施术方。
    let Ok(
        [
            (_, attacker_combatant, mut p_stats, _, mut p_shield, mut _p_aura, _),
            (_, e_combatant, mut e_stats, _, mut e_shield, mut e_aura, _),
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
        &mut event_writer,
    );

    let should_go_check_end = query
        .get(p_entity)
        .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
        .unwrap_or(false)
        || query
            .get(e_entity)
            .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false);
    if should_go_check_end {
        turn_ctx.player_ended = true;
        next_phase.set(BattlePhase::CheckEnd);
        return;
    }

    if action_points.player <= 0 {
        turn_ctx.player_ended = true;
        next_phase.set(BattlePhase::EnemyTurn);
    }
}

/// 敌方 AI：连续行动，直到 AP 为 0。
pub fn enemy_turn_ai_system(
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    mut pending_boosts: ResMut<PendingBoosts>,
    card_db: Res<CardDb>,
    skill_db: Res<SkillDb>,
    player_team: Res<crate::battle::PlayerTeam>,
    enemy_team: Res<crate::battle::EnemyTeam>,
    mut battle_log: ResMut<BattleLog>,
    mut battle_result: ResMut<BattleResult>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut exec_query: Query<
        (
            Entity,
            &Combatant,
            &mut Stats,
            &SkillList,
            &mut Shield,
            &mut ElementAura,
            &Name,
        ),
        With<InBattle>,
    >,
) {
    if turn_ctx.enemy_ended {
        return;
    }
    if action_points.enemy <= 0 {
        turn_ctx.enemy_ended = true;
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

    while action_points.enemy > 0 {
        let should_go_check_end = exec_query
            .get(p_entity)
            .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false)
            || exec_query
                .get(e_entity)
                .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false);
        if should_go_check_end {
            turn_ctx.enemy_ended = true;
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }

        let Ok(
            [
                (_, e_combatant, mut e_stats_m, e_skills_m, mut e_shield_m, mut e_aura_m, _),
                (_, p_combatant, mut p_stats_m, _, mut p_shield_m, mut p_aura_m, _),
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

        let enemy_hp_pct = if e_max_hp > 0 {
            (e_hp.max(0) * 100) / e_max_hp
        } else {
            0
        };

        // 先决定是否打牌：优先补“对接下一次技能”的 PendingBoost。
        let mut played_card = false;
        if pending_boosts.enemy.next_attack_bonus == 0 && action_points.enemy >= 2 {
            let can_attack0 = action_points.enemy >= monster_skill_ap_cost(0);
            let can_attack1 = action_points.enemy >= monster_skill_ap_cost(1);
            if can_attack0 || can_attack1 {
                if let Some((idx, _)) = hand.enemy.iter().enumerate().find(|(_, cid)| {
                    card_db.0.get(cid).is_some_and(|c| {
                        matches!(c.effect, CardEffect::NextAttackBoost { .. })
                            && c.cost_ap <= action_points.enemy
                    })
                }) {
                    let card_id = hand.enemy.remove(idx);
                    if let Some(card) = card_db.0.get(&card_id) {
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
            && action_points.enemy >= monster_skill_ap_cost(3)
        {
            if let Some((idx, _)) = hand.enemy.iter().enumerate().find(|(_, cid)| {
                card_db.0.get(cid).is_some_and(|c| {
                    matches!(c.effect, CardEffect::NextHealBoost { .. })
                        && c.cost_ap <= action_points.enemy
                })
            }) {
                let card_id = hand.enemy.remove(idx);
                if let Some(card) = card_db.0.get(&card_id) {
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
            && action_points.enemy >= monster_skill_ap_cost(2)
        {
            if let Some((idx, _)) = hand.enemy.iter().enumerate().find(|(_, cid)| {
                card_db.0.get(cid).is_some_and(|c| {
                    matches!(c.effect, CardEffect::NextShieldBoost { .. })
                        && c.cost_ap <= action_points.enemy
                })
            }) {
                let card_id = hand.enemy.remove(idx);
                if let Some(card) = card_db.0.get(&card_id) {
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
            if action_points.enemy <= 0 {
                break;
            }
            continue;
        }

        // 否则优先使用精灵技能（根据 HP 简单选择）。
        let mut chosen_slot: Option<usize> = None;

        if enemy_hp_pct < 40 && action_points.enemy >= monster_skill_ap_cost(3) {
            chosen_slot = Some(3);
        } else if action_points.enemy >= monster_skill_ap_cost(2) && e_shield_value <= 0 {
            chosen_slot = Some(2);
        } else if action_points.enemy >= monster_skill_ap_cost(1) {
            chosen_slot = Some(1);
        } else if action_points.enemy >= monster_skill_ap_cost(0) {
            chosen_slot = Some(0);
        }

        if let Some(slot) = chosen_slot {
            let skill_id = e_skills_arr[slot];
            let Some(skill) = skill_db.0.get(&skill_id) else {
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
                &mut event_writer,
            );

            let should_go_check_end = exec_query
                .get(p_entity)
                .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false)
                || exec_query
                    .get(e_entity)
                    .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
                    .unwrap_or(false);
            if should_go_check_end {
                turn_ctx.enemy_ended = true;
                next_phase.set(BattlePhase::CheckEnd);
                return;
            }
        } else {
            // 没有可用技能：弃牌换 AP（或直接结束）
            if !hand.enemy.is_empty() {
                let card_id = hand.enemy.remove(0);
                action_points.enemy += 1;
                let card_name = card_db
                    .0
                    .get(&card_id)
                    .map(|c| c.name.to_string())
                    .unwrap_or_else(|| format!("{card_id:?}"));
                event_writer.write(BattleEvent::CardDiscarded {
                    side: Side::Enemy,
                    card_name,
                });
            } else {
                break;
            }
        }

        let should_go_check_end = exec_query
            .get(p_entity)
            .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
            .unwrap_or(false)
            || exec_query
                .get(e_entity)
                .map(|(_, _, s, _, _, _, _)| s.hp <= 0)
                .unwrap_or(false);
        if should_go_check_end {
            turn_ctx.enemy_ended = true;
            next_phase.set(BattlePhase::CheckEnd);
            return;
        }

        if action_points.enemy <= 0 {
            break;
        }
    }

    turn_ctx.enemy_ended = true;
    next_phase.set(BattlePhase::CheckEnd);
}

/// 处理玩家输入（键盘 1-4 / Space），记录本回合玩家技能。按 Q 切换精灵。
pub fn player_input_system(
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

/// 敌方 AI 选招：低血优先治疗，被克制时提高防御权重，否则按收益选择。
pub fn enemy_choose_skill_system(
    mut turn_ctx: ResMut<TurnContext>,
    skill_db: Res<SkillDb>,
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
    let under_counter = crate::data::ElementMatrix::get_effectiveness(
        player_combatant.element,
        enemy_combatant.element,
    ) > 1.0;

    let mut heal_candidates = Vec::new();
    let mut best_scored: Option<(crate::data::SkillId, i32)> = None;

    for skill_id in enemy_skills.0 {
        let Some(skill) = skill_db.0.get(&skill_id) else {
            continue;
        };

        let score = match &skill.effect {
            SkillEffect::Attack { power } => {
                let base_damage = *power + enemy_stats.atk - player_stats.def;
                let effectiveness = if let Some(skill_element) = skill.element {
                    crate::data::ElementMatrix::get_effectiveness(
                        skill_element,
                        player_combatant.element,
                    )
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
        .find(|sid| skill_db.0.contains_key(sid));

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

/// 回合结算：按行动优先级排序（换人 > 防御/治疗 > 攻击），同优先级按速度。
pub fn resolve_turn_system(
    mut query: Query<
        (
            Entity,
            &Combatant,
            &mut Stats,
            &SkillList,
            &mut Shield,
            &mut ElementAura,
            &Name,
        ),
        With<InBattle>,
    >,
    mut turn_ctx: ResMut<TurnContext>,
    skill_db: Res<SkillDb>,
    player_team: Res<crate::battle::PlayerTeam>,
    enemy_team: Res<crate::battle::EnemyTeam>,
    mut battle_log: ResMut<BattleLog>,
    mut battle_result: ResMut<BattleResult>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut turn_count: ResMut<TurnCount>,
    mut pending_boosts: ResMut<PendingBoosts>,
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

    let Ok((_, _, p_stats, _, _, _, _)) = query.get(player_entity) else {
        abort_battle(
            "回合结算失败：玩家成员数据缺失。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    let Ok((_, _, e_stats, _, _, _, _)) = query.get(enemy_entity) else {
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
            action_priority(&player_action, &skill_db),
            player_spd,
            0_u8,
        ),
        (
            Side::Enemy,
            enemy_action,
            action_priority(&enemy_action, &skill_db),
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
            if let Ok((_, _, _, _, _, _, name)) = query.get(active_side_entity) {
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
            .map(|(_, _, _, sl, _, _, _)| sl.0.iter().position(|&s| s == skill_id).unwrap_or(0))
            .unwrap_or(0);

        let Ok(
            [
                (_, a_combatant, mut a_stats, _, mut a_shield, mut a_aura, _),
                (_, t_combatant, mut t_stats, _, mut t_shield, mut t_aura, _),
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

        let Some(skill) = skill_db.0.get(&skill_id) else {
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
                &skill.effect,
                skill.element,
                attacker_side,
                target_side,
                t_combatant.element,
                &mut a_stats,
                &mut a_shield,
                &mut t_stats,
                &mut t_shield,
                &mut t_aura,
                &mut pending_boosts,
                &mut event_writer,
            );
        } else {
            apply_effect(
                &skill.effect,
                skill.element,
                attacker_side,
                target_side,
                a_combatant.element,
                &mut t_stats,
                &mut t_shield,
                &mut a_stats,
                &mut a_shield,
                &mut a_aura,
                &mut pending_boosts,
                &mut event_writer,
            );
        }
    }

    turn_ctx.player_action = None;
    turn_ctx.enemy_action = None;
    next_phase.set(BattlePhase::CheckEnd);
}

/// 判定胜负并切换到结果页，如果当前精灵倒下则自动切换，全队倒下才结束战斗。
pub fn check_end_system(
    query: Query<(&Combatant, &Stats, &Name), With<InBattle>>,
    mut player_team: ResMut<crate::battle::PlayerTeam>,
    mut enemy_team: ResMut<crate::battle::EnemyTeam>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut battle_result: ResMut<BattleResult>,
    mut battle_log: ResMut<BattleLog>,
) {
    let Some(p_entity) = player_team.0.active_combatant() else {
        abort_battle(
            "结算失败：玩家上场成员无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    let mut p_dead = false;
    let Ok((_, p_stats, p_name)) = query.get(p_entity) else {
        abort_battle(
            "结算失败：玩家成员数据缺失。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    if p_stats.hp <= 0 {
        p_dead = true;
        event_writer.write(BattleEvent::CombatantFainted {
            side: Side::Player,
            name: p_name.to_string(),
        });

        let mut next_idx = None;
        for (i, &e) in player_team.0.combatants.iter().enumerate() {
            if let Ok((_, s, _)) = query.get(e) {
                if s.hp > 0 {
                    next_idx = Some(i);
                    break;
                }
            }
        }

        if let Some(idx) = next_idx {
            player_team.0.active_index = idx;
            p_dead = false;
            let new_e = player_team.0.combatants[idx];
            if let Ok((_, _, new_n)) = query.get(new_e) {
                event_writer.write(BattleEvent::Switched {
                    side: Side::Player,
                    name: new_n.to_string(),
                });
                push_battle_line(&mut battle_log, format!("玩家换上了 {}！", new_n));
            }
        }
    }

    let Some(e_entity) = enemy_team.0.active_combatant() else {
        abort_battle(
            "结算失败：敌方上场成员无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    let mut e_dead = false;
    let Ok((_, e_stats, e_name)) = query.get(e_entity) else {
        abort_battle(
            "结算失败：敌方成员数据缺失。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    if e_stats.hp <= 0 {
        e_dead = true;
        event_writer.write(BattleEvent::CombatantFainted {
            side: Side::Enemy,
            name: e_name.to_string(),
        });

        let mut next_idx = None;
        for (i, &e) in enemy_team.0.combatants.iter().enumerate() {
            if let Ok((_, s, _)) = query.get(e) {
                if s.hp > 0 {
                    next_idx = Some(i);
                    break;
                }
            }
        }

        if let Some(idx) = next_idx {
            enemy_team.0.active_index = idx;
            e_dead = false;
            let new_e = enemy_team.0.combatants[idx];
            if let Ok((_, _, new_n)) = query.get(new_e) {
                event_writer.write(BattleEvent::Switched {
                    side: Side::Enemy,
                    name: new_n.to_string(),
                });
                push_battle_line(&mut battle_log, format!("敌方换上了 {}！", new_n));
            }
        }
    }

    if p_dead || e_dead {
        battle_result.message = if p_dead && e_dead {
            "平局！按 R 重新开始。".to_string()
        } else if e_dead {
            "胜利！全歼敌方。按 R 重新开始。".to_string()
        } else {
            "失败！队伍全灭。按 R 重新开始。".to_string()
        };
        next_game_state.set(GameState::Result);
    } else {
        next_phase.set(BattlePhase::RoundStart);
    }
}

/// 结果页热键：按 R 立即重置战斗。
pub fn restart_from_result_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        next_phase.set(BattlePhase::Init);
        next_game_state.set(GameState::Battle);
    }
}

/// 消费战斗事件并生成中文日志，供控制台与 UI 同步展示。
pub fn consume_battle_events_system(
    mut events: MessageReader<BattleEvent>,
    mut log: ResMut<BattleLog>,
    state: Res<State<GameState>>,
    result: Res<BattleResult>,
    mut last_printed_result: Local<Option<String>>,
) {
    for event in events.read() {
        let line = match event {
            BattleEvent::TurnStarted(turn) => format!("--- 第 {turn} 回合 ---"),
            BattleEvent::CardUsed { side, card_name } => {
                format!("{} 使用了卡牌：{}。", side_text(*side), card_name)
            }
            BattleEvent::CardDiscarded { side, card_name } => {
                format!("{} 弃置了卡牌：{}。", side_text(*side), card_name)
            }
            BattleEvent::ElementAuraApplied {
                side,
                from,
                to,
                effectiveness,
            } => {
                let from_str = from.map(element_text).unwrap_or("无");
                format!(
                    "{} 元素附着：{} -> {}（克制倍率 {}）。",
                    side_text(*side),
                    from_str,
                    element_text(*to),
                    effectiveness
                )
            }
            BattleEvent::SkillUsed {
                side, skill_name, ..
            } => {
                format!("{} 使用了 {}。", side_text(*side), skill_name)
            }
            BattleEvent::DamageDealt {
                source,
                target,
                amount,
            } => format!(
                "{} 对 {} 造成了 {} 点实际伤害。",
                side_text(*source),
                side_text(*target),
                amount
            ),
            BattleEvent::ShieldAbsorbed { side, amount } => {
                format!("{} 的护盾吸收了 {} 点伤害。", side_text(*side), amount)
            }
            BattleEvent::Healed { side, amount } => {
                format!("{} 恢复了 {} 点生命。", side_text(*side), amount)
            }
            BattleEvent::ShieldGained { side, amount } => {
                format!("{} 获得了 {} 点护盾。", side_text(*side), amount)
            }
            BattleEvent::CombatantFainted { side, name } => {
                format!("【{}】{} 倒下了。", side_text(*side), name)
            }
            BattleEvent::Switched { side, name } => {
                format!("{} 换上了 {}！", side_text(*side), name)
            }
        };

        push_battle_line(&mut log, line);
    }

    if *state.get() == GameState::Result && !result.message.is_empty() {
        let should_print = match last_printed_result.as_ref() {
            Some(last) => last != &result.message,
            None => true,
        };
        if should_print {
            println!("{}", result.message);
            *last_printed_result = Some(result.message.clone());
        }
    } else {
        *last_printed_result = None;
    }
}

/// 执行单个技能效果：攻击/治疗/护盾，支持元素克制。
fn apply_effect(
    effect: &SkillEffect,
    skill_element: Option<crate::data::ElementType>,
    attacker_side: Side,
    target_side: Side,
    target_element: crate::data::ElementType,
    attacker_stats: &mut Stats,
    attacker_shield: &mut Shield,
    target_stats: &mut Stats,
    target_shield: &mut Shield,
    target_aura: &mut ElementAura,
    pending_boosts: &mut PendingBoosts,
    event_writer: &mut MessageWriter<BattleEvent>,
) {
    match effect {
        SkillEffect::Attack { power } => {
            let mut effective_power = *power;
            // 待命增益只作用于“对应类型的精灵技能”，并在结算后清空。
            if attacker_side == Side::Player {
                effective_power += pending_boosts.player.next_attack_bonus;
                pending_boosts.player.next_attack_bonus = 0;
            } else {
                effective_power += pending_boosts.enemy.next_attack_bonus;
                pending_boosts.enemy.next_attack_bonus = 0;
            }

            let raw = effective_power + attacker_stats.atk - target_stats.def;

            if let Some(incoming_element) = skill_element {
                // 先用“附着元素（若存在）”计算克制倍率。
                let defender_elem_with_aura = target_aura.attached.unwrap_or(target_element);
                let effectiveness_with_aura = crate::data::ElementMatrix::get_effectiveness(
                    incoming_element,
                    defender_elem_with_aura,
                );
                let theoretical_damage_with_aura =
                    (raw as f32 * effectiveness_with_aura).max(1.0) as i32;
                let absorbed_with_aura = target_shield.0.min(theoretical_damage_with_aura);

                if absorbed_with_aura > 0 {
                    // 盾免疫：元素附着/反应不生效，改用“固有元素”重算伤害。
                    let effectiveness_no_aura = crate::data::ElementMatrix::get_effectiveness(
                        incoming_element,
                        target_element,
                    );
                    let theoretical_damage_no_aura =
                        (raw as f32 * effectiveness_no_aura).max(1.0) as i32;
                    let absorbed = target_shield.0.min(theoretical_damage_no_aura);

                    if absorbed > 0 {
                        target_shield.0 -= absorbed;
                        event_writer.write(BattleEvent::ShieldAbsorbed {
                            side: target_side,
                            amount: absorbed,
                        });
                    }

                    let hp_damage = (theoretical_damage_no_aura - absorbed).max(0);
                    if hp_damage > 0 {
                        target_stats.hp = (target_stats.hp - hp_damage).max(0);
                    }
                    event_writer.write(BattleEvent::DamageDealt {
                        source: attacker_side,
                        target: target_side,
                        amount: hp_damage,
                    });
                } else {
                    // 盾未吸收：允许附着/反应；并在成功元素攻击后设置 incoming 元素。
                    let hp_damage = theoretical_damage_with_aura;
                    if hp_damage > 0 {
                        target_stats.hp = (target_stats.hp - hp_damage).max(0);
                    }
                    event_writer.write(BattleEvent::DamageDealt {
                        source: attacker_side,
                        target: target_side,
                        amount: hp_damage,
                    });
                    let from = target_aura.attached;
                    target_aura.attached = Some(incoming_element);
                    event_writer.write(BattleEvent::ElementAuraApplied {
                        side: target_side,
                        from,
                        to: incoming_element,
                        effectiveness: effectiveness_with_aura,
                    });
                }
            } else {
                // 非元素攻击：不进行附着/反应处理。
                let theoretical_damage = (raw as f32).max(1.0) as i32;
                let absorbed = target_shield.0.min(theoretical_damage);
                if absorbed > 0 {
                    target_shield.0 -= absorbed;
                    event_writer.write(BattleEvent::ShieldAbsorbed {
                        side: target_side,
                        amount: absorbed,
                    });
                }

                let hp_damage = (theoretical_damage - absorbed).max(0);
                if hp_damage > 0 {
                    target_stats.hp = (target_stats.hp - hp_damage).max(0);
                }
                event_writer.write(BattleEvent::DamageDealt {
                    source: attacker_side,
                    target: target_side,
                    amount: hp_damage,
                });
            }
        }
        SkillEffect::Heal { amount } => {
            let mut heal_amount = *amount;
            if attacker_side == Side::Player {
                heal_amount += pending_boosts.player.next_heal_bonus;
                pending_boosts.player.next_heal_bonus = 0;
            } else {
                heal_amount += pending_boosts.enemy.next_heal_bonus;
                pending_boosts.enemy.next_heal_bonus = 0;
            }
            let before = attacker_stats.hp;
            attacker_stats.hp = (attacker_stats.hp + heal_amount).min(attacker_stats.max_hp);
            event_writer.write(BattleEvent::Healed {
                side: attacker_side,
                amount: attacker_stats.hp - before,
            });
        }
        SkillEffect::Shield { amount } => {
            let mut shield_amount = *amount;
            if attacker_side == Side::Player {
                shield_amount += pending_boosts.player.next_shield_bonus;
                pending_boosts.player.next_shield_bonus = 0;
            } else {
                shield_amount += pending_boosts.enemy.next_shield_bonus;
                pending_boosts.enemy.next_shield_bonus = 0;
            }

            attacker_shield.0 += shield_amount;
            event_writer.write(BattleEvent::ShieldGained {
                side: attacker_side,
                amount: shield_amount,
            });
        }
    }
}

/// 阵营文案转换，用于日志与 UI 展示。
fn side_text(side: Side) -> &'static str {
    match side {
        Side::Player => "玩家",
        Side::Enemy => "敌方",
    }
}

fn element_text(element: crate::data::ElementType) -> &'static str {
    match element {
        crate::data::ElementType::Fire => "火",
        crate::data::ElementType::Water => "水",
        crate::data::ElementType::Grass => "草",
        crate::data::ElementType::Light => "光",
        crate::data::ElementType::Dark => "暗",
        crate::data::ElementType::Thunder => "雷",
        crate::data::ElementType::Wind => "风",
    }
}

fn action_priority(action: &TurnAction, skill_db: &SkillDb) -> i32 {
    match action {
        TurnAction::Switch => 300,
        TurnAction::Skill(skill_id) => {
            let Some(skill) = skill_db.0.get(skill_id) else {
                return 0;
            };
            match skill.effect {
                SkillEffect::Shield { .. } | SkillEffect::Heal { .. } => 200,
                SkillEffect::Attack { .. } => 100,
            }
        }
    }
}

fn abort_battle(
    reason: &str,
    battle_log: &mut BattleLog,
    battle_result: &mut BattleResult,
    next_game_state: &mut NextState<GameState>,
) {
    let message = format!("战斗中断：{reason} 按 R 重新开始。");
    battle_result.message = message.clone();
    push_battle_line(battle_log, message);
    next_game_state.set(GameState::Result);
}
