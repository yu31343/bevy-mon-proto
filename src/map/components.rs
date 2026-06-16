use bevy::prelude::*;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentMap {
    Map1,
    Map2,
    Map3,
}

impl Default for CurrentMap {
    fn default() -> Self {
        Self::Map1
    }
}

impl CurrentMap {
    pub fn asset_path(self) -> &'static str {
        match self {
            Self::Map1 => "map/map1.png",
            Self::Map2 => "map/map2.png",
            Self::Map3 => "map/map3.png",
        }
    }
}

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
    pub click_radius: f32,
}

#[derive(Component)]
pub struct SpriteNameLabel;

#[derive(Component)]
pub struct MapSpriteOutline;

#[derive(Component)]
pub struct MapUiRoot;

#[derive(Component)]
pub struct EnterLobbyButton;

#[derive(Component)]
pub struct MapNavigationButton {
    pub target: CurrentMap,
}
