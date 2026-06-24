use std::{
    fs,
    path::{Path, PathBuf},
};

use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    battle::{
        ActionTrace, BattleControlMode, BattleEvent, BattleLog, BattlePerformanceOutcome,
        BattlePerformanceReport, BattlePerformanceStats, BattleResult, BattleResultAction,
        BattleResultNotice, Combatant, ElementAura, InBattle, PendingBattleResultAction,
        PendingKoResolution, PerformanceTeamSnapshot, ReplayEventLog, RoundOrder, Shield, Side,
        Stats, StructuredBattleLog, TurnContext, TurnCount,
        ai::{
            AiBattleOutcome, AiEpisodeStats, AiSideEpisodeStats, AiTeamSnapshot, reward_for_episode,
        },
        battle_phase_for_side, build_performance_report, format_performance_report,
        note_action_phase, note_structured_phase, push_battle_line, push_named_action_trace,
        transfer_status_by_id,
    },
    data::{EnemyAiConfig, TeamSelections},
    game_state::{BattlePhase, GameState},
    pvp,
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

    if p_hp <= 0 || e_hp <= 0 {
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
        pending_ko.player_switch_index = None;
        pending_ko.enemy_switch_index = None;
        pending_ko.player_defeated = false;
        pending_ko.enemy_defeated = false;
        pending_ko.resume_phase = None;

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
                        pending_boosts: None,
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
                        pending_boosts: None,
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

#[derive(SystemParam)]
pub struct ResolveKoPerformance<'w> {
    battle_mode: Res<'w, BattleControlMode>,
    stats: Res<'w, BattlePerformanceStats>,
    report: ResMut<'w, BattlePerformanceReport>,
    ai_config: Option<Res<'w, EnemyAiConfig>>,
    ai_decision_log: Option<ResMut<'w, crate::battle::ai::AiDecisionLog>>,
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
    turn_ctx: Res<TurnContext>,
    round_order: Res<RoundOrder>,
    mut performance: ResolveKoPerformance,
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
        let ai_outcome = if pending_ko.player_defeated && pending_ko.enemy_defeated {
            AiBattleOutcome::Draw
        } else if pending_ko.enemy_defeated {
            AiBattleOutcome::EnemyDefeat
        } else {
            AiBattleOutcome::EnemyVictory
        };
        let (player_snapshot, enemy_snapshot) = performance_team_snapshots(&mut query);
        let ai_episode_stats =
            ai_episode_stats(&performance.stats, player_snapshot, enemy_snapshot);
        let ai_reward = reward_for_episode(ai_outcome, ai_episode_stats);
        if *performance.battle_mode == BattleControlMode::PlayerVsRemote {
            performance.report.summary = None;
        } else {
            let outcome = if pending_ko.player_defeated && pending_ko.enemy_defeated {
                BattlePerformanceOutcome::Draw
            } else if pending_ko.enemy_defeated {
                BattlePerformanceOutcome::Victory
            } else {
                BattlePerformanceOutcome::Defeat
            };
            performance.report.summary = Some(build_performance_report(
                &performance.stats,
                outcome,
                turn_count.0,
                player_snapshot,
                enemy_snapshot,
            ));
        }
        if let Some(ai_decision_log) = performance.ai_decision_log.as_mut() {
            ai_decision_log.finalize_with_reward(ai_outcome, ai_reward);
        }

        battle_result.message = if pending_ko.player_defeated && pending_ko.enemy_defeated {
            "平局！按 R 返回。".to_string()
        } else if pending_ko.enemy_defeated {
            "胜利！全歼敌方。按 R 返回。".to_string()
        } else {
            "失败！队伍全灭。按 R 返回。".to_string()
        };
        note_structured_phase(
            &mut structured_log,
            "battle-result",
            "战斗结束",
            battle_result.message.clone(),
        );
        if let (Some(ai_config), Some(ai_decision_log)) = (
            performance.ai_config.as_ref(),
            performance.ai_decision_log.as_ref(),
        ) {
            match ai_decision_log.export_if_enabled(ai_config) {
                Ok(Some(path)) => {
                    battle_result.export_status = Some(format!("AI 决策样本已导出：{path}"));
                }
                Ok(None) => {}
                Err(err) => {
                    battle_result.export_status = Some(format!("AI 决策样本导出失败：{err}"));
                }
            }
        }
        push_named_action_trace(
            &mut action_trace,
            turn_count.0,
            Side::Player,
            "battle_result",
            battle_result.message.clone(),
        );
        pending_ko.player_defeated = false;
        pending_ko.enemy_defeated = false;
        pending_ko.resume_phase = None;
        next_game_state.set(GameState::Result);
    } else if let Some(phase) = pending_ko.resume_phase.take() {
        note_structured_phase(
            &mut structured_log,
            "death-resolve",
            "死亡结算完成",
            "双方已完成换人，返回当前行动方。",
        );
        next_phase.set(phase);
    } else {
        let next = if round_order.first == Side::Player
            && turn_ctx.player_ended
            && !turn_ctx.enemy_ended
        {
            battle_phase_for_side(round_order.second)
        } else if round_order.first == Side::Enemy && turn_ctx.enemy_ended && !turn_ctx.player_ended
        {
            battle_phase_for_side(round_order.second)
        } else {
            BattlePhase::CheckEnd
        };
        note_structured_phase(
            &mut structured_log,
            "death-resolve",
            "死亡结算完成",
            "双方已完成换人，继续回合结算。",
        );
        next_phase.set(next);
    }
}

fn performance_team_snapshots(
    query: &mut Query<
        (
            &Combatant,
            &mut Stats,
            &Name,
            &mut crate::battle::StatusBoard,
        ),
        With<InBattle>,
    >,
) -> (PerformanceTeamSnapshot, PerformanceTeamSnapshot) {
    let mut player = PerformanceTeamSnapshot::default();
    let mut enemy = PerformanceTeamSnapshot::default();
    for (combatant, stats, _, _) in query.iter_mut() {
        let snapshot = match combatant.side {
            Side::Player => &mut player,
            Side::Enemy => &mut enemy,
        };
        snapshot.current_hp += stats.hp.max(0);
        snapshot.max_hp += stats.max_hp.max(0);
        snapshot.member_count += 1;
        if stats.hp > 0 {
            snapshot.alive_count += 1;
        }
    }
    (player, enemy)
}

fn ai_episode_stats(
    stats: &BattlePerformanceStats,
    player: PerformanceTeamSnapshot,
    enemy: PerformanceTeamSnapshot,
) -> AiEpisodeStats {
    AiEpisodeStats {
        enemy: AiSideEpisodeStats {
            damage_dealt: stats.enemy.damage_dealt,
            knockouts: stats.enemy.knockouts,
            two_element_reactions: stats.enemy.two_element_reactions,
            advanced_reactions: stats.enemy.advanced_reactions,
        },
        player: AiSideEpisodeStats {
            damage_dealt: stats.player.damage_dealt,
            knockouts: stats.player.knockouts,
            two_element_reactions: stats.player.two_element_reactions,
            advanced_reactions: stats.player.advanced_reactions,
        },
        enemy_team: AiTeamSnapshot {
            current_hp: enemy.current_hp,
            max_hp: enemy.max_hp,
            alive_count: enemy.alive_count,
            member_count: enemy.member_count,
        },
        player_team: AiTeamSnapshot {
            current_hp: player.current_hp,
            max_hp: player.max_hp,
            alive_count: player.alive_count,
            member_count: player.member_count,
        },
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
    performance_report: Option<&BattlePerformanceReport>,
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

    let mut exported_paths = vec![
        replay_path.display().to_string(),
        action_path.display().to_string(),
    ];
    if let Some(summary) = performance_report.and_then(|report| report.summary.as_ref()) {
        let performance_path = export_dir.join(format!("{result_slug}_performance.txt"));
        fs::write(&performance_path, format_performance_report(summary))
            .map_err(|err| format!("写入 {:?} 失败：{err}", performance_path))?;
        exported_paths.push(performance_path.display().to_string());
    }

    Ok(format!("已导出：{}", exported_paths.join("、")))
}

fn export_logs(
    battle_result: &BattleResult,
    replay_log: &ReplayEventLog,
    action_trace: &ActionTrace,
    performance_report: &BattlePerformanceReport,
) -> Result<String, String> {
    let export_dir = PathBuf::from("battle_logs");
    write_export_logs(
        &export_dir,
        battle_result,
        replay_log,
        action_trace,
        Some(performance_report),
    )
}

#[derive(SystemParam)]
pub struct ResultRuntime<'w> {
    selection_state: ResMut<'w, crate::team_selection::SelectionState>,
    entry_mode: ResMut<'w, crate::team_selection::SelectionEntryMode>,
    map_battle_context: ResMut<'w, crate::data::MapBattleContext>,
    current_map: ResMut<'w, crate::map::components::CurrentMap>,
    team_selections: Option<ResMut<'w, TeamSelections>>,
    replay_log: Res<'w, ReplayEventLog>,
    action_trace: Res<'w, ActionTrace>,
    performance_report: Res<'w, BattlePerformanceReport>,
    battle_mode: ResMut<'w, BattleControlMode>,
    ui_control_side: ResMut<'w, crate::battle::UiControlSide>,
    selected_cards: ResMut<'w, crate::battle::SelectedCards>,
    battle_result: ResMut<'w, BattleResult>,
    result_notice: ResMut<'w, BattleResultNotice>,
    pending_action: ResMut<'w, PendingBattleResultAction>,
    pvp_connection: Option<ResMut<'w, pvp::PvpConnection>>,
    next_phase: ResMut<'w, NextState<BattlePhase>>,
    next_game_state: ResMut<'w, NextState<GameState>>,
}

pub fn restart_from_result_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut runtime: ResultRuntime,
) {
    if runtime.result_notice.remaining > 0.0 {
        runtime.result_notice.remaining =
            (runtime.result_notice.remaining - time.delta_secs()).max(0.0);
        if runtime.result_notice.remaining == 0.0 {
            runtime.result_notice.text.clear();
        }
    }

    if keyboard.just_pressed(KeyCode::KeyL) {
        match export_logs(
            &runtime.battle_result,
            &runtime.replay_log,
            &runtime.action_trace,
            &runtime.performance_report,
        ) {
            Ok(_) => {
                runtime.result_notice.text = "导出成功".to_string();
                runtime.result_notice.remaining = 2.0;
            }
            Err(message) => {
                runtime.result_notice.text = format!("导出失败：{message}");
                runtime.result_notice.remaining = 2.5;
            }
        }
    }

    let keyboard_action = keyboard
        .just_pressed(KeyCode::KeyR)
        .then_some(BattleResultAction::Return);
    let action = keyboard_action.or(runtime.pending_action.0);
    let Some(action) = action else {
        return;
    };

    let is_pvp = *runtime.battle_mode == BattleControlMode::PlayerVsRemote;
    match action {
        BattleResultAction::Return => {
            runtime.pending_action.0 = None;
            return_from_result(
                is_pvp,
                &mut runtime.selection_state,
                &mut runtime.entry_mode,
                &mut runtime.map_battle_context,
                &mut runtime.current_map,
                runtime.team_selections.as_deref_mut(),
                &mut runtime.battle_mode,
                &mut runtime.ui_control_side,
                &mut runtime.selected_cards,
                &mut runtime.battle_result,
                runtime.pvp_connection.as_deref_mut(),
                &mut runtime.next_phase,
                &mut runtime.next_game_state,
            );
        }
        BattleResultAction::RestartSameTeams if !is_pvp => {
            runtime.pending_action.0 = None;
            runtime.battle_result.message.clear();
            runtime.battle_result.export_status = None;
            runtime.result_notice.text.clear();
            runtime.result_notice.remaining = 0.0;
            *runtime.selected_cards = crate::battle::SelectedCards::default();
            runtime.next_phase.set(BattlePhase::Init);
            runtime.next_game_state.set(GameState::Battle);
        }
        BattleResultAction::Rematch if !is_pvp => {
            runtime.pending_action.0 = None;
            let previous_battle_mode = *runtime.battle_mode;
            rematch_from_result(
                previous_battle_mode,
                &mut runtime.selection_state,
                &mut runtime.entry_mode,
                &mut runtime.map_battle_context,
                runtime.team_selections.as_deref_mut(),
                &mut runtime.battle_mode,
                &mut runtime.ui_control_side,
                &mut runtime.selected_cards,
                &mut runtime.battle_result,
                &mut runtime.result_notice,
                &mut runtime.next_phase,
                &mut runtime.next_game_state,
            );
        }
        BattleResultAction::RestartSameTeams => {
            runtime.pending_action.0 = None;
        }
        BattleResultAction::Rematch
        | BattleResultAction::AcceptRematch
        | BattleResultAction::RejectRematch => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn return_from_result(
    is_pvp: bool,
    selection_state: &mut crate::team_selection::SelectionState,
    entry_mode: &mut crate::team_selection::SelectionEntryMode,
    map_battle_context: &mut crate::data::MapBattleContext,
    current_map: &mut crate::map::components::CurrentMap,
    team_selections: Option<&mut TeamSelections>,
    battle_mode: &mut BattleControlMode,
    ui_control_side: &mut crate::battle::UiControlSide,
    selected_cards: &mut crate::battle::SelectedCards,
    battle_result: &mut BattleResult,
    pvp_connection: Option<&mut pvp::PvpConnection>,
    next_phase: &mut NextState<BattlePhase>,
    next_game_state: &mut NextState<GameState>,
) {
    let return_map = map_battle_context.return_map.take();
    map_battle_context.enemy_monster_index = None;
    battle_result.message.clear();
    battle_result.export_status = None;
    selection_state.reset();
    *entry_mode = crate::team_selection::SelectionEntryMode::VsAi;
    *battle_mode = BattleControlMode::PlayerVsAi;
    ui_control_side.0 = Side::Player;
    *selected_cards = crate::battle::SelectedCards::default();
    if let Some(team_selections) = team_selections {
        team_selections.player_indices.clear();
        team_selections.enemy_indices.clear();
    }

    if is_pvp {
        if let Some(connection) = pvp_connection {
            connection.stop_with_leave(Some("对方已退出结算界面。".to_string()));
            connection.status = pvp::PvpStatus::Idle;
        }
    }

    next_phase.set(BattlePhase::Init);
    if !is_pvp {
        if let Some(map) = return_map {
            *current_map = map;
            next_game_state.set(GameState::Map);
            return;
        }
    }
    next_game_state.set(GameState::Lobby);
}

fn rematch_from_result(
    previous_battle_mode: BattleControlMode,
    selection_state: &mut crate::team_selection::SelectionState,
    entry_mode: &mut crate::team_selection::SelectionEntryMode,
    map_battle_context: &mut crate::data::MapBattleContext,
    team_selections: Option<&mut TeamSelections>,
    battle_mode: &mut BattleControlMode,
    ui_control_side: &mut crate::battle::UiControlSide,
    selected_cards: &mut crate::battle::SelectedCards,
    battle_result: &mut BattleResult,
    result_notice: &mut BattleResultNotice,
    next_phase: &mut NextState<BattlePhase>,
    next_game_state: &mut NextState<GameState>,
) {
    selection_state.reset();
    match previous_battle_mode {
        BattleControlMode::DebugPlayerControlsBoth => {
            *entry_mode = crate::team_selection::SelectionEntryMode::Debug;
            *battle_mode = BattleControlMode::DebugPlayerControlsBoth;
        }
        _ => {
            *entry_mode = crate::team_selection::SelectionEntryMode::VsAi;
            *battle_mode = BattleControlMode::PlayerVsAi;
        }
    }
    ui_control_side.0 = Side::Player;
    *selected_cards = crate::battle::SelectedCards::default();
    if let Some(team_selections) = team_selections {
        if map_battle_context.return_map.is_some() {
            map_battle_context.enemy_monster_index = team_selections.enemy_indices.first().copied();
        }
        team_selections.player_indices.clear();
        team_selections.enemy_indices.clear();
    }
    battle_result.message.clear();
    battle_result.export_status = None;
    result_notice.text.clear();
    result_notice.remaining = 0.0;
    next_phase.set(BattlePhase::Init);
    next_game_state.set(GameState::TeamSelection);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        battle::{
            ActionTraceEntry, BattleEvent, BattleFormulaEvent, BattleLifecycleEvent,
            BattlePerformanceReport, BattlePerformanceStats, BattleStateEvent, BattleStatusEvent,
            BattleTraceEvent, ElementAura, EnemyTeam, PlayerTeam, ReplayEventLog, Shield, Side,
            StatusBoard, StatusInstance, StructuredBattleLog, Team,
            systems::consume_battle_events_system,
        },
        data::{BattleFormulaRules, ElementType, StatusCategory, StatusTickTiming},
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

    fn spawn_test_combatant(app: &mut App, name: &str, side: Side, hp: i32) -> Entity {
        app.world_mut()
            .spawn((
                InBattle,
                Name::new(name.to_string()),
                Combatant {
                    side,
                    element: ElementType::Fire,
                },
                test_stats(hp),
                StatusBoard::default(),
            ))
            .id()
    }

    fn setup_resolve_ko_app(pending_ko: PendingKoResolution) -> App {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.init_resource::<Messages<BattleEvent>>();
        app.insert_resource(pending_ko);
        app.insert_resource(BattleControlMode::PlayerVsAi);
        app.insert_resource(BattlePerformanceStats::default());
        app.insert_resource(BattlePerformanceReport::default());
        app.insert_resource(TurnCount(5));
        app.insert_resource(TurnContext::default());
        app.insert_resource(RoundOrder::default());
        app.insert_resource(BattleLog::default());
        app.insert_resource(StructuredBattleLog::default());
        app.insert_resource(ActionTrace::default());
        app.insert_resource(BattleResult::default());
        app.insert_resource(NextState::<BattlePhase>::default());
        app.insert_resource(NextState::<GameState>::default());
        app.add_systems(Update, resolve_ko_system);
        app
    }

    #[test]
    fn enemy_ai_reward_keeps_terminal_outcome_dominant() {
        let mut stats = BattlePerformanceStats::default();
        stats.enemy.damage_dealt = 200;
        stats.enemy.knockouts = 2;
        stats.enemy.advanced_reactions = 3;
        let enemy = PerformanceTeamSnapshot {
            current_hp: 40,
            max_hp: 60,
            alive_count: 2,
            member_count: 3,
        };
        let player = PerformanceTeamSnapshot {
            current_hp: 5,
            max_hp: 60,
            alive_count: 1,
            member_count: 3,
        };

        let episode_stats = ai_episode_stats(&stats, player, enemy);
        let defeat_reward = reward_for_episode(AiBattleOutcome::EnemyDefeat, episode_stats);
        let victory_reward = reward_for_episode(AiBattleOutcome::EnemyVictory, episode_stats);

        assert!((-1.25..=-0.75).contains(&defeat_reward));
        assert!((0.75..=1.25).contains(&victory_reward));
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
    fn check_end_clears_stale_pending_ko_when_no_combatant_fainted() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.init_resource::<Messages<BattleEvent>>();
        app.init_resource::<Messages<BattleFormulaEvent>>();
        app.init_resource::<Messages<BattleStatusEvent>>();
        app.insert_resource(PendingKoResolution {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            player_switch_index: Some(1),
            enemy_switch_index: Some(1),
            player_defeated: true,
            enemy_defeated: true,
            resume_phase: Some(BattlePhase::PlayerTurn),
        });
        app.insert_resource(TurnCount(1));
        app.insert_resource(BattleFormulaRules::default());
        app.insert_resource(BattleLog::default());
        app.insert_resource(StructuredBattleLog::default());
        app.insert_resource(ActionTrace::default());
        app.insert_resource(BattleResult::default());
        app.insert_resource(NextState::<BattlePhase>::default());
        app.insert_resource(NextState::<GameState>::default());
        app.add_systems(Update, check_end_system);

        let player_active = spawn_test_combatant(&mut app, "Player A", Side::Player, 12);
        let enemy_active = spawn_test_combatant(&mut app, "Enemy A", Side::Enemy, 12);
        app.insert_resource(PlayerTeam(Team {
            combatants: vec![player_active],
            active_index: 0,
        }));
        app.insert_resource(EnemyTeam(Team {
            combatants: vec![enemy_active],
            active_index: 0,
        }));

        app.update();

        let pending_ko = app.world().resource::<PendingKoResolution>();
        assert_eq!(pending_ko.player_switch_index, None);
        assert_eq!(pending_ko.enemy_switch_index, None);
        assert!(!pending_ko.player_defeated);
        assert!(!pending_ko.enemy_defeated);
        assert_eq!(pending_ko.resume_phase, None);
        let next_phase = app.world().resource::<NextState<BattlePhase>>();
        assert!(matches!(
            next_phase,
            NextState::Pending(BattlePhase::RoundStart)
        ));
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
        let mut app = setup_resolve_ko_app(PendingKoResolution {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            player_switch_index: Some(1),
            enemy_switch_index: None,
            player_defeated: false,
            enemy_defeated: false,
            resume_phase: Some(BattlePhase::PlayerTurn),
        });

        let player_active = spawn_test_combatant(&mut app, "Player A", Side::Player, 0);
        let player_bench = spawn_test_combatant(&mut app, "Player B", Side::Player, 14);
        let enemy_active = spawn_test_combatant(&mut app, "Enemy A", Side::Enemy, 8);
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
            NextState::Pending(BattlePhase::PlayerTurn)
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
    fn resolve_ko_returns_to_enemy_turn_when_enemy_action_caused_ko() {
        let mut app = setup_resolve_ko_app(PendingKoResolution {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            player_switch_index: None,
            enemy_switch_index: Some(1),
            player_defeated: false,
            enemy_defeated: false,
            resume_phase: Some(BattlePhase::EnemyTurn),
        });

        let player_active = spawn_test_combatant(&mut app, "Player A", Side::Player, 12);
        let enemy_active = spawn_test_combatant(&mut app, "Enemy A", Side::Enemy, 0);
        let enemy_bench = spawn_test_combatant(&mut app, "Enemy B", Side::Enemy, 14);
        app.insert_resource(PlayerTeam(Team {
            combatants: vec![player_active],
            active_index: 0,
        }));
        app.insert_resource(EnemyTeam(Team {
            combatants: vec![enemy_active, enemy_bench],
            active_index: 0,
        }));

        app.update();

        let enemy_team = app.world().resource::<EnemyTeam>();
        assert_eq!(enemy_team.0.active_index, 1);
        let next_phase = app.world().resource::<NextState<BattlePhase>>();
        assert!(matches!(
            next_phase,
            NextState::Pending(BattlePhase::EnemyTurn)
        ));
    }

    #[test]
    fn resolve_ko_enters_result_when_defeated_even_with_resume_phase() {
        let mut app = setup_resolve_ko_app(PendingKoResolution {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            player_switch_index: None,
            enemy_switch_index: None,
            player_defeated: false,
            enemy_defeated: true,
            resume_phase: Some(BattlePhase::PlayerTurn),
        });
        let player_active = spawn_test_combatant(&mut app, "Player A", Side::Player, 12);
        let enemy_active = spawn_test_combatant(&mut app, "Enemy A", Side::Enemy, 0);
        app.insert_resource(PlayerTeam(Team {
            combatants: vec![player_active],
            active_index: 0,
        }));
        app.insert_resource(EnemyTeam(Team {
            combatants: vec![enemy_active],
            active_index: 0,
        }));

        app.update();

        let next_game_state = app.world().resource::<NextState<GameState>>();
        assert!(matches!(
            next_game_state,
            NextState::Pending(GameState::Result)
        ));
        let pending_ko = app.world().resource::<PendingKoResolution>();
        assert_eq!(pending_ko.resume_phase, None);
    }

    #[test]
    fn resolve_ko_with_no_resume_continues_to_second_side() {
        let mut app = setup_resolve_ko_app(PendingKoResolution {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            player_switch_index: Some(1),
            enemy_switch_index: None,
            player_defeated: false,
            enemy_defeated: false,
            resume_phase: None,
        });
        app.world_mut().resource_mut::<TurnContext>().player_ended = true;
        let player_active = spawn_test_combatant(&mut app, "Player A", Side::Player, 0);
        let player_bench = spawn_test_combatant(&mut app, "Player B", Side::Player, 14);
        let enemy_active = spawn_test_combatant(&mut app, "Enemy A", Side::Enemy, 12);
        app.insert_resource(PlayerTeam(Team {
            combatants: vec![player_active, player_bench],
            active_index: 0,
        }));
        app.insert_resource(EnemyTeam(Team {
            combatants: vec![enemy_active],
            active_index: 0,
        }));

        app.update();

        let next_phase = app.world().resource::<NextState<BattlePhase>>();
        assert!(matches!(
            next_phase,
            NextState::Pending(BattlePhase::EnemyTurn)
        ));
    }

    #[test]
    fn resolve_ko_with_no_resume_and_both_ended_goes_check_end() {
        let mut app = setup_resolve_ko_app(PendingKoResolution {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            player_switch_index: Some(1),
            enemy_switch_index: None,
            player_defeated: false,
            enemy_defeated: false,
            resume_phase: None,
        });
        {
            let mut turn_ctx = app.world_mut().resource_mut::<TurnContext>();
            turn_ctx.player_ended = true;
            turn_ctx.enemy_ended = true;
        }
        let player_active = spawn_test_combatant(&mut app, "Player A", Side::Player, 0);
        let player_bench = spawn_test_combatant(&mut app, "Player B", Side::Player, 14);
        let enemy_active = spawn_test_combatant(&mut app, "Enemy A", Side::Enemy, 12);
        app.insert_resource(PlayerTeam(Team {
            combatants: vec![player_active, player_bench],
            active_index: 0,
        }));
        app.insert_resource(EnemyTeam(Team {
            combatants: vec![enemy_active],
            active_index: 0,
        }));

        app.update();

        let next_phase = app.world().resource::<NextState<BattlePhase>>();
        assert!(matches!(
            next_phase,
            NextState::Pending(BattlePhase::CheckEnd)
        ));
    }

    #[test]
    fn simultaneous_nonterminal_ko_auto_switches_both_and_resumes_actor() {
        let mut app = setup_resolve_ko_app(PendingKoResolution {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            player_switch_index: Some(1),
            enemy_switch_index: Some(1),
            player_defeated: false,
            enemy_defeated: false,
            resume_phase: Some(BattlePhase::PlayerTurn),
        });
        let player_active = spawn_test_combatant(&mut app, "Player A", Side::Player, 0);
        let player_bench = spawn_test_combatant(&mut app, "Player B", Side::Player, 14);
        let enemy_active = spawn_test_combatant(&mut app, "Enemy A", Side::Enemy, 0);
        let enemy_bench = spawn_test_combatant(&mut app, "Enemy B", Side::Enemy, 14);
        app.insert_resource(PlayerTeam(Team {
            combatants: vec![player_active, player_bench],
            active_index: 0,
        }));
        app.insert_resource(EnemyTeam(Team {
            combatants: vec![enemy_active, enemy_bench],
            active_index: 0,
        }));

        app.update();

        assert_eq!(app.world().resource::<PlayerTeam>().0.active_index, 1);
        assert_eq!(app.world().resource::<EnemyTeam>().0.active_index, 1);
        let next_phase = app.world().resource::<NextState<BattlePhase>>();
        assert!(matches!(
            next_phase,
            NextState::Pending(BattlePhase::PlayerTurn)
        ));
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

        let export_message = write_export_logs(
            &export_dir,
            &battle_result,
            &replay_log,
            &action_trace,
            None,
        )
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
                skill_id: crate::data::SkillId::FirePunch,
                skill_name: "火拳".to_string(),
                slot: 0,
                cost_ap: 2,
            });
        app.world_mut()
            .resource_mut::<Messages<BattleEvent>>()
            .write(BattleEvent::DamageDealt {
                source: Side::Player,
                target: Side::Enemy,
                amount: 7,
                damage_type: crate::battle::DamageType::Direct,
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
        assert_eq!(replay_log.0[1].detail, "玩家 对 敌方 造成了 7 点直接伤害。");
        assert_eq!(replay_log.0[2].phase, "trace-r3");
        assert_eq!(replay_log.0[2].summary, "skill:FirePunch");
        assert_eq!(replay_log.0[3].phase, "battle-result");
        assert_eq!(replay_log.0[3].summary, "战斗结束");
        assert_eq!(replay_log.0[4].phase, "formula-r3");
        assert_eq!(replay_log.0[4].summary, "damage_formula");
        assert_eq!(replay_log.0[5].phase, "status-r3");
        assert_eq!(replay_log.0[5].summary, "burning_aura:applied");

        let export_message =
            write_export_logs(&export_dir, battle_result, replay_log, &action_trace, None)
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
        assert!(replay_text.contains("detail: \"玩家 对 敌方 造成了 7 点直接伤害。\""));
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

    #[test]
    fn exported_full_round_logs_preserve_player_enemy_action_order() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let export_dir =
            std::env::temp_dir().join(format!("bevy_mon_proto_round_sequence_export_{unique}"));

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
        app.insert_resource(TurnCount(2));
        app.insert_resource(State::new(GameState::Battle));
        app.add_systems(Update, consume_battle_events_system);

        app.world_mut()
            .resource_mut::<Messages<BattleTraceEvent>>()
            .write(BattleTraceEvent {
                round: 2,
                side: Side::Player,
                action: "player_skill:FirePunch".to_string(),
                detail: "玩家在第2回合行动。".to_string(),
            });
        app.world_mut()
            .resource_mut::<Messages<BattleTraceEvent>>()
            .write(BattleTraceEvent {
                round: 2,
                side: Side::Enemy,
                action: "enemy_skill:WaterShot".to_string(),
                detail: "敌方在第2回合行动。".to_string(),
            });
        app.world_mut()
            .resource_mut::<Messages<BattleLifecycleEvent>>()
            .write(BattleLifecycleEvent {
                phase: "round-end-r2".to_string(),
                summary: "完整回合结束".to_string(),
                detail: "玩家与敌方均已行动。".to_string(),
            });

        app.update();

        let replay_log = app.world().resource::<ReplayEventLog>();
        assert_eq!(replay_log.0.len(), 3);
        assert_eq!(replay_log.0[0].seq, 1);
        assert_eq!(replay_log.0[0].phase, "trace-r2");
        assert_eq!(replay_log.0[0].summary, "player_skill:FirePunch");
        assert_eq!(replay_log.0[1].seq, 2);
        assert_eq!(replay_log.0[1].phase, "trace-r2");
        assert_eq!(replay_log.0[1].summary, "enemy_skill:WaterShot");
        assert_eq!(replay_log.0[2].seq, 3);
        assert_eq!(replay_log.0[2].phase, "round-end-r2");
        assert_eq!(replay_log.0[2].summary, "完整回合结束");

        let action_trace = ActionTrace(vec![
            ActionTraceEntry {
                seq: 1,
                round: 2,
                side: Side::Player,
                action: "player_skill:FirePunch".to_string(),
                detail: "玩家在第2回合行动。".to_string(),
            },
            ActionTraceEntry {
                seq: 2,
                round: 2,
                side: Side::Enemy,
                action: "enemy_skill:WaterShot".to_string(),
                detail: "敌方在第2回合行动。".to_string(),
            },
        ]);
        let battle_result = app.world().resource::<BattleResult>();
        write_export_logs(&export_dir, battle_result, replay_log, &action_trace, None)
            .expect("export should succeed");

        let slug = sanitize_filename_segment(&battle_result.message);
        let replay_text = fs::read_to_string(export_dir.join(format!("{slug}_replay.ron")))
            .expect("read replay export");
        let action_text = fs::read_to_string(export_dir.join(format!("{slug}_action_trace.ron")))
            .expect("read action export");

        assert!(replay_text.contains("summary: \"player_skill:FirePunch\""));
        assert!(replay_text.contains("summary: \"enemy_skill:WaterShot\""));
        assert!(replay_text.contains("summary: \"完整回合结束\""));
        assert!(action_text.contains("seq: 1"));
        assert!(action_text.contains("side: Player"));
        assert!(action_text.contains("seq: 2"));
        assert!(action_text.contains("side: Enemy"));

        let _ = fs::remove_dir_all(&export_dir);
    }
}
