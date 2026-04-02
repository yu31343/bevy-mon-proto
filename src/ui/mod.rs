//! 战斗 UI：上敌方 / 下玩家，血条与护盾条，技能格占位图与特效。
pub(crate) mod battle;

use bevy::prelude::*;

use battle::{
    layout::{load_cjk_font_system, setup_ui_system, spawn_camera},
    resources::UiFontHandle,
    systems::*,
    theme::UiTheme,
};

use crate::{
    battle::{Team, Combatant, ElementAura, InBattle, Shield, Stats},
    data::{BattleDbs, CardDef, ElementType, SkillId},
    game_state::{BattlePhase, GameState},
};

use battle::fx::{
    keyboard_button_flash_system, process_battle_fx_events, spawn_button_click_flash,
    tick_button_click_flash, tick_fx_lifetimes, tick_screen_flashes, tick_skill_flash_timer,
};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        battle::register(app);
    }
}

/// 旧版战斗 UI 注册入口（供 `ui::battle` 桥接）。
///
/// 后续重构会逐步把实现迁移进 `src/ui/battle/`，此处保持行为不变。
pub(crate) fn register_legacy_battle_ui(app: &mut App) {
    app.init_resource::<UiTheme>()
        .add_systems(Startup, (spawn_camera, load_cjk_font_system, setup_ui_system).chain())
        .add_systems(
            Update,
            (
                button_select_skill_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn))),
                button_discard_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn))),
                button_switch_member_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn))),
                button_play_card_two_step_system
                    .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn))),
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
        update_battle_text_system.run_if(in_state(GameState::Battle)),
    );

    app.add_systems(
        Update,
        (
            button_end_turn_system
                .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn))),
            button_visual_state_system.run_if(in_state(GameState::Battle)),
            update_battle_bars_system.run_if(in_state(GameState::Battle)),
            update_action_points_text_system.run_if(in_state(GameState::Battle)),
            update_battle_action_text_system.run_if(in_state(GameState::Battle)),
            process_battle_fx_events,
            tick_skill_flash_timer.after(process_battle_fx_events),
            tick_screen_flashes,
            tick_fx_lifetimes,
            spawn_button_click_flash
                .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn)))
                .after(update_player_roster_ui_system),
            keyboard_button_flash_system
                .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerTurn)))
                .after(spawn_button_click_flash),
            tick_button_click_flash
                .run_if(in_state(GameState::Battle))
                .after(keyboard_button_flash_system),
        ),
    );

    app.add_systems(Update, update_result_ui_system);
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
fn monster_skill_ap_cost_ui(slot: usize) -> i32 {
    battle::helpers::monster_skill_ap_cost_ui(slot)
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
fn aura_label(aura: Option<ElementType>) -> &'static str {
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