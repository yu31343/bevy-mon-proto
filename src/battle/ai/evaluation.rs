use crate::{
    battle::Shield,
    data::{
        AttributeStageModifier, AttributeType, BattleDbs, CardDef, CardEffect, CardId,
        EffectTarget, ElementType, EnemyAiWeights, ReactionDb, SkillCategory, SkillDef,
        SkillEffect, SkillId,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnemyAiSkillKind {
    Attack,
    Heal,
    Shield,
    Debuff,
}

#[derive(Debug, Clone)]
pub(crate) struct EnemyAiContext {
    pub(crate) enemy_hp: i32,
    pub(crate) enemy_max_hp: i32,
    pub(crate) enemy_shield: i32,
    pub(crate) max_shield_hp_ratio: f32,
    pub(crate) enemy_atk: i32,
    pub(crate) enemy_def: i32,
    pub(crate) enemy_atk_stage: i32,
    pub(crate) enemy_def_stage: i32,
    pub(crate) enemy_spd_stage: i32,
    pub(crate) enemy_acc_stage: i32,
    pub(crate) enemy_element: ElementType,
    pub(crate) enemy_attached_auras: [Option<ElementType>; 2],
    pub(crate) enemy_status_ids: Vec<String>,
    pub(crate) enemy_has_aura: bool,
    pub(crate) enemy_has_cleansable_debuff: bool,
    pub(crate) player_atk_stage: i32,
    pub(crate) player_def_stage: i32,
    pub(crate) player_spd_stage: i32,
    pub(crate) player_acc_stage: i32,
    pub(crate) player_def: i32,
    pub(crate) player_hp: i32,
    pub(crate) player_shield: i32,
    pub(crate) target_element: crate::data::ElementType,
    pub(crate) target_attached_auras: [Option<crate::data::ElementType>; 2],
    pub(crate) target_status_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScoredEnemySkill {
    pub(crate) slot: usize,
    pub(crate) skill_id: SkillId,
    pub(crate) score: f32,
    pub(crate) kind: EnemyAiSkillKind,
}

#[derive(Debug, Clone)]
pub(crate) struct EnemySwitchCandidate {
    pub(crate) index: usize,
    pub(crate) hp: i32,
    pub(crate) max_hp: i32,
    pub(crate) shield: i32,
    pub(crate) max_shield_hp_ratio: f32,
    pub(crate) atk: i32,
    pub(crate) def: i32,
    pub(crate) atk_stage: i32,
    pub(crate) def_stage: i32,
    pub(crate) spd_stage: i32,
    pub(crate) acc_stage: i32,
    pub(crate) element: ElementType,
    pub(crate) attached_auras: [Option<ElementType>; 2],
    pub(crate) skill_ids: [SkillId; 4],
    pub(crate) skill_count: usize,
    pub(crate) status_ids: Vec<String>,
    pub(crate) has_aura: bool,
    pub(crate) has_cleansable_debuff: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlayerThreatContext {
    pub(crate) skill_ids: [SkillId; 4],
    pub(crate) skill_count: usize,
    pub(crate) atk: i32,
    pub(crate) current_ap: i32,
    pub(crate) projected_ap: i32,
    pub(crate) attack_bonus: i32,
    pub(crate) fixed_damage_bonus: i32,
    pub(crate) defensive_value: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct ScoredEnemySwitch {
    pub(crate) index: usize,
    pub(crate) score: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct ScoredEnemyCard {
    pub(crate) index: usize,
    pub(crate) score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnemyDiscardFollowUp {
    Skill,
    Switch,
    ImmediateCardThenSkill,
    ImmediateCardThenSwitch,
    ImmediateCard,
    None,
}

fn discard_followup_priority(followup: EnemyDiscardFollowUp) -> u8 {
    match followup {
        EnemyDiscardFollowUp::Skill => 5,
        EnemyDiscardFollowUp::Switch => 4,
        EnemyDiscardFollowUp::ImmediateCardThenSkill => 3,
        EnemyDiscardFollowUp::ImmediateCardThenSwitch => 2,
        EnemyDiscardFollowUp::ImmediateCard => 1,
        EnemyDiscardFollowUp::None => 0,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ScoredEnemyDiscard {
    pub(crate) index: usize,
    pub(crate) score: f32,
    pub(crate) keep_score: f32,
    pub(crate) followup_score: f32,
    pub(crate) followup: EnemyDiscardFollowUp,
}

#[derive(Debug, Clone)]
pub(crate) enum EnemyPlannedAction {
    UseCardForSkill {
        card_index: usize,
        skill: ScoredEnemySkill,
    },
    UseSkill(ScoredEnemySkill),
    Switch(ScoredEnemySwitch),
    ImmediateCard(ScoredEnemyCard),
    Discard(ScoredEnemyDiscard),
    EndTurn,
}

#[derive(Debug, Clone)]
pub(crate) struct EnemyAiPlan {
    pub(crate) action: EnemyPlannedAction,
    pub(crate) score: f32,
    pub(crate) summary: String,
}

fn planned_action_priority(action: &EnemyPlannedAction) -> u8 {
    match action {
        EnemyPlannedAction::UseCardForSkill { .. } => 6,
        EnemyPlannedAction::UseSkill(_) => 5,
        EnemyPlannedAction::Switch(_) => 4,
        EnemyPlannedAction::ImmediateCard(_) => 3,
        EnemyPlannedAction::Discard(_) => 2,
        EnemyPlannedAction::EndTurn => 0,
    }
}

fn apply_skill_weight(mut scored: ScoredEnemySkill, weights: &EnemyAiWeights) -> ScoredEnemySkill {
    scored.score *= match scored.kind {
        EnemyAiSkillKind::Attack => weights.attack_value,
        EnemyAiSkillKind::Heal => weights.healing_value,
        EnemyAiSkillKind::Shield => weights.shield_value,
        EnemyAiSkillKind::Debuff => weights.status_value,
    };
    scored
}

pub(crate) fn build_player_threat_context(
    skill_ids: [SkillId; 4],
    skill_count: usize,
    atk: i32,
    current_ap: i32,
    hand: &[CardId],
    include_hand: bool,
    dbs: &BattleDbs,
) -> PlayerThreatContext {
    let mut projected_ap = current_ap;
    let mut attack_bonus = 0;
    let mut fixed_damage_bonus = 0;
    let mut defensive_value = 0.0;

    if include_hand {
        for card_id in hand {
            let Some(card) = dbs.cards.get(card_id) else {
                continue;
            };
            match &card.effect {
                CardEffect::GainAp { amount } => {
                    if current_ap >= card.cost_ap {
                        projected_ap += *amount - card.cost_ap;
                    }
                }
                CardEffect::NextAttackBoost { amount } => {
                    if current_ap >= card.cost_ap {
                        attack_bonus = attack_bonus.max(*amount);
                    }
                }
                CardEffect::NextReactionFixedDamage { amount, .. }
                | CardEffect::NextWindSpreadDamage { amount, .. } => {
                    if current_ap >= card.cost_ap {
                        fixed_damage_bonus = fixed_damage_bonus.max(*amount);
                    }
                }
                CardEffect::GainShield { amount } => {
                    if current_ap >= card.cost_ap {
                        defensive_value += (*amount).max(0) as f32;
                    }
                }
                CardEffect::NextShieldBoost { amount } | CardEffect::NextHealBoost { amount } => {
                    if current_ap >= card.cost_ap {
                        defensive_value += (*amount).max(0) as f32 * 0.7;
                    }
                }
                CardEffect::CleanseOrGainAp { fallback_ap, .. } => {
                    if current_ap >= card.cost_ap {
                        projected_ap += *fallback_ap - card.cost_ap;
                    }
                }
                CardEffect::DrawAndGainApIfAliveTeam { gain_ap, .. }
                | CardEffect::ShieldAbsorbGainAp { amount: gain_ap } => {
                    if current_ap >= card.cost_ap {
                        projected_ap += *gain_ap - card.cost_ap;
                    }
                }
                _ => {}
            }
        }
    }

    PlayerThreatContext {
        skill_ids,
        skill_count,
        atk,
        current_ap,
        projected_ap: projected_ap.max(current_ap),
        attack_bonus,
        fixed_damage_bonus,
        defensive_value,
    }
}

pub(crate) fn score_card_for_skill(
    card: &CardDef,
    chosen_skill_kind: EnemyAiSkillKind,
    skill: &SkillDef,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
    available_ap_after_skill: i32,
    weights: &EnemyAiWeights,
) -> Option<f32> {
    if card.cost_ap > available_ap_after_skill {
        return None;
    }

    let target_has_aura = ctx.target_attached_auras.iter().flatten().next().is_some();
    let elemental_attack = skill.element.is_some() && primary_attack_power(&skill.effect).is_some();
    let reaction_ready = skill.element.is_some_and(|skill_element| {
        ctx.target_attached_auras
            .iter()
            .flatten()
            .any(|aura| *aura != skill_element)
    });
    let attack_value = estimate_attack_value(skill, ctx, dbs).max(0.0);

    let score = match (&card.effect, chosen_skill_kind) {
        (CardEffect::NextAttackBoost { amount }, EnemyAiSkillKind::Attack)
        | (CardEffect::NextAttackBoost { amount }, EnemyAiSkillKind::Debuff) => {
            (12.0 + *amount as f32) * weights.card_value * weights.attack_value
        }
        (CardEffect::NextHealBoost { amount }, EnemyAiSkillKind::Heal) => {
            (12.0 + *amount as f32) * weights.card_value * weights.healing_value
        }
        (CardEffect::NextShieldBoost { amount }, EnemyAiSkillKind::Shield) => {
            let base_shield = match primary_effect(&skill.effect) {
                Some(SkillEffect::Shield { amount }) => amount.max(&0),
                _ => &0,
            };
            let room_after_skill = (remaining_shield_room(
                ctx.enemy_max_hp,
                ctx.enemy_shield,
                ctx.max_shield_hp_ratio,
            ) - *base_shield)
                .max(0);
            let effective_amount = amount.max(&0).min(&room_after_skill);
            if *effective_amount <= 0 {
                return None;
            }
            (12.0 + *effective_amount as f32) * weights.card_value * weights.shield_value
        }
        (CardEffect::NextElementAttachmentGainAp { amount }, EnemyAiSkillKind::Attack)
        | (CardEffect::NextElementAttachmentGainAp { amount }, EnemyAiSkillKind::Debuff)
            if elemental_attack =>
        {
            (10.0 + *amount as f32 * 7.0 + if target_has_aura { 6.0 } else { 0.0 })
                * weights.card_value
                * weights.reaction_value
        }
        (CardEffect::NextReactionFixedDamage { amount, .. }, EnemyAiSkillKind::Attack)
        | (CardEffect::NextReactionFixedDamage { amount, .. }, EnemyAiSkillKind::Debuff)
            if reaction_ready =>
        {
            (16.0
                + *amount as f32 * 1.4
                + if *amount as f32 >= ctx.player_hp.max(0) as f32 {
                    18.0
                } else {
                    0.0
                })
                * weights.card_value
                * weights.reaction_value
        }
        (
            CardEffect::NextWindSpreadDamage {
                amount, elements, ..
            },
            EnemyAiSkillKind::Attack | EnemyAiSkillKind::Debuff,
        ) if skill.element == Some(ElementType::Wind)
            && elemental_attack
            && (elements.is_empty()
                || ctx
                    .target_attached_auras
                    .iter()
                    .flatten()
                    .any(|aura| elements.contains(aura))) =>
        {
            (14.0 + *amount as f32 * 1.2 + elements.len() as f32 * 4.0)
                * weights.card_value
                * weights.reaction_value
        }
        (CardEffect::NextAuraAttackDraw { amount }, EnemyAiSkillKind::Attack)
        | (CardEffect::NextAuraAttackDraw { amount }, EnemyAiSkillKind::Debuff)
            if elemental_attack && target_has_aura =>
        {
            (10.0 + *amount as f32 * 6.0 + attack_value.min(20.0) * 0.2) * weights.card_value
        }
        (CardEffect::NextSkillCostDraw { skill_cost, draw }, _) if skill.cost_ap == *skill_cost => {
            (8.0 + *draw as f32 * 6.0 + skill.cost_ap as f32 * 1.5) * weights.card_value
        }
        (CardEffect::DrawIfKnockedOutThisTurn { amount }, EnemyAiSkillKind::Attack)
        | (CardEffect::DrawIfKnockedOutThisTurn { amount }, EnemyAiSkillKind::Debuff)
            if attack_value >= ctx.player_hp.max(0) as f32 =>
        {
            (8.0 + *amount as f32 * 5.0 + 18.0) * weights.card_value
        }
        _ => return None,
    };

    Some(score - card.cost_ap.max(0) as f32 * 1.5)
}

pub(crate) fn score_card_for_immediate_use(
    card: &CardDef,
    current_ap: i32,
    hand_len: usize,
    ctx: &EnemyAiContext,
    weights: &EnemyAiWeights,
) -> Option<f32> {
    if current_ap < card.cost_ap {
        return None;
    }

    let hp_ratio = enemy_hp_ratio(ctx);
    let score = match &card.effect {
        CardEffect::GainAp { amount } => 18.0 + *amount as f32 * 8.0,
        CardEffect::GainShield { amount } => {
            let effective_amount = effective_shield_gain(
                *amount,
                ctx.enemy_max_hp,
                ctx.enemy_shield,
                ctx.max_shield_hp_ratio,
            );
            if effective_amount <= 0 {
                return None;
            }
            (effective_amount as f32 * (0.6 + (1.0 - hp_ratio) * 0.7) + 8.0) * weights.shield_value
        }
        CardEffect::ShieldAbsorbGainAp { amount } if ctx.enemy_shield > 0 => {
            (10.0 + *amount as f32 * 6.0 + ctx.enemy_shield.min(12) as f32 * 0.5)
                * weights.shield_value
        }
        CardEffect::GainShieldDrawIfSwitchedThisTurn { shield, draw } => {
            let effective_shield = effective_shield_gain(
                *shield,
                ctx.enemy_max_hp,
                ctx.enemy_shield,
                ctx.max_shield_hp_ratio,
            );
            (effective_shield as f32 * (0.5 + (1.0 - hp_ratio) * 0.6) + *draw as f32 * 2.5 + 7.0)
                * weights.shield_value
        }
        CardEffect::CleanseOrGainAp { fallback_ap, .. } => {
            if ctx.enemy_has_aura || ctx.enemy_has_cleansable_debuff {
                24.0 * weights.status_value
            } else {
                10.0 + *fallback_ap as f32 * 6.0
            }
        }
        CardEffect::DrawAndGainApIfAliveTeam { draw, gain_ap, .. } => {
            12.0 + *draw as f32 * 5.0 + *gain_ap as f32 * 7.0
        }
        CardEffect::DiscardOtherDrawGainAp { draw, gain_ap } if hand_len > 1 => {
            8.0 + *draw as f32 * 5.0 + *gain_ap as f32 * 7.0
        }
        CardEffect::ModifyStages { amount, .. } => 10.0 + amount.abs() as f32 * 5.0,
        _ => return None,
    };

    Some(score * weights.card_value - card.cost_ap.max(0) as f32 * 2.0)
}

pub(crate) fn choose_enemy_immediate_card(
    hand: &[CardId],
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    weights: &EnemyAiWeights,
) -> Option<ScoredEnemyCard> {
    hand.iter()
        .copied()
        .enumerate()
        .filter_map(|(index, card_id)| {
            let card = dbs.cards.get(&card_id)?;
            score_card_for_immediate_use(card, current_ap, hand.len(), ctx, weights)
                .map(|score| ScoredEnemyCard { index, score })
        })
        .filter(|candidate| candidate.score > 0.0)
        .max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.index.cmp(&a.index))
        })
}

pub(crate) fn card_keep_value(card: &CardDef, weights: &EnemyAiWeights) -> f32 {
    let base = match &card.effect {
        CardEffect::GainAp { amount } => 12.0 + *amount as f32 * 7.0,
        CardEffect::NextAttackBoost { amount } => 14.0 + *amount as f32 * weights.attack_value,
        CardEffect::NextShieldBoost { amount } => 12.0 + *amount as f32 * weights.shield_value,
        CardEffect::NextHealBoost { amount } => 12.0 + *amount as f32 * weights.healing_value,
        CardEffect::NextElementAttachmentGainAp { amount } => {
            16.0 + *amount as f32 * 5.0 * weights.reaction_value
        }
        CardEffect::NextReactionFixedDamage { amount, .. } => {
            16.0 + *amount as f32 * weights.reaction_value
        }
        CardEffect::NextWindSpreadDamage {
            amount, elements, ..
        } => 14.0 + *amount as f32 * weights.reaction_value + elements.len() as f32 * 2.0,
        CardEffect::NextAuraAttackDraw { amount } => 12.0 + *amount as f32 * 6.0,
        CardEffect::DiscardOtherDrawGainAp { draw, gain_ap } => {
            10.0 + *draw as f32 * 5.0 + *gain_ap as f32 * 6.0
        }
        CardEffect::NextSkillCostDraw { skill_cost, draw } => {
            10.0 + *skill_cost as f32 * 1.5 + *draw as f32 * 5.0
        }
        CardEffect::DrawIfKnockedOutThisTurn { amount } => 8.0 + *amount as f32 * 4.0,
        CardEffect::GainShield { amount } => 8.0 + *amount as f32 * 0.8 * weights.shield_value,
        CardEffect::ShieldAbsorbGainAp { amount } => 10.0 + *amount as f32 * 5.0,
        CardEffect::ModifyStages { amount, .. } => 12.0 + amount.abs() as f32 * 6.0,
        CardEffect::CleanseOrGainAp { fallback_ap, .. } => 12.0 + *fallback_ap as f32 * 5.0,
        CardEffect::DrawAndGainApIfAliveTeam { draw, gain_ap, .. } => {
            10.0 + *draw as f32 * 5.0 + *gain_ap as f32 * 6.0
        }
        CardEffect::GainShieldDrawIfSwitchedThisTurn { shield, draw } => {
            9.0 + *shield as f32 * 0.6 * weights.shield_value + *draw as f32 * 5.0
        }
    };

    (base * weights.card_value) - card.cost_ap.max(0) as f32 * 2.0
}

pub(crate) fn choose_enemy_discard_card(
    hand: &[CardId],
    dbs: &BattleDbs,
    weights: &EnemyAiWeights,
) -> Option<ScoredEnemyCard> {
    hand.iter()
        .copied()
        .enumerate()
        .filter_map(|(index, card_id)| {
            let card = dbs.cards.get(&card_id)?;
            Some(ScoredEnemyCard {
                index,
                score: card_keep_value(card, weights),
            })
        })
        .min_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.index.cmp(&b.index))
        })
}

fn projected_ap_after_immediate_card(
    card: &CardDef,
    current_ap: i32,
    ctx: &EnemyAiContext,
) -> Option<i32> {
    if current_ap < card.cost_ap {
        return None;
    }
    let gained_ap = match &card.effect {
        CardEffect::GainAp { amount } => *amount,
        CardEffect::ShieldAbsorbGainAp { amount } if ctx.enemy_shield > 0 => *amount,
        CardEffect::CleanseOrGainAp { fallback_ap, .. }
            if !ctx.enemy_has_aura && !ctx.enemy_has_cleansable_debuff =>
        {
            *fallback_ap
        }
        CardEffect::DrawAndGainApIfAliveTeam { gain_ap, .. } => *gain_ap,
        _ => return None,
    };
    Some(current_ap - card.cost_ap + gained_ap)
}

fn projected_switch_followup_score(
    switch_context: Option<(
        &EnemySwitchCandidate,
        &[EnemySwitchCandidate],
        f32,
        bool,
        f32,
        Option<PlayerThreatContext>,
    )>,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    weights: &EnemyAiWeights,
) -> f32 {
    switch_context
        .and_then(
            |(
                current,
                candidates,
                current_best_action,
                already_switched,
                switch_score_threshold,
                player_threat,
            )| {
                choose_enemy_switch(
                    current,
                    candidates,
                    current_best_action,
                    current_ap,
                    dbs,
                    ctx,
                    already_switched,
                    switch_score_threshold,
                    weights,
                    player_threat.as_ref(),
                )
                .map(|chosen| chosen.score)
            },
        )
        .unwrap_or(0.0)
}

type EnemySwitchContext<'a> = Option<(
    &'a EnemySwitchCandidate,
    &'a [EnemySwitchCandidate],
    f32,
    bool,
    f32,
    Option<PlayerThreatContext>,
)>;

pub(crate) fn choose_enemy_discard_for_followup(
    hand: &[CardId],
    current_ap: i32,
    skill_ids: &[SkillId; 4],
    skill_count: usize,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    switch_context: EnemySwitchContext<'_>,
    weights: &EnemyAiWeights,
) -> Option<ScoredEnemyDiscard> {
    hand.iter()
        .copied()
        .enumerate()
        .filter_map(|(index, card_id)| {
            let card = dbs.cards.get(&card_id)?;
            let keep_score = card_keep_value(card, weights);
            let remaining_hand = hand
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(other_index, other_card_id)| {
                    (other_index != index).then_some(other_card_id)
                })
                .collect::<Vec<_>>();
            let ap_after_discard = current_ap + 1;
            let followup_skill_score =
                choose_enemy_skill(skill_ids, skill_count, ap_after_discard, dbs, ctx, weights)
                    .map(|chosen| chosen.score)
                    .unwrap_or(0.0);
            let followup_switch_score = projected_switch_followup_score(
                switch_context,
                ap_after_discard,
                dbs,
                ctx,
                weights,
            );
            let followup_card_score =
                choose_enemy_immediate_card(&remaining_hand, ap_after_discard, dbs, ctx, weights)
                    .map(|chosen| chosen.score)
                    .unwrap_or(0.0);
            let best_card_chain = remaining_hand
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(card_index, card_id)| {
                    let card = dbs.cards.get(&card_id)?;
                    let immediate_score = score_card_for_immediate_use(
                        card,
                        ap_after_discard,
                        remaining_hand.len(),
                        ctx,
                        weights,
                    )?;
                    if immediate_score <= 0.0 {
                        return None;
                    }
                    let ap_after_card =
                        projected_ap_after_immediate_card(card, ap_after_discard, ctx)?;
                    if ap_after_card <= ap_after_discard {
                        return None;
                    }
                    let skill_after_card = choose_enemy_skill(
                        skill_ids,
                        skill_count,
                        ap_after_card,
                        dbs,
                        ctx,
                        weights,
                    )
                    .map(|chosen| chosen.score)
                    .unwrap_or(0.0);
                    let switch_after_card = projected_switch_followup_score(
                        switch_context,
                        ap_after_card,
                        dbs,
                        ctx,
                        weights,
                    );
                    let (sequence_score, followup) = if skill_after_card >= switch_after_card {
                        (
                            immediate_score + skill_after_card * 0.75,
                            EnemyDiscardFollowUp::ImmediateCardThenSkill,
                        )
                    } else {
                        (
                            immediate_score + switch_after_card * 0.75,
                            EnemyDiscardFollowUp::ImmediateCardThenSwitch,
                        )
                    };
                    (sequence_score > immediate_score).then_some((
                        sequence_score,
                        followup,
                        card_index,
                    ))
                })
                .max_by(|a, b| {
                    a.0.partial_cmp(&b.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| b.2.cmp(&a.2))
                })
                .map(|(score, followup, _)| (score, followup));
            let (followup_score, followup) = [
                Some((followup_skill_score, EnemyDiscardFollowUp::Skill)),
                Some((followup_switch_score, EnemyDiscardFollowUp::Switch)),
                best_card_chain,
                Some((followup_card_score, EnemyDiscardFollowUp::ImmediateCard)),
            ]
            .into_iter()
            .flatten()
            .max_by(|a, b| {
                a.0.partial_cmp(&b.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        discard_followup_priority(a.1).cmp(&discard_followup_priority(b.1))
                    })
            })
            .unwrap_or((0.0, EnemyDiscardFollowUp::None));
            let score = followup_score - keep_score * weights.discard_value * 0.35
                + if followup_score > 0.0 { 6.0 } else { 0.0 };
            Some(ScoredEnemyDiscard {
                index,
                score,
                keep_score,
                followup_score,
                followup,
            })
        })
        .max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.index.cmp(&a.index))
        })
}

pub(crate) fn choose_enemy_plan_candidates(
    hand: &[CardId],
    current_ap: i32,
    skill_ids: &[SkillId; 4],
    skill_count: usize,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    switch_context: EnemySwitchContext<'_>,
    weights: &EnemyAiWeights,
    player_threat: Option<&PlayerThreatContext>,
    search_depth: usize,
    top_candidates: usize,
) -> Vec<EnemyAiPlan> {
    let candidate_cap = top_candidates.max(1);
    let depth = search_depth.max(1);
    let mut plans = Vec::new();

    let chosen_skill = choose_enemy_skill_with_threat(
        skill_ids,
        skill_count,
        current_ap,
        dbs,
        ctx,
        weights,
        player_threat,
    );

    if let Some(skill_candidate) = chosen_skill {
        plans.push(EnemyAiPlan {
            action: EnemyPlannedAction::UseSkill(skill_candidate),
            score: skill_candidate.score,
            summary: format!(
                "UseSkill(slot={}, skill={:?}, score={:.2})",
                skill_candidate.slot, skill_candidate.skill_id, skill_candidate.score
            ),
        });

        if let Some(skill) = dbs.skills.get(&skill_candidate.skill_id) {
            let available_ap_after_skill = current_ap - skill.cost_ap;
            if let Some((card_index, card_score)) = hand
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(card_index, card_id)| {
                    let card = dbs.cards.get(&card_id)?;
                    score_card_for_skill(
                        card,
                        skill_candidate.kind,
                        skill,
                        ctx,
                        dbs,
                        available_ap_after_skill,
                        weights,
                    )
                    .filter(|score| *score > 0.0)
                    .map(|score| (card_index, score))
                })
                .max_by(|a, b| {
                    a.1.partial_cmp(&b.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| b.0.cmp(&a.0))
                })
            {
                let chain_bonus = if depth > 1 { 0.9 } else { 0.6 };
                let total_score = skill_candidate.score + card_score * chain_bonus;
                plans.push(EnemyAiPlan {
                    action: EnemyPlannedAction::UseCardForSkill {
                        card_index,
                        skill: skill_candidate,
                    },
                    score: total_score,
                    summary: format!(
                        "UseCardForSkill(card_index={}, skill_slot={}, card_score={:.2}, total={:.2})",
                        card_index, skill_candidate.slot, card_score, total_score
                    ),
                });
            }
        }
    }

    if let Some((current, candidates, current_best_action, already_switched, threshold, threat)) =
        switch_context
        && let Some(chosen_switch) = choose_enemy_switch(
            current,
            candidates,
            current_best_action,
            current_ap,
            dbs,
            ctx,
            already_switched,
            threshold,
            weights,
            threat.as_ref(),
        )
    {
        plans.push(EnemyAiPlan {
            action: EnemyPlannedAction::Switch(chosen_switch.clone()),
            score: chosen_switch.score,
            summary: format!(
                "Switch(index={}, score={:.2})",
                chosen_switch.index, chosen_switch.score
            ),
        });
    }

    if let Some(immediate_card) = choose_enemy_immediate_card(hand, current_ap, dbs, ctx, weights) {
        let mut total_score = immediate_card.score;
        if depth > 1
            && let Some(card_id) = hand.get(immediate_card.index)
            && let Some(card) = dbs.cards.get(card_id)
            && let Some(ap_after_card) = projected_ap_after_immediate_card(card, current_ap, ctx)
        {
            let followup_skill = choose_enemy_skill_with_threat(
                skill_ids,
                skill_count,
                ap_after_card,
                dbs,
                ctx,
                weights,
                player_threat,
            )
            .map(|chosen| chosen.score)
            .unwrap_or(0.0);
            let followup_switch =
                projected_switch_followup_score(switch_context, ap_after_card, dbs, ctx, weights);
            total_score += followup_skill.max(followup_switch) * 0.65;
        }
        plans.push(EnemyAiPlan {
            action: EnemyPlannedAction::ImmediateCard(immediate_card.clone()),
            score: total_score,
            summary: format!(
                "ImmediateCard(index={}, score={:.2})",
                immediate_card.index, total_score
            ),
        });
    }

    if !hand.is_empty()
        && let Some(discard) = choose_enemy_discard_for_followup(
            hand,
            current_ap,
            skill_ids,
            skill_count,
            dbs,
            ctx,
            switch_context,
            weights,
        )
        && discard.score > 0.0
    {
        plans.push(EnemyAiPlan {
            action: EnemyPlannedAction::Discard(discard.clone()),
            score: discard.score,
            summary: format!(
                "Discard(index={}, followup={:?}, score={:.2})",
                discard.index, discard.followup, discard.score
            ),
        });
    }

    plans.push(EnemyAiPlan {
        action: EnemyPlannedAction::EndTurn,
        score: 0.0,
        summary: "EndTurn".to_string(),
    });

    plans.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                planned_action_priority(&b.action).cmp(&planned_action_priority(&a.action))
            })
    });
    plans.truncate(candidate_cap);
    plans
}

#[cfg(test)]
pub(crate) fn choose_enemy_plan(
    hand: &[CardId],
    current_ap: i32,
    skill_ids: &[SkillId; 4],
    skill_count: usize,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    switch_context: EnemySwitchContext<'_>,
    weights: &EnemyAiWeights,
    player_threat: Option<&PlayerThreatContext>,
    search_depth: usize,
    top_candidates: usize,
) -> Option<EnemyAiPlan> {
    choose_enemy_plan_candidates(
        hand,
        current_ap,
        skill_ids,
        skill_count,
        dbs,
        ctx,
        switch_context,
        weights,
        player_threat,
        search_depth,
        top_candidates,
    )
    .into_iter()
    .next()
}

fn enemy_hp_ratio(ctx: &EnemyAiContext) -> f32 {
    if ctx.enemy_max_hp <= 0 {
        0.0
    } else {
        ctx.enemy_hp.max(0) as f32 / ctx.enemy_max_hp as f32
    }
}

fn remaining_shield_room(max_hp: i32, shield: i32, max_hp_ratio: f32) -> i32 {
    (Shield::max_for_hp(max_hp, max_hp_ratio) - shield.max(0)).max(0)
}

fn effective_shield_gain(amount: i32, max_hp: i32, shield: i32, max_hp_ratio: f32) -> i32 {
    amount
        .max(0)
        .min(remaining_shield_room(max_hp, shield, max_hp_ratio))
}

fn primary_effect(effect: &SkillEffect) -> Option<&SkillEffect> {
    match effect {
        SkillEffect::Sequence { effects } => effects.first().and_then(primary_effect),
        SkillEffect::Conditional { .. } => None,
        _ => Some(effect),
    }
}

fn primary_attack_power(effect: &SkillEffect) -> Option<i32> {
    match primary_effect(effect)? {
        SkillEffect::Attack { power, .. } => Some(*power),
        _ => None,
    }
}

fn condition_matches_precast(
    condition: &crate::data::SkillCondition,
    ctx: &EnemyAiContext,
) -> bool {
    match condition {
        crate::data::SkillCondition::TargetHadAura { element } => ctx
            .target_attached_auras
            .iter()
            .flatten()
            .any(|aura| aura == element),
        crate::data::SkillCondition::TargetHadStatus { status_id } => {
            ctx.target_status_ids.iter().any(|id| id == status_id)
        }
        crate::data::SkillCondition::TargetHadNoShield => ctx.player_shield <= 0,
        crate::data::SkillCondition::Any { conditions } => conditions
            .iter()
            .any(|nested| condition_matches_precast(nested, ctx)),
        crate::data::SkillCondition::All { conditions } => conditions
            .iter()
            .all(|nested| condition_matches_precast(nested, ctx)),
        crate::data::SkillCondition::LastReactionName { .. }
        | crate::data::SkillCondition::LastWindSpreadSucceeded
        | crate::data::SkillCondition::LastWindSpreadFailed
        | crate::data::SkillCondition::LastCleanseSucceeded
        | crate::data::SkillCondition::LastCleanseFailed
        | crate::data::SkillCondition::LastTargetFainted => false,
    }
}

fn conditional_bonus_score(effect: &SkillEffect, ctx: &EnemyAiContext) -> f32 {
    match effect {
        SkillEffect::Conditional { branches } => branches
            .iter()
            .filter(|branch| condition_matches_precast(&branch.condition, ctx))
            .map(|branch| match branch.effect.as_ref() {
                SkillEffect::DealFixedDamage { amount, .. } => *amount as f32,
                SkillEffect::DealStatDifferenceDamage { .. } => 10.0,
                SkillEffect::Dispel { status_ids, .. } => dispel_value(status_ids, ctx),
                SkillEffect::ModifyStages { .. } | SkillEffect::ApplyStatus { .. } => 8.0,
                SkillEffect::Heal { amount } => (*amount as f32) * 0.6,
                SkillEffect::Shield { amount } => (*amount as f32) * 0.4,
                _ => 0.0,
            })
            .sum(),
        _ => 0.0,
    }
}

fn dispel_value(status_ids: &[String], ctx: &EnemyAiContext) -> f32 {
    let removed_count = status_ids
        .iter()
        .map(|status_id| {
            if matches!(
                status_id.as_str(),
                "stage_shift_buff" | "stage_shift_debuff"
            ) {
                ctx.target_status_ids
                    .iter()
                    .filter(|id| *id == status_id || id.starts_with(&format!("{status_id}_")))
                    .count()
            } else if ctx.target_status_ids.iter().any(|id| id == status_id) {
                1
            } else {
                0
            }
        })
        .sum::<usize>();

    removed_count as f32 * 12.0
}

fn cleanse_value(
    prefer_aura: bool,
    fallback_to_debuff: bool,
    amount: usize,
    ctx: &EnemyAiContext,
) -> f32 {
    let mut value = 0.0;
    let mut remaining = amount;
    let mut aura_available = ctx.enemy_has_aura;
    let mut debuff_available = ctx.enemy_has_cleansable_debuff;

    while remaining > 0 {
        if prefer_aura && aura_available {
            value += 18.0;
            aura_available = false;
            remaining -= 1;
            continue;
        }
        if fallback_to_debuff && debuff_available {
            value += 14.0;
            debuff_available = false;
            remaining -= 1;
            continue;
        }
        break;
    }

    value
}

fn status_future_value(status_id: &str, dbs: &BattleDbs) -> f32 {
    let Some(status) = dbs.statuses.statuses.get(status_id) else {
        return 8.0;
    };
    let duration = status.duration_turns.max(1) as f32;
    let tick_damage_value = status.fixed_damage_on_tick.max(0) as f32 * duration * 0.85;
    let heal_pressure = status.heal_taken_multiplier.map_or(0.0, |multiplier| {
        if multiplier < 1.0 {
            (1.0 - multiplier) * 18.0 * duration.min(3.0)
        } else {
            0.0
        }
    });
    let stage_value = status
        .stage_modifiers
        .iter()
        .map(|modifier| (-modifier.amount).max(0) as f32 * 7.0)
        .sum::<f32>();
    let evade_value = status.evade_charges.max(0) as f32 * 22.0;
    let category_value = match status.category {
        crate::data::StatusCategory::Debuff => 10.0,
        crate::data::StatusCategory::Special => 8.0,
        crate::data::StatusCategory::Aura => 5.0,
        crate::data::StatusCategory::Buff => -8.0,
    };

    (tick_damage_value + heal_pressure + stage_value + evade_value + category_value).max(0.0)
}

fn target_has_status(status_ids: &[String], status_id: &str) -> bool {
    status_ids.iter().any(|id| id == status_id)
}

fn skill_targets_self(skill_category: SkillCategory) -> bool {
    matches!(
        skill_category,
        SkillCategory::SelfUtility | SkillCategory::AllyUtility
    )
}

fn effect_targets_self(target: EffectTarget, skill_category: SkillCategory) -> bool {
    match target {
        EffectTarget::SelfTarget => true,
        EffectTarget::Opponent => false,
        EffectTarget::Infer => skill_targets_self(skill_category),
    }
}

fn target_status_ids_for_skill(skill_category: SkillCategory, ctx: &EnemyAiContext) -> &[String] {
    if skill_targets_self(skill_category) {
        &ctx.enemy_status_ids
    } else {
        &ctx.target_status_ids
    }
}

fn stage_modifier_unit_value(attribute: AttributeType, target_self: bool) -> f32 {
    match (attribute, target_self) {
        (AttributeType::Atk, true) => 9.5,
        (AttributeType::Def, true) => 8.0,
        (AttributeType::Spd, true) => 6.5,
        (AttributeType::Acc, true) => 7.5,
        (AttributeType::Atk, false) => 8.5,
        (AttributeType::Def, false) => 8.0,
        (AttributeType::Spd, false) => 6.0,
        (AttributeType::Acc, false) => 6.5,
    }
}

fn stage_modifiers_value(
    modifiers: &[AttributeStageModifier],
    target_self: bool,
    ctx: &EnemyAiContext,
) -> f32 {
    modifiers
        .iter()
        .map(|modifier| {
            let current_stage = if target_self {
                match modifier.attribute {
                    AttributeType::Atk => ctx.enemy_atk_stage,
                    AttributeType::Def => ctx.enemy_def_stage,
                    AttributeType::Spd => ctx.enemy_spd_stage,
                    AttributeType::Acc => ctx.enemy_acc_stage,
                }
            } else {
                match modifier.attribute {
                    AttributeType::Atk => ctx.player_atk_stage,
                    AttributeType::Def => ctx.player_def_stage,
                    AttributeType::Spd => ctx.player_spd_stage,
                    AttributeType::Acc => ctx.player_acc_stage,
                }
            };
            let helpful =
                (target_self && modifier.amount > 0) || (!target_self && modifier.amount < 0);
            if !helpful {
                return 0.0;
            }
            let useful_steps = if target_self {
                modifier.amount.min(6 - current_stage).max(0)
            } else {
                (-modifier.amount).min(current_stage - (-6)).max(0)
            } as f32;
            if useful_steps <= 0.0 {
                return 0.0;
            }
            let existing_pressure = if target_self {
                current_stage.max(0) as f32
            } else {
                (-current_stage).max(0) as f32
            };
            let repeat_factor = 1.0 / (1.0 + existing_pressure * 1.35);
            stage_modifier_unit_value(modifier.attribute, target_self)
                * useful_steps
                * repeat_factor
        })
        .sum()
}

fn apply_status_tactical_value(
    skill_category: SkillCategory,
    status_id: &str,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
) -> f32 {
    let status_ids = target_status_ids_for_skill(skill_category, ctx);
    if target_has_status(status_ids, status_id) {
        0.0
    } else {
        status_future_value(status_id, dbs)
    }
}

fn modifier_pressure(modifier: &crate::data::AttributeStageModifier) -> f32 {
    (-modifier.amount).max(0) as f32 * 8.0
}

fn reaction_matches_context(
    reaction: &crate::data::ReactionDef,
    current_auras: &[ElementType],
    current_status_ids: &[String],
    incoming_element: ElementType,
) -> bool {
    reaction.trigger_element == incoming_element
        && (reaction.required_elements.is_empty()
            || reaction.required_elements.contains(&incoming_element))
        && reaction
            .required_elements
            .iter()
            .filter(|element| **element != incoming_element)
            .all(|element| current_auras.contains(element))
        && reaction
            .required_statuses
            .iter()
            .all(|status_id| current_status_ids.iter().any(|id| id == status_id))
}

fn reaction_priority(reaction: &crate::data::ReactionDef) -> (usize, usize) {
    (
        reaction.required_statuses.len(),
        reaction.required_elements.len(),
    )
}

fn choose_reaction<'a>(
    reactions: &'a ReactionDb,
    current_auras: &[ElementType],
    current_status_ids: &[String],
    incoming_element: ElementType,
) -> Option<&'a crate::data::ReactionDef> {
    let mut best = None;
    for reaction in reactions.reactions.iter().filter(|reaction| {
        reaction_matches_context(
            reaction,
            current_auras,
            current_status_ids,
            incoming_element,
        )
    }) {
        if best.is_none_or(|best_reaction| {
            reaction_priority(reaction) > reaction_priority(best_reaction)
        }) {
            best = Some(reaction);
        }
    }
    best
}

fn reaction_value(
    reaction: &crate::data::ReactionDef,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
) -> f32 {
    let shielded_damage = (reaction.fixed_damage - ctx.player_shield.max(0)).max(0) as f32;
    let shield_break_value = reaction.fixed_damage.min(ctx.player_shield.max(0)) as f32 * 0.35;
    let kill_bonus = if shielded_damage >= ctx.player_hp.max(0) as f32 {
        28.0
    } else {
        0.0
    };
    let heal_value = reaction.heal_attacker.max(0) as f32 * 0.65;
    let status_value = reaction
        .apply_statuses
        .iter()
        .map(|status_id| status_future_value(status_id, dbs))
        .sum::<f32>();
    let clear_penalty = reaction
        .clear_statuses
        .iter()
        .filter(|status_id| ctx.target_status_ids.iter().any(|id| id == *status_id))
        .map(|status_id| status_future_value(status_id, dbs) * 0.5)
        .sum::<f32>();
    let aura_value = reaction.aura_results.len() as f32 * 3.0;

    shielded_damage + shield_break_value + kill_bonus + heal_value + status_value + aura_value
        - clear_penalty
}

fn estimate_reaction_value(
    incoming_element: ElementType,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
) -> f32 {
    let current_auras = ctx
        .target_attached_auras
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let Some(reaction) = choose_reaction(
        &dbs.reactions,
        &current_auras,
        &ctx.target_status_ids,
        incoming_element,
    ) else {
        return 0.0;
    };

    reaction_value(reaction, ctx, dbs)
}

fn estimate_aura_setup_value(
    incoming_element: ElementType,
    hp_damage: f32,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
) -> f32 {
    if hp_damage <= 0.0 {
        return 0.0;
    }
    if ctx
        .target_attached_auras
        .iter()
        .flatten()
        .any(|aura| *aura == incoming_element)
    {
        return 2.0;
    }

    let future_reaction_value = dbs
        .reactions
        .reactions
        .iter()
        .filter(|reaction| {
            reaction.trigger_element != incoming_element
                && reaction.required_elements.contains(&incoming_element)
        })
        .map(|reaction| {
            let status_factor = if reaction
                .required_statuses
                .iter()
                .all(|status_id| ctx.target_status_ids.iter().any(|id| id == status_id))
            {
                1.0
            } else {
                0.45
            };
            reaction_value(reaction, ctx, dbs).max(0.0) * status_factor
        })
        .fold(0.0, f32::max);

    let aura_slots_used = ctx.target_attached_auras.iter().flatten().count();
    let slot_factor = match aura_slots_used {
        0 => 1.0,
        1 => 0.8,
        _ => 0.55,
    };
    let setup_base = if future_reaction_value > 0.0 {
        6.0
    } else {
        3.0
    };

    (setup_base + future_reaction_value * 0.45) * slot_factor
}

fn fixed_damage_value(amount: i32, ctx: &EnemyAiContext) -> f32 {
    let shielded_damage = (amount - ctx.player_shield.max(0)).max(0) as f32;
    let shield_break_value = amount.min(ctx.player_shield.max(0)) as f32 * 0.35;
    let kill_bonus = if shielded_damage >= ctx.player_hp.max(0) as f32 {
        28.0
    } else {
        0.0
    };
    shielded_damage + shield_break_value + kill_bonus
}

fn tactical_effect_value(effect: &SkillEffect, ctx: &EnemyAiContext, dbs: &BattleDbs) -> f32 {
    match effect {
        SkillEffect::ApplyStatus { status_id } => {
            apply_status_tactical_value(SkillCategory::EnemyDebuff, status_id, ctx, dbs)
        }
        SkillEffect::ModifyStages {
            modifiers, target, ..
        } => {
            let target_self = effect_targets_self(*target, SkillCategory::EnemyDebuff);
            stage_modifiers_value(modifiers, target_self, ctx)
                .max(modifiers.iter().map(modifier_pressure).sum::<f32>())
        }
        SkillEffect::Dispel { status_ids, .. } => dispel_value(status_ids, ctx),
        SkillEffect::DealFixedDamage { amount, .. } => fixed_damage_value(*amount, ctx),
        SkillEffect::DealStatDifferenceDamage { .. } => 14.0,
        SkillEffect::Conditional { branches } => branches
            .iter()
            .filter(|branch| condition_matches_precast(&branch.condition, ctx))
            .map(|branch| tactical_effect_value(&branch.effect, ctx, dbs))
            .sum(),
        SkillEffect::Sequence { effects } => effects
            .iter()
            .map(|effect| tactical_effect_value(effect, ctx, dbs))
            .sum(),
        _ => 0.0,
    }
}

fn estimate_attack_value(skill: &SkillDef, ctx: &EnemyAiContext, dbs: &BattleDbs) -> f32 {
    estimate_attack_value_with_atk(skill, ctx.enemy_atk, ctx, dbs)
}

fn estimate_attack_value_with_atk(
    skill: &SkillDef,
    attacker_atk: i32,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
) -> f32 {
    let Some(power) = primary_attack_power(&skill.effect) else {
        return 0.0;
    };

    let raw = (power + attacker_atk - ctx.player_def).max(1) as f32;
    let effectiveness = if let Some(skill_element) = skill.element {
        let defender_element = if ctx.player_shield > 0 {
            ctx.target_element
        } else {
            ctx.target_attached_auras
                .iter()
                .flatten()
                .copied()
                .next()
                .unwrap_or(ctx.target_element)
        };
        dbs.elements
            .get_effectiveness(skill_element, defender_element)
    } else {
        1.0
    };

    let theoretical_damage = (raw * effectiveness).max(1.0);
    let hp_damage = (theoretical_damage - ctx.player_shield.max(0) as f32).max(0.0);
    let mut score = hp_damage;

    if effectiveness > 1.0 {
        score += 18.0 + (effectiveness - 1.0) * 22.0;
    } else if effectiveness < 1.0 {
        score -= 16.0 + (1.0 - effectiveness) * 24.0;
    }

    if hp_damage >= ctx.player_hp.max(0) as f32 {
        score += 28.0;
    }

    if let Some(skill_element) = skill.element {
        let reaction_value = estimate_reaction_value(skill_element, ctx, dbs);
        if reaction_value > 0.0 {
            score += reaction_value + 6.0;
        } else {
            score += estimate_aura_setup_value(skill_element, hp_damage, ctx, dbs);
        }
    }

    score
}

fn score_enemy_skill(
    slot: usize,
    skill_id: SkillId,
    skill: &SkillDef,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
) -> ScoredEnemySkill {
    let hp_ratio = enemy_hp_ratio(ctx);

    match &skill.effect {
        SkillEffect::Attack { .. } => {
            let mut score = estimate_attack_value(skill, ctx, dbs);
            score += 10.0;
            if hp_ratio >= 0.7 {
                score += 12.0;
            } else if hp_ratio <= 0.35 {
                score -= 2.0;
            }
            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind: EnemyAiSkillKind::Attack,
            }
        }
        SkillEffect::Heal { amount } => {
            let missing_hp = (ctx.enemy_max_hp - ctx.enemy_hp).max(0) as f32;
            let effective_heal = (*amount as f32).min(missing_hp);
            let urgency = 1.0 - hp_ratio;
            let mut score = effective_heal * (0.5 + urgency * 1.8);

            if hp_ratio <= 0.25 {
                score += 35.0;
            } else if hp_ratio <= 0.4 {
                score += 18.0;
            } else if hp_ratio >= 0.8 {
                score -= 24.0;
            }

            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind: EnemyAiSkillKind::Heal,
            }
        }
        SkillEffect::Shield { amount } => {
            let effective_amount = effective_shield_gain(
                *amount,
                ctx.enemy_max_hp,
                ctx.enemy_shield,
                ctx.max_shield_hp_ratio,
            );
            let mut score = if effective_amount > 0 {
                effective_amount as f32 + 5.0
            } else {
                1.0
            };
            if effective_amount > 0 && ctx.enemy_shield <= 0 {
                score += 2.0;
            }
            if effective_amount > 0 && hp_ratio <= 0.35 {
                score += 3.0;
            }
            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind: EnemyAiSkillKind::Shield,
            }
        }
        SkillEffect::ApplyStatus { status_id } => {
            let self_target = skill_targets_self(skill.category);
            let status_ids = target_status_ids_for_skill(skill.category, ctx);
            let already_has = target_has_status(status_ids, status_id);
            let mut score = if already_has {
                2.0
            } else if self_target {
                10.0 + status_future_value(status_id, dbs)
            } else {
                24.0 + status_future_value(status_id, dbs) + (1.0 - hp_ratio) * 8.0
            };
            if skill.category == SkillCategory::EnemyDebuff && !already_has {
                score += 10.0;
            }
            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind: EnemyAiSkillKind::Debuff,
            }
        }
        SkillEffect::ModifyStages {
            modifiers, target, ..
        } => {
            let self_target = effect_targets_self(*target, skill.category);
            let stage_value = stage_modifiers_value(modifiers, self_target, ctx).max(0.0);
            let mut score = if stage_value <= 0.0 {
                2.0
            } else if self_target {
                8.0 + stage_value
            } else {
                24.0 + stage_value + (1.0 - hp_ratio) * 8.0
            };
            if skill.category == SkillCategory::EnemyDebuff && !self_target {
                score += 10.0;
            }
            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind: if self_target {
                    EnemyAiSkillKind::Shield
                } else {
                    EnemyAiSkillKind::Debuff
                },
            }
        }
        SkillEffect::Cleanse { .. }
        | SkillEffect::Dispel { .. }
        | SkillEffect::DealFixedDamage { .. }
        | SkillEffect::DealStatDifferenceDamage { .. }
        | SkillEffect::Conditional { .. } => {
            let mut score =
                24.0 + tactical_effect_value(&skill.effect, ctx, dbs) + (1.0 - hp_ratio) * 8.0;
            if skill.category == SkillCategory::EnemyDebuff {
                score += 10.0;
            }
            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind: EnemyAiSkillKind::Debuff,
            }
        }
        SkillEffect::Sequence { effects } => {
            let contains_attack = effects
                .iter()
                .any(|effect| matches!(effect, SkillEffect::Attack { .. }));
            let contains_heal = effects
                .iter()
                .any(|effect| matches!(effect, SkillEffect::Heal { .. }));
            let contains_shield = effects
                .iter()
                .any(|effect| matches!(effect, SkillEffect::Shield { .. }));
            let contains_debuff = effects.iter().any(|effect| {
                matches!(
                    effect,
                    SkillEffect::ApplyStatus { .. }
                        | SkillEffect::ModifyStages { .. }
                        | SkillEffect::Cleanse { .. }
                        | SkillEffect::Dispel { .. }
                        | SkillEffect::DealFixedDamage { .. }
                        | SkillEffect::Conditional { .. }
                )
            });
            let conditional_bonus = effects
                .iter()
                .map(|effect| conditional_bonus_score(effect, ctx))
                .sum::<f32>();
            let followup_bonus = effects
                .iter()
                .skip(1)
                .map(|effect| tactical_effect_value(effect, ctx, dbs))
                .sum::<f32>();
            let (score, kind) = match primary_effect(&skill.effect) {
                Some(SkillEffect::Attack { .. }) => (
                    estimate_attack_value(skill, ctx, dbs)
                        + 14.0
                        + conditional_bonus
                        + followup_bonus,
                    EnemyAiSkillKind::Attack,
                ),
                Some(SkillEffect::Heal { amount }) => {
                    let missing_hp = (ctx.enemy_max_hp - ctx.enemy_hp).max(0) as f32;
                    let effective_heal = (*amount as f32).min(missing_hp);
                    let urgency = 1.0 - hp_ratio;
                    let mut score = effective_heal * (0.5 + urgency * 1.8);
                    if hp_ratio <= 0.25 {
                        score += 35.0;
                    } else if hp_ratio <= 0.4 {
                        score += 18.0;
                    } else if hp_ratio >= 0.8 {
                        score -= 24.0;
                    }
                    (score, EnemyAiSkillKind::Heal)
                }
                Some(SkillEffect::Shield { amount }) => {
                    let effective_amount = effective_shield_gain(
                        *amount,
                        ctx.enemy_max_hp,
                        ctx.enemy_shield,
                        ctx.max_shield_hp_ratio,
                    );
                    let mut score = if effective_amount > 0 {
                        effective_amount as f32 + 5.0
                    } else {
                        1.0
                    };
                    if effective_amount > 0 && ctx.enemy_shield <= 0 {
                        score += 2.0;
                    }
                    if effective_amount > 0 && hp_ratio <= 0.35 {
                        score += 3.0;
                    }
                    (score, EnemyAiSkillKind::Shield)
                }
                Some(SkillEffect::Cleanse {
                    prefer_aura,
                    fallback_to_debuff,
                    amount,
                }) => {
                    let score = cleanse_value(*prefer_aura, *fallback_to_debuff, *amount, ctx)
                        + 18.0
                        + (1.0 - hp_ratio) * 10.0;
                    (score, EnemyAiSkillKind::Heal)
                }
                Some(SkillEffect::Dispel { status_ids, .. }) => (
                    24.0 + dispel_value(status_ids, ctx),
                    EnemyAiSkillKind::Debuff,
                ),
                Some(SkillEffect::ApplyStatus { status_id }) => {
                    let self_target = skill_targets_self(skill.category);
                    let status_ids = target_status_ids_for_skill(skill.category, ctx);
                    let already_has = target_has_status(status_ids, status_id);
                    let mut score = if already_has {
                        tactical_effect_value(&skill.effect, ctx, dbs).min(4.0)
                    } else if self_target {
                        10.0 + tactical_effect_value(&skill.effect, ctx, dbs)
                    } else {
                        24.0 + tactical_effect_value(&skill.effect, ctx, dbs)
                            + (1.0 - hp_ratio) * 8.0
                    };
                    if skill.category == SkillCategory::EnemyDebuff && !already_has {
                        score += 10.0;
                    }
                    (score, EnemyAiSkillKind::Debuff)
                }
                Some(SkillEffect::ModifyStages {
                    modifiers, target, ..
                }) => {
                    let self_target = effect_targets_self(*target, skill.category);
                    let stage_value = stage_modifiers_value(modifiers, self_target, ctx).max(0.0);
                    let score = if stage_value <= 0.0 {
                        2.0
                    } else if self_target {
                        8.0 + stage_value
                    } else {
                        24.0 + stage_value + (1.0 - hp_ratio) * 8.0
                    };
                    (
                        score,
                        if self_target {
                            EnemyAiSkillKind::Shield
                        } else {
                            EnemyAiSkillKind::Debuff
                        },
                    )
                }
                Some(SkillEffect::DealFixedDamage { .. })
                | Some(SkillEffect::DealStatDifferenceDamage { .. })
                | Some(SkillEffect::Conditional { .. }) => {
                    let mut score = 24.0
                        + tactical_effect_value(&skill.effect, ctx, dbs)
                        + (1.0 - hp_ratio) * 8.0;
                    if skill.category == SkillCategory::EnemyDebuff {
                        score += 10.0;
                    }
                    (score, EnemyAiSkillKind::Debuff)
                }
                Some(SkillEffect::Sequence { .. }) | None => {
                    if contains_attack {
                        (
                            estimate_attack_value(skill, ctx, dbs) + 14.0,
                            EnemyAiSkillKind::Attack,
                        )
                    } else if contains_heal {
                        (32.0 + (1.0 - hp_ratio) * 28.0, EnemyAiSkillKind::Heal)
                    } else if contains_shield {
                        (30.0 + (1.0 - hp_ratio) * 16.0, EnemyAiSkillKind::Shield)
                    } else if contains_debuff {
                        (48.0, EnemyAiSkillKind::Debuff)
                    } else {
                        (20.0, EnemyAiSkillKind::Debuff)
                    }
                }
            };
            ScoredEnemySkill {
                slot,
                skill_id,
                score,
                kind,
            }
        }
    }
}

pub(crate) fn best_attack_value(
    skill_ids: &[SkillId; 4],
    skill_count: usize,
    attacker_atk: i32,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
) -> f32 {
    skill_ids
        .iter()
        .copied()
        .take(skill_count)
        .filter_map(|skill_id| dbs.skills.get(&skill_id))
        .filter(|skill| current_ap >= skill.cost_ap)
        .map(|skill| estimate_attack_value_with_atk(skill, attacker_atk, ctx, dbs))
        .fold(0.0, f32::max)
}

pub(crate) fn best_action_value(
    candidate: &EnemySwitchCandidate,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    weights: &EnemyAiWeights,
) -> f32 {
    if candidate.hp <= 0 || current_ap <= 0 {
        return 0.0;
    }

    let mut action_ctx = ctx.clone();
    action_ctx.enemy_hp = candidate.hp;
    action_ctx.enemy_max_hp = candidate.max_hp;
    action_ctx.enemy_shield = candidate.shield;
    action_ctx.max_shield_hp_ratio = candidate.max_shield_hp_ratio;
    action_ctx.enemy_atk = candidate.atk;
    action_ctx.enemy_def = candidate.def;
    action_ctx.enemy_atk_stage = candidate.atk_stage;
    action_ctx.enemy_def_stage = candidate.def_stage;
    action_ctx.enemy_spd_stage = candidate.spd_stage;
    action_ctx.enemy_acc_stage = candidate.acc_stage;
    action_ctx.enemy_has_aura = candidate.has_aura;
    action_ctx.enemy_has_cleansable_debuff = candidate.has_cleansable_debuff;

    choose_enemy_skill(
        &candidate.skill_ids,
        candidate.skill_count,
        current_ap,
        dbs,
        &action_ctx,
        weights,
    )
    .map(|skill| skill.score)
    .unwrap_or(0.0)
}

fn threat_target_context(
    target: &EnemySwitchCandidate,
    threat: &PlayerThreatContext,
) -> EnemyAiContext {
    EnemyAiContext {
        enemy_hp: 0,
        enemy_max_hp: 0,
        enemy_shield: 0,
        max_shield_hp_ratio: target.max_shield_hp_ratio,
        enemy_atk: threat.atk,
        enemy_def: 0,
        enemy_atk_stage: 0,
        enemy_def_stage: 0,
        enemy_spd_stage: 0,
        enemy_acc_stage: 0,
        enemy_element: ElementType::Fire,
        enemy_attached_auras: [None, None],
        enemy_status_ids: Vec::new(),
        enemy_has_aura: false,
        enemy_has_cleansable_debuff: false,
        player_atk_stage: target.atk_stage,
        player_def_stage: target.def_stage,
        player_spd_stage: target.spd_stage,
        player_acc_stage: target.acc_stage,
        player_def: target.def,
        player_hp: target.hp,
        player_shield: target.shield,
        target_element: target.element,
        target_attached_auras: target.attached_auras,
        target_status_ids: target.status_ids.clone(),
    }
}

fn player_threat_score(
    target: &EnemySwitchCandidate,
    threat: Option<&PlayerThreatContext>,
    dbs: &BattleDbs,
) -> f32 {
    let Some(threat) = threat else {
        return 0.0;
    };
    let projected_ap = threat.projected_ap.max(threat.current_ap);
    if target.hp <= 0 || projected_ap <= 0 {
        return 0.0;
    }

    let target_ctx = threat_target_context(target, threat);
    let mut attack_value = best_attack_value(
        &threat.skill_ids,
        threat.skill_count,
        threat.atk + threat.attack_bonus,
        projected_ap,
        dbs,
        &target_ctx,
    );
    if attack_value > 0.0 {
        attack_value += threat.fixed_damage_bonus.max(0) as f32;
    }
    let hp_ratio = if target.max_hp <= 0 {
        0.0
    } else {
        target.hp.max(0) as f32 / target.max_hp as f32
    };
    let lethal_pressure = if attack_value >= target.hp.max(0) as f32 {
        20.0 + (1.0 - hp_ratio) * 12.0
    } else {
        0.0
    };

    attack_value + lethal_pressure
}

fn active_player_threat_score(
    ctx: &EnemyAiContext,
    threat: &PlayerThreatContext,
    dbs: &BattleDbs,
) -> f32 {
    let target = EnemySwitchCandidate {
        index: 0,
        hp: ctx.enemy_hp,
        max_hp: ctx.enemy_max_hp,
        shield: ctx.enemy_shield,
        max_shield_hp_ratio: ctx.max_shield_hp_ratio,
        atk: ctx.enemy_atk,
        def: ctx.enemy_def,
        atk_stage: ctx.enemy_atk_stage,
        def_stage: ctx.enemy_def_stage,
        spd_stage: ctx.enemy_spd_stage,
        acc_stage: ctx.enemy_acc_stage,
        element: ctx.enemy_element,
        attached_auras: ctx.enemy_attached_auras,
        skill_ids: [SkillId::FirePunch; 4],
        skill_count: 0,
        status_ids: ctx.enemy_status_ids.clone(),
        has_aura: ctx.enemy_has_aura,
        has_cleansable_debuff: ctx.enemy_has_cleansable_debuff,
    };
    player_threat_score(&target, Some(threat), dbs)
}

fn player_response_value(threat: &PlayerThreatContext, dbs: &BattleDbs) -> f32 {
    let skill_response = threat
        .skill_ids
        .iter()
        .copied()
        .take(threat.skill_count)
        .filter_map(|skill_id| dbs.skills.get(&skill_id))
        .filter(|skill| threat.projected_ap >= skill.cost_ap)
        .map(|skill| match primary_effect(&skill.effect) {
            Some(SkillEffect::Heal { amount }) => *amount as f32,
            Some(SkillEffect::Shield { amount }) => *amount as f32 * 0.8,
            Some(SkillEffect::Cleanse { .. }) => 8.0,
            Some(SkillEffect::Sequence { effects }) => effects
                .iter()
                .map(|effect| match effect {
                    SkillEffect::Heal { amount } => *amount as f32,
                    SkillEffect::Shield { amount } => *amount as f32 * 0.8,
                    SkillEffect::Cleanse { .. } => 8.0,
                    _ => 0.0,
                })
                .sum(),
            _ => 0.0,
        })
        .fold(0.0, f32::max);

    skill_response + threat.defensive_value
}

fn apply_player_threat_adjustment(
    mut scored: ScoredEnemySkill,
    skill: &SkillDef,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
    weights: &EnemyAiWeights,
    player_threat: Option<&PlayerThreatContext>,
) -> ScoredEnemySkill {
    let Some(player_threat) = player_threat else {
        return scored;
    };

    let incoming_threat = active_player_threat_score(ctx, player_threat, dbs);
    let enemy_hp = ctx.enemy_hp.max(1) as f32;
    let lethal_threat = incoming_threat >= enemy_hp;
    let danger_pressure = (incoming_threat / enemy_hp).clamp(0.0, 2.0);
    let player_response = player_response_value(player_threat, dbs);
    let threat_weight = weights.player_threat;

    match scored.kind {
        EnemyAiSkillKind::Attack | EnemyAiSkillKind::Debuff => {
            let attack_value = estimate_attack_value(skill, ctx, dbs).max(0.0);
            if attack_value >= ctx.player_hp.max(0) as f32 {
                scored.score += (24.0 + player_response * 0.35 + danger_pressure * 8.0)
                    * weights.kill_bonus
                    * threat_weight;
            } else if player_response > 0.0 && ctx.player_hp <= 12 {
                scored.score += attack_value.min(18.0) * 0.35 * threat_weight;
            }
        }
        EnemyAiSkillKind::Heal => {
            if let Some(SkillEffect::Heal { amount }) = primary_effect(&skill.effect) {
                let missing_hp = (ctx.enemy_max_hp - ctx.enemy_hp).max(0) as f32;
                let effective_heal = (*amount as f32).min(missing_hp + incoming_threat * 0.35);
                scored.score += effective_heal * (0.6 + danger_pressure * 0.5) * threat_weight;
            }
            if lethal_threat {
                scored.score += 22.0 * threat_weight;
            }
        }
        EnemyAiSkillKind::Shield => {
            if let Some(SkillEffect::Shield { amount }) = primary_effect(&skill.effect) {
                let effective_amount = effective_shield_gain(
                    *amount,
                    ctx.enemy_max_hp,
                    ctx.enemy_shield,
                    ctx.max_shield_hp_ratio,
                ) as f32;
                let prevented = effective_amount.min(incoming_threat.max(0.0));
                scored.score += prevented * (0.9 + danger_pressure * 0.4) * threat_weight;
            }
            if lethal_threat {
                scored.score += 18.0 * threat_weight;
            }
        }
    }

    scored
}

fn status_pressure(status_ids: &[String], has_aura: bool, has_cleansable_debuff: bool) -> f32 {
    let damaging_statuses = status_ids
        .iter()
        .filter(|id| {
            matches!(
                id.as_str(),
                "burning"
                    | "seeded"
                    | "conduct_from_thunder"
                    | "conduct_from_water"
                    | "burning_from_fire"
                    | "burning_from_grass"
            )
        })
        .count() as f32;

    damaging_statuses * 8.0
        + if has_aura { 4.0 } else { 0.0 }
        + if has_cleansable_debuff { 6.0 } else { 0.0 }
}

fn score_switch_candidate(
    current: &EnemySwitchCandidate,
    candidate: &EnemySwitchCandidate,
    current_best_action: f32,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    weights: &EnemyAiWeights,
    player_threat: Option<&PlayerThreatContext>,
) -> f32 {
    if candidate.hp <= 0 {
        return f32::MIN;
    }

    let current_hp_ratio = if current.max_hp <= 0 {
        0.0
    } else {
        current.hp.max(0) as f32 / current.max_hp as f32
    };
    let candidate_hp_ratio = if candidate.max_hp <= 0 {
        0.0
    } else {
        candidate.hp.max(0) as f32 / candidate.max_hp as f32
    };
    let current_defense = dbs
        .elements
        .get_effectiveness(ctx.target_element, current.element);
    let candidate_defense = dbs
        .elements
        .get_effectiveness(ctx.target_element, candidate.element);
    let defensive_gain = (current_defense - candidate_defense) * 34.0;
    let health_gain = (candidate_hp_ratio - current_hp_ratio) * 30.0;
    let shield_gain = (candidate.shield - current.shield) as f32 * 0.35;
    let pressure_relief = status_pressure(
        &current.status_ids,
        current.has_aura,
        current.has_cleansable_debuff,
    ) - status_pressure(
        &candidate.status_ids,
        candidate.has_aura,
        candidate.has_cleansable_debuff,
    );
    let candidate_action = best_action_value(candidate, current_ap - 1, dbs, ctx, weights);
    let action_gain = (candidate_action - current_best_action) * 0.42;
    let current_threat = player_threat_score(current, player_threat, dbs);
    let candidate_threat = player_threat_score(candidate, player_threat, dbs);
    let threat_relief = (current_threat - candidate_threat) * 0.55 * weights.player_threat;
    let key_reserve_penalty =
        if candidate_threat > current_threat && candidate_action > current_best_action {
            ((candidate_threat - current_threat) * 0.25
                + (candidate_action - current_best_action) * 0.15)
                .min(22.0)
        } else {
            0.0
        };
    let danger_bonus = if current_hp_ratio <= 0.25 { 20.0 } else { 0.0 };

    defensive_gain
        + health_gain
        + shield_gain
        + pressure_relief
        + action_gain
        + threat_relief
        + danger_bonus
        - key_reserve_penalty
        - 12.0
}

pub(crate) fn choose_enemy_switch(
    current: &EnemySwitchCandidate,
    candidates: &[EnemySwitchCandidate],
    current_best_action: f32,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    already_switched: bool,
    switch_score_threshold: f32,
    weights: &EnemyAiWeights,
    player_threat: Option<&PlayerThreatContext>,
) -> Option<ScoredEnemySwitch> {
    if already_switched {
        return None;
    }
    if current_ap < 2 {
        return None;
    }

    candidates
        .iter()
        .filter(|candidate| candidate.index != current.index && candidate.hp > 0)
        .map(|candidate| ScoredEnemySwitch {
            index: candidate.index,
            score: score_switch_candidate(
                current,
                candidate,
                current_best_action,
                current_ap,
                dbs,
                ctx,
                weights,
                player_threat,
            ) * weights.switch_value,
        })
        .filter(|candidate| candidate.score >= switch_score_threshold)
        .max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.index.cmp(&a.index))
        })
}

fn enemy_skill_candidates(
    skill_ids: &[SkillId; 4],
    skill_count: usize,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    weights: &EnemyAiWeights,
) -> Vec<ScoredEnemySkill> {
    skill_ids
        .iter()
        .copied()
        .take(skill_count)
        .enumerate()
        .filter_map(|(slot, skill_id)| {
            let skill = dbs.skills.get(&skill_id)?;
            (current_ap >= skill.cost_ap).then(|| {
                apply_skill_weight(score_enemy_skill(slot, skill_id, skill, ctx, dbs), weights)
            })
        })
        .collect()
}

pub(crate) fn choose_enemy_skill(
    skill_ids: &[SkillId; 4],
    skill_count: usize,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    weights: &EnemyAiWeights,
) -> Option<ScoredEnemySkill> {
    choose_enemy_skill_with_threat(skill_ids, skill_count, current_ap, dbs, ctx, weights, None)
}

pub(crate) fn choose_enemy_skill_with_threat(
    skill_ids: &[SkillId; 4],
    skill_count: usize,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    weights: &EnemyAiWeights,
    player_threat: Option<&PlayerThreatContext>,
) -> Option<ScoredEnemySkill> {
    enemy_skill_candidates(skill_ids, skill_count, current_ap, dbs, ctx, weights)
        .into_iter()
        .map(|scored| {
            dbs.skills
                .get(&scored.skill_id)
                .map(|skill| {
                    apply_player_threat_adjustment(scored, skill, ctx, dbs, weights, player_threat)
                })
                .unwrap_or(scored)
        })
        .max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.slot.cmp(&a.slot))
        })
}

fn skill_effect_summary(skill: &SkillDef) -> &'static str {
    match primary_effect(&skill.effect) {
        Some(SkillEffect::Attack { .. }) => "攻击",
        Some(SkillEffect::Heal { .. }) => "治疗",
        Some(SkillEffect::Shield { .. }) => "护盾",
        Some(SkillEffect::Cleanse { .. }) => "净化",
        Some(SkillEffect::Dispel { .. }) => "驱散",
        Some(SkillEffect::ApplyStatus { .. }) => "施加状态",
        Some(SkillEffect::ModifyStages { .. }) => "属性变化",
        Some(SkillEffect::DealFixedDamage { .. }) => "固定伤害",
        Some(SkillEffect::DealStatDifferenceDamage { .. }) => "属性差伤害",
        Some(SkillEffect::Conditional { .. }) => "条件效果",
        Some(SkillEffect::Sequence { .. }) => "复合效果",
        None => "未知效果",
    }
}

fn enemy_skill_score_detail(
    skill: &SkillDef,
    scored: &ScoredEnemySkill,
    ctx: &EnemyAiContext,
    dbs: &BattleDbs,
) -> String {
    let hp_ratio = enemy_hp_ratio(ctx);
    let mut detail = match primary_effect(&skill.effect) {
        Some(SkillEffect::Attack { power, .. }) => {
            let attack_value = estimate_attack_value(skill, ctx, dbs);
            let element_text = skill
                .element
                .map(|element| format!("{:?}", element))
                .unwrap_or_else(|| "无".to_string());
            format!(
                "构成=攻击；威力={}；攻击估值={:.2}；技能元素={}；目标HP={}；目标护盾={}；敌方HP率={:.0}%",
                power,
                attack_value,
                element_text,
                ctx.player_hp,
                ctx.player_shield,
                hp_ratio * 100.0
            )
        }
        Some(SkillEffect::Heal { amount }) => {
            let missing_hp = (ctx.enemy_max_hp - ctx.enemy_hp).max(0);
            format!(
                "构成=治疗；基础治疗={}；缺失HP={}；敌方HP率={:.0}%",
                amount,
                missing_hp,
                hp_ratio * 100.0
            )
        }
        Some(SkillEffect::Shield { amount }) => format!(
            "构成=护盾；基础护盾={}；当前护盾={}；敌方HP率={:.0}%",
            amount,
            ctx.enemy_shield,
            hp_ratio * 100.0
        ),
        Some(SkillEffect::Cleanse {
            prefer_aura,
            fallback_to_debuff,
            amount,
        }) => format!(
            "构成=净化；可净化附着={}；可净化减益={}；偏好附着={}；回退减益={}；次数={}",
            ctx.enemy_has_aura,
            ctx.enemy_has_cleansable_debuff,
            prefer_aura,
            fallback_to_debuff,
            amount
        ),
        Some(SkillEffect::Dispel { status_ids, .. }) => format!(
            "构成=驱散；目标状态=[{}]；可驱散价值={:.2}",
            ctx.target_status_ids.join(" / "),
            dispel_value(status_ids, ctx)
        ),
        Some(SkillEffect::Sequence { effects }) => {
            let conditional_bonus = effects
                .iter()
                .map(|effect| conditional_bonus_score(effect, ctx))
                .sum::<f32>();
            let followup_bonus = effects
                .iter()
                .map(|effect| match effect {
                    SkillEffect::Dispel { status_ids, .. } => dispel_value(status_ids, ctx),
                    _ => 0.0,
                })
                .sum::<f32>();
            format!(
                "构成=复合；段数={}；条件加分={:.2}；后续驱散价值={:.2}；主类型={}",
                effects.len(),
                conditional_bonus,
                followup_bonus,
                skill_effect_summary(skill)
            )
        }
        _ => format!(
            "构成={}；目标状态数={}；敌方HP率={:.0}%；类别={:?}",
            skill_effect_summary(skill),
            ctx.target_status_ids.len(),
            hp_ratio * 100.0,
            skill.category
        ),
    };
    detail.push_str(&format!(
        "；类型={:?}；总分={:.2}",
        scored.kind, scored.score
    ));
    detail
}

pub(crate) fn enemy_skill_candidate_report(
    skill_ids: &[SkillId; 4],
    skill_count: usize,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    weights: &EnemyAiWeights,
) -> String {
    let entries = skill_ids
        .iter()
        .copied()
        .take(skill_count)
        .enumerate()
        .map(|(slot, skill_id)| {
            let Some(skill) = dbs.skills.get(&skill_id) else {
                return format!("槽位{}：未知技能 {:?}", slot, skill_id);
            };
            if current_ap < skill.cost_ap {
                return format!(
                    "槽位{}：{}；AP不足 {}/{}；类型={}",
                    slot,
                    skill.name,
                    current_ap,
                    skill.cost_ap,
                    skill_effect_summary(skill)
                );
            }
            let scored =
                apply_skill_weight(score_enemy_skill(slot, skill_id, skill, ctx, dbs), weights);
            format!(
                "槽位{}：{}；cost={}；{}",
                slot,
                skill.name,
                skill.cost_ap,
                enemy_skill_score_detail(skill, &scored, ctx, dbs)
            )
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        "无技能候选".to_string()
    } else {
        entries.join(" | ")
    }
}

pub(crate) fn enemy_switch_candidate_report(
    current: &EnemySwitchCandidate,
    candidates: &[EnemySwitchCandidate],
    current_best_action: f32,
    current_ap: i32,
    dbs: &BattleDbs,
    ctx: &EnemyAiContext,
    already_switched: bool,
    switch_score_threshold: f32,
    weights: &EnemyAiWeights,
    player_threat: Option<&PlayerThreatContext>,
) -> String {
    candidates
        .iter()
        .map(|candidate| {
            if candidate.index == current.index {
                return format!("#{} 当前前场", candidate.index + 1);
            }
            if candidate.hp <= 0 {
                return format!("#{} 已倒下", candidate.index + 1);
            }
            if already_switched {
                return format!("#{} 已本回合换人，跳过", candidate.index + 1);
            }
            if current_ap < 2 {
                return format!(
                    "#{} AP不足以换人后行动：{}",
                    candidate.index + 1,
                    current_ap
                );
            }
            let candidate_action = best_action_value(candidate, current_ap - 1, dbs, ctx, weights);
            let current_threat = player_threat_score(current, player_threat, dbs);
            let candidate_threat = player_threat_score(candidate, player_threat, dbs);
            let score = score_switch_candidate(
                current,
                candidate,
                current_best_action,
                current_ap,
                dbs,
                ctx,
                weights,
                player_threat,
            ) * weights.switch_value;
            format!(
                "#{} HP {}/{} 护盾 {} ATK {} DEF {} 元素 {:?}；出场行动={:.2}；玩家威胁 {:.2}->{:.2}；换人评分={:.2}；{}",
                candidate.index + 1,
                candidate.hp,
                candidate.max_hp,
                candidate.shield,
                candidate.atk,
                candidate.def,
                candidate.element,
                candidate_action,
                current_threat,
                candidate_threat,
                score,
                if score >= switch_score_threshold {
                    "可选"
                } else {
                    "低于阈值"
                }
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{
        EffectTarget, ElementDb, ElementType, ReactionDb, ReactionDef, StatusCategory, StatusDb,
        StatusDef,
    };
    use std::collections::HashMap;

    fn test_ai_context(target_status_ids: Vec<String>) -> EnemyAiContext {
        EnemyAiContext {
            enemy_hp: 20,
            enemy_max_hp: 20,
            enemy_shield: 0,
            max_shield_hp_ratio: 0.5,
            enemy_atk: 5,
            enemy_def: 5,
            enemy_atk_stage: 0,
            enemy_def_stage: 0,
            enemy_spd_stage: 0,
            enemy_acc_stage: 0,
            enemy_element: ElementType::Fire,
            enemy_attached_auras: [None, None],
            enemy_status_ids: Vec::new(),
            enemy_has_aura: false,
            enemy_has_cleansable_debuff: false,
            player_atk_stage: 0,
            player_def_stage: 0,
            player_spd_stage: 0,
            player_acc_stage: 0,
            player_def: 5,
            player_hp: 20,
            player_shield: 0,
            target_element: ElementType::Dark,
            target_attached_auras: [None, None],
            target_status_ids,
        }
    }

    fn test_dbs(skill: SkillDef) -> BattleDbs {
        BattleDbs {
            skills: HashMap::from([(skill.id, skill)]),
            cards: HashMap::new(),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        }
    }

    fn test_switch_dbs() -> BattleDbs {
        let attack = SkillDef {
            id: SkillId::WaterBlade,
            name: "水刃".to_string(),
            category: SkillCategory::ElementAttack,
            cost_ap: 1,
            effect: SkillEffect::Attack {
                power: 12,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: Some(ElementType::Water),
            base_accuracy: None,
        };
        BattleDbs {
            skills: HashMap::from([(attack.id, attack)]),
            cards: HashMap::new(),
            elements: ElementDb::from_default_config(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        }
    }

    fn test_switch_candidate(
        index: usize,
        hp: i32,
        max_hp: i32,
        shield: i32,
        element: ElementType,
    ) -> EnemySwitchCandidate {
        EnemySwitchCandidate {
            index,
            hp,
            max_hp,
            shield,
            max_shield_hp_ratio: 0.5,
            atk: 8,
            def: 5,
            atk_stage: 0,
            def_stage: 0,
            spd_stage: 0,
            acc_stage: 0,
            element,
            attached_auras: [None, None],
            skill_ids: [SkillId::WaterBlade; 4],
            skill_count: 1,
            status_ids: Vec::new(),
            has_aura: false,
            has_cleansable_debuff: false,
        }
    }

    #[test]
    fn enemy_switch_prefers_healthy_resistant_candidate_when_current_is_low() {
        let dbs = test_switch_dbs();
        let mut ctx = test_ai_context(Vec::new());
        ctx.target_element = ElementType::Water;
        let current = test_switch_candidate(0, 5, 40, 0, ElementType::Fire);
        let candidate = test_switch_candidate(1, 34, 40, 0, ElementType::Grass);

        let chosen = choose_enemy_switch(
            &current,
            &[current.clone(), candidate],
            0.0,
            3,
            &dbs,
            &ctx,
            false,
            18.0,
            &EnemyAiWeights::default(),
            None,
        )
        .expect("low HP and bad matchup should make switching valuable");

        assert_eq!(chosen.index, 1);
        assert!(chosen.score >= 18.0);
    }

    #[test]
    fn enemy_switch_requires_ap_after_switch_and_only_once_per_turn() {
        let dbs = test_switch_dbs();
        let mut ctx = test_ai_context(Vec::new());
        ctx.target_element = ElementType::Water;
        let current = test_switch_candidate(0, 5, 40, 0, ElementType::Fire);
        let candidate = test_switch_candidate(1, 34, 40, 0, ElementType::Grass);
        let candidates = [current.clone(), candidate];

        assert!(
            choose_enemy_switch(
                &current,
                &candidates,
                0.0,
                1,
                &dbs,
                &ctx,
                false,
                18.0,
                &EnemyAiWeights::default(),
                None,
            )
            .is_none()
        );
        assert!(
            choose_enemy_switch(
                &current,
                &candidates,
                0.0,
                3,
                &dbs,
                &ctx,
                true,
                18.0,
                &EnemyAiWeights::default(),
                None,
            )
            .is_none()
        );
    }

    #[test]
    fn enemy_switch_ignores_defeated_candidates() {
        let dbs = test_switch_dbs();
        let mut ctx = test_ai_context(Vec::new());
        ctx.target_element = ElementType::Water;
        let current = test_switch_candidate(0, 5, 40, 0, ElementType::Fire);
        let defeated = test_switch_candidate(1, 0, 40, 0, ElementType::Grass);

        assert!(
            choose_enemy_switch(
                &current,
                &[current.clone(), defeated],
                0.0,
                3,
                &dbs,
                &ctx,
                false,
                18.0,
                &EnemyAiWeights::default(),
                None,
            )
            .is_none()
        );
    }

    #[test]
    fn enemy_switch_values_post_switch_action() {
        let dbs = test_switch_dbs();
        let ctx = test_ai_context(Vec::new());
        let current = test_switch_candidate(0, 8, 40, 0, ElementType::Dark);
        let mut safe_passive = test_switch_candidate(1, 34, 40, 0, ElementType::Dark);
        safe_passive.atk = 0;
        let mut active_striker = test_switch_candidate(2, 24, 40, 0, ElementType::Dark);
        active_striker.atk = 30;

        let chosen = choose_enemy_switch(
            &current,
            &[current.clone(), safe_passive, active_striker],
            0.0,
            3,
            &dbs,
            &ctx,
            false,
            18.0,
            &EnemyAiWeights::default(),
            None,
        )
        .expect("post-switch action value should make an active striker worth switching to");

        assert_eq!(chosen.index, 2);
    }

    #[test]
    fn enemy_switch_avoids_exposing_key_candidate_to_player_threat() {
        let dbs = test_switch_dbs();
        let ctx = test_ai_context(Vec::new());
        let mut current = test_switch_candidate(0, 30, 40, 0, ElementType::Dark);
        current.def = 16;
        let mut vulnerable_striker = test_switch_candidate(1, 30, 40, 0, ElementType::Dark);
        vulnerable_striker.atk = 30;
        vulnerable_striker.def = 0;
        vulnerable_striker.attached_auras = [Some(ElementType::Fire), None];
        let candidates = [current.clone(), vulnerable_striker];

        let without_threat = choose_enemy_switch(
            &current,
            &candidates,
            0.0,
            3,
            &dbs,
            &ctx,
            false,
            5.0,
            &EnemyAiWeights::default(),
            None,
        );
        let player_threat = PlayerThreatContext {
            skill_ids: [SkillId::WaterBlade; 4],
            skill_count: 1,
            atk: 10,
            current_ap: 3,
            projected_ap: 3,
            attack_bonus: 0,
            fixed_damage_bonus: 0,
            defensive_value: 0.0,
        };
        let with_threat = choose_enemy_switch(
            &current,
            &candidates,
            0.0,
            3,
            &dbs,
            &ctx,
            false,
            5.0,
            &EnemyAiWeights::default(),
            Some(&player_threat),
        );

        assert_eq!(without_threat.map(|chosen| chosen.index), Some(1));
        assert!(with_threat.is_none());
    }

    #[test]
    fn player_hand_projection_adds_visible_ap_and_attack_pressure() {
        let ap_card = CardDef {
            id: CardId::GainAp,
            name: "整备".to_string(),
            cost_ap: 0,
            effect: CardEffect::GainAp { amount: 2 },
        };
        let attack_card = CardDef {
            id: CardId::NextAttackBoost,
            name: "猛攻".to_string(),
            cost_ap: 0,
            effect: CardEffect::NextAttackBoost { amount: 6 },
        };
        let dbs = BattleDbs {
            skills: HashMap::new(),
            cards: HashMap::from([(ap_card.id, ap_card), (attack_card.id, attack_card)]),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let hand = [CardId::GainAp, CardId::NextAttackBoost];

        let public =
            build_player_threat_context([SkillId::FirePunch; 4], 0, 5, 1, &hand, false, &dbs);
        let full = build_player_threat_context([SkillId::FirePunch; 4], 0, 5, 1, &hand, true, &dbs);

        assert_eq!(public.projected_ap, 1);
        assert_eq!(public.attack_bonus, 0);
        assert_eq!(full.projected_ap, 3);
        assert_eq!(full.attack_bonus, 6);
    }

    #[test]
    fn player_threat_can_push_high_difficulty_ai_to_shield() {
        let attack = SkillDef {
            id: SkillId::FirePunch,
            name: "火拳".to_string(),
            category: SkillCategory::NormalAttack,
            cost_ap: 1,
            effect: SkillEffect::Attack {
                power: 12,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: None,
            base_accuracy: None,
        };
        let shield = SkillDef {
            id: SkillId::WaterScreen,
            name: "水幕".to_string(),
            category: SkillCategory::SelfUtility,
            cost_ap: 1,
            effect: SkillEffect::Shield { amount: 14 },
            element: None,
            base_accuracy: None,
        };
        let player_attack = SkillDef {
            id: SkillId::WaterBlade,
            name: "水刃".to_string(),
            category: SkillCategory::ElementAttack,
            cost_ap: 1,
            effect: SkillEffect::Attack {
                power: 22,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: Some(ElementType::Water),
            base_accuracy: None,
        };
        let dbs = BattleDbs {
            skills: HashMap::from([
                (attack.id, attack),
                (shield.id, shield),
                (player_attack.id, player_attack),
            ]),
            cards: HashMap::new(),
            elements: ElementDb::from_default_config(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let mut ctx = test_ai_context(Vec::new());
        ctx.enemy_hp = 18;
        ctx.enemy_max_hp = 40;
        ctx.enemy_element = ElementType::Fire;
        ctx.enemy_def = 0;
        let skills = [
            SkillId::FirePunch,
            SkillId::WaterScreen,
            SkillId::FirePunch,
            SkillId::FirePunch,
        ];
        let player_threat =
            build_player_threat_context([SkillId::WaterBlade; 4], 1, 12, 1, &[], false, &dbs);

        let default_choice =
            choose_enemy_skill(&skills, 2, 2, &dbs, &ctx, &EnemyAiWeights::default())
                .expect("AI should choose a skill without threat modeling");
        let threatened_choice = choose_enemy_skill_with_threat(
            &skills,
            2,
            2,
            &dbs,
            &ctx,
            &EnemyAiWeights::default(),
            Some(&player_threat),
        )
        .expect("high-difficulty threat model should choose a defensive skill");

        assert_eq!(default_choice.skill_id, SkillId::FirePunch);
        assert_eq!(threatened_choice.skill_id, SkillId::WaterScreen);
    }

    #[test]
    fn planner_prefers_card_into_skill_sequence() {
        let attack = SkillDef {
            id: SkillId::FirePunch,
            name: "火拳".to_string(),
            category: SkillCategory::NormalAttack,
            cost_ap: 1,
            effect: SkillEffect::Attack {
                power: 12,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: None,
            base_accuracy: None,
        };
        let boost = CardDef {
            id: CardId::NextAttackBoost,
            name: "猛攻".to_string(),
            cost_ap: 0,
            effect: CardEffect::NextAttackBoost { amount: 6 },
        };
        let dbs = BattleDbs {
            skills: HashMap::from([(attack.id, attack)]),
            cards: HashMap::from([(boost.id, boost)]),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let ctx = test_ai_context(Vec::new());
        let hand = [CardId::NextAttackBoost];
        let skills = [SkillId::FirePunch; 4];

        let plan = choose_enemy_plan(
            &hand,
            2,
            &skills,
            1,
            &dbs,
            &ctx,
            None,
            &EnemyAiWeights::default(),
            None,
            2,
            4,
        )
        .expect("planner should produce an action");

        assert!(matches!(
            plan.action,
            EnemyPlannedAction::UseCardForSkill { card_index: 0, .. }
        ));
    }

    #[test]
    fn planner_can_choose_discard_to_unlock_skill() {
        let attack = SkillDef {
            id: SkillId::FirePunch,
            name: "火拳".to_string(),
            category: SkillCategory::NormalAttack,
            cost_ap: 2,
            effect: SkillEffect::Attack {
                power: 12,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: None,
            base_accuracy: None,
        };
        let weak_card = CardDef {
            id: CardId::Pursuit,
            name: "追击补牌".to_string(),
            cost_ap: 0,
            effect: CardEffect::DrawIfKnockedOutThisTurn { amount: 1 },
        };
        let dbs = BattleDbs {
            skills: HashMap::from([(attack.id, attack)]),
            cards: HashMap::from([(weak_card.id, weak_card)]),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let ctx = test_ai_context(Vec::new());
        let hand = [CardId::Pursuit];
        let skills = [SkillId::FirePunch; 4];

        let plan = choose_enemy_plan(
            &hand,
            1,
            &skills,
            1,
            &dbs,
            &ctx,
            None,
            &EnemyAiWeights::default(),
            None,
            2,
            4,
        )
        .expect("planner should choose a discard sequence");

        assert!(matches!(plan.action, EnemyPlannedAction::Discard(_)));
    }

    #[test]
    fn planner_can_choose_switch_into_better_action() {
        let dbs = test_switch_dbs();
        let ctx = test_ai_context(Vec::new());
        let current = test_switch_candidate(0, 8, 40, 0, ElementType::Dark);
        let mut striker = test_switch_candidate(1, 34, 40, 0, ElementType::Dark);
        striker.atk = 32;
        let candidates = [current.clone(), striker];
        let skills = [SkillId::FirePunch; 4];

        let plan = choose_enemy_plan(
            &[],
            3,
            &skills,
            0,
            &dbs,
            &ctx,
            Some((&current, &candidates, 0.0, false, 18.0, None)),
            &EnemyAiWeights::default(),
            None,
            2,
            4,
        )
        .expect("planner should choose switch when it is the only valuable sequence");

        assert!(matches!(plan.action, EnemyPlannedAction::Switch(_)));
    }

    #[test]
    fn skill_weights_can_change_chosen_skill() {
        let attack = SkillDef {
            id: SkillId::FirePunch,
            name: "火拳".to_string(),
            category: SkillCategory::NormalAttack,
            cost_ap: 1,
            effect: SkillEffect::Attack {
                power: 12,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: None,
            base_accuracy: None,
        };
        let shield = SkillDef {
            id: SkillId::WaterScreen,
            name: "水幕".to_string(),
            category: SkillCategory::SelfUtility,
            cost_ap: 1,
            effect: SkillEffect::Shield { amount: 5 },
            element: None,
            base_accuracy: None,
        };
        let dbs = BattleDbs {
            skills: HashMap::from([(attack.id, attack), (shield.id, shield)]),
            cards: HashMap::new(),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let ctx = test_ai_context(Vec::new());
        let skills = [
            SkillId::FirePunch,
            SkillId::WaterScreen,
            SkillId::FirePunch,
            SkillId::FirePunch,
        ];

        let default_choice =
            choose_enemy_skill(&skills, 2, 2, &dbs, &ctx, &EnemyAiWeights::default())
                .expect("default weights should choose an available skill");
        assert_eq!(default_choice.skill_id, SkillId::FirePunch);

        let shield_weights = EnemyAiWeights {
            shield_value: 3.0,
            ..EnemyAiWeights::default()
        };
        let shield_choice = choose_enemy_skill(&skills, 2, 2, &dbs, &ctx, &shield_weights)
            .expect("shield-weighted AI should choose an available skill");
        assert_eq!(shield_choice.skill_id, SkillId::WaterScreen);
    }

    #[test]
    fn reaction_bonus_increases_elemental_attack_score() {
        let skill = SkillDef {
            id: SkillId::FirePunch,
            name: "火拳".to_string(),
            category: SkillCategory::ElementAttack,
            cost_ap: 1,
            effect: SkillEffect::Attack {
                power: 10,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: Some(ElementType::Fire),
            base_accuracy: None,
        };
        let mut dbs = test_dbs(skill.clone());
        dbs.reactions = ReactionDb {
            reactions: vec![ReactionDef {
                id: "steam_burst".to_string(),
                name: "蒸汽爆发".to_string(),
                required_elements: vec![ElementType::Fire, ElementType::Water],
                required_statuses: Vec::new(),
                trigger_element: ElementType::Fire,
                fixed_damage: 10,
                heal_attacker: 0,
                apply_statuses: Vec::new(),
                clear_statuses: Vec::new(),
                clear_elements: Vec::new(),
                preserve_current_auras: false,
                aura_results: Vec::new(),
            }],
        };
        let no_reaction = test_ai_context(Vec::new());
        let mut with_reaction = test_ai_context(Vec::new());
        with_reaction.target_attached_auras = [Some(ElementType::Water), None];

        let base_score = score_enemy_skill(0, SkillId::FirePunch, &skill, &no_reaction, &dbs).score;
        let reaction_score =
            score_enemy_skill(0, SkillId::FirePunch, &skill, &with_reaction, &dbs).score;

        assert!(reaction_score > base_score + 6.0);
    }

    #[test]
    fn aura_setup_value_rewards_element_setup_for_future_reaction() {
        let skill = SkillDef {
            id: SkillId::FlameStorm,
            name: "烈焰风暴".to_string(),
            category: SkillCategory::ElementAttack,
            cost_ap: 3,
            effect: SkillEffect::Attack {
                power: 10,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: Some(ElementType::Fire),
            base_accuracy: None,
        };
        let no_reaction_dbs = test_dbs(skill.clone());
        let mut setup_dbs = test_dbs(skill.clone());
        setup_dbs.reactions = ReactionDb {
            reactions: vec![ReactionDef {
                id: "future_vaporize".to_string(),
                name: "后续蒸发".to_string(),
                required_elements: vec![ElementType::Fire, ElementType::Water],
                required_statuses: Vec::new(),
                trigger_element: ElementType::Water,
                fixed_damage: 16,
                heal_attacker: 0,
                apply_statuses: Vec::new(),
                clear_statuses: Vec::new(),
                clear_elements: Vec::new(),
                preserve_current_auras: false,
                aura_results: Vec::new(),
            }],
        };
        let ctx = test_ai_context(Vec::new());

        let no_setup_score =
            score_enemy_skill(0, SkillId::FlameStorm, &skill, &ctx, &no_reaction_dbs).score;
        let setup_score = score_enemy_skill(0, SkillId::FlameStorm, &skill, &ctx, &setup_dbs).score;

        assert!(setup_score > no_setup_score);
    }

    #[test]
    fn status_future_value_increases_debuff_score_and_penalizes_duplicates() {
        let skill = SkillDef {
            id: SkillId::CurseWhisper,
            name: "诅咒低语".to_string(),
            category: SkillCategory::EnemyDebuff,
            cost_ap: 2,
            effect: SkillEffect::ApplyStatus {
                status_id: "burning".to_string(),
            },
            element: None,
            base_accuracy: None,
        };
        let mut dbs = test_dbs(skill.clone());
        dbs.statuses = StatusDb {
            statuses: HashMap::from([(
                "burning".to_string(),
                StatusDef {
                    id: "burning".to_string(),
                    name: "燃烧".to_string(),
                    category: StatusCategory::Debuff,
                    duration_turns: 3,
                    tick_timing: None,
                    stage_modifiers: Vec::new(),
                    fixed_damage_on_tick: 4,
                    heal_on_tick: 0,
                    heal_taken_multiplier: None,
                    evade_charges: 0,
                },
            )]),
        };
        let fresh_target = test_ai_context(Vec::new());
        let duplicate_target = test_ai_context(vec!["burning".to_string()]);

        let fresh_score =
            score_enemy_skill(0, SkillId::CurseWhisper, &skill, &fresh_target, &dbs).score;
        let duplicate_score =
            score_enemy_skill(0, SkillId::CurseWhisper, &skill, &duplicate_target, &dbs).score;

        assert!(fresh_score > duplicate_score);
        assert!(fresh_score > 40.0);
        assert!(duplicate_score < 10.0);
    }

    #[test]
    fn duplicate_status_skill_does_not_crowd_out_followup_attacks() {
        let shadow_blade = SkillDef {
            id: SkillId::ShadowBlade,
            name: "暗影刃".to_string(),
            category: SkillCategory::NormalAttack,
            cost_ap: 2,
            effect: SkillEffect::Attack {
                power: 15,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: None,
            base_accuracy: None,
        };
        let curse = SkillDef {
            id: SkillId::CurseWhisper,
            name: "诅咒低语".to_string(),
            category: SkillCategory::EnemyDebuff,
            cost_ap: 2,
            effect: SkillEffect::ApplyStatus {
                status_id: "cursed".to_string(),
            },
            element: None,
            base_accuracy: None,
        };
        let blood_touch = SkillDef {
            id: SkillId::BloodTouch,
            name: "噬血之触".to_string(),
            category: SkillCategory::SpecialAttack,
            cost_ap: 3,
            effect: SkillEffect::Sequence {
                effects: vec![
                    SkillEffect::Attack {
                        power: 15,
                        lifesteal_ratio: Some(0.25),
                        ignore_shield: false,
                    },
                    SkillEffect::Conditional {
                        branches: vec![crate::data::ConditionalSkillEffect {
                            condition: crate::data::SkillCondition::TargetHadStatus {
                                status_id: "cursed".to_string(),
                            },
                            effect: Box::new(SkillEffect::DealFixedDamage {
                                amount: 6,
                                ignore_shield: false,
                                target: EffectTarget::Opponent,
                            }),
                        }],
                    },
                ],
            },
            element: Some(ElementType::Dark),
            base_accuracy: None,
        };
        let dbs = BattleDbs {
            skills: HashMap::from([
                (shadow_blade.id, shadow_blade),
                (curse.id, curse),
                (blood_touch.id, blood_touch),
            ]),
            cards: HashMap::new(),
            elements: ElementDb::default(),
            statuses: StatusDb {
                statuses: HashMap::from([(
                    "cursed".to_string(),
                    StatusDef {
                        id: "cursed".to_string(),
                        name: "诅咒".to_string(),
                        category: StatusCategory::Debuff,
                        duration_turns: 2,
                        tick_timing: None,
                        stage_modifiers: vec![
                            crate::data::AttributeStageModifier {
                                attribute: crate::data::AttributeType::Atk,
                                amount: -2,
                            },
                            crate::data::AttributeStageModifier {
                                attribute: crate::data::AttributeType::Acc,
                                amount: -2,
                            },
                        ],
                        fixed_damage_on_tick: 0,
                        heal_on_tick: 0,
                        heal_taken_multiplier: None,
                        evade_charges: 0,
                    },
                )]),
            },
            reactions: ReactionDb::default(),
        };
        let mut ctx = test_ai_context(vec!["cursed".to_string()]);
        ctx.enemy_atk = 21;
        ctx.player_def = 18;
        ctx.player_hp = 60;
        let chosen = choose_enemy_skill(
            &[
                SkillId::ShadowBlade,
                SkillId::CurseWhisper,
                SkillId::BloodTouch,
                SkillId::FirePunch,
            ],
            3,
            4,
            &dbs,
            &ctx,
            &EnemyAiWeights::default(),
        )
        .expect("dark elf should have an attack available");

        assert_ne!(chosen.skill_id, SkillId::CurseWhisper);
    }

    #[test]
    fn repeated_self_stage_buff_loses_priority_after_existing_stages() {
        let swift = SkillDef {
            id: SkillId::SwiftThunder,
            name: "疾风迅雷".to_string(),
            category: SkillCategory::SelfUtility,
            cost_ap: 2,
            effect: SkillEffect::ModifyStages {
                modifiers: vec![
                    AttributeStageModifier {
                        attribute: AttributeType::Spd,
                        amount: 1,
                    },
                    AttributeStageModifier {
                        attribute: AttributeType::Acc,
                        amount: 1,
                    },
                ],
                duration_turns: 2,
                target: EffectTarget::Infer,
            },
            element: None,
            base_accuracy: None,
        };
        let attack = SkillDef {
            id: SkillId::ThunderStrike,
            name: "闪击".to_string(),
            category: SkillCategory::NormalAttack,
            cost_ap: 2,
            effect: SkillEffect::Attack {
                power: 15,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: None,
            base_accuracy: None,
        };
        let dbs = BattleDbs {
            skills: HashMap::from([(swift.id, swift.clone()), (attack.id, attack.clone())]),
            cards: HashMap::new(),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let mut fresh_ctx = test_ai_context(Vec::new());
        fresh_ctx.player_shield = 33;
        let mut stacked_ctx = fresh_ctx.clone();
        stacked_ctx.enemy_spd_stage = 1;
        stacked_ctx.enemy_acc_stage = 1;

        let fresh_swift =
            score_enemy_skill(0, SkillId::SwiftThunder, &swift, &fresh_ctx, &dbs).score;
        let repeated_swift =
            score_enemy_skill(0, SkillId::SwiftThunder, &swift, &stacked_ctx, &dbs).score;
        let attack_score =
            score_enemy_skill(1, SkillId::ThunderStrike, &attack, &stacked_ctx, &dbs).score;

        assert!(fresh_swift > repeated_swift);
        assert!(repeated_swift < attack_score);
    }

    #[test]
    fn repeated_self_evade_status_is_not_scored_as_fresh_buff() {
        let gale_evasion = SkillDef {
            id: SkillId::GaleEvasion,
            name: "疾风闪避".to_string(),
            category: SkillCategory::SelfUtility,
            cost_ap: 3,
            effect: SkillEffect::ApplyStatus {
                status_id: "wind_evade".to_string(),
            },
            element: None,
            base_accuracy: None,
        };
        let dbs = BattleDbs {
            skills: HashMap::from([(gale_evasion.id, gale_evasion.clone())]),
            cards: HashMap::new(),
            elements: ElementDb::default(),
            statuses: StatusDb {
                statuses: HashMap::from([(
                    "wind_evade".to_string(),
                    StatusDef {
                        id: "wind_evade".to_string(),
                        name: "闪避".to_string(),
                        category: StatusCategory::Buff,
                        duration_turns: 1,
                        tick_timing: None,
                        stage_modifiers: Vec::new(),
                        fixed_damage_on_tick: 0,
                        heal_on_tick: 0,
                        heal_taken_multiplier: None,
                        evade_charges: 1,
                    },
                )]),
            },
            reactions: ReactionDb::default(),
        };
        let fresh_ctx = test_ai_context(Vec::new());
        let mut repeated_ctx = fresh_ctx.clone();
        repeated_ctx.enemy_status_ids = vec!["wind_evade".to_string()];

        let fresh_score =
            score_enemy_skill(0, SkillId::GaleEvasion, &gale_evasion, &fresh_ctx, &dbs).score;
        let repeated_score =
            score_enemy_skill(0, SkillId::GaleEvasion, &gale_evasion, &repeated_ctx, &dbs).score;

        assert!(fresh_score > repeated_score);
        assert!(repeated_score < 10.0);
    }

    #[test]
    fn immediate_card_choice_prefers_playable_ap_card() {
        let ap_card = CardDef {
            id: CardId::GainAp,
            name: "整备".to_string(),
            cost_ap: 0,
            effect: CardEffect::GainAp { amount: 2 },
        };
        let weak_card = CardDef {
            id: CardId::Pursuit,
            name: "追击补牌".to_string(),
            cost_ap: 0,
            effect: CardEffect::DrawIfKnockedOutThisTurn { amount: 1 },
        };
        let dbs = BattleDbs {
            skills: HashMap::new(),
            cards: HashMap::from([(ap_card.id, ap_card), (weak_card.id, weak_card)]),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let hand = [CardId::Pursuit, CardId::GainAp];
        let ctx = test_ai_context(Vec::new());

        let chosen = choose_enemy_immediate_card(&hand, 0, &dbs, &ctx, &EnemyAiWeights::default())
            .expect("AI should find a playable immediate AP card");

        assert_eq!(chosen.index, 1);
        assert_eq!(hand[chosen.index], CardId::GainAp);
    }

    #[test]
    fn immediate_card_choice_can_use_shield_absorb_when_shielded() {
        let shield_absorb = CardDef {
            id: CardId::GuardCounter,
            name: "护盾反制".to_string(),
            cost_ap: 0,
            effect: CardEffect::ShieldAbsorbGainAp { amount: 2 },
        };
        let weak_card = CardDef {
            id: CardId::Pursuit,
            name: "追击补牌".to_string(),
            cost_ap: 0,
            effect: CardEffect::DrawIfKnockedOutThisTurn { amount: 1 },
        };
        let dbs = BattleDbs {
            skills: HashMap::new(),
            cards: HashMap::from([(shield_absorb.id, shield_absorb), (weak_card.id, weak_card)]),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let hand = [CardId::Pursuit, CardId::GuardCounter];
        let mut ctx = test_ai_context(Vec::new());
        ctx.enemy_shield = 6;

        let chosen = choose_enemy_immediate_card(&hand, 0, &dbs, &ctx, &EnemyAiWeights::default())
            .expect("shielded AI should consider shield absorb AP setup useful");

        assert_eq!(chosen.index, 1);
        assert_eq!(hand[chosen.index], CardId::GuardCounter);
    }

    #[test]
    fn skill_card_score_requires_reaction_window_for_reaction_damage() {
        let skill = SkillDef {
            id: SkillId::FirePunch,
            name: "火拳".to_string(),
            category: SkillCategory::ElementAttack,
            cost_ap: 1,
            effect: SkillEffect::Attack {
                power: 12,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: Some(ElementType::Fire),
            base_accuracy: None,
        };
        let card = CardDef {
            id: CardId::ReactionCatalyst,
            name: "反应催化".to_string(),
            cost_ap: 0,
            effect: CardEffect::NextReactionFixedDamage {
                amount: 8,
                ignore_shield: true,
            },
        };
        let dbs = test_dbs(skill.clone());
        let mut reaction_ctx = test_ai_context(Vec::new());
        reaction_ctx.target_attached_auras = [Some(ElementType::Water), None];
        let no_reaction_ctx = test_ai_context(Vec::new());

        assert!(
            score_card_for_skill(
                &card,
                EnemyAiSkillKind::Attack,
                &skill,
                &reaction_ctx,
                &dbs,
                1,
                &EnemyAiWeights::default(),
            )
            .is_some()
        );
        assert!(
            score_card_for_skill(
                &card,
                EnemyAiSkillKind::Attack,
                &skill,
                &no_reaction_ctx,
                &dbs,
                1,
                &EnemyAiWeights::default(),
            )
            .is_none()
        );
    }

    #[test]
    fn skill_card_score_handles_aura_attack_draw() {
        let skill = SkillDef {
            id: SkillId::WaterBlade,
            name: "水刃".to_string(),
            category: SkillCategory::ElementAttack,
            cost_ap: 1,
            effect: SkillEffect::Attack {
                power: 10,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: Some(ElementType::Water),
            base_accuracy: None,
        };
        let card = CardDef {
            id: CardId::ReadyToAct,
            name: "蓄势待发".to_string(),
            cost_ap: 0,
            effect: CardEffect::NextAuraAttackDraw { amount: 1 },
        };
        let dbs = test_dbs(skill.clone());
        let mut ctx = test_ai_context(Vec::new());
        ctx.target_attached_auras = [Some(ElementType::Fire), None];

        assert!(
            score_card_for_skill(
                &card,
                EnemyAiSkillKind::Attack,
                &skill,
                &ctx,
                &dbs,
                1,
                &EnemyAiWeights::default(),
            )
            .is_some()
        );
    }

    #[test]
    fn discard_choice_prefers_lowest_keep_value_card() {
        let weak_card = CardDef {
            id: CardId::Pursuit,
            name: "追击补牌".to_string(),
            cost_ap: 0,
            effect: CardEffect::DrawIfKnockedOutThisTurn { amount: 1 },
        };
        let ap_card = CardDef {
            id: CardId::GainAp,
            name: "整备".to_string(),
            cost_ap: 0,
            effect: CardEffect::GainAp { amount: 2 },
        };
        let dbs = BattleDbs {
            skills: HashMap::new(),
            cards: HashMap::from([(weak_card.id, weak_card), (ap_card.id, ap_card)]),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let hand = [CardId::GainAp, CardId::Pursuit];

        let chosen = choose_enemy_discard_card(&hand, &dbs, &EnemyAiWeights::default())
            .expect("discard chooser should score known cards");

        assert_eq!(chosen.index, 1);
        assert_eq!(hand[chosen.index], CardId::Pursuit);
    }

    #[test]
    fn discard_for_followup_preserves_newly_playable_card() {
        let shield_card = CardDef {
            id: CardId::EmergencyShield,
            name: "紧急护盾".to_string(),
            cost_ap: 2,
            effect: CardEffect::GainShield { amount: 8 },
        };
        let weak_card = CardDef {
            id: CardId::Pursuit,
            name: "追击补牌".to_string(),
            cost_ap: 0,
            effect: CardEffect::DrawIfKnockedOutThisTurn { amount: 1 },
        };
        let dbs = BattleDbs {
            skills: HashMap::new(),
            cards: HashMap::from([(shield_card.id, shield_card), (weak_card.id, weak_card)]),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let hand = [CardId::EmergencyShield, CardId::Pursuit];
        let ctx = test_ai_context(Vec::new());
        let skills = [SkillId::FirePunch; 4];

        let chosen = choose_enemy_discard_for_followup(
            &hand,
            1,
            &skills,
            0,
            &dbs,
            &ctx,
            None,
            &EnemyAiWeights::default(),
        )
        .expect("discard sequence chooser should score known cards");

        assert_eq!(chosen.index, 1);
        assert_eq!(hand[chosen.index], CardId::Pursuit);
        assert_eq!(chosen.followup, EnemyDiscardFollowUp::ImmediateCard);
        assert!(chosen.followup_score > 0.0);
    }

    #[test]
    fn discard_for_followup_records_unlocked_skill() {
        let attack = SkillDef {
            id: SkillId::FirePunch,
            name: "火拳".to_string(),
            category: SkillCategory::NormalAttack,
            cost_ap: 2,
            effect: SkillEffect::Attack {
                power: 12,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: None,
            base_accuracy: None,
        };
        let weak_card = CardDef {
            id: CardId::Pursuit,
            name: "追击补牌".to_string(),
            cost_ap: 0,
            effect: CardEffect::DrawIfKnockedOutThisTurn { amount: 1 },
        };
        let ap_card = CardDef {
            id: CardId::GainAp,
            name: "整备".to_string(),
            cost_ap: 0,
            effect: CardEffect::GainAp { amount: 2 },
        };
        let dbs = BattleDbs {
            skills: HashMap::from([(attack.id, attack)]),
            cards: HashMap::from([(weak_card.id, weak_card), (ap_card.id, ap_card)]),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let hand = [CardId::GainAp, CardId::Pursuit];
        let ctx = test_ai_context(Vec::new());
        let skills = [SkillId::FirePunch; 4];

        let chosen = choose_enemy_discard_for_followup(
            &hand,
            1,
            &skills,
            1,
            &dbs,
            &ctx,
            None,
            &EnemyAiWeights::default(),
        )
        .expect("discard sequence chooser should score an unlocked skill");

        assert_eq!(chosen.index, 1);
        assert_eq!(hand[chosen.index], CardId::Pursuit);
        assert_eq!(
            chosen.followup,
            EnemyDiscardFollowUp::ImmediateCardThenSkill
        );
        assert!(chosen.followup_score > 0.0);
    }

    #[test]
    fn discard_for_followup_prefers_ap_card_chain_into_skill() {
        let attack = SkillDef {
            id: SkillId::FirePunch,
            name: "火拳".to_string(),
            category: SkillCategory::NormalAttack,
            cost_ap: 2,
            effect: SkillEffect::Attack {
                power: 12,
                lifesteal_ratio: None,
                ignore_shield: false,
            },
            element: None,
            base_accuracy: None,
        };
        let weak_card = CardDef {
            id: CardId::Pursuit,
            name: "追击补牌".to_string(),
            cost_ap: 0,
            effect: CardEffect::DrawIfKnockedOutThisTurn { amount: 1 },
        };
        let ap_card = CardDef {
            id: CardId::GainAp,
            name: "整备".to_string(),
            cost_ap: 0,
            effect: CardEffect::GainAp { amount: 2 },
        };
        let dbs = BattleDbs {
            skills: HashMap::from([(attack.id, attack)]),
            cards: HashMap::from([(weak_card.id, weak_card), (ap_card.id, ap_card)]),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        };
        let hand = [CardId::GainAp, CardId::Pursuit];
        let ctx = test_ai_context(Vec::new());
        let skills = [SkillId::FirePunch; 4];

        let chosen = choose_enemy_discard_for_followup(
            &hand,
            0,
            &skills,
            1,
            &dbs,
            &ctx,
            None,
            &EnemyAiWeights::default(),
        )
        .expect("discard sequence chooser should score AP-card chain");

        assert_eq!(chosen.index, 1);
        assert_eq!(hand[chosen.index], CardId::Pursuit);
        assert_eq!(
            chosen.followup,
            EnemyDiscardFollowUp::ImmediateCardThenSkill
        );
        assert!(chosen.followup_score > 0.0);
    }

    #[test]
    fn discard_for_followup_records_unlocked_switch() {
        let weak_card = CardDef {
            id: CardId::Pursuit,
            name: "追击补牌".to_string(),
            cost_ap: 0,
            effect: CardEffect::DrawIfKnockedOutThisTurn { amount: 1 },
        };
        let mut dbs = test_switch_dbs();
        dbs.cards = HashMap::from([(weak_card.id, weak_card)]);
        let hand = [CardId::Pursuit];
        let mut ctx = test_ai_context(Vec::new());
        ctx.target_element = ElementType::Water;
        let current = test_switch_candidate(0, 5, 40, 0, ElementType::Fire);
        let candidate = test_switch_candidate(1, 34, 40, 0, ElementType::Grass);
        let candidates = [current.clone(), candidate];
        let skills = [SkillId::FirePunch; 4];

        let chosen = choose_enemy_discard_for_followup(
            &hand,
            1,
            &skills,
            0,
            &dbs,
            &ctx,
            Some((&current, &candidates, 0.0, false, 18.0, None)),
            &EnemyAiWeights::default(),
        )
        .expect("discard sequence chooser should score an unlocked switch");

        assert_eq!(chosen.index, 0);
        assert_eq!(chosen.followup, EnemyDiscardFollowUp::Switch);
        assert!(chosen.followup_score >= 18.0);
    }

    #[test]
    fn dispel_score_uses_target_stage_shift_prefix_matches() {
        let skill = SkillDef {
            id: SkillId::SacredJudgment,
            name: "圣辉裁决".to_string(),
            category: SkillCategory::SpecialAttack,
            cost_ap: 4,
            effect: SkillEffect::Sequence {
                effects: vec![
                    SkillEffect::Attack {
                        power: 1,
                        lifesteal_ratio: None,
                        ignore_shield: true,
                    },
                    SkillEffect::Dispel {
                        status_ids: vec!["stage_shift_buff".to_string()],
                        target: EffectTarget::Opponent,
                    },
                ],
            },
            element: Some(ElementType::Light),
            base_accuracy: None,
        };
        let dbs = test_dbs(skill.clone());
        let without_buff = test_ai_context(vec!["cursed".to_string()]);
        let with_buff = test_ai_context(vec![
            "stage_shift_buff_atk_1".to_string(),
            "stage_shift_buff_def_2".to_string(),
            "stage_shift_debuff_acc_1".to_string(),
        ]);

        let score_without_buff =
            score_enemy_skill(0, SkillId::SacredJudgment, &skill, &without_buff, &dbs).score;
        let score_with_buff =
            score_enemy_skill(0, SkillId::SacredJudgment, &skill, &with_buff, &dbs).score;

        assert_eq!(
            dispel_value(&["stage_shift_buff".to_string()], &with_buff),
            24.0
        );
        assert_eq!(
            dispel_value(&["stage_shift_buff".to_string()], &without_buff),
            0.0
        );
        assert!(score_with_buff > score_without_buff);
    }
}
