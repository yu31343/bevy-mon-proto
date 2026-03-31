//! 战斗简易特效系统
//! 
//! 负责处理战斗中的视觉特效，包括：
//! - 技能格闪白效果（技能释放时）
//! - 伤害/治疗飘字效果（受到伤害或治疗时）
//! - 受击闪屏效果（受到攻击时）

use bevy::prelude::*;

use crate::battle::{BattleEvent, Side};

use super::{components::{BattleUiRoot, DiscardButton, EndTurnButton, SkillButton, SkillSlotId}, resources::UiFontHandle, theme::UiTheme};

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
    root: Query<Entity, With<BattleUiRoot>>,
    slots: Query<(Entity, &SkillSlotId)>,
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

    // 遍历所有战斗事件
    for event in events.read() {
        match event {
            // 处理技能使用事件：触发技能格闪白效果
            BattleEvent::SkillUsed { side, slot, .. } => {
                // 查找对应的技能槽位实体
                for (entity, sid) in &slots {
                    if sid.side == *side && sid.index == *slot {
                        // 为技能槽位添加闪白计时器组件
                        commands
                            .entity(entity)
                            .insert(SkillFlashTimer(Timer::from_seconds(0.2, TimerMode::Once)));
                    }
                }
            }
            // 处理伤害事件：显示伤害飘字和受击闪屏
            BattleEvent::DamageDealt { target, amount, .. } => {
                // 跳过无效伤害值
                if *amount <= 0 {
                    continue;
                }
                
                // 根据目标阵营确定飘字位置
                let top = if *target == Side::Enemy {
                    Val::Percent(22.0)  // 敌方飘字位置
                } else {
                    Val::Percent(68.0)  // 玩家飘字位置
                };
                
                // 创建伤害飘字
                commands.entity(root).with_children(|p| {
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(44.0),
                            top,
                            ..default()
                        },
                        Text::new(format!("-{amount}")),  // 伤害值显示（带负号）
                        make_font(26.0),
                        TextColor(Color::srgb(1.0, 0.35, 0.35)),  // 红色伤害文字
                        TextShadow {
                            offset: Vec2::new(1.0, 1.0),
                            color: Color::srgba(0.0, 0.0, 0.0, 0.75),
                        },
                        FxLifetime(Timer::from_seconds(1.1, TimerMode::Once)),  // 飘字持续1.1秒
                    ));
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
                        BackgroundColor(Color::srgba(0.85, 0.15, 0.1, 0.22)),  // 淡红色闪屏
                        ScreenFlashTimer(Timer::from_seconds(0.12, TimerMode::Once)),  // 闪屏持续0.12秒
                    ));
                });
            }
            // 处理治疗事件：显示治疗飘字
            BattleEvent::Healed { amount, .. } => {
                // 跳过无效治疗值
                if *amount <= 0 {
                    continue;
                }
                
                // 创建治疗飘字
                commands.entity(root).with_children(|p| {
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(44.0),
                            top: Val::Percent(52.0),  // 治疗飘字位置
                            ..default()
                        },
                        Text::new(format!("+{amount}")),  // 治疗值显示（带加号）
                        make_font(24.0),
                        TextColor(Color::srgb(0.45, 1.0, 0.55)),  // 绿色治疗文字
                        TextShadow {
                            offset: Vec2::new(1.0, 1.0),
                            color: Color::srgba(0.0, 0.0, 0.0, 0.65),
                        },
                        FxLifetime(Timer::from_seconds(1.0, TimerMode::Once)),  // 飘字持续1.0秒
                    ));
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

/// 检测所有可交互按钮的按下事件，插入 `ButtonClickFlash` 并立即显示闪光色。
pub fn spawn_button_click_flash(
    mut commands: Commands,
    mut q: Query<
        (Entity, &Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            Changed<Interaction>,
            Or<(
                With<SkillButton>,
                With<EndTurnButton>,
                With<DiscardButton>,
            )>,
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
    mut q: Query<(Entity, &mut ButtonClickFlash, &mut BackgroundColor, &mut BorderColor)>,
    theme: Res<UiTheme>,
) {
    for (entity, mut flash, mut bg, mut border) in &mut q {
        flash.0.tick(time.delta());
        if flash.0.just_finished() {
            commands.entity(entity).remove::<ButtonClickFlash>();
            *bg = BackgroundColor(theme.button_idle);
            *border = BorderColor::all(theme.button_border_idle);
        }
    }
}

