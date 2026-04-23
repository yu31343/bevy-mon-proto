use bevy::prelude::*;

use crate::{
    battle::{Combatant, ElementAura, InBattle, Team, Shield, Stats},
    data::{BattleDbs, CardDef, CardEffect, ElementType, SkillEffect, SkillId},
    game_state::BattlePhase,
};

use super::{
    resources::UiFontHandle,
};

pub(crate) fn skill_name(skill_id: SkillId, dbs: &BattleDbs) -> String {
    dbs.skills
        .get(&skill_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("{skill_id:?}"))
}

pub(crate) fn skill_meta(skill_id: SkillId, dbs: &BattleDbs) -> String {
    let Some(skill) = dbs.skills.get(&skill_id) else {
        return "类型：未知".to_string();
    };
    match &skill.effect {
        SkillEffect::Attack { .. } => {
            let element_text = match skill.element {
                Some(ElementType::Water) => "·水系",
                Some(ElementType::Fire) => "·火系",
                Some(ElementType::Grass) => "·草系",
                Some(ElementType::Light) => "·光系",
                Some(ElementType::Dark) => "·暗系",
                Some(ElementType::Thunder) => "·雷系",
                Some(ElementType::Wind) => "·风系",
                None => "",
            };
            format!("类型：攻击{}", element_text)
        }
        SkillEffect::Heal { .. } => "类型：治疗".to_string(),
        SkillEffect::Shield { .. } => "类型：护盾".to_string(),
    }
}

pub(crate) fn monster_skill_ap_cost_ui(slot: usize) -> i32 {
    match slot {
        0 => 2,
        1 => 3,
        2 => 1,
        3 => 1,
        _ => 999,
    }
}

pub(crate) fn phase_label(phase: BattlePhase) -> &'static str {
    match phase {
        BattlePhase::Init => "初始化",
        BattlePhase::RoundStart => "回合开始",
        BattlePhase::PlayerTurn => "玩家回合",
        BattlePhase::EnemyTurn => "敌方回合",
        BattlePhase::CheckEnd => "胜负判定",
    }
}

pub(crate) fn active_hp_percent(
    team: &Team,
    query: &Query<(
        &Combatant,
        &Stats,
        &Name,
        &Shield,
        &ElementAura,
    ), With<InBattle>>,
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
    query: &Query<(
        &Combatant,
        &Stats,
        &Name,
        &Shield,
        &ElementAura,
    ), With<InBattle>>,
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
    query: &Query<(
        &Combatant,
        &Stats,
        &Name,
        &Shield,
        &ElementAura,
    ), With<InBattle>>,
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

pub(crate) fn aura_label(aura: Option<ElementType>) -> &'static str {
    match aura {
        Some(element) => element_name(element),
        None => "无",
    }
}

pub(crate) fn card_hotkey_label(index: usize) -> &'static str {
    match index {
        0 => "Z",
        1 => "X",
        2 => "C",
        3 => "V",
        4 => "B",
        _ => "",
    }
}

pub(crate) fn card_description(card: &CardDef) -> String {
    match card.effect {
        CardEffect::GainAp { amount } => format!("效果：获得 +{} AP。", amount),
        CardEffect::NextAttackBoost { amount } => format!("效果：下次进攻 +{}。", amount),
        CardEffect::NextShieldBoost { amount } => format!("效果：下次护盾 +{}。", amount),
        CardEffect::NextHealBoost { amount } => format!("效果：下次治疗 +{}。", amount),
    }
}

pub(crate) fn make_text_font(size: f32, ui_font: Option<&UiFontHandle>) -> TextFont {
    let mut text_font = TextFont::from_font_size(size);
    if let Some(font) = ui_font {
        text_font.font = font.0.clone();
    }
    text_font
}

