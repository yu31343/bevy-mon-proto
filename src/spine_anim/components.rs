use bevy::prelude::*;

use crate::battle::Side;

#[derive(Component, Clone)]
pub(super) struct MonsterAnimationHandle {
    pub monster_name: String,
    pub skeleton: Handle<bevy_spine::SkeletonData>,
    pub death_fade_seconds: f32,
}

#[derive(Component, Clone, Copy)]
pub(super) struct MonsterVisual {
    pub owner: Entity,
    pub side: Side,
    pub dying: bool,
    pub facing_scale: f32,
    pub flip_y: bool,
}

#[derive(Component)]
pub(super) struct DeathFade {
    pub timer: Timer,
}

#[derive(Component)]
pub(super) struct PendingAnimationAction {
    pub animation: String,
    pub on_complete: AnimationCompleteAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AnimationCompleteAction {
    ReturnToIdle,
    StartDeathFade,
}
