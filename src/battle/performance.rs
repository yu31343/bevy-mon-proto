use std::collections::HashSet;

use bevy::prelude::*;

use crate::{
    battle::{BattleEvent, Side},
    data::{BattleDbs, ReactionDef},
};

#[derive(Resource, Debug, Clone, Default)]
pub struct BattlePerformanceStats {
    pub player: SidePerformanceStats,
    pub enemy: SidePerformanceStats,
}

impl BattlePerformanceStats {
    fn side_mut(&mut self, side: Side) -> &mut SidePerformanceStats {
        match side {
            Side::Player => &mut self.player,
            Side::Enemy => &mut self.enemy,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SidePerformanceStats {
    pub ap_spent: i32,
    pub cards_used: u32,
    pub cards_discarded: u32,
    pub high_cost_skills: u32,
    pub damage_dealt: i32,
    pub healing_done: i32,
    pub shield_absorbed: i32,
    pub two_element_reactions: u32,
    pub advanced_reactions: u32,
    pub wind_spreads: u32,
    pub knockouts: u32,
    pub misses: u32,
    reaction_names: HashSet<String>,
}

impl SidePerformanceStats {
    pub fn unique_reaction_count(&self) -> u32 {
        self.reaction_names.len() as u32
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct BattlePerformanceReport {
    pub summary: Option<BattlePerformanceSummary>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BattlePerformanceSummary {
    pub grade: &'static str,
    pub total_score: u32,
    pub dimensions: PerformanceDimensions,
    pub counters: PerformanceCounters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceDimensions {
    pub chain: u32,
    pub resource: u32,
    pub offense: u32,
    pub tempo: u32,
    pub survival: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceCounters {
    pub two_element_reactions: u32,
    pub advanced_reactions: u32,
    pub ap_spent: i32,
    pub rounds: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattlePerformanceOutcome {
    Victory,
    Draw,
    Defeat,
}

impl BattlePerformanceOutcome {
    fn score_cap(self) -> u32 {
        match self {
            Self::Victory => 100,
            Self::Draw => 80,
            Self::Defeat => 65,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PerformanceTeamSnapshot {
    pub current_hp: i32,
    pub max_hp: i32,
    pub alive_count: usize,
    pub member_count: usize,
}

pub fn record_performance_event(
    stats: &mut BattlePerformanceStats,
    dbs: &BattleDbs,
    event: &BattleEvent,
) {
    match event {
        BattleEvent::SkillUsed { side, cost_ap, .. } => {
            let side_stats = stats.side_mut(*side);
            side_stats.ap_spent += (*cost_ap).max(0);
            if *cost_ap >= 3 {
                side_stats.high_cost_skills += 1;
            }
        }
        BattleEvent::CardUsed { side, cost_ap, .. } => {
            let side_stats = stats.side_mut(*side);
            side_stats.ap_spent += (*cost_ap).max(0);
            side_stats.cards_used += 1;
        }
        BattleEvent::CardDiscarded { side, .. } => {
            stats.side_mut(*side).cards_discarded += 1;
        }
        BattleEvent::DamageDealt { source, amount, .. } if *amount > 0 => {
            stats.side_mut(*source).damage_dealt += *amount;
        }
        BattleEvent::AttackMissed { source, .. } => {
            stats.side_mut(*source).misses += 1;
        }
        BattleEvent::ShieldAbsorbed { side, amount } if *amount > 0 => {
            stats.side_mut(*side).shield_absorbed += *amount;
        }
        BattleEvent::Healed { side, amount } if *amount > 0 => {
            stats.side_mut(*side).healing_done += *amount;
        }
        BattleEvent::ReactionTriggered {
            source,
            reaction_id,
            reaction_name,
            ..
        } => {
            let side_stats = stats.side_mut(*source);
            side_stats.reaction_names.insert(reaction_name.clone());
            if let Some(reaction) = dbs
                .reactions
                .reactions
                .iter()
                .find(|reaction| reaction.id == *reaction_id)
            {
                match reaction_performance_kind(reaction) {
                    ReactionPerformanceKind::TwoElement => side_stats.two_element_reactions += 1,
                    ReactionPerformanceKind::Advanced => side_stats.advanced_reactions += 1,
                    ReactionPerformanceKind::Other => {}
                }
            }
        }
        BattleEvent::WindSpreadTriggered { source, .. } => {
            stats.side_mut(*source).wind_spreads += 1;
        }
        BattleEvent::CombatantFainted { side, .. } => {
            let scorer = match side {
                Side::Player => Side::Enemy,
                Side::Enemy => Side::Player,
            };
            stats.side_mut(scorer).knockouts += 1;
        }
        _ => {}
    }
}

pub fn build_performance_report(
    stats: &BattlePerformanceStats,
    outcome: BattlePerformanceOutcome,
    rounds: u32,
    player: PerformanceTeamSnapshot,
    enemy: PerformanceTeamSnapshot,
) -> BattlePerformanceSummary {
    let player_stats = &stats.player;
    let chain = chain_score(player_stats);
    let resource = resource_score(player_stats, rounds);
    let offense = offense_score(player_stats, enemy);
    let tempo = tempo_score(outcome, rounds, enemy.member_count);
    let survival = survival_score(outcome, player, player_stats);
    let dimensions = PerformanceDimensions {
        chain,
        resource,
        offense,
        tempo,
        survival,
    };
    let uncapped_total = chain + resource + offense + tempo + survival;
    let total_score = uncapped_total.min(outcome.score_cap());
    BattlePerformanceSummary {
        grade: grade_for_score(total_score),
        total_score,
        dimensions,
        counters: PerformanceCounters {
            two_element_reactions: player_stats.two_element_reactions,
            advanced_reactions: player_stats.advanced_reactions,
            ap_spent: player_stats.ap_spent,
            rounds,
        },
    }
}

pub fn format_performance_report(summary: &BattlePerformanceSummary) -> String {
    format!(
        "操作评级：{} {}\n连锁 {}/30 · 资源 {}/25 · 进攻 {}/20 · 节奏 {}/10 · 生存 {}/15\n二元素反应 {} · 进阶反应 {} · AP消耗 {} · {}回合",
        summary.grade,
        summary.total_score,
        summary.dimensions.chain,
        summary.dimensions.resource,
        summary.dimensions.offense,
        summary.dimensions.tempo,
        summary.dimensions.survival,
        summary.counters.two_element_reactions,
        summary.counters.advanced_reactions,
        summary.counters.ap_spent,
        summary.counters.rounds
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReactionPerformanceKind {
    TwoElement,
    Advanced,
    Other,
}

fn reaction_performance_kind(reaction: &ReactionDef) -> ReactionPerformanceKind {
    if reaction.required_elements.len() >= 3 || !reaction.required_statuses.is_empty() {
        ReactionPerformanceKind::Advanced
    } else if reaction.required_elements.len() == 2 {
        ReactionPerformanceKind::TwoElement
    } else {
        ReactionPerformanceKind::Other
    }
}

fn chain_score(stats: &SidePerformanceStats) -> u32 {
    let reaction_score = (stats.two_element_reactions * 4).min(16)
        + (stats.advanced_reactions * 8).min(16)
        + (stats.wind_spreads * 3).min(6)
        + stats.unique_reaction_count().min(4);
    reaction_score.min(30)
}

fn resource_score(stats: &SidePerformanceStats, rounds: u32) -> u32 {
    let ap_budget = (rounds as i32 * 6).max(1) as f32;
    let ap_efficiency = 12.0 * ((stats.ap_spent.max(0) as f32) / ap_budget).min(1.0);
    let card_use = (stats.cards_used as f32 * 1.5).min(6.0);
    let discard = stats.cards_discarded.min(4) as f32;
    let high_cost_skill = (stats.high_cost_skills as f32 * 1.5).min(3.0);
    round_clamped(ap_efficiency + card_use + discard + high_cost_skill, 0, 25)
}

fn offense_score(stats: &SidePerformanceStats, enemy: PerformanceTeamSnapshot) -> u32 {
    let hp_lost = (enemy.max_hp - enemy.current_hp).max(0) as f32;
    let hp_pressure = if enemy.max_hp > 0 {
        10.0 * (hp_lost / enemy.max_hp as f32).min(1.0)
    } else {
        0.0
    };
    let damage_efficiency = 6.0
        * (stats.damage_dealt.max(0) as f32 / (stats.ap_spent.max(0) * 5).max(1) as f32).min(1.0);
    let knockouts = (stats.knockouts * 3).min(6) as f32;
    let misses = stats.misses as f32 * 2.0;
    round_clamped(hp_pressure + damage_efficiency + knockouts - misses, 0, 20)
}

fn tempo_score(outcome: BattlePerformanceOutcome, rounds: u32, enemy_team_size: usize) -> u32 {
    let target_rounds = (enemy_team_size as u32 * 3).max(3);
    let overtime = rounds.saturating_sub(target_rounds);
    let score = 10i32.saturating_sub(overtime as i32 * 2).max(0) as u32;
    if outcome == BattlePerformanceOutcome::Victory {
        score
    } else {
        score.min(4)
    }
}

fn survival_score(
    outcome: BattlePerformanceOutcome,
    player: PerformanceTeamSnapshot,
    stats: &SidePerformanceStats,
) -> u32 {
    let hp_ratio = if player.max_hp > 0 {
        (player.current_hp.max(0) as f32 / player.max_hp as f32).min(1.0)
    } else {
        0.0
    };
    let alive_ratio = if player.member_count > 0 {
        player.alive_count as f32 / player.member_count as f32
    } else {
        0.0
    };
    let sustain = ((stats.healing_done + stats.shield_absorbed).max(0) / 20).min(3) as f32;
    let score = round_clamped(hp_ratio * 8.0 + alive_ratio * 4.0 + sustain, 0, 15);
    if outcome == BattlePerformanceOutcome::Defeat {
        score.min(4)
    } else {
        score
    }
}

fn grade_for_score(score: u32) -> &'static str {
    match score {
        90..=100 => "S",
        80..=89 => "A",
        70..=79 => "B",
        60..=69 => "C",
        _ => "D",
    }
}

fn round_clamped(value: f32, min: u32, max: u32) -> u32 {
    value.round().clamp(min as f32, max as f32) as u32
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        battle::{DamageType, Side},
        data::{ElementDb, ElementType, ReactionDb, ReactionDef, StatusDb},
    };

    use super::*;

    fn test_dbs(reactions: Vec<ReactionDef>) -> BattleDbs {
        BattleDbs {
            skills: HashMap::new(),
            cards: HashMap::new(),
            elements: ElementDb::default(),
            statuses: StatusDb::default(),
            reactions: ReactionDb { reactions },
        }
    }

    fn reaction(
        id: &str,
        name: &str,
        elements: Vec<ElementType>,
        statuses: Vec<&str>,
    ) -> ReactionDef {
        ReactionDef {
            id: id.to_string(),
            name: name.to_string(),
            required_elements: elements,
            required_statuses: statuses.into_iter().map(str::to_string).collect(),
            trigger_element: ElementType::Fire,
            fixed_damage: 0,
            heal_attacker: 0,
            apply_statuses: Vec::new(),
            clear_statuses: Vec::new(),
            clear_elements: Vec::new(),
            preserve_current_auras: false,
            aura_results: Vec::new(),
        }
    }

    #[test]
    fn score_grade_uses_expected_thresholds() {
        assert_eq!(grade_for_score(90), "S");
        assert_eq!(grade_for_score(80), "A");
        assert_eq!(grade_for_score(70), "B");
        assert_eq!(grade_for_score(60), "C");
        assert_eq!(grade_for_score(59), "D");
    }

    #[test]
    fn reaction_classification_counts_two_element_and_advanced() {
        let two = reaction(
            "vaporize",
            "蒸发",
            vec![ElementType::Fire, ElementType::Water],
            vec![],
        );
        let advanced = reaction(
            "steam_thunder_explosion",
            "蒸汽雷爆",
            vec![ElementType::Water, ElementType::Thunder, ElementType::Fire],
            vec![],
        );
        let status_advanced = reaction(
            "electro_bloom",
            "感电绽放",
            vec![ElementType::Thunder],
            vec!["seeded"],
        );
        let dbs = test_dbs(vec![two, advanced, status_advanced]);
        let mut stats = BattlePerformanceStats::default();

        record_performance_event(
            &mut stats,
            &dbs,
            &BattleEvent::ReactionTriggered {
                source: Side::Player,
                target: Side::Enemy,
                reaction_id: "vaporize".to_string(),
                reaction_name: "蒸发".to_string(),
            },
        );
        record_performance_event(
            &mut stats,
            &dbs,
            &BattleEvent::ReactionTriggered {
                source: Side::Player,
                target: Side::Enemy,
                reaction_id: "steam_thunder_explosion".to_string(),
                reaction_name: "蒸汽雷爆".to_string(),
            },
        );
        record_performance_event(
            &mut stats,
            &dbs,
            &BattleEvent::ReactionTriggered {
                source: Side::Player,
                target: Side::Enemy,
                reaction_id: "electro_bloom".to_string(),
                reaction_name: "感电绽放".to_string(),
            },
        );

        assert_eq!(stats.player.two_element_reactions, 1);
        assert_eq!(stats.player.advanced_reactions, 2);
        assert_eq!(stats.player.unique_reaction_count(), 3);
    }

    #[test]
    fn failure_report_is_capped_at_65() {
        let mut stats = BattlePerformanceStats::default();
        stats.player.ap_spent = 60;
        stats.player.damage_dealt = 999;
        stats.player.knockouts = 3;
        stats.player.two_element_reactions = 4;
        stats.player.advanced_reactions = 4;
        stats.player.wind_spreads = 4;
        stats.player.cards_used = 8;
        stats.player.cards_discarded = 8;
        stats.player.high_cost_skills = 4;
        stats.player.healing_done = 100;
        stats.player.shield_absorbed = 100;

        let report = build_performance_report(
            &stats,
            BattlePerformanceOutcome::Defeat,
            3,
            PerformanceTeamSnapshot {
                current_hp: 100,
                max_hp: 100,
                alive_count: 3,
                member_count: 3,
            },
            PerformanceTeamSnapshot {
                current_hp: 0,
                max_hp: 100,
                alive_count: 0,
                member_count: 3,
            },
        );

        assert_eq!(report.total_score, 65);
        assert_eq!(report.grade, "C");
    }

    #[test]
    fn event_accumulation_tracks_player_action_stats() {
        let dbs = test_dbs(vec![]);
        let mut stats = BattlePerformanceStats::default();

        record_performance_event(
            &mut stats,
            &dbs,
            &BattleEvent::SkillUsed {
                side: Side::Player,
                skill_id: crate::data::SkillId::FirePunch,
                skill_name: "火焰拳".to_string(),
                slot: 0,
                cost_ap: 3,
            },
        );
        record_performance_event(
            &mut stats,
            &dbs,
            &BattleEvent::CardUsed {
                side: Side::Player,
                card_id: crate::data::CardId::GainAp,
                card_name: "行动充能".to_string(),
                cost_ap: 1,
            },
        );
        record_performance_event(
            &mut stats,
            &dbs,
            &BattleEvent::CardDiscarded {
                side: Side::Player,
                card_id: crate::data::CardId::GainAp,
                card_name: "行动充能".to_string(),
                ap_gain: 1,
            },
        );
        record_performance_event(
            &mut stats,
            &dbs,
            &BattleEvent::DamageDealt {
                source: Side::Player,
                target: Side::Enemy,
                amount: 12,
                damage_type: DamageType::Direct,
            },
        );
        record_performance_event(
            &mut stats,
            &dbs,
            &BattleEvent::ShieldAbsorbed {
                side: Side::Player,
                amount: 8,
            },
        );
        record_performance_event(
            &mut stats,
            &dbs,
            &BattleEvent::CombatantFainted {
                owner: Entity::PLACEHOLDER,
                side: Side::Enemy,
                name: "敌方".to_string(),
            },
        );

        assert_eq!(stats.player.ap_spent, 4);
        assert_eq!(stats.player.high_cost_skills, 1);
        assert_eq!(stats.player.cards_used, 1);
        assert_eq!(stats.player.cards_discarded, 1);
        assert_eq!(stats.player.damage_dealt, 12);
        assert_eq!(stats.player.shield_absorbed, 8);
        assert_eq!(stats.player.knockouts, 1);
    }
}
