//! 战斗 UI：上敌方 / 下玩家，血条与护盾条，技能格占位图与特效。
pub(crate) mod battle;

use bevy::{prelude::*, window::PrimaryWindow};

use battle::{
    components::BattleUiRoot,
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
    keyboard_button_flash_system, process_battle_fx_events, spawn_button_click_flash,
    tick_button_click_flash, tick_fx_lifetimes, tick_screen_flashes, tick_skill_flash_timer,
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
    app.init_resource::<SwitchOverlayOpen>()
        .init_resource::<PendingSwitchOverlayToggle>()
        .init_resource::<RetreatConfirmState>()
        .add_systems(Startup, (spawn_camera, load_cjk_font_system).chain())
        .add_systems(OnEnter(GameState::Battle), setup_ui_system)
        .add_systems(OnEnter(GameState::TeamSelection), cleanup_battle_ui_system)
        .add_systems(OnEnter(GameState::Lobby), cleanup_battle_ui_system)
        .add_systems(
            Update,
            (
                button_select_skill_system.run_if(in_state(GameState::Battle)),
                button_discard_system.run_if(in_state(GameState::Battle)),
                button_switch_member_system.run_if(in_state(GameState::Battle)),
                button_play_card_two_step_system.run_if(in_state(GameState::Battle)),
                button_toggle_switch_overlay_system.run_if(in_state(GameState::Battle)),
                close_switch_overlay_on_switch_system.run_if(in_state(GameState::Battle)),
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
        update_player_roster_ui_system.run_if(in_state(GameState::Battle)),
    );
    app.add_systems(
        Update,
        update_enemy_roster_ui_system.run_if(in_state(GameState::Battle)),
    );

    app.add_systems(
        Update,
        (
            update_phase_text_system,
            update_active_panel_text_system,
            update_active_panel_tokens_system,
            update_skill_text_system,
        )
            .run_if(in_state(GameState::Battle)),
    );

    app.add_systems(
        Update,
        (
            button_end_turn_system.run_if(in_state(GameState::Battle)),
            button_retreat_system.run_if(in_state(GameState::Battle)),
            button_visual_state_system.run_if(in_state(GameState::Battle)),
            update_discard_armed_visual_system
                .run_if(in_state(GameState::Battle))
                .after(button_visual_state_system)
                .before(process_battle_fx_events),
            update_battle_bars_system.run_if(in_state(GameState::Battle)),
            update_action_points_text_system.run_if(in_state(GameState::Battle)),
            update_battle_action_text_system.run_if(in_state(GameState::Battle)),
            process_battle_fx_events,
            tick_skill_flash_timer.after(process_battle_fx_events),
            tick_screen_flashes,
            tick_fx_lifetimes,
            spawn_button_click_flash
                .run_if(in_state(GameState::Battle))
                .after(update_player_roster_ui_system),
            keyboard_button_flash_system
                .run_if(in_state(GameState::Battle))
                .after(spawn_button_click_flash),
            tick_button_click_flash
                .run_if(in_state(GameState::Battle))
                .after(keyboard_button_flash_system),
        ),
    );

    app.add_systems(Update, update_result_ui_system);
}

fn cleanup_battle_ui_system(mut commands: Commands, query: Query<Entity, With<BattleUiRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
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
