use std::collections::HashMap;

use bevy::prelude::*;
use bevy_spine::{
    SkeletonData, SpineEvent, SpineLoader, SpineUiAnimation, SpineUiFit, SpineUiNode, SpineUiProxy,
    SpineUiReadyEvent, SpineUiSkeleton,
};

use crate::battle::{BattleEvent, Combatant, EnemyTeam, InBattle, PlayerTeam, Side, Stats};
use crate::ui::battle::components::BattleUiRoot;

use super::components::{
    AnimationCompleteAction, DeathFade, MonsterAnimationHandle, MonsterVisual,
    PendingAnimationAction,
};
use super::config::{
    MONSTER_NAMES, NODE_SIZE, REFERENCE_SIZE, atlas_asset_path, death_fade_seconds,
    skeleton_asset_path, skill_animation, supports_spine_animation, visual_config,
};

#[derive(Resource, Default, Clone)]
pub(super) struct MonsterAnimationLibrary(pub HashMap<String, Handle<SkeletonData>>);

pub(super) fn load_monster_skeletons(
    asset_server: Res<AssetServer>,
    mut skeletons: ResMut<Assets<SkeletonData>>,
    mut commands: Commands,
) {
    let mut library = MonsterAnimationLibrary::default();

    for monster_name in MONSTER_NAMES {
        let skeleton = SkeletonData::new_from_binary(
            asset_server.load(skeleton_asset_path(monster_name)),
            asset_server.load(atlas_asset_path(monster_name)),
        );
        let handle = skeletons.add(skeleton);
        library.0.insert(monster_name.to_string(), handle);
    }

    commands.insert_resource(library);
}

pub(super) fn spawn_monster_ui_visuals(
    mut commands: Commands,
    library: Option<Res<MonsterAnimationLibrary>>,
    ui_root: Query<Entity, With<BattleUiRoot>>,
    new_battle_units: Query<(Entity, &Name, &Combatant), Added<InBattle>>,
) {
    let Some(library) = library else {
        warn!("Spine: animation library is not ready yet.");
        return;
    };
    let Ok(root_entity) = ui_root.single() else {
        warn!("Spine: BattleUiRoot not found, skip spawning monster UI.");
        return;
    };

    for (owner, name, combatant) in &new_battle_units {
        if !supports_spine_animation(name.as_str()) {
            continue;
        }

        let Some(skeleton) = library.0.get(name.as_str()) else {
            continue;
        };

        info!(
            "Spine: spawning animation for {} ({:?})",
            name.as_str(),
            combatant.side
        );

        let (left, right, top, bottom) = match combatant.side {
            //Side::Player => (Val::Px(28.0), Val::Auto, Val::Px(196.0), Val::Auto),
            Side::Player => (Val::Px(28.0), Val::Auto, Val::Px(370.0), Val::Auto), //debug mode
            //Side::Enemy => (Val::Auto, Val::Px(28.0), Val::Px(196.0), Val::Auto),
            Side::Enemy => (Val::Auto, Val::Px(100.0), Val::Px(196.0), Val::Auto), //debug mode
        };
        let visual = visual_config(name.as_str(), combatant.side);

        commands.entity(root_entity).with_children(|root| {
            root.spawn((
                Name::new(format!("MonsterVisual({})", name.as_str())),
                InBattle,
                MonsterAnimationHandle {
                    monster_name: name.as_str().to_string(),
                    skeleton: skeleton.clone(),
                    death_fade_seconds: death_fade_seconds(name.as_str()),
                },
                MonsterVisual {
                    owner,
                    side: combatant.side,
                    dying: false,
                    facing_scale: visual.facing_scale,
                    flip_y: visual.flip_y,
                },
                Node {
                    position_type: PositionType::Absolute,
                    left,
                    right,
                    top,
                    bottom,
                    width: Val::Px(NODE_SIZE.x),
                    height: Val::Px(NODE_SIZE.y),
                    display: Display::None,
                    ..default()
                },
                SpineUiNode {
                    fit: SpineUiFit::Contain,
                    auto_size: Some(NODE_SIZE),
                    reference_size: Some(REFERENCE_SIZE),
                    offset: visual.offset,
                    scale: visual.facing_scale,
                    flip_y: visual.flip_y,
                    animation: Some(SpineUiAnimation::looping("idle_loop")),
                    ..default()
                },
                SpineUiSkeleton(skeleton.clone()),
            ));
        });
    }
}

pub(super) fn sync_active_visibility_and_facing(
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    mut visuals: Query<(&MonsterVisual, &mut Node, &mut SpineUiNode)>,
) {
    let player_active = player_team.as_ref().and_then(|t| t.0.active_combatant());
    let enemy_active = enemy_team.as_ref().and_then(|t| t.0.active_combatant());

    for (visual, mut node, mut spine_ui) in &mut visuals {
        let active_owner = match visual.side {
            Side::Player => player_active,
            Side::Enemy => enemy_active,
        };
        let should_show = visual.dying || Some(visual.owner) == active_owner;
        let new_display = if should_show {
            Display::Flex
        } else {
            Display::None
        };
        let old_display = node.display;
        node.display = new_display;

        if new_display == Display::Flex && old_display != new_display {
            spine_ui.scale = visual.facing_scale;
            spine_ui.flip_y = visual.flip_y;
        }
    }
}

fn trigger_death_animation(
    commands: &mut Commands,
    entity: Entity,
    handle: &MonsterAnimationHandle,
    visual: &mut MonsterVisual,
    node: &mut Node,
    spine_ui: &mut SpineUiNode,
) {
    if visual.dying {
        return;
    }

    visual.dying = true;
    node.display = Display::Flex;
    spine_ui.tint = Color::WHITE;
    spine_ui.animation = Some(SpineUiAnimation {
        name: "die".to_string(),
        repeat: false,
    });
    commands.entity(entity).remove::<DeathFade>();
    commands.entity(entity).insert(PendingAnimationAction {
        animation: "die".to_string(),
        on_complete: AnimationCompleteAction::StartDeathFade,
    });
}

pub(super) fn react_to_battle_events(
    mut commands: Commands,
    mut events: MessageReader<BattleEvent>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combatants: Query<&Stats, With<InBattle>>,
    mut visuals: Query<(
        Entity,
        &MonsterAnimationHandle,
        &mut MonsterVisual,
        &mut Node,
        &mut SpineUiNode,
    )>,
) {
    for event in events.read() {
        match event {
            BattleEvent::SkillUsed { side, slot, .. } => {
                for (entity, handle, visual, node, mut spine_ui) in &mut visuals {
                    if visual.side != *side || visual.dying || node.display == Display::None {
                        continue;
                    }
                    let choice = skill_animation(&handle.monster_name, *slot);
                    spine_ui.animation = Some(SpineUiAnimation {
                        name: choice.to_string(),
                        repeat: false,
                    });
                    commands.entity(entity).insert(PendingAnimationAction {
                        animation: choice.to_string(),
                        on_complete: AnimationCompleteAction::ReturnToIdle,
                    });
                }
            }
            BattleEvent::DamageDealt { target, .. } => {
                let active_owner = match target {
                    Side::Player => player_team.as_ref().and_then(|t| t.0.active_combatant()),
                    Side::Enemy => enemy_team.as_ref().and_then(|t| t.0.active_combatant()),
                };

                for (entity, handle, mut visual, mut node, mut spine_ui) in &mut visuals {
                    if visual.side != *target || visual.dying || node.display == Display::None {
                        continue;
                    }

                    let target_is_dead = Some(visual.owner) == active_owner
                        && combatants
                            .get(visual.owner)
                            .map(|stats| stats.hp <= 0)
                            .unwrap_or(false);

                    if target_is_dead {
                        trigger_death_animation(
                            &mut commands,
                            entity,
                            handle,
                            &mut visual,
                            &mut node,
                            &mut spine_ui,
                        );
                        continue;
                    }

                    spine_ui.animation = Some(SpineUiAnimation {
                        name: "hurt".to_string(),
                        repeat: false,
                    });
                    commands.entity(entity).insert(PendingAnimationAction {
                        animation: "hurt".to_string(),
                        on_complete: AnimationCompleteAction::ReturnToIdle,
                    });
                }
            }
            BattleEvent::CombatantFainted { owner, side, .. } => {
                for (entity, handle, mut visual, mut node, mut spine_ui) in &mut visuals {
                    if visual.side != *side || visual.dying || visual.owner != *owner {
                        continue;
                    }

                    trigger_death_animation(
                        &mut commands,
                        entity,
                        handle,
                        &mut visual,
                        &mut node,
                        &mut spine_ui,
                    );
                }
            }
            BattleEvent::Switched { side, .. } => {
                for (_entity, _handle, visual, _node, mut spine_ui) in &mut visuals {
                    if visual.side == *side && !visual.dying {
                        spine_ui.tint = Color::WHITE;
                        spine_ui.animation = Some(SpineUiAnimation::looping("idle_loop"));
                    }
                }
            }
            _ => {}
        }
    }
}

pub(super) fn handle_spine_animation_complete(
    mut commands: Commands,
    mut events: MessageReader<SpineEvent>,
    visuals: Query<(
        Entity,
        &MonsterAnimationHandle,
        &MonsterVisual,
        &SpineUiProxy,
        Option<&PendingAnimationAction>,
    )>,
    mut ui_nodes: Query<(&mut Node, &mut SpineUiNode)>,
) {
    for event in events.read() {
        let SpineEvent::Complete { entity, animation } = event else {
            continue;
        };

        for (ui_entity, handle, visual, proxy, pending) in &visuals {
            if proxy.proxy_entity != *entity {
                continue;
            }

            let Some(pending) = pending else {
                continue;
            };

            if pending.animation != *animation {
                continue;
            }

            let Ok((mut node, mut spine_ui)) = ui_nodes.get_mut(ui_entity) else {
                continue;
            };

            match pending.on_complete {
                AnimationCompleteAction::ReturnToIdle => {
                    if !visual.dying && node.display != Display::None {
                        spine_ui.animation = Some(SpineUiAnimation::looping("idle_loop"));
                    }
                    commands
                        .entity(ui_entity)
                        .remove::<PendingAnimationAction>();
                }
                AnimationCompleteAction::StartDeathFade => {
                    spine_ui.animation = None;
                    if spine_ui.tint != Color::WHITE {
                        spine_ui.tint = Color::WHITE;
                    }
                    commands
                        .entity(ui_entity)
                        .remove::<PendingAnimationAction>()
                        .insert(DeathFade {
                            timer: Timer::from_seconds(handle.death_fade_seconds, TimerMode::Once),
                        });
                }
            }
        }
    }
}

pub(super) fn tick_death_fade(
    mut commands: Commands,
    time: Res<Time>,
    mut dying_query: Query<(
        Entity,
        &mut MonsterVisual,
        &mut Node,
        &mut SpineUiNode,
        &mut DeathFade,
    )>,
) {
    for (entity, mut visual, mut node, mut spine_ui, mut fade) in &mut dying_query {
        fade.timer.tick(time.delta());
        let alpha = 1.0 - fade.timer.fraction();
        spine_ui.tint = Color::srgba(1.0, 1.0, 1.0, alpha);
        if fade.timer.is_finished() {
            node.display = Display::None;
            commands.entity(entity).remove::<DeathFade>();
            visual.dying = false;
            spine_ui.tint = Color::srgba(1.0, 1.0, 1.0, 1.0);
            spine_ui.animation = Some(SpineUiAnimation::looping("idle_loop"));
        }
    }
}

pub(super) fn log_spine_ui_ready_events(mut events: MessageReader<SpineUiReadyEvent>) {
    for evt in events.read() {
        info!(
            "Spine: UI node ready. ui_entity={:?}, proxy_entity={:?}",
            evt.entity, evt.proxy_entity
        );
    }
}

pub(super) fn log_spine_loader_failures(
    proxies: Query<(Entity, &SpineUiProxy, &MonsterAnimationHandle), With<MonsterVisual>>,
    proxy_loaders: Query<&SpineLoader>,
) {
    for (ui_entity, proxy, handle) in &proxies {
        if let Ok(loader) = proxy_loaders.get(proxy.proxy_entity) {
            if matches!(loader, SpineLoader::Failed) {
                warn!(
                    "Spine: loader failed for monster animation. ui_entity={:?}, proxy_entity={:?}, skeleton={:?}",
                    ui_entity, proxy.proxy_entity, handle.skeleton
                );
            }
        }
    }
}
