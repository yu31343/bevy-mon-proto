use bevy::prelude::*;

use crate::{
    battle::{
        push_battle_line, ActionPoints, BattleLog, BattleResult, Combatant, ElementAura, Hand,
        InBattle, PendingBoosts, SelectedCard, Shield, Side, SkillList, Stats, TurnContext,
        TurnCount,
    },
    data::{BattleDataStatus, BattleDbs, MonsterPool, TeamSelections},
    game_state::{BattlePhase, GameState},
};

pub fn init_battle_system(
    mut commands: Commands,
    monster_pool: Res<MonsterPool>,
    team_selections: Option<Res<TeamSelections>>,
    dbs: Res<BattleDbs>,
    data_status: Option<Res<BattleDataStatus>>,
    mut turn_ctx: ResMut<TurnContext>,
    mut battle_log: ResMut<BattleLog>,
    mut result: ResMut<BattleResult>,
    mut turn_count: ResMut<TurnCount>,
    cleanup_query: Query<Entity, With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    // Wait for team selections to be made
    let Some(team_selections) = team_selections else {
        println!("等待队伍选择...");
        return;
    };
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
    commands.insert_resource(SelectedCard::default());

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

    if team_selections.player_indices.len() != 3 || team_selections.enemy_indices.len() != 3 {
        abort_battle(
            "战斗初始化失败：队伍选择不完整（需要各选择 3 个精灵）。",
            &mut battle_log,
            &mut result,
            &mut next_game_state,
        );
        return;
    }

    if dbs.skills.is_empty() {
        abort_battle(
            "战斗初始化失败：技能数据库为空。",
            &mut battle_log,
            &mut result,
            &mut next_game_state,
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
                &mut battle_log,
                &mut result,
                &mut next_game_state,
            );
            return;
        }

        let mon = &monster_pool.monsters[idx];
        for sid in mon.skills {
            if !dbs.skills.contains_key(&sid) {
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

    for &idx in &team_selections.player_indices {
        let combatant = &monster_pool.monsters[idx];
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

    for &idx in &team_selections.enemy_indices {
        let combatant = &monster_pool.monsters[idx];
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

pub(crate) fn abort_battle(
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
