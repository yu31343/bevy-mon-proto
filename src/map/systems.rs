use bevy::prelude::*;
use crate::game_state::GameState;
use crate::map::components::{Map, Character, SpriteEntity};
use crate::team_selection::SelectionEntryMode;

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

pub fn spawn_sprites(mut commands: Commands) {
    // 放置几个精灵：暂时使用蓝色方块，稍后替换为Spine
    let monster_names = ["火精灵", "水精灵", "风精灵"];
    let positions = vec![
        Vec3::new(100.0, 100.0, 1.0),
        Vec3::new(-100.0, 50.0, 1.0),
        Vec3::new(200.0, -50.0, 1.0),
    ];
    
    for (pos, &monster_name) in positions.iter().zip(monster_names.iter()) {
        commands.spawn((
            Sprite {
                color: Color::srgb(0.0, 0.0, 1.0),
                custom_size: Some(Vec2::new(30.0, 30.0)),
                ..default()
            },
            Transform::from_translation(*pos),
            SpriteEntity {
                monster_type: monster_name.to_string(),
            },
        ));
    }
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
            
            if distance > 5.0 { // 到达阈值
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
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        if let Ok(window) = windows.single() {
            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok((camera, camera_transform)) = camera_q.single() {
                    if let Ok(world_pos) = camera.viewport_to_world(camera_transform, cursor_pos) {
                        let world_pos: Vec2 = world_pos.origin.truncate(); // 2D
                        for (transform, sprite) in sprite_q.iter() {
                            let distance = (transform.translation.truncate() - world_pos).length();
                            if distance < 30.0 { // 点击范围
                                println!("Clicked on {}", sprite.monster_type);
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

pub fn cleanup_map(mut commands: Commands, query: Query<Entity, Or<(With<Map>, With<Character>, With<SpriteEntity>)>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}