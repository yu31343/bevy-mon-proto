use bevy::prelude::*;

use crate::{
    battle::{push_battle_line, BattleEvent, BattleLog, BattleResult},
    game_state::GameState,
};

use super::{element_text, side_text};

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
                let from_str = from.map(element_text).unwrap_or("无");
                format!(
                    "{} 元素附着：{} -> {}（克制倍率 {}）。",
                    side_text(*side),
                    from_str,
                    element_text(*to),
                    effectiveness
                )
            }
            BattleEvent::SkillUsed {
                side, skill_name, ..
            } => {
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
            BattleEvent::Healed { side, amount } => {
                format!("{} 恢复了 {} 点生命。", side_text(*side), amount)
            }
            BattleEvent::ShieldGained { side, amount } => {
                format!("{} 获得了 {} 点护盾。", side_text(*side), amount)
            }
            BattleEvent::CombatantFainted { side, name } => {
                format!("【{}】{} 倒下了。", side_text(*side), name)
            }
            BattleEvent::Switched { side, name } => {
                format!("{} 换上了 {}！", side_text(*side), name)
            }
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
