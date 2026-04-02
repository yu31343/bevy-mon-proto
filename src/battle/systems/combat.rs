use bevy::prelude::*;

use crate::{
    battle::{BattleEvent, ElementAura, PendingBoosts, Shield, Side, Stats},
    data::{ElementDb, SkillEffect},
};

pub(crate) fn monster_skill_ap_cost(slot: usize) -> i32 {
    match slot {
        0 => 2, // 普通攻击
        1 => 3, // 元素战技
        2 => 1, // 防御（护盾）
        3 => 1, // 治疗
        _ => 999,
    }
}

/// 执行单个技能效果：攻击/治疗/护盾，支持元素克制。
pub(crate) fn apply_effect(
    effect: &SkillEffect,
    skill_element: Option<crate::data::ElementType>,
    attacker_side: Side,
    target_side: Side,
    target_element: crate::data::ElementType,
    attacker_stats: &mut Stats,
    attacker_shield: &mut Shield,
    target_stats: &mut Stats,
    target_shield: &mut Shield,
    target_aura: &mut ElementAura,
    pending_boosts: &mut PendingBoosts,
    element_db: &ElementDb,
    event_writer: &mut MessageWriter<BattleEvent>,
) {
    match effect {
        SkillEffect::Attack { power } => {
            let mut effective_power = *power;
            // 待命增益只作用于”对应类型的精灵技能”，并在结算后清空。
            if attacker_side == Side::Player {
                effective_power += pending_boosts.player.next_attack_bonus;
                pending_boosts.player.next_attack_bonus = 0;
            } else {
                effective_power += pending_boosts.enemy.next_attack_bonus;
                pending_boosts.enemy.next_attack_bonus = 0;
            }

            let raw = effective_power + attacker_stats.atk - target_stats.def;

            if let Some(incoming_element) = skill_element {
                // 先用”附着元素（若存在）”计算克制倍率。
                let defender_elem_with_aura = target_aura.attached.unwrap_or(target_element);
                let effectiveness_with_aura = element_db.get_effectiveness(
                    incoming_element,
                    defender_elem_with_aura,
                );
                let theoretical_damage_with_aura =
                    (raw as f32 * effectiveness_with_aura).max(1.0) as i32;
                let absorbed_with_aura = target_shield.0.min(theoretical_damage_with_aura);

                if absorbed_with_aura > 0 {
                    // 盾免疫：元素附着/反应不生效，改用”固有元素”重算伤害。
                    let effectiveness_no_aura = element_db.get_effectiveness(
                        incoming_element,
                        target_element,
                    );
                    let theoretical_damage_no_aura =
                        (raw as f32 * effectiveness_no_aura).max(1.0) as i32;
                    let absorbed = target_shield.0.min(theoretical_damage_no_aura);

                    if absorbed > 0 {
                        target_shield.0 -= absorbed;
                        event_writer.write(BattleEvent::ShieldAbsorbed {
                            side: target_side,
                            amount: absorbed,
                        });
                    }

                    let hp_damage = (theoretical_damage_no_aura - absorbed).max(0);
                    if hp_damage > 0 {
                        target_stats.hp = (target_stats.hp - hp_damage).max(0);
                    }
                    event_writer.write(BattleEvent::DamageDealt {
                        source: attacker_side,
                        target: target_side,
                        amount: hp_damage,
                    });
                } else {
                    // 盾未吸收：允许附着/反应；并在成功元素攻击后设置 incoming 元素。
                    let hp_damage = theoretical_damage_with_aura;
                    if hp_damage > 0 {
                        target_stats.hp = (target_stats.hp - hp_damage).max(0);
                    }
                    event_writer.write(BattleEvent::DamageDealt {
                        source: attacker_side,
                        target: target_side,
                        amount: hp_damage,
                    });
                    let from = target_aura.attached;
                    target_aura.attached = Some(incoming_element);
                    event_writer.write(BattleEvent::ElementAuraApplied {
                        side: target_side,
                        from,
                        to: incoming_element,
                        effectiveness: effectiveness_with_aura,
                    });
                }
            } else {
                // 非元素攻击：不进行附着/反应处理。
                let theoretical_damage = (raw as f32).max(1.0) as i32;
                let absorbed = target_shield.0.min(theoretical_damage);
                if absorbed > 0 {
                    target_shield.0 -= absorbed;
                    event_writer.write(BattleEvent::ShieldAbsorbed {
                        side: target_side,
                        amount: absorbed,
                    });
                }

                let hp_damage = (theoretical_damage - absorbed).max(0);
                if hp_damage > 0 {
                    target_stats.hp = (target_stats.hp - hp_damage).max(0);
                }
                event_writer.write(BattleEvent::DamageDealt {
                    source: attacker_side,
                    target: target_side,
                    amount: hp_damage,
                });
            }
        }
        SkillEffect::Heal { amount } => {
            let mut heal_amount = *amount;
            if attacker_side == Side::Player {
                heal_amount += pending_boosts.player.next_heal_bonus;
                pending_boosts.player.next_heal_bonus = 0;
            } else {
                heal_amount += pending_boosts.enemy.next_heal_bonus;
                pending_boosts.enemy.next_heal_bonus = 0;
            }
            let before = attacker_stats.hp;
            attacker_stats.hp = (attacker_stats.hp + heal_amount).min(attacker_stats.max_hp);
            event_writer.write(BattleEvent::Healed {
                side: attacker_side,
                amount: attacker_stats.hp - before,
            });
        }
        SkillEffect::Shield { amount } => {
            let mut shield_amount = *amount;
            if attacker_side == Side::Player {
                shield_amount += pending_boosts.player.next_shield_bonus;
                pending_boosts.player.next_shield_bonus = 0;
            } else {
                shield_amount += pending_boosts.enemy.next_shield_bonus;
                pending_boosts.enemy.next_shield_bonus = 0;
            }

            attacker_shield.0 += shield_amount;
            event_writer.write(BattleEvent::ShieldGained {
                side: attacker_side,
                amount: shield_amount,
            });
        }
    }
}
