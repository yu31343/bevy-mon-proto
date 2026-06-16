use bevy::prelude::*;

use super::resources::UiFontHandle;
use crate::{
    battle::{Combatant, ElementAura, InBattle, Shield, Stats, StatusBoard, Team},
    data::{
        AttributeStageModifier, AttributeType, BattleDbs, CardDef, CardEffect, EffectTarget,
        ElementType, SkillCategory, SkillCondition, SkillEffect, SkillId, StatusCategory,
        StatusTickTiming,
    },
    game_state::BattlePhase,
};

pub(crate) fn skill_name(skill_id: SkillId, dbs: &BattleDbs) -> String {
    dbs.skills
        .get(&skill_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("{skill_id:?}"))
}

fn skill_element_text(element: Option<ElementType>) -> &'static str {
    match element {
        Some(ElementType::Water) => "·水系",
        Some(ElementType::Fire) => "·火系",
        Some(ElementType::Grass) => "·草系",
        Some(ElementType::Light) => "·光系",
        Some(ElementType::Dark) => "·暗系",
        Some(ElementType::Thunder) => "·雷系",
        Some(ElementType::Wind) => "·风系",
        None => "",
    }
}

fn primary_effect(effect: &SkillEffect) -> &SkillEffect {
    match effect {
        SkillEffect::Sequence { effects } if !effects.is_empty() => primary_effect(&effects[0]),
        _ => effect,
    }
}

pub(crate) fn skill_meta(skill_id: SkillId, dbs: &BattleDbs) -> String {
    let Some(skill) = dbs.skills.get(&skill_id) else {
        return "类型：未知".to_string();
    };

    let primary = primary_effect(&skill.effect);
    match skill.category {
        SkillCategory::NormalAttack => "类型：普通攻击".to_string(),
        SkillCategory::ElementAttack => {
            format!("类型：元素攻击{}", skill_element_text(skill.element))
        }
        SkillCategory::SpecialAttack => {
            format!("类型：特殊攻击{}", skill_element_text(skill.element))
        }
        SkillCategory::SelfUtility => match primary {
            SkillEffect::Shield { .. } => "类型：自身护盾".to_string(),
            SkillEffect::Heal { .. } => "类型：自身治疗".to_string(),
            SkillEffect::Cleanse { .. } => "类型：自身净化".to_string(),
            SkillEffect::ModifyStages { .. } | SkillEffect::ApplyStatus { .. } => {
                "类型：自身增益".to_string()
            }
            _ => "类型：自身辅助".to_string(),
        },
        SkillCategory::AllyUtility => match primary {
            SkillEffect::Heal { .. } => "类型：己方治疗".to_string(),
            SkillEffect::Shield { .. } => "类型：己方护盾".to_string(),
            SkillEffect::Cleanse { .. } => "类型：己方净化".to_string(),
            SkillEffect::ModifyStages { .. } | SkillEffect::ApplyStatus { .. } => {
                "类型：己方增益".to_string()
            }
            _ => "类型：己方辅助".to_string(),
        },
        SkillCategory::EnemyDebuff => "类型：敌方减益".to_string(),
    }
}

pub(crate) fn monster_skill_ap_cost_ui(skill_id: SkillId, dbs: &BattleDbs) -> i32 {
    dbs.skills
        .get(&skill_id)
        .map(|skill| skill.cost_ap)
        .unwrap_or(999)
}

fn attribute_name(attribute: AttributeType) -> &'static str {
    match attribute {
        AttributeType::Atk => "Atk",
        AttributeType::Def => "Def",
        AttributeType::Spd => "Spd",
        AttributeType::Acc => "Acc",
    }
}

fn effect_target_label(target: EffectTarget) -> &'static str {
    match target {
        EffectTarget::Infer => "目标",
        EffectTarget::SelfTarget => "自身",
        EffectTarget::Opponent => "敌方",
    }
}

fn format_stage_modifiers(modifiers: &[AttributeStageModifier]) -> String {
    modifiers
        .iter()
        .map(|modifier| {
            format!(
                "{}{:+}",
                attribute_name(modifier.attribute),
                modifier.amount
            )
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn status_effect_name(status_id: &str, dbs: &BattleDbs) -> String {
    match status_id {
        "stage_shift_buff" => "属性增益".to_string(),
        "stage_shift_debuff" => "属性减益".to_string(),
        _ => dbs
            .statuses
            .statuses
            .get(status_id)
            .map(|status| status.name.clone())
            .unwrap_or_else(|| status_id.to_string()),
    }
}

fn status_effects_label(status_ids: &[String], dbs: &BattleDbs) -> String {
    status_ids
        .iter()
        .map(|status_id| status_effect_name(status_id, dbs))
        .collect::<Vec<_>>()
        .join("/")
}

fn condition_summary(condition: &SkillCondition, dbs: &BattleDbs) -> String {
    match condition {
        SkillCondition::TargetHadAura { element } => {
            format!("目标已有{}附着", element_name(*element))
        }
        SkillCondition::TargetHadStatus { status_id } => {
            format!("目标有状态{}", status_effect_name(status_id, dbs))
        }
        SkillCondition::TargetHadNoShield => "目标没有护盾".to_string(),
        SkillCondition::LastReactionName { reaction_name } => format!("触发{}", reaction_name),
        SkillCondition::LastWindSpreadSucceeded => "成功扩散".to_string(),
        SkillCondition::LastWindSpreadFailed => "未扩散".to_string(),
        SkillCondition::LastCleanseSucceeded => "净化成功".to_string(),
        SkillCondition::LastCleanseFailed => "净化失败".to_string(),
        SkillCondition::LastTargetFainted => "目标倒下".to_string(),
        SkillCondition::Any { conditions } => conditions
            .iter()
            .map(|condition| condition_summary(condition, dbs))
            .collect::<Vec<_>>()
            .join(" 或 "),
        SkillCondition::All { conditions } => conditions
            .iter()
            .map(|condition| condition_summary(condition, dbs))
            .collect::<Vec<_>>()
            .join(" 且 "),
    }
}

fn effect_summary(effect: &SkillEffect, dbs: &BattleDbs) -> String {
    match effect {
        SkillEffect::Attack {
            power,
            lifesteal_ratio,
            ignore_shield,
        } => {
            let mut parts = vec![format!("{:.2}*Atk", (*power as f32 / 10.0).max(0.0))];
            if *ignore_shield {
                parts.push("无视护盾".to_string());
            }
            if let Some(ratio) = lifesteal_ratio {
                parts.push(format!("吸血{}%", (ratio * 100.0).round() as i32));
            }
            parts.join("，")
        }
        SkillEffect::Heal { amount } => format!("治疗{}", amount),
        SkillEffect::Shield { amount } => format!("护盾{}", amount),
        SkillEffect::ApplyStatus { status_id } => {
            format!("施加状态{}", status_effect_name(status_id, dbs))
        }
        SkillEffect::Cleanse {
            prefer_aura,
            fallback_to_debuff,
            amount,
        } => {
            let target = if *prefer_aura && *fallback_to_debuff {
                "优先清附着，否则清减益"
            } else if *prefer_aura {
                "清附着"
            } else if *fallback_to_debuff {
                "清减益"
            } else {
                "净化"
            };
            format!("{}×{}", target, amount)
        }
        SkillEffect::Dispel { status_ids, target } => {
            format!(
                "驱散{}{}",
                effect_target_label(*target),
                status_effects_label(status_ids, dbs)
            )
        }
        SkillEffect::DealFixedDamage {
            amount,
            ignore_shield,
            target,
        } => {
            if *ignore_shield {
                format!(
                    "对{}造成{}固定伤害(无视盾)",
                    effect_target_label(*target),
                    amount
                )
            } else {
                format!("对{}造成{}固定伤害", effect_target_label(*target), amount)
            }
        }
        SkillEffect::DealStatDifferenceDamage {
            source_attribute,
            target_attribute,
            multiply_by_source_stage,
            ignore_shield,
            target,
        } => {
            let mut parts = vec![format!(
                "对{}造成{}-{}属性差固定伤害",
                effect_target_label(*target),
                attribute_name(*source_attribute),
                attribute_name(*target_attribute)
            )];
            if *multiply_by_source_stage {
                parts.push("受来源等级修正".to_string());
            }
            if *ignore_shield {
                parts.push("无视护盾".to_string());
            }
            parts.join("，")
        }
        SkillEffect::ModifyStages {
            modifiers,
            duration_turns,
            target,
        } => format!(
            "{}属性{}，{}回合",
            effect_target_label(*target),
            format_stage_modifiers(modifiers),
            duration_turns
        ),
        SkillEffect::Conditional { branches } => branches
            .iter()
            .map(|branch| {
                format!(
                    "若{}，则{}",
                    condition_summary(&branch.condition, dbs),
                    effect_summary(&branch.effect, dbs)
                )
            })
            .collect::<Vec<_>>()
            .join("；"),
        SkillEffect::Sequence { effects } => effects
            .iter()
            .map(|effect| effect_summary(effect, dbs))
            .collect::<Vec<_>>()
            .join("；"),
    }
}

pub(crate) fn skill_summary(skill_id: SkillId, dbs: &BattleDbs) -> String {
    let Some(skill) = dbs.skills.get(&skill_id) else {
        return "效果：未知".to_string();
    };

    let mut summary = effect_summary(&skill.effect, dbs);
    if matches!(skill.category, SkillCategory::ElementAttack) {
        if let Some(element) = skill.element {
            summary = format!("附着{}；{}", element_name(element), summary);
        }
    } else if matches!(skill.category, SkillCategory::SpecialAttack)
        && matches!(skill.element, Some(ElementType::Wind))
    {
        summary = format!("风扩散；{}", summary);
    }

    format!("效果：{}", summary)
}

pub(crate) fn phase_label(phase: BattlePhase) -> &'static str {
    match phase {
        BattlePhase::Init => "初始化",
        BattlePhase::RoundStart => "回合开始",
        BattlePhase::PlayerTurn => "玩家回合",
        BattlePhase::EnemyTurn => "敌方回合",
        BattlePhase::Discard => "弃牌阶段",
        BattlePhase::CheckEnd => "胜负判定",
        BattlePhase::DeathResolve => "死亡结算",
    }
}

pub(crate) fn active_hp_percent(
    team: &Team,
    query: &Query<(&Combatant, &Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
) -> f32 {
    let Some(entity) = team.active_combatant() else {
        return 0.0;
    };
    let Ok((_, stats, _, _, _)) = query.get(entity) else {
        return 0.0;
    };
    if stats.max_hp <= 0 {
        return 0.0;
    }
    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
}

pub(crate) fn active_shield(
    team: &Team,
    query: &Query<(&Combatant, &Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
) -> i32 {
    let Some(entity) = team.active_combatant() else {
        return 0;
    };
    let Ok((_, _, _, shield, _)) = query.get(entity) else {
        return 0;
    };
    shield.0.max(0)
}

pub(crate) fn active_shield_percent(
    team: &Team,
    query: &Query<(&Combatant, &Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
) -> f32 {
    let Some(entity) = team.active_combatant() else {
        return 0.0;
    };
    let Ok((_, stats, _, shield, _)) = query.get(entity) else {
        return 0.0;
    };
    if stats.max_hp <= 0 {
        return 0.0;
    }
    ((shield.0.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
}

pub(crate) fn element_name(element: ElementType) -> &'static str {
    match element {
        ElementType::Water => "水",
        ElementType::Fire => "火",
        ElementType::Grass => "草",
        ElementType::Light => "光",
        ElementType::Dark => "暗",
        ElementType::Thunder => "雷",
        ElementType::Wind => "风",
    }
}

pub(crate) fn aura_label(aura: &[ElementType]) -> String {
    if aura.is_empty() {
        return "无".to_string();
    }
    aura.iter()
        .map(|element| element_name(*element))
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn compact_aura_label(aura: &[ElementType]) -> String {
    if aura.is_empty() {
        return String::new();
    }
    aura.iter().map(|element| element_name(*element)).collect()
}

pub(crate) fn status_label(statuses: &StatusBoard) -> String {
    let labels = statuses
        .entries
        .iter()
        .filter(|entry| entry.category != StatusCategory::Aura && entry.stage_modifiers.is_empty())
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();

    if labels.is_empty() {
        "无".to_string()
    } else {
        labels.join("/")
    }
}

#[allow(dead_code)]
pub(crate) fn aura_and_status_label(aura: &[ElementType], statuses: &StatusBoard) -> String {
    format!(
        "附着: {} | 状态: {}",
        aura_label(aura),
        status_label(statuses)
    )
}

#[allow(dead_code)]
fn status_category_label(category: StatusCategory) -> &'static str {
    match category {
        StatusCategory::Aura => "附着",
        StatusCategory::Buff => "增益",
        StatusCategory::Debuff => "减益",
        StatusCategory::Special => "特殊",
    }
}

#[allow(dead_code)]
fn status_tick_timing_label(timing: Option<StatusTickTiming>) -> &'static str {
    match timing {
        Some(StatusTickTiming::OwnerActionEnd) => "行动后",
        None => "即时",
    }
}

#[allow(dead_code)]
pub(crate) fn status_debug_label(statuses: &StatusBoard) -> String {
    let labels = statuses
        .entries
        .iter()
        .map(|entry| {
            let mut parts = vec![format!(
                "{}[{}|{}回合|{}]",
                entry.name,
                status_category_label(entry.category),
                entry.remaining_turns.max(0),
                status_tick_timing_label(entry.tick_timing)
            )];
            if !entry.stage_modifiers.is_empty() {
                parts.push(format!(
                    "阶段{}",
                    entry
                        .stage_modifiers
                        .iter()
                        .map(|modifier| {
                            format!(
                                "{}{:+}",
                                attribute_name(modifier.attribute),
                                modifier.amount
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("/")
                ));
            }
            if entry.fixed_damage_on_tick > 0 {
                parts.push(format!("持续伤害{}", entry.fixed_damage_on_tick));
            }
            if entry.heal_on_tick > 0 {
                parts.push(format!("持续治疗{}", entry.heal_on_tick));
            }
            if let Some(multiplier) = entry.heal_taken_multiplier {
                parts.push(format!("治疗倍率{:.2}", multiplier));
            }
            if entry.evade_charges > 0 {
                parts.push(format!("闪避{}次", entry.evade_charges));
            }
            parts.join("·")
        })
        .collect::<Vec<_>>();

    if labels.is_empty() {
        "无".to_string()
    } else {
        labels.join(" / ")
    }
}

pub(crate) fn stat_stage_debug_label(stats: &Stats) -> String {
    format!(
        "Atk{:+} Def{:+} Spd{:+} Acc{:+}",
        stats.atk_stage, stats.def_stage, stats.spd_stage, stats.acc_stage
    )
}

pub(crate) fn aura_status_stage_label(
    stats: &Stats,
    aura: &[ElementType],
    statuses: &StatusBoard,
) -> String {
    format!(
        "附着: {} | 状态: {} | 阶段: {}",
        aura_label(aura),
        status_label(statuses),
        stat_stage_debug_label(stats)
    )
}

pub(crate) fn status_stage_label(stats: &Stats, statuses: &StatusBoard) -> String {
    format!(
        "状态: {} | 阶段: {}",
        status_label(statuses),
        stat_stage_debug_label(stats)
    )
}

#[allow(dead_code)]
pub(crate) fn combatant_debug_summary(
    name: &Name,
    stats: &Stats,
    shield: &Shield,
    aura: &ElementAura,
    statuses: &StatusBoard,
) -> String {
    format!(
        "{} | HP {}/{} | 护盾 {} | 附着 {} | 属性等级 {} | 详细状态 {}",
        name,
        stats.hp.max(0),
        stats.max_hp,
        shield.0.max(0),
        aura_label(&aura.elements()),
        stat_stage_debug_label(stats),
        status_debug_label(statuses)
    )
}

#[allow(dead_code)]
pub(crate) fn team_debug_summary(
    header: &str,
    team: &Team,
    query: &Query<(&Stats, &Name, &Shield, &ElementAura, &StatusBoard), With<InBattle>>,
) -> String {
    if team.combatants.is_empty() {
        return format!("{header}\n无成员");
    }

    let mut lines = vec![header.to_string()];
    for (index, entity) in team.combatants.iter().copied().enumerate() {
        let prefix = if index == team.active_index {
            format!("[前场{}]", index + 1)
        } else {
            format!("[后场{}]", index + 1)
        };
        let line = if let Ok((stats, name, shield, aura, statuses)) = query.get(entity) {
            format!(
                "{} {}",
                prefix,
                combatant_debug_summary(name, stats, shield, aura, statuses)
            )
        } else {
            format!("{} 数据读取失败", prefix)
        };
        lines.push(line);
    }
    lines.join("\n")
}

pub(crate) fn card_hotkey_label(index: usize) -> &'static str {
    match index {
        0 => "Z",
        1 => "X",
        2 => "C",
        3 => "V",
        4 => "B",
        5 => "N",
        6 => "A",
        7 => "S",
        8 => "D",
        9 => "G",
        10 => "H",
        11 => "J",
        12 => "K",
        13 => "L",
        14 => "U",
        15 => "I",
        16 => "O",
        17 => "P",
        _ => "",
    }
}

pub(crate) fn card_description(card: &CardDef) -> String {
    match &card.effect {
        CardEffect::GainAp { amount } => format!("效果：获得 +{} AP。", amount),
        CardEffect::NextAttackBoost { amount } => format!("效果：下次进攻 +{}。", amount),
        CardEffect::NextShieldBoost { amount } => format!("效果：下次护盾 +{}。", amount),
        CardEffect::NextHealBoost { amount } => format!("效果：下次治疗 +{}。", amount),
        CardEffect::NextElementAttachmentGainAp { amount } => {
            format!("效果：下次元素附着/反应成功时获得 +{} AP。", amount)
        }
        CardEffect::NextReactionFixedDamage {
            amount,
            ignore_shield,
        } => {
            let shield_text = if *ignore_shield {
                "，无视护盾"
            } else {
                ""
            };
            format!(
                "效果：下次触发元素反应时追加 {} 点固定伤害{}。",
                amount, shield_text
            )
        }
        CardEffect::NextWindSpreadDamage {
            amount,
            elements,
            ignore_shield,
        } => {
            let shield_text = if *ignore_shield {
                "，无视护盾"
            } else {
                ""
            };
            format!(
                "效果：下次风扩散 {:?} 时追加 {} 点固定伤害{}。",
                elements, amount, shield_text
            )
        }
        CardEffect::NextAuraAttackDraw { amount } => {
            format!("效果：下次攻击命中已有附着目标时抽 {} 张牌。", amount)
        }
        CardEffect::DiscardOtherDrawGainAp { draw, gain_ap } => {
            format!(
                "效果：弃置 1 张其他手牌，抽 {} 张牌，获得 +{} AP。",
                draw, gain_ap
            )
        }
        CardEffect::NextSkillCostDraw { skill_cost, draw } => {
            format!("效果：下次使用 {} AP 技能后抽 {} 张牌。", skill_cost, draw)
        }
        CardEffect::DrawIfKnockedOutThisTurn { amount } => {
            format!("效果：若本行动内击倒敌方精灵，抽 {} 张牌。", amount)
        }
        CardEffect::GainShield { amount } => format!("效果：己方前场获得 {} 点护盾。", amount),
        CardEffect::ShieldAbsorbGainAp { amount } => {
            format!(
                "效果：敌方下次行动结束前，己方护盾吸收伤害时获得 +{} AP。",
                amount
            )
        }
        CardEffect::ModifyStages {
            attribute,
            amount,
            duration_turns,
        } => {
            format!(
                "效果：己方前场 {:?} 等级 {:+}，持续 {} 回合。",
                attribute, amount, duration_turns
            )
        }
        CardEffect::CleanseOrGainAp { fallback_ap, .. } => {
            format!(
                "效果：清除 1 个普通附着/Debuff/特殊负面状态；若失败则获得 +{} AP。",
                fallback_ap
            )
        }
        CardEffect::DrawAndGainApIfAliveTeam {
            draw,
            min_alive,
            gain_ap,
        } => {
            format!(
                "效果：抽 {} 张牌；若己方至少 {} 只未倒下，额外获得 +{} AP。",
                draw, min_alive, gain_ap
            )
        }
        CardEffect::GainShieldDrawIfSwitchedThisTurn { shield, draw } => {
            format!(
                "效果：己方前场获得 {} 点护盾；本行动结束前发生换人时，额外抽 {} 张牌。",
                shield, draw
            )
        }
    }
}

pub(crate) fn element_color(element: ElementType, theme: &super::theme::UiTheme) -> Color {
    match element {
        ElementType::Fire => theme.aura_fire,
        ElementType::Water => theme.aura_water,
        ElementType::Grass => theme.aura_grass,
        ElementType::Light => theme.aura_light,
        ElementType::Dark => theme.aura_dark,
        ElementType::Thunder => theme.aura_thunder,
        ElementType::Wind => theme.aura_wind,
    }
}

pub(crate) fn status_color(
    entry: &crate::battle::StatusInstance,
    theme: &super::theme::UiTheme,
) -> Color {
    let lower_id = entry.id.to_ascii_lowercase();
    let lower_name = entry.name.to_ascii_lowercase();
    if lower_id.contains("burn") || lower_name.contains("燃") {
        return theme.status_burn;
    }
    if lower_id.contains("scorch") || lower_name.contains("灼") || lower_name.contains("燎") {
        return theme.status_scorch;
    }
    if lower_id.contains("paral")
        || lower_id.contains("electro")
        || lower_name.contains("麻")
        || lower_name.contains("电")
    {
        return theme.status_paralysis;
    }
    if lower_id.contains("curse")
        || lower_id.contains("seed")
        || lower_id.contains("entangle")
        || lower_name.contains("诅")
        || lower_name.contains("草")
        || lower_name.contains("缠")
    {
        return theme.status_poison_like;
    }
    match entry.category {
        StatusCategory::Buff => theme.status_buff,
        StatusCategory::Debuff => theme.status_debuff,
        StatusCategory::Special => theme.status_special,
        StatusCategory::Aura => theme.text_muted,
    }
}

pub(crate) fn effective_atk_value(stats: &Stats, rules: &crate::data::BattleFormulaRules) -> i32 {
    let stage = stats.atk_stage.clamp(
        rules.attribute_stage_bounds.min,
        rules.attribute_stage_bounds.max,
    );
    let ratio = if stage >= 0 {
        (2 + stage) as f32 / 2.0
    } else {
        2.0 / (2 + stage.abs()) as f32
    };
    ((stats.atk as f32) * ratio).round() as i32
}

pub(crate) fn effective_def_value(stats: &Stats, rules: &crate::data::BattleFormulaRules) -> i32 {
    let stage = stats.def_stage.clamp(
        rules.attribute_stage_bounds.min,
        rules.attribute_stage_bounds.max,
    );
    let ratio = if stage >= 0 {
        (2 + stage) as f32 / 2.0
    } else {
        2.0 / (2 + stage.abs()) as f32
    };
    ((stats.def as f32) * ratio).round() as i32
}

pub(crate) fn effective_spd_value(stats: &Stats, rules: &crate::data::BattleFormulaRules) -> i32 {
    let stage = stats.spd_stage.clamp(
        rules.attribute_stage_bounds.min,
        rules.attribute_stage_bounds.max,
    );
    (stats.spd + stage * 2).max(1)
}

pub(crate) fn effective_acc_value(stats: &Stats, rules: &crate::data::BattleFormulaRules) -> i32 {
    let stage = stats.acc_stage.clamp(
        rules.attribute_stage_bounds.min,
        rules.attribute_stage_bounds.max,
    );
    ((stats.acc as f32 / 100.0 + stage as f32 * rules.accuracy.stage_step)
        .clamp(rules.accuracy.min, rules.accuracy.max)
        * 100.0)
        .round() as i32
}

pub(crate) fn stage_prefix(stage: i32) -> String {
    if stage == 0 {
        String::new()
    } else {
        format!("{:+} ", stage)
    }
}

pub(crate) fn replace_debug_tokens(
    commands: &mut Commands,
    line_entity: Entity,
    children: Option<&Children>,
    font: &TextFont,
    items: &[(String, Color)],
    token_kind: impl Component + Clone,
) {
    if let Some(children) = children {
        for child in children.iter().skip(1) {
            commands.entity(child).despawn();
        }
    }
    commands.entity(line_entity).with_children(|line| {
        for (idx, (text, color)) in items.iter().enumerate() {
            let prefix = if idx == 0 { "" } else { "" };
            line.spawn((
                Text::new(format!("{prefix}{text}")),
                font.clone(),
                TextColor(*color),
                token_kind.clone(),
            ));
        }
    });
}

pub(crate) enum DebugTokenContent {
    Text(String, Color),
    Image(&'static str),
}

pub(crate) fn replace_debug_tokens_with_images(
    commands: &mut Commands,
    line_entity: Entity,
    children: Option<&Children>,
    asset_server: &AssetServer,
    font: &TextFont,
    items: &[DebugTokenContent],
    token_kind: impl Component + Clone,
) {
    if let Some(children) = children {
        for child in children.iter().skip(1) {
            commands.entity(child).despawn();
        }
    }
    commands.entity(line_entity).with_children(|line| {
        for item in items {
            match item {
                DebugTokenContent::Text(text, color) => {
                    line.spawn((
                        Text::new(text.clone()),
                        font.clone(),
                        TextColor(*color),
                        token_kind.clone(),
                    ));
                }
                DebugTokenContent::Image(path) => {
                    line.spawn((
                        Node {
                            width: Val::Px(28.0),
                            height: Val::Px(28.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        ImageNode::new(asset_server.load(*path)),
                        token_kind.clone(),
                    ));
                }
            }
        }
    });
}

pub(crate) fn make_text_font(size: f32, ui_font: Option<&UiFontHandle>) -> TextFont {
    let mut text_font = TextFont::from_font_size(size);
    if let Some(font) = ui_font {
        text_font.font = font.0.clone();
    }
    text_font
}
