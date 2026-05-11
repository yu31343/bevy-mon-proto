use bevy::prelude::*;

#[derive(Component)]
pub struct Map;

#[derive(Component)]
pub struct Character {
    pub speed: f32,
    pub target_position: Option<Vec2>,
}

#[derive(Component)]
pub struct SpriteEntity {
    pub monster_type: String, // 简化为字符串，实际可扩展
}