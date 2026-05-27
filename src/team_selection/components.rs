use bevy::prelude::*;

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
