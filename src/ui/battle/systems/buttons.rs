use bevy::prelude::*;

use crate::{
    battle::{
        ActionPoints, BattleEvent, Hand, InBattle, PendingBoosts, PlayerTeam, SelectedCard, Side,
        SkillCount, SkillList, Stats, TurnContext,
    },
    data::BattleDbs,
    game_state::{BattlePhase, GameState},
};

use super::super::components::*;

#[derive(Resource, Default)]
pub(crate) struct SwitchOverlayOpen(pub bool);

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

pub(crate) fn button_toggle_switch_overlay_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameState>>,
    battle_phase: Res<State<BattlePhase>>,
    mut interaction_query: Query<&Interaction, (Changed<Interaction>, With<SwitchMonsterButton>)>,
    mut cancel_query: Query<&Interaction, (Changed<Interaction>, With<SwitchCancelButton>)>,
    open: Res<SwitchOverlayOpen>,
    mut pending_toggle: ResMut<PendingSwitchOverlayToggle>,
) {
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

    if *game_state.get() == GameState::Battle
        && *battle_phase.get() == BattlePhase::PlayerTurn
        && keyboard.just_pressed(KeyCode::KeyQ)
    {
        queue_switch_overlay_toggle(&mut pending_toggle, !open.0);
    }
}

pub(crate) fn close_switch_overlay_on_switch_system(
    mut switch_events: Query<&Interaction, (Changed<Interaction>, With<TeamMemberButton>)>,
    mut pending_toggle: ResMut<PendingSwitchOverlayToggle>,
) {
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
    mut interaction_query: Query<
        (&Interaction, &SkillButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut turn_ctx: ResMut<TurnContext>,
    action_points: Res<ActionPoints>,
    player_team: Option<Res<PlayerTeam>>,
    query: Query<(&SkillList, &SkillCount), With<InBattle>>,
    battle_dbs: Res<BattleDbs>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    if let Some(player_team) = player_team {
        if let Some(active_entity) = player_team.0.active_combatant() {
            let Ok((skills, skill_count)) = query.get(active_entity) else {
                return;
            };
            let skills = skills.0;

            for (interaction, button) in &mut interaction_query {
                if *interaction == Interaction::Pressed {
                    if button.index >= skill_count.0 {
                        continue;
                    }
                    let skill_id = skills[button.index];
                    let cost =
                        super::super::helpers::monster_skill_ap_cost_ui(skill_id, &battle_dbs);
                    if action_points.player < cost {
                        continue;
                    }
                    turn_ctx.player_action =
                        Some(crate::battle::TurnAction::Skill(skills[button.index]));
                    // 保持在玩家回合内，由 `player_turn_input_system` 按 AP 规则执行。
                    next_phase.set(BattlePhase::PlayerTurn);
                }
            }
        }
    }
}

pub(crate) fn button_switch_member_system(
    mut interaction_query: Query<
        (&Interaction, &TeamMemberButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut action_points: ResMut<ActionPoints>,
    mut player_team: ResMut<PlayerTeam>,
    mut event_writer: MessageWriter<BattleEvent>,
    combat_query: Query<(&Stats, &Name), With<InBattle>>,
) {
    for (interaction, button) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let target_index = button.index;
        if action_points.player < 1 {
            continue;
        }
        if target_index >= player_team.0.combatants.len()
            || target_index == player_team.0.active_index
        {
            continue;
        }

        let target_entity = player_team.0.combatants[target_index];
        let Ok((stats, name)) = combat_query.get(target_entity) else {
            continue;
        };
        if stats.hp <= 0 {
            continue;
        }

        action_points.player -= 1;
        player_team.0.active_index = target_index;
        event_writer.write(BattleEvent::Switched {
            side: Side::Player,
            name: name.to_string(),
        });
    }
}

pub(crate) fn button_play_card_two_step_system(
    mut interaction_query: Query<
        (&Interaction, &PlayerCardButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut selected: ResMut<SelectedCard>,
    mut turn_ctx: ResMut<TurnContext>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    mut pending_boosts: ResMut<PendingBoosts>,
    dbs: Res<crate::data::BattleDbs>,
    mut event_writer: MessageWriter<BattleEvent>,
) {
    for (interaction, button) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = button.index;
        if idx >= hand.player.len() {
            selected.index = None;
            selected.discard_armed = false;
            continue;
        }

        if selected.discard_armed {
            let card_id = hand.player[idx];
            hand.player.remove(idx);
            action_points.player += 1;

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
            selected.index = None;
            selected.discard_armed = false;
            break;
        }

        if selected.index != Some(idx) {
            selected.index = Some(idx);
            continue;
        }

        let card_id = hand.player[idx];
        let Some(card) = dbs.cards.get(&card_id) else {
            continue;
        };
        if action_points.player < card.cost_ap {
            continue;
        }

        hand.player.remove(idx);
        action_points.player -= card.cost_ap;
        event_writer.write(BattleEvent::CardUsed {
            side: Side::Player,
            card_name: card.name.to_string(),
        });

        match card.effect {
            crate::data::CardEffect::GainAp { amount } => action_points.player += amount,
            crate::data::CardEffect::NextAttackBoost { amount } => {
                pending_boosts.player.next_attack_bonus += amount
            }
            crate::data::CardEffect::NextShieldBoost { amount } => {
                pending_boosts.player.next_shield_bonus += amount
            }
            crate::data::CardEffect::NextHealBoost { amount } => {
                pending_boosts.player.next_heal_bonus += amount
            }
        }

        turn_ctx.player_action = None;
        selected.index = None;
        selected.discard_armed = false;

        if action_points.player <= 0 {
            turn_ctx.player_end_requested = true;
        }
        break;
    }
}

pub(crate) fn button_discard_system(
    mut interaction_query: Query<
        (&Interaction, &DiscardButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut turn_ctx: ResMut<TurnContext>,
    mut selected: ResMut<SelectedCard>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    dbs: Res<crate::data::BattleDbs>,
    mut event_writer: MessageWriter<BattleEvent>,
) {
    for (interaction, _) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            if hand.player.is_empty() {
                selected.discard_armed = false;
                selected.index = None;
                continue;
            }

            let target_index = match selected.index {
                Some(idx) if idx < hand.player.len() => idx,
                _ => {
                    // 无已选牌：切换武装状态（再次点击取消）
                    selected.discard_armed = !selected.discard_armed;
                    continue;
                }
            };

            let card_id = hand.player.remove(target_index);
            action_points.player += 1;

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
            selected.index = None;
            selected.discard_armed = false;
            break;
        }
    }
}

pub(crate) fn button_end_turn_system(
    mut interaction_query: Query<
        (&Interaction, &EndTurnButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut turn_ctx: ResMut<TurnContext>,
) {
    for (interaction, _) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            turn_ctx.player_action = None;
            turn_ctx.player_end_requested = true;
            break;
        }
    }
}
