#!/usr/bin/env python3
"""Deterministic, network-free tests for the eval corpus and grader contract."""
from __future__ import annotations

import importlib.util
import json
import pathlib
import subprocess
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("run_eval", HERE / "run_eval.py")
assert SPEC and SPEC.loader
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


def git_repo(root: pathlib.Path) -> None:
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=root, check=True)
    (root / "tracked.txt").write_text("seed\n", encoding="utf-8")
    subprocess.run(["git", "add", "tracked.txt"], cwd=root, check=True)
    subprocess.run(["git", "-c", "user.name=eval", "-c", "user.email=eval@example.invalid", "commit", "-qm", "seed"], cwd=root, check=True)


def task_for(root: pathlib.Path) -> dict:
    return {
        "id": "test-task",
        "instructions": "test",
        "setup_seed": {"files": []},
        "expected_contract": {"checks": []},
        "allowed_diff_paths": ["allowed.txt"],
        "forbidden_side_effects": {"paths": ["forbidden.txt"]},
        "grader": {"command": ["python3", "-c", "print('exercised')"], "exit_code": 0, "output_schema": {"type": "text"}},
        "required_evidence": [{"path": ".canon/evidence/result.json", "format": "json", "required_keys": ["status"]}],
        "gate": {"evidence_path": ".canon/evidence/gate.json", "required_keys": ["status", "checks", "evidence"]},
    }


class CorpusTests(unittest.TestCase):
    def test_corpus_is_exactly_pinned_and_has_required_categories(self) -> None:
        tasks = RUNNER.load_tasks(HERE / "tasks.json")
        self.assertEqual(len(tasks), 40)
        self.assertEqual(len({tuple(task["allowed_diff_paths"]) for task in tasks.values()}), 40)
        categories = {task["category"] for task in tasks.values()}
        for category in ("doc-schema-conflict", "plan-adapter", "stale-evidence-refusal", "env-diagnosis", "generated-output", "provider-projection", "run-lineage", "unsafe-approval"):
            self.assertIn(category, categories)


class GraderContractTests(unittest.TestCase):
    def test_forbidden_edit_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            git_repo(root)
            task = task_for(root)
            (root / "forbidden.txt").write_text("must reject\n", encoding="utf-8")
            checks, failures, passed = RUNNER.check_contract(task, root, ["forbidden.txt"], 1, 1000, {}, {"command": ["python3", "-c", "print('exercised')"], "exit_code": 0, "timed_out": False})
            self.assertFalse(passed)
            self.assertTrue(any(check["name"] == "forbidden_side_effects" for check in failures))

    def test_false_green_and_missing_evidence_are_not_passes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            git_repo(root)
            task = task_for(root)
            result = RUNNER.grade(task, root, {"exit_code": 0, "timed_out": False, "command": ["fake-agent"]}, "seed", 1, 1000)
            self.assertFalse(result["outcome"]["passed"])
            self.assertTrue(result["outcome"]["false_pass"])
            self.assertTrue(any(not check["passed"] for check in result["evidence"]["checks"]))

    def test_missing_agent_requires_explicit_mode(self) -> None:
        self.assertEqual(RUNNER.main(["--task-id", "doc-schema-conflict-run-01"]), 2)

    def test_tampered_result_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            payload = RUNNER.finalize({"schema_version": 1, "task_id": "x", "outcome": {"passed": True}, "integrity": {"schema_version": 1}})
            payload["outcome"]["passed"] = False
            path = root / "result.json"
            path.write_text(json.dumps(payload), encoding="utf-8")
            with self.assertRaises(RUNNER.EvalError):
                RUNNER.verify_payload(RUNNER.load_json(path))

    def test_scoreboard_requires_pinned_task_sets_and_keeps_unknown_metrics_null(self) -> None:
        tasks = RUNNER.load_tasks(HERE / "tasks.json")
        corpus_hash = RUNNER.corpus_digest(HERE / "tasks.json")
        def result(task_id: str) -> dict:
            task = tasks[task_id]
            return RUNNER.finalize({"schema_version": RUNNER.SCHEMA_VERSION, "task_id": task_id, "provenance": {"corpus_digest": corpus_hash, "task_digest": RUNNER.digest(task), "base_commit": "base", "current_commit": "current"}, "outcome": {"passed": True, "false_pass": False}, "metrics": {"latency_ms": None, "cost_usd": None}, "integrity": {"schema_version": RUNNER.SCHEMA_VERSION}})
        baseline = [result(task_id) for task_id in tasks]
        candidate = [result(task_id) for task_id in tasks]
        score = RUNNER.scoreboard(baseline, candidate, tasks, corpus_hash)
        self.assertEqual(score["baseline"]["success_at_1"], 1.0)
        self.assertIsNone(score["candidate"]["latency_ms"])
        self.assertEqual(len(score["rows"]), 40)


if __name__ == "__main__":
    unittest.main()
