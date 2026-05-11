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
    pub monster_type: String,
    pub monster_index: usize,
}

#[derive(Component)]
pub struct SpriteNameLabel;

#[derive(Component)]
pub struct MapUiRoot;

#[derive(Component)]
pub struct EnterLobbyButton;
