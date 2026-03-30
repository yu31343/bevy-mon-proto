use bevy::prelude::*;

use crate::{
    data::{BattleDataStatus, SkillDb, SkillEffect, TeamSetup},
    game_state::{BattlePhase, GameState},
};

use super::{
    push_battle_line, BattleEvent, BattleLog, BattleResult, Combatant, InBattle, Shield, Side, SkillList, Stats,
    TurnAction, TurnContext, TurnCount,
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
    battle_log.0.clear();
    result.message.clear();
    turn_count.0 = 0;

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
        "战斗已开始，按 1-4 选择技能，按 Q 切换精灵。",
    );
    next_phase.set(BattlePhase::PlayerCommand);
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
                    next_phase.set(BattlePhase::EnemyCommand);
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
        next_phase.set(BattlePhase::EnemyCommand);
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
    let under_counter = crate::data::ElementMatrix::get_effectiveness(player_combatant.element, enemy_combatant.element) > 1.0;

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
                    crate::data::ElementMatrix::get_effectiveness(skill_element, player_combatant.element)
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
    next_phase.set(BattlePhase::Resolve);
}

/// 回合结算：按行动优先级排序（换人 > 防御/治疗 > 攻击），同优先级按速度。
pub fn resolve_turn_system(
    mut query: Query<(Entity, &Combatant, &mut Stats, &SkillList, &mut Shield, &Name), With<InBattle>>,
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

    let Ok((_, _, p_stats, _, _, _)) = query.get(player_entity) else {
        abort_battle(
            "回合结算失败：玩家成员数据缺失。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };
    let Ok((_, _, e_stats, _, _, _)) = query.get(enemy_entity) else {
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
            if let Ok((_, _, _, _, _, name)) = query.get(active_side_entity) {
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
            .map(|(_, _, _, sl, _, _)| sl.0.iter().position(|&s| s == skill_id).unwrap_or(0))
            .unwrap_or(0);

        let Ok([
            (_, a_combatant, mut a_stats, _, mut a_shield, _),
            (_, t_combatant, mut t_stats, _, mut t_shield, _),
        ]) = query.get_many_mut([player_entity, enemy_entity])
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
        next_phase.set(BattlePhase::PlayerCommand);
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
            BattleEvent::SkillUsed { side, skill_name, .. } => {
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
            BattleEvent::Healed { side, amount } => format!("{} 恢复了 {} 点生命。", side_text(*side), amount),
            BattleEvent::ShieldGained { side, amount } => {
                format!("{} 获得了 {} 点护盾。", side_text(*side), amount)
            }
            BattleEvent::CombatantFainted { side, name } => format!("【{}】{} 倒下了。", side_text(*side), name),
            BattleEvent::Switched { side, name } => format!("{} 换上了 {}！", side_text(*side), name),
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
    event_writer: &mut MessageWriter<BattleEvent>,
) {
    match effect {
        SkillEffect::Attack { power } => {
            let raw = power + attacker_stats.atk - target_stats.def;
            let effectiveness = if let Some(element) = skill_element {
                crate::data::ElementMatrix::get_effectiveness(element, target_element)
            } else {
                1.0
            };

            let theoretical_damage = (raw as f32 * effectiveness).max(1.0) as i32;
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
        SkillEffect::Heal { amount } => {
            let before = attacker_stats.hp;
            attacker_stats.hp = (attacker_stats.hp + amount).min(attacker_stats.max_hp);
            event_writer.write(BattleEvent::Healed {
                side: attacker_side,
                amount: attacker_stats.hp - before,
            });
        }
        SkillEffect::Shield { amount } => {
            attacker_shield.0 += amount;
            event_writer.write(BattleEvent::ShieldGained {
                side: attacker_side,
                amount: *amount,
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
