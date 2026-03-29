use std::{
    collections::HashMap,
    fs,
};

use bevy::prelude::*;
use serde::Deserialize;

/// 技能唯一标识（逻辑层使用）。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Deserialize)]
pub enum SkillId {
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
    pub stats: StatsData,
    pub skills: [SkillId; 4],
}

#[derive(Debug, Clone, Deserialize)]
struct BattleConfig {
    pub skills: Vec<SkillDef>,
    pub player: MonsterPrototype,
    pub enemy: MonsterPrototype,
}

#[derive(Resource, Debug, Clone)]
pub struct SkillDb(pub HashMap<SkillId, SkillDef>);

#[derive(Resource, Debug, Clone)]
pub struct TeamSetup {
    pub player: MonsterPrototype,
    pub enemy: MonsterPrototype,
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
    let raw = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("failed to read {path}: {e}");
    });
    let config: BattleConfig = ron::from_str(&raw).unwrap_or_else(|e| {
        panic!("failed to parse {path}: {e}");
    });

    let mut skills = HashMap::new();
    for skill in config.skills {
        skills.insert(skill.id, skill);
    }

    commands.insert_resource(SkillDb(skills));
    commands.insert_resource(TeamSetup {
        player: config.player,
        enemy: config.enemy,
    });
}
