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

    pub fn name(self) -> &'static str {
        match self {
            Self::Map1 => "翠熔流彩溪",
            Self::Map2 => "晶光浅湾",
            Self::Map3 => "暗雷深渊",
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

#[derive(Component)]
pub struct MapNameBannerRoot;

#[derive(Component)]
pub struct MapNameBannerText;

#[derive(Component)]
pub struct MapNameBannerLine;

/// 地图名横幅动画计时器（总时长 2.0 秒：渐入 0.5 + 保持 1.0 + 渐出 0.5）。
#[derive(Resource)]
pub struct MapNameBannerTimer {
    pub timer: Timer,
}

impl Default for MapNameBannerTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(2.0, TimerMode::Once),
        }
    }
}
