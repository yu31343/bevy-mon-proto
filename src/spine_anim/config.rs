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
pub(super) const REFERENCE_SIZE: Vec2 = Vec2::new(1387.0, 1268.0);

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

#[derive(Clone, Copy, Debug)]
struct MonsterVisualTuning {
    name: &'static str,
    player: MonsterVisualConfig,
    enemy: MonsterVisualConfig,
}

const DEFAULT_PLAYER_VISUAL: MonsterVisualConfig = MonsterVisualConfig {
    facing_scale: -0.88,
    flip_y: true,
    offset: Vec2::new(0.0, 0.0),
};

const DEFAULT_ENEMY_VISUAL: MonsterVisualConfig = MonsterVisualConfig {
    facing_scale: 0.88,
    flip_y: false,
    offset: Vec2::new(0.0, 0.0),
};

const MONSTER_VISUAL_TUNINGS: [MonsterVisualTuning; 7] = [
    MonsterVisualTuning {
        name: "水精灵",
        player: MonsterVisualConfig {
            facing_scale: -0.5,
            flip_y: true,
            offset: Vec2::new(0.0, 0.0),
        },
        enemy: MonsterVisualConfig {
            facing_scale: 0.5,
            flip_y: false,
            offset: Vec2::new(0.0, 0.0),
        },
    },
    MonsterVisualTuning {
        name: "草精灵",
        player: DEFAULT_PLAYER_VISUAL,
        enemy: DEFAULT_ENEMY_VISUAL,
    },
    MonsterVisualTuning {
        name: "火精灵",
        player: MonsterVisualConfig {
            facing_scale: 0.66,
            flip_y: false,
            offset: Vec2::new(0.0, 0.0),
        },
        enemy: MonsterVisualConfig {
            facing_scale: -0.66,
            flip_y: true,
            offset: Vec2::new(0.0, 0.0),
        },
    },
    MonsterVisualTuning {
        name: "暗精灵",
        player: MonsterVisualConfig {
            facing_scale: -0.7,
            flip_y: true,
            offset: Vec2::new(0.0, 0.0),
        },
        enemy: MonsterVisualConfig {
            facing_scale: 0.7,
            flip_y: false,
            offset: Vec2::new(0.0, 0.0),
        },
    },
    MonsterVisualTuning {
        name: "光精灵",
        player: MonsterVisualConfig {
            facing_scale: 0.66,
            flip_y: false,
            offset: Vec2::new(0.0, 0.0),
        },
        enemy: MonsterVisualConfig {
            facing_scale: -0.66,
            flip_y: true,
            offset: Vec2::new(0.0, 0.0),
        },
    },
    MonsterVisualTuning {
        name: "雷精灵",
        player: MonsterVisualConfig {
            facing_scale: -0.35,
            flip_y: true,
            offset: Vec2::new(0.0, 0.0),
        },
        enemy: MonsterVisualConfig {
            facing_scale: 0.35,
            flip_y: false,
            offset: Vec2::new(0.0, 0.0),
        },
    },
    MonsterVisualTuning {
        name: "风精灵",
        player: MonsterVisualConfig {
            facing_scale: 0.66,
            flip_y: false,
            offset: Vec2::new(0.0, 0.0),
        },
        enemy: MonsterVisualConfig {
            facing_scale: -0.66,
            flip_y: true,
            offset: Vec2::new(0.0, 0.0),
        },
    },
];

pub(super) fn visual_config(name: &str, side: Side) -> MonsterVisualConfig {
    let tuning = MONSTER_VISUAL_TUNINGS
        .iter()
        .find(|tuning| tuning.name == name);

    match (tuning, side) {
        (Some(tuning), Side::Player) => tuning.player,
        (Some(tuning), Side::Enemy) => tuning.enemy,
        (None, Side::Player) => DEFAULT_PLAYER_VISUAL,
        (None, Side::Enemy) => DEFAULT_ENEMY_VISUAL,
    }
}

pub(super) fn skill_animation(monster_name: &str, skill_name: &str, slot: usize) -> &'static str {
    match monster_name {
        "水精灵" => match skill_name {
            "水刃" => "attack_heavy",
            "水幕屏障" => "beckon",
            _ => "attack_heavy",
        },
        "草精灵" => {
            if slot <= 1 {
                "attack"
            } else {
                "hiss"
            }
        }
        "火精灵" => {
            if slot <= 1 {
                "attack"
            } else {
                "cast"
            }
        }
        "暗精灵" => match skill_name {
            "暗影刃" | "噬血之触" => "attack",
            _ => "cast",
        },
        "光精灵" => match skill_name {
            "光刃斩" => "attack",
            "圣辉裁决" => "attack_sovereign",
            _ => "cast",
        },
        "雷精灵" => match skill_name {
            "闪击" => "attack",
            _ => "cast",
        },
        "风精灵" => match skill_name {
            "风刃" => "shiv",
            "疾风闪避" => "cast",
            _ => "attack",
        },
        _ => "attack",
    }
}

pub(super) fn water_shield_animation(animation: &str) -> &str {
    match animation {
        "idle_loop" => "intangible_loop",
        "hurt" => "hurt_intangible",
        "die" => "die_intangible",
        _ => animation,
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SpineVfxConfig {
    pub key: &'static str,
    pub skeleton_path: &'static str,
    pub atlas_path: &'static str,
    pub animation: &'static str,
    pub repeat: bool,
    pub mirror_by_default: bool,
    pub mirror_on_enemy: bool,
}

pub(super) const VFX_ADRENALINE: &str = "vfx_adrenaline";
pub(super) const VFX_GAZE: &str = "vfx_gaze";
pub(super) const VFX_CHAIN: &str = "vfx_chain";
pub(super) const VFX_SCRATCH: &str = "vfx_scratch";
pub(super) const VFX_FLYING_SLASH: &str = "vfx_flying_slash";
pub(super) const VFX_BITE: &str = "vfx_bite";

pub(super) const VFX_CONFIGS: [SpineVfxConfig; 6] = [
    SpineVfxConfig {
        key: VFX_ADRENALINE,
        skeleton_path: "animation/vfx/vfx_adrenaline/adrenaline.skel",
        atlas_path: "animation/vfx/vfx_adrenaline/adrenaline.atlas",
        animation: "adrenaline",
        repeat: false,
        mirror_by_default: false,
        mirror_on_enemy: false,
    },
    SpineVfxConfig {
        key: VFX_GAZE,
        skeleton_path: "animation/vfx/vfx_gaze/gaze.skel",
        atlas_path: "animation/vfx/vfx_gaze/gaze.atlas",
        animation: "gaze",
        repeat: false,
        mirror_by_default: false,
        mirror_on_enemy: false,
    },
    SpineVfxConfig {
        key: VFX_CHAIN,
        skeleton_path: "animation/vfx/vfx_chain/chain.skel",
        atlas_path: "animation/vfx/vfx_chain/chain.atlas",
        animation: "chain",
        repeat: true,
        mirror_by_default: false,
        mirror_on_enemy: false,
    },
    SpineVfxConfig {
        key: VFX_SCRATCH,
        skeleton_path: "animation/vfx/vfx_scratch/scratch.skel",
        atlas_path: "animation/vfx/vfx_scratch/scratch.atlas",
        animation: "scratch",
        repeat: false,
        mirror_by_default: false,
        mirror_on_enemy: true,
    },
    SpineVfxConfig {
        key: VFX_FLYING_SLASH,
        skeleton_path: "animation/vfx/vfx_flying_slash/flying_slash.skel",
        atlas_path: "animation/vfx/vfx_flying_slash/flying_slash.atlas",
        animation: "flying_slash",
        repeat: false,
        mirror_by_default: true,
        mirror_on_enemy: true,
    },
    SpineVfxConfig {
        key: VFX_BITE,
        skeleton_path: "animation/vfx/vfx_bite/attackbite.skel",
        atlas_path: "animation/vfx/vfx_bite/attackbite.atlas",
        animation: "bite",
        repeat: false,
        mirror_by_default: false,
        mirror_on_enemy: true,
    },
];

pub(super) fn vfx_config(key: &str) -> Option<&'static SpineVfxConfig> {
    VFX_CONFIGS.iter().find(|config| config.key == key)
}

pub(super) fn death_fade_seconds(_name: &str) -> f32 {
    0.85
}
