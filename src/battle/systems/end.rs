use bevy::prelude::*;

use crate::{
    battle::{push_battle_line, BattleEvent, BattleLog, BattleResult, Combatant, InBattle, Side, Stats},
    game_state::{BattlePhase, GameState},
};

use super::abort_battle;

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

pub fn restart_from_result_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection_state: ResMut<crate::team_selection::SelectionState>,
    team_selections: Option<ResMut<crate::data::TeamSelections>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        // Clear previous selection
        selection_state.selected_indices.clear();
        if let Some(mut team_selections) = team_selections {
            team_selections.player_indices.clear();
            team_selections.enemy_indices.clear();
        }

        // Return to team selection instead of battle
        next_phase.set(BattlePhase::Init);
        next_game_state.set(GameState::TeamSelection);
    }
}
