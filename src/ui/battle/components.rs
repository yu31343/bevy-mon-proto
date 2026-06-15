use bevy::prelude::*;

use crate::battle::Side;

#[derive(Component)]
pub(crate) struct BattleUiRoot;

#[derive(Component)]
pub(crate) struct BattleUiCleanupPending {
    pub frames_remaining: u8,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct SkillSlotId {
    pub side: Side,
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerStatsText;
#[derive(Component)]
pub(crate) struct EnemyStatsText;
#[derive(Component)]
pub(crate) struct ResultText;
#[derive(Component)]
pub(crate) struct BattlePhaseText;

#[derive(Component)]
pub(crate) struct TurnBannerText;

#[derive(Component)]
pub(crate) struct ActionPointsText;

#[derive(Component)]
pub(crate) struct BattleHintText;

#[derive(Component)]
pub(crate) struct BattleActionText;

#[derive(Component)]
pub(crate) struct PlayerCardButton {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerCardHotkeyText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerCardHotkeyBadge {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerCardNameText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerCardCostText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerCardDescText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct DiscardButton;

#[derive(Component)]
pub(crate) struct EndTurnButton;

#[derive(Component)]
pub(crate) struct RetreatButton;

#[derive(Component)]
pub(crate) struct RetreatButtonText;

#[derive(Component)]
pub(crate) struct SwitchMonsterButton;

#[derive(Component)]
pub(crate) struct ActionDialButton;

#[derive(Component)]
pub(crate) struct SwitchCancelButton;

#[derive(Component)]
pub(crate) struct TeamMemberButton {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberButtonText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberAuraText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberHpBarFill {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberShieldBarTrack {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberShieldBarFill {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchCard {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchNameText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchAuraText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchStatsText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchHpBarFill {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchHpValueText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchShieldBarTrack {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchShieldBarFill {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchShieldValueText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchDetailRoot {
    pub index: usize,
}

#[derive(Resource, Default)]
pub(crate) struct PlayerBenchDetailState {
    pub expanded_index: Option<usize>,
}

#[derive(Component)]
pub(crate) struct EnemyBenchCard {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyBenchNameText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyBenchAuraText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyBenchStatsText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyBenchHpBarFill {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyBenchHpValueText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyBenchShieldBarTrack {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyBenchShieldBarFill {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyBenchShieldValueText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyTeamMemberButton {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyTeamMemberButtonText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyTeamMemberAuraText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyTeamMemberHpBarFill {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyTeamMemberShieldBarTrack {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyTeamMemberShieldBarFill {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct SkillButton {
    pub index: usize,
}
#[derive(Component)]
pub(crate) struct SkillButtonText {
    pub index: usize,
}
#[derive(Component)]
pub(crate) struct SkillButtonMetaText {
    pub index: usize,
}
#[derive(Component)]
pub(crate) struct SkillButtonIconText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemySkillText {
    pub index: usize,
}
#[derive(Component)]
pub(crate) struct EnemySkillMetaText {
    pub index: usize,
}
#[derive(Component)]
pub(crate) struct EnemySkillIconText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerHpBarFill;
#[derive(Component)]
pub(crate) struct EnemyHpBarFill;

#[derive(Component)]
pub(crate) struct PlayerShieldBarTrack;
#[derive(Component)]
pub(crate) struct EnemyShieldBarTrack;

#[derive(Component)]
pub(crate) struct PlayerShieldBarFill;
#[derive(Component)]
pub(crate) struct EnemyShieldBarFill;

#[derive(Component)]
pub(crate) struct SwitchOverlayRoot;

#[derive(Component)]
pub(crate) struct SkillPanelRoot;

#[derive(Component)]
pub(crate) struct HandCardsRoot;

#[derive(Component)]
pub(crate) struct PlayerInfoPanel;

#[derive(Component)]
pub(crate) struct EnemyInfoPanel;

#[derive(Component)]
pub(crate) struct PlayerNameText;

#[derive(Component)]
pub(crate) struct EnemyNameText;

#[derive(Component)]
pub(crate) struct PlayerHpValueText;

#[derive(Component)]
pub(crate) struct PlayerHpStatText;

#[derive(Component)]
pub(crate) struct EnemyHpValueText;

#[derive(Component)]
pub(crate) struct PlayerShieldValueText;

#[derive(Component)]
pub(crate) struct EnemyShieldValueText;

#[derive(Component)]
pub(crate) struct PlayerAtkText;

#[derive(Component)]
pub(crate) struct EnemyAtkText;

#[derive(Component)]
pub(crate) struct PlayerDefText;

#[derive(Component)]
pub(crate) struct EnemyDefText;

#[derive(Component)]
pub(crate) struct PlayerAccText;

#[derive(Component)]
pub(crate) struct EnemyAccText;

#[derive(Component)]
pub(crate) struct PlayerSpdText;

#[derive(Component)]
pub(crate) struct EnemySpdText;

#[derive(Component)]
pub(crate) struct PlayerAuraLine;

#[derive(Component)]
pub(crate) struct EnemyAuraLine;

#[derive(Component)]
pub(crate) struct PlayerNameAuraRow;

#[derive(Component)]
pub(crate) struct EnemyNameAuraRow;

#[derive(Component)]
pub(crate) struct PlayerStatusLine;

#[derive(Component)]
pub(crate) struct EnemyStatusLine;

#[derive(Component, Clone, Copy)]
pub(crate) struct DebugAuraToken;

#[derive(Component, Clone, Copy)]
pub(crate) struct DebugStatusToken;
