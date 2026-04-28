use std::{
    fs,
    path::{Path, PathBuf},
};

use bevy::prelude::*;

use crate::{
    battle::{
        ActionTrace, BattleEvent, BattleLog, BattleResult, Combatant, ElementAura, InBattle,
        PendingKoResolution, ReplayEventLog, Shield, Side, Stats, StructuredBattleLog, TurnCount,
        note_action_phase, note_structured_phase, push_battle_line, push_named_action_trace,
        transfer_status_by_id,
    },
    game_state::{BattlePhase, GameState},
};

use super::{abort_battle, process_round_end_status_durations};

pub fn check_end_system(
    mut queries: ParamSet<(
        Query<(&Combatant, &Stats, &Name), With<InBattle>>,
        Query<
            (
                &mut Stats,
                &mut Shield,
                &mut crate::battle::StatusBoard,
                &mut ElementAura,
            ),
            With<InBattle>,
        >,
    )>,
    player_team: ResMut<crate::battle::PlayerTeam>,
    enemy_team: ResMut<crate::battle::EnemyTeam>,
    mut pending_ko: ResMut<PendingKoResolution>,
    turn_count: Res<TurnCount>,
    formula_rules: Res<crate::data::BattleFormulaRules>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut formula_writer: MessageWriter<crate::battle::BattleFormulaEvent>,
    mut status_writer: MessageWriter<crate::battle::BattleStatusEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut battle_result: ResMut<BattleResult>,
    mut battle_log: ResMut<BattleLog>,
    mut structured_log: ResMut<StructuredBattleLog>,
    mut action_trace: ResMut<ActionTrace>,
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

    let Some(e_entity) = enemy_team.0.active_combatant() else {
        abort_battle(
            "结算失败：敌方上场成员无效。",
            &mut battle_log,
            &mut battle_result,
            &mut next_game_state,
        );
        return;
    };

    let (p_hp, p_name, player_next_idx, e_hp, e_name, enemy_next_idx) = {
        let query = queries.p0();

        let Ok((_, p_stats, p_name)) = query.get(p_entity) else {
            abort_battle(
                "结算失败：玩家成员数据缺失。",
                &mut battle_log,
                &mut battle_result,
                &mut next_game_state,
            );
            return;
        };
        let Ok((_, e_stats, e_name)) = query.get(e_entity) else {
            abort_battle(
                "结算失败：敌方成员数据缺失。",
                &mut battle_log,
                &mut battle_result,
                &mut next_game_state,
            );
            return;
        };

        let mut player_next_idx = None;
        if p_stats.hp <= 0 {
            for (i, &entity) in player_team.0.combatants.iter().enumerate() {
                if let Ok((_, stats, _)) = query.get(entity) {
                    if stats.hp > 0 {
                        player_next_idx = Some(i);
                        break;
                    }
                }
            }
        }

        let mut enemy_next_idx = None;
        if e_stats.hp <= 0 {
            for (i, &entity) in enemy_team.0.combatants.iter().enumerate() {
                if let Ok((_, stats, _)) = query.get(entity) {
                    if stats.hp > 0 {
                        enemy_next_idx = Some(i);
                        break;
                    }
                }
            }
        }

        (
            p_stats.hp,
            p_name.to_string(),
            player_next_idx,
            e_stats.hp,
            e_name.to_string(),
            enemy_next_idx,
        )
    };

    let mut p_dead = false;
    if p_hp <= 0 {
        event_writer.write(BattleEvent::CombatantFainted {
            owner: p_entity,
            side: Side::Player,
            name: p_name.clone(),
        });
        note_action_phase(
            &mut structured_log,
            turn_count.0,
            Side::Player,
            "玩家倒下",
            format!("{} 倒下；当前HP={}", p_name, p_hp),
        );
        push_named_action_trace(
            &mut action_trace,
            turn_count.0,
            Side::Player,
            "fainted",
            format!("{} 倒下；当前HP={}", p_name, p_hp),
        );
        pending_ko.player_switch_index = player_next_idx;
        p_dead = player_next_idx.is_none();
    }

    let mut e_dead = false;
    if e_hp <= 0 {
        event_writer.write(BattleEvent::CombatantFainted {
            owner: e_entity,
            side: Side::Enemy,
            name: e_name.clone(),
        });
        note_action_phase(
            &mut structured_log,
            turn_count.0,
            Side::Enemy,
            "敌方倒下",
            format!("{} 倒下；当前HP={}", e_name, e_hp),
        );
        push_named_action_trace(
            &mut action_trace,
            turn_count.0,
            Side::Enemy,
            "fainted",
            format!("{} 倒下；当前HP={}", e_name, e_hp),
        );
        pending_ko.enemy_switch_index = enemy_next_idx;
        e_dead = enemy_next_idx.is_none();
    }

    if pending_ko.player_switch_index.is_some()
        || pending_ko.enemy_switch_index.is_some()
        || p_dead
        || e_dead
    {
        pending_ko.player_defeated = p_dead;
        pending_ko.enemy_defeated = e_dead;
        pending_ko.timer = Timer::from_seconds(1.0, TimerMode::Once);
        note_structured_phase(
            &mut structured_log,
            "death-resolve",
            "进入死亡结算",
            format!(
                "玩家待切换={:?}；敌方待切换={:?}；玩家全灭={}；敌方全灭={}",
                pending_ko.player_switch_index,
                pending_ko.enemy_switch_index,
                pending_ko.player_defeated,
                pending_ko.enemy_defeated
            ),
        );
        next_phase.set(BattlePhase::DeathResolve);
    } else {
        for &entity in &player_team.0.combatants {
            if let Ok((mut stats, _shield, mut statuses, mut aura)) = queries.p1().get_mut(entity) {
                process_round_end_status_durations(
                    &mut stats,
                    &mut aura,
                    &mut statuses,
                    crate::battle::SideEndTickParams {
                        side: Side::Player,
                        round: turn_count.0,
                        formula_rules: &formula_rules,
                        event_writer: &mut event_writer,
                        formula_writer: &mut formula_writer,
                        status_writer: &mut status_writer,
                        structured_log: &mut structured_log,
                    },
                );
            }
        }
        for &entity in &enemy_team.0.combatants {
            if let Ok((mut stats, _shield, mut statuses, mut aura)) = queries.p1().get_mut(entity) {
                process_round_end_status_durations(
                    &mut stats,
                    &mut aura,
                    &mut statuses,
                    crate::battle::SideEndTickParams {
                        side: Side::Enemy,
                        round: turn_count.0,
                        formula_rules: &formula_rules,
                        event_writer: &mut event_writer,
                        formula_writer: &mut formula_writer,
                        status_writer: &mut status_writer,
                        structured_log: &mut structured_log,
                    },
                );
            }
        }
        next_phase.set(BattlePhase::RoundStart);
    }
}

pub fn resolve_ko_system(
    time: Res<Time>,
    mut query: Query<
        (
            &Combatant,
            &mut Stats,
            &Name,
            &mut crate::battle::StatusBoard,
        ),
        With<InBattle>,
    >,
    mut player_team: ResMut<crate::battle::PlayerTeam>,
    mut enemy_team: ResMut<crate::battle::EnemyTeam>,
    mut pending_ko: ResMut<PendingKoResolution>,
    turn_count: Res<TurnCount>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut battle_result: ResMut<BattleResult>,
    mut battle_log: ResMut<BattleLog>,
    mut structured_log: ResMut<StructuredBattleLog>,
    mut action_trace: ResMut<ActionTrace>,
) {
    pending_ko.timer.tick(time.delta());
    if !pending_ko.timer.is_finished() {
        return;
    }

    if let Some(idx) = pending_ko.player_switch_index.take() {
        let old_e = player_team.0.combatants[player_team.0.active_index];
        let new_e = player_team.0.combatants[idx];
        if let Ok(
            [
                (_, mut old_stats, _, mut old_statuses),
                (_, new_stats, new_n, new_statuses),
            ],
        ) = query.get_many_mut([old_e, new_e])
        {
            if new_stats.hp > 0 {
                transfer_status_by_id(
                    &mut old_statuses,
                    &mut old_stats,
                    new_statuses.into_inner(),
                    new_stats.into_inner(),
                    "nature_regen",
                );
                player_team.0.active_index = idx;
                event_writer.write(BattleEvent::Switched {
                    side: Side::Player,
                    name: new_n.to_string(),
                });
                push_battle_line(&mut battle_log, format!("玩家换上了 {}！", new_n));
                note_action_phase(
                    &mut structured_log,
                    turn_count.0,
                    Side::Player,
                    "玩家自动换人",
                    format!("换上 {}；索引={idx}", new_n),
                );
                push_named_action_trace(
                    &mut action_trace,
                    turn_count.0,
                    Side::Player,
                    "auto_switch",
                    format!("换上 {}；索引={idx}", new_n),
                );
            }
        }
    }

    if let Some(idx) = pending_ko.enemy_switch_index.take() {
        let old_e = enemy_team.0.combatants[enemy_team.0.active_index];
        let new_e = enemy_team.0.combatants[idx];
        if let Ok(
            [
                (_, mut old_stats, _, mut old_statuses),
                (_, new_stats, new_n, new_statuses),
            ],
        ) = query.get_many_mut([old_e, new_e])
        {
            if new_stats.hp > 0 {
                transfer_status_by_id(
                    &mut old_statuses,
                    &mut old_stats,
                    new_statuses.into_inner(),
                    new_stats.into_inner(),
                    "nature_regen",
                );
                enemy_team.0.active_index = idx;
                event_writer.write(BattleEvent::Switched {
                    side: Side::Enemy,
                    name: new_n.to_string(),
                });
                push_battle_line(&mut battle_log, format!("敌方换上了 {}！", new_n));
                note_action_phase(
                    &mut structured_log,
                    turn_count.0,
                    Side::Enemy,
                    "敌方自动换人",
                    format!("换上 {}；索引={idx}", new_n),
                );
                push_named_action_trace(
                    &mut action_trace,
                    turn_count.0,
                    Side::Enemy,
                    "auto_switch",
                    format!("换上 {}；索引={idx}", new_n),
                );
            }
        }
    }

    if pending_ko.player_defeated || pending_ko.enemy_defeated {
        battle_result.message = if pending_ko.player_defeated && pending_ko.enemy_defeated {
            "平局！按 R 返回大厅。".to_string()
        } else if pending_ko.enemy_defeated {
            "胜利！全歼敌方。按 R 返回大厅。".to_string()
        } else {
            "失败！队伍全灭。按 R 返回大厅。".to_string()
        };
        note_structured_phase(
            &mut structured_log,
            "battle-result",
            "战斗结束",
            battle_result.message.clone(),
        );
        push_named_action_trace(
            &mut action_trace,
            turn_count.0,
            Side::Player,
            "battle_result",
            battle_result.message.clone(),
        );
        pending_ko.player_defeated = false;
        pending_ko.enemy_defeated = false;
        next_game_state.set(GameState::Result);
    } else {
        note_structured_phase(
            &mut structured_log,
            "death-resolve",
            "死亡结算完成",
            "双方已完成换人，返回下一回合。",
        );
        next_phase.set(BattlePhase::RoundStart);
    }
}

fn sanitize_filename_segment(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch);
        } else if matches!(ch, ' ' | '-' | '_') {
            output.push('_');
        }
    }
    let trimmed = output.trim_matches('_');
    if trimmed.is_empty() {
        "battle_result".to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn write_export_logs(
    export_dir: &Path,
    battle_result: &BattleResult,
    replay_log: &ReplayEventLog,
    action_trace: &ActionTrace,
) -> Result<String, String> {
    fs::create_dir_all(export_dir).map_err(|err| format!("创建导出目录失败：{err}"))?;

    let result_slug = sanitize_filename_segment(&battle_result.message);
    let replay_path = export_dir.join(format!("{result_slug}_replay.ron"));
    let action_path = export_dir.join(format!("{result_slug}_action_trace.ron"));

    let replay_text = ron::ser::to_string_pretty(&replay_log.0, ron::ser::PrettyConfig::default())
        .map_err(|err| format!("序列化 replay log 失败：{err}"))?;
    fs::write(&replay_path, replay_text)
        .map_err(|err| format!("写入 {:?} 失败：{err}", replay_path))?;

    let action_text =
        ron::ser::to_string_pretty(&action_trace.0, ron::ser::PrettyConfig::default())
            .map_err(|err| format!("序列化 action trace 失败：{err}"))?;
    fs::write(&action_path, action_text)
        .map_err(|err| format!("写入 {:?} 失败：{err}", action_path))?;

    Ok(format!(
        "已导出：{} 与 {}",
        replay_path.display(),
        action_path.display()
    ))
}

fn export_logs(
    battle_result: &BattleResult,
    replay_log: &ReplayEventLog,
    action_trace: &ActionTrace,
) -> Result<String, String> {
    let export_dir = PathBuf::from("battle_logs");
    write_export_logs(&export_dir, battle_result, replay_log, action_trace)
}

pub fn restart_from_result_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection_state: ResMut<crate::team_selection::SelectionState>,
    mut entry_mode: ResMut<crate::team_selection::SelectionEntryMode>,
    team_selections: Option<ResMut<crate::data::TeamSelections>>,
    replay_log: Res<ReplayEventLog>,
    action_trace: Res<ActionTrace>,
    mut battle_mode: ResMut<crate::battle::BattleControlMode>,
    mut ui_control_side: ResMut<crate::battle::UiControlSide>,
    mut selected_cards: ResMut<crate::battle::SelectedCards>,
    mut battle_result: ResMut<BattleResult>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyL) {
        battle_result.export_status = Some(
            match export_logs(&battle_result, &replay_log, &action_trace) {
                Ok(message) => message,
                Err(message) => format!("导出失败：{message}"),
            },
        );
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        battle_result.export_status = None;
        selection_state.reset();
        *entry_mode = crate::team_selection::SelectionEntryMode::VsAi;
        *battle_mode = crate::battle::BattleControlMode::PlayerVsAi;
        ui_control_side.0 = Side::Player;
        *selected_cards = crate::battle::SelectedCards::default();
        if let Some(mut team_selections) = team_selections {
            team_selections.player_indices.clear();
            team_selections.enemy_indices.clear();
        }

        next_phase.set(BattlePhase::Init);
        next_game_state.set(GameState::Lobby);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        battle::{
            ActionTraceEntry, BattleEvent, BattleFormulaEvent, BattleLifecycleEvent,
            BattleStateEvent, BattleStatusEvent, BattleTraceEvent, ElementAura, EnemyTeam,
            PlayerTeam, ReplayEventLog, Shield, Side, StatusBoard, StatusInstance,
            StructuredBattleLog, Team, systems::consume_battle_events_system,
        },
        data::{BattleFormulaRules, StatusCategory, StatusTickTiming},
        game_state::GameState,
    };
    use bevy::{ecs::message::Messages, prelude::State, time::TimePlugin};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_stats(hp: i32) -> Stats {
        Stats {
            hp,
            max_hp: 20,
            atk: 5,
            def: 5,
            spd: 5,
            acc: 100,
            atk_stage: 0,
            def_stage: 0,
            spd_stage: 0,
            acc_stage: 0,
        }
    }

    #[test]
    fn check_end_marks_auto_switch_and_enters_death_resolve() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.init_resource::<Messages<BattleEvent>>();
        app.init_resource::<Messages<BattleFormulaEvent>>();
        app.init_resource::<Messages<BattleStatusEvent>>();
        app.init_resource::<PendingKoResolution>();
        app.insert_resource(TurnCount(4));
        app.insert_resource(BattleFormulaRules::default());
        app.insert_resource(BattleLog::default());
        app.insert_resource(StructuredBattleLog::default());
        app.insert_resource(ActionTrace::default());
        app.insert_resource(BattleResult::default());
        app.insert_resource(NextState::<BattlePhase>::default());
        app.insert_resource(NextState::<GameState>::default());
        app.add_systems(Update, check_end_system);

        let player_active = app
            .world_mut()
            .spawn((
                InBattle,
                Name::new("Player A"),
                Combatant {
                    side: Side::Player,
                    element: crate::data::ElementType::Fire,
                },
                test_stats(0),
                StatusBoard::default(),
            ))
            .id();
        let player_bench = app
            .world_mut()
            .spawn((
                InBattle,
                Name::new("Player B"),
                Combatant {
                    side: Side::Player,
                    element: crate::data::ElementType::Water,
                },
                test_stats(12),
                StatusBoard::default(),
            ))
            .id();
        let enemy_active = app
            .world_mut()
            .spawn((
                InBattle,
                Name::new("Enemy A"),
                Combatant {
                    side: Side::Enemy,
                    element: crate::data::ElementType::Grass,
                },
                test_stats(10),
                StatusBoard::default(),
            ))
            .id();

        app.insert_resource(PlayerTeam(Team {
            combatants: vec![player_active, player_bench],
            active_index: 0,
        }));
        app.insert_resource(EnemyTeam(Team {
            combatants: vec![enemy_active],
            active_index: 0,
        }));

        app.update();

        let pending = app.world().resource::<PendingKoResolution>();
        assert_eq!(pending.player_switch_index, Some(1));
        assert_eq!(pending.enemy_switch_index, None);
        assert!(!pending.player_defeated);
        assert!(!pending.enemy_defeated);

        let next_phase = app.world().resource::<NextState<BattlePhase>>();
        assert!(matches!(
            next_phase,
            NextState::Pending(BattlePhase::DeathResolve)
        ));

        let trace = app.world().resource::<ActionTrace>();
        assert!(trace.0.iter().any(|entry| {
            entry.action == "fainted" && entry.detail.contains("Player A 倒下")
        }));
    }

    #[test]
    fn check_end_decrements_statuses_only_at_full_round_end() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.init_resource::<Messages<BattleEvent>>();
        app.init_resource::<Messages<BattleFormulaEvent>>();
        app.init_resource::<Messages<BattleStatusEvent>>();
        app.init_resource::<PendingKoResolution>();
        app.insert_resource(TurnCount(1));
        app.insert_resource(BattleFormulaRules::default());
        app.insert_resource(BattleLog::default());
        app.insert_resource(StructuredBattleLog::default());
        app.insert_resource(ActionTrace::default());
        app.insert_resource(BattleResult::default());
        app.insert_resource(NextState::<BattlePhase>::default());
        app.insert_resource(NextState::<GameState>::default());
        app.add_systems(Update, check_end_system);

        let player_active = app
            .world_mut()
            .spawn((
                InBattle,
                Name::new("Player A"),
                Combatant {
                    side: Side::Player,
                    element: crate::data::ElementType::Wind,
                },
                test_stats(10),
                Shield(0),
                ElementAura::default(),
                StatusBoard {
                    entries: vec![StatusInstance {
                        id: "wind_evade".to_string(),
                        name: "闪避".to_string(),
                        category: StatusCategory::Buff,
                        remaining_turns: 1,
                        applied_round: 1,
                        source_side: Some(Side::Player),
                        tick_timing: Some(StatusTickTiming::OwnerActionEnd),
                        stage_modifiers: vec![],
                        fixed_damage_on_tick: 0,
                        heal_on_tick: 0,
                        heal_taken_multiplier: None,
                        evade_charges: 1,
                    }],
                },
            ))
            .id();
        let enemy_active = app
            .world_mut()
            .spawn((
                InBattle,
                Name::new("Enemy A"),
                Combatant {
                    side: Side::Enemy,
                    element: crate::data::ElementType::Grass,
                },
                test_stats(10),
                Shield(0),
                ElementAura::default(),
                StatusBoard::default(),
            ))
            .id();

        app.insert_resource(PlayerTeam(Team {
            combatants: vec![player_active],
            active_index: 0,
        }));
        app.insert_resource(EnemyTeam(Team {
            combatants: vec![enemy_active],
            active_index: 0,
        }));

        app.update();

        let player_statuses = app
            .world()
            .entity(player_active)
            .get::<StatusBoard>()
            .unwrap();
        assert_eq!(player_statuses.entries.len(), 1);
        assert_eq!(player_statuses.entries[0].remaining_turns, 1);

        app.world_mut().resource_mut::<TurnCount>().0 = 2;
        app.update();

        let player_statuses = app
            .world()
            .entity(player_active)
            .get::<StatusBoard>()
            .unwrap();
        assert!(player_statuses.entries.is_empty());
    }

    #[test]
    fn resolve_ko_switches_to_next_living_combatant() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.init_resource::<Messages<BattleEvent>>();
        app.insert_resource(PendingKoResolution {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            player_switch_index: Some(1),
            enemy_switch_index: None,
            player_defeated: false,
            enemy_defeated: false,
        });
        app.insert_resource(TurnCount(5));
        app.insert_resource(BattleLog::default());
        app.insert_resource(StructuredBattleLog::default());
        app.insert_resource(ActionTrace::default());
        app.insert_resource(BattleResult::default());
        app.insert_resource(NextState::<BattlePhase>::default());
        app.insert_resource(NextState::<GameState>::default());
        app.add_systems(Update, resolve_ko_system);

        let player_active = app
            .world_mut()
            .spawn((
                InBattle,
                Name::new("Player A"),
                Combatant {
                    side: Side::Player,
                    element: crate::data::ElementType::Fire,
                },
                test_stats(0),
                StatusBoard::default(),
            ))
            .id();
        let player_bench = app
            .world_mut()
            .spawn((
                InBattle,
                Name::new("Player B"),
                Combatant {
                    side: Side::Player,
                    element: crate::data::ElementType::Water,
                },
                test_stats(14),
                StatusBoard::default(),
            ))
            .id();
        let enemy_active = app
            .world_mut()
            .spawn((
                InBattle,
                Name::new("Enemy A"),
                Combatant {
                    side: Side::Enemy,
                    element: crate::data::ElementType::Grass,
                },
                test_stats(8),
                StatusBoard::default(),
            ))
            .id();

        app.insert_resource(PlayerTeam(Team {
            combatants: vec![player_active, player_bench],
            active_index: 0,
        }));
        app.insert_resource(EnemyTeam(Team {
            combatants: vec![enemy_active],
            active_index: 0,
        }));

        app.update();

        let player_team = app.world().resource::<PlayerTeam>();
        assert_eq!(player_team.0.active_index, 1);

        let next_phase = app.world().resource::<NextState<BattlePhase>>();
        assert!(matches!(
            next_phase,
            NextState::Pending(BattlePhase::RoundStart)
        ));

        let battle_log = app.world().resource::<BattleLog>();
        assert!(
            battle_log
                .0
                .iter()
                .any(|line| line.contains("玩家换上了 Player B"))
        );

        let trace = app.world().resource::<ActionTrace>();
        assert!(
            trace
                .0
                .iter()
                .any(|entry| entry.action == "auto_switch" && entry.detail.contains("Player B"))
        );
    }

    #[test]
    fn write_export_logs_writes_replay_and_action_trace_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let export_dir = std::env::temp_dir().join(format!("bevy_mon_proto_test_{unique}"));

        let battle_result = BattleResult {
            message: "胜利！全歼敌方。按 R 重新开始。".to_string(),
            export_status: None,
        };
        let replay_log = ReplayEventLog(vec![crate::battle::ReplayLogEntry {
            seq: 1,
            phase: "battle-event".to_string(),
            summary: "BattleEvent".to_string(),
            detail: "玩家使用了 火拳。".to_string(),
        }]);
        let action_trace = ActionTrace(vec![ActionTraceEntry {
            seq: 1,
            round: 1,
            side: Side::Player,
            action: "skill:FirePunch".to_string(),
            detail: "玩家释放火拳".to_string(),
        }]);

        let export_message =
            write_export_logs(&export_dir, &battle_result, &replay_log, &action_trace)
                .expect("export should succeed");

        let slug = sanitize_filename_segment(&battle_result.message);
        let replay_path = export_dir.join(format!("{slug}_replay.ron"));
        let action_path = export_dir.join(format!("{slug}_action_trace.ron"));
        assert!(replay_path.exists());
        assert!(action_path.exists());
        assert!(export_message.contains(&format!("{slug}_replay.ron")));
        assert!(export_message.contains(&format!("{slug}_action_trace.ron")));

        let replay_text = fs::read_to_string(&replay_path).expect("read replay export");
        let action_text = fs::read_to_string(&action_path).expect("read action export");
        assert!(replay_text.contains("BattleEvent"));
        assert!(replay_text.contains("玩家使用了 火拳"));
        assert!(action_text.contains("skill:FirePunch"));
        assert!(action_text.contains("玩家释放火拳"));

        let _ = fs::remove_dir_all(&export_dir);
    }

    #[test]
    fn exported_replay_and_action_logs_match_runtime_generated_content() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let export_dir =
            std::env::temp_dir().join(format!("bevy_mon_proto_runtime_export_{unique}"));

        let mut app = App::new();
        app.init_resource::<Messages<BattleEvent>>();
        app.init_resource::<Messages<BattleTraceEvent>>();
        app.init_resource::<Messages<BattleStateEvent>>();
        app.init_resource::<Messages<BattleLifecycleEvent>>();
        app.init_resource::<Messages<BattleFormulaEvent>>();
        app.init_resource::<Messages<BattleStatusEvent>>();
        app.insert_resource(BattleLog::default());
        app.insert_resource(StructuredBattleLog::default());
        app.insert_resource(ReplayEventLog::default());
        app.insert_resource(BattleResult {
            message: "胜利！全歼敌方。按 R 重新开始。".to_string(),
            export_status: None,
        });
        app.insert_resource(TurnCount(3));
        app.insert_resource(State::new(GameState::Battle));
        app.add_systems(Update, consume_battle_events_system);

        let player = app
            .world_mut()
            .spawn((
                InBattle,
                Name::new("Player A"),
                Combatant {
                    side: Side::Player,
                    element: crate::data::ElementType::Fire,
                },
                test_stats(18),
                Shield(4),
                ElementAura::default(),
                crate::battle::StatusBoard::default(),
            ))
            .id();
        let enemy = app
            .world_mut()
            .spawn((
                InBattle,
                Name::new("Enemy A"),
                Combatant {
                    side: Side::Enemy,
                    element: crate::data::ElementType::Water,
                },
                test_stats(11),
                Shield(0),
                ElementAura::default(),
                crate::battle::StatusBoard::default(),
            ))
            .id();

        app.insert_resource(PlayerTeam(Team {
            combatants: vec![player],
            active_index: 0,
        }));
        app.insert_resource(EnemyTeam(Team {
            combatants: vec![enemy],
            active_index: 0,
        }));

        app.world_mut()
            .resource_mut::<Messages<BattleEvent>>()
            .write(BattleEvent::SkillUsed {
                side: Side::Player,
                skill_name: "火拳".to_string(),
                slot: 0,
            });
        app.world_mut()
            .resource_mut::<Messages<BattleEvent>>()
            .write(BattleEvent::DamageDealt {
                source: Side::Player,
                target: Side::Enemy,
                amount: 7,
            });
        app.world_mut()
            .resource_mut::<Messages<BattleTraceEvent>>()
            .write(BattleTraceEvent {
                round: 3,
                side: Side::Player,
                action: "skill:FirePunch".to_string(),
                detail: "玩家释放火拳并命中。".to_string(),
            });
        app.world_mut()
            .resource_mut::<Messages<BattleFormulaEvent>>()
            .write(BattleFormulaEvent {
                round: 3,
                source: Side::Player,
                target: Side::Enemy,
                action: "damage_formula".to_string(),
                detail: "基础伤害=7；护盾吸收=0；最终生命伤害=7。".to_string(),
            });
        app.world_mut()
            .resource_mut::<Messages<BattleStatusEvent>>()
            .write(BattleStatusEvent {
                round: 3,
                subject: Side::Enemy,
                status_id: "burning_aura".to_string(),
                action: "applied".to_string(),
                detail: "敌方获得燃烧附着。".to_string(),
            });
        app.world_mut()
            .resource_mut::<Messages<BattleLifecycleEvent>>()
            .write(BattleLifecycleEvent {
                phase: "battle-result".to_string(),
                summary: "战斗结束".to_string(),
                detail: "胜利！全歼敌方。按 R 重新开始。".to_string(),
            });

        app.update();

        let replay_log = app.world().resource::<ReplayEventLog>();
        let action_trace = ActionTrace(vec![ActionTraceEntry {
            seq: 1,
            round: 3,
            side: Side::Player,
            action: "skill:FirePunch".to_string(),
            detail: "玩家释放火拳并命中。".to_string(),
        }]);
        let battle_result = app.world().resource::<BattleResult>();

        assert_eq!(replay_log.0.len(), 6);
        assert_eq!(replay_log.0[0].seq, 1);
        assert_eq!(replay_log.0[0].phase, "battle-event");
        assert_eq!(replay_log.0[0].summary, "BattleEvent");
        assert_eq!(replay_log.0[0].detail, "玩家 使用了 火拳。");
        assert_eq!(replay_log.0[1].seq, 2);
        assert_eq!(replay_log.0[1].detail, "玩家 对 敌方 造成了 7 点实际伤害。");
        assert_eq!(replay_log.0[2].phase, "trace-r3");
        assert_eq!(replay_log.0[2].summary, "skill:FirePunch");
        assert_eq!(replay_log.0[3].phase, "battle-result");
        assert_eq!(replay_log.0[3].summary, "战斗结束");
        assert_eq!(replay_log.0[4].phase, "formula-r3");
        assert_eq!(replay_log.0[4].summary, "damage_formula");
        assert_eq!(replay_log.0[5].phase, "status-r3");
        assert_eq!(replay_log.0[5].summary, "burning_aura:applied");

        let export_message =
            write_export_logs(&export_dir, battle_result, replay_log, &action_trace)
                .expect("export should succeed");
        let slug = sanitize_filename_segment(&battle_result.message);
        let replay_path = export_dir.join(format!("{slug}_replay.ron"));
        let action_path = export_dir.join(format!("{slug}_action_trace.ron"));
        assert!(export_message.contains(&format!("{slug}_replay.ron")));
        assert!(export_message.contains(&format!("{slug}_action_trace.ron")));

        let replay_text = fs::read_to_string(&replay_path).expect("read replay export");
        let action_text = fs::read_to_string(&action_path).expect("read action export");

        assert!(replay_text.contains("phase: \"battle-event\""));
        assert!(replay_text.contains("detail: \"玩家 使用了 火拳。\""));
        assert!(replay_text.contains("detail: \"玩家 对 敌方 造成了 7 点实际伤害。\""));
        assert!(replay_text.contains("phase: \"trace-r3\""));
        assert!(replay_text.contains("summary: \"skill:FirePunch\""));
        assert!(replay_text.contains("phase: \"battle-result\""));
        assert!(replay_text.contains("summary: \"战斗结束\""));
        assert!(replay_text.contains("phase: \"formula-r3\""));
        assert!(replay_text.contains("detail: \"基础伤害=7；护盾吸收=0；最终生命伤害=7。\""));
        assert!(replay_text.contains("summary: \"burning_aura:applied\""));
        assert!(action_text.contains("seq: 1"));
        assert!(action_text.contains("round: 3"));
        assert!(action_text.contains("action: \"skill:FirePunch\""));
        assert!(action_text.contains("detail: \"玩家释放火拳并命中。\""));

        let _ = fs::remove_dir_all(&export_dir);
    }
}
