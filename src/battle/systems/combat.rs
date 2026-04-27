use bevy::prelude::*;

use crate::{
    battle::{
        AccuracyRng, BattleEvent, BattleFormulaEvent, BattleStatusEvent, ElementAura,
        PendingBoosts, Shield, Side, Stats, StatusBoard, StatusInstance, StructuredBattleLog,
        apply_status_from_def, formula_event, note_action_phase, remove_status_by_id, status_event,
        upsert_status_instance,
    },
    data::{
        AttributeStageModifier, AttributeType, BattleFormulaRules, EffectTarget, ElementDb,
        ElementType, ReactionDb, ReactionDef, SkillCategory, SkillCondition, SkillDef, SkillEffect,
        StatusCategory, StatusDb, StatusTickTiming,
    },
};

pub(crate) fn effective_atk(stats: &Stats, rules: &BattleFormulaRules) -> i32 {
    scale_staged_ratio(stats.atk, stats.atk_stage, rules).max(1)
}

pub(crate) fn effective_def(stats: &Stats, rules: &BattleFormulaRules) -> i32 {
    scale_staged_ratio(stats.def, stats.def_stage, rules).max(0)
}

pub(crate) fn effective_spd(stats: &Stats, rules: &BattleFormulaRules) -> i32 {
    let stage = clamped_stage(stats.spd_stage, rules);
    (stats.spd + stage * 2).max(1)
}

pub(crate) fn effective_acc(stats: &Stats, skill: &SkillDef, rules: &BattleFormulaRules) -> f32 {
    let stage = clamped_stage(stats.acc_stage, rules);
    let base = stats.acc as f32 / 100.0;
    let skill_scale = skill.base_accuracy.unwrap_or(1.0);
    (base * skill_scale + stage as f32 * rules.accuracy.stage_step)
        .clamp(rules.accuracy.min, rules.accuracy.max)
}

fn clamped_stage(stage: i32, rules: &BattleFormulaRules) -> i32 {
    stage.clamp(
        rules.attribute_stage_bounds.min,
        rules.attribute_stage_bounds.max,
    )
}

fn stage_ratio(stage: i32, rules: &BattleFormulaRules) -> f32 {
    let stage = clamped_stage(stage, rules);
    if stage >= 0 {
        (2 + stage) as f32 / 2.0
    } else {
        2.0 / (2 + stage.abs()) as f32
    }
}

fn scale_staged_ratio(base: i32, stage: i32, rules: &BattleFormulaRules) -> i32 {
    ((base as f32) * stage_ratio(stage, rules)).round() as i32
}

fn effective_attribute_value(
    stats: &Stats,
    attribute: AttributeType,
    rules: &BattleFormulaRules,
) -> i32 {
    match attribute {
        AttributeType::Atk => effective_atk(stats, rules),
        AttributeType::Def => effective_def(stats, rules),
        AttributeType::Spd => effective_spd(stats, rules),
        AttributeType::Acc => {
            ((effective_acc_percent(stats, rules) * 100.0).round() as i32).clamp(0, 100)
        }
    }
}

fn effective_acc_percent(stats: &Stats, rules: &BattleFormulaRules) -> f32 {
    let stage = clamped_stage(stats.acc_stage, rules);
    let base = stats.acc as f32 / 100.0;
    (base + stage as f32 * rules.accuracy.stage_step).clamp(rules.accuracy.min, rules.accuracy.max)
}

fn stat_difference_damage(
    attacker_stats: &Stats,
    target_stats: &Stats,
    source_attribute: AttributeType,
    target_attribute: AttributeType,
    multiply_by_source_stage: bool,
    rules: &BattleFormulaRules,
) -> i32 {
    let source_value = effective_attribute_value(attacker_stats, source_attribute, rules);
    let target_value = effective_attribute_value(target_stats, target_attribute, rules);
    let mut difference = (source_value - target_value).max(0);
    if multiply_by_source_stage {
        difference = scale_staged_ratio(difference, attacker_stats.def_stage, rules).max(0);
    }
    difference
}

fn attack_multiplier(power: i32) -> f32 {
    power as f32 / 10.0
}

fn scaled_damage_after_defense(
    power: i32,
    attacker_stats: &Stats,
    target_stats: &Stats,
    rules: &BattleFormulaRules,
) -> (i32, i32, i32, f32) {
    let atk = effective_atk(attacker_stats, rules);
    let def = effective_def(target_stats, rules);
    let multiplier = attack_multiplier(power).max(0.0);
    let skill_damage = ((atk as f32) * multiplier).round() as i32;
    let damage = (skill_damage - def).max(rules.damage.min_damage);
    (atk, def, damage, multiplier)
}

fn final_damage(raw_damage: i32, effectiveness: f32, rules: &BattleFormulaRules) -> i32 {
    ((raw_damage as f32) * effectiveness)
        .round()
        .max(rules.damage.min_damage as f32) as i32
}

fn apply_status_by_id(
    status_id: &str,
    source_side: Side,
    target_side: Side,
    target_stats: &mut Stats,
    target_statuses: &mut StatusBoard,
    status_db: &StatusDb,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
) -> bool {
    let Some(def) = status_db.statuses.get(status_id) else {
        return false;
    };

    let refreshed = apply_status_from_def(
        target_statuses,
        target_stats,
        def,
        Some(source_side),
        round.unwrap_or(0),
    );

    if let Some(round) = round {
        if let Some(writer) = status_writer.as_deref_mut() {
            writer.write(status_event(
                round,
                target_side,
                status_id,
                if refreshed { "refreshed" } else { "applied" },
                format!(
                    "name={} category={:?} duration={} tick_timing={:?} stage_modifiers={:?} fixed_damage_on_tick={} heal_taken_multiplier={:?} evade_charges={}",
                    def.name,
                    def.category,
                    def.duration_turns,
                    def.tick_timing,
                    def.stage_modifiers,
                    def.fixed_damage_on_tick,
                    def.heal_taken_multiplier,
                    def.evade_charges
                ),
            ));
        }
        if let Some(log) = structured_log.as_deref_mut() {
            note_action_phase(
                log,
                round,
                source_side,
                "状态结算",
                format!(
                    "{}{}；状态={}；分类={:?}；持续={}回合；tick={:?}；阶段修正={:?}；固定伤害={}；治疗修正={:?}；闪避次数={}",
                    if refreshed { "刷新" } else { "施加" },
                    if target_side == Side::Player {
                        "玩家状态"
                    } else {
                        "敌方状态"
                    },
                    def.name,
                    def.category,
                    def.duration_turns,
                    def.tick_timing,
                    def.stage_modifiers,
                    def.fixed_damage_on_tick,
                    def.heal_taken_multiplier,
                    def.evade_charges
                ),
            );
        }
    }

    true
}

fn attack_hits(
    skill: &SkillDef,
    attacker_side: Side,
    target_side: Side,
    attacker_stats: &Stats,
    target_stats: &mut Stats,
    target_statuses: &mut StatusBoard,
    rules: &BattleFormulaRules,
    accuracy_rng: &mut AccuracyRng,
    event_writer: &mut MessageWriter<BattleEvent>,
    formula_writer: Option<&mut MessageWriter<BattleFormulaEvent>>,
    status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
) -> bool {
    let mut formula_writer = formula_writer;
    let mut status_writer = status_writer;
    let mut structured_log = structured_log;
    let accuracy = effective_acc(attacker_stats, skill, rules);
    let roll = accuracy_rng.next_unit_f32();
    let hit = roll <= accuracy;

    if !hit {
        event_writer.write(BattleEvent::AttackMissed {
            source: attacker_side,
            target: target_side,
        });
    }

    if let Some(round) = round {
        if let Some(writer) = formula_writer.as_deref_mut() {
            writer.write(formula_event(
                round,
                attacker_side,
                target_side,
                if hit { "accuracy_hit" } else { "accuracy_miss" },
                format!(
                    "base_acc={} acc_stage={} skill_base_accuracy={:?} final_accuracy={:.2} roll={:.4}",
                    attacker_stats.acc,
                    attacker_stats.acc_stage,
                    skill.base_accuracy,
                    accuracy,
                    roll
                ),
            ));
        }
        if let Some(log) = structured_log.as_deref_mut() {
            note_action_phase(
                log,
                round,
                attacker_side,
                "命中判定",
                format!(
                    "基础Acc={}%；Acc等级={}；技能命中系数={:?}；最终命中率={:.0}%；掷值={:.2}%；结果={}",
                    attacker_stats.acc,
                    attacker_stats.acc_stage,
                    skill.base_accuracy,
                    accuracy * 100.0,
                    roll * 100.0,
                    if hit { "命中" } else { "未命中" }
                ),
            );
        }
    }

    if !hit {
        return false;
    }

    let evaded = if target_statuses
        .entries
        .iter()
        .any(|entry| entry.evade_charges > 0)
    {
        let evade_accuracy =
            ((target_stats.acc as f32) / 100.0).clamp(rules.accuracy.min, rules.accuracy.max);
        let evade_roll = accuracy_rng.next_unit_f32();
        let evaded = evade_roll <= evade_accuracy;
        if let Some(round) = round {
            if let Some(writer) = formula_writer.as_deref_mut() {
                writer.write(formula_event(
                    round,
                    target_side,
                    attacker_side,
                    if evaded {
                        "evade_success"
                    } else {
                        "evade_failed"
                    },
                    format!(
                        "base_acc={} acc_stage={} final_evade_chance={:.2} roll={:.4}",
                        target_stats.acc, target_stats.acc_stage, evade_accuracy, evade_roll
                    ),
                ));
            }
            if let Some(log) = structured_log.as_deref_mut() {
                note_action_phase(
                    log,
                    round,
                    target_side,
                    "闪避判定",
                    format!(
                        "基础Acc={}%；Acc等级={}；闪避概率={:.0}%；掷值={:.2}%；结果={}",
                        target_stats.acc,
                        target_stats.acc_stage,
                        evade_accuracy * 100.0,
                        evade_roll * 100.0,
                        if evaded {
                            "闪避成功"
                        } else {
                            "闪避失败"
                        }
                    ),
                );
            }
        }
        evaded
    } else {
        false
    };

    if !evaded {
        return true;
    }

    if let Some(status_id) = target_statuses.try_consume_evade_charge() {
        if let Some(round) = round {
            if let Some(writer) = status_writer.as_deref_mut() {
                writer.write(status_event(
                    round,
                    target_side,
                    status_id,
                    "consumed",
                    "evade_charge_consumed_by_single_target_attack",
                ));
            }
            if let Some(log) = structured_log.as_deref_mut() {
                note_action_phase(
                    log,
                    round,
                    target_side,
                    "闪避消耗",
                    "消耗1次闪避，免疫本次单体攻击的伤害、附着和减益",
                );
            }
        }
    }

    event_writer.write(BattleEvent::AttackMissed {
        source: attacker_side,
        target: target_side,
    });
    false
}

fn has_status(statuses: &StatusBoard, status_id: &str) -> bool {
    statuses.entries.iter().any(|entry| entry.id == status_id)
}

fn reaction_matches(
    reaction: &ReactionDef,
    current_auras: &[ElementType],
    current_statuses: &StatusBoard,
    incoming_element: ElementType,
) -> bool {
    if reaction.trigger_element != incoming_element {
        return false;
    }
    if reaction.required_elements.len() > 2 {
        return false;
    }
    if !reaction.required_elements.is_empty()
        && !reaction.required_elements.contains(&incoming_element)
    {
        return false;
    }
    if !reaction
        .required_elements
        .iter()
        .filter(|element| **element != incoming_element)
        .all(|element| current_auras.contains(element))
    {
        return false;
    }
    reaction
        .required_statuses
        .iter()
        .all(|status_id| has_status(current_statuses, status_id))
}

fn detect_element_reaction<'a>(
    reaction_db: &'a ReactionDb,
    current_auras: &[ElementType],
    current_statuses: &StatusBoard,
    incoming_element: ElementType,
) -> Option<&'a ReactionDef> {
    reaction_db.reactions.iter().find(|reaction| {
        reaction_matches(reaction, current_auras, current_statuses, incoming_element)
    })
}

fn clear_status_by_id(
    status_id: &str,
    target_side: Side,
    target_stats: &mut Stats,
    target_statuses: &mut StatusBoard,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
) -> bool {
    let removed = remove_status_by_id(target_statuses, target_stats, status_id);
    if removed {
        if let Some(round) = round {
            if let Some(writer) = status_writer.as_deref_mut() {
                writer.write(status_event(
                    round,
                    target_side,
                    status_id,
                    "removed",
                    "removed_by_reaction_or_sync",
                ));
            }
            if let Some(log) = structured_log.as_deref_mut() {
                note_action_phase(
                    log,
                    round,
                    target_side,
                    "状态移除",
                    format!("移除状态={status_id}"),
                );
            }
        }
    }
    removed
}

fn sync_aura_status(
    aura: &[ElementType],
    source_side: Side,
    target_side: Side,
    target_stats: &mut Stats,
    target_statuses: &mut StatusBoard,
    status_db: &StatusDb,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
) {
    for (element, status_id) in [
        (ElementType::Fire, "burning_aura"),
        (ElementType::Water, "wet_aura"),
        (ElementType::Grass, "thorn_aura"),
        (ElementType::Thunder, "paralysis_aura"),
    ] {
        if aura.contains(&element) {
            let _ = apply_status_by_id(
                status_id,
                source_side,
                target_side,
                target_stats,
                target_statuses,
                status_db,
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
            );
        } else {
            let _ = clear_status_by_id(
                status_id,
                target_side,
                target_stats,
                target_statuses,
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
            );
        }
    }
}

fn emit_reaction(
    reaction_name: &str,
    attacker_side: Side,
    target_side: Side,
    event_writer: &mut MessageWriter<BattleEvent>,
    mut formula_writer: Option<&mut MessageWriter<BattleFormulaEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
    detail: impl Into<String>,
) {
    let detail = detail.into();
    event_writer.write(BattleEvent::ReactionTriggered {
        source: attacker_side,
        target: target_side,
        reaction_name: reaction_name.to_string(),
    });
    if let Some(round) = round {
        if let Some(writer) = formula_writer.as_deref_mut() {
            writer.write(formula_event(
                round,
                attacker_side,
                target_side,
                "element_reaction",
                format!("reaction={} {}", reaction_name, detail),
            ));
        }
        if let Some(log) = structured_log.as_deref_mut() {
            note_action_phase(
                log,
                round,
                attacker_side,
                "元素反应",
                format!("反应={}；{}", reaction_name, detail),
            );
        }
    }
}

fn apply_damage_with_shield(
    source_side: Side,
    target_side: Side,
    amount: i32,
    target_stats: &mut Stats,
    target_shield: &mut Shield,
    event_writer: &mut MessageWriter<BattleEvent>,
) -> (i32, i32) {
    let absorbed = target_shield.0.min(amount.max(0));
    if absorbed > 0 {
        target_shield.0 -= absorbed;
        event_writer.write(BattleEvent::ShieldAbsorbed {
            side: target_side,
            amount: absorbed,
        });
    }

    let hp_damage = (amount - absorbed).max(0);
    if hp_damage > 0 {
        target_stats.hp = (target_stats.hp - hp_damage).max(0);
    }
    event_writer.write(BattleEvent::DamageDealt {
        source: source_side,
        target: target_side,
        amount: hp_damage,
    });
    (absorbed, hp_damage)
}

fn supports_normal_attachment(element: ElementType) -> bool {
    matches!(
        element,
        ElementType::Fire | ElementType::Water | ElementType::Grass | ElementType::Thunder
    )
}

fn heal_target(
    heal_side: Side,
    raw_amount: i32,
    target_stats: &mut Stats,
    target_statuses: &StatusBoard,
    event_writer: &mut MessageWriter<BattleEvent>,
) -> (i32, f32, i32) {
    let multiplier = target_statuses.heal_taken_multiplier();
    let final_heal = ((raw_amount as f32) * multiplier).round() as i32;
    let before = target_stats.hp;
    target_stats.hp = (target_stats.hp + final_heal).min(target_stats.max_hp);
    let actual_heal = target_stats.hp - before;
    event_writer.write(BattleEvent::Healed {
        side: heal_side,
        amount: actual_heal,
    });
    (final_heal, multiplier, actual_heal)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EffectResolutionContext {
    target_had_aura: Vec<ElementType>,
    target_had_statuses: Vec<String>,
    target_had_shield: bool,
    target_fainted: bool,
    last_hp_damage: i32,
    last_reaction_name: Option<String>,
    last_wind_spread_succeeded: bool,
    last_cleanse_succeeded: bool,
    last_attack_resolved: bool,
}

fn next_status_id_with_prefix(statuses: &StatusBoard, prefix: &str) -> String {
    let mut index = 1;
    loop {
        let candidate = format!("{prefix}_{index}");
        if !statuses.entries.iter().any(|entry| entry.id == candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn apply_stage_modifiers_to_target(
    modifiers: &[AttributeStageModifier],
    duration_turns: i32,
    source_side: Side,
    target_side: Side,
    target_stats: &mut Stats,
    target_statuses: &mut StatusBoard,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
) {
    if modifiers.is_empty() {
        return;
    }

    let has_debuff = modifiers.iter().any(|modifier| modifier.amount < 0);
    let status_prefix = if has_debuff {
        "stage_shift_debuff"
    } else {
        "stage_shift_buff"
    };

    let status = StatusInstance {
        id: next_status_id_with_prefix(target_statuses, status_prefix),
        name: "属性变化".to_string(),
        category: if has_debuff {
            StatusCategory::Debuff
        } else {
            StatusCategory::Buff
        },
        remaining_turns: duration_turns,
        applied_round: round.unwrap_or(0),
        source_side: Some(source_side),
        tick_timing: Some(StatusTickTiming::OwnerActionEnd),
        stage_modifiers: modifiers
            .iter()
            .map(|modifier| crate::battle::StatusStageModifier {
                attribute: modifier.attribute,
                amount: modifier.amount,
            })
            .collect(),
        fixed_damage_on_tick: 0,
        heal_on_tick: 0,
        heal_taken_multiplier: None,
        evade_charges: 0,
    };

    let refreshed = upsert_status_instance(target_statuses, status, target_stats);

    if let Some(round) = round {
        if let Some(writer) = status_writer.as_deref_mut() {
            writer.write(status_event(
                round,
                target_side,
                "stage_shift",
                if refreshed { "refreshed" } else { "applied" },
                format!("duration={} modifiers={:?}", duration_turns, modifiers),
            ));
        }
        if let Some(log) = structured_log.as_deref_mut() {
            note_action_phase(
                log,
                round,
                source_side,
                "属性变化",
                format!(
                    "{}{}属性变化；持续={}回合；修正={:?}",
                    if refreshed { "刷新" } else { "施加" },
                    if target_side == Side::Player {
                        "玩家"
                    } else {
                        "敌方"
                    },
                    duration_turns,
                    modifiers
                ),
            );
        }
    }
}

fn aura_status_id(element: ElementType) -> Option<&'static str> {
    match element {
        ElementType::Fire => Some("burning_aura"),
        ElementType::Water => Some("wet_aura"),
        ElementType::Grass => Some("thorn_aura"),
        ElementType::Thunder => Some("paralysis_aura"),
        _ => None,
    }
}

fn clear_one_aura(
    target_side: Side,
    target_stats: &mut Stats,
    target_aura: &mut ElementAura,
    target_statuses: &mut StatusBoard,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
) -> Option<ElementType> {
    let element = target_aura.primary()?;
    target_aura.remove(element);
    if let Some(status_id) = aura_status_id(element) {
        let _ = clear_status_by_id(
            status_id,
            target_side,
            target_stats,
            target_statuses,
            status_writer.as_deref_mut(),
            structured_log.as_deref_mut(),
            round,
        );
    }
    if let Some(round) = round {
        if let Some(writer) = status_writer.as_deref_mut() {
            writer.write(status_event(
                round,
                target_side,
                "element_aura",
                "cleansed",
                format!(
                    "cleared_element={:?} aura_after={:?}",
                    element,
                    target_aura.elements()
                ),
            ));
        }
        if let Some(log) = structured_log.as_deref_mut() {
            note_action_phase(
                log,
                round,
                target_side,
                "净化结算",
                format!(
                    "移除元素附着={:?}；剩余附着={:?}",
                    element,
                    target_aura.elements()
                ),
            );
        }
    }
    Some(element)
}

fn clear_one_debuff_status(
    target_side: Side,
    target_stats: &mut Stats,
    target_statuses: &mut StatusBoard,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
) -> Option<String> {
    let status_id = target_statuses
        .entries
        .iter()
        .find(|entry| {
            matches!(
                entry.category,
                StatusCategory::Debuff | StatusCategory::Special
            )
        })?
        .id
        .clone();
    let removed = clear_status_by_id(
        &status_id,
        target_side,
        target_stats,
        target_statuses,
        status_writer.as_deref_mut(),
        structured_log.as_deref_mut(),
        round,
    );
    removed.then_some(status_id)
}

fn dispel_statuses(
    status_ids: &[String],
    source_side: Side,
    target_side: Side,
    target_stats: &mut Stats,
    target_statuses: &mut StatusBoard,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
) -> usize {
    let mut removed_count = 0;
    for status_id in status_ids {
        let matched_ids: Vec<String> = if matches!(
            status_id.as_str(),
            "stage_shift_buff" | "stage_shift_debuff"
        ) {
            target_statuses
                .entries
                .iter()
                .filter(|entry| {
                    entry.id == *status_id || entry.id.starts_with(&format!("{status_id}_"))
                })
                .map(|entry| entry.id.clone())
                .collect()
        } else {
            vec![status_id.clone()]
        };

        for matched_id in matched_ids {
            if clear_status_by_id(
                &matched_id,
                target_side,
                target_stats,
                target_statuses,
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
            ) {
                removed_count += 1;
            }
        }
    }
    if removed_count > 0 {
        if let Some(round) = round {
            if let Some(log) = structured_log.as_deref_mut() {
                note_action_phase(
                    log,
                    round,
                    source_side,
                    "驱散结算",
                    format!(
                        "驱散目标{}状态；移除数量={}；候选={:?}",
                        if target_side == Side::Player {
                            "玩家"
                        } else {
                            "敌方"
                        },
                        removed_count,
                        status_ids
                    ),
                );
            }
        }
    }
    removed_count
}

fn evaluate_condition(condition: &SkillCondition, ctx: &EffectResolutionContext) -> bool {
    match condition {
        SkillCondition::TargetHadAura { element } => ctx.target_had_aura.contains(element),
        SkillCondition::TargetHadStatus { status_id } => {
            ctx.target_had_statuses.iter().any(|id| id == status_id)
        }
        SkillCondition::TargetHadNoShield => !ctx.target_had_shield,
        SkillCondition::LastReactionName { reaction_name } => {
            ctx.last_reaction_name.as_deref() == Some(reaction_name.as_str())
        }
        SkillCondition::LastWindSpreadSucceeded => {
            ctx.last_attack_resolved && ctx.last_wind_spread_succeeded
        }
        SkillCondition::LastWindSpreadFailed => {
            ctx.last_attack_resolved && !ctx.last_wind_spread_succeeded
        }
        SkillCondition::LastCleanseSucceeded => ctx.last_cleanse_succeeded,
        SkillCondition::LastCleanseFailed => !ctx.last_cleanse_succeeded,
        SkillCondition::LastTargetFainted => ctx.target_fainted,
        SkillCondition::Any { conditions } => conditions
            .iter()
            .any(|nested| evaluate_condition(nested, ctx)),
        SkillCondition::All { conditions } => conditions
            .iter()
            .all(|nested| evaluate_condition(nested, ctx)),
    }
}

fn effect_target_side(
    explicit: EffectTarget,
    fallback: SkillTargetMode,
    actor_side: Side,
    target_side: Side,
) -> Side {
    match explicit {
        EffectTarget::SelfTarget => actor_side,
        EffectTarget::Opponent => target_side,
        EffectTarget::Infer => match fallback {
            SkillTargetMode::SelfOnly => actor_side,
            SkillTargetMode::Opponent => target_side,
        },
    }
}

fn apply_fixed_damage(
    source_side: Side,
    target_side: Side,
    amount: i32,
    ignore_shield: bool,
    target_stats: &mut Stats,
    target_shield: &mut Shield,
    event_writer: &mut MessageWriter<BattleEvent>,
) -> (i32, i32) {
    if ignore_shield {
        let hp_damage = amount.max(0);
        if hp_damage > 0 {
            target_stats.hp = (target_stats.hp - hp_damage).max(0);
        }
        event_writer.write(BattleEvent::DamageDealt {
            source: source_side,
            target: target_side,
            amount: hp_damage,
        });
        (0, hp_damage)
    } else {
        apply_damage_with_shield(
            source_side,
            target_side,
            amount,
            target_stats,
            target_shield,
            event_writer,
        )
    }
}

fn apply_stat_difference_damage(
    source_side: Side,
    target_side: Side,
    source_attribute: AttributeType,
    target_attribute: AttributeType,
    multiply_by_source_stage: bool,
    ignore_shield: bool,
    attacker_stats: &Stats,
    target_stats: &mut Stats,
    target_shield: &mut Shield,
    formula_rules: &BattleFormulaRules,
    event_writer: &mut MessageWriter<BattleEvent>,
) -> (i32, i32, i32) {
    let amount = stat_difference_damage(
        attacker_stats,
        target_stats,
        source_attribute,
        target_attribute,
        multiply_by_source_stage,
        formula_rules,
    );
    let (absorbed, hp_damage) = apply_fixed_damage(
        source_side,
        target_side,
        amount,
        ignore_shield,
        target_stats,
        target_shield,
        event_writer,
    );
    (amount, absorbed, hp_damage)
}

fn spread_priority(element: ElementType) -> i32 {
    match element {
        ElementType::Fire => 3,
        ElementType::Water => 2,
        ElementType::Thunder => 1,
        _ => 0,
    }
}

fn spread_damage(element: ElementType, is_front: bool) -> i32 {
    match (element, is_front) {
        (ElementType::Fire, true) => 5,
        (ElementType::Fire, false) => 4,
        (ElementType::Water, true) => 3,
        (ElementType::Water, false) => 2,
        (ElementType::Thunder, true) => 4,
        (ElementType::Thunder, false) => 3,
        _ => 0,
    }
}

fn select_spread_element(
    target_aura: &ElementAura,
    target_statuses: &StatusBoard,
) -> Option<ElementType> {
    let mut best: Option<(ElementType, i32, i32)> = None;
    for element in [ElementType::Fire, ElementType::Water, ElementType::Thunder] {
        if !target_aura.contains(element) {
            continue;
        }
        let remaining_turns = aura_status_id(element)
            .and_then(|status_id| {
                target_statuses
                    .entries
                    .iter()
                    .find(|entry| entry.id == status_id)
                    .map(|entry| entry.remaining_turns)
            })
            .unwrap_or(0);
        let candidate = (element, remaining_turns, spread_priority(element));
        match best {
            Some((_, best_turns, best_priority))
                if remaining_turns < best_turns
                    || (remaining_turns == best_turns && candidate.2 <= best_priority) => {}
            _ => best = Some(candidate),
        }
    }
    best.map(|(element, _, _)| element)
}

struct AttachmentResolutionOutcome {
    aura_from: Vec<ElementType>,
    aura_to: Vec<ElementType>,
    reaction_name: Option<String>,
    reaction_id: Option<String>,
    reaction_fixed_damage: i32,
    reaction_absorbed: i32,
    reaction_hp_damage: i32,
    reaction_heal_raw: i32,
    reaction_final_heal: i32,
    reaction_heal_multiplier: f32,
    reaction_actual_heal: i32,
    evicted_aura: Option<ElementType>,
}

impl Default for AttachmentResolutionOutcome {
    fn default() -> Self {
        Self {
            aura_from: Vec::new(),
            aura_to: Vec::new(),
            reaction_name: None,
            reaction_id: None,
            reaction_fixed_damage: 0,
            reaction_absorbed: 0,
            reaction_hp_damage: 0,
            reaction_heal_raw: 0,
            reaction_final_heal: 0,
            reaction_heal_multiplier: 1.0,
            reaction_actual_heal: 0,
            evicted_aura: None,
        }
    }
}

fn resolve_element_attachment_only(
    incoming_element: ElementType,
    attacker_side: Side,
    target_side: Side,
    attacker_stats: &mut Stats,
    attacker_statuses: &mut StatusBoard,
    target_stats: &mut Stats,
    target_shield: &mut Shield,
    target_aura: &mut ElementAura,
    target_statuses: &mut StatusBoard,
    status_db: &StatusDb,
    reaction_db: &ReactionDb,
    event_writer: &mut MessageWriter<BattleEvent>,
    mut formula_writer: Option<&mut MessageWriter<BattleFormulaEvent>>,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
    action_label: &str,
) -> AttachmentResolutionOutcome {
    let from = target_aura.elements();
    let mut outcome = AttachmentResolutionOutcome {
        aura_from: from.clone(),
        aura_to: from.clone(),
        ..Default::default()
    };

    let mut reaction_required_elements: Vec<ElementType> = Vec::new();
    let mut reaction_required_statuses: Vec<&str> = Vec::new();
    let mut reaction_statuses_to_apply: Vec<&str> = Vec::new();
    let mut reaction_statuses_to_clear: Vec<&str> = Vec::new();

    if let Some(round) = round {
        if let Some(writer) = formula_writer.as_deref_mut() {
            writer.write(formula_event(
                round,
                attacker_side,
                target_side,
                format!("{}_attempt", action_label),
                format!(
                    "incoming_element={:?} aura_before={:?} current_statuses={:?}",
                    incoming_element,
                    from,
                    target_statuses
                        .entries
                        .iter()
                        .map(|entry| entry.id.as_str())
                        .collect::<Vec<_>>()
                ),
            ));
        }
    }

    if supports_normal_attachment(incoming_element) {
        if let Some(reaction) =
            detect_element_reaction(reaction_db, &from, target_statuses, incoming_element)
        {
            outcome.reaction_id = Some(reaction.id.clone());
            outcome.reaction_name = Some(reaction.name.clone());
            reaction_required_elements = reaction.required_elements.clone();
            reaction_required_statuses = reaction
                .required_statuses
                .iter()
                .map(String::as_str)
                .collect();
            outcome.reaction_fixed_damage = reaction.fixed_damage;
            outcome.reaction_heal_raw = reaction.heal_attacker;
            reaction_statuses_to_apply =
                reaction.apply_statuses.iter().map(String::as_str).collect();
            reaction_statuses_to_clear =
                reaction.clear_statuses.iter().map(String::as_str).collect();
            outcome.aura_to = reaction.aura_results.clone();
        } else if !outcome.aura_to.contains(&incoming_element) {
            if outcome.aura_to.len() >= 2 {
                outcome.evicted_aura = outcome.aura_to.first().copied();
                outcome.aura_to.remove(0);
            }
            outcome.aura_to.push(incoming_element);
        }
    }

    for status_id in reaction_statuses_to_clear.iter().copied() {
        let removed = clear_status_by_id(
            status_id,
            target_side,
            target_stats,
            target_statuses,
            status_writer.as_deref_mut(),
            structured_log.as_deref_mut(),
            round,
        );
        if removed {
            if let Some(round) = round {
                if let Some(writer) = status_writer.as_deref_mut() {
                    writer.write(status_event(
                        round,
                        target_side,
                        status_id,
                        "consumed_by_reaction",
                        format!(
                            "reaction_id={:?} reaction_name={:?} aura_before={:?}",
                            outcome.reaction_id, outcome.reaction_name, from
                        ),
                    ));
                }
            }
        }
    }

    if let Some(round) = round {
        if let Some(writer) = formula_writer.as_deref_mut() {
            writer.write(formula_event(
                round,
                attacker_side,
                target_side,
                format!("{}_resolved", action_label),
                format!(
                    "incoming_element={:?} reaction_id={:?} reaction_name={:?} required_elements={:?} required_statuses={:?} statuses_to_clear={:?} statuses_to_apply={:?} evicted_aura={:?} aura_before={:?} aura_after_candidate={:?}",
                    incoming_element,
                    outcome.reaction_id,
                    outcome.reaction_name,
                    reaction_required_elements,
                    reaction_required_statuses,
                    reaction_statuses_to_clear,
                    reaction_statuses_to_apply,
                    outcome.evicted_aura,
                    from,
                    outcome.aura_to
                ),
            ));
        }
    }

    target_aura.set_elements(&outcome.aura_to);
    outcome.aura_to = target_aura.elements();
    sync_aura_status(
        &outcome.aura_to,
        attacker_side,
        target_side,
        target_stats,
        target_statuses,
        status_db,
        status_writer.as_deref_mut(),
        structured_log.as_deref_mut(),
        round,
    );

    for status_id in reaction_statuses_to_apply {
        let _ = apply_status_by_id(
            status_id,
            attacker_side,
            target_side,
            target_stats,
            target_statuses,
            status_db,
            status_writer.as_deref_mut(),
            structured_log.as_deref_mut(),
            round,
        );
    }

    if outcome.reaction_fixed_damage > 0 {
        (outcome.reaction_absorbed, outcome.reaction_hp_damage) = apply_damage_with_shield(
            attacker_side,
            target_side,
            outcome.reaction_fixed_damage,
            target_stats,
            target_shield,
            event_writer,
        );
    }

    if outcome.reaction_heal_raw > 0 {
        (
            outcome.reaction_final_heal,
            outcome.reaction_heal_multiplier,
            outcome.reaction_actual_heal,
        ) = heal_target(
            attacker_side,
            outcome.reaction_heal_raw,
            attacker_stats,
            attacker_statuses,
            event_writer,
        );
    }

    if from != outcome.aura_to {
        event_writer.write(BattleEvent::ElementAuraApplied {
            side: target_side,
            from: from.clone(),
            to: outcome.aura_to.clone(),
            effectiveness: 1.0,
        });
        if let Some(round) = round {
            if let Some(writer) = status_writer.as_deref_mut() {
                writer.write(status_event(
                    round,
                    target_side,
                    "element_aura",
                    if outcome.aura_to.is_empty() {
                        "cleared"
                    } else {
                        "applied"
                    },
                    format!(
                        "from={:?} to={:?} effectiveness=1.00",
                        from, outcome.aura_to
                    ),
                ));
                writer.write(status_event(
                    round,
                    target_side,
                    "element_aura_final",
                    "resolved",
                    format!(
                        "incoming_element={:?} reaction_id={:?} reaction_name={:?} aura_before={:?} aura_after={:?} evicted_aura={:?}",
                        incoming_element,
                        outcome.reaction_id,
                        outcome.reaction_name,
                        from,
                        outcome.aura_to,
                        outcome.evicted_aura
                    ),
                ));
            }
        }
    }

    if let Some(name) = outcome.reaction_name.as_deref() {
        emit_reaction(
            name,
            attacker_side,
            target_side,
            event_writer,
            formula_writer.as_deref_mut(),
            structured_log.as_deref_mut(),
            round,
            format!(
                "incoming_element={:?} aura_before={:?} aura_after={:?} reaction_damage={} reaction_absorbed={} reaction_hp_damage={} reaction_heal_raw={} reaction_heal_final={} reaction_heal_multiplier={:.2} reaction_actual_heal={}",
                incoming_element,
                from,
                outcome.aura_to,
                outcome.reaction_fixed_damage,
                outcome.reaction_absorbed,
                outcome.reaction_hp_damage,
                outcome.reaction_heal_raw,
                outcome.reaction_final_heal,
                outcome.reaction_heal_multiplier,
                outcome.reaction_actual_heal
            ),
        );
    }

    outcome
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillExecutionMode {
    Standard,
    WindSpread,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillTargetMode {
    SelfOnly,
    Opponent,
}

fn primary_attack_effect(effect: &SkillEffect) -> Option<&SkillEffect> {
    match effect {
        SkillEffect::Attack { .. } => Some(effect),
        SkillEffect::Sequence { effects } => effects.first().and_then(primary_attack_effect),
        _ => None,
    }
}

pub(crate) fn skill_execution_mode(skill: &SkillDef) -> SkillExecutionMode {
    match (
        skill.category,
        skill.element,
        primary_attack_effect(&skill.effect),
    ) {
        (
            SkillCategory::SpecialAttack,
            Some(ElementType::Wind),
            Some(SkillEffect::Attack { .. }),
        ) => SkillExecutionMode::WindSpread,
        _ => SkillExecutionMode::Standard,
    }
}

pub(crate) fn skill_target_mode(skill: &SkillDef) -> SkillTargetMode {
    match skill.category {
        SkillCategory::SelfUtility | SkillCategory::AllyUtility => SkillTargetMode::SelfOnly,
        SkillCategory::NormalAttack
        | SkillCategory::ElementAttack
        | SkillCategory::SpecialAttack
        | SkillCategory::EnemyDebuff => SkillTargetMode::Opponent,
    }
}

pub(crate) struct WindSpreadTarget<'a> {
    pub base_element: ElementType,
    pub stats: &'a mut Stats,
    pub shield: &'a mut Shield,
    pub aura: &'a mut ElementAura,
    pub statuses: &'a mut StatusBoard,
}

pub(crate) fn apply_wind_effect(
    skill: &SkillDef,
    attacker_side: Side,
    target_side: Side,
    attacker_stats: &mut Stats,
    attacker_shield: &mut Shield,
    attacker_statuses: &mut StatusBoard,
    front_target: WindSpreadTarget<'_>,
    back_target_a: Option<WindSpreadTarget<'_>>,
    back_target_b: Option<WindSpreadTarget<'_>>,
    pending_boosts: &mut PendingBoosts,
    formula_rules: &BattleFormulaRules,
    accuracy_rng: &mut AccuracyRng,
    element_db: &ElementDb,
    status_db: &StatusDb,
    reaction_db: &ReactionDb,
    event_writer: &mut MessageWriter<BattleEvent>,
    mut formula_writer: Option<&mut MessageWriter<BattleFormulaEvent>>,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
    ctx: Option<&mut EffectResolutionContext>,
) {
    let WindSpreadTarget {
        base_element: front_base_element,
        stats: front_stats,
        shield: front_shield,
        aura: front_aura,
        statuses: front_statuses,
    } = front_target;
    let target_had_aura = front_aura.elements();
    let target_had_statuses = front_statuses
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    let target_had_shield = front_shield.0 > 0;

    if !attack_hits(
        skill,
        attacker_side,
        target_side,
        attacker_stats,
        front_stats,
        front_statuses,
        formula_rules,
        accuracy_rng,
        event_writer,
        formula_writer.as_deref_mut(),
        status_writer.as_deref_mut(),
        structured_log.as_deref_mut(),
        round,
    ) {
        if let Some(ctx) = ctx {
            ctx.target_had_aura = target_had_aura;
            ctx.target_had_statuses = target_had_statuses;
            ctx.target_had_shield = target_had_shield;
            ctx.last_hp_damage = 0;
            ctx.target_fainted = false;
            ctx.last_reaction_name = None;
            ctx.last_wind_spread_succeeded = false;
            ctx.last_attack_resolved = false;

            if let SkillEffect::Sequence { effects } = &skill.effect {
                for nested_effect in effects.iter().skip(1) {
                    let nested_skill = SkillDef {
                        effect: nested_effect.clone(),
                        ..skill.clone()
                    };
                    apply_effect_with_context(
                        &nested_skill,
                        attacker_side,
                        target_side,
                        front_base_element,
                        attacker_stats,
                        attacker_shield,
                        attacker_statuses,
                        front_stats,
                        front_shield,
                        front_aura,
                        front_statuses,
                        pending_boosts,
                        formula_rules,
                        accuracy_rng,
                        element_db,
                        status_db,
                        reaction_db,
                        event_writer,
                        formula_writer.as_deref_mut(),
                        status_writer.as_deref_mut(),
                        structured_log.as_deref_mut(),
                        round,
                        ctx,
                    );
                }
            }
        }
        return;
    }

    let Some(SkillEffect::Attack { power, .. }) = primary_attack_effect(&skill.effect) else {
        return;
    };

    let mut boosted_power = *power;
    if attacker_side == Side::Player {
        boosted_power += pending_boosts.player.next_attack_bonus;
        pending_boosts.player.next_attack_bonus = 0;
    } else {
        boosted_power += pending_boosts.enemy.next_attack_bonus;
        pending_boosts.enemy.next_attack_bonus = 0;
    }

    let (actual_atk, actual_def, raw_damage, multiplier) =
        scaled_damage_after_defense(boosted_power, attacker_stats, front_stats, formula_rules);
    let defender_element = front_aura.primary().unwrap_or(front_base_element);
    let effectiveness = element_db.get_effectiveness(ElementType::Wind, defender_element);
    let direct_damage = final_damage(raw_damage, effectiveness, formula_rules);
    let (direct_absorbed, direct_hp_damage) = apply_damage_with_shield(
        attacker_side,
        target_side,
        direct_damage,
        front_stats,
        front_shield,
        event_writer,
    );

    if let Some(round) = round {
        if let Some(writer) = formula_writer.as_deref_mut() {
            writer.write(formula_event(
                round,
                attacker_side,
                target_side,
                "wind_attack_direct",
                format!(
                    "atk_base={} atk_stage={} atk_effective={} def_base={} def_stage={} def_effective={} skill_power={} multiplier={:.2} raw_damage={} effectiveness={:.2} direct_damage={} absorbed={} hp_damage={} target_hp={} target_shield={}",
                    attacker_stats.atk,
                    attacker_stats.atk_stage,
                    actual_atk,
                    front_stats.def,
                    front_stats.def_stage,
                    actual_def,
                    boosted_power,
                    multiplier,
                    raw_damage,
                    effectiveness,
                    direct_damage,
                    direct_absorbed,
                    direct_hp_damage,
                    front_stats.hp,
                    front_shield.0
                ),
            ));
        }
    }

    let Some(spread_element) = select_spread_element(front_aura, front_statuses) else {
        if let Some(ctx) = ctx {
            ctx.target_had_aura = target_had_aura;
            ctx.target_had_statuses = target_had_statuses;
            ctx.target_had_shield = target_had_shield;
            ctx.last_hp_damage = direct_hp_damage;
            ctx.target_fainted = front_stats.hp <= 0;
            ctx.last_reaction_name = None;
            ctx.last_attack_resolved = true;
            ctx.last_wind_spread_succeeded = false;

            if let SkillEffect::Sequence { effects } = &skill.effect {
                for nested_effect in effects.iter().skip(1) {
                    let nested_skill = SkillDef {
                        effect: nested_effect.clone(),
                        ..skill.clone()
                    };
                    apply_effect_with_context(
                        &nested_skill,
                        attacker_side,
                        target_side,
                        front_base_element,
                        attacker_stats,
                        attacker_shield,
                        attacker_statuses,
                        front_stats,
                        front_shield,
                        front_aura,
                        front_statuses,
                        pending_boosts,
                        formula_rules,
                        accuracy_rng,
                        element_db,
                        status_db,
                        reaction_db,
                        event_writer,
                        formula_writer.as_deref_mut(),
                        status_writer.as_deref_mut(),
                        structured_log.as_deref_mut(),
                        round,
                        ctx,
                    );
                }
            }
        }
        let current_aura = front_aura.elements();
        event_writer.write(BattleEvent::WindSpreadSkipped {
            source: attacker_side,
            target: target_side,
            aura: current_aura.clone(),
        });
        if let Some(round) = round {
            if let Some(writer) = formula_writer.as_deref_mut() {
                writer.write(formula_event(
                    round,
                    attacker_side,
                    target_side,
                    "wind_spread_skipped",
                    format!(
                        "aura={:?} statuses={:?}",
                        current_aura,
                        front_statuses
                            .entries
                            .iter()
                            .map(|entry| entry.id.as_str())
                            .collect::<Vec<_>>()
                    ),
                ));
            }
            if let Some(log) = structured_log.as_deref_mut() {
                note_action_phase(
                    log,
                    round,
                    attacker_side,
                    "风扩散",
                    format!("未触发扩散；当前前场附着={:?}", front_aura.elements()),
                );
            }
        }
        return;
    };

    event_writer.write(BattleEvent::WindSpreadTriggered {
        source: attacker_side,
        target: target_side,
        element: spread_element,
    });

    if let Some(status_id) = aura_status_id(spread_element) {
        let _ = apply_status_by_id(
            status_id,
            attacker_side,
            target_side,
            front_stats,
            front_statuses,
            status_db,
            status_writer.as_deref_mut(),
            structured_log.as_deref_mut(),
            round,
        );
    }

    let front_spread_damage = spread_damage(spread_element, true);
    let (front_spread_absorbed, front_spread_hp_damage) = apply_damage_with_shield(
        attacker_side,
        target_side,
        front_spread_damage,
        front_stats,
        front_shield,
        event_writer,
    );

    if let Some(round) = round {
        if let Some(writer) = formula_writer.as_deref_mut() {
            writer.write(formula_event(
                round,
                attacker_side,
                target_side,
                "wind_spread_front",
                format!(
                    "spread_element={:?} spread_damage={} absorbed={} hp_damage={} target_hp={} target_shield={}",
                    spread_element,
                    front_spread_damage,
                    front_spread_absorbed,
                    front_spread_hp_damage,
                    front_stats.hp,
                    front_shield.0
                ),
            ));
        }
        if let Some(log) = structured_log.as_deref_mut() {
            note_action_phase(
                log,
                round,
                attacker_side,
                "风扩散",
                format!(
                    "扩散元素={:?}；前场扩散伤害={}；护盾吸收={}；生命伤害={}",
                    spread_element,
                    front_spread_damage,
                    front_spread_absorbed,
                    front_spread_hp_damage
                ),
            );
        }
    }

    let mut spread_reaction_name = None;
    for back_target in [back_target_a, back_target_b] {
        let Some(WindSpreadTarget {
            base_element,
            stats,
            shield,
            aura,
            statuses,
        }) = back_target
        else {
            continue;
        };

        let back_damage = spread_damage(spread_element, false);
        let (spread_absorbed, spread_hp_damage) = apply_damage_with_shield(
            attacker_side,
            target_side,
            back_damage,
            stats,
            shield,
            event_writer,
        );
        let attachment = resolve_element_attachment_only(
            spread_element,
            attacker_side,
            target_side,
            attacker_stats,
            attacker_statuses,
            stats,
            shield,
            aura,
            statuses,
            status_db,
            reaction_db,
            event_writer,
            formula_writer.as_deref_mut(),
            status_writer.as_deref_mut(),
            structured_log.as_deref_mut(),
            round,
            "wind_spread_attachment",
        );
        if spread_reaction_name.is_none() {
            spread_reaction_name = attachment.reaction_name.clone();
        }

        if let Some(round) = round {
            if let Some(writer) = formula_writer.as_deref_mut() {
                writer.write(formula_event(
                    round,
                    attacker_side,
                    target_side,
                    "wind_spread_backline",
                    format!(
                        "spread_element={:?} base_element={:?} spread_damage={} absorbed={} hp_damage={} aura_from={:?} aura_to={:?} reaction={:?} reaction_absorbed={} reaction_hp_damage={} target_hp={} target_shield={}",
                        spread_element,
                        base_element,
                        back_damage,
                        spread_absorbed,
                        spread_hp_damage,
                        attachment.aura_from,
                        attachment.aura_to,
                        attachment.reaction_name,
                        attachment.reaction_absorbed,
                        attachment.reaction_hp_damage,
                        stats.hp,
                        shield.0
                    ),
                ));
            }
        }
    }

    if let Some(ctx) = ctx {
        ctx.target_had_aura = target_had_aura;
        ctx.target_had_statuses = target_had_statuses;
        ctx.target_had_shield = target_had_shield;
        ctx.last_hp_damage = direct_hp_damage + front_spread_hp_damage;
        ctx.target_fainted = front_stats.hp <= 0;
        ctx.last_reaction_name = spread_reaction_name;
        ctx.last_wind_spread_succeeded = true;
        ctx.last_attack_resolved = true;

        if let SkillEffect::Sequence { effects } = &skill.effect {
            for nested_effect in effects.iter().skip(1) {
                let nested_skill = SkillDef {
                    effect: nested_effect.clone(),
                    ..skill.clone()
                };
                apply_effect_with_context(
                    &nested_skill,
                    attacker_side,
                    target_side,
                    front_base_element,
                    attacker_stats,
                    attacker_shield,
                    attacker_statuses,
                    front_stats,
                    front_shield,
                    front_aura,
                    front_statuses,
                    pending_boosts,
                    formula_rules,
                    accuracy_rng,
                    element_db,
                    status_db,
                    reaction_db,
                    event_writer,
                    formula_writer.as_deref_mut(),
                    status_writer.as_deref_mut(),
                    structured_log.as_deref_mut(),
                    round,
                    ctx,
                );
            }
        }
    }

    let _ = attacker_shield;
}

pub(crate) fn apply_self_effect(
    skill: &SkillDef,
    actor_side: Side,
    actor_stats: &mut Stats,
    actor_shield: &mut Shield,
    actor_aura: &mut ElementAura,
    actor_statuses: &mut StatusBoard,
    pending_boosts: &mut PendingBoosts,
    formula_rules: &BattleFormulaRules,
    accuracy_rng: &mut AccuracyRng,
    element_db: &ElementDb,
    status_db: &StatusDb,
    reaction_db: &ReactionDb,
    event_writer: &mut MessageWriter<BattleEvent>,
    formula_writer: Option<&mut MessageWriter<BattleFormulaEvent>>,
    status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
) {
    let mut formula_writer = formula_writer;
    let mut status_writer = status_writer;
    let mut structured_log = structured_log;
    let mut ctx = EffectResolutionContext {
        target_had_aura: actor_aura.elements(),
        target_had_statuses: actor_statuses
            .entries
            .iter()
            .map(|entry| entry.id.clone())
            .collect(),
        target_had_shield: actor_shield.0 > 0,
        ..Default::default()
    };
    apply_self_skill_effect_with_context(
        &skill.effect,
        skill,
        actor_side,
        actor_stats,
        actor_shield,
        actor_aura,
        actor_statuses,
        pending_boosts,
        formula_rules,
        accuracy_rng,
        element_db,
        status_db,
        reaction_db,
        event_writer,
        formula_writer.as_deref_mut(),
        status_writer.as_deref_mut(),
        structured_log.as_deref_mut(),
        round,
        &mut ctx,
    );
}

fn apply_self_skill_effect_with_context(
    effect: &SkillEffect,
    skill: &SkillDef,
    actor_side: Side,
    actor_stats: &mut Stats,
    actor_shield: &mut Shield,
    actor_aura: &mut ElementAura,
    actor_statuses: &mut StatusBoard,
    pending_boosts: &mut PendingBoosts,
    formula_rules: &BattleFormulaRules,
    accuracy_rng: &mut AccuracyRng,
    element_db: &ElementDb,
    status_db: &StatusDb,
    reaction_db: &ReactionDb,
    event_writer: &mut MessageWriter<BattleEvent>,
    mut formula_writer: Option<&mut MessageWriter<BattleFormulaEvent>>,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
    ctx: &mut EffectResolutionContext,
) {
    match effect {
        SkillEffect::Sequence { effects } => {
            for nested_effect in effects {
                apply_self_skill_effect_with_context(
                    nested_effect,
                    skill,
                    actor_side,
                    actor_stats,
                    actor_shield,
                    actor_aura,
                    actor_statuses,
                    pending_boosts,
                    formula_rules,
                    accuracy_rng,
                    element_db,
                    status_db,
                    reaction_db,
                    event_writer,
                    formula_writer.as_deref_mut(),
                    status_writer.as_deref_mut(),
                    structured_log.as_deref_mut(),
                    round,
                    ctx,
                );
            }
        }
        SkillEffect::Conditional { branches } => {
            for branch in branches {
                if evaluate_condition(&branch.condition, ctx) {
                    apply_self_skill_effect_with_context(
                        &branch.effect,
                        skill,
                        actor_side,
                        actor_stats,
                        actor_shield,
                        actor_aura,
                        actor_statuses,
                        pending_boosts,
                        formula_rules,
                        accuracy_rng,
                        element_db,
                        status_db,
                        reaction_db,
                        event_writer,
                        formula_writer.as_deref_mut(),
                        status_writer.as_deref_mut(),
                        structured_log.as_deref_mut(),
                        round,
                        ctx,
                    );
                }
            }
        }
        SkillEffect::ModifyStages {
            modifiers,
            duration_turns,
            target,
        } => {
            let target_for_stages =
                effect_target_side(*target, SkillTargetMode::SelfOnly, actor_side, actor_side);
            apply_stage_modifiers_to_target(
                modifiers,
                *duration_turns,
                actor_side,
                target_for_stages,
                actor_stats,
                actor_statuses,
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
            )
        }
        SkillEffect::Heal { amount } => {
            let mut heal_amount = *amount;
            if actor_side == Side::Player {
                heal_amount += pending_boosts.player.next_heal_bonus;
                pending_boosts.player.next_heal_bonus = 0;
            } else {
                heal_amount += pending_boosts.enemy.next_heal_bonus;
                pending_boosts.enemy.next_heal_bonus = 0;
            }
            let raw_heal_amount = heal_amount;
            let heal_multiplier = actor_statuses.heal_taken_multiplier();
            heal_amount = ((heal_amount as f32) * heal_multiplier).round() as i32;
            let before = actor_stats.hp;
            actor_stats.hp = (actor_stats.hp + heal_amount).min(actor_stats.max_hp);
            let actual_heal = actor_stats.hp - before;
            event_writer.write(BattleEvent::Healed {
                side: actor_side,
                amount: actual_heal,
            });
            if let Some(round) = round {
                if let Some(writer) = formula_writer.as_deref_mut() {
                    writer.write(formula_event(
                        round,
                        actor_side,
                        actor_side,
                        "heal",
                        format!(
                            "raw_heal={} heal_multiplier={:.2} final_heal={} actual_heal={} hp={}/{}",
                            raw_heal_amount,
                            heal_multiplier,
                            heal_amount,
                            actual_heal,
                            actor_stats.hp,
                            actor_stats.max_hp
                        ),
                    ));
                }
                if let Some(log) = structured_log.as_deref_mut() {
                    note_action_phase(
                        log,
                        round,
                        actor_side,
                        "治疗结算",
                        format!(
                            "基础治疗={}；治疗修正={:.2}；最终治疗={}；实际回复={}；当前HP={}/{}",
                            raw_heal_amount,
                            heal_multiplier,
                            heal_amount,
                            actual_heal,
                            actor_stats.hp,
                            actor_stats.max_hp
                        ),
                    );
                }
            }
        }
        SkillEffect::Shield { amount } => {
            let mut shield_amount = *amount;
            if actor_side == Side::Player {
                shield_amount += pending_boosts.player.next_shield_bonus;
                pending_boosts.player.next_shield_bonus = 0;
            } else {
                shield_amount += pending_boosts.enemy.next_shield_bonus;
                pending_boosts.enemy.next_shield_bonus = 0;
            }

            actor_shield.0 += shield_amount;
            event_writer.write(BattleEvent::ShieldGained {
                side: actor_side,
                amount: shield_amount,
            });
            if let Some(round) = round {
                if let Some(writer) = formula_writer.as_deref_mut() {
                    writer.write(formula_event(
                        round,
                        actor_side,
                        actor_side,
                        "shield_gain",
                        format!(
                            "shield_amount={} current_shield={}",
                            shield_amount, actor_shield.0
                        ),
                    ));
                }
                if let Some(log) = structured_log.as_deref_mut() {
                    note_action_phase(
                        log,
                        round,
                        actor_side,
                        "护盾结算",
                        format!("获得护盾={}；当前护盾={}", shield_amount, actor_shield.0),
                    );
                }
            }
        }
        SkillEffect::ApplyStatus { status_id } => {
            let _ = apply_status_by_id(
                status_id,
                actor_side,
                actor_side,
                actor_stats,
                actor_statuses,
                status_db,
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
            );
        }
        SkillEffect::Cleanse {
            prefer_aura,
            fallback_to_debuff,
            amount,
        } => {
            let mut removed_any = false;
            for _ in 0..*amount {
                if *prefer_aura {
                    if clear_one_aura(
                        actor_side,
                        actor_stats,
                        actor_aura,
                        actor_statuses,
                        status_writer.as_deref_mut(),
                        structured_log.as_deref_mut(),
                        round,
                    )
                    .is_some()
                    {
                        removed_any = true;
                        continue;
                    }
                }
                if *fallback_to_debuff
                    && clear_one_debuff_status(
                        actor_side,
                        actor_stats,
                        actor_statuses,
                        status_writer.as_deref_mut(),
                        structured_log.as_deref_mut(),
                        round,
                    )
                    .is_some()
                {
                    removed_any = true;
                    continue;
                }
            }
            ctx.last_cleanse_succeeded = removed_any;
        }
        SkillEffect::Dispel { status_ids, target } => {
            let target_for_dispel =
                effect_target_side(*target, skill_target_mode(skill), actor_side, actor_side);
            let removed_count = dispel_statuses(
                status_ids,
                actor_side,
                target_for_dispel,
                actor_stats,
                actor_statuses,
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
            );
            ctx.last_cleanse_succeeded = removed_count > 0;
        }
        SkillEffect::DealFixedDamage {
            amount,
            ignore_shield,
            target,
        } => {
            let target_for_damage =
                effect_target_side(*target, skill_target_mode(skill), actor_side, actor_side);
            let (absorbed, hp_damage) = apply_fixed_damage(
                actor_side,
                target_for_damage,
                *amount,
                *ignore_shield,
                actor_stats,
                actor_shield,
                event_writer,
            );
            ctx.last_hp_damage = hp_damage;
            ctx.target_fainted = actor_stats.hp <= 0;
            if let Some(round) = round {
                if let Some(writer) = formula_writer.as_deref_mut() {
                    writer.write(formula_event(
                        round,
                        actor_side,
                        target_for_damage,
                        "fixed_damage",
                        format!(
                            "amount={} ignore_shield={} absorbed={} hp_damage={} target_hp={} target_shield={}",
                            amount,
                            ignore_shield,
                            absorbed,
                            hp_damage,
                            actor_stats.hp,
                            actor_shield.0
                        ),
                    ));
                }
            }
        }
        SkillEffect::DealStatDifferenceDamage {
            source_attribute,
            target_attribute,
            multiply_by_source_stage,
            ignore_shield,
            target,
        } => {
            let target_for_damage =
                effect_target_side(*target, skill_target_mode(skill), actor_side, actor_side);
            let source_value =
                effective_attribute_value(actor_stats, *source_attribute, formula_rules);
            let target_value =
                effective_attribute_value(actor_stats, *target_attribute, formula_rules);
            let amount = stat_difference_damage(
                actor_stats,
                actor_stats,
                *source_attribute,
                *target_attribute,
                *multiply_by_source_stage,
                formula_rules,
            );
            let (absorbed, hp_damage) = apply_fixed_damage(
                actor_side,
                target_for_damage,
                amount,
                *ignore_shield,
                actor_stats,
                actor_shield,
                event_writer,
            );
            ctx.last_hp_damage = hp_damage;
            ctx.target_fainted = actor_stats.hp <= 0;
            if let Some(round) = round {
                if let Some(writer) = formula_writer.as_deref_mut() {
                    writer.write(formula_event(
                        round,
                        actor_side,
                        target_for_damage,
                        "stat_difference_damage",
                        format!(
                            "source_attribute={:?} source_value={} target_attribute={:?} target_value={} multiply_by_source_stage={} amount={} ignore_shield={} absorbed={} hp_damage={} target_hp={} target_shield={}",
                            source_attribute,
                            source_value,
                            target_attribute,
                            target_value,
                            multiply_by_source_stage,
                            amount,
                            ignore_shield,
                            absorbed,
                            hp_damage,
                            actor_stats.hp,
                            actor_shield.0
                        ),
                    ));
                }
            }
        }
        SkillEffect::Attack { .. } => {
            let _ = (skill, formula_rules, accuracy_rng, element_db, reaction_db);
        }
    }
}

pub(crate) fn apply_effect(
    skill: &SkillDef,
    attacker_side: Side,
    target_side: Side,
    target_element: crate::data::ElementType,
    attacker_stats: &mut Stats,
    attacker_shield: &mut Shield,
    attacker_statuses: &mut StatusBoard,
    target_stats: &mut Stats,
    target_shield: &mut Shield,
    target_aura: &mut ElementAura,
    target_statuses: &mut StatusBoard,
    pending_boosts: &mut PendingBoosts,
    formula_rules: &BattleFormulaRules,
    accuracy_rng: &mut AccuracyRng,
    element_db: &ElementDb,
    status_db: &StatusDb,
    reaction_db: &ReactionDb,
    event_writer: &mut MessageWriter<BattleEvent>,
    formula_writer: Option<&mut MessageWriter<BattleFormulaEvent>>,
    status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
) {
    let mut formula_writer = formula_writer;
    let mut status_writer = status_writer;
    let mut ctx = EffectResolutionContext {
        target_had_aura: target_aura.elements(),
        target_had_statuses: target_statuses
            .entries
            .iter()
            .map(|entry| entry.id.clone())
            .collect(),
        target_had_shield: target_shield.0 > 0,
        ..Default::default()
    };

    apply_effect_with_context(
        skill,
        attacker_side,
        target_side,
        target_element,
        attacker_stats,
        attacker_shield,
        attacker_statuses,
        target_stats,
        target_shield,
        target_aura,
        target_statuses,
        pending_boosts,
        formula_rules,
        accuracy_rng,
        element_db,
        status_db,
        reaction_db,
        event_writer,
        formula_writer.as_deref_mut(),
        status_writer.as_deref_mut(),
        structured_log.as_deref_mut(),
        round,
        &mut ctx,
    );
}

fn apply_effect_with_context(
    skill: &SkillDef,
    attacker_side: Side,
    target_side: Side,
    target_element: crate::data::ElementType,
    attacker_stats: &mut Stats,
    attacker_shield: &mut Shield,
    attacker_statuses: &mut StatusBoard,
    target_stats: &mut Stats,
    target_shield: &mut Shield,
    target_aura: &mut ElementAura,
    target_statuses: &mut StatusBoard,
    pending_boosts: &mut PendingBoosts,
    formula_rules: &BattleFormulaRules,
    accuracy_rng: &mut AccuracyRng,
    element_db: &ElementDb,
    status_db: &StatusDb,
    reaction_db: &ReactionDb,
    event_writer: &mut MessageWriter<BattleEvent>,
    mut formula_writer: Option<&mut MessageWriter<BattleFormulaEvent>>,
    mut status_writer: Option<&mut MessageWriter<BattleStatusEvent>>,
    mut structured_log: Option<&mut StructuredBattleLog>,
    round: Option<u32>,
    ctx: &mut EffectResolutionContext,
) {
    match &skill.effect {
        SkillEffect::Sequence { effects } => {
            for (index, nested_effect) in effects.iter().enumerate() {
                let nested_skill = SkillDef {
                    effect: nested_effect.clone(),
                    ..skill.clone()
                };
                apply_effect_with_context(
                    &nested_skill,
                    attacker_side,
                    target_side,
                    target_element,
                    attacker_stats,
                    attacker_shield,
                    attacker_statuses,
                    target_stats,
                    target_shield,
                    target_aura,
                    target_statuses,
                    pending_boosts,
                    formula_rules,
                    accuracy_rng,
                    element_db,
                    status_db,
                    reaction_db,
                    event_writer,
                    formula_writer.as_deref_mut(),
                    status_writer.as_deref_mut(),
                    structured_log.as_deref_mut(),
                    round,
                    ctx,
                );
                if index == 0
                    && matches!(nested_effect, SkillEffect::Attack { .. })
                    && !ctx.last_attack_resolved
                {
                    break;
                }
            }
        }
        SkillEffect::Conditional { branches } => {
            for branch in branches {
                if evaluate_condition(&branch.condition, ctx) {
                    let nested_skill = SkillDef {
                        effect: (*branch.effect).clone(),
                        ..skill.clone()
                    };
                    apply_effect_with_context(
                        &nested_skill,
                        attacker_side,
                        target_side,
                        target_element,
                        attacker_stats,
                        attacker_shield,
                        attacker_statuses,
                        target_stats,
                        target_shield,
                        target_aura,
                        target_statuses,
                        pending_boosts,
                        formula_rules,
                        accuracy_rng,
                        element_db,
                        status_db,
                        reaction_db,
                        event_writer,
                        formula_writer.as_deref_mut(),
                        status_writer.as_deref_mut(),
                        structured_log.as_deref_mut(),
                        round,
                        ctx,
                    );
                }
            }
        }
        SkillEffect::ModifyStages {
            modifiers,
            duration_turns,
            target,
        } => {
            let target_for_stages = effect_target_side(
                *target,
                skill_target_mode(skill),
                attacker_side,
                target_side,
            );
            let (stats, statuses) = if target_for_stages == attacker_side {
                (attacker_stats, attacker_statuses)
            } else {
                (target_stats, target_statuses)
            };
            apply_stage_modifiers_to_target(
                modifiers,
                *duration_turns,
                attacker_side,
                target_for_stages,
                stats,
                statuses,
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
            );
        }
        SkillEffect::ApplyStatus { status_id } => {
            let target_for_status = if matches!(skill_target_mode(skill), SkillTargetMode::SelfOnly)
            {
                attacker_side
            } else {
                target_side
            };
            if target_for_status != attacker_side && !ctx.last_attack_resolved {
                if !attack_hits(
                    skill,
                    attacker_side,
                    target_for_status,
                    attacker_stats,
                    target_stats,
                    target_statuses,
                    formula_rules,
                    accuracy_rng,
                    event_writer,
                    formula_writer.as_deref_mut(),
                    status_writer.as_deref_mut(),
                    structured_log.as_deref_mut(),
                    round,
                ) {
                    ctx.last_attack_resolved = false;
                    return;
                }
            }
            let (stats, statuses) = if target_for_status == attacker_side {
                (attacker_stats, attacker_statuses)
            } else {
                (target_stats, target_statuses)
            };
            let _ = apply_status_by_id(
                status_id,
                attacker_side,
                target_for_status,
                stats,
                statuses,
                status_db,
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
            );
            ctx.last_attack_resolved = true;
        }
        SkillEffect::Cleanse {
            prefer_aura,
            fallback_to_debuff,
            amount,
        } => {
            let mut removed_any = false;
            for _ in 0..*amount {
                if *prefer_aura
                    && clear_one_aura(
                        target_side,
                        target_stats,
                        target_aura,
                        target_statuses,
                        status_writer.as_deref_mut(),
                        structured_log.as_deref_mut(),
                        round,
                    )
                    .is_some()
                {
                    removed_any = true;
                    continue;
                }
                if *fallback_to_debuff
                    && clear_one_debuff_status(
                        target_side,
                        target_stats,
                        target_statuses,
                        status_writer.as_deref_mut(),
                        structured_log.as_deref_mut(),
                        round,
                    )
                    .is_some()
                {
                    removed_any = true;
                }
            }
            ctx.last_cleanse_succeeded = removed_any;
        }
        SkillEffect::Dispel { status_ids, target } => {
            let target_for_dispel = effect_target_side(
                *target,
                skill_target_mode(skill),
                attacker_side,
                target_side,
            );
            let (stats, statuses) = if target_for_dispel == attacker_side {
                (attacker_stats, attacker_statuses)
            } else {
                (target_stats, target_statuses)
            };
            let removed_count = dispel_statuses(
                status_ids,
                attacker_side,
                target_for_dispel,
                stats,
                statuses,
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
            );
            ctx.last_cleanse_succeeded = removed_count > 0;
        }
        SkillEffect::DealFixedDamage {
            amount,
            ignore_shield,
            target,
        } => {
            let target_for_damage = effect_target_side(
                *target,
                skill_target_mode(skill),
                attacker_side,
                target_side,
            );
            let (stats, shield) = if target_for_damage == attacker_side {
                (attacker_stats, attacker_shield)
            } else {
                (target_stats, target_shield)
            };
            let (absorbed, hp_damage) = apply_fixed_damage(
                attacker_side,
                target_for_damage,
                *amount,
                *ignore_shield,
                stats,
                shield,
                event_writer,
            );
            ctx.last_hp_damage = hp_damage;
            ctx.target_fainted = stats.hp <= 0;
            if let Some(round) = round {
                if let Some(writer) = formula_writer.as_deref_mut() {
                    writer.write(formula_event(
                        round,
                        attacker_side,
                        target_for_damage,
                        "fixed_damage",
                        format!(
                            "amount={} ignore_shield={} absorbed={} hp_damage={} target_hp={} target_shield={}",
                            amount,
                            ignore_shield,
                            absorbed,
                            hp_damage,
                            stats.hp,
                            shield.0
                        ),
                    ));
                }
            }
        }
        SkillEffect::DealStatDifferenceDamage {
            source_attribute,
            target_attribute,
            multiply_by_source_stage,
            ignore_shield,
            target,
        } => {
            let target_for_damage = effect_target_side(
                *target,
                skill_target_mode(skill),
                attacker_side,
                target_side,
            );
            if target_for_damage == attacker_side {
                let source_stats_snapshot = *attacker_stats;
                let source_value = effective_attribute_value(
                    &source_stats_snapshot,
                    *source_attribute,
                    formula_rules,
                );
                let target_value =
                    effective_attribute_value(attacker_stats, *target_attribute, formula_rules);
                let (amount, absorbed, hp_damage) = apply_stat_difference_damage(
                    attacker_side,
                    target_for_damage,
                    *source_attribute,
                    *target_attribute,
                    *multiply_by_source_stage,
                    *ignore_shield,
                    &source_stats_snapshot,
                    attacker_stats,
                    attacker_shield,
                    formula_rules,
                    event_writer,
                );
                ctx.last_hp_damage = hp_damage;
                ctx.target_fainted = attacker_stats.hp <= 0;
                if let Some(round) = round {
                    if let Some(writer) = formula_writer.as_deref_mut() {
                        writer.write(formula_event(
                            round,
                            attacker_side,
                            target_for_damage,
                            "stat_difference_damage",
                            format!(
                                "source_attribute={:?} source_value={} target_attribute={:?} target_value={} multiply_by_source_stage={} amount={} ignore_shield={} absorbed={} hp_damage={} target_hp={} target_shield={}",
                                source_attribute,
                                source_value,
                                target_attribute,
                                target_value,
                                multiply_by_source_stage,
                                amount,
                                ignore_shield,
                                absorbed,
                                hp_damage,
                                attacker_stats.hp,
                                attacker_shield.0
                            ),
                        ));
                    }
                    if let Some(log) = structured_log.as_deref_mut() {
                        note_action_phase(
                            log,
                            round,
                            attacker_side,
                            "固定伤害结算",
                            format!(
                                "按属性差造成固定伤害；来源{:?}={}；目标{:?}={}；是否受施法者等级修正={}；最终固定伤害={}；护盾吸收={}；生命伤害={}；目标剩余HP={}；目标剩余护盾={}",
                                source_attribute,
                                source_value,
                                target_attribute,
                                target_value,
                                multiply_by_source_stage,
                                amount,
                                absorbed,
                                hp_damage,
                                attacker_stats.hp,
                                attacker_shield.0
                            ),
                        );
                    }
                }
            } else {
                let source_value =
                    effective_attribute_value(attacker_stats, *source_attribute, formula_rules);
                let target_value =
                    effective_attribute_value(target_stats, *target_attribute, formula_rules);
                let (amount, absorbed, hp_damage) = apply_stat_difference_damage(
                    attacker_side,
                    target_for_damage,
                    *source_attribute,
                    *target_attribute,
                    *multiply_by_source_stage,
                    *ignore_shield,
                    attacker_stats,
                    target_stats,
                    target_shield,
                    formula_rules,
                    event_writer,
                );
                ctx.last_hp_damage = hp_damage;
                ctx.target_fainted = target_stats.hp <= 0;
                if let Some(round) = round {
                    if let Some(writer) = formula_writer.as_deref_mut() {
                        writer.write(formula_event(
                            round,
                            attacker_side,
                            target_for_damage,
                            "stat_difference_damage",
                            format!(
                                "source_attribute={:?} source_value={} target_attribute={:?} target_value={} multiply_by_source_stage={} amount={} ignore_shield={} absorbed={} hp_damage={} target_hp={} target_shield={}",
                                source_attribute,
                                source_value,
                                target_attribute,
                                target_value,
                                multiply_by_source_stage,
                                amount,
                                ignore_shield,
                                absorbed,
                                hp_damage,
                                target_stats.hp,
                                target_shield.0
                            ),
                        ));
                    }
                    if let Some(log) = structured_log.as_deref_mut() {
                        note_action_phase(
                            log,
                            round,
                            attacker_side,
                            "固定伤害结算",
                            format!(
                                "按属性差造成固定伤害；来源{:?}={}；目标{:?}={}；是否受施法者等级修正={}；最终固定伤害={}；护盾吸收={}；生命伤害={}；目标剩余HP={}；目标剩余护盾={}",
                                source_attribute,
                                source_value,
                                target_attribute,
                                target_value,
                                multiply_by_source_stage,
                                amount,
                                absorbed,
                                hp_damage,
                                target_stats.hp,
                                target_shield.0
                            ),
                        );
                    }
                }
            }
        }
        SkillEffect::Attack {
            power,
            lifesteal_ratio,
            ignore_shield,
        } => {
            let target_had_aura = target_aura.elements();
            let target_had_statuses = target_statuses
                .entries
                .iter()
                .map(|entry| entry.id.clone())
                .collect::<Vec<_>>();
            let target_had_shield = target_shield.0 > 0;

            if !attack_hits(
                skill,
                attacker_side,
                target_side,
                attacker_stats,
                target_stats,
                target_statuses,
                formula_rules,
                accuracy_rng,
                event_writer,
                formula_writer.as_deref_mut(),
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
            ) {
                ctx.target_had_aura = target_had_aura;
                ctx.target_had_statuses = target_had_statuses;
                ctx.target_had_shield = target_had_shield;
                ctx.last_hp_damage = 0;
                ctx.target_fainted = false;
                ctx.last_reaction_name = None;
                ctx.last_attack_resolved = false;
                return;
            }

            let mut boosted_power = *power;
            if attacker_side == Side::Player {
                boosted_power += pending_boosts.player.next_attack_bonus;
                pending_boosts.player.next_attack_bonus = 0;
            } else {
                boosted_power += pending_boosts.enemy.next_attack_bonus;
                pending_boosts.enemy.next_attack_bonus = 0;
            }

            let (actual_atk, actual_def, raw_damage, multiplier) = scaled_damage_after_defense(
                boosted_power,
                attacker_stats,
                target_stats,
                formula_rules,
            );

            if let Some(incoming_element) = skill.element {
                let defender_elem_with_aura = target_aura.primary().unwrap_or(target_element);
                let effectiveness_with_aura =
                    element_db.get_effectiveness(incoming_element, defender_elem_with_aura);
                let theoretical_damage_with_aura =
                    final_damage(raw_damage, effectiveness_with_aura, formula_rules);
                let shield_blocks_attachment =
                    !*ignore_shield && target_shield.0.min(theoretical_damage_with_aura) > 0;

                if shield_blocks_attachment {
                    let effectiveness_no_aura =
                        element_db.get_effectiveness(incoming_element, target_element);
                    let theoretical_damage_no_aura =
                        final_damage(raw_damage, effectiveness_no_aura, formula_rules);
                    let (absorbed, hp_damage) = apply_damage_with_shield(
                        attacker_side,
                        target_side,
                        theoretical_damage_no_aura,
                        target_stats,
                        target_shield,
                        event_writer,
                    );
                    ctx.target_had_aura = target_had_aura;
                    ctx.target_had_statuses = target_had_statuses;
                    ctx.target_had_shield = target_had_shield;
                    ctx.last_hp_damage = hp_damage;
                    ctx.target_fainted = target_stats.hp <= 0;
                    ctx.last_reaction_name = None;
                    ctx.last_attack_resolved = true;
                    if let Some(round) = round {
                        if let Some(writer) = formula_writer.as_deref_mut() {
                            writer.write(formula_event(
                                round,
                                attacker_side,
                                target_side,
                                "element_attack_shield_blocked",
                                format!(
                                    "atk_base={} atk_stage={} atk_effective={} def_base={} def_stage={} def_effective={} skill_power={} multiplier={:.2} raw_damage={} effectiveness_with_aura={:.2} effectiveness_base={:.2} damage={} absorbed={} hp_damage={} target_hp={} target_shield={}",
                                    attacker_stats.atk,
                                    attacker_stats.atk_stage,
                                    actual_atk,
                                    target_stats.def,
                                    target_stats.def_stage,
                                    actual_def,
                                    boosted_power,
                                    multiplier,
                                    raw_damage,
                                    effectiveness_with_aura,
                                    effectiveness_no_aura,
                                    theoretical_damage_no_aura,
                                    absorbed,
                                    hp_damage,
                                    target_stats.hp,
                                    target_shield.0
                                ),
                            ));
                        }
                        if let Some(log) = structured_log.as_deref_mut() {
                            note_action_phase(
                                log,
                                round,
                                attacker_side,
                                "伤害结算",
                                format!(
                                    "元素攻击命中护盾；实际Atk={}；实际Def={}；倍率系数={:.2}；基础伤害={}；固有元素倍率后伤害={}；护盾吸收={}；生命伤害={}；目标剩余HP={}；目标剩余护盾={}",
                                    actual_atk,
                                    actual_def,
                                    multiplier,
                                    raw_damage,
                                    theoretical_damage_no_aura,
                                    absorbed,
                                    hp_damage,
                                    target_stats.hp,
                                    target_shield.0
                                ),
                            );
                        }
                    }
                } else {
                    let from = target_aura.elements();
                    let (main_absorbed, main_hp_damage) = apply_fixed_damage(
                        attacker_side,
                        target_side,
                        theoretical_damage_with_aura,
                        *ignore_shield,
                        target_stats,
                        target_shield,
                        event_writer,
                    );

                    let mut aura_after = from.clone();
                    let mut reaction_id: Option<&str> = None;
                    let mut reaction_name: Option<&str> = None;
                    let mut reaction_required_elements: Vec<ElementType> = Vec::new();
                    let mut reaction_required_statuses: Vec<&str> = Vec::new();
                    let mut reaction_fixed_damage = 0;
                    let mut reaction_statuses_to_apply: Vec<&str> = Vec::new();
                    let mut reaction_statuses_to_clear: Vec<&str> = Vec::new();
                    let mut reaction_heal_amount = 0;
                    let mut evicted_aura: Option<ElementType> = None;

                    if let Some(round) = round {
                        if let Some(writer) = formula_writer.as_deref_mut() {
                            writer.write(formula_event(
                                round,
                                attacker_side,
                                target_side,
                                "element_attachment_attempt",
                                format!(
                                    "incoming_element={:?} aura_before={:?} target_base_element={:?} supports_attachment={} shield_blocks_attachment={} current_statuses={:?}",
                                    incoming_element,
                                    from,
                                    target_element,
                                    supports_normal_attachment(incoming_element),
                                    shield_blocks_attachment,
                                    target_statuses
                                        .entries
                                        .iter()
                                        .map(|entry| entry.id.as_str())
                                        .collect::<Vec<_>>()
                                ),
                            ));
                        }
                    }

                    if supports_normal_attachment(incoming_element) {
                        if let Some(reaction) = detect_element_reaction(
                            reaction_db,
                            &from,
                            target_statuses,
                            incoming_element,
                        ) {
                            reaction_id = Some(reaction.id.as_str());
                            reaction_name = Some(reaction.name.as_str());
                            reaction_required_elements = reaction.required_elements.clone();
                            reaction_required_statuses = reaction
                                .required_statuses
                                .iter()
                                .map(String::as_str)
                                .collect();
                            reaction_fixed_damage = reaction.fixed_damage;
                            reaction_heal_amount = reaction.heal_attacker;
                            reaction_statuses_to_apply =
                                reaction.apply_statuses.iter().map(String::as_str).collect();
                            reaction_statuses_to_clear =
                                reaction.clear_statuses.iter().map(String::as_str).collect();
                            aura_after = reaction.aura_results.clone();
                        } else {
                            aura_after = from.clone();
                            if !aura_after.contains(&incoming_element) {
                                if aura_after.len() >= 2 {
                                    evicted_aura = aura_after.first().copied();
                                    aura_after.remove(0);
                                }
                                aura_after.push(incoming_element);
                            }
                        }
                    }

                    for status_id in reaction_statuses_to_clear.iter().copied() {
                        let removed = clear_status_by_id(
                            status_id,
                            target_side,
                            target_stats,
                            target_statuses,
                            status_writer.as_deref_mut(),
                            structured_log.as_deref_mut(),
                            round,
                        );
                        if removed {
                            if let Some(round) = round {
                                if let Some(writer) = status_writer.as_deref_mut() {
                                    writer.write(status_event(
                                        round,
                                        target_side,
                                        status_id,
                                        "consumed_by_reaction",
                                        format!(
                                            "reaction_id={:?} reaction_name={:?} aura_before={:?}",
                                            reaction_id, reaction_name, from
                                        ),
                                    ));
                                }
                            }
                        }
                    }

                    if let Some(round) = round {
                        if let Some(writer) = formula_writer.as_deref_mut() {
                            writer.write(formula_event(
                                round,
                                attacker_side,
                                target_side,
                                "element_attachment_resolved",
                                format!(
                                    "incoming_element={:?} reaction_id={:?} reaction_name={:?} required_elements={:?} required_statuses={:?} statuses_to_clear={:?} statuses_to_apply={:?} evicted_aura={:?} aura_before={:?} aura_after_candidate={:?}",
                                    incoming_element,
                                    reaction_id,
                                    reaction_name,
                                    reaction_required_elements,
                                    reaction_required_statuses,
                                    reaction_statuses_to_clear,
                                    reaction_statuses_to_apply,
                                    evicted_aura,
                                    from,
                                    aura_after
                                ),
                            ));
                        }
                    }

                    target_aura.set_elements(&aura_after);
                    let aura_after = target_aura.elements();
                    sync_aura_status(
                        &aura_after,
                        attacker_side,
                        target_side,
                        target_stats,
                        target_statuses,
                        status_db,
                        status_writer.as_deref_mut(),
                        structured_log.as_deref_mut(),
                        round,
                    );

                    for status_id in reaction_statuses_to_apply {
                        let _ = apply_status_by_id(
                            status_id,
                            attacker_side,
                            target_side,
                            target_stats,
                            target_statuses,
                            status_db,
                            status_writer.as_deref_mut(),
                            structured_log.as_deref_mut(),
                            round,
                        );
                    }

                    let mut reaction_absorbed = 0;
                    let mut reaction_hp_damage = 0;
                    if reaction_fixed_damage > 0 {
                        (reaction_absorbed, reaction_hp_damage) = apply_damage_with_shield(
                            attacker_side,
                            target_side,
                            reaction_fixed_damage,
                            target_stats,
                            target_shield,
                            event_writer,
                        );
                    }

                    let mut reaction_final_heal = 0;
                    let mut reaction_heal_multiplier = 1.0;
                    let mut reaction_actual_heal = 0;
                    if reaction_heal_amount > 0 {
                        (
                            reaction_final_heal,
                            reaction_heal_multiplier,
                            reaction_actual_heal,
                        ) = heal_target(
                            attacker_side,
                            reaction_heal_amount,
                            attacker_stats,
                            attacker_statuses,
                            event_writer,
                        );
                    }

                    if from != aura_after {
                        event_writer.write(BattleEvent::ElementAuraApplied {
                            side: target_side,
                            from: from.clone(),
                            to: aura_after.clone(),
                            effectiveness: effectiveness_with_aura,
                        });
                        if let Some(round) = round {
                            if let Some(writer) = status_writer.as_deref_mut() {
                                writer.write(status_event(
                                    round,
                                    target_side,
                                    "element_aura",
                                    if aura_after.is_empty() {
                                        "cleared"
                                    } else {
                                        "applied"
                                    },
                                    format!(
                                        "from={:?} to={:?} effectiveness={:.2}",
                                        from, aura_after, effectiveness_with_aura
                                    ),
                                ));
                                writer.write(status_event(
                                    round,
                                    target_side,
                                    "element_aura_final",
                                    "resolved",
                                    format!(
                                        "incoming_element={:?} reaction_id={:?} reaction_name={:?} aura_before={:?} aura_after={:?} evicted_aura={:?}",
                                        incoming_element,
                                        reaction_id,
                                        reaction_name,
                                        from,
                                        aura_after,
                                        evicted_aura
                                    ),
                                ));
                            }
                        }
                    }

                    let total_hp_damage = main_hp_damage + reaction_hp_damage;
                    ctx.target_had_aura = target_had_aura;
                    ctx.target_had_statuses = target_had_statuses;
                    ctx.target_had_shield = target_had_shield;
                    ctx.last_hp_damage = total_hp_damage;
                    ctx.target_fainted = target_stats.hp <= 0;
                    ctx.last_reaction_name = reaction_name.map(str::to_string);
                    ctx.last_attack_resolved = true;

                    if let Some(ratio) = lifesteal_ratio {
                        if total_hp_damage > 0 {
                            let lifesteal_raw = ((total_hp_damage as f32) * *ratio).round() as i32;
                            let (lifesteal_final, lifesteal_multiplier, lifesteal_actual) =
                                heal_target(
                                    attacker_side,
                                    lifesteal_raw,
                                    attacker_stats,
                                    attacker_statuses,
                                    event_writer,
                                );
                            if let Some(round) = round {
                                if let Some(writer) = formula_writer.as_deref_mut() {
                                    writer.write(formula_event(
                                        round,
                                        attacker_side,
                                        attacker_side,
                                        "lifesteal",
                                        format!(
                                            "ratio={:.2} based_on_hp_damage={} raw_heal={} final_heal={} heal_multiplier={:.2} actual_heal={} attacker_hp={}/{}",
                                            ratio,
                                            total_hp_damage,
                                            lifesteal_raw,
                                            lifesteal_final,
                                            lifesteal_multiplier,
                                            lifesteal_actual,
                                            attacker_stats.hp,
                                            attacker_stats.max_hp
                                        ),
                                    ));
                                }
                            }
                        }
                    }

                    if let Some(name) = reaction_name {
                        emit_reaction(
                            name,
                            attacker_side,
                            target_side,
                            event_writer,
                            formula_writer.as_deref_mut(),
                            structured_log.as_deref_mut(),
                            round,
                            format!(
                                "incoming_element={:?} aura_before={:?} aura_after={:?} reaction_damage={} reaction_absorbed={} reaction_hp_damage={} reaction_heal_raw={} reaction_heal_final={} reaction_heal_multiplier={:.2} reaction_actual_heal={}",
                                incoming_element,
                                from,
                                aura_after,
                                reaction_fixed_damage,
                                reaction_absorbed,
                                reaction_hp_damage,
                                reaction_heal_amount,
                                reaction_final_heal,
                                reaction_heal_multiplier,
                                reaction_actual_heal
                            ),
                        );
                    }

                    if let Some(round) = round {
                        if let Some(writer) = formula_writer.as_deref_mut() {
                            writer.write(formula_event(
                                round,
                                attacker_side,
                                target_side,
                                if reaction_name.is_some() {
                                    "element_attack_reacted"
                                } else {
                                    "element_attack_applied"
                                },
                                format!(
                                    "atk_base={} atk_stage={} atk_effective={} def_base={} def_stage={} def_effective={} skill_power={} multiplier={:.2} raw_damage={} effectiveness={:.2} main_absorbed={} main_hp_damage={} reaction={:?} reaction_absorbed={} reaction_hp_damage={} target_hp={} target_shield={} aura_from={:?} aura_to={:?}",
                                    attacker_stats.atk,
                                    attacker_stats.atk_stage,
                                    actual_atk,
                                    target_stats.def,
                                    target_stats.def_stage,
                                    actual_def,
                                    boosted_power,
                                    multiplier,
                                    raw_damage,
                                    effectiveness_with_aura,
                                    main_absorbed,
                                    main_hp_damage,
                                    reaction_name,
                                    reaction_absorbed,
                                    reaction_hp_damage,
                                    target_stats.hp,
                                    target_shield.0,
                                    from,
                                    aura_after
                                ),
                            ));
                        }
                        if let Some(log) = structured_log.as_deref_mut() {
                            note_action_phase(
                                log,
                                round,
                                attacker_side,
                                "伤害结算",
                                format!(
                                    "元素攻击生效；实际Atk={}；实际Def={}；倍率系数={:.2}；基础伤害={}；元素倍率={:.2}；主伤害={}；反应={:?}；反应附加伤害={}；附着从 {:?} 变为 {:?}；目标剩余HP={}；目标剩余护盾={}",
                                    actual_atk,
                                    actual_def,
                                    multiplier,
                                    raw_damage,
                                    effectiveness_with_aura,
                                    main_hp_damage,
                                    reaction_name,
                                    reaction_hp_damage,
                                    from,
                                    aura_after,
                                    target_stats.hp,
                                    target_shield.0
                                ),
                            );
                        }
                    }
                }
            } else {
                let theoretical_damage = raw_damage;
                let (absorbed, hp_damage) = apply_fixed_damage(
                    attacker_side,
                    target_side,
                    theoretical_damage,
                    *ignore_shield,
                    target_stats,
                    target_shield,
                    event_writer,
                );
                ctx.target_had_aura = target_had_aura;
                ctx.target_had_statuses = target_had_statuses;
                ctx.target_had_shield = target_had_shield;
                ctx.last_hp_damage = hp_damage;
                ctx.target_fainted = target_stats.hp <= 0;
                ctx.last_reaction_name = None;
                ctx.last_attack_resolved = true;

                if let Some(ratio) = lifesteal_ratio {
                    if hp_damage > 0 {
                        let lifesteal_raw = ((hp_damage as f32) * *ratio).round() as i32;
                        let (lifesteal_final, lifesteal_multiplier, lifesteal_actual) = heal_target(
                            attacker_side,
                            lifesteal_raw,
                            attacker_stats,
                            attacker_statuses,
                            event_writer,
                        );
                        if let Some(round) = round {
                            if let Some(writer) = formula_writer.as_deref_mut() {
                                writer.write(formula_event(
                                    round,
                                    attacker_side,
                                    attacker_side,
                                    "lifesteal",
                                    format!(
                                        "ratio={:.2} based_on_hp_damage={} raw_heal={} final_heal={} heal_multiplier={:.2} actual_heal={} attacker_hp={}/{}",
                                        ratio,
                                        hp_damage,
                                        lifesteal_raw,
                                        lifesteal_final,
                                        lifesteal_multiplier,
                                        lifesteal_actual,
                                        attacker_stats.hp,
                                        attacker_stats.max_hp
                                    ),
                                ));
                            }
                        }
                    }
                }
                if let Some(round) = round {
                    if let Some(writer) = formula_writer.as_deref_mut() {
                        writer.write(formula_event(
                            round,
                            attacker_side,
                            target_side,
                            "physical_attack",
                            format!(
                                "atk_base={} atk_stage={} atk_effective={} def_base={} def_stage={} def_effective={} skill_power={} multiplier={:.2} raw_damage={} absorbed={} hp_damage={} target_hp={} target_shield={}",
                                attacker_stats.atk,
                                attacker_stats.atk_stage,
                                actual_atk,
                                target_stats.def,
                                target_stats.def_stage,
                                actual_def,
                                boosted_power,
                                multiplier,
                                raw_damage,
                                absorbed,
                                hp_damage,
                                target_stats.hp,
                                target_shield.0
                            ),
                        ));
                    }
                    if let Some(log) = structured_log.as_deref_mut() {
                        note_action_phase(
                            log,
                            round,
                            attacker_side,
                            "伤害结算",
                            format!(
                                "普通攻击结算；实际Atk={}；实际Def={}；倍率系数={:.2}；基础伤害={}；护盾吸收={}；生命伤害={}；目标剩余HP={}；目标剩余护盾={}",
                                actual_atk,
                                actual_def,
                                multiplier,
                                raw_damage,
                                absorbed,
                                hp_damage,
                                target_stats.hp,
                                target_shield.0
                            ),
                        );
                    }
                }
            }
        }
        SkillEffect::Heal { .. } | SkillEffect::Shield { .. } => {
            apply_self_skill_effect_with_context(
                &skill.effect,
                skill,
                attacker_side,
                attacker_stats,
                attacker_shield,
                target_aura,
                attacker_statuses,
                pending_boosts,
                formula_rules,
                accuracy_rng,
                element_db,
                status_db,
                reaction_db,
                event_writer,
                formula_writer.as_deref_mut(),
                status_writer.as_deref_mut(),
                structured_log.as_deref_mut(),
                round,
                ctx,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        ecs::{message::Messages, system::SystemState},
        prelude::World,
    };
    use std::collections::HashMap;

    fn test_skill(condition: SkillCondition) -> SkillDef {
        SkillDef {
            id: crate::data::SkillId::ShadowWingAssassinate,
            name: "test".to_string(),
            category: SkillCategory::SpecialAttack,
            cost_ap: 1,
            effect: SkillEffect::Conditional {
                branches: vec![crate::data::ConditionalSkillEffect {
                    condition,
                    effect: Box::new(SkillEffect::ModifyStages {
                        modifiers: vec![AttributeStageModifier {
                            attribute: AttributeType::Acc,
                            amount: -1,
                        }],
                        duration_turns: 1,
                        target: EffectTarget::Opponent,
                    }),
                }],
            },
            element: Some(ElementType::Wind),
            base_accuracy: Some(1.0),
        }
    }

    #[test]
    fn last_wind_spread_failed_requires_attack_resolved() {
        let mut ctx = EffectResolutionContext {
            last_attack_resolved: false,
            last_wind_spread_succeeded: false,
            ..Default::default()
        };
        assert!(!evaluate_condition(
            &SkillCondition::LastWindSpreadFailed,
            &ctx
        ));

        ctx.last_attack_resolved = true;
        assert!(evaluate_condition(
            &SkillCondition::LastWindSpreadFailed,
            &ctx
        ));
    }

    #[test]
    fn last_wind_spread_succeeded_requires_attack_resolved() {
        let mut ctx = EffectResolutionContext {
            last_attack_resolved: false,
            last_wind_spread_succeeded: true,
            ..Default::default()
        };
        assert!(!evaluate_condition(
            &SkillCondition::LastWindSpreadSucceeded,
            &ctx
        ));

        ctx.last_attack_resolved = true;
        assert!(evaluate_condition(
            &SkillCondition::LastWindSpreadSucceeded,
            &ctx
        ));
    }

    #[test]
    fn sequence_wind_skill_is_detected_as_wind_spread() {
        let skill = SkillDef {
            id: crate::data::SkillId::CycloneRend,
            name: "气旋撕裂".to_string(),
            category: SkillCategory::SpecialAttack,
            cost_ap: 2,
            element: Some(ElementType::Wind),
            base_accuracy: Some(1.0),
            effect: SkillEffect::Sequence {
                effects: vec![
                    SkillEffect::Attack {
                        power: 12,
                        lifesteal_ratio: None,
                        ignore_shield: false,
                    },
                    SkillEffect::Conditional { branches: vec![] },
                ],
            },
        };

        assert_eq!(skill_execution_mode(&skill), SkillExecutionMode::WindSpread);
    }

    #[test]
    fn conditional_branch_uses_failed_wind_spread_semantics() {
        let skill = test_skill(SkillCondition::LastWindSpreadFailed);
        let mut ctx = EffectResolutionContext {
            last_attack_resolved: true,
            last_wind_spread_succeeded: false,
            ..Default::default()
        };
        assert!(evaluate_condition(
            match &skill.effect {
                SkillEffect::Conditional { branches } => &branches[0].condition,
                _ => unreachable!(),
            },
            &ctx,
        ));

        ctx.last_attack_resolved = false;
        assert!(!evaluate_condition(
            match &skill.effect {
                SkillEffect::Conditional { branches } => &branches[0].condition,
                _ => unreachable!(),
            },
            &ctx,
        ));
    }

    fn formula_rules() -> BattleFormulaRules {
        BattleFormulaRules::default()
    }

    fn empty_status_db() -> StatusDb {
        StatusDb {
            statuses: HashMap::new(),
        }
    }

    fn test_reaction_db() -> ReactionDb {
        ReactionDb {
            reactions: vec![ReactionDef {
                id: "vaporize".to_string(),
                name: "蒸发".to_string(),
                required_elements: vec![ElementType::Water, ElementType::Fire],
                required_statuses: vec![],
                trigger_element: ElementType::Fire,
                fixed_damage: 3,
                heal_attacker: 0,
                apply_statuses: vec![],
                clear_statuses: vec![],
                aura_results: vec![],
            }],
        }
    }

    fn base_stats(hp: i32) -> Stats {
        Stats {
            hp,
            max_hp: hp,
            atk: 10,
            def: 0,
            spd: 10,
            acc: 100,
            atk_stage: 0,
            def_stage: 0,
            spd_stage: 0,
            acc_stage: 0,
        }
    }

    #[test]
    fn wind_spread_backline_triggers_secondary_reaction() {
        let mut world = World::new();
        world.init_resource::<Messages<BattleEvent>>();
        let mut system_state: SystemState<MessageWriter<BattleEvent>> =
            SystemState::new(&mut world);

        let mut attacker_stats = base_stats(30);
        let mut attacker_shield = Shield(0);
        let mut attacker_statuses = StatusBoard::default();
        let mut front_stats = base_stats(30);
        let mut front_shield = Shield(0);
        let mut front_aura = ElementAura {
            slots: [Some(ElementType::Fire), None],
        };
        let mut front_statuses = StatusBoard::default();
        let mut back_stats = base_stats(30);
        let mut back_shield = Shield(0);
        let mut back_aura = ElementAura {
            slots: [Some(ElementType::Water), None],
        };
        let mut back_statuses = StatusBoard::default();
        let mut pending_boosts = PendingBoosts::default();
        let mut accuracy_rng = AccuracyRng::default();
        let element_db = ElementDb::from_default_config();
        let status_db = empty_status_db();
        let reaction_db = test_reaction_db();
        let mut structured_log = StructuredBattleLog::default();
        let mut ctx = EffectResolutionContext::default();
        let skill = SkillDef {
            id: crate::data::SkillId::CycloneRend,
            name: "气旋撕裂".to_string(),
            category: SkillCategory::SpecialAttack,
            cost_ap: 2,
            effect: SkillEffect::Attack {
                power: 10,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: Some(ElementType::Wind),
            base_accuracy: Some(1.0),
        };

        {
            let mut event_writer = system_state.get_mut(&mut world);
            apply_wind_effect(
                &skill,
                Side::Player,
                Side::Enemy,
                &mut attacker_stats,
                &mut attacker_shield,
                &mut attacker_statuses,
                WindSpreadTarget {
                    base_element: ElementType::Grass,
                    stats: &mut front_stats,
                    shield: &mut front_shield,
                    aura: &mut front_aura,
                    statuses: &mut front_statuses,
                },
                Some(WindSpreadTarget {
                    base_element: ElementType::Grass,
                    stats: &mut back_stats,
                    shield: &mut back_shield,
                    aura: &mut back_aura,
                    statuses: &mut back_statuses,
                }),
                None,
                &mut pending_boosts,
                &formula_rules(),
                &mut accuracy_rng,
                &element_db,
                &status_db,
                &reaction_db,
                &mut event_writer,
                None,
                None,
                Some(&mut structured_log),
                Some(1),
                Some(&mut ctx),
            );
            system_state.apply(&mut world);
        }

        assert_eq!(front_aura.elements(), vec![ElementType::Fire]);
        assert!(back_aura.elements().is_empty());
        assert_eq!(ctx.last_reaction_name.as_deref(), Some("蒸发"));
        assert!(ctx.last_wind_spread_succeeded);
        assert!(ctx.last_attack_resolved);
        assert!(back_stats.hp < back_stats.max_hp);

        let events = world.resource::<Messages<BattleEvent>>();
        let mut cursor = events.get_cursor();
        let collected: Vec<_> = cursor.read(events).cloned().collect();
        assert!(collected.iter().any(|event| matches!(
            event,
            BattleEvent::WindSpreadTriggered {
                source: Side::Player,
                target: Side::Enemy,
                element: ElementType::Fire,
            }
        )));
        assert!(collected.iter().any(|event| matches!(
            event,
            BattleEvent::ReactionTriggered { reaction_name, .. } if reaction_name == "蒸发"
        )));
    }

    #[test]
    fn elemental_attack_shield_blocks_attachment_and_reaction() {
        let mut world = World::new();
        world.init_resource::<Messages<BattleEvent>>();
        let mut system_state: SystemState<MessageWriter<BattleEvent>> =
            SystemState::new(&mut world);

        let mut attacker_stats = base_stats(30);
        let mut attacker_shield = Shield(0);
        let mut attacker_statuses = StatusBoard::default();
        let mut target_stats = base_stats(30);
        let mut target_shield = Shield(99);
        let mut target_aura = ElementAura {
            slots: [Some(ElementType::Fire), None],
        };
        let mut target_statuses = StatusBoard::default();
        let mut pending_boosts = PendingBoosts::default();
        let mut accuracy_rng = AccuracyRng::default();
        let element_db = ElementDb::from_default_config();
        let status_db = empty_status_db();
        let reaction_db = test_reaction_db();
        let skill = SkillDef {
            id: crate::data::SkillId::WaterBlade,
            name: "水刃".to_string(),
            category: SkillCategory::ElementAttack,
            cost_ap: 1,
            effect: SkillEffect::Attack {
                power: 10,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: Some(ElementType::Water),
            base_accuracy: Some(1.0),
        };

        {
            let mut event_writer = system_state.get_mut(&mut world);
            apply_effect(
                &skill,
                Side::Player,
                Side::Enemy,
                ElementType::Grass,
                &mut attacker_stats,
                &mut attacker_shield,
                &mut attacker_statuses,
                &mut target_stats,
                &mut target_shield,
                &mut target_aura,
                &mut target_statuses,
                &mut pending_boosts,
                &formula_rules(),
                &mut accuracy_rng,
                &element_db,
                &status_db,
                &reaction_db,
                &mut event_writer,
                None,
                None,
                None,
                Some(1),
            );
            system_state.apply(&mut world);
        }

        assert_eq!(target_aura.elements(), vec![ElementType::Fire]);
        assert_eq!(target_stats.hp, target_stats.max_hp);
        assert!(target_shield.0 < 99);

        let events = world.resource::<Messages<BattleEvent>>();
        let mut cursor = events.get_cursor();
        let collected: Vec<_> = cursor.read(events).cloned().collect();
        assert!(
            !collected
                .iter()
                .any(|event| matches!(event, BattleEvent::ElementAuraApplied { .. }))
        );
        assert!(
            !collected
                .iter()
                .any(|event| matches!(event, BattleEvent::ReactionTriggered { .. }))
        );
    }
}
