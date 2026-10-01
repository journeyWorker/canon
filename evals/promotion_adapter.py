#!/usr/bin/env python3
"""Convert independently observed Canon eval result files into a Rust promotion bundle.

This adapter deliberately accepts raw runner result files, not a hand-written
scoreboard or a claimed summary. It verifies every result integrity envelope,
recomputes the runner's paired scoreboard, rejects task/context mismatches and
regressions, and emits only measured provenance. It never invents a baseline,
quality metric, or candidate result.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
from typing import Any

HERE = pathlib.Path(__file__).resolve().parent


def canonical(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode()


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def load(path: pathlib.Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot read result {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise ValueError(f"result {path} is not an object")
    return value


def result_files(paths: list[pathlib.Path]) -> list[dict[str, Any]]:
    values = [load(path) for path in paths]
    if not values:
        raise ValueError("at least one baseline and candidate result file is required")
    try:
        runner_path = HERE / "run_eval.py"
        import importlib.util
        spec = importlib.util.spec_from_file_location("canon_run_eval", runner_path)
        if spec is None or spec.loader is None:
            raise ValueError("cannot load eval runner")
        runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runner)
        for value in values:
            runner.verify_payload(value)
    except ValueError:
        raise
    except Exception as exc:
        raise ValueError(f"result integrity verification failed: {exc}") from exc
    return values


def convert(args: argparse.Namespace) -> dict[str, Any]:
    baseline = result_files(args.baseline)
    candidate = result_files(args.candidate)
    import importlib.util
    spec = importlib.util.spec_from_file_location("canon_run_eval", HERE / "run_eval.py")
    assert spec and spec.loader
    runner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runner)
    tasks = runner.load_tasks(HERE / "tasks.json")
    corpus = runner.corpus_digest(HERE / "tasks.json")
    score = runner.scoreboard(baseline, candidate, tasks, corpus)
    if score["regressions"]:
        raise ValueError("paired evaluation contains regressions")
    rows = score.get("rows", [])
    if not rows or len(rows) != len(tasks):
        raise ValueError("paired evaluation does not cover the pinned corpus")
    baseline_success = score["baseline"].get("success_at_1")
    candidate_success = score["candidate"].get("success_at_1")
    if not isinstance(baseline_success, (int, float)) or not isinstance(candidate_success, (int, float)):
        raise ValueError("paired quality is unknown; success measurements are absent")
    uplift = float(candidate_success) - float(baseline_success)
    if uplift <= 0:
        raise ValueError("paired evaluation has no positive measured uplift")
    baseline_digests = score["provenance"].get("baseline_result_digests")
    candidate_digests = score["provenance"].get("candidate_result_digests")
    if not isinstance(baseline_digests, list) or not isinstance(candidate_digests, list) or len(baseline_digests) != len(candidate_digests):
        raise ValueError("paired result digest provenance is missing or unequal")
    if any(not isinstance(value, str) or not value for value in baseline_digests + candidate_digests):
        raise ValueError("paired result digest provenance contains unknown values")
    source_ids = args.source_trajectory_id
    if not source_ids:
        raise ValueError("at least one --source-trajectory-id is required")
    if any(not value.strip() for value in source_ids):
        raise ValueError("source trajectory ids must be nonempty")
    metrics = {
        "pairs": float(len(rows)),
        "uplift": uplift,
        "regressions": float(score["regressions"]),
        "baseline_success_at_1": float(baseline_success),
        "candidate_success_at_1": float(candidate_success),
    }
    unsigned = {
        "schema_version": 1,
        "candidate_strategy_id": args.candidate_strategy_id,
        "candidate_version": args.candidate_version,
        "candidate_lifecycle": "quarantined",
        "candidate_digest": args.candidate_digest,
        "baseline_result_digests": baseline_digests,
        "candidate_result_digests": candidate_digests,
        "corpus_version": corpus,
        "eval_version": args.eval_version,
        "context_digest": args.context_digest,
        "policy_digest": args.policy_digest,
        "model_digest": args.model_digest,
        "tool_digest": args.tool_digest,
        "paired_metrics": metrics,
        "passed": True,
        "decision": "pass",
        "source_trajectory_ids": source_ids,
    }
    # Matches canon-learn's EvaluationIntegrity field order and serde JSON
    # encoding. Rust activation recomputes this digest and validates all rows.
    return {**unsigned, "integrity_digest": digest(unsigned)}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", action="append", type=pathlib.Path, required=True)
    parser.add_argument("--candidate", action="append", type=pathlib.Path, required=True)
    parser.add_argument("--candidate-strategy-id", required=True)
    parser.add_argument("--candidate-version", required=True)
    parser.add_argument("--candidate-digest", required=True)
    parser.add_argument("--source-trajectory-id", action="append", required=True)
    parser.add_argument("--eval-version", required=True)
    parser.add_argument("--context-digest", required=True)
    parser.add_argument("--policy-digest", required=True)
    parser.add_argument("--model-digest", required=True)
    parser.add_argument("--tool-digest", required=True)
    parser.add_argument("-o", "--output", type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        result = convert(args)
        text = json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
        if args.output:
            args.output.write_text(text, encoding="utf-8")
        else:
            sys.stdout.write(text)
    except (ValueError, OSError) as exc:
        print(f"promotion adapter: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
