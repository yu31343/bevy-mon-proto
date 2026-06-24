use std::{fs, io::Write, path::PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::{AiDifficulty, CardId, ElementType, EnemyAiConfig, EnemyAiPolicyMode, SkillId};

use super::{EnemyAiPlan, EnemyPlannedAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum AiActionKind {
    UseCardForSkill,
    UseSkill,
    Switch,
    ImmediateCard,
    Discard,
    EndTurn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct AiCombatantObservation {
    pub(crate) hp: i32,
    pub(crate) max_hp: i32,
    pub(crate) shield: i32,
    pub(crate) atk: i32,
    pub(crate) def: i32,
    pub(crate) atk_stage: i32,
    pub(crate) def_stage: i32,
    pub(crate) spd_stage: i32,
    pub(crate) acc_stage: i32,
    pub(crate) element: ElementType,
    pub(crate) attached_auras: [Option<ElementType>; 2],
    pub(crate) status_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct AiSideObservation {
    pub(crate) active_index: usize,
    pub(crate) alive_count: usize,
    pub(crate) member_count: usize,
    pub(crate) ap: i32,
    pub(crate) active: AiCombatantObservation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct AiObservation {
    pub(crate) round: u32,
    pub(crate) battle_seed: u64,
    pub(crate) difficulty: AiDifficulty,
    pub(crate) enemy: AiSideObservation,
    pub(crate) player: AiSideObservation,
    pub(crate) enemy_hand: Vec<CardId>,
    pub(crate) player_hand_count: usize,
    pub(crate) player_hand: Option<Vec<CardId>>,
    pub(crate) enemy_skill_uses_remaining: [u8; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct AiActionFeatures {
    pub(crate) kind: AiActionKind,
    pub(crate) heuristic_score: f32,
    pub(crate) estimated_value: f32,
    pub(crate) skill_slot: Option<usize>,
    pub(crate) skill_id: Option<SkillId>,
    pub(crate) card_index: Option<usize>,
    pub(crate) card_id: Option<CardId>,
    pub(crate) switch_index: Option<usize>,
    pub(crate) summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum AiDecisionSource {
    Heuristic,
    ModelRanker,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum AiBattleOutcome {
    EnemyVictory,
    EnemyDefeat,
    Draw,
}

impl AiBattleOutcome {
    fn label(self) -> &'static str {
        match self {
            Self::EnemyVictory => "enemy_victory",
            Self::EnemyDefeat => "enemy_defeat",
            Self::Draw => "draw",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct AiTeamSnapshot {
    pub(crate) current_hp: i32,
    pub(crate) max_hp: i32,
    pub(crate) alive_count: usize,
    pub(crate) member_count: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct AiSideEpisodeStats {
    pub(crate) damage_dealt: i32,
    pub(crate) knockouts: u32,
    pub(crate) two_element_reactions: u32,
    pub(crate) advanced_reactions: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct AiEpisodeStats {
    pub(crate) enemy: AiSideEpisodeStats,
    pub(crate) player: AiSideEpisodeStats,
    pub(crate) enemy_team: AiTeamSnapshot,
    pub(crate) player_team: AiTeamSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AiDecisionSample {
    pub(crate) seq: u64,
    pub(crate) round: u32,
    pub(crate) battle_seed: u64,
    pub(crate) difficulty: AiDifficulty,
    pub(crate) policy_mode: EnemyAiPolicyMode,
    pub(crate) decision_source: AiDecisionSource,
    pub(crate) observation: AiObservation,
    pub(crate) candidates: Vec<AiActionFeatures>,
    pub(crate) heuristic_scores: Vec<f32>,
    pub(crate) model_scores: Option<Vec<f32>>,
    pub(crate) chosen_index: usize,
    pub(crate) outcome: Option<AiBattleOutcome>,
    pub(crate) reward: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) episode_stats: Option<AiEpisodeStats>,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct AiDecisionLog {
    samples: Vec<AiDecisionSample>,
    next_seq: u64,
}

impl AiDecisionLog {
    pub(crate) fn clear(&mut self) {
        self.samples.clear();
        self.next_seq = 0;
    }

    pub(crate) fn record(&mut self, mut sample: AiDecisionSample) {
        self.next_seq = self.next_seq.saturating_add(1);
        sample.seq = self.next_seq;
        self.samples.push(sample);
    }

    pub(crate) fn finalize_with_reward(&mut self, outcome: AiBattleOutcome, reward: f32) {
        for sample in &mut self.samples {
            sample.outcome = Some(outcome);
            sample.reward = Some(reward);
        }
    }

    pub(crate) fn export_if_enabled(
        &self,
        config: &EnemyAiConfig,
    ) -> Result<Option<String>, String> {
        if !should_collect_decision_samples(config) || self.samples.is_empty() {
            return Ok(None);
        }

        let export_dir = PathBuf::from(&config.decision_sample_export_dir);
        fs::create_dir_all(&export_dir).map_err(|err| format!("创建 AI 样本目录失败：{err}"))?;

        let seed = self
            .samples
            .first()
            .map(|sample| sample.battle_seed)
            .unwrap_or_default();
        let outcome = self
            .samples
            .first()
            .and_then(|sample| sample.outcome)
            .map(AiBattleOutcome::label)
            .unwrap_or("unfinished");
        let ron_path = export_dir.join(format!(
            "ai_decisions_seed_{seed}_{outcome}_n{}.ron",
            self.samples.len()
        ));
        let jsonl_path = export_dir.join(format!(
            "ai_decisions_seed_{seed}_{outcome}_n{}.jsonl",
            self.samples.len()
        ));
        let text = ron::ser::to_string_pretty(&self.samples, ron::ser::PrettyConfig::default())
            .map_err(|err| format!("序列化 AI 决策样本失败：{err}"))?;
        fs::write(&ron_path, text).map_err(|err| format!("写入 {:?} 失败：{err}", ron_path))?;

        let mut jsonl_file = fs::File::create(&jsonl_path)
            .map_err(|err| format!("创建 {:?} 失败：{err}", jsonl_path))?;
        for sample in &self.samples {
            serde_json::to_writer(&mut jsonl_file, sample)
                .map_err(|err| format!("序列化 AI 决策 JSONL 失败：{err}"))?;
            jsonl_file
                .write_all(b"\n")
                .map_err(|err| format!("写入 {:?} 失败：{err}", jsonl_path))?;
        }
        Ok(Some(format!(
            "{}、{}",
            ron_path.display(),
            jsonl_path.display()
        )))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct LinearAiRankerModel {
    #[serde(default)]
    bias: f32,
    #[serde(default = "default_heuristic_score_weight")]
    heuristic_score_weight: f32,
    #[serde(default)]
    estimated_value_weight: f32,
    #[serde(default)]
    use_card_for_skill_weight: f32,
    #[serde(default)]
    use_skill_weight: f32,
    #[serde(default)]
    switch_weight: f32,
    #[serde(default)]
    immediate_card_weight: f32,
    #[serde(default)]
    discard_weight: f32,
    #[serde(default)]
    end_turn_weight: f32,
    #[serde(default)]
    low_enemy_hp_weight: f32,
    #[serde(default)]
    low_player_hp_weight: f32,
}

fn default_heuristic_score_weight() -> f32 {
    1.0
}

impl LinearAiRankerModel {
    fn score(&self, observation: &AiObservation, candidate: &AiActionFeatures) -> f32 {
        let enemy_hp_ratio = hp_ratio(observation.enemy.active.hp, observation.enemy.active.max_hp);
        let player_hp_ratio = hp_ratio(
            observation.player.active.hp,
            observation.player.active.max_hp,
        );

        self.bias
            + candidate.heuristic_score * self.heuristic_score_weight
            + candidate.estimated_value * self.estimated_value_weight
            + self.action_kind_weight(candidate.kind)
            + (1.0 - enemy_hp_ratio) * self.low_enemy_hp_weight
            + (1.0 - player_hp_ratio) * self.low_player_hp_weight
    }

    fn action_kind_weight(&self, kind: AiActionKind) -> f32 {
        match kind {
            AiActionKind::UseCardForSkill => self.use_card_for_skill_weight,
            AiActionKind::UseSkill => self.use_skill_weight,
            AiActionKind::Switch => self.switch_weight,
            AiActionKind::ImmediateCard => self.immediate_card_weight,
            AiActionKind::Discard => self.discard_weight,
            AiActionKind::EndTurn => self.end_turn_weight,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub(crate) struct AiPolicyRuntime {
    loaded_path: Option<String>,
    model: Option<LinearAiRankerModel>,
    last_error: Option<String>,
}

impl AiPolicyRuntime {
    fn model_for_path(&mut self, path: &str) -> Option<&LinearAiRankerModel> {
        if self.loaded_path.as_deref() == Some(path) {
            return self.model.as_ref();
        }

        self.loaded_path = Some(path.to_string());
        self.model = None;
        self.last_error = None;

        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                self.last_error = Some(format!("读取 AI 模型失败：{err}"));
                return None;
            }
        };
        match ron::from_str::<LinearAiRankerModel>(&text) {
            Ok(model) => {
                self.model = Some(model);
                self.model.as_ref()
            }
            Err(err) => {
                self.last_error = Some(format!("解析 AI 模型失败：{err}"));
                None
            }
        }
    }

    pub(crate) fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AiModelSelection {
    pub(crate) index: usize,
    pub(crate) scores: Vec<f32>,
}

pub(crate) fn should_collect_decision_samples(config: &EnemyAiConfig) -> bool {
    config.export_decision_samples
        || matches!(
            config.policy_mode,
            EnemyAiPolicyMode::CollectOnly | EnemyAiPolicyMode::SelfPlayTraining
        )
}

pub(crate) fn reward_for_outcome(outcome: AiBattleOutcome) -> f32 {
    match outcome {
        AiBattleOutcome::EnemyVictory => 1.0,
        AiBattleOutcome::EnemyDefeat => -1.0,
        AiBattleOutcome::Draw => 0.0,
    }
}

pub(crate) fn reward_for_episode(outcome: AiBattleOutcome, stats: AiEpisodeStats) -> f32 {
    let terminal = reward_for_outcome(outcome);
    let damage_balance =
        ((stats.enemy.damage_dealt - stats.player.damage_dealt) as f32 / 100.0).clamp(-0.12, 0.12);
    let knockout_balance = ((stats.enemy.knockouts as i32 - stats.player.knockouts as i32) as f32
        * 0.08)
        .clamp(-0.12, 0.12);
    let reaction_bonus = (stats.enemy.two_element_reactions as f32 * 0.01
        + stats.enemy.advanced_reactions as f32 * 0.03)
        .min(0.08);
    let survival_balance = ((team_hp_ratio(stats.enemy_team) - team_hp_ratio(stats.player_team))
        * 0.15)
        .clamp(-0.10, 0.10);
    let shaping =
        (damage_balance + knockout_balance + reaction_bonus + survival_balance).clamp(-0.25, 0.25);
    (terminal + shaping).clamp(-1.25, 1.25)
}

fn team_hp_ratio(snapshot: AiTeamSnapshot) -> f32 {
    if snapshot.max_hp <= 0 {
        0.0
    } else {
        (snapshot.current_hp.max(0) as f32 / snapshot.max_hp as f32).clamp(0.0, 1.0)
    }
}

pub(crate) fn build_action_features(
    plans: &[EnemyAiPlan],
    enemy_hand: &[CardId],
) -> Vec<AiActionFeatures> {
    plans
        .iter()
        .map(|plan| action_features_from_plan(plan, enemy_hand))
        .collect()
}

pub(crate) fn score_candidates(
    config: &EnemyAiConfig,
    runtime: &mut AiPolicyRuntime,
    observation: &AiObservation,
    candidates: &[AiActionFeatures],
) -> Option<AiModelSelection> {
    if config.policy_mode != EnemyAiPolicyMode::ModelRanker || candidates.is_empty() {
        return None;
    }
    let path = config.policy_model_path.as_deref()?.trim();
    if path.is_empty() {
        return None;
    }

    let model = runtime.model_for_path(path)?;
    let scores = candidates
        .iter()
        .map(|candidate| model.score(observation, candidate))
        .collect::<Vec<_>>();
    best_model_score_index(&scores, candidates.len())
        .map(|index| AiModelSelection { index, scores })
}

pub(crate) fn best_model_score_index(scores: &[f32], candidate_count: usize) -> Option<usize> {
    if candidate_count == 0 || scores.len() != candidate_count {
        return None;
    }
    if scores.iter().any(|score| !score.is_finite()) {
        return None;
    }

    scores
        .iter()
        .copied()
        .enumerate()
        .max_by(|(index_a, score_a), (index_b, score_b)| {
            score_a
                .partial_cmp(score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| index_b.cmp(index_a))
        })
        .map(|(index, _)| index)
}

fn action_features_from_plan(plan: &EnemyAiPlan, enemy_hand: &[CardId]) -> AiActionFeatures {
    let mut features = AiActionFeatures {
        kind: AiActionKind::EndTurn,
        heuristic_score: plan.score,
        estimated_value: plan.score,
        skill_slot: None,
        skill_id: None,
        card_index: None,
        card_id: None,
        switch_index: None,
        summary: plan.summary.clone(),
    };

    match &plan.action {
        EnemyPlannedAction::UseCardForSkill { card_index, skill } => {
            features.kind = AiActionKind::UseCardForSkill;
            features.card_index = Some(*card_index);
            features.card_id = enemy_hand.get(*card_index).copied();
            features.skill_slot = Some(skill.slot);
            features.skill_id = Some(skill.skill_id);
        }
        EnemyPlannedAction::UseSkill(skill) => {
            features.kind = AiActionKind::UseSkill;
            features.skill_slot = Some(skill.slot);
            features.skill_id = Some(skill.skill_id);
        }
        EnemyPlannedAction::Switch(switch) => {
            features.kind = AiActionKind::Switch;
            features.switch_index = Some(switch.index);
        }
        EnemyPlannedAction::ImmediateCard(card) => {
            features.kind = AiActionKind::ImmediateCard;
            features.card_index = Some(card.index);
            features.card_id = enemy_hand.get(card.index).copied();
        }
        EnemyPlannedAction::Discard(discard) => {
            features.kind = AiActionKind::Discard;
            features.card_index = Some(discard.index);
            features.card_id = enemy_hand.get(discard.index).copied();
        }
        EnemyPlannedAction::EndTurn => {}
    }

    features
}

fn hp_ratio(hp: i32, max_hp: i32) -> f32 {
    if max_hp <= 0 {
        0.0
    } else {
        (hp.max(0) as f32 / max_hp as f32).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::data::{AiDifficulty, ElementType, EnemyAiPolicyMode};

    fn test_combatant() -> AiCombatantObservation {
        AiCombatantObservation {
            hp: 20,
            max_hp: 30,
            shield: 3,
            atk: 12,
            def: 8,
            atk_stage: 0,
            def_stage: 0,
            spd_stage: 0,
            acc_stage: 0,
            element: ElementType::Fire,
            attached_auras: [Some(ElementType::Water), None],
            status_ids: vec!["burning_aura".to_string()],
        }
    }

    fn test_observation() -> AiObservation {
        AiObservation {
            round: 2,
            battle_seed: 42,
            difficulty: AiDifficulty::Hard,
            enemy: AiSideObservation {
                active_index: 0,
                alive_count: 2,
                member_count: 3,
                ap: 4,
                active: test_combatant(),
            },
            player: AiSideObservation {
                active_index: 1,
                alive_count: 3,
                member_count: 3,
                ap: 5,
                active: test_combatant(),
            },
            enemy_hand: vec![CardId::GainAp],
            player_hand_count: 2,
            player_hand: None,
            enemy_skill_uses_remaining: [2, 1, 0, 0],
        }
    }

    fn test_candidate(kind: AiActionKind, heuristic_score: f32) -> AiActionFeatures {
        AiActionFeatures {
            kind,
            heuristic_score,
            estimated_value: heuristic_score,
            skill_slot: None,
            skill_id: None,
            card_index: None,
            card_id: None,
            switch_index: None,
            summary: format!("{kind:?}"),
        }
    }

    fn temp_model_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("bevy_mon_proto_{name}_{}.ron", std::process::id()))
    }

    #[test]
    fn model_score_index_rejects_invalid_outputs() {
        assert_eq!(best_model_score_index(&[0.1, 0.3], 2), Some(1));
        assert_eq!(best_model_score_index(&[0.1], 2), None);
        assert_eq!(best_model_score_index(&[f32::NAN], 1), None);
        assert_eq!(best_model_score_index(&[], 0), None);
    }

    #[test]
    fn ai_observation_serializes_stably() {
        let observation = test_observation();
        let first =
            ron::ser::to_string_pretty(&observation, ron::ser::PrettyConfig::default()).unwrap();
        let second =
            ron::ser::to_string_pretty(&observation, ron::ser::PrettyConfig::default()).unwrap();
        assert_eq!(first, second);
        assert!(first.contains("battle_seed: 42"));
    }

    #[test]
    fn collect_modes_enable_sample_capture() {
        let mut config = EnemyAiConfig::default();
        assert!(!should_collect_decision_samples(&config));

        config.policy_mode = EnemyAiPolicyMode::CollectOnly;
        assert!(should_collect_decision_samples(&config));

        config.policy_mode = EnemyAiPolicyMode::Heuristic;
        config.export_decision_samples = true;
        assert!(should_collect_decision_samples(&config));
    }

    #[test]
    fn episode_reward_keeps_terminal_outcome_dominant() {
        let stats = AiEpisodeStats {
            enemy: AiSideEpisodeStats {
                damage_dealt: 200,
                knockouts: 2,
                two_element_reactions: 0,
                advanced_reactions: 3,
            },
            player: AiSideEpisodeStats::default(),
            enemy_team: AiTeamSnapshot {
                current_hp: 40,
                max_hp: 60,
                alive_count: 1,
                member_count: 1,
            },
            player_team: AiTeamSnapshot {
                current_hp: 5,
                max_hp: 60,
                alive_count: 1,
                member_count: 1,
            },
        };

        let defeat_reward = reward_for_episode(AiBattleOutcome::EnemyDefeat, stats);
        let victory_reward = reward_for_episode(AiBattleOutcome::EnemyVictory, stats);

        assert!((-1.25..=-0.75).contains(&defeat_reward));
        assert!((0.75..=1.25).contains(&victory_reward));
    }

    #[test]
    fn model_ranker_scores_candidates_from_ron_model() {
        let path = temp_model_path("linear_ranker");
        fs::write(
            &path,
            r#"(
                heuristic_score_weight: 0.0,
                use_skill_weight: 4.0,
                end_turn_weight: -2.0,
            )"#,
        )
        .unwrap();

        let mut config = EnemyAiConfig {
            policy_mode: EnemyAiPolicyMode::ModelRanker,
            policy_model_path: Some(path.display().to_string()),
            ..EnemyAiConfig::default()
        };
        let mut runtime = AiPolicyRuntime::default();
        let candidates = vec![
            test_candidate(AiActionKind::EndTurn, 100.0),
            test_candidate(AiActionKind::UseSkill, 0.0),
        ];

        let selection =
            score_candidates(&config, &mut runtime, &test_observation(), &candidates).unwrap();
        assert_eq!(selection.index, 1);

        config.policy_model_path = Some("".to_string());
        assert!(
            score_candidates(&config, &mut runtime, &test_observation(), &candidates).is_none()
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn model_ranker_returns_none_for_missing_model() {
        let config = EnemyAiConfig {
            policy_mode: EnemyAiPolicyMode::ModelRanker,
            policy_model_path: Some("missing_ai_ranker_model.ron".to_string()),
            ..EnemyAiConfig::default()
        };
        let mut runtime = AiPolicyRuntime::default();
        let candidates = vec![test_candidate(AiActionKind::UseSkill, 1.0)];

        assert!(
            score_candidates(&config, &mut runtime, &test_observation(), &candidates).is_none()
        );
        assert!(runtime.last_error().is_some());
    }

    #[test]
    fn decision_log_exports_ron_and_jsonl_samples() {
        let dir =
            std::env::temp_dir().join(format!("bevy_mon_proto_ai_export_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let config = EnemyAiConfig {
            policy_mode: EnemyAiPolicyMode::CollectOnly,
            decision_sample_export_dir: dir.display().to_string(),
            ..EnemyAiConfig::default()
        };
        let mut log = AiDecisionLog::default();
        log.record(AiDecisionSample {
            seq: 0,
            round: 2,
            battle_seed: 42,
            difficulty: AiDifficulty::Hard,
            policy_mode: EnemyAiPolicyMode::CollectOnly,
            decision_source: AiDecisionSource::Heuristic,
            observation: test_observation(),
            candidates: vec![test_candidate(AiActionKind::UseSkill, 1.0)],
            heuristic_scores: vec![1.0],
            model_scores: None,
            chosen_index: 0,
            outcome: None,
            reward: None,
            episode_stats: None,
        });
        log.finalize_with_reward(
            AiBattleOutcome::EnemyVictory,
            reward_for_outcome(AiBattleOutcome::EnemyVictory),
        );

        let exported = log.export_if_enabled(&config).unwrap().unwrap();
        assert!(exported.contains(".ron"));
        assert!(exported.contains(".jsonl"));

        let ron_path = dir.join("ai_decisions_seed_42_enemy_victory_n1.ron");
        let jsonl_path = dir.join("ai_decisions_seed_42_enemy_victory_n1.jsonl");
        assert!(ron_path.exists());
        assert!(jsonl_path.exists());
        let jsonl = fs::read_to_string(&jsonl_path).unwrap();
        assert_eq!(jsonl.lines().count(), 1);
        assert!(jsonl.contains("\"outcome\":\"EnemyVictory\""));

        let _ = fs::remove_dir_all(dir);
    }
}
