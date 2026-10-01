#!/usr/bin/env python3
"""Trusted evaluator-side gate reducer."""
from __future__ import annotations

import argparse
import json


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True)
    args = parser.parse_args()
    value = json.loads(open(args.input, encoding="utf-8").read())
    checks = value.get("checks")
    passed = isinstance(checks, list) and all(isinstance(check, dict) and check.get("passed") is True for check in checks)
    print(json.dumps({"status": "pass" if passed else "fail", "checks": checks if isinstance(checks, list) else [], "evidence": {"task_id": value.get("task_id"), "corpus_digest": value.get("corpus_digest"), "base_commit": value.get("base_commit"), "current_commit": value.get("current_commit")}}, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
