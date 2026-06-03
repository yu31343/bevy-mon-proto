use bevy::prelude::*;

use crate::{
    battle::{
        ActionPoints, BattleControlMode, BattleEvent, CardPiles, CardTurnMemory, Combatant,
        ElementAura, EnemyTeam, Hand, InBattle, PendingBoost, PendingBoosts, PendingHandDiscard,
        PlayerTeam, Shield, Side, Stats, StatusBoard, StatusInstance, upsert_status_instance,
    },
    data::{
        AttributeType, BattleDbs, BattleRules, CardDeck, CardDef, CardEffect, CardId, ElementType,
        StatusCategory,
    },
    game_state::BattlePhase,
};

pub(crate) type CardCombatQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Combatant,
        &'static mut Stats,
        &'static mut Shield,
        &'static mut StatusBoard,
        &'static mut ElementAura,
    ),
    With<InBattle>,
>;

fn ap_mut(side: Side, action_points: &mut ActionPoints) -> &mut i32 {
    match side {
        Side::Player => &mut action_points.player,
        Side::Enemy => &mut action_points.enemy,
    }
}

fn hand_mut(side: Side, hand: &mut Hand) -> &mut Vec<CardId> {
    match side {
        Side::Player => &mut hand.player,
        Side::Enemy => &mut hand.enemy,
    }
}

fn pending_mut(side: Side, pending_boosts: &mut PendingBoosts) -> &mut PendingBoost {
    match side {
        Side::Player => &mut pending_boosts.player,
        Side::Enemy => &mut pending_boosts.enemy,
    }
}

fn active_entity(
    side: Side,
    player_team: Option<&PlayerTeam>,
    enemy_team: Option<&EnemyTeam>,
) -> Option<Entity> {
    match side {
        Side::Player => player_team.and_then(|team| team.0.active_combatant()),
        Side::Enemy => enemy_team.and_then(|team| team.0.active_combatant()),
    }
}

fn team_alive_count(
    side: Side,
    player_team: Option<&PlayerTeam>,
    enemy_team: Option<&EnemyTeam>,
    query: &CardCombatQuery,
) -> usize {
    let combatants = match side {
        Side::Player => player_team.map(|team| team.0.combatants.as_slice()),
        Side::Enemy => enemy_team.map(|team| team.0.combatants.as_slice()),
    };
    combatants
        .into_iter()
        .flatten()
        .filter(|entity| {
            query
                .get(**entity)
                .is_ok_and(|(_, stats, _, _, _)| stats.hp > 0)
        })
        .count()
}

pub(crate) fn gain_ap(
    side: Side,
    amount: i32,
    rules: &BattleRules,
    action_points: &mut ActionPoints,
) {
    let ap = ap_mut(side, action_points);
    *ap = (*ap + amount).clamp(0, rules.max_ap);
}

pub(crate) fn draw_cards(
    side: Side,
    amount: usize,
    hand: &mut Hand,
    piles: &mut CardPiles,
    deck: &CardDeck,
) -> usize {
    let mut drawn = 0;
    for _ in 0..amount {
        let Some(card_id) = piles.draw_one(side, &deck.0) else {
            break;
        };
        hand_mut(side, hand).push(card_id);
        drawn += 1;
    }
    drawn
}

pub(crate) fn clear_action_scoped_card_effects(side: Side, pending_boosts: &mut PendingBoosts) {
    pending_mut(side, pending_boosts).next_switch_draw = None;
}

pub(crate) fn discard_card_from_hand(
    side: Side,
    card_index: usize,
    hand: &mut Hand,
    _piles: &mut CardPiles,
    rules: &BattleRules,
    action_points: &mut ActionPoints,
    dbs: &BattleDbs,
    event_writer: &mut MessageWriter<BattleEvent>,
) -> Option<String> {
    let cards = hand_mut(side, hand);
    if card_index >= cards.len() {
        return None;
    }
    let card_id = cards.remove(card_index);
    gain_ap(side, rules.discard_ap_gain, rules, action_points);
    let card_name = dbs
        .cards
        .get(&card_id)
        .map(|card| card.name.clone())
        .unwrap_or_else(|| format!("{card_id:?}"));
    event_writer.write(BattleEvent::CardDiscarded {
        side,
        card_name: card_name.clone(),
    });
    Some(card_name)
}

#[allow(dead_code)]
pub(crate) fn auto_discard_excess_hand(
    side: Side,
    hand: &mut Hand,
    piles: &mut CardPiles,
    rules: &BattleRules,
    action_points: &mut ActionPoints,
    dbs: &BattleDbs,
    event_writer: &mut MessageWriter<BattleEvent>,
) {
    while hand_mut(side, hand).len() > rules.max_retained_hand {
        let index = hand_mut(side, hand).len() - 1;
        if discard_card_from_hand(
            side,
            index,
            hand,
            piles,
            rules,
            action_points,
            dbs,
            event_writer,
        )
        .is_none()
        {
            break;
        }
    }
}

pub(crate) fn enter_discard_phase_or_continue(
    side: Side,
    next_after_discard: BattlePhase,
    hand: &Hand,
    rules: &BattleRules,
    commands: &mut Commands,
    next_phase: &mut ResMut<NextState<BattlePhase>>,
) {
    let hand_len = match side {
        Side::Player => hand.player.len(),
        Side::Enemy => hand.enemy.len(),
    };
    if hand_len > rules.max_retained_hand {
        commands.insert_resource(PendingHandDiscard {
            side,
            next_phase: next_after_discard,
        });
        next_phase.set(BattlePhase::Discard);
    } else {
        commands.remove_resource::<PendingHandDiscard>();
        next_phase.set(next_after_discard);
    }
}

fn card_hotkeys() -> [(KeyCode, usize); 18] {
    [
        (KeyCode::KeyZ, 0),
        (KeyCode::KeyX, 1),
        (KeyCode::KeyC, 2),
        (KeyCode::KeyV, 3),
        (KeyCode::KeyB, 4),
        (KeyCode::KeyN, 5),
        (KeyCode::KeyA, 6),
        (KeyCode::KeyS, 7),
        (KeyCode::KeyD, 8),
        (KeyCode::KeyG, 9),
        (KeyCode::KeyH, 10),
        (KeyCode::KeyJ, 11),
        (KeyCode::KeyK, 12),
        (KeyCode::KeyL, 13),
        (KeyCode::KeyU, 14),
        (KeyCode::KeyI, 15),
        (KeyCode::KeyO, 16),
        (KeyCode::KeyP, 17),
    ]
}

fn hand_len(side: Side, hand: &Hand) -> usize {
    match side {
        Side::Player => hand.player.len(),
        Side::Enemy => hand.enemy.len(),
    }
}

fn locally_controls_discard_side(side: Side, battle_mode: BattleControlMode) -> bool {
    match battle_mode {
        BattleControlMode::PlayerVsAi => side == Side::Player,
        BattleControlMode::DebugPlayerControlsBoth | BattleControlMode::PlayerVsRemote => true,
    }
}

pub(crate) fn hand_discard_phase_system(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    pending: Option<Res<PendingHandDiscard>>,
    battle_mode: Res<BattleControlMode>,
    rules: Res<BattleRules>,
    dbs: Res<BattleDbs>,
    mut hand: ResMut<Hand>,
    mut piles: ResMut<CardPiles>,
    mut action_points: ResMut<ActionPoints>,
    mut selected: ResMut<crate::battle::SelectedCards>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    let Some(pending) = pending else {
        return;
    };
    let discard_side = pending.side;
    let next_after_discard = pending.next_phase;

    if locally_controls_discard_side(discard_side, *battle_mode) {
        for (key, index) in card_hotkeys() {
            if !keyboard.just_pressed(key) {
                continue;
            }
            let _ = discard_card_from_hand(
                discard_side,
                index,
                &mut hand,
                &mut piles,
                &rules,
                &mut action_points,
                &dbs,
                &mut event_writer,
            );
            break;
        }
    } else {
        while hand_len(discard_side, &hand) > rules.max_retained_hand {
            let index = hand_len(discard_side, &hand) - 1;
            if discard_card_from_hand(
                discard_side,
                index,
                &mut hand,
                &mut piles,
                &rules,
                &mut action_points,
                &dbs,
                &mut event_writer,
            )
            .is_none()
            {
                break;
            }
        }
    }

    match discard_side {
        Side::Player => {
            selected.player.index = None;
            selected.player.discard_armed = false;
        }
        Side::Enemy => {
            selected.enemy.index = None;
            selected.enemy.discard_armed = false;
        }
    }

    if hand_len(discard_side, &hand) <= rules.max_retained_hand {
        commands.remove_resource::<PendingHandDiscard>();
        next_phase.set(next_after_discard);
    }
}

#[allow(dead_code)]
pub(crate) struct CardPlayContext<'a, 'qw, 'qs, 'mw> {
    pub side: Side,
    pub card_index: usize,
    pub dbs: &'a BattleDbs,
    pub rules: &'a BattleRules,
    pub deck: &'a CardDeck,
    pub hand: &'a mut Hand,
    pub piles: &'a mut CardPiles,
    pub action_points: &'a mut ActionPoints,
    pub pending_boosts: &'a mut PendingBoosts,
    pub memory: &'a mut CardTurnMemory,
    pub player_team: Option<&'a PlayerTeam>,
    pub enemy_team: Option<&'a EnemyTeam>,
    pub combat_query: &'a mut CardCombatQuery<'qw, 'qs>,
    pub event_writer: &'a mut MessageWriter<'mw, BattleEvent>,
}

#[allow(dead_code)]
pub(crate) fn use_card_from_hand(ctx: CardPlayContext) -> Option<(String, String)> {
    let cards = hand_mut(ctx.side, ctx.hand);
    if ctx.card_index >= cards.len() {
        return None;
    }
    let card_id = cards[ctx.card_index];
    let card = ctx.dbs.cards.get(&card_id)?.clone();
    if *ap_mut(ctx.side, ctx.action_points) < card.cost_ap {
        return None;
    }
    cards.remove(ctx.card_index);
    gain_ap(ctx.side, -card.cost_ap, ctx.rules, ctx.action_points);
    ctx.event_writer.write(BattleEvent::CardUsed {
        side: ctx.side,
        card_name: card.name.clone(),
    });
    let detail = apply_card_effect(ctx, &card);
    Some((card.name, detail))
}

fn apply_card_effect(ctx: CardPlayContext, card: &CardDef) -> String {
    match &card.effect {
        CardEffect::GainAp { amount } => {
            gain_ap(ctx.side, *amount, ctx.rules, ctx.action_points);
            format!("获得AP={amount}")
        }
        CardEffect::NextAttackBoost { amount } => {
            pending_mut(ctx.side, ctx.pending_boosts).next_attack_bonus += *amount;
            format!("下次攻击加成={amount}")
        }
        CardEffect::NextShieldBoost { amount } => {
            pending_mut(ctx.side, ctx.pending_boosts).next_shield_bonus += *amount;
            format!("下次护盾加成={amount}")
        }
        CardEffect::NextHealBoost { amount } => {
            pending_mut(ctx.side, ctx.pending_boosts).next_heal_bonus += *amount;
            format!("下次治疗加成={amount}")
        }
        CardEffect::NextElementAttachmentGainAp { amount } => {
            pending_mut(ctx.side, ctx.pending_boosts).next_element_attachment_ap = Some(*amount);
            format!("下次元素附着/反应获得AP={amount}")
        }
        CardEffect::NextReactionFixedDamage {
            amount,
            ignore_shield,
        } => {
            let _ = ignore_shield;
            pending_mut(ctx.side, ctx.pending_boosts).next_reaction_fixed_damage = Some(*amount);
            format!("下次元素反应追加固定伤害={amount}")
        }
        CardEffect::NextWindSpreadDamage {
            amount,
            elements,
            ignore_shield,
        } => {
            let _ = ignore_shield;
            pending_mut(ctx.side, ctx.pending_boosts).next_wind_spread_damage =
                Some((*amount, elements.clone()));
            format!("下次指定元素扩散追加固定伤害={amount}")
        }
        CardEffect::NextAuraAttackDraw { amount } => {
            pending_mut(ctx.side, ctx.pending_boosts).next_aura_attack_draw = Some(*amount);
            format!("下次攻击命中附着目标抽牌={amount}")
        }
        CardEffect::DiscardOtherDrawGainAp {
            draw,
            gain_ap: ap_gain,
        } => {
            let other_index = hand_mut(ctx.side, ctx.hand)
                .iter()
                .enumerate()
                .find(|(_, card_id)| **card_id != card.id)
                .map(|(idx, _)| idx);
            if let Some(index) = other_index {
                let _ = discard_card_from_hand(
                    ctx.side,
                    index,
                    ctx.hand,
                    ctx.piles,
                    ctx.rules,
                    ctx.action_points,
                    ctx.dbs,
                    ctx.event_writer,
                );
                draw_cards(ctx.side, *draw, ctx.hand, ctx.piles, ctx.deck);
                gain_ap(ctx.side, *ap_gain, ctx.rules, ctx.action_points);
                format!("弃置其他手牌1；抽牌={draw}；获得AP={ap_gain}")
            } else {
                "没有其他手牌，效果未触发".to_string()
            }
        }
        CardEffect::NextSkillCostDraw { skill_cost, draw } => {
            pending_mut(ctx.side, ctx.pending_boosts).next_skill_cost_draw =
                Some((*skill_cost, *draw));
            format!("下次使用{skill_cost}AP技能后抽牌={draw}")
        }
        CardEffect::DrawIfKnockedOutThisTurn { amount } => {
            let can_draw = match ctx.side {
                Side::Player => ctx.memory.player.knocked_out_opponent_this_turn,
                Side::Enemy => ctx.memory.enemy.knocked_out_opponent_this_turn,
            };
            if can_draw {
                let drawn = draw_cards(ctx.side, *amount, ctx.hand, ctx.piles, ctx.deck);
                format!("本行动已击倒目标；抽牌={drawn}")
            } else {
                "本行动尚未击倒目标，效果未触发".to_string()
            }
        }
        CardEffect::GainShield { amount } => {
            let gained = gain_shield_on_active(
                ctx.side,
                *amount,
                ctx.player_team,
                ctx.enemy_team,
                ctx.combat_query,
                ctx.event_writer,
            );
            format!("获得护盾={gained}")
        }
        CardEffect::ShieldAbsorbGainAp { amount } => {
            pending_mut(ctx.side, ctx.pending_boosts).shield_absorb_ap = Some(*amount);
            format!("敌方下次行动前护盾吸收伤害时获得AP={amount}")
        }
        CardEffect::ModifyStages {
            attribute,
            amount,
            duration_turns,
        } => {
            let applied = apply_stage_status_on_active(
                ctx.side,
                *attribute,
                *amount,
                *duration_turns,
                ctx.player_team,
                ctx.enemy_team,
                ctx.combat_query,
            );
            if applied {
                format!(
                    "{:?}等级变化={}，持续{}回合",
                    attribute, amount, duration_turns
                )
            } else {
                "属性等级效果未触发".to_string()
            }
        }
        CardEffect::CleanseOrGainAp {
            categories,
            fallback_ap,
        } => {
            if cleanse_one(
                ctx.side,
                categories,
                ctx.player_team,
                ctx.enemy_team,
                ctx.combat_query,
            ) {
                "清除1个可清除效果".to_string()
            } else {
                gain_ap(ctx.side, *fallback_ap, ctx.rules, ctx.action_points);
                format!("无可清除效果；获得AP={fallback_ap}")
            }
        }
        CardEffect::DrawAndGainApIfAliveTeam {
            draw,
            min_alive,
            gain_ap: ap_gain,
        } => {
            let drawn = draw_cards(ctx.side, *draw, ctx.hand, ctx.piles, ctx.deck);
            let alive =
                team_alive_count(ctx.side, ctx.player_team, ctx.enemy_team, ctx.combat_query);
            if alive >= *min_alive {
                gain_ap(ctx.side, *ap_gain, ctx.rules, ctx.action_points);
                format!("抽牌={drawn}；存活队友数={alive}，获得AP={ap_gain}")
            } else {
                format!("抽牌={drawn}；存活队友数={alive}，未获得额外AP")
            }
        }
        CardEffect::GainShieldDrawIfSwitchedThisTurn { shield, draw } => {
            let gained = gain_shield_on_active(
                ctx.side,
                *shield,
                ctx.player_team,
                ctx.enemy_team,
                ctx.combat_query,
                ctx.event_writer,
            );
            let switched = match ctx.side {
                Side::Player => ctx.memory.player.switched_this_turn,
                Side::Enemy => ctx.memory.enemy.switched_this_turn,
            };
            if switched {
                let drawn = draw_cards(ctx.side, *draw, ctx.hand, ctx.piles, ctx.deck);
                format!("获得护盾={gained}；本行动已换人，抽牌={drawn}")
            } else {
                pending_mut(ctx.side, ctx.pending_boosts).next_switch_draw = Some(*draw);
                format!("获得护盾={gained}；等待本行动内换人后抽牌={draw}")
            }
        }
    }
}

fn gain_shield_on_active(
    side: Side,
    amount: i32,
    player_team: Option<&PlayerTeam>,
    enemy_team: Option<&EnemyTeam>,
    query: &mut CardCombatQuery,
    event_writer: &mut MessageWriter<BattleEvent>,
) -> i32 {
    let Some(entity) = active_entity(side, player_team, enemy_team) else {
        return 0;
    };
    let Ok((_, _, mut shield, _, _)) = query.get_mut(entity) else {
        return 0;
    };
    let amount = amount.max(0);
    shield.0 += amount;
    event_writer.write(BattleEvent::ShieldGained { side, amount });
    amount
}

fn apply_stage_status_on_active(
    side: Side,
    attribute: AttributeType,
    amount: i32,
    duration_turns: i32,
    player_team: Option<&PlayerTeam>,
    enemy_team: Option<&EnemyTeam>,
    query: &mut CardCombatQuery,
) -> bool {
    let Some(entity) = active_entity(side, player_team, enemy_team) else {
        return false;
    };
    let Ok((_, mut stats, _, mut statuses, _)) = query.get_mut(entity) else {
        return false;
    };
    let status = StatusInstance {
        id: format!("card_stage_{side:?}_{attribute:?}"),
        name: "技能牌属性强化".to_string(),
        category: StatusCategory::Buff,
        remaining_turns: duration_turns,
        applied_round: 0,
        source_side: Some(side),
        tick_timing: None,
        stage_modifiers: vec![crate::battle::StatusStageModifier { attribute, amount }],
        fixed_damage_on_tick: 0,
        heal_on_tick: 0,
        heal_taken_multiplier: None,
        evade_charges: 0,
    };
    upsert_status_instance(&mut statuses, status, &mut stats);
    true
}

fn cleanse_one(
    side: Side,
    categories: &[StatusCategory],
    player_team: Option<&PlayerTeam>,
    enemy_team: Option<&EnemyTeam>,
    query: &mut CardCombatQuery,
) -> bool {
    let Some(entity) = active_entity(side, player_team, enemy_team) else {
        return false;
    };
    let Ok((_, mut stats, _, mut statuses, mut aura)) = query.get_mut(entity) else {
        return false;
    };
    for category in categories {
        if *category == StatusCategory::Aura {
            if let Some(element) = aura.elements().first().copied() {
                aura.remove(element);
                let status_ids = aura_status_ids(element);
                statuses
                    .entries
                    .retain(|entry| !status_ids.contains(&entry.id.as_str()));
                crate::battle::recalculate_stage_modifiers(&mut stats, &statuses);
                return true;
            }
        }
        if let Some(index) = statuses
            .entries
            .iter()
            .position(|entry| entry.category == *category)
        {
            statuses.entries.remove(index);
            crate::battle::recalculate_stage_modifiers(&mut stats, &statuses);
            return true;
        }
    }
    false
}

fn aura_status_ids(element: ElementType) -> &'static [&'static str] {
    match element {
        ElementType::Fire => &["burning_aura"],
        ElementType::Water => &["wet_aura"],
        ElementType::Grass => &["thorn_aura"],
        ElementType::Thunder => &["paralysis_aura"],
        _ => &[],
    }
}

fn apply_fixed_damage_to_active(
    source: Side,
    target: Side,
    amount: i32,
    player_team: Option<&PlayerTeam>,
    enemy_team: Option<&EnemyTeam>,
    query: &mut CardCombatQuery,
    event_writer: &mut MessageWriter<BattleEvent>,
) {
    let Some(entity) = active_entity(target, player_team, enemy_team) else {
        return;
    };
    let Ok((_, mut stats, mut shield, _, _)) = query.get_mut(entity) else {
        return;
    };
    let absorbed = shield.0.min(amount.max(0));
    if absorbed > 0 {
        shield.0 -= absorbed;
        event_writer.write(BattleEvent::ShieldAbsorbed {
            side: target,
            amount: absorbed,
        });
    }
    let hp_damage = (amount - absorbed).max(0);
    if hp_damage > 0 {
        stats.hp = (stats.hp - hp_damage).max(0);
    }
    event_writer.write(BattleEvent::DamageDealt {
        source,
        target,
        amount: hp_damage,
    });
}

pub(crate) fn card_trigger_event_system(
    mut messages: ParamSet<(MessageReader<BattleEvent>, MessageWriter<BattleEvent>)>,
    mut pending_boosts: ResMut<PendingBoosts>,
    mut memory: ResMut<CardTurnMemory>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    mut piles: ResMut<CardPiles>,
    rules: Res<BattleRules>,
    deck: Res<CardDeck>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    dbs: Res<BattleDbs>,
    mut query: CardCombatQuery,
) {
    let events: Vec<_> = messages.p0().read().cloned().collect();
    for event in events {
        match &event {
            BattleEvent::CardDiscarded { side, card_name } => {
                if let Some(card) = dbs.cards.values().find(|card| card.name == *card_name) {
                    piles.push_discard(*side, card.id);
                }
            }
            BattleEvent::CardUsed { side, card_name } => {
                if let Some(card) = dbs
                    .cards
                    .values()
                    .find(|card| card.name == *card_name)
                    .cloned()
                {
                    piles.push_discard(*side, card.id);
                    let mut writer = messages.p1();
                    let _ = apply_card_effect(
                        CardPlayContext {
                            side: *side,
                            card_index: 0,
                            dbs: &dbs,
                            rules: &rules,
                            deck: &deck,
                            hand: &mut hand,
                            piles: &mut piles,
                            action_points: &mut action_points,
                            pending_boosts: &mut pending_boosts,
                            memory: &mut memory,
                            player_team: player_team.as_deref(),
                            enemy_team: enemy_team.as_deref(),
                            combat_query: &mut query,
                            event_writer: &mut writer,
                        },
                        &card,
                    );
                }
            }
            BattleEvent::ReactionTriggered { source, target, .. } => {
                if let Some(amount) = pending_mut(*source, &mut pending_boosts)
                    .next_element_attachment_ap
                    .take()
                {
                    gain_ap(*source, amount, &rules, &mut action_points);
                }
                if let Some(draw) = pending_mut(*source, &mut pending_boosts)
                    .next_aura_attack_draw
                    .take()
                {
                    draw_cards(*source, draw, &mut hand, &mut piles, &deck);
                }
                if let Some(amount) = pending_mut(*source, &mut pending_boosts)
                    .next_reaction_fixed_damage
                    .take()
                {
                    let mut writer = messages.p1();
                    apply_fixed_damage_to_active(
                        *source,
                        *target,
                        amount,
                        player_team.as_deref(),
                        enemy_team.as_deref(),
                        &mut query,
                        &mut writer,
                    );
                }
            }
            BattleEvent::ElementAuraApplied {
                side: target,
                from,
                to,
                ..
            } => {
                let source = crate::battle::opposite_side(*target);
                if (!from.is_empty() || !to.is_empty())
                    && let Some(amount) = pending_mut(source, &mut pending_boosts)
                        .next_element_attachment_ap
                        .take()
                {
                    gain_ap(source, amount, &rules, &mut action_points);
                }
                if !from.is_empty()
                    && let Some(draw) = pending_mut(source, &mut pending_boosts)
                        .next_aura_attack_draw
                        .take()
                {
                    draw_cards(source, draw, &mut hand, &mut piles, &deck);
                }
            }
            BattleEvent::WindSpreadTriggered {
                source,
                target,
                element,
            } => {
                let pending = pending_mut(*source, &mut pending_boosts)
                    .next_wind_spread_damage
                    .take();
                if let Some((amount, elements)) = pending {
                    if elements.contains(element) {
                        let mut writer = messages.p1();
                        apply_fixed_damage_to_active(
                            *source,
                            *target,
                            amount,
                            player_team.as_deref(),
                            enemy_team.as_deref(),
                            &mut query,
                            &mut writer,
                        );
                    } else {
                        pending_mut(*source, &mut pending_boosts).next_wind_spread_damage =
                            Some((amount, elements));
                    }
                }
            }
            BattleEvent::ShieldAbsorbed { side, amount } if *amount > 0 => {
                if let Some(ap) = pending_mut(*side, &mut pending_boosts)
                    .shield_absorb_ap
                    .take()
                {
                    gain_ap(*side, ap, &rules, &mut action_points);
                }
            }
            BattleEvent::SkillUsed { side, .. } => {
                let Some((skill_cost, draw)) =
                    pending_mut(*side, &mut pending_boosts).next_skill_cost_draw
                else {
                    continue;
                };
                let matched = dbs.skills.values().any(|skill| {
                    skill.cost_ap == skill_cost && skill.name == skill_name_from_event(&event)
                });
                if matched {
                    pending_mut(*side, &mut pending_boosts).next_skill_cost_draw = None;
                    draw_cards(*side, draw, &mut hand, &mut piles, &deck);
                }
            }
            BattleEvent::DamageDealt {
                source,
                target,
                amount,
            } if *amount > 0 => {
                let has_aura =
                    active_entity(*target, player_team.as_deref(), enemy_team.as_deref())
                        .and_then(|entity| {
                            query
                                .get(entity)
                                .ok()
                                .map(|(_, _, _, _, aura)| !aura.elements().is_empty())
                        })
                        .unwrap_or(false);
                if has_aura
                    && let Some(draw) = pending_mut(*source, &mut pending_boosts)
                        .next_aura_attack_draw
                        .take()
                {
                    draw_cards(*source, draw, &mut hand, &mut piles, &deck);
                }
            }
            BattleEvent::CombatantFainted { side, .. } => match side {
                Side::Player => memory.enemy.knocked_out_opponent_this_turn = true,
                Side::Enemy => memory.player.knocked_out_opponent_this_turn = true,
            },
            BattleEvent::Switched { side, .. } => {
                match side {
                    Side::Player => memory.player.switched_this_turn = true,
                    Side::Enemy => memory.enemy.switched_this_turn = true,
                }
                if let Some(draw) = pending_mut(*side, &mut pending_boosts)
                    .next_switch_draw
                    .take()
                {
                    draw_cards(*side, draw, &mut hand, &mut piles, &deck);
                }
            }
            _ => {}
        }
    }
}

fn skill_name_from_event(event: &BattleEvent) -> &str {
    if let BattleEvent::SkillUsed { skill_name, .. } = event {
        skill_name
    } else {
        ""
    }
}
