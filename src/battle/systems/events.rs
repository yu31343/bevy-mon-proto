use bevy::prelude::*;

use super::{element_text, side_text};
use crate::{
    battle::{
        BattleEvent, BattleFormulaEvent, BattleLifecycleEvent, BattleLog, BattleResult,
        BattleStateEvent, BattleStatusEvent, BattleTraceEvent, Combatant, ElementAura, EnemyTeam,
        InBattle, PlayerTeam, ReplayEventLog, Shield, Side, Stats, StatusBoard,
        StructuredBattleLog, TurnCount, push_battle_line, push_replay_log_entry,
        push_structured_battle_line,
    },
    game_state::GameState,
};

pub fn consume_battle_events_system(
    mut events: MessageReader<BattleEvent>,
    mut trace_events: MessageReader<BattleTraceEvent>,
    mut state_events: MessageReader<BattleStateEvent>,
    mut lifecycle_events: MessageReader<BattleLifecycleEvent>,
    mut formula_events: MessageReader<BattleFormulaEvent>,
    mut status_events: MessageReader<BattleStatusEvent>,
    mut log: ResMut<BattleLog>,
    mut structured_log: ResMut<StructuredBattleLog>,
    mut replay_log: ResMut<ReplayEventLog>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<
        (
            &Combatant,
            &Stats,
            &Name,
            &Shield,
            &ElementAura,
            &StatusBoard,
        ),
        With<InBattle>,
    >,
    turn_count: Res<TurnCount>,
    state: Res<State<GameState>>,
    result: Res<BattleResult>,
    mut last_printed_result: Local<Option<String>>,
) {
    fn snapshot_side(
        team: &crate::battle::Team,
        side: Side,
        query: &Query<
            (
                &Combatant,
                &Stats,
                &Name,
                &Shield,
                &ElementAura,
                &StatusBoard,
            ),
            With<InBattle>,
        >,
    ) -> String {
        let mut lines = Vec::new();
        for (index, entity) in team.combatants.iter().copied().enumerate() {
            if let Ok((_combatant, stats, name, shield, aura, statuses)) = query.get(entity) {
                let active = if index == team.active_index {
                    "前场"
                } else {
                    "后场"
                };
                lines.push(format!(
                    "{}#{} {} | HP {}/{} | 护盾 {} | 附着 {} | 状态 {} | 属性等级 Atk{:+}/Def{:+}/Spd{:+}/Acc{:+}",
                    active,
                    index + 1,
                    name,
                    stats.hp.max(0),
                    stats.max_hp,
                    shield.0.max(0),
                    super::text::element_texts(&aura.elements()),
                    super::text::status_names(statuses),
                    stats.atk_stage,
                    stats.def_stage,
                    stats.spd_stage,
                    stats.acc_stage,
                ));
            }
        }
        if lines.is_empty() {
            format!("{:?} 无可用成员", side)
        } else {
            lines.join(" || ")
        }
    }

    fn should_emit_snapshot(event: &BattleEvent) -> bool {
        matches!(
            event,
            BattleEvent::TurnStarted(_)
                | BattleEvent::SkillUsed { .. }
                | BattleEvent::CardUsed { .. }
                | BattleEvent::CardDiscarded { .. }
                | BattleEvent::DamageDealt { .. }
                | BattleEvent::Healed { .. }
                | BattleEvent::ShieldGained { .. }
                | BattleEvent::ElementAuraApplied { .. }
                | BattleEvent::ReactionTriggered { .. }
                | BattleEvent::WindSpreadTriggered { .. }
                | BattleEvent::WindSpreadSkipped { .. }
                | BattleEvent::CombatantFainted { .. }
                | BattleEvent::Switched { .. }
        )
    }

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
                let from_str = if from.is_empty() {
                    "无".to_string()
                } else {
                    from.iter()
                        .map(|element| element_text(*element))
                        .collect::<Vec<_>>()
                        .join("/")
                };
                let to_str = if to.is_empty() {
                    "无".to_string()
                } else {
                    to.iter()
                        .map(|element| element_text(*element))
                        .collect::<Vec<_>>()
                        .join("/")
                };
                format!(
                    "{} 元素附着：{} -> {}（克制倍率 {}）。",
                    side_text(*side),
                    from_str,
                    to_str,
                    effectiveness
                )
            }
            BattleEvent::SkillUsed {
                side, skill_name, ..
            } => {
                format!("{} 使用了 {}。", side_text(*side), skill_name)
            }
            BattleEvent::ReactionTriggered {
                source,
                target,
                reaction_name,
            } => format!(
                "{} 对 {} 触发了元素反应：{}。",
                side_text(*source),
                side_text(*target),
                reaction_name
            ),
            BattleEvent::WindSpreadTriggered {
                source,
                target,
                element,
            } => format!(
                "{} 对 {} 触发了风扩散：{}。",
                side_text(*source),
                side_text(*target),
                element_text(*element)
            ),
            BattleEvent::WindSpreadSkipped {
                source,
                target,
                aura,
            } => {
                let aura_text = if aura.is_empty() {
                    "无".to_string()
                } else {
                    aura.iter()
                        .map(|element| element_text(*element))
                        .collect::<Vec<_>>()
                        .join("/")
                };
                format!(
                    "{} 对 {} 未触发风扩散（当前附着：{}）。",
                    side_text(*source),
                    side_text(*target),
                    aura_text
                )
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
            BattleEvent::AttackMissed { source, target } => format!(
                "{} 对 {} 的攻击未命中。",
                side_text(*source),
                side_text(*target)
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
            BattleEvent::CombatantFainted { side, name, .. } => {
                format!("【{}】{} 倒下了。", side_text(*side), name)
            }
            BattleEvent::Switched { side, name } => {
                format!("{} 换上了 {}！", side_text(*side), name)
            }
        };

        push_replay_log_entry(&mut replay_log, "battle-event", "BattleEvent", line.clone());
        push_battle_line(&mut log, line);

        if should_emit_snapshot(event) {
            if let (Some(player_team), Some(enemy_team)) =
                (player_team.as_ref(), enemy_team.as_ref())
            {
                push_structured_battle_line(
                    &mut structured_log,
                    format!("state-r{}", turn_count.0),
                    "双方状态快照",
                    format!(
                        "我方 => {}\n敌方 => {}",
                        snapshot_side(&player_team.0, Side::Player, &combat_query),
                        snapshot_side(&enemy_team.0, Side::Enemy, &combat_query)
                    ),
                );
            }
        }
    }

    for event in trace_events.read() {
        let phase = format!("trace-r{}", event.round);
        println!("[{}] {}\n{}", phase, event.action, event.detail);
        push_replay_log_entry(
            &mut replay_log,
            phase.clone(),
            event.action.clone(),
            event.detail.clone(),
        );
        push_structured_battle_line(
            &mut structured_log,
            phase,
            event.action.clone(),
            event.detail.clone(),
        );
    }

    for event in state_events.read() {
        let phase = format!("state-r{}", event.round);
        println!("[{}] {}\n{}", phase, event.summary, event.detail);
        push_replay_log_entry(
            &mut replay_log,
            phase.clone(),
            event.summary.clone(),
            event.detail.clone(),
        );
        push_structured_battle_line(
            &mut structured_log,
            phase,
            event.summary.clone(),
            event.detail.clone(),
        );
    }

    for event in lifecycle_events.read() {
        println!("[{}] {}\n{}", event.phase, event.summary, event.detail);
        push_replay_log_entry(
            &mut replay_log,
            event.phase.clone(),
            event.summary.clone(),
            event.detail.clone(),
        );
        push_structured_battle_line(
            &mut structured_log,
            event.phase.clone(),
            event.summary.clone(),
            event.detail.clone(),
        );
    }

    for event in formula_events.read() {
        let phase = format!("formula-r{}", event.round);
        println!("[{}] {}\n{}", phase, event.action, event.detail);
        push_replay_log_entry(
            &mut replay_log,
            phase.clone(),
            event.action.clone(),
            event.detail.clone(),
        );
        push_structured_battle_line(
            &mut structured_log,
            phase,
            event.action.clone(),
            event.detail.clone(),
        );
    }

    for event in status_events.read() {
        let summary = format!("{}:{}", event.status_id, event.action);
        let phase = format!("status-r{}", event.round);
        println!("[{}] {}\n{}", phase, summary, event.detail);
        push_replay_log_entry(
            &mut replay_log,
            phase.clone(),
            summary.clone(),
            event.detail.clone(),
        );
        push_structured_battle_line(&mut structured_log, phase, summary, event.detail.clone());
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
