#!/usr/bin/env python3
"""Focused integrity tests for the promotion result adapter."""
from __future__ import annotations

import importlib.util
import json
import pathlib
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("promotion_adapter", HERE / "promotion_adapter.py")
assert SPEC and SPEC.loader
ADAPTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADAPTER)

RUN_SPEC = importlib.util.spec_from_file_location("run_eval", HERE / "run_eval.py")
assert RUN_SPEC and RUN_SPEC.loader
RUNNER = importlib.util.module_from_spec(RUN_SPEC)
RUN_SPEC.loader.exec_module(RUNNER)


class PromotionAdapterIntegrityTests(unittest.TestCase):
    def test_result_files_reject_tampered_runner_payload(self) -> None:
        payload = RUNNER.finalize({"schema_version": RUNNER.SCHEMA_VERSION, "task_id": "x", "outcome": {"passed": True}})
        payload["outcome"]["passed"] = False
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "result.json"
            path.write_text(json.dumps(payload), encoding="utf-8")
            with self.assertRaises(ValueError):
                ADAPTER.result_files([path])

    def test_result_files_require_actual_object_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "summary.json"
            path.write_text(json.dumps({"rows": [], "passed": True}), encoding="utf-8")
            with self.assertRaises(ValueError):
                ADAPTER.result_files([path])


if __name__ == "__main__":
    unittest.main()
