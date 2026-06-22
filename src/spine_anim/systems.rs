use std::collections::HashMap;

use bevy::prelude::*;
use bevy_spine::{
    SkeletonData, SpineEvent, SpineLoader, SpineUiAnimation, SpineUiFit, SpineUiNode, SpineUiProxy,
    SpineUiReadyEvent, SpineUiSkeleton,
};

use crate::battle::{
    BattleEvent, Combatant, EnemyTeam, InBattle, PlayerTeam, Shield, Side, Stats, StatusBoard,
};
use crate::console_log::{ConsoleLogCategory, log as console_log};
use crate::ui::battle::components::{BattleUiCleanupPending, BattleUiRoot};

use super::components::{
    AnimationCompleteAction, BattleVfx, DeathFade, MonsterAnimationHandle, MonsterVisual,
    PendingAnimationAction, PendingSpineUiDespawn, PendingVfxDespawn, PersistentCursedChainVfx,
};
use super::config::{
    MONSTER_NAMES, NODE_SIZE, REFERENCE_SIZE, VFX_ADRENALINE, VFX_BITE, VFX_CHAIN, VFX_CONFIGS,
    VFX_FLYING_SLASH, VFX_GAZE, VFX_SCRATCH, atlas_asset_path, death_fade_seconds,
    skeleton_asset_path, skill_animation, supports_spine_animation, vfx_config, visual_config,
    water_shield_animation,
};

#[derive(Resource, Default, Clone)]
pub(super) struct MonsterAnimationLibrary(pub HashMap<String, Handle<SkeletonData>>);

#[derive(Resource, Default, Clone)]
pub(super) struct VfxAnimationLibrary(pub HashMap<&'static str, Handle<SkeletonData>>);

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

    let mut vfx_library = VfxAnimationLibrary::default();
    for config in VFX_CONFIGS {
        let skeleton = SkeletonData::new_from_binary(
            asset_server.load(config.skeleton_path),
            asset_server.load(config.atlas_path),
        );
        let handle = skeletons.add(skeleton);
        vfx_library.0.insert(config.key, handle);
    }

    commands.insert_resource(library);
    commands.insert_resource(vfx_library);
}

pub(super) fn spawn_monster_ui_visuals(
    mut commands: Commands,
    library: Option<Res<MonsterAnimationLibrary>>,
    ui_root: Query<Entity, (With<BattleUiRoot>, Without<BattleUiCleanupPending>)>,
    new_battle_units: Query<(Entity, &Name, &Combatant), Added<InBattle>>,
) {
    let Some(library) = library else {
        console_log(
            ConsoleLogCategory::Spine,
            "动画库尚未就绪，跳过生成怪物动画 UI",
        );
        return;
    };
    let Ok(root_entity) = ui_root.single() else {
        console_log(
            ConsoleLogCategory::Spine,
            "未找到 BattleUiRoot，跳过生成怪物动画 UI",
        );
        return;
    };

    for (owner, name, combatant) in &new_battle_units {
        if !supports_spine_animation(name.as_str()) {
            continue;
        }

        let Some(skeleton) = library.0.get(name.as_str()) else {
            continue;
        };

        console_log(
            ConsoleLogCategory::SpineDetail,
            format!("生成怪物动画：{}（{:?}）", name.as_str(), combatant.side),
        );

        let (left, right, top, bottom) = side_node_bounds(combatant.side);
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
                    water_intangible: false,
                    water_shield_breaking: false,
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

/// 战斗重开时，上一局的怪物动画实体（仅带 `InBattle`，不带 `Combatant`）不会被
/// `init_battle_system` 清理；若其死亡动画尚未播完（`dying` 仍为真），
/// `sync_active_visibility_and_facing` 会继续显示它，导致残留到下一局界面。
/// 这里在拥有者实体已被销毁（重开时旧战斗实体被 despawn）后回收对应动画节点。
pub(super) fn despawn_stale_monster_visuals(
    mut commands: Commands,
    visuals: Query<
        (Entity, &MonsterVisual),
        (
            Without<BattleUiCleanupPending>,
            Without<PendingSpineUiDespawn>,
        ),
    >,
    combatants: Query<(), (With<Combatant>, With<InBattle>)>,
) {
    for (entity, visual) in &visuals {
        if combatants.get(visual.owner).is_err() {
            commands.entity(entity).insert(PendingSpineUiDespawn);
        }
    }
}

pub(super) fn sync_active_visibility_and_facing(
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    mut visuals: Query<
        (&MonsterVisual, &mut Node, &mut SpineUiNode),
        Without<BattleUiCleanupPending>,
    >,
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

fn side_node_bounds(side: Side) -> (Val, Val, Val, Val) {
    let shared_top = Val::Px(356.0);
    match side {
        Side::Player => (Val::Px(-200.0), Val::Auto, shared_top, Val::Auto),
        Side::Enemy => (Val::Auto, Val::Px(-200.0), shared_top, Val::Auto),
    }
}

fn opponent_side(side: Side) -> Side {
    match side {
        Side::Player => Side::Enemy,
        Side::Enemy => Side::Player,
    }
}

fn active_owner_for_side(
    side: Side,
    player_team: Option<&PlayerTeam>,
    enemy_team: Option<&EnemyTeam>,
) -> Option<Entity> {
    match side {
        Side::Player => player_team.and_then(|team| team.0.active_combatant()),
        Side::Enemy => enemy_team.and_then(|team| team.0.active_combatant()),
    }
}

fn active_shield_amount(
    side: Side,
    player_team: Option<&PlayerTeam>,
    enemy_team: Option<&EnemyTeam>,
    shields: &Query<&Shield, With<InBattle>>,
) -> i32 {
    active_owner_for_side(side, player_team, enemy_team)
        .and_then(|owner| shields.get(owner).ok())
        .map(|shield| shield.0)
        .unwrap_or(0)
}

fn active_side_has_status(
    side: Side,
    status_id: &str,
    player_team: Option<&PlayerTeam>,
    enemy_team: Option<&EnemyTeam>,
    statuses: &Query<&StatusBoard, With<InBattle>>,
) -> bool {
    active_owner_for_side(side, player_team, enemy_team)
        .and_then(|owner| statuses.get(owner).ok())
        .map(|board| board.entries.iter().any(|entry| entry.id == status_id))
        .unwrap_or(false)
}

fn idle_animation(handle: &MonsterAnimationHandle, visual: &MonsterVisual) -> &'static str {
    if handle.monster_name == "水精灵" && visual.water_intangible {
        "intangible_loop"
    } else {
        "idle_loop"
    }
}

fn resolve_monster_animation<'a>(
    handle: &MonsterAnimationHandle,
    visual: &MonsterVisual,
    animation: &'a str,
) -> &'a str {
    if handle.monster_name == "水精灵" && visual.water_intangible {
        water_shield_animation(animation)
    } else {
        animation
    }
}

fn play_one_shot(
    commands: &mut Commands,
    entity: Entity,
    spine_ui: &mut SpineUiNode,
    animation: &str,
    on_complete: AnimationCompleteAction,
) {
    spine_ui.animation = Some(SpineUiAnimation {
        name: animation.to_string(),
        repeat: false,
    });
    commands.entity(entity).insert(PendingAnimationAction {
        animation: animation.to_string(),
        on_complete,
    });
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

    let animation = resolve_monster_animation(handle, visual, "die");
    visual.dying = true;
    visual.water_shield_breaking = false;
    node.display = Display::Flex;
    spine_ui.tint = Color::WHITE;
    play_one_shot(
        commands,
        entity,
        spine_ui,
        animation,
        AnimationCompleteAction::StartDeathFade,
    );
}

fn spawn_battle_vfx(
    commands: &mut Commands,
    root_entity: Option<Entity>,
    library: Option<&VfxAnimationLibrary>,
    key: &'static str,
    side: Side,
    persistent_chain_side: Option<Side>,
) {
    let Some(root_entity) = root_entity else {
        return;
    };
    let Some(library) = library else {
        return;
    };
    let Some(config) = vfx_config(key) else {
        return;
    };
    let Some(skeleton) = library.0.get(config.key) else {
        console_log(
            ConsoleLogCategory::Spine,
            format!("VFX 骨骼资源未加载：{}", config.key),
        );
        return;
    };

    let (left, right, top, bottom) = side_node_bounds(side);
    let mirror_vfx = config.mirror_by_default ^ (config.mirror_on_enemy && side == Side::Enemy);

    commands.entity(root_entity).with_children(|root| {
        let mut spawned = root.spawn((
            Name::new(format!("BattleVfx({})", config.key)),
            InBattle,
            BattleVfx,
            Node {
                position_type: PositionType::Absolute,
                left,
                right,
                top,
                bottom,
                width: Val::Px(NODE_SIZE.x),
                height: Val::Px(NODE_SIZE.y),
                display: Display::Flex,
                ..default()
            },
            SpineUiNode {
                fit: SpineUiFit::Contain,
                auto_size: Some(NODE_SIZE),
                reference_size: Some(REFERENCE_SIZE),
                offset: config.offset,
                scale: if mirror_vfx { -0.9 } else { 0.9 },
                flip_y: mirror_vfx,
                animation: Some(if config.repeat {
                    SpineUiAnimation::looping(config.animation)
                } else {
                    SpineUiAnimation {
                        name: config.animation.to_string(),
                        repeat: false,
                    }
                }),
                ..default()
            },
            SpineUiSkeleton(skeleton.clone()),
        ));

        if !config.repeat {
            spawned.insert(PendingVfxDespawn {
                animation: config.animation.to_string(),
            });
        }
        if let Some(target_side) = persistent_chain_side {
            spawned.insert(PersistentCursedChainVfx { target_side });
        }
    });
}

pub(super) fn react_to_battle_events(
    mut commands: Commands,
    mut events: MessageReader<BattleEvent>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combatants: Query<&Stats, With<InBattle>>,
    shields: Query<&Shield, With<InBattle>>,
    vfx_library: Option<Res<VfxAnimationLibrary>>,
    ui_root: Query<Entity, (With<BattleUiRoot>, Without<BattleUiCleanupPending>)>,
    mut visuals: Query<
        (
            Entity,
            &MonsterAnimationHandle,
            &mut MonsterVisual,
            &mut Node,
            &mut SpineUiNode,
        ),
        Without<BattleUiCleanupPending>,
    >,
) {
    let root_entity = ui_root.single().ok();
    let player_team_ref = player_team.as_deref();
    let enemy_team_ref = enemy_team.as_deref();
    let vfx_library_ref = vfx_library.as_deref();

    for event in events.read() {
        match event {
            BattleEvent::SkillUsed {
                side,
                skill_name,
                slot,
                ..
            } => {
                match skill_name.as_str() {
                    "燃魂强化" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_ADRENALINE,
                        *side,
                        None,
                    ),
                    "导电脉冲" | "万钧雷霆" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_ADRENALINE,
                        *side,
                        None,
                    ),
                    "诅咒低语" | "恶魔低语" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_GAZE,
                        opponent_side(*side),
                        None,
                    ),
                    "圣辉裁决" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_GAZE,
                        opponent_side(*side),
                        None,
                    ),
                    "气旋撕裂" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_SCRATCH,
                        opponent_side(*side),
                        None,
                    ),
                    "光刃斩" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_FLYING_SLASH,
                        opponent_side(*side),
                        None,
                    ),
                    "逆流碾压" | "激浪冲击" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_FLYING_SLASH,
                        opponent_side(*side),
                        None,
                    ),
                    "风刃" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_FLYING_SLASH,
                        opponent_side(*side),
                        None,
                    ),
                    "烈焰风暴" | "爆裂引燃" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_FLYING_SLASH,
                        opponent_side(*side),
                        None,
                    ),
                    "草藤抽击" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_SCRATCH,
                        opponent_side(*side),
                        None,
                    ),
                    "缠绕之藤" | "生命绽放" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_BITE,
                        opponent_side(*side),
                        None,
                    ),
                    "水刃" => spawn_battle_vfx(
                        &mut commands,
                        root_entity,
                        vfx_library_ref,
                        VFX_BITE,
                        opponent_side(*side),
                        None,
                    ),
                    _ => {}
                }

                for (entity, handle, visual, node, mut spine_ui) in &mut visuals {
                    if visual.side != *side || visual.dying || node.display == Display::None {
                        continue;
                    }
                    let base_animation = skill_animation(&handle.monster_name, skill_name, *slot);
                    let choice = resolve_monster_animation(handle, &visual, base_animation);
                    play_one_shot(
                        &mut commands,
                        entity,
                        &mut spine_ui,
                        choice,
                        AnimationCompleteAction::ReturnToIdle,
                    );
                }
            }
            BattleEvent::ShieldGained { side, amount } => {
                if *amount <= 0 {
                    continue;
                }
                let active_owner = active_owner_for_side(*side, player_team_ref, enemy_team_ref);
                let shield_amount =
                    active_shield_amount(*side, player_team_ref, enemy_team_ref, &shields);
                if shield_amount <= 0 {
                    continue;
                }

                for (_entity, handle, mut visual, node, mut spine_ui) in &mut visuals {
                    if visual.side != *side
                        || visual.dying
                        || Some(visual.owner) != active_owner
                        || handle.monster_name != "水精灵"
                    {
                        continue;
                    }

                    visual.water_intangible = true;
                    if !visual.water_shield_breaking
                        && node.display != Display::None
                        && spine_ui
                            .animation
                            .as_ref()
                            .map(|animation| animation.repeat)
                            .unwrap_or(false)
                    {
                        spine_ui.animation =
                            Some(SpineUiAnimation::looping(idle_animation(handle, &visual)));
                    }
                }
            }
            BattleEvent::ShieldAbsorbed { side, amount } => {
                if *amount <= 0 {
                    continue;
                }
                let active_owner = active_owner_for_side(*side, player_team_ref, enemy_team_ref);
                let shield_amount =
                    active_shield_amount(*side, player_team_ref, enemy_team_ref, &shields);
                if shield_amount > 0 {
                    continue;
                }

                for (entity, handle, mut visual, node, mut spine_ui) in &mut visuals {
                    if visual.side != *side
                        || visual.dying
                        || node.display == Display::None
                        || Some(visual.owner) != active_owner
                        || handle.monster_name != "水精灵"
                        || !visual.water_intangible
                    {
                        continue;
                    }

                    visual.water_intangible = false;
                    visual.water_shield_breaking = true;
                    play_one_shot(
                        &mut commands,
                        entity,
                        &mut spine_ui,
                        "intangible_end",
                        AnimationCompleteAction::ReturnToIdle,
                    );
                }
            }
            BattleEvent::DamageDealt { target, .. } => {
                let active_owner = active_owner_for_side(*target, player_team_ref, enemy_team_ref);

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

                    if visual.water_shield_breaking {
                        continue;
                    }

                    let choice = resolve_monster_animation(handle, &visual, "hurt");
                    play_one_shot(
                        &mut commands,
                        entity,
                        &mut spine_ui,
                        choice,
                        AnimationCompleteAction::ReturnToIdle,
                    );
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
                let active_owner = active_owner_for_side(*side, player_team_ref, enemy_team_ref);
                let shield_amount =
                    active_shield_amount(*side, player_team_ref, enemy_team_ref, &shields);
                for (_entity, handle, mut visual, _node, mut spine_ui) in &mut visuals {
                    if visual.side == *side && !visual.dying {
                        visual.water_shield_breaking = false;
                        visual.water_intangible = handle.monster_name == "水精灵"
                            && Some(visual.owner) == active_owner
                            && shield_amount > 0;
                        spine_ui.tint = Color::WHITE;
                        spine_ui.animation =
                            Some(SpineUiAnimation::looping(idle_animation(handle, &visual)));
                    }
                }
            }
            _ => {}
        }
    }
}

pub(super) fn sync_cursed_chain_vfx(
    mut commands: Commands,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    statuses: Query<&StatusBoard, With<InBattle>>,
    vfx_library: Option<Res<VfxAnimationLibrary>>,
    ui_root: Query<Entity, (With<BattleUiRoot>, Without<BattleUiCleanupPending>)>,
    chains: Query<(Entity, &PersistentCursedChainVfx), Without<BattleUiCleanupPending>>,
) {
    let player_team_ref = player_team.as_deref();
    let enemy_team_ref = enemy_team.as_deref();
    let player_cursed = active_side_has_status(
        Side::Player,
        "cursed",
        player_team_ref,
        enemy_team_ref,
        &statuses,
    );
    let enemy_cursed = active_side_has_status(
        Side::Enemy,
        "cursed",
        player_team_ref,
        enemy_team_ref,
        &statuses,
    );

    for (entity, chain) in &chains {
        let should_keep = match chain.target_side {
            Side::Player => player_cursed,
            Side::Enemy => enemy_cursed,
        };
        if !should_keep {
            commands.entity(entity).insert(PendingSpineUiDespawn);
        }
    }

    let root_entity = ui_root.single().ok();
    let vfx_library_ref = vfx_library.as_deref();
    for (side, should_exist) in [(Side::Player, player_cursed), (Side::Enemy, enemy_cursed)] {
        if !should_exist {
            continue;
        }
        let already_exists = chains.iter().any(|(_, chain)| chain.target_side == side);
        if already_exists {
            continue;
        }
        spawn_battle_vfx(
            &mut commands,
            root_entity,
            vfx_library_ref,
            VFX_CHAIN,
            side,
            Some(side),
        );
    }
}

pub(super) fn handle_spine_animation_complete(
    mut commands: Commands,
    mut events: MessageReader<SpineEvent>,
    mut visuals: Query<
        (
            Entity,
            &MonsterAnimationHandle,
            &mut MonsterVisual,
            &SpineUiProxy,
            Option<&PendingAnimationAction>,
        ),
        Without<BattleUiCleanupPending>,
    >,
    mut ui_nodes: Query<(&mut Node, &mut SpineUiNode)>,
) {
    for event in events.read() {
        let SpineEvent::Complete { entity, animation } = event else {
            continue;
        };

        for (ui_entity, handle, mut visual, proxy, pending) in &mut visuals {
            if proxy.proxy_entity != *entity {
                continue;
            }

            let Some(pending) = pending else {
                continue;
            };

            if pending.animation != *animation {
                continue;
            }

            let Ok((node, mut spine_ui)) = ui_nodes.get_mut(ui_entity) else {
                continue;
            };

            match pending.on_complete {
                AnimationCompleteAction::ReturnToIdle => {
                    if pending.animation == "intangible_end" {
                        visual.water_shield_breaking = false;
                    }
                    if !visual.dying && node.display != Display::None {
                        spine_ui.animation =
                            Some(SpineUiAnimation::looping(idle_animation(handle, &visual)));
                    }
                    commands
                        .entity(ui_entity)
                        .remove::<PendingAnimationAction>();
                }
                AnimationCompleteAction::StartDeathFade => {
                    visual.water_shield_breaking = false;
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

pub(super) fn handle_vfx_animation_complete(
    mut commands: Commands,
    mut events: MessageReader<SpineEvent>,
    vfx_nodes: Query<
        (Entity, &SpineUiProxy, &PendingVfxDespawn),
        (With<BattleVfx>, Without<BattleUiCleanupPending>),
    >,
) {
    for event in events.read() {
        let SpineEvent::Complete { entity, animation } = event else {
            continue;
        };

        for (ui_entity, proxy, pending) in &vfx_nodes {
            if proxy.proxy_entity == *entity && pending.animation == *animation {
                commands.entity(ui_entity).insert(PendingSpineUiDespawn);
            }
        }
    }
}

pub(super) fn despawn_ready_spine_ui_nodes(
    mut commands: Commands,
    query: Query<
        (Entity, Option<&SpineUiProxy>),
        (
            With<PendingSpineUiDespawn>,
            Or<(With<SpineUiProxy>, Without<SpineUiNode>)>,
        ),
    >,
) {
    for (entity, proxy) in &query {
        if let Some(proxy) = proxy {
            commands.entity(proxy.proxy_entity).despawn();
            commands.entity(proxy.camera_entity).despawn();
        }
        commands.entity(entity).despawn();
    }
}

pub(super) fn tick_death_fade(
    mut commands: Commands,
    time: Res<Time>,
    mut dying_query: Query<
        (
            Entity,
            &mut MonsterVisual,
            &mut Node,
            &mut SpineUiNode,
            &mut DeathFade,
        ),
        Without<BattleUiCleanupPending>,
    >,
) {
    for (entity, mut visual, mut node, mut spine_ui, mut fade) in &mut dying_query {
        fade.timer.tick(time.delta());
        let alpha = 1.0 - fade.timer.fraction();
        spine_ui.tint = Color::srgba(1.0, 1.0, 1.0, alpha);
        if fade.timer.is_finished() {
            node.display = Display::None;
            commands.entity(entity).remove::<DeathFade>();
            visual.dying = false;
            visual.water_intangible = false;
            visual.water_shield_breaking = false;
            spine_ui.tint = Color::srgba(1.0, 1.0, 1.0, 1.0);
            spine_ui.animation = Some(SpineUiAnimation::looping("idle_loop"));
        }
    }
}

pub(super) fn log_spine_ui_ready_events(mut events: MessageReader<SpineUiReadyEvent>) {
    for evt in events.read() {
        console_log(
            ConsoleLogCategory::SpineDetail,
            format!(
                "UI 节点就绪：ui_entity={:?}，proxy_entity={:?}",
                evt.entity, evt.proxy_entity
            ),
        );
    }
}

pub(super) fn log_spine_loader_failures(
    proxies: Query<(Entity, &SpineUiProxy, &MonsterAnimationHandle), With<MonsterVisual>>,
    proxy_loaders: Query<&SpineLoader>,
) {
    for (ui_entity, proxy, handle) in &proxies {
        if let Ok(loader) = proxy_loaders.get(proxy.proxy_entity)
            && matches!(loader, SpineLoader::Failed)
        {
            console_log(
                ConsoleLogCategory::Spine,
                format!(
                    "怪物动画加载失败：ui_entity={:?}，proxy_entity={:?}，skeleton={:?}",
                    ui_entity, proxy.proxy_entity, handle.skeleton
                ),
            );
        }
    }
}
