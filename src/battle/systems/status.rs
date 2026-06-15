use bevy::prelude::*;

use crate::{
    battle::{
        BattleEvent, BattleFormulaEvent, BattleStatusEvent, ElementAura, PendingBoosts, Shield,
        Side, Stats, StatusBoard, StructuredBattleLog, decrement_status_durations_for_round,
        note_action_phase, status_event, tick_statuses_for_timing,
    },
    data::{BattleFormulaRules, ElementType, StatusTickTiming},
};

pub(crate) struct SideEndTickParams<'a, 'event, 'formula, 'status> {
    pub side: Side,
    pub round: u32,
    pub formula_rules: &'a BattleFormulaRules,
    pub pending_boosts: Option<&'a mut PendingBoosts>,
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

fn take_next_heal_bonus(side: Side, pending_boosts: &mut PendingBoosts) -> i32 {
    let pending = match side {
        Side::Player => &mut pending_boosts.player,
        Side::Enemy => &mut pending_boosts.enemy,
    };
    let bonus = pending.next_heal_bonus;
    pending.next_heal_bonus = 0;
    bonus
}

fn apply_status_tick_heal(
    stats: &mut Stats,
    raw_heal_amount: i32,
    heal_multiplier: f32,
) -> Option<(i32, i32)> {
    if raw_heal_amount <= 0 || stats.hp <= 0 {
        return None;
    }

    let heal_amount = ((raw_heal_amount as f32) * heal_multiplier).round() as i32;
    let before = stats.hp;
    stats.hp = (stats.hp + heal_amount).min(stats.max_hp);
    Some((heal_amount, stats.hp - before))
}

pub(crate) fn process_side_end_statuses(
    stats: &mut Stats,
    shield: &mut Shield,
    _aura: &mut ElementAura,
    status_board: &mut StatusBoard,
    mut params: SideEndTickParams<'_, '_, '_, '_>,
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
            let heal_bonus = params
                .pending_boosts
                .as_deref_mut()
                .map(|pending_boosts| take_next_heal_bonus(params.side, pending_boosts))
                .unwrap_or(0);
            let raw_heal_amount = outcome.heal_amount + heal_bonus;
            let heal_multiplier = status_board.heal_taken_multiplier();
            if let Some((heal_amount, actual_heal)) =
                apply_status_tick_heal(stats, raw_heal_amount, heal_multiplier)
            {
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
                        "status_id={} raw_heal={} heal_multiplier={:.2} final_heal={} actual_heal={} target_hp={}",
                        outcome.status_id,
                        raw_heal_amount,
                        heal_multiplier,
                        heal_amount,
                        actual_heal,
                        stats.hp
                    ),
                ));
                note_action_phase(
                    params.structured_log,
                    params.round,
                    params.side,
                    "状态触发",
                    format!(
                        "状态={} 触发治疗={}；治疗修正={:.2}；最终治疗={}；实际回复={}；当前HP={}",
                        outcome.status_name,
                        raw_heal_amount,
                        heal_multiplier,
                        heal_amount,
                        actual_heal,
                        stats.hp
                    ),
                );
                params.status_writer.write(status_event(
                    params.round,
                    params.side,
                    outcome.status_id.clone(),
                    "triggered",
                    format!(
                        "raw_heal={} heal_multiplier={:.2} final_heal={} actual_heal={} remaining_turns={}",
                        raw_heal_amount,
                        heal_multiplier,
                        heal_amount,
                        actual_heal,
                        outcome.remaining_turns
                    ),
                ));
            }
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
            if outcome.expired {
                "expired"
            } else {
                "duration_ticked"
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_stats(hp: i32) -> Stats {
        Stats {
            hp,
            max_hp: 30,
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
    fn status_tick_heal_does_not_revive_fainted_combatant() {
        let mut stats = test_stats(0);

        let heal_result = apply_status_tick_heal(&mut stats, 6, 0.85);

        assert_eq!(heal_result, None);
        assert_eq!(stats.hp, 0);
    }

    #[test]
    fn status_tick_heal_still_heals_living_combatant() {
        let mut stats = test_stats(10);

        let heal_result = apply_status_tick_heal(&mut stats, 6, 1.0);

        assert_eq!(heal_result, Some((6, 6)));
        assert_eq!(stats.hp, 16);
    }

    #[test]
    fn status_tick_heal_uses_heal_taken_multiplier() {
        let mut stats = test_stats(10);

        let heal_result = apply_status_tick_heal(&mut stats, 20, 0.85);

        assert_eq!(heal_result, Some((17, 17)));
        assert_eq!(stats.hp, 27);
    }
}
