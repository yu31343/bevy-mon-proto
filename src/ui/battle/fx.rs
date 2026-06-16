//! 战斗简易特效系统
//!
//! 负责处理战斗中的视觉特效，包括：
//! - 技能格闪白效果（技能释放时）
//! - 伤害/治疗飘字效果（受到伤害或治疗时）
//! - 受击闪屏效果（受到攻击时）

use bevy::prelude::*;

use crate::battle::{BattleEvent, DamageType, Side};
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
    mut commands: Commands,
    mut q: Query<
        (Entity, &Interaction, &mut BackgroundColor, &mut BorderColor),
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
    for (entity, interaction, mut bg, mut border) in &mut q {
        if *interaction == Interaction::Pressed {
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
