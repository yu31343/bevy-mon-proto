use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConsoleLogCategory {
    Battle,
    BattleDebug,
    Cards,
    CardsDetail,
    Data,
    Pvp,
    PvpDetail,
    Ai,
    AiDetail,
    Selection,
    Map,
    Formula,
    Status,
    Trace,
    State,
    Result,
}

impl ConsoleLogCategory {
    fn label(self) -> &'static str {
        match self {
            Self::Battle => "battle",
            Self::BattleDebug => "battle-debug",
            Self::Cards => "cards",
            Self::CardsDetail => "cards",
            Self::Data => "data",
            Self::Pvp => "pvp",
            Self::PvpDetail => "pvp",
            Self::Ai => "ai",
            Self::AiDetail => "ai",
            Self::Selection => "selection",
            Self::Map => "map",
            Self::Formula => "formula",
            Self::Status => "status",
            Self::Trace => "trace",
            Self::State => "state",
            Self::Result => "result",
        }
    }

    fn is_enabled(self) -> bool {
        let settings = settings();
        match self {
            Self::Battle
            | Self::Cards
            | Self::Data
            | Self::Pvp
            | Self::Ai
            | Self::Selection
            | Self::Map
            | Self::Result => true,
            Self::BattleDebug => settings.battle_debug,
            Self::CardsDetail => settings.cards_detail,
            Self::PvpDetail => settings.pvp_detail,
            Self::AiDetail => settings.ai_detail,
            Self::Formula | Self::Status | Self::Trace | Self::State => settings.battle_debug,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConsoleLogSettings {
    pub battle_debug: bool,
    pub cards_detail: bool,
    pub pvp_detail: bool,
    pub ai_detail: bool,
}

impl ConsoleLogSettings {
    fn from_env() -> Self {
        let debug = env_flag("BEVY_MON_LOG_DEBUG");
        Self {
            battle_debug: debug || env_flag("BEVY_MON_LOG_BATTLE_DEBUG"),
            cards_detail: debug || env_flag("BEVY_MON_LOG_CARDS"),
            pvp_detail: debug || env_flag("BEVY_MON_LOG_PVP"),
            ai_detail: debug || env_flag("BEVY_MON_LOG_AI"),
        }
    }
}

static SETTINGS: OnceLock<ConsoleLogSettings> = OnceLock::new();

pub fn settings() -> &'static ConsoleLogSettings {
    SETTINGS.get_or_init(ConsoleLogSettings::from_env)
}

pub fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub fn log(category: ConsoleLogCategory, message: impl AsRef<str>) {
    if category.is_enabled() {
        println!("[{}] {}", category.label(), message.as_ref());
    }
}

pub fn log_with_prefix(
    category: ConsoleLogCategory,
    prefix: impl AsRef<str>,
    message: impl AsRef<str>,
) {
    if category.is_enabled() {
        println!(
            "[{}]{} {}",
            category.label(),
            prefix.as_ref(),
            message.as_ref()
        );
    }
}

pub fn log_enabled(category: ConsoleLogCategory) -> bool {
    category.is_enabled()
}
