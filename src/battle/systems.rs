use bevy::prelude::*;

use crate::{
    data::{SkillDb, SkillEffect, TeamSetup},
    game_state::{BattlePhase, GameState},
};

use super::{BattleEvent, BattleLog, BattleResult, Combatant, InBattle, Shield, Side, SkillList, Stats, TurnContext, TurnCount};

const LOG_LIMIT: usize = 10;

/// 初始化战斗：清理旧实体、生成双方单位并进入玩家指令阶段。
pub fn init_battle_system(
    mut commands: Commands,
    setup: Res<TeamSetup>,
    mut turn_ctx: ResMut<TurnContext>,
    mut battle_log: ResMut<BattleLog>,
    mut result: ResMut<BattleResult>,
    mut turn_count: ResMut<TurnCount>,
    cleanup_query: Query<Entity, With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    for entity in &cleanup_query {
        commands.entity(entity).despawn();
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
        let entity = commands.spawn((
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
        )).id();
        player_team.combatants.push(entity);
    }

    for combatant in &setup.enemy {
        let entity = commands.spawn((
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
        )).id();
        enemy_team.combatants.push(entity);
    }

    commands.insert_resource(crate::battle::PlayerTeam(player_team));
    commands.insert_resource(crate::battle::EnemyTeam(enemy_team));

    turn_ctx.player_action = None;
    turn_ctx.enemy_action = None;
    battle_log.0.clear();
    result.message = String::new();
    turn_count.0 = 0;  // 重置回合计数
    battle_log
        .0
        .push_back("战斗已开始，按 1-4 选择技能，按 Q 切换精灵。".to_string());
    next_phase.set(BattlePhase::PlayerCommand);
}

/// 处理玩家输入（键盘 1-4 / Space），记录本回合玩家技能。按 Q 切换精灵。
pub fn player_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut player_team: ResMut<crate::battle::PlayerTeam>,
    query: Query<(&Combatant, &SkillList, &Stats), With<InBattle>>,
) {
    if keyboard.just_pressed(KeyCode::KeyQ) {
        // 尝试切换到下一个活着的成员
        let current = player_team.0.active_index;
        for offset in 1..player_team.0.combatants.len() {
            let next_idx = (current + offset) % player_team.0.combatants.len();
            let entity = player_team.0.combatants[next_idx];
            if let Ok((_, _, stats)) = query.get(entity) {
                if stats.hp > 0 {
                    player_team.0.active_index = next_idx;
                    // 记录换人行动
                    turn_ctx.player_action = Some(crate::battle::TurnAction::Switch);
                    next_phase.set(BattlePhase::EnemyCommand);
                    return;
                }
            }
        }
    }

    let active_entity = player_team.0.active_combatant().unwrap();
    let Ok((_, skill_list, _)) = query.get(active_entity) else {
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
        turn_ctx.player_action = Some(crate::battle::TurnAction::Skill(skill));
        next_phase.set(BattlePhase::EnemyCommand);
    }
}

/// 敌方 AI 选招：只选择攻击技能，优先高伤害，考虑克制关系。
pub fn enemy_choose_skill_system(
    mut turn_ctx: ResMut<TurnContext>,
    skill_db: Res<SkillDb>,
    player_team: Res<crate::battle::PlayerTeam>,
    enemy_team: Res<crate::battle::EnemyTeam>,
    query: Query<(&Combatant, &Stats, &SkillList), With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    let p_entity = player_team.0.active_combatant().unwrap();
    let e_entity = enemy_team.0.active_combatant().unwrap();

    let Ok((player_combatant, player_stats, _)) = query.get(p_entity) else { return; };
    let Ok((_, enemy_stats, enemy_skills)) = query.get(e_entity) else { return; };

    // 收集所有攻击技能，按预估伤害排序
    let mut attack_skills = Vec::new();

    for skill_id in enemy_skills.0 {
        let Some(skill) = skill_db.0.get(&skill_id) else {
            continue;
        };
        
        // 只考虑攻击技能
        if let SkillEffect::Attack { power } = skill.effect {
            // 计算基础伤害
            let base_damage = power + enemy_stats.atk - player_stats.def;
            
            // 计算克制倍数
            let effectiveness = if let Some(skill_element) = skill.element {
                crate::data::ElementMatrix::get_effectiveness(skill_element, player_combatant.element)
            } else {
                1.0
            };
            
            // 最终伤害 = 基础伤害 × 克制倍数，最小 1
            let final_damage = (base_damage as f32 * effectiveness).max(1.0) as i32;
            
            attack_skills.push((skill_id, final_damage));
        }
    }

    // 如果没有攻击技能，使用第一个技能（不应该发生，但保险起见）
    let selected = if attack_skills.is_empty() {
        enemy_skills.0[0]
    } else {
        // 选择伤害最高的技能
        attack_skills
            .into_iter()
            .max_by_key(|(_, damage)| *damage)
            .map(|(skill_id, _)| skill_id)
            .unwrap_or(enemy_skills.0[0])
    };

    turn_ctx.enemy_action = Some(crate::battle::TurnAction::Skill(selected));
    next_phase.set(BattlePhase::Resolve);
}

/// 回合结算：按速度排序后依次执行双方技能。
pub fn resolve_turn_system(
    mut query: Query<(Entity, &Combatant, &mut Stats, &SkillList, &mut Shield, &Name), With<InBattle>>,
    mut turn_ctx: ResMut<TurnContext>,
    skill_db: Res<SkillDb>,
    player_team: Res<crate::battle::PlayerTeam>,
    enemy_team: Res<crate::battle::EnemyTeam>,
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

    let player_entity = player_team.0.active_combatant().unwrap();
    let enemy_entity = enemy_team.0.active_combatant().unwrap();

    let mut player_spd = 0;
    let mut enemy_spd = 0;
    if let Ok((_, _, stats, _, _, _)) = query.get(player_entity) { player_spd = stats.spd; }
    if let Ok((_, _, stats, _, _, _)) = query.get(enemy_entity) { enemy_spd = stats.spd; }

    turn_count.0 += 1;
    event_writer.write(BattleEvent::TurnStarted(turn_count.0));

    let order = if player_spd >= enemy_spd {
        [(Side::Player, player_action), (Side::Enemy, enemy_action)]
    } else {
        [(Side::Enemy, enemy_action), (Side::Player, player_action)]
    };

    for (side, action) in order {
        let active_side_entity = if side == Side::Player {
            player_entity
        } else {
            enemy_entity
        };

        if let crate::battle::TurnAction::Switch = action {
            if let Ok((_, _, _, _, _, name)) = query.get(active_side_entity) {
                event_writer.write(BattleEvent::Switched { side, name: name.to_string() });
            }
            continue;
        }

        let skill_id = match action {
            crate::battle::TurnAction::Skill(id) => id,
            _ => unreachable!(),
        };

        let Ok([(a_entity, a_combatant, mut a_stats, _, mut a_shield, a_name), (t_entity, t_combatant, mut t_stats, _, mut t_shield, _)]) =
            query.get_many_mut([player_entity, enemy_entity])
        else {
            return;
        };

        let (attacker_side, attacker_skill, attacker_name, target_side) = if a_combatant.side == side {
            (a_combatant.side, skill_id, a_name.to_string(), t_combatant.side)
        } else {
            (t_combatant.side, skill_id, String::from("Enemy"), a_combatant.side)
        };

        let attacker_dead = if a_combatant.side == side { a_stats.hp <= 0 } else { t_stats.hp <= 0 };
        if attacker_dead {
            continue;
        }

        let Some(skill) = skill_db.0.get(&attacker_skill) else {
            continue;
        };
        event_writer.write(BattleEvent::SkillUsed {
            side: attacker_side,
            skill_name: skill.name.clone(),
        });

        if a_combatant.side == side {
            apply_effect(
                &skill.effect,
                skill.element,
                attacker_side,
                a_combatant.element,
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
                t_combatant.element,
                target_side,
                a_combatant.element,
                &mut t_stats,
                &mut t_shield,
                &mut a_stats,
                &mut a_shield,
                &mut event_writer,
            );
        }

        let _ = (a_entity, t_entity, attacker_name);
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
    // 检查并自动切换
    let p_entity = player_team.0.active_combatant().unwrap();
    let mut p_dead = false;
    if let Ok((_, stats, name)) = query.get(p_entity) {
        if stats.hp <= 0 {
            p_dead = true;
            event_writer.write(BattleEvent::CombatantFainted { side: Side::Player, name: name.to_string() });
            
            // 找下一个存活的
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
                p_dead = false; // 换上了新的
                let new_e = player_team.0.combatants[idx];
                if let Ok((_, _, new_n)) = query.get(new_e) {
                    battle_log.0.push_back(format!("玩家换上了 {}！", new_n));
                }
            }
        }
    }

    let e_entity = enemy_team.0.active_combatant().unwrap();
    let mut e_dead = false;
    if let Ok((_, stats, name)) = query.get(e_entity) {
        if stats.hp <= 0 {
            e_dead = true;
            event_writer.write(BattleEvent::CombatantFainted { side: Side::Enemy, name: name.to_string() });
            
            // 找下一个存活的
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
                e_dead = false; // 换上了新的
                let new_e = enemy_team.0.combatants[idx];
                if let Ok((_, _, new_n)) = query.get(new_e) {
                    battle_log.0.push_back(format!("敌方换上了 {}！", new_n));
                }
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
) {
    for event in events.read() {
        let line = match event {
            BattleEvent::TurnStarted(turn) => format!("--- 第 {turn} 回合 ---"),
            BattleEvent::SkillUsed { side, skill_name } => {
                format!("{} 使用了 {}。", side_text(*side), skill_name)
            }
            BattleEvent::DamageDealt {
                source,
                target,
                amount,
            } => format!("{} 对 {} 造成了 {} 点伤害。", side_text(*source), side_text(*target), amount),
            BattleEvent::Healed { side, amount } => format!("{} 恢复了 {} 点生命。", side_text(*side), amount),
            BattleEvent::ShieldGained { side, amount } => {
                format!("{} 获得了 {} 点护盾。", side_text(*side), amount)
            }
            BattleEvent::CombatantFainted { side, name } => format!("【{}】{} 倒下了。", side_text(*side), name),
            BattleEvent::Switched { side, name } => format!("{} 换上了 {}！", side_text(*side), name),
        };

        println!("{line}");
        log.0.push_back(line);
        while log.0.len() > LOG_LIMIT {
            log.0.pop_front();
        }
    }

    if *state.get() == GameState::Result && !result.message.is_empty() {
        println!("{}", result.message);
    }
}

/// 执行单个技能效果：攻击/治疗/护盾，支持元素克制。
fn apply_effect(
    effect: &SkillEffect,
    skill_element: Option<crate::data::ElementType>,
    attacker_side: Side,
    _attacker_element: crate::data::ElementType,
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
            
            // 计算元素克制倍数
            let effectiveness = if let Some(element) = skill_element {
                crate::data::ElementMatrix::get_effectiveness(element, target_element)
            } else {
                1.0
            };
            
            // 最终伤害 = 基础伤害 × 克制倍数，最小 1
            let mut damage = (raw as f32 * effectiveness).max(1.0) as i32;
            
            let absorbed = target_shield.0.min(damage);
            if absorbed > 0 {
                target_shield.0 -= absorbed;
                damage -= absorbed;
            }
            if damage > 0 {
                target_stats.hp -= damage;
            }
            event_writer.write(BattleEvent::DamageDealt {
                source: attacker_side,
                target: target_side,
                amount: (raw as f32 * effectiveness).max(1.0) as i32,
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
