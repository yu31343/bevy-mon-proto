use bevy::prelude::*;

use crate::{
    battle::{
        ActionPoints, BattleControlMode, Hand, PendingHandDiscard, SelectedCards, Side,
        UiControlSide,
    },
    data::BattleDbs,
    game_state::BattlePhase,
};

use super::super::components::*;
use super::super::theme::UiTheme;

const HAND_CARDS_PER_LAYER: usize = 6;
const HAND_CARD_WIDTH: f32 = 130.0;
const HAND_CARD_GAP: f32 = 8.0;
const HAND_CARD_FULL_HEIGHT: f32 = 180.0;
const HAND_CARD_STACK_HEIGHT: f32 = 34.0;
const HAND_CARD_SELECTED_WIDTH: f32 = 142.0;
const HAND_CARD_SELECTED_HEIGHT: f32 = 240.0;

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
    mut card_nodes: Query<(&PlayerCardButton, &mut Node, &mut ZIndex)>,
    mut card_band_q: Query<(&CardCategoryBand, &mut BackgroundColor)>,
    mut card_glow_q: Query<(&CardGlow, &mut BorderColor)>,
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

    for (mut text, is_hint, hotkey, name, cost, desc, cat) in &mut card_text_q {
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
            text.0 = card.name.to_string();
        } else if cost.is_some() {
            text.0 = card.cost_ap.to_string();
        } else if desc.is_some() {
            text.0 = super::super::helpers::card_description(card);
        } else if cat.is_some() {
            text.0 = super::super::helpers::card_category_label(card).to_string();
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
    for (glow, mut border) in &mut card_glow_q {
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
// （平移 / 缩放 / 旋转的相对偏移），与布局系统互不争用。所有抽牌渠道最终都把
// 卡牌 push 到 `hand.player` / `hand.enemy` 末尾，因此"展示侧手牌长度增长"即为
// 新牌进入信号——对新增的末尾槽位逐一挂上动画即可覆盖全部获牌渠道。
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

/// 追踪上一帧"展示侧手牌"的阵营与长度，用于检测新增的末尾卡槽。
#[derive(Resource, Default)]
pub(crate) struct HandDrawAnimTracker {
    last_side: Option<Side>,
    last_len: usize,
}

/// 挂在正在播放入手动画的 `PlayerCardButton` 上的动画状态。
#[derive(Component)]
pub(crate) struct CardDrawAnim {
    elapsed: f32,
    delay: f32,
    duration: f32,
}

impl CardDrawAnim {
    /// `order` 为该牌在本批新牌中的序号，用于错峰起播。
    fn new(order: usize) -> Self {
        Self {
            elapsed: 0.0,
            delay: order as f32 * CARD_DRAW_ANIM_STAGGER,
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

/// 进入战斗时重置追踪基线，使开局起手牌也能播放飞入动画。
pub(crate) fn reset_hand_draw_anim_tracker(mut tracker: ResMut<HandDrawAnimTracker>) {
    *tracker = HandDrawAnimTracker::default();
}

/// 检测展示侧手牌新增的末尾卡槽并为其启动入手动画，同时推进进行中的动画。
///
/// 展示侧判定与 `update_player_hand_ui_system` 保持一致（弃牌阶段镜像处理），
/// 这样仅对本地玩家"看得见正在进入自己手牌"的卡播放动画；切换展示侧只更新基线、
/// 不触发整手动画。
pub(crate) fn animate_hand_card_draw_system(
    time: Res<Time>,
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

    let hand_len = match display_side {
        Side::Player => hand.player.len(),
        Side::Enemy => hand.enemy.len(),
    };

    // 仅当展示侧未变（或首次观测）且手牌变长时，末尾新增的槽位才是新摸到的牌。
    let same_or_first = tracker.last_side == Some(display_side) || tracker.last_side.is_none();
    let new_range =
        (same_or_first && hand_len > tracker.last_len).then(|| tracker.last_len..hand_len);
    tracker.last_side = Some(display_side);
    tracker.last_len = hand_len;

    let dt = time.delta_secs();
    for (entity, btn, mut transform, anim) in &mut cards {
        // 新摸到的牌：设为起点姿态并挂上动画，下一帧起推进。
        if let Some(range) = &new_range {
            if range.contains(&btn.index) {
                apply_card_draw_pose(&mut transform, 0.0);
                commands
                    .entity(entity)
                    .insert(CardDrawAnim::new(btn.index - range.start));
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
