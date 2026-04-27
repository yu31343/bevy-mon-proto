use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    battle::{
        ActionPoints, ActionTrace, BattleEvent, Hand, PendingBoosts, PlayerTeam, RoundOrder,
        SelectedCard, Side, Stats, StructuredBattleLog, TurnContext, TurnCount, note_round_phase,
        opposite_side, push_named_action_trace,
    },
    data::{BattleDbs, BattleFormulaRules, BattleRules, CardDeck},
    game_state::BattlePhase,
};

fn hand_names(hand: &[crate::data::CardId], dbs: &BattleDbs) -> String {
    hand.iter()
        .map(|cid| {
            dbs.cards
                .get(cid)
                .map(|c| c.name.to_string())
                .unwrap_or_else(|| format!("{cid:?}"))
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

#[derive(SystemParam)]
pub(crate) struct RoundStartResources<'w> {
    dbs: Res<'w, BattleDbs>,
    rules: Res<'w, BattleRules>,
    formula_rules: Res<'w, BattleFormulaRules>,
    player_team: Res<'w, PlayerTeam>,
    enemy_team: Res<'w, crate::battle::EnemyTeam>,
    action_points: ResMut<'w, ActionPoints>,
    hand: ResMut<'w, Hand>,
    turn_ctx: ResMut<'w, TurnContext>,
    turn_count: ResMut<'w, TurnCount>,
    round_order: ResMut<'w, RoundOrder>,
    pending_boosts: ResMut<'w, PendingBoosts>,
    selected: ResMut<'w, SelectedCard>,
    structured_log: ResMut<'w, StructuredBattleLog>,
    action_trace: ResMut<'w, ActionTrace>,
}

pub fn round_start_system(
    card_deck: Res<CardDeck>,
    query: Query<&Stats>,
    mut runtime: RoundStartResources,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    let dbs = &runtime.dbs;
    let rules = &runtime.rules;
    let formula_rules = &runtime.formula_rules;
    let player_team = &runtime.player_team;
    let enemy_team = &runtime.enemy_team;
    let action_points = &mut runtime.action_points;
    let hand = &mut runtime.hand;
    let turn_ctx = &mut runtime.turn_ctx;
    let turn_count = &mut runtime.turn_count;
    let round_order = &mut runtime.round_order;
    let pending_boosts = &mut runtime.pending_boosts;
    let selected = &mut runtime.selected;
    let structured_log = &mut runtime.structured_log;
    let action_trace = &mut runtime.action_trace;

    if card_deck.0.is_empty() {
        return;
    }

    turn_count.0 += 1;
    event_writer.write(BattleEvent::TurnStarted(turn_count.0));

    hand.player.clear();
    hand.enemy.clear();
    let deck_len = card_deck.0.len();
    let seed = turn_count.0 as usize;
    for i in 0..rules.cards_per_round {
        let idx = (seed * 7 + i * 3) % deck_len;
        hand.player.push(card_deck.0[idx]);
    }
    for i in 0..rules.cards_per_round {
        let idx = (seed * 11 + i * 5) % deck_len;
        hand.enemy.push(card_deck.0[idx]);
    }

    let player_cards = hand_names(&hand.player, &dbs);
    let enemy_cards = hand_names(&hand.enemy, &dbs);
    println!("玩家抽到: {}", player_cards);
    println!("敌方抽到: {}", enemy_cards);

    action_points.player += rules.ap_per_round;
    action_points.enemy += rules.ap_per_round;

    turn_ctx.player_ended = false;
    turn_ctx.enemy_ended = false;
    **pending_boosts = PendingBoosts::default();
    **selected = SelectedCard::default();

    let Some(player_entity) = player_team.0.active_combatant() else {
        return;
    };
    let Some(enemy_entity) = enemy_team.0.active_combatant() else {
        return;
    };
    let Ok(player_stats) = query.get(player_entity) else {
        return;
    };
    let Ok(enemy_stats) = query.get(enemy_entity) else {
        return;
    };

    let player_spd = super::combat::effective_spd(player_stats, &formula_rules);
    let enemy_spd = super::combat::effective_spd(enemy_stats, &formula_rules);
    let first_side = if player_spd > enemy_spd {
        Side::Player
    } else if enemy_spd > player_spd {
        Side::Enemy
    } else {
        round_order
            .previous_first
            .map(opposite_side)
            .unwrap_or(Side::Player)
    };
    round_order.set_first(first_side);

    let ap_snapshot = format!(
        "玩家AP={}, 敌方AP={}",
        action_points.player, action_points.enemy
    );
    note_round_phase(
        structured_log,
        turn_count.0,
        format!(
            "玩家手牌 [{}]；敌方手牌 [{}]；{}；先手={:?}；玩家Spd={}；敌方Spd={}",
            player_cards, enemy_cards, ap_snapshot, round_order.first, player_spd, enemy_spd
        ),
    );
    push_named_action_trace(
        action_trace,
        turn_count.0,
        Side::Player,
        "round_draw",
        format!("玩家抽牌 [{}]；{}", player_cards, ap_snapshot),
    );
    push_named_action_trace(
        action_trace,
        turn_count.0,
        Side::Enemy,
        "round_draw",
        format!("敌方抽牌 [{}]；{}", enemy_cards, ap_snapshot),
    );

    next_phase.set(match round_order.first {
        Side::Player => BattlePhase::PlayerTurn,
        Side::Enemy => BattlePhase::EnemyTurn,
    });
}
