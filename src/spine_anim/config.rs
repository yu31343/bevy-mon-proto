use bevy::prelude::*;

use crate::battle::Side;

pub(super) const MONSTER_NAMES: [&str; 7] = [
    "水精灵",
    "草精灵",
    "火精灵",
    "暗精灵",
    "光精灵",
    "雷精灵",
    "风精灵",
];

pub(super) const NODE_SIZE: Vec2 = Vec2::new(1024.0, 360.0);
pub(super) const REFERENCE_SIZE: Vec2 = Vec2::new(887.0, 768.0);

#[derive(Clone, Copy, Debug)]
pub(super) struct MonsterVisualConfig {
    pub facing_scale: f32,
    pub flip_y: bool,
    pub offset: Vec2,
}

pub(super) fn supports_spine_animation(name: &str) -> bool {
    MONSTER_NAMES.contains(&name)
}

pub(super) fn skeleton_asset_path(name: &str) -> String {
    format!("animation/{name}/{name}.skel")
}

pub(super) fn atlas_asset_path(name: &str) -> String {
    format!("animation/{name}/{name}.atlas")
}

pub(super) fn visual_config(name: &str, side: Side) -> MonsterVisualConfig {
    match (name, side) {
        ("火精灵", Side::Player) => MonsterVisualConfig {
            facing_scale: 0.66,
            flip_y: false,
            offset: Vec2::new(-400.0, -28.0),
        },
        ("火精灵", Side::Enemy) => MonsterVisualConfig {
            facing_scale: -0.66,
            flip_y: true,
            offset: Vec2::new(400.0, -28.0),
        },
        (_, Side::Player) => MonsterVisualConfig {
            facing_scale: -0.88,
            flip_y: true,
            offset: Vec2::new(0.0, -16.0),
        },
        (_, Side::Enemy) => MonsterVisualConfig {
            facing_scale: 0.88,
            flip_y: false,
            offset: Vec2::new(0.0, -16.0),
        },
    }
}

pub(super) fn skill_animation(monster_name: &str, slot: usize) -> &'static str {
    if slot <= 1 {
        return "attack";
    }

    match monster_name {
        "火精灵" => "cast",
        _ => "hiss",
    }
}

pub(super) fn death_fade_seconds(_name: &str) -> f32 {
    0.85
}
