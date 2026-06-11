use bevy::prelude::*;

use crate::data::{AiDifficulty, EnemyAiConfig};

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
    pub fn config(self) -> EnemyAiConfig {
        EnemyAiConfig::preset(self.difficulty)
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
