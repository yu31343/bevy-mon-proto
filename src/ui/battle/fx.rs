//! 战斗简易特效：技能格闪白、伤害/治疗飘字、受击闪屏。

use bevy::prelude::*;

use crate::battle::{BattleEvent, Side};

use super::{components::{BattleUiRoot, DiscardButton, EndTurnButton, PlayerCardButton, SkillButton, SkillSlotId}, resources::UiFontHandle, theme::UiTheme};

/// 技能格闪白计时（与 `SkillSlotId` 同实体）。
#[derive(Component)]
pub struct SkillFlashTimer(pub Timer);

/// 临时 UI 实体生命周期。
#[derive(Component)]
pub struct FxLifetime(pub Timer);

/// 全屏受击闪屏计时。
#[derive(Component)]
pub struct ScreenFlashTimer(pub Timer);

/// 消费本帧全部战斗事件并触发对应特效（单系统只读一次消息）。
pub fn process_battle_fx_events(
    mut events: MessageReader<BattleEvent>,
    mut commands: Commands,
    root: Query<Entity, With<BattleUiRoot>>,
    slots: Query<(Entity, &SkillSlotId)>,
    ui_font: Option<Res<UiFontHandle>>,
) {
    let Ok(root) = root.single() else {
        return;
    };

    let make_font = |size: f32| {
        let mut f = TextFont::from_font_size(size);
        if let Some(h) = ui_font.as_ref() {
            f.font = h.0.clone();
        }
        f
    };

    for event in events.read() {
        match event {
            BattleEvent::SkillUsed { side, slot, .. } => {
                for (entity, sid) in &slots {
                    if sid.side == *side && sid.index == *slot {
                        commands
                            .entity(entity)
                            .insert(SkillFlashTimer(Timer::from_seconds(0.2, TimerMode::Once)));
                    }
                }
            }
            BattleEvent::DamageDealt { target, amount, .. } => {
                if *amount <= 0 {
                    continue;
                }
                let top = if *target == Side::Enemy {
                    Val::Percent(22.0)
                } else {
                    Val::Percent(68.0)
                };
                commands.entity(root).with_children(|p| {
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(44.0),
                            top,
                            ..default()
                        },
                        Text::new(format!("-{amount}")),
                        make_font(26.0),
                        TextColor(Color::srgb(1.0, 0.35, 0.35)),
                        TextShadow {
                            offset: Vec2::new(1.0, 1.0),
                            color: Color::srgba(0.0, 0.0, 0.0, 0.75),
                        },
                        FxLifetime(Timer::from_seconds(1.1, TimerMode::Once)),
                    ));
                });
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
                        BackgroundColor(Color::srgba(0.85, 0.15, 0.1, 0.22)),
                        ScreenFlashTimer(Timer::from_seconds(0.12, TimerMode::Once)),
                    ));
                });
            }
            BattleEvent::Healed { amount, .. } => {
                if *amount <= 0 {
                    continue;
                }
                commands.entity(root).with_children(|p| {
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(44.0),
                            top: Val::Percent(52.0),
                            ..default()
                        },
                        Text::new(format!("+{amount}")),
                        make_font(24.0),
                        TextColor(Color::srgb(0.45, 1.0, 0.55)),
                        TextShadow {
                            offset: Vec2::new(1.0, 1.0),
                            color: Color::srgba(0.0, 0.0, 0.0, 0.65),
                        },
                        FxLifetime(Timer::from_seconds(1.0, TimerMode::Once)),
                    ));
                });
            }
            _ => {}
        }
    }
}

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
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            commands.entity(entity).remove::<SkillFlashTimer>();
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
            *bg = BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.92));
            *border = BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.85));
        }
    }
}

pub fn tick_screen_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut ScreenFlashTimer)>,
) {
    for (entity, mut t) in &mut q {
        t.0.tick(time.delta());
        if t.0.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub fn tick_fx_lifetimes(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut FxLifetime)>,
) {
    for (entity, mut life) in &mut q {
        life.0.tick(time.delta());
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
                With<PlayerCardButton>,
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

