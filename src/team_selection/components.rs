use bevy::prelude::*;

use crate::data::{
    AiDifficulty, EnemyAiConfig, EnemyAiPolicyFallback, EnemyAiPolicyMode, EnemyAiPresets,
};

/// Root marker for the team selection UI.
#[derive(Component)]
pub struct SelectionUiRoot;

/// Marker for clickable monster card buttons.
#[derive(Component)]
pub struct MonsterCardButton {
    pub monster_index: usize,
}

/// Visual indicator (order number) shown when a monster is selected.
#[derive(Component)]
pub struct MonsterCardSelectionIndicator {
    pub monster_index: usize,
}

/// Text showing the selection order (1/2/3) inside the indicator.
#[derive(Component)]
pub struct SelectionOrderText {
    pub monster_index: usize,
}

/// Confirm button to proceed to battle.
#[derive(Component)]
pub struct ConfirmSelectionButton;

#[derive(Component)]
pub struct ConfirmSelectionButtonText;

/// Back button to return to lobby.
#[derive(Component)]
pub struct BackToLobbyButton;

#[derive(Component)]
pub struct BackToLobbyButtonText;

#[derive(Component)]
pub struct AiDifficultySelectorRoot;

#[derive(Component)]
pub struct AiDifficultyButton {
    pub difficulty: AiDifficulty,
}

#[derive(Component)]
pub struct AiDifficultyButtonText {
    pub difficulty: AiDifficulty,
}

#[derive(Component)]
pub struct AiDifficultySummaryText;

#[derive(Component)]
pub struct AiPolicyButton {
    pub mode: EnemyAiPolicyMode,
}

#[derive(Component)]
pub struct AiPolicyButtonText {
    pub mode: EnemyAiPolicyMode,
}

#[derive(Component)]
pub struct AiPolicySummaryText;

#[derive(Component)]
pub struct SelectionTitleText;

#[derive(Component)]
pub struct SelectionInstructionsText;

/// Text showing "已选择: X / 3".
#[derive(Component)]
pub struct SelectionCountText;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionEntryMode {
    #[default]
    VsAi,
    Debug,
    Pvp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionStage {
    #[default]
    Player,
    Enemy,
}

/// Resource tracking the current selection state.
#[derive(Resource, Debug, Clone, Copy)]
pub struct SelectedAiDifficulty {
    pub difficulty: AiDifficulty,
}

impl Default for SelectedAiDifficulty {
    fn default() -> Self {
        Self {
            difficulty: AiDifficulty::Normal,
        }
    }
}

impl SelectedAiDifficulty {
    pub fn config(self, presets: &EnemyAiPresets, policy: SelectedAiPolicy) -> EnemyAiConfig {
        let mut config = presets
            .config_for(self.difficulty)
            .unwrap_or_else(|| presets.default_config());
        policy.apply_to_config(&mut config);
        config
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct SelectedAiPolicy {
    pub mode: EnemyAiPolicyMode,
}

impl Default for SelectedAiPolicy {
    fn default() -> Self {
        Self {
            mode: EnemyAiPolicyMode::Heuristic,
        }
    }
}

impl SelectedAiPolicy {
    pub fn apply_to_config(self, config: &mut EnemyAiConfig) {
        match self.mode {
            EnemyAiPolicyMode::Heuristic => {
                config.policy_mode = EnemyAiPolicyMode::Heuristic;
                config.policy_model_path = None;
                config.export_decision_samples = false;
            }
            EnemyAiPolicyMode::CollectOnly => {
                config.policy_mode = EnemyAiPolicyMode::CollectOnly;
                config.policy_model_path = None;
                config.export_decision_samples = true;
            }
            EnemyAiPolicyMode::ModelRanker => {
                config.policy_mode = EnemyAiPolicyMode::ModelRanker;
                if config.policy_model_path.is_none() {
                    config.policy_model_path =
                        Some("assets/data/ai_ranker_default.ron".to_string());
                }
                config.policy_fallback = EnemyAiPolicyFallback::Heuristic;
                config.export_decision_samples = true;
            }
            EnemyAiPolicyMode::SelfPlayTraining => {
                config.policy_mode = EnemyAiPolicyMode::SelfPlayTraining;
                config.policy_model_path = None;
                config.export_decision_samples = true;
            }
        }
    }
}

pub fn ai_policy_label(mode: EnemyAiPolicyMode) -> &'static str {
    match mode {
        EnemyAiPolicyMode::Heuristic => "启发式",
        EnemyAiPolicyMode::CollectOnly => "采样",
        EnemyAiPolicyMode::ModelRanker => "模型",
        EnemyAiPolicyMode::SelfPlayTraining => "自对战",
    }
}

impl SelectedAiDifficulty {
    pub fn base_config(self, presets: &EnemyAiPresets) -> EnemyAiConfig {
        presets
            .config_for(self.difficulty)
            .unwrap_or_else(|| presets.default_config())
    }
}

pub fn ai_difficulty_label(difficulty: AiDifficulty) -> &'static str {
    match difficulty {
        AiDifficulty::Easy => "简单",
        AiDifficulty::Normal => "普通",
        AiDifficulty::Hard => "困难",
        AiDifficulty::Expert => "专家",
    }
}

pub fn ai_difficulty_description(difficulty: AiDifficulty) -> &'static str {
    match difficulty {
        AiDifficulty::Easy => "适合熟悉规则：较少规划，不读取玩家威胁。",
        AiDifficulty::Normal => "标准体验：基础规划，使用公开信息。",
        AiDifficulty::Hard => "强化挑战：两步规划，联合玩家 AP / 技能威胁。",
        AiDifficulty::Expert => "高压挑战：更深规划，并投影可见手牌威胁。",
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct SelectionState {
    pub stage: SelectionStage,
    pub selected_indices: Vec<usize>,
    pub player_indices: Vec<usize>,
}

impl SelectionState {
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

    pub fn toggle(&mut self, index: usize, max_team_size: usize) {
        if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
            self.selected_indices.remove(pos);
        } else if self.selected_indices.len() < max_team_size {
            self.selected_indices.push(index);
        }
    }

    pub fn can_confirm(&self) -> bool {
        !self.selected_indices.is_empty()
    }

    pub fn reset(&mut self) {
        self.stage = SelectionStage::Player;
        self.selected_indices.clear();
        self.player_indices.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_ai_policy_applies_runtime_overrides() {
        let mut collect_config = EnemyAiConfig::default();
        SelectedAiPolicy {
            mode: EnemyAiPolicyMode::CollectOnly,
        }
        .apply_to_config(&mut collect_config);
        assert_eq!(collect_config.policy_mode, EnemyAiPolicyMode::CollectOnly);
        assert!(collect_config.export_decision_samples);
        assert!(collect_config.policy_model_path.is_none());

        let mut model_config = EnemyAiConfig::default();
        SelectedAiPolicy {
            mode: EnemyAiPolicyMode::ModelRanker,
        }
        .apply_to_config(&mut model_config);
        assert_eq!(model_config.policy_mode, EnemyAiPolicyMode::ModelRanker);
        assert_eq!(
            model_config.policy_model_path.as_deref(),
            Some("assets/data/ai_ranker_default.ron")
        );
        assert_eq!(
            model_config.policy_fallback,
            EnemyAiPolicyFallback::Heuristic
        );
        assert!(model_config.export_decision_samples);
    }
}
