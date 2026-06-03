use bevy::prelude::*;

use crate::{
    battle::{BattleControlMode, Hand, PendingHandDiscard, SelectedCards, Side, UiControlSide},
    data::BattleDbs,
    game_state::BattlePhase,
};

use super::super::components::*;

const HAND_CARDS_PER_LAYER: usize = 6;
const HAND_CARD_WIDTH: f32 = 130.0;
const HAND_CARD_GAP: f32 = 8.0;
const HAND_CARD_FULL_HEIGHT: f32 = 180.0;
const HAND_CARD_STACK_HEIGHT: f32 = 34.0;
const HAND_CARD_SELECTED_WIDTH: f32 = 142.0;
const HAND_CARD_SELECTED_HEIGHT: f32 = 240.0;

fn hand_card_layer(index: usize) -> usize {
    index / HAND_CARDS_PER_LAYER
}

fn hand_card_column(index: usize) -> usize {
    index % HAND_CARDS_PER_LAYER
}

fn hand_card_left(index: usize) -> f32 {
    hand_card_column(index) as f32 * (HAND_CARD_WIDTH + HAND_CARD_GAP)
}

fn hand_card_bottom(index: usize) -> f32 {
    let layer = hand_card_layer(index);
    if layer == 0 {
        0.0
    } else {
        HAND_CARD_FULL_HEIGHT + (layer - 1) as f32 * HAND_CARD_STACK_HEIGHT
    }
}

pub(crate) fn update_player_hand_ui_system(
    battle_phase: Res<State<BattlePhase>>,
    hand: Res<Hand>,
    dbs: Res<BattleDbs>,
    selected: Res<SelectedCards>,
    ui_control_side: Res<UiControlSide>,
    battle_mode: Res<BattleControlMode>,
    pending_discard: Option<Res<PendingHandDiscard>>,
    mut card_text_q: Query<
        (
            &mut Text,
            Option<&BattleHintText>,
            Option<&PlayerCardHotkeyText>,
            Option<&PlayerCardNameText>,
            Option<&PlayerCardCostText>,
            Option<&PlayerCardDescText>,
        ),
        Without<ResultText>,
    >,
    mut visibility_queries: ParamSet<(
        Query<(&PlayerCardDescText, &mut Visibility)>,
        Query<(&PlayerCardHotkeyBadge, &mut Visibility)>,
        Query<(&PlayerCardCostText, &mut Visibility)>,
    )>,
    mut card_nodes: Query<(&PlayerCardButton, &mut Node, &mut ZIndex)>,
) {
    let display_side = if *battle_phase.get() == BattlePhase::Discard
        && pending_discard
            .as_ref()
            .is_some_and(|pending| pending.side == Side::Enemy)
        && *battle_mode == BattleControlMode::PlayerVsAi
    {
        Side::Player
    } else {
        ui_control_side.0
    };

    let active_hand = match display_side {
        Side::Player => &hand.player,
        Side::Enemy => &hand.enemy,
    };
    let active_selected = match display_side {
        Side::Player => selected.player,
        Side::Enemy => selected.enemy,
    };

    for (meta, mut node, mut z_index) in &mut card_nodes {
        let has_card = meta.index < active_hand.len();
        node.display = if has_card {
            Display::Flex
        } else {
            Display::None
        };
        if !has_card {
            continue;
        }

        let is_selected = active_selected.index == Some(meta.index);
        let is_stacked = hand_card_layer(meta.index) > 0;
        node.left = Val::Px(hand_card_left(meta.index));
        node.bottom = Val::Px(if is_selected {
            58.0
        } else {
            hand_card_bottom(meta.index)
        });
        node.width = Val::Px(if is_selected {
            HAND_CARD_SELECTED_WIDTH
        } else {
            HAND_CARD_WIDTH
        });
        node.height = Val::Px(if is_selected {
            HAND_CARD_SELECTED_HEIGHT
        } else if is_stacked {
            HAND_CARD_STACK_HEIGHT
        } else {
            HAND_CARD_FULL_HEIGHT
        });
        node.padding = if is_selected || !is_stacked {
            UiRect::px(10.0, 10.0, 12.0, 10.0)
        } else {
            UiRect::px(8.0, 8.0, 4.0, 8.0)
        };
        node.row_gap = if is_selected || !is_stacked {
            Val::Px(4.0)
        } else {
            Val::Px(0.0)
        };
        *z_index = ZIndex(if is_selected {
            40
        } else {
            10 + hand_card_layer(meta.index) as i32
        });
    }

    for (mut text, is_hint, hotkey, name, cost, desc) in &mut card_text_q {
        if is_hint.is_some() {
            let side_label = match display_side {
                Side::Player => "我方",
                Side::Enemy => "敌方",
            };
            text.0 = if *battle_phase.get() == BattlePhase::Discard {
                if pending_discard
                    .as_ref()
                    .is_some_and(|pending| pending.side == Side::Enemy)
                    && *battle_mode == BattleControlMode::PlayerVsAi
                {
                    "弃牌阶段：敌方手牌超过上限，AI 正在自动弃牌。".to_string()
                } else {
                    format!(
                        "弃牌阶段：{}手牌超过上限，请点击手牌或按 Z/X/C/V/B/N/A/S/D/G/H/J/K/L/U/I/O/P 弃到上限；每弃1张获得 AP。",
                        side_label
                    )
                }
            } else {
                format!(
                    "当前控制：{}；按 1-4 使用精灵技能（未配置槽位无效）；按 5/6/7 切换当前队伍；手牌每层最多6张，超出会堆叠；快捷键 Z/X/C/V/B/N/A/S/D/G/H/J/K/L/U/I/O/P 或点击手牌可选中/弹出，再次触发则出牌；F 弃选中牌换 AP；按 E 结束回合；按 R 重新开始",
                    side_label
                )
            };
            continue;
        }
        let Some(idx) = hotkey
            .map(|m| m.index)
            .or_else(|| name.map(|m| m.index))
            .or_else(|| cost.map(|m| m.index))
            .or_else(|| desc.map(|m| m.index))
        else {
            continue;
        };

        if idx >= active_hand.len() {
            if name.is_some() {
                text.0 = "—".to_string();
            } else if cost.is_some() {
                text.0 = "AP—".to_string();
            } else if hotkey.is_some() {
                text.0 = super::super::helpers::card_hotkey_label(idx).to_string();
            } else if desc.is_some() {
                text.0.clear();
            }
            continue;
        }

        let card_id = active_hand[idx];
        let Some(card) = dbs.cards.get(&card_id) else {
            continue;
        };

        if hotkey.is_some() {
            text.0 = super::super::helpers::card_hotkey_label(idx).to_string();
        } else if name.is_some() {
            text.0 = card.name.to_string();
        } else if cost.is_some() {
            text.0 = format!("AP{}", card.cost_ap);
        } else if desc.is_some() {
            text.0 = super::super::helpers::card_description(card);
        }
    }

    for (meta, mut vis) in &mut visibility_queries.p1() {
        let is_selected = active_selected.index == Some(meta.index);
        let is_stacked = hand_card_layer(meta.index) > 0;
        *vis = if meta.index < active_hand.len() && (!is_stacked || is_selected) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (meta, mut vis) in &mut visibility_queries.p2() {
        let is_selected = active_selected.index == Some(meta.index);
        let is_stacked = hand_card_layer(meta.index) > 0;
        *vis = if meta.index < active_hand.len() && (!is_stacked || is_selected) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (meta, mut vis) in &mut visibility_queries.p0() {
        *vis = if active_selected.index == Some(meta.index) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
