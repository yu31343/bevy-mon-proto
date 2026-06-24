#!/usr/bin/env python3
"""Train the lightweight enemy AI ranker from exported JSONL samples.

The game writes one AiDecisionSample per JSONL line. This script trains the
same linear model shape consumed by ModelRanker and writes a RON model file.
"""

from __future__ import annotations

import argparse
import json
import math
import pathlib
import random
import sys
from typing import Iterable


FEATURES = [
    "bias",
    "heuristic_score_weight",
    "estimated_value_weight",
    "use_card_for_skill_weight",
    "use_skill_weight",
    "switch_weight",
    "immediate_card_weight",
    "discard_weight",
    "end_turn_weight",
    "low_enemy_hp_weight",
    "low_player_hp_weight",
]

ACTION_FEATURE = {
    "UseCardForSkill": "use_card_for_skill_weight",
    "UseSkill": "use_skill_weight",
    "Switch": "switch_weight",
    "ImmediateCard": "immediate_card_weight",
    "Discard": "discard_weight",
    "EndTurn": "end_turn_weight",
}


def iter_jsonl_paths(path: pathlib.Path) -> Iterable[pathlib.Path]:
    if path.is_file():
        yield path
        return
    yield from sorted(path.rglob("*.jsonl"))


def load_samples(path: pathlib.Path, positive_outcomes_only: bool) -> list[dict]:
    samples: list[dict] = []
    for jsonl_path in iter_jsonl_paths(path):
        with jsonl_path.open("r", encoding="utf-8") as file:
            for line_number, line in enumerate(file, start=1):
                line = line.strip()
                if not line:
                    continue
                try:
                    sample = json.loads(line)
                except json.JSONDecodeError as exc:
                    raise SystemExit(
                        f"{jsonl_path}:{line_number}: invalid JSONL sample: {exc}"
                    ) from exc
                if positive_outcomes_only and sample.get("reward", 0.0) <= 0.0:
                    continue
                if valid_sample(sample):
                    samples.append(sample)
    return samples


def valid_sample(sample: dict) -> bool:
    candidates = sample.get("candidates")
    chosen_index = sample.get("chosen_index")
    return (
        isinstance(candidates, list)
        and isinstance(chosen_index, int)
        and 0 <= chosen_index < len(candidates)
    )


def hp_pressure(side: dict) -> float:
    active = side.get("active", {})
    hp = max(float(active.get("hp", 0.0)), 0.0)
    max_hp = float(active.get("max_hp", 0.0))
    if max_hp <= 0.0:
        return 1.0
    return 1.0 - min(hp / max_hp, 1.0)


def candidate_features(sample: dict, candidate: dict) -> list[float]:
    observation = sample.get("observation", {})
    vector = dict.fromkeys(FEATURES, 0.0)
    vector["bias"] = 1.0
    vector["heuristic_score_weight"] = float(candidate.get("heuristic_score", 0.0))
    vector["estimated_value_weight"] = float(candidate.get("estimated_value", 0.0))
    action_field = ACTION_FEATURE.get(candidate.get("kind"))
    if action_field is not None:
        vector[action_field] = 1.0
    vector["low_enemy_hp_weight"] = hp_pressure(observation.get("enemy", {}))
    vector["low_player_hp_weight"] = hp_pressure(observation.get("player", {}))
    return [vector[name] for name in FEATURES]


def dot(weights: list[float], features: list[float]) -> float:
    return sum(weight * feature for weight, feature in zip(weights, features))


def softmax(scores: list[float]) -> list[float]:
    high = max(scores)
    exps = [math.exp(score - high) for score in scores]
    total = sum(exps)
    if total <= 0.0 or not math.isfinite(total):
        return [1.0 / len(scores)] * len(scores)
    return [value / total for value in exps]


def sample_targets(sample: dict, reward_weighting: bool) -> tuple[list[float], float] | None:
    candidates = sample["candidates"]
    chosen_index = sample["chosen_index"]
    if not reward_weighting:
        return (
            [1.0 if index == chosen_index else 0.0 for index in range(len(candidates))],
            1.0,
        )

    reward = float(sample.get("reward", 0.0) or 0.0)
    weight = abs(reward)
    if weight <= 0.0:
        return None

    if reward > 0.0 or len(candidates) == 1:
        targets = [1.0 if index == chosen_index else 0.0 for index in range(len(candidates))]
    else:
        alternative_count = len(candidates) - 1
        targets = [
            0.0 if index == chosen_index else 1.0 / alternative_count
            for index in range(len(candidates))
        ]
    return targets, weight


def train(
    samples: list[dict],
    epochs: int,
    learning_rate: float,
    l2: float,
    seed: int,
    reward_weighting: bool,
) -> list[float]:
    weights = [0.0] * len(FEATURES)
    weights[FEATURES.index("heuristic_score_weight")] = 1.0
    rng = random.Random(seed)

    for _epoch in range(epochs):
        rng.shuffle(samples)
        for sample in samples:
            target_info = sample_targets(sample, reward_weighting)
            if target_info is None:
                continue
            targets, sample_weight = target_info
            candidate_vectors = [
                candidate_features(sample, candidate) for candidate in sample["candidates"]
            ]
            scores = [dot(weights, vector) for vector in candidate_vectors]
            probabilities = softmax(scores)

            for index, vector in enumerate(candidate_vectors):
                error = (probabilities[index] - targets[index]) * sample_weight
                for feature_index, value in enumerate(vector):
                    weights[feature_index] -= learning_rate * error * value

            if l2 > 0.0:
                for index, weight in enumerate(weights):
                    if FEATURES[index] != "bias":
                        weights[index] = weight * (1.0 - learning_rate * l2)

    return weights


def filter_by_min_abs_reward(samples: list[dict], min_abs_reward: float) -> list[dict]:
    if min_abs_reward <= 0.0:
        return samples
    return [
        sample
        for sample in samples
        if abs(float(sample.get("reward", 0.0) or 0.0)) >= min_abs_reward
    ]


def reward_summary(samples: list[dict]) -> tuple[float, int, int, int]:
    if not samples:
        return 0.0, 0, 0, 0
    rewards = [float(sample.get("reward", 0.0) or 0.0) for sample in samples]
    positive = sum(1 for reward in rewards if reward > 0.0)
    negative = sum(1 for reward in rewards if reward < 0.0)
    neutral = len(rewards) - positive - negative
    return sum(rewards) / len(rewards), positive, negative, neutral


def write_ron_model(path: pathlib.Path, weights: list[float]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = ["("]
    for name, weight in zip(FEATURES, weights):
        lines.append(f"    {name}: {weight:.8f},")
    lines.append(")")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Train a lightweight RON enemy AI ranker from exported JSONL samples."
    )
    parser.add_argument(
        "input",
        nargs="?",
        default="battle_logs/ai_decisions",
        help="JSONL file or directory containing exported AI decision samples.",
    )
    parser.add_argument(
        "-o",
        "--output",
        default="assets/data/ai_ranker_trained.ron",
        help="Output RON model path used by policy_model_path.",
    )
    parser.add_argument("--epochs", type=int, default=50)
    parser.add_argument("--learning-rate", type=float, default=0.0005)
    parser.add_argument("--l2", type=float, default=0.0)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument(
        "--reward-weighting",
        choices=("on", "off"),
        default="on",
        help="Use final reward to weight or reverse ranking updates.",
    )
    parser.add_argument(
        "--min-abs-reward",
        type=float,
        default=0.0,
        help="Drop samples whose absolute reward is below this value.",
    )
    parser.add_argument(
        "--positive-outcomes-only",
        action="store_true",
        help="Train only from samples whose final reward is positive.",
    )
    args = parser.parse_args()

    input_path = pathlib.Path(args.input)
    samples = load_samples(input_path, args.positive_outcomes_only)
    samples = filter_by_min_abs_reward(samples, max(args.min_abs_reward, 0.0))
    if not samples:
        print(f"no usable samples found under {input_path}", file=sys.stderr)
        return 1

    avg_reward, positive_count, negative_count, neutral_count = reward_summary(samples)
    weights = train(
        samples,
        epochs=max(args.epochs, 1),
        learning_rate=args.learning_rate,
        l2=max(args.l2, 0.0),
        seed=args.seed,
        reward_weighting=args.reward_weighting == "on",
    )
    output_path = pathlib.Path(args.output)
    write_ron_model(output_path, weights)
    print(
        "trained "
        f"{len(samples)} samples -> {output_path} "
        f"(avg_reward={avg_reward:.4f}, positive={positive_count}, "
        f"negative={negative_count}, neutral={neutral_count}, "
        f"reward_weighting={args.reward_weighting})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
