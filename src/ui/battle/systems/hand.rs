use bevy::prelude::*;

use crate::{
    battle::{Hand, SelectedCards, Side, UiControlSide},
    data::BattleDbs,
};

use super::super::components::*;

pub(crate) fn update_player_hand_ui_system(
    hand: Res<Hand>,
    dbs: Res<BattleDbs>,
    selected: Res<SelectedCards>,
    ui_control_side: Res<UiControlSide>,
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
    mut desc_vis_q: Query<(&PlayerCardDescText, &mut Visibility)>,
    mut card_nodes: Query<(&PlayerCardButton, &mut Node)>,
) {
    let active_hand = match ui_control_side.0 {
        Side::Player => &hand.player,
        Side::Enemy => &hand.enemy,
    };
    let active_selected = match ui_control_side.0 {
        Side::Player => selected.player,
        Side::Enemy => selected.enemy,
    };

    for (meta, mut node) in &mut card_nodes {
        let has_card = meta.index < active_hand.len();
        node.display = if has_card {
            Display::Flex
        } else {
            Display::None
        };
        let is_selected = active_selected.index == Some(meta.index);
        if is_selected {
            node.width = Val::Px(140.0);
            node.height = Val::Px(240.0);
            node.margin.top = Val::Px(0.0);
            node.margin.bottom = Val::Px(78.0);
        } else {
            node.width = Val::Px(130.0);
            node.height = Val::Px(180.0);
            node.margin.top = Val::Px(0.0);
            node.margin.bottom = Val::Px(0.0);
        }
    }

    for (mut text, is_hint, hotkey, name, cost, desc) in &mut card_text_q {
        if is_hint.is_some() {
            let side_label = match ui_control_side.0 {
                Side::Player => "我方",
                Side::Enemy => "敌方",
            };
            text.0 = format!(
                "当前控制：{}；按 1-4 使用精灵技能（未配置槽位无效）；按 5/6/7 切换当前队伍；手牌热键 Z/X/C/V/B 首按选中/再按出牌；F 弃选中牌换 AP；点击与快捷键可交叉使用；按 E 结束回合；按 R 重新开始",
                side_label
            );
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

    for (meta, mut vis) in &mut desc_vis_q {
        *vis = if active_selected.index == Some(meta.index) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
