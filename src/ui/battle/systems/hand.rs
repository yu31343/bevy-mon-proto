use bevy::prelude::*;
use std::ops::Range;

use crate::{
    battle::{
        ActionPoints, BattleControlMode, BattleEvent, Hand, PendingHandDiscard, SelectedCards,
        Side, UiControlSide,
    },
    data::{BattleDbs, CardId},
    game_state::BattlePhase,
};

use super::super::components::*;
use super::super::resources::UiFontHandle;
use super::super::theme::UiTheme;

const HAND_CARDS_PER_LAYER: usize = 6;
const HAND_CARD_WIDTH: f32 = 130.0;
const HAND_CARD_GAP: f32 = 8.0;
const HAND_CARD_FULL_HEIGHT: f32 = 180.0;
const HAND_CARD_STACK_HEIGHT: f32 = 34.0;
const HAND_CARD_SELECTED_WIDTH: f32 = 142.0;
const HAND_CARD_SELECTED_HEIGHT: f32 = 240.0;
const HAND_CARD_NAME_FONT_SIZE: f32 = 16.0;
const HAND_CARD_STACK_NAME_FONT_SIZE: f32 = 14.0;
const HAND_CARD_GLOW_OUTSET: f32 = -4.0;
const HAND_CARD_GLOW_BORDER: f32 = 3.0;
const HAND_CARD_GLOW_RADIUS: f32 = 18.0;
const HAND_CARD_STACK_GLOW_BORDER: f32 = 2.0;

/// AP 足够时可出牌的发光描边——饱和亮金，与暗化禁用环拉开强对比。
const CARD_PLAYABLE_GLOW: Color = Color::srgba(1.0, 0.86, 0.42, 0.98);
/// AP 不足时不可出牌的禁用描边——冷暗灰褐，明确传达“不可用”。
const CARD_UNPLAYABLE_GLOW: Color = Color::srgba(0.30, 0.27, 0.30, 0.82);

fn hand_card_layer(index: usize) -> usize {
    index / HAND_CARDS_PER_LAYER
}

fn hand_card_column(index: usize) -> usize {
    index % HAND_CARDS_PER_LAYER
}

fn hand_layer_card_count(hand_len: usize, layer: usize) -> usize {
    hand_len
        .saturating_sub(layer * HAND_CARDS_PER_LAYER)
        .min(HAND_CARDS_PER_LAYER)
}

fn hand_row_width(card_count: usize) -> f32 {
    if card_count == 0 {
        0.0
    } else {
        card_count as f32 * HAND_CARD_WIDTH + (card_count - 1) as f32 * HAND_CARD_GAP
    }
}

fn hand_card_center_offset(index: usize, hand_len: usize) -> f32 {
    let layer = hand_card_layer(index);
    let row_width = hand_row_width(hand_layer_card_count(hand_len, layer));
    hand_card_column(index) as f32 * (HAND_CARD_WIDTH + HAND_CARD_GAP) - row_width * 0.5
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
    action_points: Res<ActionPoints>,
    theme: Res<UiTheme>,
    pending_discard: Option<Res<PendingHandDiscard>>,
    mut card_text_q: Query<
        (
            &mut Text,
            Option<&mut TextFont>,
            Option<&BattleHintText>,
            Option<&PlayerCardHotkeyText>,
            Option<&PlayerCardNameText>,
            Option<&PlayerCardCostText>,
            Option<&PlayerCardDescText>,
            Option<&CardCategoryLabel>,
        ),
        Without<ResultText>,
    >,
    mut visibility_queries: ParamSet<(
        Query<(&PlayerCardDescText, &mut Visibility)>,
        Query<(&PlayerCardHotkeyBadge, &mut Visibility)>,
        Query<(&PlayerCardCostText, &mut Visibility)>,
    )>,
    mut node_queries: ParamSet<(
        Query<(&PlayerCardButton, &mut Node, &mut ZIndex)>,
        Query<(&PlayerCardTopRow, &mut Node)>,
        Query<(&PlayerCardDivider, &mut Node)>,
        Query<(&PlayerCardNameText, &mut Node)>,
        Query<(&PlayerCardDescText, &mut Node)>,
        Query<(&CardGlow, &mut Node, &mut BorderColor)>,
    )>,
    mut card_band_q: Query<(&CardCategoryBand, &mut BackgroundColor)>,
) {
    let display_side = if *battle_phase.get() == BattlePhase::Discard
        && pending_discard
            .as_ref()
            .is_some_and(|pending| pending.side == Side::Enemy)
        && matches!(
            *battle_mode,
            BattleControlMode::PlayerVsAi | BattleControlMode::PlayerVsRemote
        ) {
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

    for (meta, mut node, mut z_index) in &mut node_queries.p0() {
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
        node.left = Val::Percent(50.0);
        node.margin.left = Val::Px(hand_card_center_offset(meta.index, active_hand.len()));
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
            UiRect::px(8.0, 8.0, 6.0, 6.0)
        };
        node.row_gap = if is_selected || !is_stacked {
            Val::Px(4.0)
        } else {
            Val::Px(0.0)
        };
        node.justify_content = if is_stacked && !is_selected {
            JustifyContent::Center
        } else {
            JustifyContent::FlexStart
        };
        node.align_items = if is_stacked && !is_selected {
            AlignItems::Center
        } else {
            AlignItems::Stretch
        };
        node.overflow = if is_stacked && !is_selected {
            Overflow::clip()
        } else {
            Overflow::visible()
        };
        *z_index = ZIndex(if is_selected {
            40
        } else {
            10 + hand_card_layer(meta.index) as i32
        });
    }

    for (mut text, text_font, is_hint, hotkey, name, cost, desc, cat) in &mut card_text_q {
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
                } else if pending_discard
                    .as_ref()
                    .is_some_and(|pending| pending.side == Side::Enemy)
                    && *battle_mode == BattleControlMode::PlayerVsRemote
                {
                    "弃牌阶段：对方手牌超过上限，等待对方弃牌。".to_string()
                } else {
                    format!(
                        "弃牌阶段：{}手牌超过上限，请点击手牌或按 Z/X/C/V/B/N/A/S/D/G/H/J/K/L/U/I/O/P 弃到上限；每弃1张获得 AP。",
                        side_label
                    )
                }
            } else {
                format!(
                    "当前控制：{}；按 1-4 使用精灵技能（未配置槽位无效）；按 5/6/7 切换当前队伍；手牌每层最多6张，超出会堆叠；快捷键 Z/X/C/V/B/N/A/S/D/G/H/J/K/L/U/I/O/P 或点击手牌可选中/弹出，再次触发则出牌；右键已弹出手牌可收回；F 弃选中牌换 AP；按 E 结束回合；按 R 重新开始",
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
            .or_else(|| cat.map(|m| m.index))
        else {
            continue;
        };

        if idx >= active_hand.len() {
            if name.is_some() {
                text.0 = "—".to_string();
            } else if cost.is_some() {
                text.0 = "—".to_string();
            } else if hotkey.is_some() {
                text.0 = super::super::helpers::card_hotkey_label(idx).to_string();
            } else if desc.is_some() || cat.is_some() {
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
            if let Some(mut font) = text_font {
                font.font_size = if hand_card_layer(idx) > 0 && active_selected.index != Some(idx) {
                    HAND_CARD_STACK_NAME_FONT_SIZE
                } else {
                    HAND_CARD_NAME_FONT_SIZE
                };
            }
            text.0 = card.name.to_string();
        } else if cost.is_some() {
            text.0 = card.cost_ap.to_string();
        } else if desc.is_some() {
            text.0 = super::super::helpers::card_description(card);
        } else if cat.is_some() {
            text.0 = super::super::helpers::card_category_label(card).to_string();
        }
    }

    for (meta, mut node) in &mut node_queries.p1() {
        let is_selected = active_selected.index == Some(meta.index);
        let is_stacked = hand_card_layer(meta.index) > 0;
        node.display = if meta.index < active_hand.len() && (!is_stacked || is_selected) {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (meta, mut node) in &mut node_queries.p2() {
        let is_selected = active_selected.index == Some(meta.index);
        let is_stacked = hand_card_layer(meta.index) > 0;
        node.display = if meta.index < active_hand.len() && (!is_stacked || is_selected) {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (meta, mut node) in &mut node_queries.p3() {
        let is_selected = active_selected.index == Some(meta.index);
        let is_stacked = hand_card_layer(meta.index) > 0;
        node.display = if meta.index < active_hand.len() {
            Display::Flex
        } else {
            Display::None
        };
        node.width = Val::Percent(100.0);
        node.overflow = if is_stacked && !is_selected {
            Overflow::clip()
        } else {
            Overflow::visible()
        };
    }

    for (meta, mut node) in &mut node_queries.p4() {
        let is_selected = active_selected.index == Some(meta.index);
        node.display = if meta.index < active_hand.len() && is_selected {
            Display::Flex
        } else {
            Display::None
        };
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

    // 类别色带：按卡牌效果类别着色（空槽透明）。
    for (band, mut bg) in &mut card_band_q {
        *bg = if band.index < active_hand.len() {
            dbs.cards
                .get(&active_hand[band.index])
                .map(|card| {
                    BackgroundColor(super::super::helpers::card_category_color(card, &theme))
                })
                .unwrap_or(BackgroundColor(Color::NONE))
        } else {
            BackgroundColor(Color::NONE)
        };
    }

    // 可出牌发光环：AP 足够时点亮饱和亮金光环，不足时显示暗灰褐禁用环，
    // 两种状态描边颜色与饱和度均明显不同，便于一眼区分能否出牌。
    let current_ap = match display_side {
        Side::Player => action_points.player,
        Side::Enemy => action_points.enemy,
    };
    for (glow, mut node, mut border) in &mut node_queries.p5() {
        let is_selected = active_selected.index == Some(glow.index);
        let is_stacked = hand_card_layer(glow.index) > 0;
        if is_stacked && !is_selected {
            node.top = Val::Px(0.0);
            node.left = Val::Px(0.0);
            node.right = Val::Px(0.0);
            node.bottom = Val::Px(0.0);
            node.border = UiRect::all(Val::Px(HAND_CARD_STACK_GLOW_BORDER));
            node.border_radius = BorderRadius::all(theme.radius_card);
        } else {
            node.top = Val::Px(HAND_CARD_GLOW_OUTSET);
            node.left = Val::Px(HAND_CARD_GLOW_OUTSET);
            node.right = Val::Px(HAND_CARD_GLOW_OUTSET);
            node.bottom = Val::Px(HAND_CARD_GLOW_OUTSET);
            node.border = UiRect::all(Val::Px(HAND_CARD_GLOW_BORDER));
            node.border_radius = BorderRadius::all(Val::Px(HAND_CARD_GLOW_RADIUS));
        }

        let lit = glow.index < active_hand.len()
            && dbs
                .cards
                .get(&active_hand[glow.index])
                .is_some_and(|card| current_ap >= card.cost_ap);
        *border = if lit {
            BorderColor::all(CARD_PLAYABLE_GLOW)
        } else {
            BorderColor::all(CARD_UNPLAYABLE_GLOW)
        };
    }
}

// ============================================================================
// 手牌进入动画（摸牌 / 技能牌抽牌等所有获牌渠道）
//
// 参考常见卡牌游戏：新牌从右下角"牌堆"方向飞入对应卡槽，并带缩放放大 +
// 回弹过冲（ease-out-back），多张同时入手时按先后错峰，形成依次插入卡槽的发牌感。
//
// 实现要点：手牌槽是 18 个固定 `PlayerCardButton`，其 `Node` 位置每帧由
// `update_player_hand_ui_system` 改写；本动画只改写各卡的 `UiTransform`
// （平移 / 缩放 / 旋转的相对偏移），与布局系统互不争用。卡牌效果抽牌优先使用
// `CardsDrawn` 事件定位新增末尾槽位，其他获牌渠道用手牌快照差分兜底。
// ============================================================================

/// 单张卡入手动画总时长（秒）。
const CARD_DRAW_ANIM_DURATION: f32 = 0.30;
/// 同批多张牌之间的错峰间隔（秒），形成依次发牌感。
const CARD_DRAW_ANIM_STAGGER: f32 = 0.07;
/// 起始相对偏移（像素）：正 x 向右、正 y 向下，模拟从右下角牌堆飞入卡槽。
const CARD_DRAW_ANIM_START_X: f32 = 300.0;
const CARD_DRAW_ANIM_START_Y: f32 = 170.0;
/// 起始缩放（飞入时偏小，落位放大到 1.0）。
const CARD_DRAW_ANIM_START_SCALE: f32 = 0.55;
/// 起始旋转弧度（轻微倾斜，落位回正），约 7°。
const CARD_DRAW_ANIM_START_ROT: f32 = 0.12;

/// 追踪上一帧"展示侧手牌"的阵营与快照，用于检测新增的末尾卡槽。
#[derive(Resource, Default)]
pub(crate) struct HandDrawAnimTracker {
    last_side: Option<Side>,
    last_hand: Vec<CardId>,
}

/// 挂在正在播放入手动画的 `PlayerCardButton` 上的动画状态。
#[derive(Component)]
pub(crate) struct CardDrawAnim {
    elapsed: f32,
    delay: f32,
    duration: f32,
}

impl CardDrawAnim {
    /// `base_delay` 用于等待同批离场动画先播放；`order` 为该牌在本批新牌中的序号。
    fn new(order: usize, base_delay: f32) -> Self {
        Self {
            elapsed: 0.0,
            delay: base_delay + order as f32 * CARD_DRAW_ANIM_STAGGER,
            duration: CARD_DRAW_ANIM_DURATION,
        }
    }
}

/// ease-out-back：末段轻微过冲再回落，赋予"啪地嵌入卡槽"的手感。t∈[0,1]，f(0)=0，f(1)=1。
fn ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    let p = t - 1.0;
    1.0 + C3 * p * p * p + C1 * p * p
}

/// 按进度 `t`（0=牌堆起点，1=落位）写入卡牌的相对位移 / 缩放 / 旋转。
fn apply_card_draw_pose(transform: &mut UiTransform, t: f32) {
    let e = ease_out_back(t);
    let remain = 1.0 - e;
    transform.translation = Val2::px(
        CARD_DRAW_ANIM_START_X * remain,
        CARD_DRAW_ANIM_START_Y * remain,
    );
    transform.scale =
        Vec2::splat(CARD_DRAW_ANIM_START_SCALE + (1.0 - CARD_DRAW_ANIM_START_SCALE) * e);
    transform.rotation = Rot2::radians(CARD_DRAW_ANIM_START_ROT * remain);
}

fn detect_drawn_card_range(
    old: &[CardId],
    current: &[CardId],
    reported_draw_count: usize,
) -> Option<Range<usize>> {
    if current.is_empty() {
        return None;
    }

    if reported_draw_count > 0 {
        let count = reported_draw_count.min(current.len());
        return Some(current.len() - count..current.len());
    }

    let mut old_index = 0;
    let mut matched_current = 0;
    for card in current {
        let Some(offset) = old
            .get(old_index..)
            .and_then(|remaining| remaining.iter().position(|old_card| old_card == card))
        else {
            break;
        };
        old_index += offset + 1;
        matched_current += 1;
    }

    (matched_current < current.len()).then_some(matched_current..current.len())
}

fn card_draw_base_delay(has_used: bool, has_discarded: bool) -> f32 {
    let mut delay = 0.0;
    if has_used {
        delay += CARD_EXIT_ANIM_DURATION;
    }
    if has_discarded {
        delay += CARD_EXIT_ANIM_DURATION;
    }
    delay
}

/// 进入战斗时重置追踪基线，使开局起手牌也能播放飞入动画。
pub(crate) fn reset_hand_draw_anim_tracker(
    mut commands: Commands,
    mut draw_tracker: ResMut<HandDrawAnimTracker>,
    mut exit_tracker: ResMut<HandExitAnimTracker>,
    mut battle_events: ResMut<Messages<BattleEvent>>,
    mut ui_notices: ResMut<Messages<BattleUiNotice>>,
    mut action_text_q: Query<(&mut Text, &mut Visibility, &mut BattleActionText)>,
    card_exit_fx_q: Query<(Entity, &CardExitFx)>,
    card_draw_anim_q: Query<Entity, With<CardDrawAnim>>,
) {
    *draw_tracker = HandDrawAnimTracker::default();
    *exit_tracker = HandExitAnimTracker::default();
    battle_events.clear();
    ui_notices.clear();

    for (entity, fx) in &card_exit_fx_q {
        if fx.role == CardExitRole::Container {
            commands.entity(entity).despawn();
        }
    }
    for entity in &card_draw_anim_q {
        commands.entity(entity).remove::<CardDrawAnim>();
    }

    for (mut text, mut visibility, mut action_text) in &mut action_text_q {
        text.0.clear();
        action_text.remaining = 0.0;
        *visibility = Visibility::Hidden;
    }
}

/// 检测展示侧手牌新增的末尾卡槽并为其启动入手动画，同时推进进行中的动画。
///
/// 展示侧判定与 `update_player_hand_ui_system` 保持一致（弃牌阶段镜像处理），
/// 这样仅对本地玩家"看得见正在进入自己手牌"的卡播放动画；切换展示侧只更新基线、
/// 不触发整手动画。
pub(crate) fn animate_hand_card_draw_system(
    time: Res<Time>,
    mut events: MessageReader<BattleEvent>,
    battle_phase: Res<State<BattlePhase>>,
    hand: Res<Hand>,
    ui_control_side: Res<UiControlSide>,
    battle_mode: Res<BattleControlMode>,
    pending_discard: Option<Res<PendingHandDiscard>>,
    mut tracker: ResMut<HandDrawAnimTracker>,
    mut commands: Commands,
    mut cards: Query<(
        Entity,
        &PlayerCardButton,
        &mut UiTransform,
        Option<&mut CardDrawAnim>,
    )>,
) {
    let display_side = if *battle_phase.get() == BattlePhase::Discard
        && pending_discard
            .as_ref()
            .is_some_and(|pending| pending.side == Side::Enemy)
        && matches!(
            *battle_mode,
            BattleControlMode::PlayerVsAi | BattleControlMode::PlayerVsRemote
        ) {
        Side::Player
    } else {
        ui_control_side.0
    };

    let current_hand: &[CardId] = match display_side {
        Side::Player => &hand.player,
        Side::Enemy => &hand.enemy,
    };

    let mut reported_draw_count = 0;
    let mut has_used = false;
    let mut has_discarded = false;
    for event in events.read() {
        match event {
            BattleEvent::CardsDrawn { side, count } if *side == display_side => {
                reported_draw_count += *count;
            }
            BattleEvent::CardUsed { side, .. } if *side == display_side => {
                has_used = true;
            }
            BattleEvent::CardDiscarded { side, .. } if *side == display_side => {
                has_discarded = true;
            }
            _ => {}
        }
    }
    let draw_base_delay = card_draw_base_delay(has_used, has_discarded);

    // 仅当展示侧未变（或首次观测）时才检测新摸到的牌；切换展示侧只更新基线。
    let same_or_first = tracker.last_side == Some(display_side) || tracker.last_side.is_none();
    let new_range = same_or_first
        .then(|| detect_drawn_card_range(&tracker.last_hand, current_hand, reported_draw_count))
        .flatten();
    tracker.last_side = Some(display_side);
    tracker.last_hand = current_hand.to_vec();

    let dt = time.delta_secs();
    for (entity, btn, mut transform, anim) in &mut cards {
        // 新摸到的牌：设为起点姿态并挂上动画，下一帧起推进。
        if let Some(range) = &new_range {
            if range.contains(&btn.index) {
                apply_card_draw_pose(&mut transform, 0.0);
                commands
                    .entity(entity)
                    .insert(CardDrawAnim::new(btn.index - range.start, draw_base_delay));
                continue;
            }
        }

        let Some(mut anim) = anim else {
            continue;
        };
        anim.elapsed += dt;
        let local = anim.elapsed - anim.delay;
        if local <= 0.0 {
            apply_card_draw_pose(&mut transform, 0.0);
        } else if local >= anim.duration {
            *transform = UiTransform::IDENTITY;
            commands.entity(entity).remove::<CardDrawAnim>();
        } else {
            apply_card_draw_pose(&mut transform, local / anim.duration);
        }
    }
}

// ============================================================================
// 手牌离场动画（使用 / 弃置）——与入手动画配套
//
// 与入手动画相反：卡牌从手牌离开时，在它原本所在的卡槽生成一张"残影"卡（独立于
// 18 张固定卡槽，因此真实手牌可立即重排补位），再让残影飞出并淡出：
//   - 使用：上抬 + 放大，如"打出/施放"；
//   - 弃置：向右下角（弃牌堆方向）抛出 + 缩小 + 旋转。
//
// 哪张牌离场、来自哪个卡槽：通过对"展示侧手牌快照"做差分得到离场槽位与 CardId；
// 使用 vs 弃置：读取 `BattleEvent::CardUsed/CardDiscarded` 并按牌名匹配（带 1~2 帧
// TTL 缓冲，吸收系统执行顺序带来的事件/资源变更错帧）。覆盖所有出牌/弃牌渠道。
// ============================================================================

/// 离场动画总时长（秒）。
const CARD_EXIT_ANIM_DURATION: f32 = 0.42;
/// 残影卡层级（盖在手牌之上，飞出时不被其他卡遮挡）。
const CARD_EXIT_Z: i32 = 60;

/// 卡牌离场方式，决定飞出轨迹。
#[derive(Clone, Copy, PartialEq)]
enum CardExitKind {
    Used,
    Discarded,
}

/// 残影子树中实体的角色：容器驱动位移/缩放/底色与销毁，文字仅淡出。
#[derive(Clone, Copy, PartialEq)]
enum CardExitRole {
    Container,
    Label,
}

/// 残影卡动画状态。
#[derive(Component)]
pub(crate) struct CardExitFx {
    timer: Timer,
    delay: f32,
    kind: CardExitKind,
    role: CardExitRole,
}

impl CardExitFx {
    fn new(kind: CardExitKind, role: CardExitRole, delay: f32) -> Self {
        Self {
            timer: Timer::from_seconds(CARD_EXIT_ANIM_DURATION, TimerMode::Once),
            delay,
            kind,
            role,
        }
    }
}

/// 追踪上一帧"展示侧手牌"快照，并缓存近期出牌/弃牌事件用于离场分类。
#[derive(Resource, Default)]
pub(crate) struct HandExitAnimTracker {
    last_side: Option<Side>,
    last_hand: Vec<CardId>,
    /// 近期事件分类：(牌名, 方式, 年龄帧数)；保留约 2 帧以容忍执行顺序错帧。
    recent: Vec<(String, CardExitKind, u8)>,
}

fn ease_out_cubic(t: f32) -> f32 {
    let p = 1.0 - t;
    1.0 - p * p * p
}

/// 透明度曲线：前半段保持不透明，后半段线性淡出。
fn card_exit_alpha(fraction: f32) -> f32 {
    if fraction < 0.5 {
        1.0
    } else {
        (1.0 - (fraction - 0.5) / 0.5).clamp(0.0, 1.0)
    }
}

/// 按方式与进度计算残影容器的相对位移 / 缩放 / 旋转。
fn card_exit_pose(kind: CardExitKind, fraction: f32) -> UiTransform {
    let e = ease_out_cubic(fraction);
    match kind {
        // 使用：上抬并放大，如"打出/施放"。
        CardExitKind::Used => UiTransform {
            translation: Val2::px(0.0, -150.0 * e),
            scale: Vec2::splat(1.0 + 0.28 * e),
            rotation: Rot2::IDENTITY,
        },
        // 弃置：向右下角抛出 + 缩小 + 旋转，如"丢入弃牌堆"。
        CardExitKind::Discarded => UiTransform {
            translation: Val2::px(300.0 * e, 170.0 * e),
            scale: Vec2::splat(1.0 - 0.55 * e),
            rotation: Rot2::radians(0.6 * e),
        },
    }
}

/// 找出从 `old` 到 `new` 被移除的卡槽：返回 (槽位下标, 被移除的 CardId)。
///
/// 所有出牌/弃牌都按下标 remove，余牌左移补位，因此首个不一致处即移除位；若 `new`
/// 是 `old` 的前缀，则移除位在末尾。仅当确有一张牌离场（长度减少，或等长但内容变化
/// 即"出牌同时摸牌"）时返回 Some。
fn detect_removed_card(old: &[CardId], new: &[CardId]) -> Option<(usize, CardId)> {
    if old.is_empty() || new.len() > old.len() {
        return None;
    }
    if new.len() == old.len() && old == new {
        return None;
    }
    let index = (0..new.len())
        .find(|&i| new[i] != old[i])
        .unwrap_or(new.len());
    old.get(index).map(|card| (index, *card))
}

/// 检测展示侧手牌的离场牌并在其原卡槽生成残影；同时维护事件分类缓冲。
pub(crate) fn spawn_hand_card_exit_system(
    mut commands: Commands,
    mut events: MessageReader<BattleEvent>,
    battle_phase: Res<State<BattlePhase>>,
    hand: Res<Hand>,
    ui_control_side: Res<UiControlSide>,
    battle_mode: Res<BattleControlMode>,
    pending_discard: Option<Res<PendingHandDiscard>>,
    dbs: Res<BattleDbs>,
    theme: Res<UiTheme>,
    ui_font: Option<Res<UiFontHandle>>,
    mut tracker: ResMut<HandExitAnimTracker>,
    root: Query<Entity, With<HandCardsRoot>>,
) {
    let display_side = if *battle_phase.get() == BattlePhase::Discard
        && pending_discard
            .as_ref()
            .is_some_and(|pending| pending.side == Side::Enemy)
        && matches!(
            *battle_mode,
            BattleControlMode::PlayerVsAi | BattleControlMode::PlayerVsRemote
        ) {
        Side::Player
    } else {
        ui_control_side.0
    };

    // 维护事件分类缓冲：先令既有条目老化（保留约 2 帧），再纳入本帧新事件。
    tracker.recent.retain_mut(|(_, _, age)| {
        *age += 1;
        *age <= 1
    });
    let mut has_new_used_event = false;
    for event in events.read() {
        match event {
            BattleEvent::CardUsed { side, card_name } if *side == display_side => {
                has_new_used_event = true;
                tracker
                    .recent
                    .push((card_name.clone(), CardExitKind::Used, 0));
            }
            BattleEvent::CardDiscarded { side, card_name } if *side == display_side => {
                tracker
                    .recent
                    .push((card_name.clone(), CardExitKind::Discarded, 0));
            }
            _ => {}
        }
    }

    let current: &[CardId] = match display_side {
        Side::Player => &hand.player,
        Side::Enemy => &hand.enemy,
    };

    let removed = (tracker.last_side == Some(display_side))
        .then(|| detect_removed_card(&tracker.last_hand, current))
        .flatten();

    if let Some((slot_index, card_id)) = removed {
        let old_len = tracker.last_hand.len();
        let card = dbs.cards.get(&card_id);
        let card_name = card
            .map(|card| card.name.clone())
            .unwrap_or_else(|| format!("{card_id:?}"));

        // 分类：优先匹配近期事件牌名；缺事件时按阶段兜底（弃牌阶段→弃置，否则→使用）。
        let kind = tracker
            .recent
            .iter()
            .position(|(name, _, _)| *name == card_name)
            .map(|pos| tracker.recent.remove(pos).1)
            .unwrap_or(if *battle_phase.get() == BattlePhase::Discard {
                CardExitKind::Discarded
            } else {
                CardExitKind::Used
            });
        let exit_delay = if kind == CardExitKind::Discarded && has_new_used_event {
            CARD_EXIT_ANIM_DURATION
        } else {
            0.0
        };

        let accent = card
            .map(|card| super::super::helpers::card_category_color(card, &theme))
            .unwrap_or(theme.gold);

        if let Ok(root) = root.single() {
            spawn_card_exit_ghost(
                &mut commands,
                root,
                &theme,
                ui_font.as_deref(),
                slot_index,
                old_len,
                card_name,
                accent,
                kind,
                exit_delay,
            );
        }
    }

    tracker.last_side = Some(display_side);
    tracker.last_hand = current.to_vec();
}

/// 在离场卡原本所在卡槽生成一张残影卡（与真实卡槽相同的定位算法）。
fn spawn_card_exit_ghost(
    commands: &mut Commands,
    root: Entity,
    theme: &UiTheme,
    ui_font: Option<&UiFontHandle>,
    slot_index: usize,
    hand_len: usize,
    card_name: String,
    accent: Color,
    kind: CardExitKind,
    delay: f32,
) {
    let name_font = super::super::helpers::make_text_font(16.0, ui_font);
    commands.entity(root).with_children(|parent| {
        parent
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    margin: UiRect::left(Val::Px(hand_card_center_offset(slot_index, hand_len))),
                    bottom: Val::Px(hand_card_bottom(slot_index)),
                    width: Val::Px(HAND_CARD_WIDTH),
                    height: Val::Px(HAND_CARD_FULL_HEIGHT),
                    padding: UiRect::px(10.0, 10.0, 12.0, 10.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(theme.radius_card),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(5.0),
                    ..default()
                },
                BackgroundColor(theme.card_bg),
                BorderColor::all(accent),
                ZIndex(CARD_EXIT_Z),
                Pickable::IGNORE,
                CardExitFx::new(kind, CardExitRole::Container, delay),
            ))
            .with_children(|ghost| {
                ghost.spawn((
                    Text::new(card_name),
                    name_font,
                    TextColor(theme.ink_primary),
                    Pickable::IGNORE,
                    CardExitFx::new(kind, CardExitRole::Label, delay),
                ));
            });
    });
}

/// 推进残影动画并在到期时销毁整张残影。
pub(crate) fn tick_hand_card_exit_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(
        Entity,
        &mut CardExitFx,
        Option<&mut UiTransform>,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
        Option<&mut TextColor>,
    )>,
) {
    for (entity, mut fx, transform, background, border, text_color) in &mut q {
        if fx.delay > 0.0 {
            fx.delay = (fx.delay - time.delta_secs()).max(0.0);
            continue;
        }

        fx.timer.tick(time.delta());
        let fraction = fx.timer.fraction();
        let alpha = card_exit_alpha(fraction);

        match fx.role {
            CardExitRole::Container => {
                if let Some(mut transform) = transform {
                    *transform = card_exit_pose(fx.kind, fraction);
                }
                if let Some(mut background) = background {
                    background.0.set_alpha(alpha * 0.95);
                }
                if let Some(mut border) = border {
                    border.top.set_alpha(alpha);
                    border.right.set_alpha(alpha);
                    border.bottom.set_alpha(alpha);
                    border.left.set_alpha(alpha);
                }
                if fx.timer.just_finished() {
                    commands.entity(entity).despawn();
                }
            }
            CardExitRole::Label => {
                if let Some(mut text_color) = text_color {
                    text_color.0.set_alpha(alpha);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::CardId;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn detects_drawn_cards_when_hand_grows() {
        let old = [CardId::GainAp, CardId::NextAttackBoost];
        let current = [
            CardId::GainAp,
            CardId::NextAttackBoost,
            CardId::TacticalRefresh,
            CardId::RotationCover,
        ];

        assert_eq!(detect_drawn_card_range(&old, &current, 0), Some(2..4));
    }

    #[test]
    fn detects_drawn_card_after_discard_with_same_final_len() {
        let old = [CardId::GainAp, CardId::NextAttackBoost];
        let current = [CardId::NextAttackBoost, CardId::TacticalRefresh];

        assert_eq!(detect_drawn_card_range(&old, &current, 0), Some(1..2));
    }

    #[test]
    fn reported_draw_count_handles_same_card_drawn_back_to_same_slot() {
        let old = [CardId::GainAp, CardId::NextAttackBoost];
        let current = [CardId::GainAp, CardId::NextAttackBoost];

        assert_eq!(detect_drawn_card_range(&old, &current, 1), Some(1..2));
    }

    #[test]
    fn draw_delay_waits_for_used_and_discarded_exits() {
        assert_eq!(card_draw_base_delay(false, false), 0.0);
        assert_eq!(card_draw_base_delay(true, false), CARD_EXIT_ANIM_DURATION);
        assert_eq!(card_draw_base_delay(false, true), CARD_EXIT_ANIM_DURATION);
        assert_eq!(
            card_draw_base_delay(true, true),
            CARD_EXIT_ANIM_DURATION * 2.0
        );
    }

    #[test]
    fn reset_hand_draw_anim_tracker_clears_stale_battle_presentation() {
        let mut app = App::new();
        app.init_resource::<HandDrawAnimTracker>();
        app.init_resource::<HandExitAnimTracker>();
        app.init_resource::<Messages<BattleEvent>>();
        app.init_resource::<Messages<BattleUiNotice>>();

        app.world_mut()
            .resource_mut::<Messages<BattleEvent>>()
            .write(BattleEvent::SkillUsed {
                side: Side::Player,
                skill_name: "火拳".to_string(),
                slot: 0,
            });
        app.world_mut()
            .resource_mut::<Messages<BattleUiNotice>>()
            .write(BattleUiNotice { text: "AP不足" });
        app.world_mut().spawn((
            Text::new("我方发动火拳"),
            Visibility::Visible,
            BattleActionText { remaining: 1.5 },
        ));

        app.world_mut()
            .run_system_once(reset_hand_draw_anim_tracker)
            .expect("reset system should run");

        assert!(app.world().resource::<Messages<BattleEvent>>().is_empty());
        assert!(
            app.world()
                .resource::<Messages<BattleUiNotice>>()
                .is_empty()
        );

        let mut query = app
            .world_mut()
            .query::<(&Text, &Visibility, &BattleActionText)>();
        let (text, visibility, action_text) = query.single(app.world()).expect("action text");
        assert_eq!(text.0, "");
        assert_eq!(*visibility, Visibility::Hidden);
        assert_eq!(action_text.remaining, 0.0);
    }
}
