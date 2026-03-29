use bevy::prelude::*;

use crate::{
    data::{SkillDb, SkillEffect, SkillId, TeamSetup},
    game_state::{BattlePhase, GameState},
};

use super::{BattleEvent, BattleLog, BattleResult, Combatant, InBattle, Shield, Side, SkillList, Stats, TurnContext};

const LOG_LIMIT: usize = 10;

/// 初始化战斗：清理旧实体、生成双方单位并进入玩家指令阶段。
pub fn init_battle_system(
    mut commands: Commands,
    setup: Res<TeamSetup>,
    mut turn_ctx: ResMut<TurnContext>,
    mut battle_log: ResMut<BattleLog>,
    mut result: ResMut<BattleResult>,
    cleanup_query: Query<Entity, With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    for entity in &cleanup_query {
        commands.entity(entity).despawn();
    }

    let player_stats = &setup.player.stats;
    let enemy_stats = &setup.enemy.stats;

    commands.spawn((
        Name::new(setup.player.name.clone()),
        InBattle,
        Combatant { side: Side::Player },
        Stats {
            hp: player_stats.hp,
            max_hp: player_stats.hp,
            atk: player_stats.atk,
            def: player_stats.def,
            spd: player_stats.spd,
        },
        SkillList(setup.player.skills),
        Shield::default(),
    ));

    commands.spawn((
        Name::new(setup.enemy.name.clone()),
        InBattle,
        Combatant { side: Side::Enemy },
        Stats {
            hp: enemy_stats.hp,
            max_hp: enemy_stats.hp,
            atk: enemy_stats.atk,
            def: enemy_stats.def,
            spd: enemy_stats.spd,
        },
        SkillList(setup.enemy.skills),
        Shield::default(),
    ));

    turn_ctx.player_skill = None;
    turn_ctx.enemy_skill = None;
    battle_log.0.clear();
    result.message = String::new();
    battle_log
        .0
        .push_back("战斗已开始，按 1-4 选择技能。".to_string());
    next_phase.set(BattlePhase::PlayerCommand);
}

/// 处理玩家输入（键盘 1-4 / Space），记录本回合玩家技能。
pub fn player_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    query: Query<(&Combatant, &SkillList), With<InBattle>>,
) {
    let mut player_skills = None;
    for (combatant, skill_list) in &query {
        if combatant.side == Side::Player {
            player_skills = Some(skill_list.0);
            break;
        }
    }

    let Some(skills) = player_skills else {
        return;
    };

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
        turn_ctx.player_skill = Some(skill);
        next_phase.set(BattlePhase::EnemyCommand);
    }
}

/// 敌方 AI 选招：低血优先治疗，其次护盾，否则选预估伤害最高技能。
pub fn enemy_choose_skill_system(
    mut turn_ctx: ResMut<TurnContext>,
    skill_db: Res<SkillDb>,
    query: Query<(&Combatant, &Stats, &SkillList), With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    let mut player = None;
    let mut enemy = None;

    for (combatant, stats, skills) in &query {
        match combatant.side {
            Side::Player => player = Some((*stats, *skills)),
            Side::Enemy => enemy = Some((*stats, *skills)),
        }
    }

    let (player_stats, _) = match player {
        Some(v) => v,
        None => return,
    };
    let (enemy_stats, enemy_skills) = match enemy {
        Some(v) => v,
        None => return,
    };

    let mut selected = enemy_skills.0[0];
    let hp_ratio = enemy_stats.hp as f32 / enemy_stats.max_hp as f32;

    if hp_ratio < 0.25 && has_heal(enemy_skills.0, &skill_db) {
        selected = SkillId::FirstAid;
    } else if hp_ratio < 0.45 && has_shield(enemy_skills.0, &skill_db) && player_stats.atk > enemy_stats.def {
        selected = SkillId::Guard;
    } else {
        let mut best_damage = i32::MIN;
        for skill_id in enemy_skills.0 {
            let Some(skill) = skill_db.0.get(&skill_id) else {
                continue;
            };
            if let SkillEffect::Attack { power } = skill.effect {
                let expected = (power + enemy_stats.atk - player_stats.def).max(1);
                if expected > best_damage {
                    best_damage = expected;
                    selected = skill_id;
                }
            }
        }
    }

    turn_ctx.enemy_skill = Some(selected);
    next_phase.set(BattlePhase::Resolve);
}

/// 回合结算：按速度排序后依次执行双方技能。
pub fn resolve_turn_system(
    mut query: Query<(Entity, &Combatant, &mut Stats, &SkillList, &mut Shield, &Name), With<InBattle>>,
    mut turn_ctx: ResMut<TurnContext>,
    skill_db: Res<SkillDb>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut turn_count: Local<u32>,
) {
    let Some(player_skill) = turn_ctx.player_skill else {
        return;
    };
    let Some(enemy_skill) = turn_ctx.enemy_skill else {
        return;
    };

    let mut player_entity = None;
    let mut enemy_entity = None;
    let mut player_spd = 0;
    let mut enemy_spd = 0;
    for (entity, combatant, stats, _, _, _) in &mut query {
        match combatant.side {
            Side::Player => {
                player_entity = Some(entity);
                player_spd = stats.spd;
            }
            Side::Enemy => {
                enemy_entity = Some(entity);
                enemy_spd = stats.spd;
            }
        }
    }

    let (Some(player_entity), Some(enemy_entity)) = (player_entity, enemy_entity) else {
        return;
    };

    *turn_count += 1;
    event_writer.write(BattleEvent::TurnStarted(*turn_count));

    let order = if player_spd >= enemy_spd {
        [(Side::Player, player_skill), (Side::Enemy, enemy_skill)]
    } else {
        [(Side::Enemy, enemy_skill), (Side::Player, player_skill)]
    };

    for (side, skill_id) in order {
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
                attacker_side,
                target_side,
                &mut a_stats,
                &mut a_shield,
                &mut t_stats,
                &mut t_shield,
                &mut event_writer,
            );
        } else {
            apply_effect(
                &skill.effect,
                attacker_side,
                target_side,
                &mut t_stats,
                &mut t_shield,
                &mut a_stats,
                &mut a_shield,
                &mut event_writer,
            );
        }

        let _ = (a_entity, t_entity, attacker_name);
    }

    turn_ctx.player_skill = None;
    turn_ctx.enemy_skill = None;
    next_phase.set(BattlePhase::CheckEnd);
}

/// 判定胜负并切换到结果页，未结束则回到玩家指令阶段。
pub fn check_end_system(
    query: Query<(&Combatant, &Stats), With<InBattle>>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut battle_result: ResMut<BattleResult>,
) {
    let mut player_hp = 0;
    let mut enemy_hp = 0;
    for (combatant, stats) in &query {
        match combatant.side {
            Side::Player => player_hp = stats.hp,
            Side::Enemy => enemy_hp = stats.hp,
        }
    }

    if player_hp <= 0 || enemy_hp <= 0 {
        if player_hp <= 0 {
            event_writer.write(BattleEvent::CombatantFainted { side: Side::Player });
        }
        if enemy_hp <= 0 {
            event_writer.write(BattleEvent::CombatantFainted { side: Side::Enemy });
        }
        battle_result.message = if player_hp <= 0 && enemy_hp <= 0 {
            "平局！按 R 重新开始。".to_string()
        } else if enemy_hp <= 0 {
            "胜利！按 R 重新开始。".to_string()
        } else {
            "失败！按 R 重新开始。".to_string()
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
            BattleEvent::CombatantFainted { side } => format!("{} 倒下了。", side_text(*side)),
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

/// 执行单个技能效果：攻击/治疗/护盾。
fn apply_effect(
    effect: &SkillEffect,
    attacker_side: Side,
    target_side: Side,
    attacker_stats: &mut Stats,
    attacker_shield: &mut Shield,
    target_stats: &mut Stats,
    target_shield: &mut Shield,
    event_writer: &mut MessageWriter<BattleEvent>,
) {
    match effect {
        SkillEffect::Attack { power } => {
            let raw = power + attacker_stats.atk - target_stats.def;
            let mut damage = raw.max(1);
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
                amount: raw.max(1),
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

/// 是否拥有治疗技能。
fn has_heal(skill_ids: [SkillId; 4], skill_db: &SkillDb) -> bool {
    skill_ids.into_iter().any(|id| {
        matches!(
            skill_db.0.get(&id).map(|s| &s.effect),
            Some(SkillEffect::Heal { .. })
        )
    })
}

/// 是否拥有护盾技能。
fn has_shield(skill_ids: [SkillId; 4], skill_db: &SkillDb) -> bool {
    skill_ids.into_iter().any(|id| {
        matches!(
            skill_db.0.get(&id).map(|s| &s.effect),
            Some(SkillEffect::Shield { .. })
        )
    })
}

/// 阵营文案转换，用于日志与 UI 展示。
fn side_text(side: Side) -> &'static str {
    match side {
        Side::Player => "玩家",
        Side::Enemy => "敌方",
    }
}
