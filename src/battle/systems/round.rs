use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    battle::{
        ActionPoints, ActionTrace, BattleControlMode, BattleEvent, CardPiles, CardTurnMemory, Hand,
        InBattle, PendingBoosts, PendingHandDiscard, PlayerTeam, PvpTurnOrder,
        ROUND_TRANSITION_SECONDS, RoundOrder, RoundTransition, SelectedCards, Side, SkillCount,
        SkillList, SkillUses, Stats, StructuredBattleLog, TurnContext, TurnCount, UiControlSide,
        note_round_phase, opposite_side, push_named_action_trace,
    },
    console_log::{ConsoleLogCategory, log as console_log},
    data::{BattleDbs, BattleFormulaRules, BattleRules, CardDeck},
    game_state::BattlePhase,
    pvp::{PvpConnection, PvpRole},
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
    card_piles: ResMut<'w, CardPiles>,
    turn_ctx: ResMut<'w, TurnContext>,
    turn_count: ResMut<'w, TurnCount>,
    round_order: ResMut<'w, RoundOrder>,
    round_transition: ResMut<'w, RoundTransition>,
    pending_boosts: ResMut<'w, PendingBoosts>,
    card_memory: ResMut<'w, CardTurnMemory>,
    selected: ResMut<'w, SelectedCards>,
    structured_log: ResMut<'w, StructuredBattleLog>,
    action_trace: ResMut<'w, ActionTrace>,
}

pub fn round_start_system(
    time: Res<Time>,
    card_deck: Res<CardDeck>,
    query: Query<&Stats>,
    mut skill_uses_q: Query<(&SkillList, &SkillCount, &mut SkillUses), With<InBattle>>,
    battle_mode: Res<BattleControlMode>,
    pvp_turn_order: Option<Res<PvpTurnOrder>>,
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
    let card_piles = &mut runtime.card_piles;
    let turn_ctx = &mut runtime.turn_ctx;
    let turn_count = &mut runtime.turn_count;
    let round_order = &mut runtime.round_order;
    let round_transition = &mut runtime.round_transition;
    let pending_boosts = &mut runtime.pending_boosts;
    let card_memory = &mut runtime.card_memory;
    let selected = &mut runtime.selected;
    let structured_log = &mut runtime.structured_log;
    let action_trace = &mut runtime.action_trace;

    if let Some(next) = round_transition.pending_next_phase {
        round_transition.timer.tick(time.delta());
        if round_transition.timer.is_finished() {
            round_transition.pending_next_phase = None;
            next_phase.set(next);
        }
        return;
    }

    if card_deck.0.is_empty() {
        return;
    }

    turn_count.0 += 1;
    event_writer.write(BattleEvent::TurnStarted(turn_count.0));

    super::cards::draw_cards(
        Side::Player,
        rules.cards_per_round,
        hand,
        card_piles,
        &card_deck,
    );
    super::cards::draw_cards(
        Side::Enemy,
        rules.cards_per_round,
        hand,
        card_piles,
        &card_deck,
    );

    let player_cards = hand_names(&hand.player, &dbs);
    let enemy_cards = hand_names(&hand.enemy, &dbs);
    console_log(
        ConsoleLogCategory::Cards,
        format!(
            "[round-r{}][player] 抽牌：{}；手牌 {} 张；牌堆 {} 张；弃牌 {} 张",
            turn_count.0,
            player_cards,
            hand.player.len(),
            card_piles.draw.len(),
            card_piles.discard.len()
        ),
    );
    console_log(
        ConsoleLogCategory::Cards,
        format!(
            "[round-r{}][enemy] 抽牌：{}；手牌 {} 张；牌堆 {} 张；弃牌 {} 张",
            turn_count.0,
            enemy_cards,
            hand.enemy.len(),
            card_piles.draw.len(),
            card_piles.discard.len()
        ),
    );

    super::cards::gain_ap(Side::Player, rules.ap_per_round, rules, action_points);
    super::cards::gain_ap(Side::Enemy, rules.ap_per_round, rules, action_points);

    turn_ctx.player_ended = false;
    turn_ctx.enemy_ended = false;
    turn_ctx.player_end_requested = false;
    turn_ctx.enemy_end_requested = false;
    super::clear_round_scoped_card_effects(pending_boosts);
    **card_memory = CardTurnMemory::default();
    **selected = SelectedCards::default();

    // 每个行动回合刷新所有参战精灵（含替补）的技能释放次数。每方每回合各行动一次，
    // 故在 RoundStart 统一重置等价于「行动回合结束后刷新」，且按精灵单独计数、互不共享。
    for (skills, count, mut uses) in &mut skill_uses_q {
        *uses = SkillUses::full(skills, *count, dbs, rules);
    }

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
    let first_side = if *battle_mode == BattleControlMode::PlayerVsRemote {
        let local_is_host = pvp_turn_order
            .as_ref()
            .is_some_and(|order| order.local_first);
        crate::pvp::pvp_host_first_side(
            player_spd,
            enemy_spd,
            local_is_host,
            round_order.previous_first,
        )
    } else if player_spd > enemy_spd {
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

    console_log(
        ConsoleLogCategory::Battle,
        format!(
            "[round {}] 先手={:?}；玩家Spd={}；敌方Spd={}；玩家AP={}；敌方AP={}；玩家手牌={}；敌方手牌={}",
            turn_count.0,
            round_order.first,
            player_spd,
            enemy_spd,
            action_points.player,
            action_points.enemy,
            hand.player.len(),
            hand.enemy.len()
        ),
    );

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

    let first_phase = match round_order.first {
        Side::Player => BattlePhase::PlayerTurn,
        Side::Enemy => BattlePhase::EnemyTurn,
    };
    round_transition.pending_next_phase = Some(first_phase);
    round_transition.timer = Timer::from_seconds(ROUND_TRANSITION_SECONDS, TimerMode::Once);
}

pub fn sync_ui_control_side_system(
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    pending_discard: Option<Res<PendingHandDiscard>>,
    pvp_connection: Option<Res<PvpConnection>>,
    mut ui_control_side: ResMut<UiControlSide>,
) {
    ui_control_side.0 = match *battle_phase.get() {
        BattlePhase::Discard => pending_discard
            .as_ref()
            .map(|pending| match *battle_mode {
                BattleControlMode::PlayerVsAi | BattleControlMode::PlayerVsRemote
                    if pending.side == Side::Enemy =>
                {
                    Side::Player
                }
                _ => pending.side,
            })
            .unwrap_or(Side::Player),
        BattlePhase::EnemyTurn if *battle_mode == BattleControlMode::DebugPlayerControlsBoth => {
            Side::Enemy
        }
        BattlePhase::EnemyTurn
            if *battle_mode == BattleControlMode::PlayerVsRemote
                && pvp_connection
                    .as_ref()
                    .is_some_and(|connection| connection.role == Some(PvpRole::Client)) =>
        {
            Side::Player
        }
        _ => Side::Player,
    };
}
