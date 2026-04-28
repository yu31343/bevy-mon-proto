use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    battle::{
        AccuracyRng, ActionPoints, ActionTrace, BattleLog, BattleResult, Combatant, ElementAura,
        Hand, InBattle, PendingBoosts, ReplayEventLog, RoundOrder, SelectedCards, Shield, Side,
        SkillCount, SkillList, Stats, StatusBoard, StructuredBattleLog, TurnContext, TurnCount,
        UiControlSide, clear_runtime_battle_logs, clear_turn_context, note_structured_phase,
        push_battle_line,
    },
    data::{BattleDataStatus, BattleDbs, BattleRules, MonsterPool, TeamSelections},
    game_state::{BattlePhase, GameState},
};

fn normalize_skill_slots(skills: &[crate::data::SkillId]) -> ([crate::data::SkillId; 4], usize) {
    let count = skills.len().clamp(1, 4);
    let fallback = skills[0];
    let mut slots = [fallback; 4];
    for (i, sid) in skills.iter().take(4).enumerate() {
        slots[i] = *sid;
    }
    (slots, count)
}

#[derive(SystemParam)]
pub(crate) struct InitBattleRuntime<'w> {
    dbs: Res<'w, BattleDbs>,
    battle_rules: Res<'w, BattleRules>,
    data_status: Option<Res<'w, BattleDataStatus>>,
    turn_ctx: ResMut<'w, TurnContext>,
    battle_log: ResMut<'w, BattleLog>,
    structured_log: ResMut<'w, StructuredBattleLog>,
    replay_log: ResMut<'w, ReplayEventLog>,
    action_trace: ResMut<'w, ActionTrace>,
    result: ResMut<'w, BattleResult>,
    turn_count: ResMut<'w, TurnCount>,
    round_order: ResMut<'w, RoundOrder>,
    accuracy_rng: ResMut<'w, AccuracyRng>,
    next_game_state: ResMut<'w, NextState<GameState>>,
}

pub fn init_battle_system(
    mut commands: Commands,
    monster_pool: Res<MonsterPool>,
    team_selections: Option<Res<TeamSelections>>,
    cleanup_query: Query<Entity, With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut runtime: InitBattleRuntime,
) {
    let dbs = &runtime.dbs;
    let battle_rules = &runtime.battle_rules;
    let data_status = &runtime.data_status;
    let turn_ctx = &mut runtime.turn_ctx;
    let battle_log = &mut runtime.battle_log;
    let structured_log = &mut runtime.structured_log;
    let replay_log = &mut runtime.replay_log;
    let action_trace = &mut runtime.action_trace;
    let result = &mut runtime.result;
    let turn_count = &mut runtime.turn_count;
    let round_order = &mut runtime.round_order;
    let accuracy_rng = &mut runtime.accuracy_rng;
    let next_game_state = &mut runtime.next_game_state;

    // Wait for team selections to be made
    let Some(team_selections) = team_selections else {
        println!("等待队伍选择...");
        return;
    };
    for entity in &cleanup_query {
        commands.entity(entity).despawn();
    }

    clear_turn_context(turn_ctx);
    clear_runtime_battle_logs(battle_log, structured_log, replay_log, action_trace);
    result.message.clear();
    result.export_status = None;
    turn_count.0 = 0;
    **round_order = RoundOrder::default();
    accuracy_rng.reset(0xA5A5_1F2D_D3C4_B7E9);
    note_structured_phase(
        structured_log,
        "battle-init",
        "初始化战斗",
        "已重置回合上下文、文本日志、结构化日志与行动追踪。",
    );

    // 回合进度状态初始化（在每次“战斗重开”时重置）。
    commands.insert_resource(ActionPoints {
        player: 0,
        enemy: 0,
    });
    commands.insert_resource(Hand::default());
    commands.insert_resource(PendingBoosts::default());
    commands.insert_resource(SelectedCards::default());
    commands.insert_resource(UiControlSide(Side::Player));

    if let Some(status) = data_status {
        if let Some(reason) = &status.error {
            abort_battle(
                &format!("战斗初始化失败：{reason}"),
                battle_log,
                result,
                next_game_state,
            );
            return;
        }
    }

    let player_count = team_selections.player_indices.len();
    let enemy_count = team_selections.enemy_indices.len();
    if !(1..=battle_rules.max_team_size).contains(&player_count) {
        abort_battle(
            &format!(
                "战斗初始化失败：玩家队伍人数非法（需 1..={}，当前 {}）。",
                battle_rules.max_team_size, player_count
            ),
            battle_log,
            result,
            next_game_state,
        );
        return;
    }
    if !(1..=battle_rules.max_team_size).contains(&enemy_count) {
        abort_battle(
            &format!(
                "战斗初始化失败：敌方队伍人数非法（需 1..={}，当前 {}）。",
                battle_rules.max_team_size, enemy_count
            ),
            battle_log,
            result,
            next_game_state,
        );
        return;
    }

    if dbs.skills.is_empty() {
        abort_battle(
            "战斗初始化失败：技能数据库为空。",
            battle_log,
            result,
            next_game_state,
        );
        return;
    }

    // Validate indices and skills
    for &idx in team_selections
        .player_indices
        .iter()
        .chain(&team_selections.enemy_indices)
    {
        if idx >= monster_pool.monsters.len() {
            abort_battle(
                &format!("战斗初始化失败：无效的精灵索引 {}。", idx),
                battle_log,
                result,
                next_game_state,
            );
            return;
        }

        let mon = &monster_pool.monsters[idx];
        if mon.skills.is_empty() || mon.skills.len() > 4 {
            abort_battle(
                &format!(
                    "战斗初始化失败：{} 技能数量非法（需 1..=4，当前 {}）。",
                    mon.name,
                    mon.skills.len()
                ),
                battle_log,
                result,
                next_game_state,
            );
            return;
        }

        for &sid in &mon.skills {
            if !dbs.skills.contains_key(&sid) {
                abort_battle(
                    &format!("战斗初始化失败：{} 存在未定义技能 {:?}。", mon.name, sid),
                    battle_log,
                    result,
                    next_game_state,
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

    for &idx in &team_selections.player_indices {
        let combatant = &monster_pool.monsters[idx];
        let (skill_slots, skill_count) = normalize_skill_slots(&combatant.skills);
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
                    acc: combatant.stats.acc,
                    atk_stage: 0,
                    def_stage: 0,
                    spd_stage: 0,
                    acc_stage: 0,
                },
                SkillList(skill_slots),
                SkillCount(skill_count),
                Shield::default(),
                StatusBoard::default(),
                ElementAura::default(),
            ))
            .id();
        player_team.combatants.push(entity);
    }

    for &idx in &team_selections.enemy_indices {
        let combatant = &monster_pool.monsters[idx];
        let (skill_slots, skill_count) = normalize_skill_slots(&combatant.skills);
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
                    acc: combatant.stats.acc,
                    atk_stage: 0,
                    def_stage: 0,
                    spd_stage: 0,
                    acc_stage: 0,
                },
                SkillList(skill_slots),
                SkillCount(skill_count),
                Shield::default(),
                StatusBoard::default(),
                ElementAura::default(),
            ))
            .id();
        enemy_team.combatants.push(entity);
    }

    if player_team.combatants.is_empty() || enemy_team.combatants.is_empty() {
        abort_battle(
            "战斗初始化失败：生成战斗实体后队伍为空。",
            battle_log,
            result,
            next_game_state,
        );
        return;
    }

    commands.insert_resource(crate::battle::PlayerTeam(player_team));
    commands.insert_resource(crate::battle::EnemyTeam(enemy_team));

    push_battle_line(
        battle_log,
        "战斗已开始：每回合开始抽取手牌并获得行动点；按 1-4 使用精灵技能，按 F 弃牌换 AP，按 E 结束回合。",
    );
    next_phase.set(BattlePhase::RoundStart);
}

pub(crate) fn abort_battle(
    reason: &str,
    battle_log: &mut BattleLog,
    battle_result: &mut BattleResult,
    next_game_state: &mut NextState<GameState>,
) {
    let message = format!("战斗中断：{reason} 按 R 重新开始。");
    battle_result.message = message.clone();
    battle_result.export_status = None;
    push_battle_line(battle_log, message);
    next_game_state.set(GameState::Result);
}
