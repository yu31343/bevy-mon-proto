pub mod cards;

use std::{
    collections::{HashMap, HashSet},
    fs,
};

use bevy::prelude::*;
use serde::Deserialize;

pub use cards::{CardDeck, CardDef, CardEffect, CardId};

/// 元素类型（系别）：火、水、草、光、暗、雷、风。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Deserialize)]
pub enum ElementType {
    Fire,
    Water,
    Grass,
    Light,
    Dark,
    Thunder,
    Wind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Deserialize)]
pub enum SkillCategory {
    NormalAttack,
    ElementAttack,
    SpecialAttack,
    SelfUtility,
    AllyUtility,
    EnemyDebuff,
}

fn default_skill_category() -> SkillCategory {
    SkillCategory::NormalAttack
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Deserialize)]
pub enum StatusCategory {
    Aura,
    Buff,
    Debuff,
    Special,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Deserialize)]
pub enum StatusTickTiming {
    OwnerActionEnd,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Deserialize)]
pub enum AttributeType {
    Atk,
    Def,
    Spd,
    Acc,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Deserialize, Default)]
pub enum EffectTarget {
    #[default]
    Infer,
    SelfTarget,
    Opponent,
}

#[derive(Debug, Clone, Deserialize)]
pub enum SkillCondition {
    TargetHadAura { element: ElementType },
    TargetHadStatus { status_id: String },
    TargetHadNoShield,
    LastReactionName { reaction_name: String },
    LastWindSpreadSucceeded,
    LastWindSpreadFailed,
    LastCleanseSucceeded,
    LastCleanseFailed,
    LastTargetFainted,
    Any { conditions: Vec<SkillCondition> },
    All { conditions: Vec<SkillCondition> },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConditionalSkillEffect {
    pub condition: SkillCondition,
    pub effect: Box<SkillEffect>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AttributeStageModifier {
    pub attribute: AttributeType,
    pub amount: i32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AttributeStageBounds {
    pub min: i32,
    pub max: i32,
}

impl Default for AttributeStageBounds {
    fn default() -> Self {
        Self { min: -6, max: 6 }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AccuracyRulesConfig {
    pub min: f32,
    pub max: f32,
    pub stage_step: f32,
}

impl Default for AccuracyRulesConfig {
    fn default() -> Self {
        Self {
            min: 0.5,
            max: 1.0,
            stage_step: 0.05,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct DamageFormulaConfig {
    pub min_damage: i32,
}

impl Default for DamageFormulaConfig {
    fn default() -> Self {
        Self { min_damage: 1 }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub struct BattleFormulaConfig {
    #[serde(default)]
    pub attribute_stage_bounds: AttributeStageBounds,
    #[serde(default)]
    pub accuracy: AccuracyRulesConfig,
    #[serde(default)]
    pub damage: DamageFormulaConfig,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct BattleFormulaRules {
    pub attribute_stage_bounds: AttributeStageBounds,
    pub accuracy: AccuracyRulesConfig,
    pub damage: DamageFormulaConfig,
}

impl BattleFormulaRules {
    pub fn from_config(config: &BattleFormulaConfig) -> Self {
        Self {
            attribute_stage_bounds: config.attribute_stage_bounds,
            accuracy: config.accuracy,
            damage: config.damage,
        }
    }
}

impl Default for BattleFormulaRules {
    fn default() -> Self {
        Self::from_config(&BattleFormulaConfig::default())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatusDef {
    pub id: String,
    pub name: String,
    pub category: StatusCategory,
    pub duration_turns: i32,
    #[serde(default)]
    pub tick_timing: Option<StatusTickTiming>,
    #[serde(default)]
    pub stage_modifiers: Vec<AttributeStageModifier>,
    #[serde(default)]
    pub fixed_damage_on_tick: i32,
    #[serde(default)]
    pub heal_on_tick: i32,
    #[serde(default)]
    pub heal_taken_multiplier: Option<f32>,
    #[serde(default)]
    pub evade_charges: i32,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct StatusDb {
    pub statuses: HashMap<String, StatusDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReactionDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub required_elements: Vec<ElementType>,
    #[serde(default)]
    pub required_statuses: Vec<String>,
    pub trigger_element: ElementType,
    #[serde(default)]
    pub fixed_damage: i32,
    #[serde(default)]
    pub heal_attacker: i32,
    #[serde(default)]
    pub apply_statuses: Vec<String>,
    #[serde(default)]
    pub clear_statuses: Vec<String>,
    #[serde(default)]
    pub aura_results: Vec<ElementType>,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ReactionDb {
    pub reactions: Vec<ReactionDef>,
}

/// 元素克制矩阵资源（数据驱动）。
/// 使用 HashMap 存储元素对之间的伤害倍率。
/// 未显式定义的组合默认为 1.0（正常伤害）。
#[derive(Resource, Debug, Clone, Default)]
pub struct ElementDb {
    /// (攻击方元素, 防御方元素) -> 伤害倍率
    matrix: HashMap<(ElementType, ElementType), f32>,
}

impl ElementDb {
    /// 获取攻击方元素对防御方元素的伤害倍数。
    /// 若未显式定义，默认返回 1.0。
    pub fn get_effectiveness(&self, attacker: ElementType, defender: ElementType) -> f32 {
        self.matrix
            .get(&(attacker, defender))
            .copied()
            .unwrap_or(1.0)
    }

    /// 从配置构建默认的元素克制矩阵（用于 fallback）。
    pub fn from_default_config() -> Self {
        let mut matrix = HashMap::new();

        // 光/暗
        matrix.insert((ElementType::Light, ElementType::Dark), 2.0);
        matrix.insert((ElementType::Dark, ElementType::Light), 0.5);

        // 水/火/草循环
        matrix.insert((ElementType::Water, ElementType::Fire), 2.0);
        matrix.insert((ElementType::Fire, ElementType::Grass), 2.0);
        matrix.insert((ElementType::Grass, ElementType::Water), 2.0);
        matrix.insert((ElementType::Water, ElementType::Grass), 0.5);
        matrix.insert((ElementType::Fire, ElementType::Water), 0.5);
        matrix.insert((ElementType::Grass, ElementType::Fire), 0.5);

        // 雷/水/风/草
        matrix.insert((ElementType::Thunder, ElementType::Water), 2.0);
        matrix.insert((ElementType::Water, ElementType::Thunder), 0.5);
        matrix.insert((ElementType::Thunder, ElementType::Wind), 2.0);
        matrix.insert((ElementType::Wind, ElementType::Thunder), 0.5);
        matrix.insert((ElementType::Thunder, ElementType::Grass), 0.5);

        // 风/草/火
        matrix.insert((ElementType::Wind, ElementType::Grass), 2.0);
        matrix.insert((ElementType::Fire, ElementType::Wind), 2.0);
        matrix.insert((ElementType::Wind, ElementType::Fire), 0.5);

        Self { matrix }
    }

    /// 从 RON 配置加载矩阵。
    pub fn from_config(config: &ElementMatrixConfig) -> Self {
        Self {
            matrix: config.to_hashmap(),
        }
    }
}

/// RON 配置中的元素矩阵结构。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ElementMatrixConfig {
    /// 克制关系列表：每项包含攻击方、防御方和倍率。
    #[serde(default)]
    pub relations: Vec<ElementRelation>,
}

impl ElementMatrixConfig {
    /// 将配置转换为 HashMap 格式。
    pub fn to_hashmap(&self) -> HashMap<(ElementType, ElementType), f32> {
        let mut matrix = HashMap::new();
        for rel in &self.relations {
            matrix.insert((rel.attacker, rel.defender), rel.multiplier);
        }
        matrix
    }
}

/// 单条元素克制关系定义。
#[derive(Debug, Clone, Deserialize)]
pub struct ElementRelation {
    pub attacker: ElementType,
    pub defender: ElementType,
    pub multiplier: f32,
}

/// 元素附着/反应的“盾免疫”判定：
/// 当护盾吸收了本次伤害的一部分（absorbed > 0）时，不触发元素附着/消耗。
#[allow(dead_code)]
pub fn shield_blocks_element_attachment(shield: i32, theoretical_damage: i32) -> bool {
    shield.min(theoretical_damage) > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_db_basic_cycles() {
        let db = ElementDb::from_default_config();
        assert_eq!(
            db.get_effectiveness(ElementType::Water, ElementType::Fire),
            2.0
        );
        assert_eq!(
            db.get_effectiveness(ElementType::Fire, ElementType::Grass),
            2.0
        );
        assert_eq!(
            db.get_effectiveness(ElementType::Grass, ElementType::Water),
            2.0
        );

        assert_eq!(
            db.get_effectiveness(ElementType::Water, ElementType::Grass),
            0.5
        );
    }

    #[test]
    fn element_db_light_dark() {
        let db = ElementDb::from_default_config();
        assert_eq!(
            db.get_effectiveness(ElementType::Light, ElementType::Dark),
            2.0
        );
        assert_eq!(
            db.get_effectiveness(ElementType::Dark, ElementType::Light),
            0.5
        );
    }

    #[test]
    fn element_db_default_fallback() {
        let db = ElementDb::default(); // 空 matrix
        // 未定义的组合应返回 1.0
        assert_eq!(
            db.get_effectiveness(ElementType::Fire, ElementType::Fire),
            1.0
        );
        assert_eq!(
            db.get_effectiveness(ElementType::Water, ElementType::Light),
            1.0
        );
    }

    #[test]
    fn shield_blocks_attachment_when_absorbed_positive() {
        // 理论伤害 > 0 且护盾 > 0：必定会吸收部分
        assert!(shield_blocks_element_attachment(5, 1));
        assert!(shield_blocks_element_attachment(1, 5));

        // 护盾为 0：不会吸收
        assert!(!shield_blocks_element_attachment(0, 10));
        // 理论伤害为 0：不会吸收
        assert!(!shield_blocks_element_attachment(10, 0));
    }
}

/// 技能唯一标识（逻辑层使用）。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Deserialize)]
pub enum SkillId {
    FirePunch,
    FlameStorm,
    BurningSoul,
    ExplosiveIgnition,

    WaterBlade,
    UndertowCrash,
    WaterScreen,
    TidalImpact,

    VineWhip,
    EntanglingVines,
    NatureHealing,
    LifeBloom,

    ThunderStrike,
    ThousandThunder,
    SwiftThunder,
    ElectroPulse,

    WindBlade,
    ShadowWingAssassinate,
    GaleEvasion,
    CycloneRend,

    LightBladeSlash,
    SacredJudgment,
    HolyShelter,
    PurifyingLight,

    ShadowBlade,
    AbyssReap,
    CurseWhisper,
    BloodTouch,
}

/// 技能配置项。
#[derive(Debug, Clone, Deserialize)]
pub struct SkillDef {
    pub id: SkillId,
    pub name: String,
    #[serde(default = "default_skill_category")]
    pub category: SkillCategory,
    pub cost_ap: i32,
    pub effect: SkillEffect,
    #[serde(default)]
    pub element: Option<ElementType>,
    #[serde(default)]
    pub base_accuracy: Option<f32>,
}

/// 技能效果定义。
#[derive(Debug, Clone, Deserialize)]
pub enum SkillEffect {
    Attack {
        power: i32,
        #[serde(default)]
        lifesteal_ratio: Option<f32>,
        #[serde(default)]
        ignore_shield: bool,
    },
    Heal {
        amount: i32,
    },
    Shield {
        amount: i32,
    },
    ApplyStatus {
        status_id: String,
    },
    Cleanse {
        #[serde(default)]
        prefer_aura: bool,
        #[serde(default)]
        fallback_to_debuff: bool,
        #[serde(default)]
        amount: usize,
    },
    Dispel {
        status_ids: Vec<String>,
        #[serde(default)]
        target: EffectTarget,
    },
    DealFixedDamage {
        amount: i32,
        #[serde(default)]
        ignore_shield: bool,
        #[serde(default)]
        target: EffectTarget,
    },
    DealStatDifferenceDamage {
        source_attribute: AttributeType,
        target_attribute: AttributeType,
        #[serde(default)]
        multiply_by_source_stage: bool,
        #[serde(default)]
        ignore_shield: bool,
        #[serde(default)]
        target: EffectTarget,
    },
    ModifyStages {
        modifiers: Vec<AttributeStageModifier>,
        duration_turns: i32,
        #[serde(default)]
        target: EffectTarget,
    },
    Conditional {
        branches: Vec<ConditionalSkillEffect>,
    },
    Sequence {
        effects: Vec<SkillEffect>,
    },
}

/// 基础属性配置。
#[derive(Debug, Clone, Deserialize)]
pub struct StatsData {
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
    #[serde(default = "default_accuracy_percent")]
    pub acc: i32,
}

/// 单个角色（怪物）原型配置。
#[derive(Debug, Clone, Deserialize)]
pub struct MonsterPrototype {
    pub name: String,
    pub element: ElementType,
    pub stats: StatsData,
    pub skills: Vec<SkillId>,
}

fn default_max_team_size() -> usize {
    3
}

fn default_cards_per_round() -> usize {
    5
}

fn default_ap_per_round() -> i32 {
    6
}

fn default_accuracy_percent() -> i32 {
    100
}

#[derive(Debug, Clone, Deserialize)]
pub struct BattleRulesConfig {
    #[serde(default = "default_max_team_size")]
    pub max_team_size: usize,
    #[serde(default = "default_cards_per_round")]
    pub cards_per_round: usize,
    #[serde(default = "default_ap_per_round")]
    pub ap_per_round: i32,
}

impl Default for BattleRulesConfig {
    fn default() -> Self {
        Self {
            max_team_size: default_max_team_size(),
            cards_per_round: default_cards_per_round(),
            ap_per_round: default_ap_per_round(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct BattleRules {
    pub max_team_size: usize,
    pub cards_per_round: usize,
    pub ap_per_round: i32,
}

impl BattleRules {
    pub fn from_config(config: &BattleRulesConfig) -> Self {
        Self {
            max_team_size: config.max_team_size,
            cards_per_round: config.cards_per_round,
            ap_per_round: config.ap_per_round,
        }
    }
}

impl Default for BattleRules {
    fn default() -> Self {
        Self::from_config(&BattleRulesConfig::default())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct BattleConfig {
    #[serde(default)]
    element_matrix: ElementMatrixConfig,
    #[serde(default)]
    rules: BattleRulesConfig,
    #[serde(default)]
    formulas: BattleFormulaConfig,
    #[serde(default)]
    statuses: Vec<StatusDef>,
    #[serde(default)]
    reactions: Vec<ReactionDef>,
    skills: Vec<SkillDef>,
    monsters: Vec<MonsterPrototype>,
    cards: Vec<CardDef>,
    deck: Vec<CardId>,
}

#[derive(Resource, Debug, Clone)]
pub struct BattleRulesBundle {
    pub battle: BattleRules,
    pub formulas: BattleFormulaRules,
}

impl Default for BattleRulesBundle {
    fn default() -> Self {
        Self {
            battle: BattleRules::default(),
            formulas: BattleFormulaRules::default(),
        }
    }
}

/// 战斗数据库资源：组合技能、卡牌和元素克制矩阵。
/// 用于减少系统参数数量，避免超过 Bevy 的 16 参数限制。
#[derive(Resource, Debug, Clone)]
pub struct BattleDbs {
    pub skills: HashMap<SkillId, SkillDef>,
    pub cards: HashMap<CardId, CardDef>,
    pub elements: ElementDb,
    pub statuses: StatusDb,
    pub reactions: ReactionDb,
}

#[derive(Resource, Debug, Clone)]
pub struct MonsterPool {
    pub monsters: Vec<MonsterPrototype>,
}

#[derive(Resource, Debug, Clone)]
pub struct TeamSelections {
    pub player_indices: Vec<usize>,
    pub enemy_indices: Vec<usize>,
}

/// 数据加载状态：当配置读取/解析/校验失败时记录错误原因。
#[derive(Resource, Debug, Clone, Default)]
pub struct BattleDataStatus {
    pub error: Option<String>,
}

/// 数据插件：启动时加载技能与双方初始数据。
pub struct DataPlugin;

impl Plugin for DataPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_battle_data);
    }
}

fn load_battle_data(mut commands: Commands) {
    // 从 RON 配置加载战斗数据，便于后续扩展为纯数据驱动。
    let path = "assets/data/battle_data.ron";
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => {
            commands.insert_resource(BattleDbs {
                skills: HashMap::new(),
                cards: HashMap::new(),
                elements: ElementDb::from_default_config(),
                statuses: StatusDb::default(),
                reactions: ReactionDb::default(),
            });
            commands.insert_resource(MonsterPool { monsters: vec![] });
            commands.insert_resource(CardDeck::default());
            commands.insert_resource(BattleRules::default());
            commands.insert_resource(BattleFormulaRules::default());
            commands.insert_resource(BattleRulesBundle::default());
            commands.insert_resource(BattleDataStatus {
                error: Some(format!("读取战斗配置失败: {path} ({e})")),
            });
            return;
        }
    };
    let config: BattleConfig = match ron::from_str(&raw) {
        Ok(config) => config,
        Err(e) => {
            commands.insert_resource(BattleDbs {
                skills: HashMap::new(),
                cards: HashMap::new(),
                elements: ElementDb::from_default_config(),
                statuses: StatusDb::default(),
                reactions: ReactionDb::default(),
            });
            commands.insert_resource(MonsterPool { monsters: vec![] });
            commands.insert_resource(CardDeck::default());
            commands.insert_resource(BattleRules::default());
            commands.insert_resource(BattleFormulaRules::default());
            commands.insert_resource(BattleRulesBundle::default());
            commands.insert_resource(BattleDataStatus {
                error: Some(format!("解析战斗配置失败: {path} ({e})")),
            });
            return;
        }
    };

    if let Err(reason) = validate_battle_config(&config) {
        commands.insert_resource(BattleDbs {
            skills: HashMap::new(),
            cards: HashMap::new(),
            elements: ElementDb::from_default_config(),
            statuses: StatusDb::default(),
            reactions: ReactionDb::default(),
        });
        commands.insert_resource(MonsterPool { monsters: vec![] });
        commands.insert_resource(CardDeck::default());
        commands.insert_resource(BattleRules::default());
        commands.insert_resource(BattleFormulaRules::default());
        commands.insert_resource(BattleRulesBundle::default());
        commands.insert_resource(BattleDataStatus {
            error: Some(format!("战斗配置非法: {reason}")),
        });
        return;
    }

    let rules = BattleRules::from_config(&config.rules);
    let formulas = BattleFormulaRules::from_config(&config.formulas);
    let rules_bundle = BattleRulesBundle {
        battle: rules.clone(),
        formulas,
    };

    let statuses = StatusDb {
        statuses: config
            .statuses
            .iter()
            .cloned()
            .map(|status| (status.id.clone(), status))
            .collect(),
    };
    let reactions = ReactionDb {
        reactions: config.reactions.clone(),
    };

    let BattleConfig {
        element_matrix,
        rules: _,
        formulas: _,
        statuses: _,
        reactions: _,
        skills: config_skills,
        monsters,
        cards: config_cards,
        deck,
    } = config;

    let rules = rules_bundle.battle.clone();
    let formulas = rules_bundle.formulas;

    let mut skills = HashMap::new();
    for skill in config_skills {
        skills.insert(skill.id, skill);
    }

    let mut cards = HashMap::new();
    for def in config_cards {
        cards.insert(def.id, def);
    }

    // 加载元素克制矩阵：若配置为空则使用默认值
    let elements = if element_matrix.relations.is_empty() {
        ElementDb::from_default_config()
    } else {
        ElementDb::from_config(&element_matrix)
    };

    commands.insert_resource(BattleDbs {
        skills,
        cards,
        elements,
        statuses,
        reactions,
    });
    commands.insert_resource(MonsterPool { monsters });
    commands.insert_resource(CardDeck(deck));
    commands.insert_resource(rules);
    commands.insert_resource(formulas);
    commands.insert_resource(rules_bundle);
    commands.insert_resource(BattleDataStatus::default());
}

fn validate_battle_config(config: &BattleConfig) -> Result<(), String> {
    if config.skills.is_empty() {
        return Err("skills 不能为空".to_string());
    }
    if config.monsters.is_empty() {
        return Err("monsters 不能为空".to_string());
    }

    let rules = BattleRules::from_config(&config.rules);

    if !(1..=3).contains(&rules.max_team_size) {
        return Err(format!(
            "rules.max_team_size 必须在 1..=3 之间，当前为 {}",
            rules.max_team_size
        ));
    }
    if rules.cards_per_round == 0 {
        return Err("rules.cards_per_round 必须 >= 1".to_string());
    }
    if rules.ap_per_round < 0 {
        return Err("rules.ap_per_round 必须 >= 0".to_string());
    }

    if config.formulas.attribute_stage_bounds.min > config.formulas.attribute_stage_bounds.max {
        return Err("formulas.attribute_stage_bounds.min 不能大于 max".to_string());
    }
    if config.formulas.accuracy.min > config.formulas.accuracy.max {
        return Err("formulas.accuracy.min 不能大于 max".to_string());
    }
    if !(0.0..=1.0).contains(&config.formulas.accuracy.min)
        || !(0.0..=1.0).contains(&config.formulas.accuracy.max)
    {
        return Err("formulas.accuracy 的 min/max 必须在 0.0..=1.0 之间".to_string());
    }
    if config.formulas.accuracy.stage_step < 0.0 {
        return Err("formulas.accuracy.stage_step 必须 >= 0.0".to_string());
    }
    if config.formulas.damage.min_damage < 0 {
        return Err("formulas.damage.min_damage 必须 >= 0".to_string());
    }

    if config.monsters.len() < rules.max_team_size {
        return Err(format!(
            "monsters 数量不足：当前 {}，至少需要 {}（max_team_size）",
            config.monsters.len(),
            rules.max_team_size
        ));
    }

    let mut status_ids = HashSet::new();
    for status in &config.statuses {
        if !status_ids.insert(status.id.as_str()) {
            return Err(format!("状态ID重复: {}", status.id));
        }
        if status.duration_turns < 0 {
            return Err(format!("状态 {} 的 duration_turns 必须 >= 0", status.id));
        }
        if status.fixed_damage_on_tick < 0 {
            return Err(format!(
                "状态 {} 的 fixed_damage_on_tick 必须 >= 0",
                status.id
            ));
        }
        if status.heal_on_tick < 0 {
            return Err(format!("状态 {} 的 heal_on_tick 必须 >= 0", status.id));
        }
        if let Some(multiplier) = status.heal_taken_multiplier {
            if multiplier < 0.0 {
                return Err(format!(
                    "状态 {} 的 heal_taken_multiplier 必须 >= 0.0",
                    status.id
                ));
            }
        }
        if status.evade_charges < 0 {
            return Err(format!("状态 {} 的 evade_charges 必须 >= 0", status.id));
        }
    }

    fn validate_skill_condition(
        condition: &SkillCondition,
        status_ids: &HashSet<&str>,
        skill_label: &str,
    ) -> Result<(), String> {
        match condition {
            SkillCondition::TargetHadStatus { status_id } => {
                if !status_ids.contains(status_id.as_str()) {
                    return Err(format!(
                        "技能 {skill_label} 的条件引用了未定义状态 {status_id}"
                    ));
                }
            }
            SkillCondition::LastReactionName { reaction_name }
                if reaction_name.trim().is_empty() =>
            {
                return Err(format!("技能 {skill_label} 的 LastReactionName 不能为空"));
            }
            SkillCondition::Any { conditions } | SkillCondition::All { conditions } => {
                if conditions.is_empty() {
                    return Err(format!("技能 {skill_label} 的复合条件不能为空"));
                }
                for nested in conditions {
                    validate_skill_condition(nested, status_ids, skill_label)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_skill_effect(
        effect: &SkillEffect,
        status_ids: &HashSet<&str>,
        skill_label: &str,
    ) -> Result<(), String> {
        match effect {
            SkillEffect::Attack {
                lifesteal_ratio, ..
            } => {
                if let Some(ratio) = lifesteal_ratio {
                    if !(0.0..=1.0).contains(ratio) {
                        return Err(format!(
                            "技能 {skill_label} 的 Attack.lifesteal_ratio 必须在 0.0..=1.0 之间"
                        ));
                    }
                }
            }
            SkillEffect::Heal { .. } | SkillEffect::Shield { .. } => {}
            SkillEffect::ApplyStatus { status_id } => {
                if !status_ids.contains(status_id.as_str()) {
                    return Err(format!("技能 {skill_label} 引用了未定义状态 {status_id}"));
                }
            }
            SkillEffect::Cleanse { amount, .. } => {
                if *amount == 0 {
                    return Err(format!("技能 {skill_label} 的 Cleanse.amount 必须 >= 1"));
                }
            }
            SkillEffect::Dispel {
                status_ids: dispel_status_ids,
                ..
            } => {
                if dispel_status_ids.is_empty() {
                    return Err(format!("技能 {skill_label} 的 Dispel.status_ids 不能为空"));
                }
                for status_id in dispel_status_ids {
                    if matches!(
                        status_id.as_str(),
                        "stage_shift_buff" | "stage_shift_debuff"
                    ) {
                        continue;
                    }
                    if !status_ids.contains(status_id.as_str()) {
                        return Err(format!(
                            "技能 {skill_label} 的 Dispel 引用了未定义状态 {status_id}"
                        ));
                    }
                }
            }
            SkillEffect::DealFixedDamage { amount, .. } => {
                if *amount < 0 {
                    return Err(format!(
                        "技能 {skill_label} 的 DealFixedDamage.amount 必须 >= 0"
                    ));
                }
            }
            SkillEffect::DealStatDifferenceDamage { .. } => {}
            SkillEffect::ModifyStages {
                modifiers: _,
                duration_turns,
                target: _,
            } => {
                if *duration_turns < 0 {
                    return Err(format!(
                        "技能 {skill_label} 的 ModifyStages.duration_turns 必须 >= 0"
                    ));
                }
            }
            SkillEffect::Conditional { branches } => {
                if branches.is_empty() {
                    return Err(format!(
                        "技能 {skill_label} 的 Conditional.branches 不能为空"
                    ));
                }
                for branch in branches {
                    validate_skill_condition(&branch.condition, status_ids, skill_label)?;
                    validate_skill_effect(&branch.effect, status_ids, skill_label)?;
                }
            }
            SkillEffect::Sequence { effects } => {
                if effects.is_empty() {
                    return Err(format!("技能 {skill_label} 的 Sequence.effects 不能为空"));
                }
                for nested_effect in effects {
                    validate_skill_effect(nested_effect, status_ids, skill_label)?;
                }
            }
        }
        Ok(())
    }

    let mut skill_ids = HashSet::new();
    for skill in &config.skills {
        if !skill_ids.insert(skill.id) {
            return Err(format!("技能ID重复: {:?}", skill.id));
        }
        if let Some(base_accuracy) = skill.base_accuracy {
            if !(0.0..=1.0).contains(&base_accuracy) {
                return Err(format!(
                    "技能 {:?} 的 base_accuracy 必须在 0.0..=1.0 之间",
                    skill.id
                ));
            }
        }
        validate_skill_effect(&skill.effect, &status_ids, &format!("{:?}", skill.id))?;
    }

    for reaction in &config.reactions {
        if reaction.required_elements.len() > 2 {
            return Err(format!(
                "反应 {} 的 required_elements 不能超过 2 个",
                reaction.id
            ));
        }
        for status_id in &reaction.required_statuses {
            if !status_ids.contains(status_id.as_str()) {
                return Err(format!(
                    "反应 {} 引用了未定义状态 {}",
                    reaction.id, status_id
                ));
            }
        }
        for status_id in &reaction.apply_statuses {
            if !status_ids.contains(status_id.as_str()) {
                return Err(format!(
                    "反应 {} 的 apply_statuses 引用了未定义状态 {}",
                    reaction.id, status_id
                ));
            }
        }
        for status_id in &reaction.clear_statuses {
            if !status_ids.contains(status_id.as_str()) {
                return Err(format!(
                    "反应 {} 的 clear_statuses 引用了未定义状态 {}",
                    reaction.id, status_id
                ));
            }
        }
    }

    for mon in &config.monsters {
        if mon.skills.is_empty() || mon.skills.len() > 4 {
            return Err(format!(
                "角色 {} 技能数量必须在 1..=4，当前为 {}",
                mon.name,
                mon.skills.len()
            ));
        }

        for &sid in &mon.skills {
            if !skill_ids.contains(&sid) {
                return Err(format!("角色 {} 使用了未定义技能 {:?}", mon.name, sid));
            }
        }
    }

    Ok(())
}
