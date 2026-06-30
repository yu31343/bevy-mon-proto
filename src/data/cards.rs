use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{AttributeType, ElementType, StatusCategory};

/// 技能卡唯一标识（与 RON 配置中的 id 字段对应）。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub enum CardId {
    /// 使用后获得行动点。
    GainAp,
    /// 使用后给下次进攻附加伤害增益。
    NextAttackBoost,
    /// 使用后给下次防御附加护盾增益。
    NextShieldBoost,
    /// 使用后给下次治疗附加回复增益。
    NextHealBoost,
    ElementWarmup,
    ReactionCatalyst,
    WindSpreadFlow,
    ReactionCountdown,
    TacticalRefresh,
    ReadyToAct,
    Pursuit,
    EmergencyShield,
    GuardCounter,
    FortifiedLine,
    CleanseConversion,
    TeamCommand,
    RotationCover,
}

/// 技能卡效果定义。
#[derive(Debug, Clone, Deserialize)]
pub enum CardEffect {
    GainAp {
        amount: i32,
    },
    NextAttackBoost {
        amount: i32,
    },
    NextShieldBoost {
        amount: i32,
    },
    NextHealBoost {
        amount: i32,
    },
    NextElementAttachmentGainAp {
        amount: i32,
    },
    NextReactionFixedDamage {
        amount: i32,
        ignore_shield: bool,
    },
    NextWindSpreadDamage {
        amount: i32,
        elements: Vec<ElementType>,
        ignore_shield: bool,
    },
    NextAuraAttackDraw {
        amount: usize,
    },
    DiscardOtherDrawGainAp {
        draw: usize,
        gain_ap: i32,
    },
    NextSkillCostDraw {
        skill_cost: i32,
        draw: usize,
    },
    DrawIfKnockedOutThisTurn {
        amount: usize,
    },
    GainShield {
        amount: i32,
    },
    ShieldAbsorbGainAp {
        amount: i32,
    },
    ModifyStages {
        attribute: AttributeType,
        amount: i32,
        duration_turns: i32,
    },
    CleanseOrGainAp {
        categories: Vec<StatusCategory>,
        fallback_ap: i32,
    },
    DrawAndGainApIfAliveTeam {
        draw: usize,
        min_alive: usize,
        gain_ap: i32,
    },
    GainShieldDrawIfSwitchedThisTurn {
        shield: i32,
        draw: usize,
    },
}

/// 单张技能卡的完整定义。
#[derive(Debug, Clone, Deserialize)]
pub struct CardDef {
    pub id: CardId,
    pub name: String,
    pub cost_ap: i32,
    pub effect: CardEffect,
}

/// 技能牌库：按顺序排列的卡牌列表，每回合随机抽取。
#[derive(Resource, Debug, Clone, Default)]
pub struct CardDeck(pub Vec<CardId>);
