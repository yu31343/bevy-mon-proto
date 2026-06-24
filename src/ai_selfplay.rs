use std::{env, fs, io::Write, path::PathBuf};

use serde::Serialize;

use crate::{
    battle::ai::{
        AiBattleOutcome, AiCombatantObservation, AiDecisionSample, AiDecisionSource,
        AiEpisodeStats, AiObservation, AiPolicyRuntime, AiSideEpisodeStats, AiSideObservation,
        AiTeamSnapshot, build_action_features, choose_enemy_plan_candidates, reward_for_episode,
        score_candidates,
    },
    data::{
        AiDifficulty, BattleDbs, BattleRules, CardEffect, CardId, ElementType, EnemyAiConfig,
        EnemyAiPolicyMode, LoadedBattleData, MonsterPrototype, SkillEffect, SkillId,
        load_battle_data_from_path,
    },
};

const DEFAULT_BATTLES: usize = 100;
const DEFAULT_MAX_ROUNDS: u32 = 12;
const DEFAULT_OUTPUT: &str = "battle_logs/ai_selfplay/selfplay_samples.jsonl";
const DEFAULT_EVAL_OUTPUT: &str = "battle_logs/ai_eval/eval_report.json";

#[derive(Debug, Clone)]
struct SelfPlayOptions {
    battles: usize,
    max_rounds: u32,
    seed: u64,
    output: PathBuf,
    data_path: String,
    model_path: Option<String>,
    difficulty: AiDifficulty,
}

#[derive(Debug, Clone)]
struct EvalOptions {
    battles: usize,
    max_rounds: u32,
    seed: u64,
    output: PathBuf,
    data_path: String,
    model_path: String,
    difficulty: AiDifficulty,
}

#[derive(Debug, Clone)]
struct BattleRunResult {
    samples: Vec<AiDecisionSample>,
    outcome: AiBattleOutcome,
    rounds: u32,
    reward: f32,
    stats: AiEpisodeStats,
    model_selection_count: usize,
    fallback_count: usize,
}

#[derive(Debug, Clone, Default)]
struct SimEpisodeCounters {
    enemy_damage_dealt: i32,
    player_damage_dealt: i32,
    enemy_knockouts: u32,
    player_knockouts: u32,
}

#[derive(Debug, Clone, Serialize)]
struct AiEvalReport {
    battles: usize,
    seed: u64,
    max_rounds: u32,
    difficulty: AiDifficulty,
    model_path: String,
    heuristic: AiEvalPolicyReport,
    model: AiEvalPolicyReport,
}

#[derive(Debug, Clone, Serialize)]
struct AiEvalPolicyReport {
    policy_mode: EnemyAiPolicyMode,
    battles: usize,
    enemy_wins: usize,
    enemy_losses: usize,
    draws: usize,
    avg_reward: f32,
    avg_rounds: f32,
    samples: usize,
    model_selection_count: usize,
    fallback_count: usize,
}

#[derive(Debug, Clone)]
struct SimCombatant {
    hp: i32,
    max_hp: i32,
    atk: i32,
    def: i32,
    element: ElementType,
    skills: [SkillId; 4],
    skill_count: usize,
}

#[derive(Debug, Clone)]
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.state
    }

    fn range(&mut self, len: usize) -> usize {
        if len <= 1 {
            0
        } else {
            (self.next_u64() as usize) % len
        }
    }
}

pub(crate) fn maybe_run_from_args() -> Option<i32> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let is_selfplay = args.iter().any(|arg| arg == "--ai-selfplay");
    let is_eval = args.iter().any(|arg| arg == "--ai-eval");
    if !is_selfplay && !is_eval {
        return None;
    }
    let result = if is_eval {
        run_eval_from_args(&args)
    } else {
        run_from_args(&args)
    };
    match result {
        Ok(summary) => {
            println!("{summary}");
            Some(0)
        }
        Err(err) => {
            eprintln!("{err}");
            Some(1)
        }
    }
}

fn run_from_args(args: &[String]) -> Result<String, String> {
    let options = parse_options(args)?;
    run_selfplay(options)
}

fn run_eval_from_args(args: &[String]) -> Result<String, String> {
    let options = parse_eval_options(args)?;
    run_eval(options)
}

fn parse_options(args: &[String]) -> Result<SelfPlayOptions, String> {
    let mut options = SelfPlayOptions {
        battles: DEFAULT_BATTLES,
        max_rounds: DEFAULT_MAX_ROUNDS,
        seed: 1,
        output: PathBuf::from(DEFAULT_OUTPUT),
        data_path: "assets/data/battle_data.ron".to_string(),
        model_path: None,
        difficulty: AiDifficulty::Expert,
    };

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--ai-selfplay" => {
                if let Some(value) = args.get(index + 1)
                    && !value.starts_with("--")
                {
                    options.battles = parse_positive_usize(value, "--ai-selfplay")?;
                    index += 1;
                }
            }
            "--ai-selfplay-output" => {
                index += 1;
                options.output =
                    PathBuf::from(required_value(args, index, "--ai-selfplay-output")?);
            }
            "--ai-selfplay-seed" => {
                index += 1;
                options.seed = required_value(args, index, "--ai-selfplay-seed")?
                    .parse::<u64>()
                    .map_err(|err| format!("--ai-selfplay-seed 必须是 u64：{err}"))?;
            }
            "--ai-selfplay-rounds" => {
                index += 1;
                options.max_rounds = required_value(args, index, "--ai-selfplay-rounds")?
                    .parse::<u32>()
                    .map_err(|err| format!("--ai-selfplay-rounds 必须是 u32：{err}"))?
                    .max(1);
            }
            "--ai-selfplay-data" => {
                index += 1;
                options.data_path = required_value(args, index, "--ai-selfplay-data")?.to_string();
            }
            "--ai-selfplay-model" => {
                index += 1;
                options.model_path =
                    Some(required_value(args, index, "--ai-selfplay-model")?.to_string());
            }
            "--ai-selfplay-difficulty" => {
                index += 1;
                options.difficulty =
                    parse_difficulty(required_value(args, index, "--ai-selfplay-difficulty")?)?;
            }
            other => return Err(format!("未知 AI selfplay 参数：{other}")),
        }
        index += 1;
    }

    if options.battles == 0 {
        return Err("--ai-selfplay 必须 >= 1".to_string());
    }
    Ok(options)
}

fn parse_eval_options(args: &[String]) -> Result<EvalOptions, String> {
    let mut options = EvalOptions {
        battles: DEFAULT_BATTLES,
        max_rounds: DEFAULT_MAX_ROUNDS,
        seed: 1,
        output: PathBuf::from(DEFAULT_EVAL_OUTPUT),
        data_path: "assets/data/battle_data.ron".to_string(),
        model_path: "assets/data/ai_ranker_default.ron".to_string(),
        difficulty: AiDifficulty::Expert,
    };

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--ai-eval" => {
                if let Some(value) = args.get(index + 1)
                    && !value.starts_with("--")
                {
                    options.battles = parse_positive_usize(value, "--ai-eval")?;
                    index += 1;
                }
            }
            "--ai-eval-model" => {
                index += 1;
                options.model_path = required_value(args, index, "--ai-eval-model")?.to_string();
            }
            "--ai-eval-output" => {
                index += 1;
                options.output = PathBuf::from(required_value(args, index, "--ai-eval-output")?);
            }
            "--ai-eval-seed" => {
                index += 1;
                options.seed = required_value(args, index, "--ai-eval-seed")?
                    .parse::<u64>()
                    .map_err(|err| format!("--ai-eval-seed 必须是 u64：{err}"))?;
            }
            "--ai-eval-rounds" => {
                index += 1;
                options.max_rounds = required_value(args, index, "--ai-eval-rounds")?
                    .parse::<u32>()
                    .map_err(|err| format!("--ai-eval-rounds 必须是 u32：{err}"))?
                    .max(1);
            }
            "--ai-eval-data" => {
                index += 1;
                options.data_path = required_value(args, index, "--ai-eval-data")?.to_string();
            }
            "--ai-eval-difficulty" => {
                index += 1;
                options.difficulty =
                    parse_difficulty(required_value(args, index, "--ai-eval-difficulty")?)?;
            }
            other => return Err(format!("未知 AI eval 参数：{other}")),
        }
        index += 1;
    }

    if options.battles == 0 {
        return Err("--ai-eval 必须 >= 1".to_string());
    }
    if options.model_path.trim().is_empty() {
        return Err("--ai-eval-model 不能是空字符串".to_string());
    }
    Ok(options)
}

fn required_value<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .filter(|value| !value.starts_with("--"))
        .ok_or_else(|| format!("{flag} 缺少参数值"))
}

fn parse_positive_usize(value: &str, flag: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|err| format!("{flag} 必须是 usize：{err}"))
        .and_then(|value| {
            if value == 0 {
                Err(format!("{flag} 必须 >= 1"))
            } else {
                Ok(value)
            }
        })
}

fn parse_difficulty(value: &str) -> Result<AiDifficulty, String> {
    match value.to_ascii_lowercase().as_str() {
        "easy" => Ok(AiDifficulty::Easy),
        "normal" => Ok(AiDifficulty::Normal),
        "hard" => Ok(AiDifficulty::Hard),
        "expert" => Ok(AiDifficulty::Expert),
        _ => Err(format!("未知 AI 难度：{value}")),
    }
}

fn run_selfplay(options: SelfPlayOptions) -> Result<String, String> {
    let loaded = load_battle_data_from_path(&options.data_path)?;
    if loaded.monster_pool.monsters.len() < 2 {
        return Err("至少需要 2 个 monster 才能运行 AI selfplay".to_string());
    }

    let mut config = base_config_for(&loaded.ai_presets, options.difficulty);
    if let Some(model_path) = options.model_path.clone() {
        config.policy_mode = EnemyAiPolicyMode::ModelRanker;
        config.policy_model_path = Some(model_path);
    } else {
        config.policy_mode = EnemyAiPolicyMode::Heuristic;
        config.policy_model_path = None;
    }

    if let Some(parent) = options.output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| format!("创建输出目录失败：{err}"))?;
    }
    let mut output = fs::File::create(&options.output)
        .map_err(|err| format!("创建 {:?} 失败：{err}", options.output))?;

    let mut rng = Lcg::new(options.seed);
    let mut policy_runtime = AiPolicyRuntime::default();
    let mut sample_count = 0usize;
    let mut enemy_wins = 0usize;
    let mut enemy_losses = 0usize;
    let mut draws = 0usize;
    let mut reward_sum = 0.0f32;

    for battle_index in 0..options.battles {
        let result = run_one_battle(
            battle_index as u64,
            options.max_rounds,
            &loaded.dbs,
            &loaded.rules,
            &loaded.monster_pool.monsters,
            &loaded.card_deck.0,
            &config,
            &mut policy_runtime,
            &mut rng,
        );
        match result.outcome {
            AiBattleOutcome::EnemyVictory => enemy_wins += 1,
            AiBattleOutcome::EnemyDefeat => enemy_losses += 1,
            AiBattleOutcome::Draw => draws += 1,
        }
        reward_sum += result.reward;
        for mut sample in result.samples {
            sample.outcome = Some(result.outcome);
            sample.reward = Some(result.reward);
            sample.episode_stats = Some(result.stats);
            serde_json::to_writer(&mut output, &sample)
                .map_err(|err| format!("序列化 selfplay 样本失败：{err}"))?;
            output
                .write_all(b"\n")
                .map_err(|err| format!("写入 {:?} 失败：{err}", options.output))?;
            sample_count += 1;
        }
    }

    Ok(format!(
        "AI selfplay 完成：battles={} samples={} enemy_wins={} enemy_losses={} draws={} avg_reward={:.3} output={}",
        options.battles,
        sample_count,
        enemy_wins,
        enemy_losses,
        draws,
        reward_sum / options.battles as f32,
        options.output.display()
    ))
}

fn run_eval(options: EvalOptions) -> Result<String, String> {
    let loaded = load_battle_data_from_path(&options.data_path)?;
    if loaded.monster_pool.monsters.len() < 2 {
        return Err("至少需要 2 个 monster 才能运行 AI eval".to_string());
    }

    let mut heuristic_config = base_config_for(&loaded.ai_presets, options.difficulty);
    heuristic_config.policy_mode = EnemyAiPolicyMode::Heuristic;
    heuristic_config.policy_model_path = None;

    let mut model_config = base_config_for(&loaded.ai_presets, options.difficulty);
    model_config.policy_mode = EnemyAiPolicyMode::ModelRanker;
    model_config.policy_model_path = Some(options.model_path.clone());

    let heuristic = evaluate_policy(
        &loaded,
        &heuristic_config,
        options.battles,
        options.max_rounds,
        options.seed,
    );
    let model = evaluate_policy(
        &loaded,
        &model_config,
        options.battles,
        options.max_rounds,
        options.seed,
    );
    let report = AiEvalReport {
        battles: options.battles,
        seed: options.seed,
        max_rounds: options.max_rounds,
        difficulty: options.difficulty,
        model_path: options.model_path,
        heuristic,
        model,
    };

    if let Some(parent) = options.output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| format!("创建评估输出目录失败：{err}"))?;
    }
    let text = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("序列化 AI eval 报告失败：{err}"))?;
    fs::write(&options.output, text)
        .map_err(|err| format!("写入 {:?} 失败：{err}", options.output))?;

    Ok(format!(
        "AI eval 完成：battles={} heuristic_avg_reward={:.3} model_avg_reward={:.3} model_wins={} model_losses={} output={}",
        report.battles,
        report.heuristic.avg_reward,
        report.model.avg_reward,
        report.model.enemy_wins,
        report.model.enemy_losses,
        options.output.display()
    ))
}

fn base_config_for(
    presets: &crate::data::EnemyAiPresets,
    difficulty: AiDifficulty,
) -> EnemyAiConfig {
    presets
        .config_for(difficulty)
        .unwrap_or_else(|| presets.default_config())
}

fn evaluate_policy(
    loaded: &LoadedBattleData,
    config: &EnemyAiConfig,
    battles: usize,
    max_rounds: u32,
    seed: u64,
) -> AiEvalPolicyReport {
    let mut rng = Lcg::new(seed);
    let mut policy_runtime = AiPolicyRuntime::default();
    let mut enemy_wins = 0usize;
    let mut enemy_losses = 0usize;
    let mut draws = 0usize;
    let mut reward_sum = 0.0f32;
    let mut rounds_sum = 0u32;
    let mut samples = 0usize;
    let mut model_selection_count = 0usize;
    let mut fallback_count = 0usize;

    for battle_index in 0..battles {
        let result = run_one_battle(
            battle_index as u64,
            max_rounds,
            &loaded.dbs,
            &loaded.rules,
            &loaded.monster_pool.monsters,
            &loaded.card_deck.0,
            config,
            &mut policy_runtime,
            &mut rng,
        );
        match result.outcome {
            AiBattleOutcome::EnemyVictory => enemy_wins += 1,
            AiBattleOutcome::EnemyDefeat => enemy_losses += 1,
            AiBattleOutcome::Draw => draws += 1,
        }
        reward_sum += result.reward;
        rounds_sum += result.rounds;
        samples += result.samples.len();
        model_selection_count += result.model_selection_count;
        fallback_count += result.fallback_count;
    }

    AiEvalPolicyReport {
        policy_mode: config.policy_mode,
        battles,
        enemy_wins,
        enemy_losses,
        draws,
        avg_reward: reward_sum / battles as f32,
        avg_rounds: rounds_sum as f32 / battles as f32,
        samples,
        model_selection_count,
        fallback_count,
    }
}

fn run_one_battle(
    battle_index: u64,
    max_rounds: u32,
    dbs: &BattleDbs,
    rules: &BattleRules,
    monsters: &[MonsterPrototype],
    deck: &[CardId],
    config: &EnemyAiConfig,
    policy_runtime: &mut AiPolicyRuntime,
    rng: &mut Lcg,
) -> BattleRunResult {
    let enemy_index = rng.range(monsters.len());
    let mut player_index = rng.range(monsters.len());
    if player_index == enemy_index {
        player_index = (player_index + 1) % monsters.len();
    }

    let mut enemy = sim_combatant(&monsters[enemy_index]);
    let mut player = sim_combatant(&monsters[player_index]);
    let mut enemy_ap = 0;
    let mut player_ap = 0;
    let battle_seed = rng.next_u64() ^ battle_index;
    let mut samples = Vec::new();
    let mut counters = SimEpisodeCounters::default();
    let mut model_selection_count = 0usize;
    let mut fallback_count = 0usize;

    for round in 1..=max_rounds {
        enemy_ap = (enemy_ap + rules.ap_per_round).min(rules.max_ap);
        player_ap = (player_ap + rules.ap_per_round).min(rules.max_ap);
        let enemy_hand = sample_hand(deck, rng, rules.initial_cards.max(2));
        let player_hand = sample_hand(deck, rng, rules.initial_cards.max(2));
        let skill_uses = skill_uses_for(&enemy, dbs, rules);
        let observation = build_observation(
            round,
            battle_seed,
            config.difficulty,
            &enemy,
            enemy_ap,
            &player,
            player_ap,
            &enemy_hand,
            player_hand.len(),
            skill_uses,
        );
        let ctx = build_context(&enemy, &player, skill_uses, rules);
        let plans = choose_enemy_plan_candidates(
            &enemy_hand,
            enemy_ap,
            &enemy.skills,
            enemy.skill_count,
            dbs,
            &ctx,
            None,
            &config.weights,
            None,
            config.search_depth,
            config.top_candidates,
        );
        if plans.is_empty() {
            continue;
        }
        let action_features = build_action_features(&plans, &enemy_hand);
        let model_selection =
            score_candidates(config, policy_runtime, &observation, &action_features);
        let (chosen_index, decision_source, model_scores) = if let Some(selection) = model_selection
        {
            model_selection_count += 1;
            (
                selection.index,
                AiDecisionSource::ModelRanker,
                Some(selection.scores),
            )
        } else {
            if config.policy_mode == EnemyAiPolicyMode::ModelRanker {
                fallback_count += 1;
            }
            (0, AiDecisionSource::Heuristic, None)
        };
        samples.push(AiDecisionSample {
            seq: samples.len() as u64 + 1,
            round,
            battle_seed,
            difficulty: config.difficulty,
            policy_mode: config.policy_mode,
            decision_source,
            observation,
            candidates: action_features,
            heuristic_scores: plans.iter().map(|plan| plan.score).collect(),
            model_scores,
            chosen_index,
            outcome: None,
            reward: None,
            episode_stats: None,
        });

        let player_hp_before = player.hp;
        let (next_enemy_ap, enemy_damage) = apply_plan(
            &plans[chosen_index],
            &enemy_hand,
            &mut enemy,
            &mut player,
            enemy_ap,
            dbs,
            rules,
        );
        enemy_ap = next_enemy_ap;
        counters.enemy_damage_dealt += enemy_damage;
        if player.hp <= 0 {
            if player_hp_before > 0 {
                counters.enemy_knockouts += 1;
            }
            return battle_result(
                samples,
                AiBattleOutcome::EnemyVictory,
                round,
                &enemy,
                &player,
                counters,
                model_selection_count,
                fallback_count,
            );
        }

        let player_skill = best_playable_attack(&player, player_ap, dbs);
        if let Some((slot, skill_id)) = player_skill {
            if let Some(skill) = dbs.skills.get(&skill_id) {
                let enemy_hp_before = enemy.hp;
                player_ap = player_ap.saturating_sub(skill.cost_ap);
                let player_damage =
                    apply_skill_effect(slot, skill_id, &mut player, &mut enemy, dbs);
                counters.player_damage_dealt += player_damage;
                if enemy_hp_before > 0 && enemy.hp <= 0 {
                    counters.player_knockouts += 1;
                }
            }
        }
        if enemy.hp <= 0 {
            return battle_result(
                samples,
                AiBattleOutcome::EnemyDefeat,
                round,
                &enemy,
                &player,
                counters,
                model_selection_count,
                fallback_count,
            );
        }
    }

    let outcome = if enemy.hp > player.hp {
        AiBattleOutcome::EnemyVictory
    } else if enemy.hp < player.hp {
        AiBattleOutcome::EnemyDefeat
    } else {
        AiBattleOutcome::Draw
    };
    battle_result(
        samples,
        outcome,
        max_rounds,
        &enemy,
        &player,
        counters,
        model_selection_count,
        fallback_count,
    )
}

fn battle_result(
    samples: Vec<AiDecisionSample>,
    outcome: AiBattleOutcome,
    rounds: u32,
    enemy: &SimCombatant,
    player: &SimCombatant,
    counters: SimEpisodeCounters,
    model_selection_count: usize,
    fallback_count: usize,
) -> BattleRunResult {
    let stats = AiEpisodeStats {
        enemy: AiSideEpisodeStats {
            damage_dealt: counters.enemy_damage_dealt,
            knockouts: counters.enemy_knockouts,
            two_element_reactions: 0,
            advanced_reactions: 0,
        },
        player: AiSideEpisodeStats {
            damage_dealt: counters.player_damage_dealt,
            knockouts: counters.player_knockouts,
            two_element_reactions: 0,
            advanced_reactions: 0,
        },
        enemy_team: team_snapshot(enemy),
        player_team: team_snapshot(player),
    };
    let reward = reward_for_episode(outcome, stats);
    BattleRunResult {
        samples,
        outcome,
        rounds,
        reward,
        stats,
        model_selection_count,
        fallback_count,
    }
}

fn team_snapshot(combatant: &SimCombatant) -> AiTeamSnapshot {
    AiTeamSnapshot {
        current_hp: combatant.hp.max(0),
        max_hp: combatant.max_hp,
        alive_count: usize::from(combatant.hp > 0),
        member_count: 1,
    }
}

fn sim_combatant(monster: &MonsterPrototype) -> SimCombatant {
    let (skills, skill_count) = normalize_skill_slots(&monster.skills);
    SimCombatant {
        hp: monster.stats.hp,
        max_hp: monster.stats.hp,
        atk: monster.stats.atk,
        def: monster.stats.def,
        element: monster.element,
        skills,
        skill_count,
    }
}

fn normalize_skill_slots(skills: &[SkillId]) -> ([SkillId; 4], usize) {
    let count = skills.len().clamp(1, 4);
    let fallback = skills[0];
    let mut slots = [fallback; 4];
    for (index, skill) in skills.iter().take(4).enumerate() {
        slots[index] = *skill;
    }
    (slots, count)
}

fn sample_hand(deck: &[CardId], rng: &mut Lcg, count: usize) -> Vec<CardId> {
    if deck.is_empty() {
        return Vec::new();
    }
    (0..count.min(deck.len()))
        .map(|_| deck[rng.range(deck.len())])
        .collect()
}

fn skill_uses_for(combatant: &SimCombatant, dbs: &BattleDbs, rules: &BattleRules) -> [u8; 4] {
    let mut uses = [0u8; 4];
    for (slot, value) in uses.iter_mut().enumerate().take(combatant.skill_count) {
        *value = dbs.skill_uses_per_turn(combatant.skills[slot], rules);
    }
    uses
}

fn build_observation(
    round: u32,
    battle_seed: u64,
    difficulty: AiDifficulty,
    enemy: &SimCombatant,
    enemy_ap: i32,
    player: &SimCombatant,
    player_ap: i32,
    enemy_hand: &[CardId],
    player_hand_count: usize,
    skill_uses: [u8; 4],
) -> AiObservation {
    AiObservation {
        round,
        battle_seed,
        difficulty,
        enemy: AiSideObservation {
            active_index: 0,
            alive_count: 1,
            member_count: 1,
            ap: enemy_ap,
            active: combatant_observation(enemy),
        },
        player: AiSideObservation {
            active_index: 0,
            alive_count: 1,
            member_count: 1,
            ap: player_ap,
            active: combatant_observation(player),
        },
        enemy_hand: enemy_hand.to_vec(),
        player_hand_count,
        player_hand: None,
        enemy_skill_uses_remaining: skill_uses,
    }
}

fn combatant_observation(combatant: &SimCombatant) -> AiCombatantObservation {
    AiCombatantObservation {
        hp: combatant.hp,
        max_hp: combatant.max_hp,
        shield: 0,
        atk: combatant.atk,
        def: combatant.def,
        atk_stage: 0,
        def_stage: 0,
        spd_stage: 0,
        acc_stage: 0,
        element: combatant.element,
        attached_auras: [None, None],
        status_ids: Vec::new(),
    }
}

fn build_context(
    enemy: &SimCombatant,
    player: &SimCombatant,
    skill_uses: [u8; 4],
    rules: &BattleRules,
) -> crate::battle::ai::EnemyAiContext {
    crate::battle::ai::EnemyAiContext {
        enemy_hp: enemy.hp,
        enemy_max_hp: enemy.max_hp,
        enemy_shield: 0,
        max_shield_hp_ratio: rules.max_shield_hp_ratio,
        enemy_atk: enemy.atk,
        enemy_def: enemy.def,
        enemy_atk_stage: 0,
        enemy_def_stage: 0,
        enemy_spd_stage: 0,
        enemy_acc_stage: 0,
        enemy_element: enemy.element,
        enemy_attached_auras: [None, None],
        enemy_status_ids: Vec::new(),
        enemy_has_aura: false,
        enemy_has_cleansable_debuff: false,
        player_atk_stage: 0,
        player_def_stage: 0,
        player_spd_stage: 0,
        player_acc_stage: 0,
        player_def: player.def,
        player_hp: player.hp,
        player_shield: 0,
        target_element: player.element,
        target_attached_auras: [None, None],
        target_status_ids: Vec::new(),
        skill_uses_remaining: skill_uses,
    }
}

fn apply_plan(
    plan: &crate::battle::ai::EnemyAiPlan,
    hand: &[CardId],
    acting: &mut SimCombatant,
    target: &mut SimCombatant,
    current_ap: i32,
    dbs: &BattleDbs,
    rules: &BattleRules,
) -> (i32, i32) {
    match &plan.action {
        crate::battle::ai::EnemyPlannedAction::UseSkill(skill)
        | crate::battle::ai::EnemyPlannedAction::UseCardForSkill { skill, .. } => {
            if let Some(def) = dbs.skills.get(&skill.skill_id) {
                let damage = apply_skill_effect(skill.slot, skill.skill_id, acting, target, dbs);
                (current_ap.saturating_sub(def.cost_ap), damage)
            } else {
                (current_ap, 0)
            }
        }
        crate::battle::ai::EnemyPlannedAction::ImmediateCard(card) => {
            let Some(card_id) = hand.get(card.index).copied() else {
                return (current_ap, 0);
            };
            (
                apply_card_effect(card_id, acting, current_ap, dbs, rules),
                0,
            )
        }
        crate::battle::ai::EnemyPlannedAction::Discard(_) => {
            (current_ap + rules.discard_ap_gain, 0)
        }
        crate::battle::ai::EnemyPlannedAction::Switch(_)
        | crate::battle::ai::EnemyPlannedAction::EndTurn => (current_ap, 0),
    }
}

fn apply_card_effect(
    card_id: CardId,
    acting: &mut SimCombatant,
    current_ap: i32,
    dbs: &BattleDbs,
    rules: &BattleRules,
) -> i32 {
    let Some(card) = dbs.cards.get(&card_id) else {
        return current_ap;
    };
    let mut ap = current_ap.saturating_sub(card.cost_ap);
    match &card.effect {
        CardEffect::GainAp { amount }
        | CardEffect::CleanseOrGainAp {
            fallback_ap: amount,
            ..
        }
        | CardEffect::ShieldAbsorbGainAp { amount } => {
            ap = (ap + *amount).min(rules.max_ap);
        }
        CardEffect::GainShield { amount }
        | CardEffect::GainShieldDrawIfSwitchedThisTurn { shield: amount, .. } => {
            acting.hp = (acting.hp + *amount / 3).min(acting.max_hp);
        }
        _ => {}
    }
    ap
}

fn best_playable_attack(
    combatant: &SimCombatant,
    current_ap: i32,
    dbs: &BattleDbs,
) -> Option<(usize, SkillId)> {
    combatant
        .skills
        .iter()
        .copied()
        .enumerate()
        .take(combatant.skill_count)
        .filter_map(|(slot, skill_id)| {
            let skill = dbs.skills.get(&skill_id)?;
            if skill.cost_ap <= current_ap && skill_contains_attack(&skill.effect) {
                Some((slot, skill_id, skill.cost_ap))
            } else {
                None
            }
        })
        .max_by_key(|(_, _, cost)| *cost)
        .map(|(slot, skill_id, _)| (slot, skill_id))
}

fn apply_skill_effect(
    _slot: usize,
    skill_id: SkillId,
    acting: &mut SimCombatant,
    target: &mut SimCombatant,
    dbs: &BattleDbs,
) -> i32 {
    let Some(skill) = dbs.skills.get(&skill_id) else {
        return 0;
    };
    apply_effect_value(&skill.effect, acting, target)
}

fn apply_effect_value(
    effect: &SkillEffect,
    acting: &mut SimCombatant,
    target: &mut SimCombatant,
) -> i32 {
    match effect {
        SkillEffect::Attack { power, .. } => {
            let damage = (*power + acting.atk / 2 - target.def / 4).max(1);
            let before = target.hp;
            target.hp = target.hp.saturating_sub(damage);
            before.saturating_sub(target.hp)
        }
        SkillEffect::DealFixedDamage { amount, .. } => {
            let before = target.hp;
            target.hp = target.hp.saturating_sub((*amount).max(1));
            before.saturating_sub(target.hp)
        }
        SkillEffect::Heal { amount } => {
            acting.hp = (acting.hp + *amount).min(acting.max_hp);
            0
        }
        SkillEffect::Shield { amount } => {
            acting.hp = (acting.hp + amount / 3).min(acting.max_hp);
            0
        }
        SkillEffect::Sequence { effects } => {
            let mut damage = 0;
            for nested in effects {
                damage += apply_effect_value(nested, acting, target);
            }
            damage
        }
        SkillEffect::Conditional { branches } => {
            let mut damage = 0;
            for branch in branches {
                damage += apply_effect_value(&branch.effect, acting, target);
            }
            damage
        }
        _ => 0,
    }
}

fn skill_contains_attack(effect: &SkillEffect) -> bool {
    match effect {
        SkillEffect::Attack { .. } => true,
        SkillEffect::Sequence { effects } => effects.iter().any(skill_contains_attack),
        SkillEffect::Conditional { branches } => branches
            .iter()
            .any(|branch| skill_contains_attack(&branch.effect)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_selfplay_options_accepts_core_flags() {
        let args = vec![
            "--ai-selfplay".to_string(),
            "12".to_string(),
            "--ai-selfplay-output".to_string(),
            "out.jsonl".to_string(),
            "--ai-selfplay-seed".to_string(),
            "99".to_string(),
            "--ai-selfplay-rounds".to_string(),
            "8".to_string(),
            "--ai-selfplay-difficulty".to_string(),
            "hard".to_string(),
            "--ai-selfplay-model".to_string(),
            "assets/data/ai_ranker_default.ron".to_string(),
        ];

        let options = parse_options(&args).expect("selfplay args should parse");

        assert_eq!(options.battles, 12);
        assert_eq!(options.output, PathBuf::from("out.jsonl"));
        assert_eq!(options.seed, 99);
        assert_eq!(options.max_rounds, 8);
        assert_eq!(options.difficulty, AiDifficulty::Hard);
        assert_eq!(
            options.model_path.as_deref(),
            Some("assets/data/ai_ranker_default.ron")
        );
    }

    #[test]
    fn parse_selfplay_options_rejects_zero_battles() {
        let args = vec!["--ai-selfplay".to_string(), "0".to_string()];
        assert!(parse_options(&args).is_err());
    }

    #[test]
    fn parse_eval_options_accepts_core_flags() {
        let args = vec![
            "--ai-eval".to_string(),
            "16".to_string(),
            "--ai-eval-model".to_string(),
            "assets/data/ai_ranker_default.ron".to_string(),
            "--ai-eval-output".to_string(),
            "eval.json".to_string(),
            "--ai-eval-seed".to_string(),
            "12".to_string(),
            "--ai-eval-rounds".to_string(),
            "6".to_string(),
            "--ai-eval-difficulty".to_string(),
            "normal".to_string(),
        ];

        let options = parse_eval_options(&args).expect("eval args should parse");

        assert_eq!(options.battles, 16);
        assert_eq!(
            options.model_path.as_str(),
            "assets/data/ai_ranker_default.ron"
        );
        assert_eq!(options.output, PathBuf::from("eval.json"));
        assert_eq!(options.seed, 12);
        assert_eq!(options.max_rounds, 6);
        assert_eq!(options.difficulty, AiDifficulty::Normal);
    }

    #[test]
    fn run_eval_writes_report() {
        let output = std::env::temp_dir().join(format!(
            "bevy_mon_proto_ai_eval_{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&output);
        let summary = run_eval(EvalOptions {
            battles: 2,
            max_rounds: 3,
            seed: 3,
            output: output.clone(),
            data_path: "assets/data/battle_data.ron".to_string(),
            model_path: "assets/data/ai_ranker_default.ron".to_string(),
            difficulty: AiDifficulty::Easy,
        })
        .expect("eval should run");

        assert!(summary.contains("AI eval 完成"));
        let report = fs::read_to_string(&output).expect("eval report should be written");
        assert!(report.contains("\"heuristic\""));
        assert!(report.contains("\"model\""));
        assert!(report.contains("\"avg_reward\""));

        let _ = fs::remove_file(output);
    }
}
