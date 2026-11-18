#!/usr/bin/env python3
"""Tests of scripts/ci-tested.py: the verdict for the answers the GitHub API can give (no call is made)."""

from __future__ import annotations

import datetime
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("ci_tested", ROOT / "ci-tested.py")
assert SPEC and SPEC.loader
TESTED = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TESTED)

REPO = "journeyWorker/canon"
TREE = "11775118aaaa"
NOW = datetime.datetime(2026, 11, 18, 12, 0, tzinfo=datetime.timezone.utc)


def marker(run_id: int = 1, **fields) -> dict:
    return {"name": f"tested-{TREE}", "expired": False, "expires_at": "2026-12-01T00:00:00Z", "workflow_run": {"id": run_id}, **fields}


def run(**fields) -> dict:
    return {
        "path": ".github/workflows/test.yml",
        "event": "pull_request",
        "status": "completed",
        "conclusion": "success",
        "repository": {"full_name": REPO},
        "head_repository": {"full_name": REPO},
        **fields,
    }


def verdict(artifacts: object, runs: dict[int, object] | None = None) -> str:
    runs = runs if runs is not None else {1: run()}

    def fetch(run_id: int) -> object:
        if run_id not in runs:
            raise OSError("no such run")
        return runs[run_id]

    return TESTED.decide(REPO, TREE, artifacts, fetch, NOW)


class VerdictTests(unittest.TestCase):
    def test_a_valid_marker_is_tested(self) -> None:
        self.assertEqual(verdict({"artifacts": [marker()]}), "tested")

    def test_no_marker_is_no_record(self) -> None:
        self.assertEqual(verdict({"artifacts": []}), "no_record")
        self.assertEqual(verdict({"artifacts": [{**marker(), "name": "tested-other"}]}), "no_record")

    def test_an_expired_marker_is_expired(self) -> None:
        self.assertEqual(verdict({"artifacts": [marker(expired=True)]}), "expired")
        self.assertEqual(verdict({"artifacts": [marker(expires_at="2026-11-01T00:00:00Z")]}), "expired")

    def test_a_run_that_is_not_green_is_run_not_green(self) -> None:
        self.assertEqual(verdict({"artifacts": [marker()]}, {1: run(conclusion="failure")}), "run_not_green")
        self.assertEqual(verdict({"artifacts": [marker()]}, {1: run(status="in_progress", conclusion=None)}), "run_not_green")

    def test_a_run_from_a_fork_is_foreign_repo(self) -> None:
        self.assertEqual(verdict({"artifacts": [marker()]}, {1: run(head_repository={"full_name": "someone/canon"})}), "foreign_repo")

    def test_another_workflow_is_wrong_workflow(self) -> None:
        self.assertEqual(verdict({"artifacts": [marker()]}, {1: run(path=".github/workflows/other.yml")}), "wrong_workflow")

    def test_a_push_run_is_wrong_event(self) -> None:
        self.assertEqual(verdict({"artifacts": [marker()]}, {1: run(event="push")}), "wrong_event")

    def test_an_answer_that_cannot_be_read_is_unreadable(self) -> None:
        for answer in (None, [], "x", {}, {"artifacts": "x"}):
            self.assertEqual(verdict(answer), "unreadable", answer)
        self.assertEqual(verdict({"artifacts": [marker()]}, {}), "unreadable")
        self.assertEqual(verdict({"artifacts": [marker()]}, {1: "not a run"}), "unreadable")
        self.assertEqual(verdict({"artifacts": [{**marker(), "workflow_run": None}]}), "unreadable")

    def test_two_markers_of_one_name_pick_the_valid_one(self) -> None:
        runs = {1: run(conclusion="failure"), 2: run()}
        self.assertEqual(verdict({"artifacts": [marker(1), marker(2)]}, runs), "tested")
        self.assertEqual(verdict({"artifacts": [marker(2), marker(1, expired=True)]}, runs), "tested")

    def test_only_a_valid_marker_is_tested(self) -> None:
        cases = [
            ({"artifacts": []}, None),
            ({"artifacts": [marker(expired=True)]}, None),
            ({"artifacts": [marker()]}, {1: run(conclusion="failure")}),
            ({"artifacts": [marker()]}, {1: run(head_repository={"full_name": "x/y"})}),
            ({"artifacts": [marker()]}, {1: run(path="x.yml")}),
            ({"artifacts": [marker()]}, {1: run(event="push")}),
            (None, None),
        ]
        for artifacts, runs in cases:
            self.assertNotEqual(verdict(artifacts, runs), "tested", (artifacts, runs))


class CommandTests(unittest.TestCase):
    def run_script(self, gh: str) -> tuple[int, dict, str]:
        with tempfile.TemporaryDirectory() as directory:
            out = pathlib.Path(directory) / "output"
            done = subprocess.run(
                [sys.executable, str(ROOT / "ci-tested.py"), "--repo", REPO, "--tree", TREE, "--gh", gh],
                env={**os.environ, "GITHUB_OUTPUT": str(out)},
                capture_output=True,
                text=True,
            )
            return done.returncode, json.loads(done.stdout), out.read_text(encoding="utf-8").strip()

    def test_a_failing_call_is_unreadable_and_exits_zero(self) -> None:
        code, document, output = self.run_script("false")
        self.assertEqual((code, document["verdict"], output), (0, "unreadable", "tested=false"))

    def test_a_missing_cli_is_unreadable_and_exits_zero(self) -> None:
        code, document, output = self.run_script("/nonexistent/gh")
        self.assertEqual((code, document["verdict"], output), (0, "unreadable", "tested=false"))

    def test_an_answer_that_is_not_json_is_unreadable_and_exits_zero(self) -> None:
        code, document, output = self.run_script("echo")
        self.assertEqual((code, document["verdict"], output), (0, "unreadable", "tested=false"))


if __name__ == "__main__":
    unittest.main()
