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
        self.matrix.get(&(attacker, defender)).copied().unwrap_or(1.0)
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
        assert_eq!(db.get_effectiveness(ElementType::Water, ElementType::Fire), 2.0);
        assert_eq!(db.get_effectiveness(ElementType::Fire, ElementType::Grass), 2.0);
        assert_eq!(db.get_effectiveness(ElementType::Grass, ElementType::Water), 2.0);

        assert_eq!(db.get_effectiveness(ElementType::Water, ElementType::Grass), 0.5);
    }

    #[test]
    fn element_db_light_dark() {
        let db = ElementDb::from_default_config();
        assert_eq!(db.get_effectiveness(ElementType::Light, ElementType::Dark), 2.0);
        assert_eq!(db.get_effectiveness(ElementType::Dark, ElementType::Light), 0.5);
    }

    #[test]
    fn element_db_default_fallback() {
        let db = ElementDb::default(); // 空 matrix
        // 未定义的组合应返回 1.0
        assert_eq!(db.get_effectiveness(ElementType::Fire, ElementType::Fire), 1.0);
        assert_eq!(db.get_effectiveness(ElementType::Water, ElementType::Light), 1.0);
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
    /// slot0：普通攻击（不附着元素）。
    NormalAttack,
    /// slot1：对应元素的元素战技（会附着元素）。
    ElementWaterAttack,
    ElementFireAttack,
    ElementGrassAttack,
    ElementLightAttack,
    ElementDarkAttack,
    ElementThunderAttack,
    ElementWindAttack,

    Slash,
    HeavyStrike,
    FirstAid,
    Guard,
}

/// 技能配置项。
#[derive(Debug, Clone, Deserialize)]
pub struct SkillDef {
    pub id: SkillId,
    pub name: String,
    pub effect: SkillEffect,
    #[serde(default)]
    pub element: Option<ElementType>,
}

/// 技能效果定义。
#[derive(Debug, Clone, Deserialize)]
pub enum SkillEffect {
    Attack { power: i32 },
    Heal { amount: i32 },
    Shield { amount: i32 },
}

/// 基础属性配置。
#[derive(Debug, Clone, Deserialize)]
pub struct StatsData {
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
}

/// 单个角色（怪物）原型配置。
#[derive(Debug, Clone, Deserialize)]
pub struct MonsterPrototype {
    pub name: String,
    pub element: ElementType,
    pub stats: StatsData,
    pub skills: [SkillId; 4],
}

#[derive(Debug, Clone, Deserialize)]
struct BattleConfig {
    #[serde(default)]
    element_matrix: ElementMatrixConfig,
    skills: Vec<SkillDef>,
    monsters: Vec<MonsterPrototype>,
    cards: Vec<CardDef>,
    deck: Vec<CardId>,
}

/// 战斗数据库资源：组合技能、卡牌和元素克制矩阵。
/// 用于减少系统参数数量，避免超过 Bevy 的 16 参数限制。
#[derive(Resource, Debug, Clone)]
pub struct BattleDbs {
    pub skills: HashMap<SkillId, SkillDef>,
    pub cards: HashMap<CardId, CardDef>,
    pub elements: ElementDb,
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
            });
            commands.insert_resource(MonsterPool { monsters: vec![] });
            commands.insert_resource(CardDeck::default());
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
            });
            commands.insert_resource(MonsterPool { monsters: vec![] });
            commands.insert_resource(CardDeck::default());
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
        });
        commands.insert_resource(MonsterPool { monsters: vec![] });
        commands.insert_resource(CardDeck::default());
        commands.insert_resource(BattleDataStatus {
            error: Some(format!("战斗配置非法: {reason}")),
        });
        return;
    }

    let mut skills = HashMap::new();
    for skill in config.skills {
        skills.insert(skill.id, skill);
    }

    let mut cards = HashMap::new();
    for def in config.cards {
        cards.insert(def.id, def);
    }

    // 加载元素克制矩阵：若配置为空则使用默认值
    let elements = if config.element_matrix.relations.is_empty() {
        ElementDb::from_default_config()
    } else {
        ElementDb::from_config(&config.element_matrix)
    };

    commands.insert_resource(BattleDbs { skills, cards, elements });
    commands.insert_resource(MonsterPool {
        monsters: config.monsters,
    });
    commands.insert_resource(CardDeck(config.deck));
    commands.insert_resource(BattleDataStatus::default());
}

fn validate_battle_config(config: &BattleConfig) -> Result<(), String> {
    if config.skills.is_empty() {
        return Err("skills 不能为空".to_string());
    }
    if config.monsters.is_empty() {
        return Err("monsters 不能为空".to_string());
    }

    let mut skill_ids = HashSet::new();
    for skill in &config.skills {
        if !skill_ids.insert(skill.id) {
            return Err(format!("技能ID重复: {:?}", skill.id));
        }
    }

    for mon in &config.monsters {
        for sid in mon.skills {
            if !skill_ids.contains(&sid) {
                return Err(format!(
                    "角色 {} 使用了未定义技能 {:?}",
                    mon.name, sid
                ));
            }
        }
    }

    Ok(())
}
