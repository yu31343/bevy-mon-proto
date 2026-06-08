use bevy::prelude::*;

use crate::{
    battle::{
        ActionPoints, BattleControlMode, BattleEvent, EnemyTeam, Hand, InBattle, PendingBoosts,
        PendingHandDiscard, PendingTacticalDiscard, PlayerTeam, SelectedCards, Side, SkillCount,
        SkillList, Stats, StatusBoard, TurnContext, UiControlSide, transfer_status_by_id,
    },
    data::BattleDbs,
    game_state::{BattlePhase, GameState},
    pvp,
};

use super::super::components::*;

#[derive(Resource, Default)]
pub(crate) struct SwitchOverlayOpen(pub bool);

#[derive(Resource, Default)]
pub(crate) struct RetreatConfirmState {
    pub armed: bool,
}

#[derive(Resource)]
pub(crate) struct PendingSwitchOverlayToggle {
    pub target: Option<bool>,
    pub timer: Timer,
}

impl Default for PendingSwitchOverlayToggle {
    fn default() -> Self {
        let mut timer = Timer::from_seconds(0.0, TimerMode::Once);
        timer.pause();
        Self {
            target: None,
            timer,
        }
    }
}

fn queue_switch_overlay_toggle(
    pending_toggle: &mut ResMut<PendingSwitchOverlayToggle>,
    visible: bool,
) {
    pending_toggle.target = Some(visible);
    pending_toggle.timer = Timer::from_seconds(0.12, TimerMode::Once);
}

fn is_controllable_phase(phase: BattlePhase, mode: BattleControlMode) -> bool {
    phase == BattlePhase::PlayerTurn
        || (phase == BattlePhase::EnemyTurn && mode == BattleControlMode::DebugPlayerControlsBoth)
}

fn waiting_for_pvp_snapshot(
    battle_mode: BattleControlMode,
    pending_intent: &Option<ResMut<pvp::PvpPendingLocalIntent>>,
) -> bool {
    battle_mode == BattleControlMode::PlayerVsRemote
        && pending_intent
            .as_ref()
            .is_some_and(|pending| pending.0.is_some())
}

fn pending_tactical_discard_for_side(
    pending: &Option<Res<PendingTacticalDiscard>>,
    side: Side,
) -> bool {
    pending.as_ref().is_some_and(|pending| pending.side == side)
}

fn send_client_intent(
    battle_mode: BattleControlMode,
    connection: &mut Option<ResMut<pvp::PvpConnection>>,
    pending_intent: &mut Option<ResMut<pvp::PvpPendingLocalIntent>>,
    intent: pvp::BattleIntent,
) -> bool {
    if battle_mode != BattleControlMode::PlayerVsRemote {
        return false;
    }
    let Some(connection) = connection.as_mut() else {
        return false;
    };
    if connection.role != Some(pvp::PvpRole::Client) {
        return false;
    }
    if let Some(pending_intent) = pending_intent.as_mut() {
        pvp::send_local_intent(connection, pending_intent, intent);
    }
    true
}

pub(crate) fn button_toggle_switch_overlay_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameState>>,
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    mut interaction_query: Query<&Interaction, (Changed<Interaction>, With<SwitchMonsterButton>)>,
    mut cancel_query: Query<&Interaction, (Changed<Interaction>, With<SwitchCancelButton>)>,
    open: Res<SwitchOverlayOpen>,
    mut pending_toggle: ResMut<PendingSwitchOverlayToggle>,
) {
    let controllable_phase = is_controllable_phase(*battle_phase.get(), *battle_mode);
    if !controllable_phase {
        return;
    }

    for interaction in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            queue_switch_overlay_toggle(&mut pending_toggle, true);
            return;
        }
    }

    for interaction in &mut cancel_query {
        if *interaction == Interaction::Pressed {
            queue_switch_overlay_toggle(&mut pending_toggle, false);
            return;
        }
    }

    let controllable_phase = *battle_phase.get() == BattlePhase::PlayerTurn
        || (*battle_phase.get() == BattlePhase::EnemyTurn
            && *battle_mode == BattleControlMode::DebugPlayerControlsBoth);
    if *game_state.get() == GameState::Battle
        && controllable_phase
        && keyboard.just_pressed(KeyCode::KeyQ)
    {
        queue_switch_overlay_toggle(&mut pending_toggle, !open.0);
    }
}

pub(crate) fn close_switch_overlay_on_switch_system(
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    mut switch_events: Query<&Interaction, (Changed<Interaction>, With<TeamMemberButton>)>,
    mut pending_toggle: ResMut<PendingSwitchOverlayToggle>,
) {
    if !is_controllable_phase(*battle_phase.get(), *battle_mode) {
        return;
    }

    for interaction in &mut switch_events {
        if *interaction == Interaction::Pressed {
            queue_switch_overlay_toggle(&mut pending_toggle, false);
            return;
        }
    }
}

pub(crate) fn apply_pending_switch_overlay_toggle_system(
    time: Res<Time>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<SwitchOverlayRoot>>,
        Query<&mut Node, (With<SkillPanelRoot>, Without<SwitchOverlayRoot>)>,
        Query<
            &mut Node,
            (
                With<HandCardsRoot>,
                Without<SwitchOverlayRoot>,
                Without<SkillPanelRoot>,
            ),
        >,
    )>,
    mut shield_tracks: Query<&mut Visibility, With<TeamMemberShieldBarTrack>>,
    mut open: ResMut<SwitchOverlayOpen>,
    mut pending_toggle: ResMut<PendingSwitchOverlayToggle>,
) {
    let Some(visible) = pending_toggle.target else {
        return;
    };

    pending_toggle.timer.tick(time.delta());
    if !pending_toggle.timer.just_finished() {
        return;
    }

    pending_toggle.target = None;
    open.0 = visible;

    for mut node in &mut nodes.p0() {
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut nodes.p1() {
        node.display = if visible {
            Display::None
        } else {
            Display::Flex
        };
    }
    for mut node in &mut nodes.p2() {
        node.display = if visible {
            Display::None
        } else {
            Display::Flex
        };
    }
    if !visible {
        for mut track_visibility in &mut shield_tracks {
            *track_visibility = Visibility::Hidden;
        }
    }
}

pub(crate) fn button_select_skill_system(
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    mut interaction_query: Query<
        (&Interaction, &SkillButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut turn_ctx: ResMut<TurnContext>,
    action_points: Res<ActionPoints>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    ui_control_side: Res<UiControlSide>,
    query: Query<(&SkillList, &SkillCount), With<InBattle>>,
    battle_dbs: Res<BattleDbs>,
    pvp_pending_intent: Option<ResMut<pvp::PvpPendingLocalIntent>>,
    pending_tactical_discard: Option<Res<PendingTacticalDiscard>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    if !is_controllable_phase(*battle_phase.get(), *battle_mode)
        || waiting_for_pvp_snapshot(*battle_mode, &pvp_pending_intent)
        || pending_tactical_discard_for_side(&pending_tactical_discard, ui_control_side.0)
    {
        return;
    }
    let active_entity = match ui_control_side.0 {
        Side::Player => player_team.and_then(|team| team.0.active_combatant()),
        Side::Enemy => enemy_team.and_then(|team| team.0.active_combatant()),
    };
    let Some(active_entity) = active_entity else {
        return;
    };
    let Ok((skills, skill_count)) = query.get(active_entity) else {
        return;
    };
    let skills = skills.0;

    for (interaction, button) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if button.index >= skill_count.0 {
            continue;
        }
        let skill_id = skills[button.index];
        let cost = super::super::helpers::monster_skill_ap_cost_ui(skill_id, &battle_dbs);
        let ap = match ui_control_side.0 {
            Side::Player => action_points.player,
            Side::Enemy => action_points.enemy,
        };
        if ap < cost {
            continue;
        }
        match ui_control_side.0 {
            Side::Player => {
                turn_ctx.player_action = Some(crate::battle::TurnAction::Skill(skill_id));
            }
            Side::Enemy => turn_ctx.enemy_action = Some(crate::battle::TurnAction::Skill(skill_id)),
        }
        next_phase.set(match ui_control_side.0 {
            Side::Player => BattlePhase::PlayerTurn,
            Side::Enemy => BattlePhase::EnemyTurn,
        });
    }
}

pub(crate) fn button_switch_member_system(
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    mut interaction_query: Query<
        (&Interaction, &TeamMemberButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut action_points: ResMut<ActionPoints>,
    player_team: Option<ResMut<PlayerTeam>>,
    enemy_team: Option<ResMut<EnemyTeam>>,
    ui_control_side: Res<UiControlSide>,
    pending_tactical_discard: Option<Res<PendingTacticalDiscard>>,
    mut pvp_connection: Option<ResMut<pvp::PvpConnection>>,
    mut pvp_pending_intent: Option<ResMut<pvp::PvpPendingLocalIntent>>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut combat_query: Query<(&mut Stats, &Name, &mut StatusBoard), With<InBattle>>,
) {
    if !is_controllable_phase(*battle_phase.get(), *battle_mode)
        || waiting_for_pvp_snapshot(*battle_mode, &pvp_pending_intent)
        || pending_tactical_discard_for_side(&pending_tactical_discard, ui_control_side.0)
    {
        return;
    }

    let mut player_team = player_team;
    let mut enemy_team = enemy_team;
    for (interaction, button) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let target_index = button.index;
        let (team, ap, side) = match ui_control_side.0 {
            Side::Player => {
                let Some(team) = player_team.as_mut() else {
                    return;
                };
                (&mut team.0, &mut action_points.player, Side::Player)
            }
            Side::Enemy => {
                let Some(team) = enemy_team.as_mut() else {
                    return;
                };
                (&mut team.0, &mut action_points.enemy, Side::Enemy)
            }
        };
        if *ap < 1 {
            continue;
        }
        if target_index >= team.combatants.len() || target_index == team.active_index {
            continue;
        }

        let current_entity = team.combatants[team.active_index];
        let target_entity = team.combatants[target_index];
        let Ok(
            [
                (mut current_stats, _, mut current_statuses),
                (target_stats, name, target_statuses),
            ],
        ) = combat_query.get_many_mut([current_entity, target_entity])
        else {
            continue;
        };
        if target_stats.hp <= 0 {
            continue;
        }

        if side == Side::Player
            && send_client_intent(
                *battle_mode,
                &mut pvp_connection,
                &mut pvp_pending_intent,
                pvp::BattleIntent::Switch { target_index },
            )
        {
            return;
        }
        transfer_status_by_id(
            &mut current_statuses,
            &mut current_stats,
            target_statuses.into_inner(),
            target_stats.into_inner(),
            "nature_regen",
        );
        *ap -= 1;
        team.active_index = target_index;
        event_writer.write(BattleEvent::Switched {
            side,
            name: name.to_string(),
        });
    }
}

pub(crate) fn button_cancel_card_selection_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    card_query: Query<(&Interaction, &PlayerCardButton), With<Button>>,
    ui_control_side: Res<UiControlSide>,
    pending_discard: Option<Res<PendingHandDiscard>>,
    pending_tactical_discard: Option<Res<PendingTacticalDiscard>>,
    mut selected: ResMut<SelectedCards>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Right)
        || pending_tactical_discard_for_side(&pending_tactical_discard, ui_control_side.0)
    {
        return;
    }
    if (!is_controllable_phase(*battle_phase.get(), *battle_mode)
        && *battle_phase.get() != BattlePhase::Discard)
        || (*battle_phase.get() == BattlePhase::Discard
            && pending_discard.as_ref().is_some_and(|pending| {
                pending.side != ui_control_side.0
                    || (*battle_mode == BattleControlMode::PlayerVsAi
                        && pending.side == Side::Enemy)
            }))
    {
        return;
    }

    let selected_state = match ui_control_side.0 {
        Side::Player => &mut selected.player,
        Side::Enemy => &mut selected.enemy,
    };
    let Some(selected_index) = selected_state.index else {
        return;
    };

    for (interaction, button) in &card_query {
        if button.index == selected_index
            && matches!(*interaction, Interaction::Hovered | Interaction::Pressed)
        {
            selected_state.index = None;
            selected_state.discard_armed = false;
            break;
        }
    }
}

pub(crate) fn button_play_card_two_step_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    mut interaction_query: Query<
        (&Interaction, &PlayerCardButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut selected: ResMut<SelectedCards>,
    mut turn_ctx: ResMut<TurnContext>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    mut pending_boosts: ResMut<PendingBoosts>,
    ui_control_side: Res<UiControlSide>,
    mut pvp_connection: Option<ResMut<pvp::PvpConnection>>,
    mut pvp_pending_intent: Option<ResMut<pvp::PvpPendingLocalIntent>>,
    dbs: Res<crate::data::BattleDbs>,
    pending_discard: Option<Res<PendingHandDiscard>>,
    pending_tactical_discard: Option<Res<PendingTacticalDiscard>>,
    mut event_writer: MessageWriter<BattleEvent>,
) {
    if !mouse_buttons.pressed(MouseButton::Left)
        || (!is_controllable_phase(*battle_phase.get(), *battle_mode)
            && *battle_phase.get() != BattlePhase::Discard)
        || waiting_for_pvp_snapshot(*battle_mode, &pvp_pending_intent)
    {
        return;
    }
    if *battle_phase.get() == BattlePhase::Discard {
        let Some(pending) = pending_discard.as_ref() else {
            return;
        };
        if pending.side != ui_control_side.0
            || (*battle_mode == BattleControlMode::PlayerVsAi && pending.side == Side::Enemy)
        {
            return;
        }
    }
    for (interaction, button) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = button.index;
        match ui_control_side.0 {
            Side::Player => {
                let cards = &mut hand.player;
                let ap = &mut action_points.player;
                let boosts = &mut pending_boosts.player;
                let selected_state = &mut selected.player;
                if idx >= cards.len() {
                    selected_state.index = None;
                    selected_state.discard_armed = false;
                    continue;
                }
                if pending_tactical_discard_for_side(&pending_tactical_discard, Side::Player) {
                    if send_client_intent(
                        *battle_mode,
                        &mut pvp_connection,
                        &mut pvp_pending_intent,
                        pvp::BattleIntent::DiscardCard { card_index: idx },
                    ) {
                        turn_ctx.player_action = None;
                        selected_state.index = None;
                        selected_state.discard_armed = false;
                        break;
                    }
                    let card_id = cards.remove(idx);
                    *ap += 1;
                    let card_name = dbs
                        .cards
                        .get(&card_id)
                        .map(|c| c.name.to_string())
                        .unwrap_or_else(|| format!("{card_id:?}"));
                    event_writer.write(BattleEvent::CardDiscarded {
                        side: Side::Player,
                        card_name,
                    });
                    turn_ctx.player_action = None;
                    selected_state.index = None;
                    selected_state.discard_armed = false;
                    break;
                }
                if *battle_phase.get() == BattlePhase::Discard {
                    let card_id = cards.remove(idx);
                    *ap += 1;
                    let card_name = dbs
                        .cards
                        .get(&card_id)
                        .map(|c| c.name.to_string())
                        .unwrap_or_else(|| format!("{card_id:?}"));
                    event_writer.write(BattleEvent::CardDiscarded {
                        side: Side::Player,
                        card_name,
                    });
                    turn_ctx.player_action = None;
                    selected_state.index = None;
                    selected_state.discard_armed = false;
                    break;
                }
                if selected_state.discard_armed {
                    if send_client_intent(
                        *battle_mode,
                        &mut pvp_connection,
                        &mut pvp_pending_intent,
                        pvp::BattleIntent::DiscardCard { card_index: idx },
                    ) {
                        turn_ctx.player_action = None;
                        selected_state.index = None;
                        selected_state.discard_armed = false;
                        break;
                    }
                    let card_id = cards[idx];
                    cards.remove(idx);
                    *ap += 1;
                    let card_name = dbs
                        .cards
                        .get(&card_id)
                        .map(|c| c.name.to_string())
                        .unwrap_or_else(|| format!("{card_id:?}"));
                    event_writer.write(BattleEvent::CardDiscarded {
                        side: Side::Player,
                        card_name,
                    });
                    turn_ctx.player_action = None;
                    selected_state.index = None;
                    selected_state.discard_armed = false;
                    break;
                }
                if selected_state.index != Some(idx) {
                    selected_state.index = Some(idx);
                    continue;
                }
                let card_id = cards[idx];
                let Some(card) = dbs.cards.get(&card_id) else {
                    continue;
                };
                if *ap < card.cost_ap {
                    continue;
                }
                if send_client_intent(
                    *battle_mode,
                    &mut pvp_connection,
                    &mut pvp_pending_intent,
                    pvp::BattleIntent::UseCard { card_index: idx },
                ) {
                    turn_ctx.player_action = None;
                    selected_state.index = None;
                    selected_state.discard_armed = false;
                    break;
                }
                cards.remove(idx);
                *ap -= card.cost_ap;
                event_writer.write(BattleEvent::CardUsed {
                    side: Side::Player,
                    card_name: card.name.to_string(),
                });
                let _ = boosts;
                turn_ctx.player_action = None;
                selected_state.index = None;
                selected_state.discard_armed = false;
            }
            Side::Enemy => {
                let cards = &mut hand.enemy;
                let ap = &mut action_points.enemy;
                let boosts = &mut pending_boosts.enemy;
                let selected_state = &mut selected.enemy;
                if idx >= cards.len() {
                    selected_state.index = None;
                    selected_state.discard_armed = false;
                    continue;
                }
                if pending_tactical_discard_for_side(&pending_tactical_discard, Side::Enemy) {
                    let card_id = cards.remove(idx);
                    *ap += 1;
                    let card_name = dbs
                        .cards
                        .get(&card_id)
                        .map(|c| c.name.to_string())
                        .unwrap_or_else(|| format!("{card_id:?}"));
                    event_writer.write(BattleEvent::CardDiscarded {
                        side: Side::Enemy,
                        card_name,
                    });
                    turn_ctx.enemy_action = None;
                    selected_state.index = None;
                    selected_state.discard_armed = false;
                    break;
                }
                if *battle_phase.get() == BattlePhase::Discard {
                    let card_id = cards.remove(idx);
                    *ap += 1;
                    let card_name = dbs
                        .cards
                        .get(&card_id)
                        .map(|c| c.name.to_string())
                        .unwrap_or_else(|| format!("{card_id:?}"));
                    event_writer.write(BattleEvent::CardDiscarded {
                        side: Side::Enemy,
                        card_name,
                    });
                    turn_ctx.enemy_action = None;
                    selected_state.index = None;
                    selected_state.discard_armed = false;
                    break;
                }
                if selected_state.discard_armed {
                    let card_id = cards[idx];
                    cards.remove(idx);
                    *ap += 1;
                    let card_name = dbs
                        .cards
                        .get(&card_id)
                        .map(|c| c.name.to_string())
                        .unwrap_or_else(|| format!("{card_id:?}"));
                    event_writer.write(BattleEvent::CardDiscarded {
                        side: Side::Enemy,
                        card_name,
                    });
                    turn_ctx.enemy_action = None;
                    selected_state.index = None;
                    selected_state.discard_armed = false;
                    break;
                }
                if selected_state.index != Some(idx) {
                    selected_state.index = Some(idx);
                    continue;
                }
                let card_id = cards[idx];
                let Some(card) = dbs.cards.get(&card_id) else {
                    continue;
                };
                if *ap < card.cost_ap {
                    continue;
                }
                cards.remove(idx);
                *ap -= card.cost_ap;
                event_writer.write(BattleEvent::CardUsed {
                    side: Side::Enemy,
                    card_name: card.name.to_string(),
                });
                let _ = boosts;
                turn_ctx.enemy_action = None;
                selected_state.index = None;
                selected_state.discard_armed = false;
            }
        }
        break;
    }
}

pub(crate) fn button_discard_system(
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    mut interaction_query: Query<
        (&Interaction, &DiscardButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut turn_ctx: ResMut<TurnContext>,
    mut selected: ResMut<SelectedCards>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    ui_control_side: Res<UiControlSide>,
    mut pvp_connection: Option<ResMut<pvp::PvpConnection>>,
    mut pvp_pending_intent: Option<ResMut<pvp::PvpPendingLocalIntent>>,
    dbs: Res<crate::data::BattleDbs>,
    pending_discard: Option<Res<PendingHandDiscard>>,
    pending_tactical_discard: Option<Res<PendingTacticalDiscard>>,
    mut event_writer: MessageWriter<BattleEvent>,
) {
    if (!is_controllable_phase(*battle_phase.get(), *battle_mode)
        && *battle_phase.get() != BattlePhase::Discard)
        || waiting_for_pvp_snapshot(*battle_mode, &pvp_pending_intent)
        || pending_tactical_discard_for_side(&pending_tactical_discard, ui_control_side.0)
    {
        return;
    }
    if *battle_phase.get() == BattlePhase::Discard {
        let Some(pending) = pending_discard.as_ref() else {
            return;
        };
        if pending.side != ui_control_side.0
            || (*battle_mode == BattleControlMode::PlayerVsAi && pending.side == Side::Enemy)
        {
            return;
        }
    }
    for (interaction, _) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match ui_control_side.0 {
            Side::Player => {
                let cards = &mut hand.player;
                let ap = &mut action_points.player;
                let selected_state = &mut selected.player;
                if cards.is_empty() {
                    selected_state.discard_armed = false;
                    selected_state.index = None;
                    continue;
                }
                let target_index = match selected_state.index {
                    Some(idx) if idx < cards.len() => idx,
                    _ => {
                        selected_state.discard_armed = !selected_state.discard_armed;
                        continue;
                    }
                };
                if send_client_intent(
                    *battle_mode,
                    &mut pvp_connection,
                    &mut pvp_pending_intent,
                    pvp::BattleIntent::DiscardCard {
                        card_index: target_index,
                    },
                ) {
                    turn_ctx.player_action = None;
                    selected_state.index = None;
                    selected_state.discard_armed = false;
                    break;
                }
                let card_id = cards.remove(target_index);
                *ap += 1;
                let card_name = dbs
                    .cards
                    .get(&card_id)
                    .map(|c| c.name.to_string())
                    .unwrap_or_else(|| format!("{card_id:?}"));
                event_writer.write(BattleEvent::CardDiscarded {
                    side: Side::Player,
                    card_name,
                });
                turn_ctx.player_action = None;
                selected_state.index = None;
                selected_state.discard_armed = false;
            }
            Side::Enemy => {
                let cards = &mut hand.enemy;
                let ap = &mut action_points.enemy;
                let selected_state = &mut selected.enemy;
                if cards.is_empty() {
                    selected_state.discard_armed = false;
                    selected_state.index = None;
                    continue;
                }
                let target_index = match selected_state.index {
                    Some(idx) if idx < cards.len() => idx,
                    _ => {
                        selected_state.discard_armed = !selected_state.discard_armed;
                        continue;
                    }
                };
                let card_id = cards.remove(target_index);
                *ap += 1;
                let card_name = dbs
                    .cards
                    .get(&card_id)
                    .map(|c| c.name.to_string())
                    .unwrap_or_else(|| format!("{card_id:?}"));
                event_writer.write(BattleEvent::CardDiscarded {
                    side: Side::Enemy,
                    card_name,
                });
                turn_ctx.enemy_action = None;
                selected_state.index = None;
                selected_state.discard_armed = false;
            }
        }
        break;
    }
}

pub(crate) fn button_end_turn_system(
    battle_phase: Res<State<BattlePhase>>,
    battle_mode: Res<BattleControlMode>,
    mut interaction_query: Query<
        (&Interaction, &EndTurnButton),
        (Changed<Interaction>, With<Button>),
    >,
    ui_control_side: Res<UiControlSide>,
    pending_tactical_discard: Option<Res<PendingTacticalDiscard>>,
    mut turn_ctx: ResMut<TurnContext>,
    mut pvp_connection: Option<ResMut<pvp::PvpConnection>>,
    mut pvp_pending_intent: Option<ResMut<pvp::PvpPendingLocalIntent>>,
) {
    if !is_controllable_phase(*battle_phase.get(), *battle_mode)
        || waiting_for_pvp_snapshot(*battle_mode, &pvp_pending_intent)
        || pending_tactical_discard_for_side(&pending_tactical_discard, ui_control_side.0)
    {
        return;
    }
    for (interaction, _) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match ui_control_side.0 {
            Side::Player => {
                turn_ctx.player_action = None;
                turn_ctx.player_end_requested = !send_client_intent(
                    *battle_mode,
                    &mut pvp_connection,
                    &mut pvp_pending_intent,
                    pvp::BattleIntent::EndTurn,
                );
            }
            Side::Enemy => {
                turn_ctx.enemy_action = None;
                turn_ctx.enemy_end_requested = true;
            }
        }
        break;
    }
}

pub(crate) fn button_retreat_system(
    mut interaction_query: Query<
        (&Interaction, &RetreatButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut retreat_confirm: ResMut<RetreatConfirmState>,
    mut retreat_button_text_q: Query<&mut Text, With<RetreatButtonText>>,
    battle_mode: Res<BattleControlMode>,
    pvp_connection: Option<Res<pvp::PvpConnection>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    for (interaction, _) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            let Ok(mut retreat_text) = retreat_button_text_q.single_mut() else {
                return;
            };

            if !retreat_confirm.armed {
                retreat_confirm.armed = true;
                retreat_text.0 = "确认撤退".to_string();
                return;
            }

            retreat_confirm.armed = false;
            retreat_text.0 = "撤退".to_string();
            if *battle_mode == BattleControlMode::PlayerVsRemote {
                if let Some(connection) = pvp_connection.as_ref() {
                    pvp::surrender(connection);
                }
            }
            next_phase.set(BattlePhase::Init);
            next_game_state.set(GameState::Lobby);
            break;
        }
    }
}
