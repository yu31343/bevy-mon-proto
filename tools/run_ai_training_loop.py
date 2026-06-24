#!/usr/bin/env python3
"""Run the lightweight AI selfplay -> train -> eval loop."""

from __future__ import annotations

import argparse
import datetime as dt
import pathlib
import shutil
import subprocess
import sys


def run(command: list[str], cwd: pathlib.Path) -> None:
    print("+ " + " ".join(command), flush=True)
    subprocess.run(command, cwd=cwd, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run AI selfplay sampling, ranker training, and model evaluation."
    )
    parser.add_argument("--battles", type=int, default=5000)
    parser.add_argument("--eval-battles", type=int, default=1000)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--rounds", type=int, default=12)
    parser.add_argument(
        "--difficulty",
        choices=("easy", "normal", "hard", "expert"),
        default="expert",
    )
    parser.add_argument(
        "--output-root",
        default="battle_logs/ai_runs",
        help="Directory that receives timestamped run folders.",
    )
    parser.add_argument(
        "--promote-output",
        default=None,
        help="Optional path to copy the trained model to after eval succeeds.",
    )
    parser.add_argument("--epochs", type=int, default=50)
    parser.add_argument("--learning-rate", type=float, default=0.0005)
    parser.add_argument("--l2", type=float, default=0.0)
    args = parser.parse_args()

    repo_root = pathlib.Path(__file__).resolve().parents[1]
    timestamp = dt.datetime.now().strftime("%Y%m%d_%H%M%S")
    run_dir = repo_root / args.output_root / timestamp
    run_dir.mkdir(parents=True, exist_ok=True)

    samples_path = run_dir / "selfplay_samples.jsonl"
    model_path = run_dir / "ai_ranker_trained.ron"
    eval_path = run_dir / "eval_report.json"

    run(
        [
            "cargo",
            "run",
            "--",
            "--ai-selfplay",
            str(max(args.battles, 1)),
            "--ai-selfplay-output",
            str(samples_path),
            "--ai-selfplay-seed",
            str(args.seed),
            "--ai-selfplay-rounds",
            str(max(args.rounds, 1)),
            "--ai-selfplay-difficulty",
            args.difficulty,
        ],
        repo_root,
    )
    run(
        [
            sys.executable,
            "tools/train_ai_ranker.py",
            str(samples_path),
            "-o",
            str(model_path),
            "--epochs",
            str(max(args.epochs, 1)),
            "--learning-rate",
            str(args.learning_rate),
            "--l2",
            str(max(args.l2, 0.0)),
            "--reward-weighting",
            "on",
        ],
        repo_root,
    )
    run(
        [
            "cargo",
            "run",
            "--",
            "--ai-eval",
            str(max(args.eval_battles, 1)),
            "--ai-eval-model",
            str(model_path),
            "--ai-eval-output",
            str(eval_path),
            "--ai-eval-seed",
            str(args.seed),
            "--ai-eval-rounds",
            str(max(args.rounds, 1)),
            "--ai-eval-difficulty",
            args.difficulty,
        ],
        repo_root,
    )

    if args.promote_output:
        promote_path = repo_root / args.promote_output
        promote_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(model_path, promote_path)
        print(f"promoted model -> {promote_path}", flush=True)

    print(f"AI training loop complete: {run_dir}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
