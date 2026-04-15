use bevy::prelude::*;

use crate::{
    battle::{
        ActionPoints, BattleEvent, Hand, InBattle, PendingBoosts, PlayerTeam, SkillList, Side,
        Stats, TurnContext,
    },
    game_state::BattlePhase,
};

use super::super::components::*;

#[derive(Resource, Default)]
pub(crate) struct SwitchOverlayOpen(pub bool);

pub(crate) fn button_toggle_switch_overlay_system(
    mut interaction_query: Query<&Interaction, (Changed<Interaction>, With<SwitchMonsterButton>)>,
    mut cancel_query: Query<&Interaction, (Changed<Interaction>, With<SwitchCancelButton>)>,
    mut overlay_q: Query<&mut Visibility, With<SwitchOverlayRoot>>,
    mut skill_panel_q: Query<&mut Visibility, (With<SkillPanelRoot>, Without<SwitchOverlayRoot>)>,
    mut hand_q: Query<&mut Visibility, (With<HandCardsRoot>, Without<SwitchOverlayRoot>, Without<SkillPanelRoot>)>,
    mut open: ResMut<SwitchOverlayOpen>,
) {
    for interaction in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            open.0 = true;
            for mut vis in &mut overlay_q {
                *vis = Visibility::Visible;
            }
            for mut vis in &mut skill_panel_q {
                *vis = Visibility::Hidden;
            }
            for mut vis in &mut hand_q {
                *vis = Visibility::Hidden;
            }
            return;
        }
    }

    for interaction in &mut cancel_query {
        if *interaction == Interaction::Pressed {
            open.0 = false;
            for mut vis in &mut overlay_q {
                *vis = Visibility::Hidden;
            }
            for mut vis in &mut skill_panel_q {
                *vis = Visibility::Visible;
            }
            for mut vis in &mut hand_q {
                *vis = Visibility::Visible;
            }
            return;
        }
    }
}

pub(crate) fn close_switch_overlay_on_switch_system(
    mut overlay_q: Query<&mut Visibility, With<SwitchOverlayRoot>>,
    mut skill_panel_q: Query<&mut Visibility, (With<SkillPanelRoot>, Without<SwitchOverlayRoot>)>,
    mut hand_q: Query<&mut Visibility, (With<HandCardsRoot>, Without<SwitchOverlayRoot>, Without<SkillPanelRoot>)>,
    mut open: ResMut<SwitchOverlayOpen>,
    mut switch_events: Query<&Interaction, (Changed<Interaction>, With<TeamMemberButton>)>,
) {
    for interaction in &mut switch_events {
        if *interaction == Interaction::Pressed {
            open.0 = false;
            for mut vis in &mut overlay_q {
                *vis = Visibility::Hidden;
            }
            for mut vis in &mut skill_panel_q {
                *vis = Visibility::Visible;
            }
            for mut vis in &mut hand_q {
                *vis = Visibility::Visible;
            }
            return;
        }
    }
}
use crate::battle::SelectedCard;

pub(crate) fn button_select_skill_system(
    mut interaction_query: Query<(&Interaction, &SkillButton), (Changed<Interaction>, With<Button>)>,
    mut turn_ctx: ResMut<TurnContext>,
    action_points: Res<ActionPoints>,
    player_team: Option<Res<PlayerTeam>>,
    query: Query<&SkillList, With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    if let Some(player_team) = player_team {
        if let Some(active_entity) = player_team.0.active_combatant() {
            let Ok(skills) = query.get(active_entity) else {
                return;
            };
            let skills = skills.0;

            for (interaction, button) in &mut interaction_query {
                if *interaction == Interaction::Pressed {
                    let cost = super::super::helpers::monster_skill_ap_cost_ui(button.index);
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
        if target_index >= player_team.0.combatants.len() || target_index == player_team.0.active_index
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
    card_db: Res<crate::data::CardDb>,
    mut event_writer: MessageWriter<BattleEvent>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
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

            let card_name = card_db
                .0
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
        let Some(card) = card_db.0.get(&card_id) else {
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
            turn_ctx.player_ended = true;
            next_phase.set(BattlePhase::EnemyTurn);
        }
        break;
    }
}

pub(crate) fn button_discard_system(
    mut interaction_query: Query<(&Interaction, &DiscardButton), (Changed<Interaction>, With<Button>)>,
    mut turn_ctx: ResMut<TurnContext>,
    mut selected: ResMut<SelectedCard>,
    mut action_points: ResMut<ActionPoints>,
    mut hand: ResMut<Hand>,
    card_db: Res<crate::data::CardDb>,
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
                    selected.discard_armed = true;
                    continue;
                }
            };

            let card_id = hand.player.remove(target_index);
            action_points.player += 1;

            let card_name = card_db
                .0
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
    mut interaction_query: Query<(&Interaction, &EndTurnButton), (Changed<Interaction>, With<Button>)>,
    mut turn_ctx: ResMut<TurnContext>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    for (interaction, _) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            turn_ctx.player_ended = true;
            turn_ctx.player_action = None;
            next_phase.set(BattlePhase::EnemyTurn);
            break;
        }
    }
}

