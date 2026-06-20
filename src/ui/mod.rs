//! 战斗 UI：上敌方 / 下玩家，血条与护盾条，技能格占位图与特效。
pub(crate) mod battle;

use bevy::{prelude::*, window::PrimaryWindow};

use battle::{
    components::{BattleBackgroundSprite, BattleUiCleanupPending, BattleUiRoot},
    layout::{load_cjk_font_system, setup_ui_system, spawn_camera},
    resources::UiFontHandle,
    systems::*,
    theme::UiTheme,
};

use crate::{
    battle::{Combatant, ElementAura, InBattle, Shield, Stats, Team},
    data::{BattleDbs, CardDef, ElementType, SkillId},
    game_state::{BattlePhase, GameState},
};

use battle::fx::{
    keyboard_button_flash_system, preload_reaction_icons, process_battle_fx_events,
    spawn_button_click_flash, spawn_reaction_banner_system, tick_action_dial_keyboard_flash_system,
    tick_active_frame_pulse_system, tick_button_click_flash, tick_fx_lifetimes,
    tick_reaction_banner_system, tick_screen_flashes, tick_skill_flash_timer,
};

use battle::systems::{PendingSwitchOverlayToggle, SwitchOverlayOpen};

pub struct UiPlugin;

const BASE_UI_WIDTH: f32 = 1440.0;
const BASE_UI_HEIGHT: f32 = 900.0;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        // Initialize theme globally so it's available to all UI systems
        app.init_resource::<UiTheme>()
            .init_resource::<UiScale>()
            .add_systems(Update, update_ui_scale_system);
        battle::register(app);
    }
}

fn update_ui_scale_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut ui_scale: ResMut<UiScale>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    ui_scale.0 = (window.width() / BASE_UI_WIDTH).min(window.height() / BASE_UI_HEIGHT);
}

/// 旧版战斗 UI 注册入口（供 `ui::battle` 桥接）。
///
/// 后续重构会逐步把实现迁移进 `src/ui/battle/`，此处保持行为不变。
pub(crate) fn register_legacy_battle_ui(app: &mut App) {
    app.add_message::<battle::components::BattleUiNotice>()
        .init_resource::<SwitchOverlayOpen>()
        .init_resource::<PendingSwitchOverlayToggle>()
        .init_resource::<RetreatConfirmState>()
        .init_resource::<BattleHintOverlayState>()
        .init_resource::<HandFullEndTurnWarning>()
        .init_resource::<HandDrawAnimTracker>()
        .init_resource::<HandExitAnimTracker>()
        .add_systems(Startup, (spawn_camera, load_cjk_font_system).chain())
        .add_systems(Startup, preload_reaction_icons)
        .add_systems(OnEnter(GameState::Battle), setup_ui_system)
        .add_systems(OnEnter(GameState::Battle), reset_hand_draw_anim_tracker)
        .add_systems(OnEnter(GameState::TeamSelection), cleanup_battle_ui_system)
        .add_systems(OnEnter(GameState::Lobby), cleanup_battle_ui_system)
        .add_systems(OnEnter(GameState::Map), cleanup_battle_ui_system)
        .add_systems(PostUpdate, despawn_pending_battle_ui_system)
        .add_systems(
            Update,
            (
                button_select_skill_system
                    .run_if(in_state(GameState::Battle))
                    .run_if(crate::battle::player_action_cooldown_ready),
                button_discard_system
                    .run_if(in_state(GameState::Battle))
                    .run_if(crate::battle::player_action_cooldown_ready),
                button_switch_member_system
                    .run_if(in_state(GameState::Battle))
                    .run_if(crate::battle::player_action_cooldown_ready),
                button_play_card_two_step_system
                    .run_if(in_state(GameState::Battle))
                    .run_if(crate::battle::player_action_cooldown_ready),
                button_cancel_card_selection_system.run_if(in_state(GameState::Battle)),
                button_toggle_switch_overlay_system.run_if(in_state(GameState::Battle)),
                close_switch_overlay_on_switch_system
                    .run_if(in_state(GameState::Battle))
                    .run_if(crate::battle::player_action_cooldown_ready),
                button_battle_hint_system.run_if(in_state(GameState::Battle)),
                update_battle_hint_overlay_system
                    .run_if(in_state(GameState::Battle))
                    .after(button_battle_hint_system),
                apply_pending_switch_overlay_toggle_system
                    .run_if(in_state(GameState::Battle))
                    .after(button_toggle_switch_overlay_system)
                    .after(close_switch_overlay_on_switch_system)
                    .after(spawn_button_click_flash)
                    .after(keyboard_button_flash_system),
            ),
        );

    app.add_systems(
        Update,
        update_player_hand_ui_system.run_if(in_state(GameState::Battle)),
    );
    app.add_systems(
        Update,
        animate_hand_card_draw_system.run_if(in_state(GameState::Battle)),
    );
    app.add_systems(
        Update,
        (spawn_hand_card_exit_system, tick_hand_card_exit_system)
            .chain()
            .run_if(in_state(GameState::Battle)),
    );
    app.add_systems(
        Update,
        update_player_roster_ui_system.run_if(in_state(GameState::Battle)),
    );
    app.add_systems(
        Update,
        update_enemy_roster_ui_system.run_if(in_state(GameState::Battle)),
    );
    app.add_systems(
        Update,
        update_dead_member_name_color_system.run_if(in_state(GameState::Battle)),
    );
    app.add_systems(
        Update,
        (
            button_retreat_system.run_if(in_state(GameState::Battle)),
            button_retreat_confirm_system
                .run_if(in_state(GameState::Battle))
                .after(button_retreat_system),
            update_retreat_confirm_overlay_system
                .run_if(in_state(GameState::Battle))
                .after(button_retreat_system)
                .after(button_retreat_confirm_system),
        ),
    );

    app.add_systems(
        Update,
        (
            update_phase_text_system,
            update_active_panel_text_system,
            update_element_icon_system,
            update_portrait_images_system,
            update_active_panel_tokens_system,
            update_skill_text_system,
            update_ap_gems_system,
            tick_active_frame_pulse_system,
        )
            .run_if(in_state(GameState::Battle)),
    );

    app.add_systems(
        Update,
        (
            button_end_turn_system
                .run_if(in_state(GameState::Battle))
                .run_if(crate::battle::player_action_cooldown_ready),
            button_visual_state_system
                .run_if(in_state(GameState::Battle).or(in_state(GameState::Result))),
            hand_full_warning_visual_system
                .run_if(in_state(GameState::Battle))
                .after(button_end_turn_system)
                .before(action_dial_visual_state_system),
            action_dial_visual_state_system.run_if(in_state(GameState::Battle)),
            update_discard_armed_visual_system
                .run_if(in_state(GameState::Battle))
                .after(button_visual_state_system)
                .after(action_dial_visual_state_system)
                .before(process_battle_fx_events),
            update_battle_bars_system.run_if(in_state(GameState::Battle)),
            update_action_points_text_system.run_if(in_state(GameState::Battle)),
            update_battle_action_text_system.run_if(in_state(GameState::Battle)),
            process_battle_fx_events,
            tick_skill_flash_timer.after(process_battle_fx_events),
            tick_screen_flashes,
            tick_fx_lifetimes,
            spawn_reaction_banner_system,
            tick_reaction_banner_system.after(spawn_reaction_banner_system),
            spawn_button_click_flash
                .run_if(in_state(GameState::Battle))
                .after(update_player_roster_ui_system),
            keyboard_button_flash_system
                .run_if(in_state(GameState::Battle))
                .after(spawn_button_click_flash),
            tick_action_dial_keyboard_flash_system
                .run_if(in_state(GameState::Battle))
                .after(action_dial_visual_state_system)
                .after(keyboard_button_flash_system),
            tick_button_click_flash
                .run_if(in_state(GameState::Battle))
                .after(keyboard_button_flash_system)
                .after(tick_action_dial_keyboard_flash_system),
            battle_action_cooldown_visual_system
                .run_if(in_state(GameState::Battle))
                .after(update_player_hand_ui_system)
                .after(update_player_roster_ui_system)
                .after(button_visual_state_system)
                .after(action_dial_visual_state_system)
                .after(update_discard_armed_visual_system)
                .after(tick_button_click_flash),
            update_skill_uses_system
                .run_if(in_state(GameState::Battle))
                .after(battle_action_cooldown_visual_system),
        ),
    );

    app.add_systems(
        Update,
        update_result_ui_system.run_if(in_state(GameState::Result)),
    );
    app.add_systems(
        Update,
        update_latency_indicator_system
            .run_if(in_state(GameState::Battle).or(in_state(GameState::Result))),
    );
    app.add_systems(
        Update,
        button_result_action_system.run_if(in_state(GameState::Result)),
    )
    .add_systems(OnExit(GameState::Result), hide_result_ui_system);
}

fn cleanup_battle_ui_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Node), (With<BattleUiRoot>, Without<BattleUiCleanupPending>)>,
    background_query: Query<Entity, With<BattleBackgroundSprite>>,
    children_query: Query<&Children>,
) {
    for entity in &background_query {
        commands.entity(entity).despawn();
    }

    for (entity, mut node) in &mut query {
        node.display = Display::None;
        mark_battle_ui_cleanup_pending(&mut commands, entity, &children_query, 1);
    }
}

fn mark_battle_ui_cleanup_pending(
    commands: &mut Commands,
    entity: Entity,
    children_query: &Query<&Children>,
    frames_remaining: u8,
) {
    commands
        .entity(entity)
        .insert(BattleUiCleanupPending { frames_remaining });

    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            mark_battle_ui_cleanup_pending(commands, child, children_query, frames_remaining);
        }
    }
}

fn despawn_pending_battle_ui_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut BattleUiCleanupPending), With<BattleUiRoot>>,
    children_query: Query<&Children>,
    spine_proxy_query: Query<&bevy_spine::SpineUiProxy>,
    uninitialized_spine_nodes: Query<
        (),
        (
            With<bevy_spine::SpineUiNode>,
            Without<bevy_spine::SpineUiProxy>,
        ),
    >,
) {
    for (entity, mut pending) in &mut query {
        if has_uninitialized_spine_ui_descendant(
            entity,
            &children_query,
            &uninitialized_spine_nodes,
        ) {
            continue;
        }

        if pending.frames_remaining > 0 {
            pending.frames_remaining -= 1;
        } else {
            despawn_spine_ui_proxy_descendants(
                &mut commands,
                entity,
                &children_query,
                &spine_proxy_query,
            );
            commands.entity(entity).despawn();
        }
    }
}

fn has_uninitialized_spine_ui_descendant(
    entity: Entity,
    children_query: &Query<&Children>,
    uninitialized_spine_nodes: &Query<
        (),
        (
            With<bevy_spine::SpineUiNode>,
            Without<bevy_spine::SpineUiProxy>,
        ),
    >,
) -> bool {
    if uninitialized_spine_nodes.contains(entity) {
        return true;
    }

    children_query.get(entity).is_ok_and(|children| {
        children.iter().any(|child| {
            has_uninitialized_spine_ui_descendant(child, children_query, uninitialized_spine_nodes)
        })
    })
}

fn despawn_spine_ui_proxy_descendants(
    commands: &mut Commands,
    entity: Entity,
    children_query: &Query<&Children>,
    spine_proxy_query: &Query<&bevy_spine::SpineUiProxy>,
) {
    if let Ok(proxy) = spine_proxy_query.get(entity) {
        commands.entity(proxy.proxy_entity).despawn();
        commands.entity(proxy.camera_entity).despawn();
    }

    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            despawn_spine_ui_proxy_descendants(commands, child, children_query, spine_proxy_query);
        }
    }
}

#[allow(dead_code)]
fn skill_name(skill_id: SkillId, dbs: &BattleDbs) -> String {
    battle::helpers::skill_name(skill_id, dbs)
}

#[allow(dead_code)]
fn skill_meta(skill_id: SkillId, dbs: &BattleDbs) -> String {
    battle::helpers::skill_meta(skill_id, dbs)
}

#[allow(dead_code)]
fn monster_skill_ap_cost_ui(skill_id: SkillId, dbs: &BattleDbs) -> i32 {
    battle::helpers::monster_skill_ap_cost_ui(skill_id, dbs)
}

#[allow(dead_code)]
fn phase_label(phase: BattlePhase) -> &'static str {
    battle::helpers::phase_label(phase)
}

#[allow(dead_code)]
fn active_hp_percent(
    team: &Team,
    query: &Query<(&Combatant, &Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
) -> f32 {
    battle::helpers::active_hp_percent(team, query)
}

#[allow(dead_code)]
fn active_shield(
    team: &Team,
    query: &Query<(&Combatant, &Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
) -> i32 {
    battle::helpers::active_shield(team, query)
}

#[allow(dead_code)]
fn active_shield_percent(
    team: &Team,
    query: &Query<(&Combatant, &Stats, &Name, &Shield, &ElementAura), With<InBattle>>,
) -> f32 {
    battle::helpers::active_shield_percent(team, query)
}

#[allow(dead_code)]
fn element_name(element: ElementType) -> &'static str {
    battle::helpers::element_name(element)
}

#[allow(dead_code)]
fn aura_label(aura: &[ElementType]) -> String {
    battle::helpers::aura_label(aura)
}

#[allow(dead_code)]
fn card_hotkey_label(index: usize) -> &'static str {
    battle::helpers::card_hotkey_label(index)
}

#[allow(dead_code)]
fn card_description(card: &CardDef) -> String {
    battle::helpers::card_description(card)
}

#[allow(dead_code)]
fn make_text_font(size: f32, ui_font: Option<&UiFontHandle>) -> TextFont {
    battle::helpers::make_text_font(size, ui_font)
}
