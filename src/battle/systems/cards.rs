use bevy::prelude::*;

use crate::{
    battle::{
        ActionPoints, BattleControlMode, BattleEvent, CardPiles, CardTurnMemory, Combatant,
        ElementAura, EnemyTeam, Hand, InBattle, PendingBoost, PendingBoosts,
        PendingGuardCounterClear, PendingHandDiscard, PendingTacticalDiscard, PlayerTeam, Shield,
        Side, Stats, StatusBoard, StatusInstance, upsert_status_instance,
    },
    console_log::{ConsoleLogCategory, log as console_log},
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

fn selected_mut(
    side: Side,
    selected: &mut crate::battle::SelectedCards,
) -> &mut crate::battle::SelectedCardState {
    match side {
        Side::Player => &mut selected.player,
        Side::Enemy => &mut selected.enemy,
    }
}

fn pending_mut(side: Side, pending_boosts: &mut PendingBoosts) -> &mut PendingBoost {
    match side {
        Side::Player => &mut pending_boosts.player,
        Side::Enemy => &mut pending_boosts.enemy,
    }
}

fn add_i32_pending(slot: &mut Option<i32>, amount: i32) {
    *slot = Some(slot.unwrap_or(0) + amount);
}

fn take_shield_bonus(side: Side, pending_boosts: &mut PendingBoosts) -> i32 {
    let pending = pending_mut(side, pending_boosts);
    let bonus = pending.next_shield_bonus;
    pending.next_shield_bonus = 0;
    bonus
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
    _rules: &BattleRules,
    action_points: &mut ActionPoints,
) {
    let ap = ap_mut(side, action_points);
    *ap = (*ap + amount).max(0);
}

pub(crate) fn clamp_ap_to_max(side: Side, rules: &BattleRules, action_points: &mut ActionPoints) {
    let ap = ap_mut(side, action_points);
    *ap = (*ap).min(rules.max_ap);
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
        let Some(card_id) = piles.draw_one(&deck.0) else {
            break;
        };
        hand_mut(side, hand).push(card_id);
        drawn += 1;
    }
    drawn
}

fn card_name(dbs: &BattleDbs, card_id: CardId) -> String {
    dbs.cards
        .get(&card_id)
        .map(|card| card.name.clone())
        .unwrap_or_else(|| format!("{card_id:?}"))
}

fn draw_cards_with_names(
    side: Side,
    amount: usize,
    hand: &mut Hand,
    piles: &mut CardPiles,
    deck: &CardDeck,
    dbs: &BattleDbs,
) -> Vec<String> {
    let mut drawn_cards = Vec::new();
    for _ in 0..amount {
        let Some(card_id) = piles.draw_one(&deck.0) else {
            break;
        };
        hand_mut(side, hand).push(card_id);
        drawn_cards.push(card_name(dbs, card_id));
    }
    drawn_cards
}

fn merge_card_effect_sources(existing: &mut String, source_card: &str) {
    if !existing.split(" + ").any(|source| source == source_card) {
        existing.push_str(" + ");
        existing.push_str(source_card);
    }
}

fn drawn_cards_text(drawn_cards: &[String]) -> String {
    if drawn_cards.is_empty() {
        "无".to_string()
    } else {
        drawn_cards.join(" / ")
    }
}

fn log_card_effect_draw(
    side: Side,
    source_card: &str,
    reason: &str,
    requested: usize,
    drawn_cards: &[String],
    hand: &Hand,
    piles: &CardPiles,
) {
    console_log(
        ConsoleLogCategory::Cards,
        format!(
            "[{}] 卡牌效果抽牌：来源={}；原因={}；请求 {} 张；实际 {} 张；抽到=[{}]；手牌 {} 张；牌堆 {} 张；弃牌 {} 张",
            super::side_text(side),
            source_card,
            reason,
            requested,
            drawn_cards.len(),
            drawn_cards_text(drawn_cards),
            hand_len(side, hand),
            piles.draw.len(),
            piles.discard.len()
        ),
    );
}

pub(crate) fn clear_action_scoped_card_effects(side: Side, pending_boosts: &mut PendingBoosts) {
    let pending = pending_mut(side, pending_boosts);
    pending.next_attack_bonus = 0;
    pending.next_shield_bonus = 0;
    pending.next_heal_bonus = 0;
    pending.next_element_attachment_ap = None;
    pending.next_reaction_fixed_damage = None;
    pending.next_wind_spread_damage = None;
    pending.next_aura_attack_draw = None;
    pending.next_skill_cost_draw = None;
    pending.next_switch_draw = None;
    pending.next_knockout_draw = None;
}

pub(crate) fn clear_round_scoped_card_effects(pending_boosts: &mut PendingBoosts) {
    let player_counter = pending_boosts.player.shield_absorb_ap;
    let enemy_counter = pending_boosts.enemy.shield_absorb_ap;
    pending_boosts.player = PendingBoost {
        shield_absorb_ap: player_counter,
        ..Default::default()
    };
    pending_boosts.enemy = PendingBoost {
        shield_absorb_ap: enemy_counter,
        ..Default::default()
    };
}

pub(crate) fn clear_opponent_guard_counter_effects(
    acting_side: Side,
    pending_boosts: &mut PendingBoosts,
) {
    let counter_owner = match acting_side {
        Side::Player => Side::Enemy,
        Side::Enemy => Side::Player,
    };
    pending_mut(counter_owner, pending_boosts).shield_absorb_ap = None;
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
    action_points: &mut ActionPoints,
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
        clamp_ap_to_max(side, rules, action_points);
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
        clamp_ap_to_max(discard_side, &rules, &mut action_points);
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
    pub pending_tactical_discard: Option<&'a mut Option<PendingTacticalDiscard>>,
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
            add_i32_pending(
                &mut pending_mut(ctx.side, ctx.pending_boosts).next_element_attachment_ap,
                *amount,
            );
            format!("下次元素附着/反应获得AP={amount}")
        }
        CardEffect::NextReactionFixedDamage {
            amount,
            ignore_shield,
        } => {
            let _ = ignore_shield;
            add_i32_pending(
                &mut pending_mut(ctx.side, ctx.pending_boosts).next_reaction_fixed_damage,
                *amount,
            );
            format!("下次元素反应追加固定伤害={amount}")
        }
        CardEffect::NextWindSpreadDamage {
            amount,
            elements,
            ignore_shield,
        } => {
            let _ = ignore_shield;
            let pending = pending_mut(ctx.side, ctx.pending_boosts);
            match &mut pending.next_wind_spread_damage {
                Some((existing_amount, existing_elements)) => {
                    *existing_amount += *amount;
                    for element in elements {
                        if !existing_elements.contains(element) {
                            existing_elements.push(*element);
                        }
                    }
                }
                None => pending.next_wind_spread_damage = Some((*amount, elements.clone())),
            }
            format!("下次指定元素扩散追加固定伤害={amount}")
        }
        CardEffect::NextAuraAttackDraw { amount } => {
            let pending = pending_mut(ctx.side, ctx.pending_boosts);
            match &mut pending.next_aura_attack_draw {
                Some((existing_amount, source_card)) => {
                    *existing_amount += *amount;
                    merge_card_effect_sources(source_card, &card.name);
                }
                None => pending.next_aura_attack_draw = Some((*amount, card.name.clone())),
            }
            format!("下次攻击命中附着目标抽牌={amount}")
        }
        CardEffect::DiscardOtherDrawGainAp {
            draw,
            gain_ap: ap_gain,
        } => {
            if hand_mut(ctx.side, ctx.hand).is_empty() {
                "没有其他手牌，效果未触发".to_string()
            } else {
                if let Some(pending_tactical_discard) = ctx.pending_tactical_discard {
                    *pending_tactical_discard = Some(PendingTacticalDiscard {
                        side: ctx.side,
                        draw: *draw,
                        source_card: card.name.clone(),
                    });
                }
                format!("等待选择弃置其他手牌1；弃后抽牌={draw}；弃牌获得AP={ap_gain}")
            }
        }
        CardEffect::NextSkillCostDraw { skill_cost, draw } => {
            let pending = pending_mut(ctx.side, ctx.pending_boosts);
            match &mut pending.next_skill_cost_draw {
                Some((existing_cost, existing_draw, source_card))
                    if *existing_cost == *skill_cost =>
                {
                    *existing_draw += *draw;
                    merge_card_effect_sources(source_card, &card.name);
                }
                _ => pending.next_skill_cost_draw = Some((*skill_cost, *draw, card.name.clone())),
            }
            format!("下次使用{skill_cost}AP技能后抽牌={draw}")
        }
        CardEffect::DrawIfKnockedOutThisTurn { amount } => {
            let can_draw = match ctx.side {
                Side::Player => ctx.memory.player.knocked_out_opponent_this_turn,
                Side::Enemy => ctx.memory.enemy.knocked_out_opponent_this_turn,
            };
            if can_draw {
                let drawn_cards = draw_cards_with_names(
                    ctx.side, *amount, ctx.hand, ctx.piles, ctx.deck, ctx.dbs,
                );
                log_card_effect_draw(
                    ctx.side,
                    &card.name,
                    "本行动已击倒目标",
                    *amount,
                    &drawn_cards,
                    ctx.hand,
                    ctx.piles,
                );
                format!("本行动已击倒目标；抽牌={}", drawn_cards.len())
            } else {
                let pending = pending_mut(ctx.side, ctx.pending_boosts);
                match &mut pending.next_knockout_draw {
                    Some((existing_amount, source_card)) => {
                        *existing_amount += *amount;
                        merge_card_effect_sources(source_card, &card.name);
                    }
                    None => pending.next_knockout_draw = Some((*amount, card.name.clone())),
                }
                format!("等待本行动内击倒敌方精灵后抽牌={amount}")
            }
        }
        CardEffect::GainShield { amount } => {
            let gained = gain_shield_on_active(
                ctx.side,
                *amount,
                ctx.pending_boosts,
                ctx.player_team,
                ctx.enemy_team,
                ctx.combat_query,
                ctx.event_writer,
            );
            format!("获得护盾={gained}")
        }
        CardEffect::ShieldAbsorbGainAp { amount } => {
            add_i32_pending(
                &mut pending_mut(ctx.side, ctx.pending_boosts).shield_absorb_ap,
                *amount,
            );
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
            let drawn_cards =
                draw_cards_with_names(ctx.side, *draw, ctx.hand, ctx.piles, ctx.deck, ctx.dbs);
            log_card_effect_draw(
                ctx.side,
                &card.name,
                "立即抽牌并检查存活队友",
                *draw,
                &drawn_cards,
                ctx.hand,
                ctx.piles,
            );
            let alive =
                team_alive_count(ctx.side, ctx.player_team, ctx.enemy_team, ctx.combat_query);
            if alive >= *min_alive {
                gain_ap(ctx.side, *ap_gain, ctx.rules, ctx.action_points);
                format!(
                    "抽牌={}；存活队友数={alive}，获得AP={ap_gain}",
                    drawn_cards.len()
                )
            } else {
                format!(
                    "抽牌={}；存活队友数={alive}，未获得额外AP",
                    drawn_cards.len()
                )
            }
        }
        CardEffect::GainShieldDrawIfSwitchedThisTurn { shield, draw } => {
            let gained = gain_shield_on_active(
                ctx.side,
                *shield,
                ctx.pending_boosts,
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
                let drawn_cards =
                    draw_cards_with_names(ctx.side, *draw, ctx.hand, ctx.piles, ctx.deck, ctx.dbs);
                log_card_effect_draw(
                    ctx.side,
                    &card.name,
                    "本行动已换人",
                    *draw,
                    &drawn_cards,
                    ctx.hand,
                    ctx.piles,
                );
                format!(
                    "获得护盾={gained}；本行动已换人，抽牌={}",
                    drawn_cards.len()
                )
            } else {
                let pending = pending_mut(ctx.side, ctx.pending_boosts);
                match &mut pending.next_switch_draw {
                    Some((existing_amount, source_card)) => {
                        *existing_amount += *draw;
                        merge_card_effect_sources(source_card, &card.name);
                    }
                    None => pending.next_switch_draw = Some((*draw, card.name.clone())),
                }
                format!("获得护盾={gained}；等待本行动内换人后抽牌={draw}")
            }
        }
    }
}

fn gain_shield_on_active(
    side: Side,
    amount: i32,
    pending_boosts: &mut PendingBoosts,
    player_team: Option<&PlayerTeam>,
    enemy_team: Option<&EnemyTeam>,
    query: &mut CardCombatQuery,
    event_writer: &mut MessageWriter<BattleEvent>,
) -> i32 {
    let Some(entity) = active_entity(side, player_team, enemy_team) else {
        return 0;
    };
    let Ok((_, stats, mut shield, _, _)) = query.get_mut(entity) else {
        return 0;
    };
    let amount = (amount + take_shield_bonus(side, pending_boosts)).max(0);
    let gained = shield.gain_capped(amount, stats.max_hp);
    event_writer.write(BattleEvent::ShieldGained {
        side,
        amount: gained,
    });
    gained
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
    mut commands: Commands,
    mut messages: ParamSet<(MessageReader<BattleEvent>, MessageWriter<BattleEvent>)>,
    pending_guard_clear: Option<Res<PendingGuardCounterClear>>,
    pending_tactical_discard: Option<Res<PendingTacticalDiscard>>,
    mut selected: ResMut<crate::battle::SelectedCards>,
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
    let pending_tactical = pending_tactical_discard.as_deref().cloned();
    let mut resolved_tactical_side = None;
    let mut new_tactical_discard = None;
    let events: Vec<_> = messages.p0().read().cloned().collect();
    for event in events {
        match &event {
            BattleEvent::CardDiscarded { side, card_name } => {
                if let Some(card) = dbs.cards.values().find(|card| card.name == *card_name) {
                    piles.push_discard(*side, card.id);
                    console_log(
                        ConsoleLogCategory::Cards,
                        format!(
                            "[{}] 弃置卡牌：{}；牌堆 {} 张；弃牌 {} 张",
                            super::side_text(*side),
                            card_name,
                            piles.draw.len(),
                            piles.discard.len()
                        ),
                    );
                }
                if resolved_tactical_side.is_none()
                    && let Some(pending) = pending_tactical
                        .as_ref()
                        .filter(|pending| pending.side == *side)
                {
                    let drawn_cards = draw_cards_with_names(
                        *side,
                        pending.draw,
                        &mut hand,
                        &mut piles,
                        &deck,
                        &dbs,
                    );
                    log_card_effect_draw(
                        *side,
                        &pending.source_card,
                        "战术整理弃置其他手牌后抽牌",
                        pending.draw,
                        &drawn_cards,
                        &hand,
                        &piles,
                    );
                    resolved_tactical_side = Some(*side);
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
                    console_log(
                        ConsoleLogCategory::Cards,
                        format!(
                            "[{}] 使用卡牌：{}；牌堆 {} 张；弃牌 {} 张",
                            super::side_text(*side),
                            card_name,
                            piles.draw.len(),
                            piles.discard.len()
                        ),
                    );
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
                            pending_tactical_discard: Some(&mut new_tactical_discard),
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
                if let Some((draw, source_card)) = pending_mut(*source, &mut pending_boosts)
                    .next_aura_attack_draw
                    .take()
                {
                    let drawn_cards =
                        draw_cards_with_names(*source, draw, &mut hand, &mut piles, &deck, &dbs);
                    log_card_effect_draw(
                        *source,
                        &source_card,
                        "元素反应触发附着目标抽牌",
                        draw,
                        &drawn_cards,
                        &hand,
                        &piles,
                    );
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
                    && let Some((draw, source_card)) = pending_mut(source, &mut pending_boosts)
                        .next_aura_attack_draw
                        .take()
                {
                    let drawn_cards =
                        draw_cards_with_names(source, draw, &mut hand, &mut piles, &deck, &dbs);
                    log_card_effect_draw(
                        source,
                        &source_card,
                        "元素附着变化触发抽牌",
                        draw,
                        &drawn_cards,
                        &hand,
                        &piles,
                    );
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
                let Some((skill_cost, draw, source_card)) = pending_mut(*side, &mut pending_boosts)
                    .next_skill_cost_draw
                    .clone()
                else {
                    continue;
                };
                let skill_name = skill_name_from_event(&event);
                let matched = dbs
                    .skills
                    .values()
                    .any(|skill| skill.cost_ap == skill_cost && skill.name == skill_name);
                if matched {
                    pending_mut(*side, &mut pending_boosts).next_skill_cost_draw = None;
                    let drawn_cards =
                        draw_cards_with_names(*side, draw, &mut hand, &mut piles, &deck, &dbs);
                    log_card_effect_draw(
                        *side,
                        &source_card,
                        &format!("使用{}AP技能：{}", skill_cost, skill_name),
                        draw,
                        &drawn_cards,
                        &hand,
                        &piles,
                    );
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
                    && let Some((draw, source_card)) = pending_mut(*source, &mut pending_boosts)
                        .next_aura_attack_draw
                        .take()
                {
                    let drawn_cards =
                        draw_cards_with_names(*source, draw, &mut hand, &mut piles, &deck, &dbs);
                    log_card_effect_draw(
                        *source,
                        &source_card,
                        "攻击命中附着目标",
                        draw,
                        &drawn_cards,
                        &hand,
                        &piles,
                    );
                }
            }
            BattleEvent::CombatantFainted { side, .. } => {
                let scoring_side = match side {
                    Side::Player => {
                        memory.enemy.knocked_out_opponent_this_turn = true;
                        Side::Enemy
                    }
                    Side::Enemy => {
                        memory.player.knocked_out_opponent_this_turn = true;
                        Side::Player
                    }
                };
                if let Some((draw, source_card)) = pending_mut(scoring_side, &mut pending_boosts)
                    .next_knockout_draw
                    .take()
                {
                    let drawn_cards = draw_cards_with_names(
                        scoring_side,
                        draw,
                        &mut hand,
                        &mut piles,
                        &deck,
                        &dbs,
                    );
                    log_card_effect_draw(
                        scoring_side,
                        &source_card,
                        "本行动击倒敌方精灵",
                        draw,
                        &drawn_cards,
                        &hand,
                        &piles,
                    );
                }
            }
            BattleEvent::Switched { side, .. } => {
                match side {
                    Side::Player => memory.player.switched_this_turn = true,
                    Side::Enemy => memory.enemy.switched_this_turn = true,
                }
                if let Some((draw, source_card)) = pending_mut(*side, &mut pending_boosts)
                    .next_switch_draw
                    .take()
                {
                    let drawn_cards =
                        draw_cards_with_names(*side, draw, &mut hand, &mut piles, &deck, &dbs);
                    log_card_effect_draw(
                        *side,
                        &source_card,
                        "本行动换人后抽牌",
                        draw,
                        &drawn_cards,
                        &hand,
                        &piles,
                    );
                }
            }
            _ => {}
        }
    }

    if let Some(side) = resolved_tactical_side {
        let selected_state = selected_mut(side, &mut selected);
        selected_state.index = None;
        selected_state.discard_armed = false;
        commands.remove_resource::<PendingTacticalDiscard>();
    }

    if let Some(pending) = new_tactical_discard {
        let selected_state = selected_mut(pending.side, &mut selected);
        selected_state.index = None;
        selected_state.discard_armed = true;
        commands.insert_resource(pending);
    }

    if let Some(pending_guard_clear) = pending_guard_clear {
        clear_opponent_guard_counter_effects(pending_guard_clear.acting_side, &mut pending_boosts);
        commands.remove_resource::<PendingGuardCounterClear>();
    }
}

fn skill_name_from_event(event: &BattleEvent) -> &str {
    if let BattleEvent::SkillUsed { skill_name, .. } = event {
        skill_name
    } else {
        ""
    }
}
