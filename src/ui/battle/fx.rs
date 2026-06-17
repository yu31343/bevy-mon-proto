//! 战斗简易特效系统
//!
//! 负责处理战斗中的视觉特效，包括：
//! - 技能格闪白效果（技能释放时）
//! - 伤害/治疗飘字效果（受到伤害或治疗时）
//! - 受击闪屏效果（受到攻击时）

use bevy::prelude::*;

use crate::battle::{BattleActionCooldown, BattleEvent, DamageType, Side};
use crate::ui::battle::systems::SwitchOverlayOpen;

use super::{
    components::{
        ActionDialButton, BattleUiCleanupPending, BattleUiRoot, DiscardButton, EndTurnButton,
        SkillButton, SkillSlotId, SwitchCancelButton, SwitchMonsterButton, TeamMemberButton,
    },
    resources::UiFontHandle,
    theme::UiTheme,
};

const DAMAGE_TEXT_PLAYER_TARGET_LEFT_PX: f32 = 356.0;
const DAMAGE_TEXT_ENEMY_TARGET_LEFT_PX: f32 = 1180.0;
const DAMAGE_TEXT_TARGET_TOP_PX: f32 = 326.0;
const DAMAGE_TEXT_FONT_SIZE: f32 = 34.0;
const DAMAGE_TEXT_FIXED_DAMAGE_ROW_OFFSET_PX: f32 = 46.0;
const DAMAGE_TEXT_STACK_ROW_GAP_PX: f32 = 40.0;

/// 技能格闪白计时器组件
/// - 与 `SkillSlotId` 组件附加在同一实体上
/// - 用于控制技能释放时的闪白效果持续时间
#[derive(Component)]
pub struct SkillFlashTimer(pub Timer);

/// 临时UI实体生命周期组件
/// - 用于控制飘字等临时特效的显示时长
#[derive(Component)]
pub struct FxLifetime(pub Timer);

/// 全屏受击闪屏计时器组件
/// - 用于控制受击时屏幕闪白的持续时间
#[derive(Component)]
pub struct ScreenFlashTimer(pub Timer);

/// 处理战斗事件并触发对应特效
///
/// 功能：
/// - 监听并处理 `BattleEvent` 消息
/// - 根据不同事件类型触发相应的视觉特效
/// - 单次系统调用中处理所有本帧事件，避免重复处理
///
/// 参数：
/// - `events`: 战斗事件读取器
/// - `commands`: 实体命令系统，用于创建/修改实体
/// - `root`: 查询UI根节点实体
/// - `slots`: 查询所有技能槽位实体
/// - `ui_font`: UI字体资源，用于飘字效果
pub fn process_battle_fx_events(
    mut events: MessageReader<BattleEvent>,
    mut commands: Commands,
    root: Query<Entity, (With<BattleUiRoot>, Without<BattleUiCleanupPending>)>,
    slots: Query<(Entity, &SkillSlotId)>,
    mut discard_button: Query<
        (Entity, &mut BackgroundColor, &mut BorderColor),
        With<DiscardButton>,
    >,
    ui_font: Option<Res<UiFontHandle>>,
) {
    // 获取UI根节点实体，若不存在则直接返回
    let Ok(root) = root.single() else {
        return;
    };

    // 辅助函数：创建指定大小的字体
    let make_font = |size: f32| {
        let mut f = TextFont::from_font_size(size);
        if let Some(h) = ui_font.as_ref() {
            f.font = h.0.clone();
        }
        f
    };

    let mut player_direct_damage_rows = 0;
    let mut player_fixed_damage_rows = 0;
    let mut player_heal_rows = 0;
    let mut player_miss_rows = 0;
    let mut enemy_direct_damage_rows = 0;
    let mut enemy_fixed_damage_rows = 0;
    let mut enemy_heal_rows = 0;
    let mut enemy_miss_rows = 0;

    // 遍历所有战斗事件
    for event in events.read() {
        match event {
            // 处理技能使用事件：触发技能格闪白效果
            BattleEvent::SkillUsed { side, slot, .. } => {
                for (entity, sid) in &slots {
                    if sid.side == *side && sid.index == *slot {
                        // 为技能槽位添加闪白计时器组件
                        commands
                            .entity(entity)
                            .insert(SkillFlashTimer(Timer::from_seconds(0.2, TimerMode::Once)));
                    }
                }
            }
            // 玩家实际弃牌时闪光弃牌按钮（武装/取消不触发，因为不会发出此事件）
            BattleEvent::CardDiscarded { side, .. } if *side == Side::Player => {
                if let Ok((entity, mut bg, mut border)) = discard_button.single_mut() {
                    *bg = BackgroundColor(CLICK_FLASH_COLOR);
                    *border = BorderColor::all(CLICK_FLASH_BORDER);
                    commands
                        .entity(entity)
                        .insert(ButtonClickFlash(Timer::from_seconds(0.15, TimerMode::Once)));
                }
            }
            // 处理伤害事件：显示伤害飘字和受击闪屏
            BattleEvent::DamageDealt {
                target,
                amount,
                damage_type,
                ..
            } => {
                // 跳过无效伤害值
                if *amount <= 0 {
                    continue;
                }

                let left = if *target == Side::Enemy {
                    Val::Px(DAMAGE_TEXT_ENEMY_TARGET_LEFT_PX)
                } else {
                    Val::Px(DAMAGE_TEXT_PLAYER_TARGET_LEFT_PX)
                };
                let row_index = match (*target, *damage_type) {
                    (Side::Player, DamageType::Direct) => {
                        let row = player_direct_damage_rows;
                        player_direct_damage_rows += 1;
                        row
                    }
                    (Side::Player, DamageType::Fixed) => {
                        let row = player_fixed_damage_rows;
                        player_fixed_damage_rows += 1;
                        row
                    }
                    (Side::Enemy, DamageType::Direct) => {
                        let row = enemy_direct_damage_rows;
                        enemy_direct_damage_rows += 1;
                        row
                    }
                    (Side::Enemy, DamageType::Fixed) => {
                        let row = enemy_fixed_damage_rows;
                        enemy_fixed_damage_rows += 1;
                        row
                    }
                };
                let type_offset = if *damage_type == DamageType::Fixed {
                    DAMAGE_TEXT_FIXED_DAMAGE_ROW_OFFSET_PX
                } else {
                    0.0
                };
                let top = Val::Px(
                    DAMAGE_TEXT_TARGET_TOP_PX
                        + type_offset
                        + row_index as f32 * DAMAGE_TEXT_STACK_ROW_GAP_PX,
                );
                let bg_color = match *damage_type {
                    DamageType::Direct => Color::srgba(0.82, 0.05, 0.04, 0.88),
                    DamageType::Fixed => Color::srgba(1.0, 0.42, 0.70, 0.88),
                };

                // 创建伤害飘字
                commands.entity(root).with_children(|p| {
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left,
                            top,
                            min_width: Val::Px(58.0),
                            padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(bg_color),
                        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.95)),
                        FxLifetime(Timer::from_seconds(1.1, TimerMode::Once)), // 飘字持续1.1秒
                    ))
                    .with_children(|damage_label| {
                        damage_label.spawn((
                            Text::new(format!("-{amount}")),
                            make_font(DAMAGE_TEXT_FONT_SIZE),
                            TextColor(Color::WHITE),
                            TextShadow {
                                offset: Vec2::new(1.0, 1.0),
                                color: Color::srgba(0.0, 0.0, 0.0, 0.70),
                            },
                        ));
                    });
                });

                // 创建受击闪屏效果
                commands.entity(root).with_children(|p| {
                    p.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.85, 0.15, 0.1, 0.22)), // 淡红色闪屏
                        ScreenFlashTimer(Timer::from_seconds(0.12, TimerMode::Once)), // 闪屏持续0.12秒
                    ));
                });
            }
            // 处理攻击未命中事件：显示 miss 提示
            BattleEvent::AttackMissed { target, .. } => {
                let left = if *target == Side::Enemy {
                    Val::Px(DAMAGE_TEXT_ENEMY_TARGET_LEFT_PX)
                } else {
                    Val::Px(DAMAGE_TEXT_PLAYER_TARGET_LEFT_PX)
                };
                let row_index = if *target == Side::Enemy {
                    let row = enemy_direct_damage_rows
                        + enemy_fixed_damage_rows
                        + enemy_heal_rows
                        + enemy_miss_rows;
                    enemy_miss_rows += 1;
                    row
                } else {
                    let row = player_direct_damage_rows
                        + player_fixed_damage_rows
                        + player_heal_rows
                        + player_miss_rows;
                    player_miss_rows += 1;
                    row
                };
                let top = Val::Px(
                    DAMAGE_TEXT_TARGET_TOP_PX + row_index as f32 * DAMAGE_TEXT_STACK_ROW_GAP_PX,
                );

                commands.entity(root).with_children(|p| {
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left,
                            top,
                            min_width: Val::Px(68.0),
                            padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.28, 0.88, 0.88)),
                        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.95)),
                        FxLifetime(Timer::from_seconds(1.0, TimerMode::Once)),
                    ))
                    .with_children(|miss_label| {
                        miss_label.spawn((
                            Text::new("miss"),
                            make_font(DAMAGE_TEXT_FONT_SIZE),
                            TextColor(Color::WHITE),
                            TextShadow {
                                offset: Vec2::new(1.0, 1.0),
                                color: Color::srgba(0.0, 0.0, 0.0, 0.70),
                            },
                        ));
                    });
                });
            }
            // 处理治疗事件：显示治疗飘字
            BattleEvent::Healed { side, amount } => {
                // 跳过无效治疗值
                if *amount <= 0 {
                    continue;
                }

                let left = if *side == Side::Enemy {
                    Val::Px(DAMAGE_TEXT_ENEMY_TARGET_LEFT_PX)
                } else {
                    Val::Px(DAMAGE_TEXT_PLAYER_TARGET_LEFT_PX)
                };
                let row_index = if *side == Side::Enemy {
                    let row = enemy_direct_damage_rows + enemy_fixed_damage_rows + enemy_heal_rows;
                    enemy_heal_rows += 1;
                    row
                } else {
                    let row =
                        player_direct_damage_rows + player_fixed_damage_rows + player_heal_rows;
                    player_heal_rows += 1;
                    row
                };
                let top = Val::Px(
                    DAMAGE_TEXT_TARGET_TOP_PX + row_index as f32 * DAMAGE_TEXT_STACK_ROW_GAP_PX,
                );

                // 创建治疗飘字
                commands.entity(root).with_children(|p| {
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left,
                            top,
                            min_width: Val::Px(58.0),
                            padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.62, 0.22, 0.88)),
                        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.95)),
                        FxLifetime(Timer::from_seconds(1.0, TimerMode::Once)), // 飘字持续1.0秒
                    ))
                    .with_children(|heal_label| {
                        heal_label.spawn((
                            Text::new(format!("+{amount}")),
                            make_font(DAMAGE_TEXT_FONT_SIZE),
                            TextColor(Color::WHITE),
                            TextShadow {
                                offset: Vec2::new(1.0, 1.0),
                                color: Color::srgba(0.0, 0.0, 0.0, 0.70),
                            },
                        ));
                    });
                });
            }
            // 其他事件类型暂不处理
            _ => {}
        }
    }
}
/// 更新技能格闪白效果
///
/// 功能：
/// - 更新技能格闪白计时器
/// - 根据计时器状态控制闪白效果的显示与结束
/// - 闪白结束后恢复技能格的原始样式
///
/// 参数：
/// - `commands`: 实体命令系统
/// - `time`: 时间资源，用于更新计时器
/// - `q`: 查询带有闪白计时器的技能槽位实体
/// - `theme`: UI主题资源，用于恢复原始样式
pub fn tick_skill_flash_timer(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(
        Entity,
        &mut SkillFlashTimer,
        &mut BackgroundColor,
        &mut BorderColor,
        &SkillSlotId,
    )>,
    theme: Res<UiTheme>,
) {
    for (entity, mut timer, mut bg, mut border, sid) in &mut q {
        // 更新计时器
        timer.0.tick(time.delta());

        if timer.0.just_finished() {
            // 计时器结束：移除闪白计时器组件，恢复原始样式
            commands.entity(entity).remove::<SkillFlashTimer>();

            // 根据阵营恢复不同的样式
            match sid.side {
                Side::Player => {
                    *bg = BackgroundColor(theme.button_idle);
                    *border = BorderColor::all(theme.button_border_idle);
                }
                Side::Enemy => {
                    *bg = BackgroundColor(theme.enemy_card_bg);
                    *border = BorderColor::all(theme.enemy_card_border);
                }
            }
        } else {
            // 计时器未结束：应用闪白效果
            *bg = BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.92));
            *border = BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.85));
        }
    }
}

/// 更新屏幕闪白效果
///
/// 功能：
/// - 更新闪屏计时器
/// - 计时器结束后销毁闪屏实体
///
/// 参数：
/// - `commands`: 实体命令系统
/// - `time`: 时间资源，用于更新计时器
/// - `q`: 查询带有闪屏计时器的实体
pub fn tick_screen_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut ScreenFlashTimer)>,
) {
    for (entity, mut t) in &mut q {
        // 更新计时器
        t.0.tick(time.delta());

        // 计时器结束：销毁闪屏实体
        if t.0.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// 更新临时特效的生命周期
///
/// 功能：
/// - 更新飘字等临时特效的生命周期计时器
/// - 计时器结束后销毁临时特效实体
///
/// 参数：
/// - `commands`: 实体命令系统
/// - `time`: 时间资源，用于更新计时器
/// - `q`: 查询带有生命周期计时器的实体
pub fn tick_fx_lifetimes(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut FxLifetime)>,
) {
    for (entity, mut life) in &mut q {
        // 更新计时器
        life.0.tick(time.delta());

        // 计时器结束：销毁临时特效实体
        if life.0.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// 按钮点击闪光计时（插入到被点击的按钮实体上）。
#[derive(Component)]
pub struct ButtonClickFlash(pub Timer);

const CLICK_FLASH_COLOR: Color = Color::srgba(0.72, 0.88, 1.0, 0.90);
const CLICK_FLASH_BORDER: Color = Color::srgba(0.72, 0.88, 1.0, 0.70);

/// 检测技能格与结束回合按钮的按下事件，插入 `ButtonClickFlash` 并立即显示闪光色。
/// 弃牌按钮不在此列——其颜色由 `update_discard_armed_visual_system` 全权管理。
pub fn spawn_button_click_flash(
    cooldown: Res<BattleActionCooldown>,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            Has<SkillButton>,
            Has<EndTurnButton>,
            Has<TeamMemberButton>,
        ),
        (
            Changed<Interaction>,
            Or<(
                With<SkillButton>,
                With<EndTurnButton>,
                With<SwitchMonsterButton>,
                With<SwitchCancelButton>,
                With<TeamMemberButton>,
            )>,
            Without<ActionDialButton>,
        ),
    >,
) {
    let locked = !cooldown.ready();
    for (entity, interaction, mut bg, mut border, is_skill, is_end_turn, is_team_member) in &mut q {
        if *interaction == Interaction::Pressed {
            if locked && (is_skill || is_end_turn || is_team_member) {
                continue;
            }
            *bg = BackgroundColor(CLICK_FLASH_COLOR);
            *border = BorderColor::all(CLICK_FLASH_BORDER);
            commands
                .entity(entity)
                .insert(ButtonClickFlash(Timer::from_seconds(0.15, TimerMode::Once)));
        }
    }
}

/// Tick 闪光计时器，到期后移除组件并恢复按钮颜色。
pub fn tick_button_click_flash(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(
        Entity,
        &mut ButtonClickFlash,
        &mut BackgroundColor,
        &mut BorderColor,
        Option<&Interaction>,
    )>,
    theme: Res<UiTheme>,
) {
    for (entity, mut flash, mut bg, mut border, interaction) in &mut q {
        flash.0.tick(time.delta());
        if flash.0.just_finished() {
            commands.entity(entity).remove::<ButtonClickFlash>();
            let interaction = interaction.copied().unwrap_or(Interaction::None);
            *bg = match interaction {
                Interaction::Pressed => BackgroundColor(theme.button_pressed),
                Interaction::Hovered => BackgroundColor(theme.button_hover),
                Interaction::None => BackgroundColor(theme.button_idle),
            };
            *border = match interaction {
                Interaction::Pressed => BorderColor::all(theme.button_border_pressed),
                Interaction::Hovered => BorderColor::all(theme.button_border_hover),
                Interaction::None => BorderColor::all(theme.button_border_idle),
            };
        }
    }
}

/// 将键盘快捷键触发的操作映射到 `ButtonClickFlash`，使视觉反馈与鼠标点击完全一致。
///
/// 纯 UI 层：只读取键盘输入、查找对应按钮实体、insert 组件并改颜色，不含任何战斗逻辑。
/// `tick_button_click_flash` 完全复用——只要实体上有 `ButtonClickFlash` 它就会运行。
///
/// 使用 `ParamSet` 规避多个 Query 同时 `&mut BackgroundColor` / `&mut BorderColor`
/// 导致的 Bevy B0001 Query 冲突——ParamSet 保证同一帧内每次只访问其中一个 Query。
///
/// 注意：弃牌按钮（F 键）不在此列，其颜色由 `update_discard_armed_visual_system` 全权管理。
pub fn keyboard_button_flash_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    cooldown: Res<BattleActionCooldown>,
    open: Res<SwitchOverlayOpen>,
    mut commands: Commands,
    mut queries: ParamSet<(
        // p0: 技能按钮（1/2/3/4）
        Query<(
            Entity,
            &super::components::SkillButton,
            &mut BackgroundColor,
            &mut BorderColor,
        )>,
        // p1: 结束回合按钮（E）
        Query<
            (Entity, &mut BackgroundColor, &mut BorderColor),
            With<super::components::EndTurnButton>,
        >,
        // p2: 手牌按钮（Z/X/C/V/B）
        Query<(
            Entity,
            &super::components::PlayerCardButton,
            &mut BackgroundColor,
            &mut BorderColor,
        )>,
        // p3: 队员切换按钮（5/6/7）
        Query<(
            Entity,
            &super::components::TeamMemberButton,
            &mut BackgroundColor,
            &mut BorderColor,
        )>,
        // p4: 换精灵按钮（Q）
        Query<
            (Entity, &mut BackgroundColor, &mut BorderColor),
            With<super::components::SwitchMonsterButton>,
        >,
        // p5: 取消按钮（Q 关闭时也闪）
        Query<
            (Entity, &mut BackgroundColor, &mut BorderColor),
            With<super::components::SwitchCancelButton>,
        >,
    )>,
) {
    let locked = !cooldown.ready();

    // 辅助宏：写颜色并 insert 计时器
    // 宏展开为内联代码，commands 来自外部作用域，entity 为 Copy，无借用冲突
    macro_rules! do_flash {
        ($entity:expr, $bg:expr, $border:expr) => {
            *$bg = BackgroundColor(CLICK_FLASH_COLOR);
            *$border = BorderColor::all(CLICK_FLASH_BORDER);
            commands
                .entity($entity)
                .insert(ButtonClickFlash(Timer::from_seconds(0.15, TimerMode::Once)));
        };
    }

    if !locked {
        // 1-4 → 技能按钮（slot 0-3）
        for (key, idx) in [
            (KeyCode::Digit1, 0usize),
            (KeyCode::Digit2, 1),
            (KeyCode::Digit3, 2),
            (KeyCode::Digit4, 3),
        ] {
            if keyboard.just_pressed(key) {
                for (entity, btn, mut bg, mut border) in queries.p0().iter_mut() {
                    if btn.index == idx {
                        do_flash!(entity, bg, border);
                        break;
                    }
                }
            }
        }

        // E → 结束回合按钮
        if keyboard.just_pressed(KeyCode::KeyE) {
            if let Ok((entity, mut bg, mut border)) = queries.p1().single_mut() {
                do_flash!(entity, bg, border);
            }
        }

        // F → 弃牌按钮：颜色由 update_discard_armed_visual_system 管理，此处不 flash

        // 手牌热键 → 手牌按钮
        for (key, idx) in [
            (KeyCode::KeyZ, 0usize),
            (KeyCode::KeyX, 1),
            (KeyCode::KeyC, 2),
            (KeyCode::KeyV, 3),
            (KeyCode::KeyB, 4),
            (KeyCode::KeyN, 5),
            (KeyCode::KeyA, 6),
            (KeyCode::KeyS, 7),
            (KeyCode::KeyD, 8),
            (KeyCode::KeyG, 9),
            (KeyCode::KeyH, 10),
            (KeyCode::KeyJ, 11),
            (KeyCode::KeyK, 12),
            (KeyCode::KeyL, 13),
            (KeyCode::KeyU, 14),
            (KeyCode::KeyI, 15),
            (KeyCode::KeyO, 16),
            (KeyCode::KeyP, 17),
        ] {
            if keyboard.just_pressed(key) {
                for (entity, btn, mut bg, mut border) in queries.p2().iter_mut() {
                    if btn.index == idx {
                        do_flash!(entity, bg, border);
                        break;
                    }
                }
            }
        }

        // 5/6/7 → 队员切换按钮（index 0/1/2）
        for (key, idx) in [
            (KeyCode::Digit5, 0usize),
            (KeyCode::Digit6, 1),
            (KeyCode::Digit7, 2),
        ] {
            if keyboard.just_pressed(key) {
                for (entity, btn, mut bg, mut border) in queries.p3().iter_mut() {
                    if btn.index == idx {
                        do_flash!(entity, bg, border);
                        break;
                    }
                }
            }
        }
    }

    // Q → 打开/关闭换人面板
    if keyboard.just_pressed(KeyCode::KeyQ) {
        if open.0 {
            if let Ok((entity, mut bg, mut border)) = queries.p5().single_mut() {
                do_flash!(entity, bg, border);
            }
        } else if let Ok((entity, mut bg, mut border)) = queries.p4().single_mut() {
            do_flash!(entity, bg, border);
        }
    }
}

// ============================================================================
// 元素反应中央横幅特效
//
// 触发元素反应时，在屏幕中央弹出一张"反应图标 + 反应名"横幅，带缩放弹出与淡入/
// 淡出动画。图标复用 `assets/images/icons/reactions/` 下的反应图标。
// ============================================================================

/// 反应横幅生命周期/动画总时长（秒）。
const REACTION_BANNER_LIFETIME_SECS: f32 = 1.4;
/// 同帧触发多个反应时，后续横幅相对屏幕中心的纵向堆叠间距（像素），避免重叠。
const REACTION_BANNER_STACK_GAP_PX: f32 = 92.0;

/// 反应名 -> (图标路径, 主题描边色)。
///
/// 反应名取自 `assets/data/battle_data.ron` 的 `reactions[].name`，多数与
/// `assets/images/icons/reactions/` 下同名 PNG 对应。未知反应名退化为纯文字横幅。
fn reaction_visual(reaction_name: &str) -> (Option<&'static str>, Color) {
    match reaction_name {
        "蒸发" => (
            Some("images/icons/reactions/蒸发.png"),
            Color::srgb(1.0, 0.58, 0.30),
        ),
        "燃烧" => (
            Some("images/icons/reactions/燃烧.png"),
            Color::srgb(1.0, 0.45, 0.22),
        ),
        "导电" => (
            Some("images/icons/reactions/导电.png"),
            Color::srgb(0.70, 0.52, 1.0),
        ),
        "绽放" => (
            Some("images/icons/reactions/绽放.png"),
            Color::srgb(0.42, 0.86, 0.50),
        ),
        "雷火燎原" => (
            Some("images/icons/reactions/雷火燎原.png"),
            Color::srgb(1.0, 0.40, 0.48),
        ),
        "超载" => (
            Some("images/icons/reactions/超载.png"),
            Color::srgb(0.93, 0.42, 0.90),
        ),
        "激化" => (
            Some("images/icons/reactions/激化.png"),
            Color::srgb(0.72, 0.92, 0.40),
        ),
        "蒸汽雷爆" => (
            Some("images/icons/reactions/蒸汽雷爆.png"),
            Color::srgb(0.50, 0.90, 1.0),
        ),
        "感电绽放" => (
            Some("images/icons/reactions/感电绽放.png"),
            Color::srgb(0.45, 0.95, 0.82),
        ),
        _ => (None, Color::srgb(1.0, 0.84, 0.40)),
    }
}

/// 全部反应图标路径（用于 Startup 预加载；须与 `reaction_visual` 的图标集保持一致）。
const REACTION_ICON_PATHS: &[&str] = &[
    "images/icons/reactions/蒸发.png",
    "images/icons/reactions/燃烧.png",
    "images/icons/reactions/导电.png",
    "images/icons/reactions/绽放.png",
    "images/icons/reactions/雷火燎原.png",
    "images/icons/reactions/超载.png",
    "images/icons/reactions/激化.png",
    "images/icons/reactions/蒸汽雷爆.png",
    "images/icons/reactions/感电绽放.png",
];

/// 预加载并持有反应图标句柄。
///
/// 横幅是短生命周期实体，若在触发瞬间才首次异步加载图标，可能来不及上传到 GPU 而短暂
/// 显示空白（露出深色底）。在 Startup 预加载并长期持有句柄，确保战斗中即时可用、不被卸载。
#[derive(Resource, Default)]
pub struct ReactionIconAssets {
    #[allow(dead_code)]
    handles: Vec<Handle<Image>>,
}

/// Startup：预加载全部反应图标。
pub fn preload_reaction_icons(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handles = REACTION_ICON_PATHS
        .iter()
        .map(|path| asset_server.load::<Image>(*path))
        .collect();
    commands.insert_resource(ReactionIconAssets { handles });
}

/// 横幅子树中实体的角色，决定淡入淡出作用于哪种颜色、是否缩放、是否负责销毁。
///
/// 关键：`Node` 会通过 `#[require]` 自动附带一个默认 `BackgroundColor`（`Color::NONE`，
/// 即 rgba(0,0,0,0) 透明黑）。若对其调用 `set_alpha` 会把它变成不透明黑，凭空画出黑块。
/// 因此只对“确实拥有可见颜色”的实体做淡入淡出，绝不碰图标/文字/遮罩的默认透明底。
#[derive(Clone, Copy, PartialEq)]
enum ReactionBannerRole {
    /// 全屏居中容器：自身无可见颜色，仅用于居中；到期时级联销毁整棵子树。
    Overlay,
    /// 胶囊：淡入淡出作用于背景 + 描边，并应用缩放弹出。
    Pill,
    /// 图标：淡入淡出作用于图片着色。
    Icon,
    /// 文字：淡入淡出作用于文字颜色。
    Text,
}

/// 元素反应中央横幅特效标记 + 动画状态。
///
/// 横幅子树（遮罩 / 胶囊 / 图标 / 文字）中每个实体各持一份相同时长的计时器，
/// 同帧 spawn 即同步推进；到期时由遮罩层负责级联销毁整棵子树。
#[derive(Component)]
pub struct ReactionBannerFx {
    timer: Timer,
    role: ReactionBannerRole,
}

impl ReactionBannerFx {
    fn new(role: ReactionBannerRole) -> Self {
        Self {
            timer: Timer::from_seconds(REACTION_BANNER_LIFETIME_SECS, TimerMode::Once),
            role,
        }
    }
}

/// 透明度曲线：前段淡入 -> 停留 -> 末段淡出。
fn reaction_banner_alpha(fraction: f32) -> f32 {
    const FADE_IN_END: f32 = 0.10;
    const FADE_OUT_START: f32 = 0.78;
    if fraction <= FADE_IN_END {
        (fraction / FADE_IN_END).clamp(0.0, 1.0)
    } else if fraction >= FADE_OUT_START {
        (1.0 - (fraction - FADE_OUT_START) / (1.0 - FADE_OUT_START)).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// 缩放曲线：0.6 弹出过冲到 1.12，回落到 1.0 停留，末段轻微收缩到 0.94。
fn reaction_banner_scale(fraction: f32) -> f32 {
    const POP_END: f32 = 0.13;
    const SETTLE_END: f32 = 0.22;
    const SHRINK_START: f32 = 0.86;
    if fraction <= POP_END {
        let t = fraction / POP_END;
        0.6 + (1.12 - 0.6) * t
    } else if fraction <= SETTLE_END {
        let t = (fraction - POP_END) / (SETTLE_END - POP_END);
        1.12 + (1.0 - 1.12) * t
    } else if fraction >= SHRINK_START {
        let t = (fraction - SHRINK_START) / (1.0 - SHRINK_START);
        1.0 + (0.94 - 1.0) * t
    } else {
        1.0
    }
}

/// 监听 `BattleEvent::ReactionTriggered`，在屏幕中央生成"反应图标 + 反应名"横幅。
///
/// 独立于 `process_battle_fx_events`：两者各自持有 `BattleEvent` 读游标，互不影响。
/// 没有战斗 UI 根（非战斗态）时直接返回，沿用本模块其它 FX 系统的约定。
pub fn spawn_reaction_banner_system(
    mut events: MessageReader<BattleEvent>,
    mut commands: Commands,
    root: Query<Entity, (With<BattleUiRoot>, Without<BattleUiCleanupPending>)>,
    ui_font: Option<Res<UiFontHandle>>,
    asset_server: Res<AssetServer>,
) {
    let Ok(root) = root.single() else {
        return;
    };

    // 辅助函数：创建指定大小的字体（复用嵌入的 CJK 字体）。
    let make_font = |size: f32| {
        let mut f = TextFont::from_font_size(size);
        if let Some(h) = ui_font.as_ref() {
            f.font = h.0.clone();
        }
        f
    };

    // 同帧多个反应时纵向堆叠，避免重叠。
    let mut stack_index = 0usize;
    for event in events.read() {
        let BattleEvent::ReactionTriggered { reaction_name, .. } = event else {
            continue;
        };

        let (icon_path, accent) = reaction_visual(reaction_name);
        let name = reaction_name.clone();
        let stack_offset_px = stack_index as f32 * REACTION_BANNER_STACK_GAP_PX;
        stack_index += 1;

        // 胶囊初始：缩小态（弹出动画起点）+ 纵向堆叠偏移。
        let pill_transform = {
            let mut t = UiTransform::from_scale(Vec2::splat(0.6));
            t.translation = Val2::px(0.0, stack_offset_px);
            t
        };

        commands.entity(root).with_children(|parent| {
            // 遮罩层：全屏，仅用于把胶囊居中；Pickable::IGNORE 确保不吞点击。
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Pickable::IGNORE,
                    ReactionBannerFx::new(ReactionBannerRole::Overlay),
                ))
                .with_children(|overlay| {
                    // 胶囊：图标 + 文字，深色半透明底 + 反应主题色描边，缩放弹出。
                    overlay
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(14.0),
                                padding: UiRect::axes(Val::Px(24.0), Val::Px(13.0)),
                                border: UiRect::all(Val::Px(3.0)),
                                border_radius: BorderRadius::all(Val::Px(16.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.06, 0.07, 0.12, 0.86)),
                            BorderColor::all(accent),
                            pill_transform,
                            Pickable::IGNORE,
                            ReactionBannerFx::new(ReactionBannerRole::Pill),
                        ))
                        .with_children(|pill| {
                            if let Some(path) = icon_path {
                                pill.spawn((
                                    Node {
                                        width: Val::Px(58.0),
                                        height: Val::Px(58.0),
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                    ImageNode::new(asset_server.load(path))
                                        .with_color(Color::srgba(1.0, 1.0, 1.0, 0.0)),
                                    Pickable::IGNORE,
                                    ReactionBannerFx::new(ReactionBannerRole::Icon),
                                ));
                            }
                            pill.spawn((
                                Text::new(name.clone()),
                                make_font(40.0),
                                TextColor(Color::WHITE),
                                TextShadow {
                                    offset: Vec2::new(2.0, 2.0),
                                    color: Color::srgba(0.0, 0.0, 0.0, 0.75),
                                },
                                Pickable::IGNORE,
                                ReactionBannerFx::new(ReactionBannerRole::Text),
                            ));
                        });
                });
        });
    }
}

/// 推进反应横幅动画并在到期时销毁。
///
/// 对子树中每个实体：按计时器进度更新透明度（淡入/停留/淡出），并对胶囊额外更新
/// 缩放；到期帧由根实体级联销毁整棵子树。
pub fn tick_reaction_banner_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(
        Entity,
        &mut ReactionBannerFx,
        Option<&mut UiTransform>,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
        Option<&mut TextColor>,
        Option<&mut ImageNode>,
    )>,
) {
    for (entity, mut fx, transform, background, border, text_color, image) in &mut q {
        fx.timer.tick(time.delta());
        let fraction = fx.timer.fraction();
        let alpha = reaction_banner_alpha(fraction);

        // 仅对各实体“确实拥有的可见颜色”做淡入淡出，绝不触碰节点自动附带的默认透明底，
        // 否则 set_alpha 会把透明黑 rgba(0,0,0,0) 变成不透明黑，画出黑块。
        match fx.role {
            ReactionBannerRole::Pill => {
                if let Some(mut background) = background {
                    background.0.set_alpha(alpha);
                }
                if let Some(mut border) = border {
                    border.top.set_alpha(alpha);
                    border.right.set_alpha(alpha);
                    border.bottom.set_alpha(alpha);
                    border.left.set_alpha(alpha);
                }
                // 缩放弹出仅作用于胶囊（保留 spawn 时设置的纵向堆叠平移）。
                if let Some(mut transform) = transform {
                    transform.scale = Vec2::splat(reaction_banner_scale(fraction));
                }
            }
            ReactionBannerRole::Icon => {
                if let Some(mut image) = image {
                    image.color = Color::srgba(1.0, 1.0, 1.0, alpha);
                }
            }
            ReactionBannerRole::Text => {
                if let Some(mut text_color) = text_color {
                    text_color.0.set_alpha(alpha);
                }
            }
            ReactionBannerRole::Overlay => {}
        }

        if fx.role == ReactionBannerRole::Overlay && fx.timer.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}
