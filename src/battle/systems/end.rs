use std::{fs, path::PathBuf};

use bevy::prelude::*;

use crate::{
    battle::{
        ActionTrace, BattleEvent, BattleLog, BattleResult, Combatant, InBattle,
        PendingKoResolution, ReplayEventLog, Side, Stats, StructuredBattleLog, TurnCount,
        note_action_phase, note_structured_phase, push_battle_line, push_named_action_trace,
    },
    game_state::{BattlePhase, GameState},
};

use super::abort_battle;

pub fn check_end_system(
    query: Query<(&Combatant, &Stats, &Name), With<InBattle>>,
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
            owner: p_entity,
            side: Side::Player,
            name: p_name.to_string(),
        });
        note_action_phase(
            &mut structured_log,
            turn_count.0,
            Side::Player,
            "玩家倒下",
            format!("{} 倒下；当前HP={}", p_name, p_stats.hp),
        );
        push_named_action_trace(
            &mut action_trace,
            turn_count.0,
            Side::Player,
            "fainted",
            format!("{} 倒下；当前HP={}", p_name, p_stats.hp),
        );

        let mut next_idx = None;
        for (i, &e) in player_team.0.combatants.iter().enumerate() {
            if let Ok((_, s, _)) = query.get(e) {
                if s.hp > 0 {
                    next_idx = Some(i);
                    break;
                }
            }
        }

        pending_ko.player_switch_index = next_idx;
        p_dead = next_idx.is_none();
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
            owner: e_entity,
            side: Side::Enemy,
            name: e_name.to_string(),
        });
        note_action_phase(
            &mut structured_log,
            turn_count.0,
            Side::Enemy,
            "敌方倒下",
            format!("{} 倒下；当前HP={}", e_name, e_stats.hp),
        );
        push_named_action_trace(
            &mut action_trace,
            turn_count.0,
            Side::Enemy,
            "fainted",
            format!("{} 倒下；当前HP={}", e_name, e_stats.hp),
        );

        let mut next_idx = None;
        for (i, &e) in enemy_team.0.combatants.iter().enumerate() {
            if let Ok((_, s, _)) = query.get(e) {
                if s.hp > 0 {
                    next_idx = Some(i);
                    break;
                }
            }
        }

        pending_ko.enemy_switch_index = next_idx;
        e_dead = next_idx.is_none();
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
        next_phase.set(BattlePhase::RoundStart);
    }
}

pub fn resolve_ko_system(
    time: Res<Time>,
    query: Query<(&Combatant, &Stats, &Name), With<InBattle>>,
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
        player_team.0.active_index = idx;
        let new_e = player_team.0.combatants[idx];
        if let Ok((_, stats, new_n)) = query.get(new_e) {
            if stats.hp > 0 {
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
        enemy_team.0.active_index = idx;
        let new_e = enemy_team.0.combatants[idx];
        if let Ok((_, stats, new_n)) = query.get(new_e) {
            if stats.hp > 0 {
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
            "平局！按 R 重新开始。".to_string()
        } else if pending_ko.enemy_defeated {
            "胜利！全歼敌方。按 R 重新开始。".to_string()
        } else {
            "失败！队伍全灭。按 R 重新开始。".to_string()
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

fn export_logs(
    battle_result: &BattleResult,
    replay_log: &ReplayEventLog,
    action_trace: &ActionTrace,
) -> Result<String, String> {
    let export_dir = PathBuf::from("battle_logs");
    fs::create_dir_all(&export_dir).map_err(|err| format!("创建导出目录失败：{err}"))?;

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

pub fn restart_from_result_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection_state: ResMut<crate::team_selection::SelectionState>,
    team_selections: Option<ResMut<crate::data::TeamSelections>>,
    replay_log: Res<ReplayEventLog>,
    action_trace: Res<ActionTrace>,
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
        selection_state.selected_indices.clear();
        if let Some(mut team_selections) = team_selections {
            team_selections.player_indices.clear();
            team_selections.enemy_indices.clear();
        }

        next_phase.set(BattlePhase::Init);
        next_game_state.set(GameState::TeamSelection);
    }
}
