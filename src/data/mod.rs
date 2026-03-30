use std::{
    collections::{HashMap, HashSet},
    fs,
};

use bevy::prelude::*;
use serde::Deserialize;

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

/// 元素克制矩阵。
/// 行表示攻击方，列表示防御方。
/// 矩阵[i][j] 表示元素 i 攻击元素 j 的伤害倍数。
/// 2.0 = 克制（超有效），1.0 = 正常，0.5 = 微弱。
pub struct ElementMatrix;

impl ElementMatrix {
    /// 获取攻击方元素对防御方元素的伤害倍数。
    /// 克制关系（2.0）、微弱关系（0.5），其他情况默认为正常伤害（1.0）。
    pub fn get_effectiveness(attacker: ElementType, defender: ElementType) -> f32 {
        match (attacker, defender) {
            // 光/暗
            (ElementType::Light, ElementType::Dark) => 2.0,
            (ElementType::Dark, ElementType::Light) => 0.5,

            // 水/火/草（保留原有循环）
            (ElementType::Water, ElementType::Fire) => 2.0,
            (ElementType::Fire, ElementType::Grass) => 2.0,
            (ElementType::Grass, ElementType::Water) => 2.0,
            (ElementType::Water, ElementType::Grass) => 0.5,
            (ElementType::Fire, ElementType::Water) => 0.5,
            (ElementType::Grass, ElementType::Fire) => 0.5,

            // 雷/水/风/草（“常识化合理”）
            (ElementType::Thunder, ElementType::Water) => 2.0,
            (ElementType::Water, ElementType::Thunder) => 0.5,

            (ElementType::Thunder, ElementType::Wind) => 2.0,
            (ElementType::Wind, ElementType::Thunder) => 0.5,

            (ElementType::Thunder, ElementType::Grass) => 0.5,
            (ElementType::Grass, ElementType::Thunder) => 1.0,

            // 风/草/火/雷（“风克草、火克风、雷惧风”）
            (ElementType::Wind, ElementType::Grass) => 2.0,
            (ElementType::Grass, ElementType::Wind) => 1.0,

            (ElementType::Fire, ElementType::Wind) => 2.0,
            (ElementType::Wind, ElementType::Fire) => 0.5,

            // 其他所有情况（包括相同系别）都是正常伤害
            _ => 1.0,
        }
    }
}

/// 元素附着/反应的“盾免疫”判定：
/// 当护盾吸收了本次伤害的一部分（absorbed > 0）时，不触发元素附着/消耗。
pub fn shield_blocks_element_attachment(shield: i32, theoretical_damage: i32) -> bool {
    shield.min(theoretical_damage) > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_matrix_basic_cycles() {
        assert_eq!(
            ElementMatrix::get_effectiveness(ElementType::Water, ElementType::Fire),
            2.0
        );
        assert_eq!(
            ElementMatrix::get_effectiveness(ElementType::Fire, ElementType::Grass),
            2.0
        );
        assert_eq!(
            ElementMatrix::get_effectiveness(ElementType::Grass, ElementType::Water),
            2.0
        );

        assert_eq!(
            ElementMatrix::get_effectiveness(ElementType::Water, ElementType::Grass),
            0.5
        );
    }

    #[test]
    fn element_matrix_light_dark() {
        assert_eq!(
            ElementMatrix::get_effectiveness(ElementType::Light, ElementType::Dark),
            2.0
        );
        assert_eq!(
            ElementMatrix::get_effectiveness(ElementType::Dark, ElementType::Light),
            0.5
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
    pub skills: Vec<SkillDef>,
    pub player: Vec<MonsterPrototype>,
    pub enemy: Vec<MonsterPrototype>,
}

#[derive(Resource, Debug, Clone)]
pub struct SkillDb(pub HashMap<SkillId, SkillDef>);

#[derive(Resource, Debug, Clone)]
pub struct TeamSetup {
    pub player: Vec<MonsterPrototype>,
    pub enemy: Vec<MonsterPrototype>,
}

/// 技能卡唯一标识。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum CardId {
    /// 使用后获得行动点（示例：消耗 1 点，获得 +2 点）。
    GainAp,
    /// 使用后给“下次进攻”附加伤害增益（示例：消耗 2 点，给 +5）。
    NextAttackBoost,
    /// 使用后给“下次防御”附加护盾增益（示例：消耗 2 点，给 +5）。
    NextShieldBoost,
    /// 使用后给“下次治疗”附加回复增益（示例：消耗 2 点，给 +5）。
    NextHealBoost,
}

/// 技能卡效果定义。
#[derive(Debug, Clone, Copy)]
pub enum CardEffect {
    GainAp { amount: i32 },
    NextAttackBoost { amount: i32 },
    NextShieldBoost { amount: i32 },
    NextHealBoost { amount: i32 },
}

#[derive(Debug, Clone)]
pub struct CardDef {
    pub id: CardId,
    pub name: &'static str,
    pub cost_ap: i32,
    pub effect: CardEffect,
}

/// 技能卡数据库（当前原型：卡表内置，不走 RON）。
#[derive(Resource, Debug, Clone)]
pub struct CardDb(pub HashMap<CardId, CardDef>);

/// 技能卡组（每回合从中抽取固定数量的卡）。
#[derive(Resource, Debug, Clone)]
pub struct CardDeck(pub Vec<CardId>);

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
            commands.insert_resource(SkillDb(HashMap::new()));
            commands.insert_resource(TeamSetup {
                player: vec![],
                enemy: vec![],
            });
            commands.insert_resource(BattleDataStatus {
                error: Some(format!("读取战斗配置失败: {path} ({e})")),
            });
            return;
        }
    };
    let config: BattleConfig = match ron::from_str(&raw) {
        Ok(config) => config,
        Err(e) => {
            commands.insert_resource(SkillDb(HashMap::new()));
            commands.insert_resource(TeamSetup {
                player: vec![],
                enemy: vec![],
            });
            commands.insert_resource(BattleDataStatus {
                error: Some(format!("解析战斗配置失败: {path} ({e})")),
            });
            return;
        }
    };

    if let Err(reason) = validate_battle_config(&config) {
        commands.insert_resource(SkillDb(HashMap::new()));
        commands.insert_resource(TeamSetup {
            player: vec![],
            enemy: vec![],
        });
        commands.insert_resource(BattleDataStatus {
            error: Some(format!("战斗配置非法: {reason}")),
        });
        return;
    }

    let mut skills = HashMap::new();
    for skill in config.skills {
        skills.insert(skill.id, skill);
    }

    commands.insert_resource(SkillDb(skills));
    commands.insert_resource(TeamSetup {
        player: config.player,
        enemy: config.enemy,
    });

    // 简易技能卡组：为了原型先内置几种卡并重复组成“牌库”。
    // 后续可把它迁移到 RON 数据驱动。
    let mut card_map = HashMap::new();
    let card_defs = [
        CardDef {
            id: CardId::GainAp,
            name: "行动充能",
            cost_ap: 1,
            effect: CardEffect::GainAp { amount: 2 },
        },
        CardDef {
            id: CardId::NextAttackBoost,
            name: "猛攻许可",
            cost_ap: 2,
            effect: CardEffect::NextAttackBoost { amount: 5 },
        },
        CardDef {
            id: CardId::NextShieldBoost,
            name: "守备许可",
            cost_ap: 2,
            effect: CardEffect::NextShieldBoost { amount: 5 },
        },
        CardDef {
            id: CardId::NextHealBoost,
            name: "治疗许可",
            cost_ap: 2,
            effect: CardEffect::NextHealBoost { amount: 5 },
        },
    ];
    for def in card_defs {
        card_map.insert(def.id, def);
    }
    commands.insert_resource(CardDb(card_map));

    // 牌库包含重复卡，用于“抽取”时有一定可选性。
    commands.insert_resource(CardDeck(vec![
        CardId::GainAp,
        CardId::GainAp,
        CardId::NextAttackBoost,
        CardId::NextShieldBoost,
        CardId::GainAp,
        CardId::NextHealBoost,
        CardId::NextAttackBoost,
        CardId::GainAp,
        CardId::NextShieldBoost,
        CardId::NextHealBoost,
    ]));

    commands.insert_resource(BattleDataStatus::default());
}

fn validate_battle_config(config: &BattleConfig) -> Result<(), String> {
    if config.skills.is_empty() {
        return Err("skills 不能为空".to_string());
    }
    if config.player.is_empty() {
        return Err("player 队伍不能为空".to_string());
    }
    if config.enemy.is_empty() {
        return Err("enemy 队伍不能为空".to_string());
    }

    let mut skill_ids = HashSet::new();
    for skill in &config.skills {
        if !skill_ids.insert(skill.id) {
            return Err(format!("技能ID重复: {:?}", skill.id));
        }
    }

    for (team_name, team) in [("player", &config.player), ("enemy", &config.enemy)] {
        for mon in team {
            for sid in mon.skills {
                if !skill_ids.contains(&sid) {
                    return Err(format!(
                        "{team_name} 队伍中的角色 {} 使用了未定义技能 {:?}",
                        mon.name, sid
                    ));
                }
            }
        }
    }

    Ok(())
}
