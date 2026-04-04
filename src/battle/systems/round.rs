use bevy::prelude::*;

use crate::{
    battle::{
        ActionPoints, BattleEvent, Hand, PendingBoosts, SelectedCard, TurnContext, TurnCount,
    },
    data::{BattleDbs, BattleRules, CardDeck},
    game_state::BattlePhase,
};

pub fn round_start_system(
    card_deck: Res<CardDeck>,
    dbs: Res<BattleDbs>,
    rules: Res<BattleRules>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    mut turn_ctx: ResMut<TurnContext>,
    mut turn_count: ResMut<TurnCount>,
    mut pending_boosts: ResMut<PendingBoosts>,
    mut selected: ResMut<SelectedCard>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    if card_deck.0.is_empty() {
        return;
    }

    turn_count.0 += 1;
    event_writer.write(BattleEvent::TurnStarted(turn_count.0));

    // 每回合开始抽取手牌（重置本回合手牌）。
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

    let player_cards: Vec<String> = hand
        .player
        .iter()
        .map(|cid| {
            dbs.cards
                .get(cid)
                .map(|c| c.name.to_string())
                .unwrap_or_else(|| format!("{cid:?}"))
        })
        .collect();
    let enemy_cards: Vec<String> = hand
        .enemy
        .iter()
        .map(|cid| {
            dbs.cards
                .get(cid)
                .map(|c| c.name.to_string())
                .unwrap_or_else(|| format!("{cid:?}"))
        })
        .collect();
    println!("玩家抽到: {}", player_cards.join(" / "));
    println!("敌方抽到: {}", enemy_cards.join(" / "));

    // “回合开始时额外 +6”，并保留继承的剩余 AP。
    action_points.player += rules.ap_per_round;
    action_points.enemy += rules.ap_per_round;

    // 重置本回合出牌权标记。
    turn_ctx.player_ended = false;
    turn_ctx.enemy_ended = false;

    // 每回合开始清空增益，增益仅本回合有效。
    *pending_boosts = PendingBoosts::default();
    // 清除上回合残留的卡牌选择状态。
    *selected = SelectedCard::default();

    next_phase.set(BattlePhase::PlayerTurn);
}
