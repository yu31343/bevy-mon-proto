use bevy::prelude::*;

use crate::battle::Side;

#[derive(Component)]
pub(crate) struct BattleUiRoot;

#[derive(Component)]
pub(crate) struct BattleBackgroundSprite;

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
pub(crate) struct PlayerElementIcon;
#[derive(Component)]
pub(crate) struct EnemyElementIcon;
#[derive(Component)]
pub(crate) struct PlayerPortraitImage;
#[derive(Component)]
pub(crate) struct EnemyPortraitImage;
#[derive(Component)]
pub(crate) struct PlayerBenchPortrait {
    pub index: usize,
}
#[derive(Component)]
pub(crate) struct EnemyBenchPortrait {
    pub index: usize,
}
#[derive(Component)]
pub(crate) struct ResultText;

/// 联机延迟指示器根节点（右上角信号条，仅 PVP 显示）。
#[derive(Component)]
pub(crate) struct LatencyIndicatorRoot;

/// 信号竖线之一；`index` 0/1/2 表示由矮到高的三条。
#[derive(Component)]
pub(crate) struct LatencyBar {
    pub index: u8,
}

/// 延迟数值文本（如 “42 ms”）。
#[derive(Component)]
pub(crate) struct LatencyText;

/// 信号条与延迟文本的配色（数值越低信号越好）。
pub(crate) const LATENCY_BAR_INACTIVE: Color = Color::srgba(1.0, 1.0, 1.0, 0.18);
pub(crate) const LATENCY_GOOD: Color = Color::srgb(0.33, 0.80, 0.42);
pub(crate) const LATENCY_MEDIUM: Color = Color::srgb(0.95, 0.77, 0.25);
pub(crate) const LATENCY_POOR: Color = Color::srgb(0.92, 0.42, 0.28);
pub(crate) const LATENCY_UNKNOWN: Color = Color::srgb(0.6, 0.6, 0.6);
/// 延迟分级阈值（毫秒，含上界）：≤GOOD 三格、≤MEDIUM 两格、其余一格。
pub(crate) const LATENCY_GOOD_MAX_MS: u32 = 80;
pub(crate) const LATENCY_MEDIUM_MAX_MS: u32 = 150;

#[derive(Component)]
pub(crate) struct ResultPopupRoot;

#[derive(Component)]
pub(crate) struct ResultTitleText;

#[derive(Component)]
pub(crate) struct ResultNoticeRoot;

#[derive(Component)]
pub(crate) struct ResultNoticeText;

#[derive(Component)]
pub(crate) struct ResultReturnButton;

#[derive(Component)]
pub(crate) struct ResultRestartButton;

#[derive(Component)]
pub(crate) struct ResultRematchButton;

#[derive(Component)]
pub(crate) struct ResultInvitePromptRoot;

#[derive(Component)]
pub(crate) struct ResultInviteAcceptButton;

#[derive(Component)]
pub(crate) struct ResultInviteRejectButton;

#[derive(Component)]
pub(crate) struct BattlePhaseText;

#[derive(Component)]
pub(crate) struct BattleTurnOrderText;

#[derive(Component)]
pub(crate) struct TurnBannerText;

#[derive(Component)]
pub(crate) struct ActionPointsText;

#[derive(Component)]
pub(crate) struct BattleHintButton;

#[derive(Component)]
pub(crate) struct BattleHintCloseButton;

#[derive(Component)]
pub(crate) struct BattleHintOverlayRoot;

#[derive(Component)]
pub(crate) struct BattleHintText;

#[derive(Component)]
pub(crate) struct BattleActionText {
    pub remaining: f32,
}

#[derive(Message)]
pub(crate) struct BattleUiNotice {
    pub text: &'static str,
}

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
pub(crate) struct RetreatConfirmOverlayRoot;

#[derive(Component)]
pub(crate) struct RetreatConfirmCancelButton;

#[derive(Component)]
pub(crate) struct RetreatConfirmProceedButton;

#[derive(Component)]
pub(crate) struct SwitchMonsterButton;

#[derive(Component)]
pub(crate) struct ActionDialButton;

#[derive(Component)]
pub(crate) struct ActionDialHighlight;

#[derive(Component)]
pub(crate) struct HandFullHintRoot;

#[derive(Component)]
pub(crate) struct HandFullHintText;

#[derive(Component)]
pub(crate) struct SwitchCancelButton;

#[derive(Component)]
pub(crate) struct TeamMemberButton {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberPortrait {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberElementIcon {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberNameText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberAuraLine {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberStatusLine {
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
pub(crate) struct TeamMemberHpValueText {
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
pub(crate) struct TeamMemberShieldValueText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberAtkText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberDefText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberAccText {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct TeamMemberSpdText {
    pub index: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct TeamMemberStatStageModifierBadge {
    pub index: usize,
    pub stat: StatStageModifierKind,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct TeamMemberStatStageModifierText {
    pub index: usize,
    pub stat: StatStageModifierKind,
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
pub(crate) struct PlayerBenchAuraLine {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct PlayerBenchStatusLine {
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
pub(crate) struct PlayerBenchShieldValueText {
    pub index: usize,
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
pub(crate) struct EnemyBenchAuraLine {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct EnemyBenchStatusLine {
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
pub(crate) struct EnemyHpStatText;

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
pub(crate) struct StatStageModifierBadge {
    pub side: crate::battle::Side,
    pub stat: StatStageModifierKind,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct StatStageModifierText {
    pub side: crate::battle::Side,
    pub stat: StatStageModifierKind,
}

#[derive(Clone, Copy)]
pub(crate) enum StatStageModifierKind {
    Atk,
    Def,
    Acc,
    Spd,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct DebugAuraToken;

#[derive(Component, Clone, Copy)]
pub(crate) struct DebugStatusToken;

// === 七圣召唤风重构新增 marker ===

/// AP 宝石 pip（每侧固定 10 个；每个表示 2 AP）。
#[derive(Component)]
pub(crate) struct ApGemPip;

/// AP 宝石点亮填充层（0 / 半颗 / 满颗）。
#[derive(Component)]
pub(crate) struct ApGemPipFill {
    pub side: Side,
    pub index: usize,
}

/// AP 数字标签（每侧一个）。
#[derive(Component)]
pub(crate) struct ApGemCountText {
    pub side: Side,
}

/// 手牌卡面顶部类别色带。
#[derive(Component)]
pub(crate) struct CardCategoryBand {
    pub index: usize,
}

/// 手牌卡面类别名文字。
#[derive(Component)]
pub(crate) struct CardCategoryLabel {
    pub index: usize,
}

/// 手牌发光覆盖层（独立子节点，不争用按钮 bg/border）。
#[derive(Component)]
pub(crate) struct CardGlow {
    pub index: usize,
}

/// 主精灵信息框的描金外框（当前行动方脉冲）。
#[derive(Component)]
pub(crate) struct ActivePortraitFrame {
    pub side: Side,
}

/// 技能格的元素类型色片（按技能元素动态着色）。
#[derive(Component)]
pub(crate) struct SkillTileTypeChip {
    pub index: usize,
}

/// 技能格的 AP 费用宝石数字。
#[derive(Component)]
pub(crate) struct SkillButtonCostText {
    pub index: usize,
}

/// 技能格的释放次数充能点（slot=技能槽 0..3；pip=该槽内第几个圆点）。
#[derive(Component)]
pub(crate) struct SkillUsePip {
    pub slot: usize,
    pub pip: usize,
}

/// 标记某技能格本回合释放次数已耗尽（进入变灰冷却态）。
/// 持有此标记的技能格由 `update_skill_uses_system` 独占着色，常规悬停/冷却系统将其跳过，避免互相覆盖。
#[derive(Component)]
pub(crate) struct SkillButtonExhausted;

/// 背景暗角覆盖层。
#[derive(Component)]
pub(crate) struct BattleVignette;
