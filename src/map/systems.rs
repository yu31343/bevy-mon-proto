use crate::console_log::{ConsoleLogCategory, log as console_log};
use crate::data::{MapBattleContext, MonsterPool};
use crate::game_state::GameState;
use crate::map::components::{
    Character, EnterLobbyButton, Map, MapUiRoot, SpriteEntity, SpriteNameLabel,
};
use crate::team_selection::SelectionEntryMode;
use crate::ui::battle::{resources::UiFontHandle, theme::UiTheme};
use bevy::prelude::*;

pub fn spawn_map(mut commands: Commands) {
    // 极简地图：一个大的矩形背景
    commands.spawn((
        Sprite {
            color: Color::srgb(0.2, 0.8, 0.2), // 绿色背景
            custom_size: Some(Vec2::new(800.0, 600.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        Map,
    ));
}

pub fn spawn_character(mut commands: Commands) {
    // 角色：一个简单的矩形
    commands.spawn((
        Sprite {
            color: Color::srgb(1.0, 0.0, 0.0), // 红色角色
            custom_size: Some(Vec2::new(20.0, 20.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 1.0),
        Character {
            speed: 100.0,
            target_position: None,
        },
    ));
}

pub fn setup_sprite_spine(
    _commands: Commands,
    // 暂时简化，稍后添加Spine
) {
    // TODO: 集成Spine动画
}

pub fn spawn_sprites(
    mut commands: Commands,
    monster_pool: Res<MonsterPool>,
    ui_font: Option<Res<UiFontHandle>>,
) {
    let positions = [
        Vec3::new(100.0, 100.0, 1.0),
        Vec3::new(-100.0, 50.0, 1.0),
        Vec3::new(200.0, -50.0, 1.0),
    ];

    for (monster_index, (monster, pos)) in monster_pool
        .monsters
        .iter()
        .zip(positions.iter())
        .enumerate()
    {
        commands
            .spawn((
                Sprite {
                    color: Color::srgb(0.0, 0.0, 1.0),
                    custom_size: Some(Vec2::new(30.0, 30.0)),
                    ..default()
                },
                Transform::from_translation(*pos),
                SpriteEntity {
                    monster_type: monster.name.clone(),
                    monster_index,
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text2d::new(monster.name.clone()),
                    make_text_font(20.0, ui_font.as_deref()),
                    TextColor(Color::WHITE),
                    Transform::from_xyz(0.0, 35.0, 1.0),
                    SpriteNameLabel,
                ));
            });
    }
}

pub fn setup_map_ui(
    mut commands: Commands,
    theme: Res<UiTheme>,
    ui_font: Option<Res<UiFontHandle>>,
    existing_ui: Query<(), With<MapUiRoot>>,
) {
    if !existing_ui.is_empty() {
        return;
    }

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Start,
                align_items: AlignItems::Start,
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            MapUiRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Button,
                Node {
                    width: Val::Px(180.0),
                    min_height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                EnterLobbyButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("进入大厅"),
                    make_text_font(22.0, ui_font.as_deref()),
                    TextColor(theme.text_primary),
                ));
            });
        });
}

pub fn move_character(
    time: Res<Time>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut query: Query<(&mut Transform, &mut Character)>,
) {
    // 设置目标位置
    if mouse_button_input.just_pressed(MouseButton::Left) {
        if let Ok(window) = windows.single() {
            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok((camera, camera_transform)) = camera_q.single() {
                    if let Ok(world_pos) = camera.viewport_to_world(camera_transform, cursor_pos) {
                        let world_pos_2d: Vec2 = world_pos.origin.truncate();
                        // 边界检查
                        let clamped_pos = Vec2::new(
                            world_pos_2d.x.clamp(-400.0 + 10.0, 400.0 - 10.0),
                            world_pos_2d.y.clamp(-300.0 + 10.0, 300.0 - 10.0),
                        );
                        for (_, mut character) in query.iter_mut() {
                            character.target_position = Some(clamped_pos);
                        }
                    }
                }
            }
        }
    }

    // 移动到目标
    for (mut transform, mut character) in query.iter_mut() {
        if let Some(target) = character.target_position {
            let current_pos = transform.translation.truncate();
            let direction = (target - current_pos).normalize();
            let distance = (target - current_pos).length();

            if distance > 5.0 {
                // 到达阈值
                let move_distance = character.speed * time.delta_secs();
                let new_pos = current_pos + direction * move_distance.min(distance);

                // 再次边界检查
                let clamped_pos = Vec2::new(
                    new_pos.x.clamp(-400.0 + 10.0, 400.0 - 10.0),
                    new_pos.y.clamp(-300.0 + 10.0, 300.0 - 10.0),
                );

                transform.translation.x = clamped_pos.x;
                transform.translation.y = clamped_pos.y;
            } else {
                character.target_position = None; // 到达目标
            }
        }
    }
}

pub fn click_sprites(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    sprite_q: Query<(&Transform, &SpriteEntity), With<SpriteEntity>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut entry_mode: ResMut<SelectionEntryMode>,
    mut map_battle_context: ResMut<MapBattleContext>,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        if let Ok(window) = windows.single() {
            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok((camera, camera_transform)) = camera_q.single() {
                    if let Ok(world_pos) = camera.viewport_to_world(camera_transform, cursor_pos) {
                        let world_pos: Vec2 = world_pos.origin.truncate(); // 2D
                        for (transform, sprite) in sprite_q.iter() {
                            let distance = (transform.translation.truncate() - world_pos).length();
                            if distance < 30.0 {
                                // 点击范围
                                console_log(
                                    ConsoleLogCategory::Map,
                                    format!(
                                        "点击地图怪物：{}（index={}）",
                                        sprite.monster_type, sprite.monster_index
                                    ),
                                );
                                map_battle_context.enemy_monster_index = Some(sprite.monster_index);
                                *entry_mode = SelectionEntryMode::VsAi;
                                next_state.set(GameState::TeamSelection);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn map_enter_lobby_button_system(
    mut next_state: ResMut<NextState<GameState>>,
    mut button_query: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<EnterLobbyButton>),
    >,
) {
    for interaction in &mut button_query {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::Lobby);
        }
    }
}

pub fn map_button_visual_system(
    theme: Res<UiTheme>,
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, With<Button>, With<EnterLobbyButton>),
    >,
) {
    for (interaction, mut background, mut border) in &mut interaction_query {
        *background = match *interaction {
            Interaction::Pressed => BackgroundColor(theme.button_pressed),
            Interaction::Hovered => BackgroundColor(theme.button_hover),
            Interaction::None => BackgroundColor(theme.button_idle),
        };

        *border = match *interaction {
            Interaction::Pressed => BorderColor::all(theme.button_border_pressed),
            Interaction::Hovered => BorderColor::all(theme.button_border_hover),
            Interaction::None => BorderColor::all(theme.button_border_idle),
        };
    }
}

pub fn cleanup_map(
    mut commands: Commands,
    query: Query<
        Entity,
        Or<(
            With<Map>,
            With<Character>,
            With<SpriteEntity>,
            With<SpriteNameLabel>,
            With<MapUiRoot>,
        )>,
    >,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn make_text_font(size: f32, ui_font: Option<&UiFontHandle>) -> TextFont {
    if let Some(ui_font) = ui_font {
        TextFont {
            font: ui_font.0.clone(),
            font_size: size,
            ..default()
        }
    } else {
        TextFont {
            font_size: size,
            ..default()
        }
    }
}
