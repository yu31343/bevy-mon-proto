mod combat;
mod end;
mod enemy_turn;
mod events;
mod init;
mod legacy;
mod player_turn;
mod round;
mod status;
mod text;

pub use end::{check_end_system, resolve_ko_system, restart_from_result_system};
pub use enemy_turn::enemy_turn_ai_system;
pub use events::consume_battle_events_system;
pub use init::init_battle_system;
pub use player_turn::player_turn_input_system;
pub use round::round_start_system;

pub(crate) use combat::{
    SkillExecutionMode, SkillTargetMode, WindSpreadTarget, apply_effect, apply_self_effect,
    apply_wind_effect, skill_execution_mode, skill_target_mode,
};
pub(crate) use init::abort_battle;
pub(crate) use status::{
    SideEndTickParams, process_round_end_status_durations, process_side_end_statuses,
};
pub(crate) use text::{element_text, side_text};
