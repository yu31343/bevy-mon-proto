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

/// Text showing "已选择: X / 3".
#[derive(Component)]
pub struct SelectionCountText;


/// Resource tracking the current selection state.
#[derive(Resource, Debug, Clone, Default)]
pub struct SelectionState {
    pub selected_indices: Vec<usize>,
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
}
