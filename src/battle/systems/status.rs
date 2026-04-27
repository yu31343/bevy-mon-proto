use bevy::prelude::*;

use crate::{
    battle::{
        BattleEvent, BattleFormulaEvent, BattleStatusEvent, ElementAura, Shield, Side, Stats,
        StatusBoard, StructuredBattleLog, decrement_status_durations_for_round,
        note_action_phase, status_event, tick_statuses_for_timing,
    },
    data::{BattleFormulaRules, ElementType, StatusTickTiming},
};

pub(crate) struct SideEndTickParams<'a, 'event, 'formula, 'status> {
    pub side: Side,
    pub round: u32,
    pub formula_rules: &'a BattleFormulaRules,
    pub event_writer: &'a mut MessageWriter<'event, BattleEvent>,
    pub formula_writer: &'a mut MessageWriter<'formula, BattleFormulaEvent>,
    pub status_writer: &'a mut MessageWriter<'status, BattleStatusEvent>,
    pub structured_log: &'a mut StructuredBattleLog,
}

fn aura_status_element(status_id: &str) -> Option<ElementType> {
    match status_id {
        "burning_aura" => Some(ElementType::Fire),
        "wet_aura" => Some(ElementType::Water),
        "thorn_aura" => Some(ElementType::Grass),
        "paralysis_aura" => Some(ElementType::Thunder),
        _ => None,
    }
}

pub(crate) fn process_side_end_statuses(
    stats: &mut Stats,
    shield: &mut Shield,
    _aura: &mut ElementAura,
    status_board: &mut StatusBoard,
    params: SideEndTickParams<'_, '_, '_, '_>,
) {
    let outcomes = tick_statuses_for_timing(status_board, StatusTickTiming::OwnerActionEnd);
    for outcome in outcomes {
        if outcome.fixed_damage > 0 {
            let absorbed = shield.0.min(outcome.fixed_damage);
            if absorbed > 0 {
                shield.0 -= absorbed;
                params.event_writer.write(BattleEvent::ShieldAbsorbed {
                    side: params.side,
                    amount: absorbed,
                });
            }
            let hp_damage = (outcome.fixed_damage - absorbed).max(0);
            stats.hp = (stats.hp - hp_damage).max(0);
            params.event_writer.write(BattleEvent::DamageDealt {
                source: params.side,
                target: params.side,
                amount: hp_damage,
            });
            params.formula_writer.write(crate::battle::formula_event(
                params.round,
                params.side,
                params.side,
                "status_tick_damage",
                format!(
                    "status_id={} fixed_damage={} absorbed={} hp_damage={} target_hp={} target_shield={} min_damage_rule={}",
                    outcome.status_id,
                    outcome.fixed_damage,
                    absorbed,
                    hp_damage,
                    stats.hp,
                    shield.0,
                    params.formula_rules.damage.min_damage
                ),
            ));
            note_action_phase(
                params.structured_log,
                params.round,
                params.side,
                "状态触发",
                format!(
                    "状态={} 触发固定伤害={}；护盾吸收={}；生命伤害={}；当前HP={}；当前护盾={}",
                    outcome.status_name,
                    outcome.fixed_damage,
                    absorbed,
                    hp_damage,
                    stats.hp,
                    shield.0
                ),
            );
            params.status_writer.write(status_event(
                params.round,
                params.side,
                outcome.status_id.clone(),
                "triggered",
                format!(
                    "fixed_damage={} absorbed={} hp_damage={} remaining_turns={}",
                    outcome.fixed_damage, absorbed, hp_damage, outcome.remaining_turns
                ),
            ));
        }

        if outcome.heal_amount > 0 {
            let before = stats.hp;
            stats.hp = (stats.hp + outcome.heal_amount).min(stats.max_hp);
            let actual_heal = stats.hp - before;
            params.event_writer.write(BattleEvent::Healed {
                side: params.side,
                amount: actual_heal,
            });
            params.formula_writer.write(crate::battle::formula_event(
                params.round,
                params.side,
                params.side,
                "status_tick_heal",
                format!(
                    "status_id={} heal_amount={} actual_heal={} target_hp={}",
                    outcome.status_id, outcome.heal_amount, actual_heal, stats.hp
                ),
            ));
            note_action_phase(
                params.structured_log,
                params.round,
                params.side,
                "状态触发",
                format!(
                    "状态={} 触发治疗={}；实际回复={}；当前HP={}",
                    outcome.status_name, outcome.heal_amount, actual_heal, stats.hp
                ),
            );
            params.status_writer.write(status_event(
                params.round,
                params.side,
                outcome.status_id.clone(),
                "triggered",
                format!(
                    "heal_amount={} actual_heal={} remaining_turns={}",
                    outcome.heal_amount, actual_heal, outcome.remaining_turns
                ),
            ));
        }

        params.status_writer.write(status_event(
            params.round,
            params.side,
            outcome.status_id.clone(),
            "triggered_timing",
            format!(
                "tick_timing={:?} remaining_turns={} status_name={}",
                StatusTickTiming::OwnerActionEnd,
                outcome.remaining_turns,
                outcome.status_name
            ),
        ));
    }
}

pub(crate) fn process_round_end_status_durations(
    stats: &mut Stats,
    aura: &mut ElementAura,
    status_board: &mut StatusBoard,
    params: SideEndTickParams<'_, '_, '_, '_>,
) {
    let outcomes = decrement_status_durations_for_round(status_board, stats, params.round);
    for outcome in outcomes {
        params.status_writer.write(status_event(
            params.round,
            params.side,
            outcome.status_id.clone(),
            if outcome.expired { "expired" } else { "duration_ticked" },
            format!(
                "remaining_turns={} status_name={}",
                outcome.remaining_turns, outcome.status_name
            ),
        ));

        if outcome.expired {
            if let Some(element) = aura_status_element(&outcome.status_id) {
                if aura.remove(element) {
                    params.status_writer.write(status_event(
                        params.round,
                        params.side,
                        "element_aura",
                        "cleared",
                        format!(
                            "expired_status_id={} aura_cleared_element={:?}",
                            outcome.status_id, element
                        ),
                    ));
                }
            }
            note_action_phase(
                params.structured_log,
                params.round,
                params.side,
                "状态移除",
                format!("状态={} 已到期移除", outcome.status_name),
            );
        } else {
            note_action_phase(
                params.structured_log,
                params.round,
                params.side,
                "状态扣减",
                format!(
                    "状态={} 剩余持续={}回合",
                    outcome.status_name, outcome.remaining_turns
                ),
            );
        }
    }
}
