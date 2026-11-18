#!/usr/bin/env python3
"""Has this tree already been tested by a green pull request run: the `tested` job's one script.

A pull request run of test.yml that is green on every job uploads the artifact `tested-<tree>` (its `record` job), `<tree>` being the
tree of the merge ref it checked out. A push to main whose commit has that tree needs no second run of the tests. This asks the
GitHub API for that artifact and for the run that uploaded it, and prints one line of JSON:

    {"verdict": "tested"}                      the tree was tested: a valid marker, from a green pull request run of this repository's test.yml
    {"verdict": "<reason>"}                    anything else; the tests run

Reasons: `no_record` (no marker of that name), `expired`, `run_not_green` (the run did not conclude `success`), `foreign_repo` (the run
is from a fork), `wrong_workflow` (not .github/workflows/test.yml), `wrong_event` (not a `pull_request` run), `unreadable` (the answer of
the API cannot be read, or the call failed). The exit status is always 0: when this cannot tell, the tests run. With GITHUB_OUTPUT set,
`tested=true|false` is written there too.

    ci-tested.py --repo <owner/name> --tree <tree> [--gh gh]
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import subprocess
import sys
from typing import Callable

WORKFLOW_PATH = ".github/workflows/test.yml"
MARKER = "tested-"


def _expired(artifact: dict, now: datetime.datetime) -> bool:
    if artifact.get("expired") is True:
        return True
    stamp = artifact.get("expires_at")
    if not stamp:
        return False
    return datetime.datetime.fromisoformat(stamp.replace("Z", "+00:00")) <= now


def _marker_verdict(artifact: dict, repo: str, run: dict, now: datetime.datetime) -> str:
    if _expired(artifact, now):
        return "expired"
    head = run.get("head_repository") or {}
    base = run.get("repository") or {}
    if head.get("full_name") != repo or base.get("full_name") != repo:
        return "foreign_repo"
    if str(run.get("path", "")).split("@", 1)[0] != WORKFLOW_PATH:
        return "wrong_workflow"
    if run.get("event") != "pull_request":
        return "wrong_event"
    if run.get("status") != "completed" or run.get("conclusion") != "success":
        return "run_not_green"
    return "tested"


def decide(repo: str, tree: str, artifacts: object, fetch_run: Callable[[int], object], now: datetime.datetime) -> str:
    """The verdict for `tree`, from the answer of the artifacts API and `fetch_run(run id)`, the answer of the run API."""
    try:
        if not isinstance(artifacts, dict) or not isinstance(artifacts.get("artifacts"), list):
            return "unreadable"
        markers = [a for a in artifacts["artifacts"] if isinstance(a, dict) and a.get("name") == f"{MARKER}{tree}"]
        if not markers:
            return "no_record"
        verdicts: list[str] = []
        for artifact in markers:
            if _expired(artifact, now):
                verdicts.append("expired")
                continue
            run_id = (artifact.get("workflow_run") or {}).get("id")
            if not isinstance(run_id, int):
                return "unreadable"
            run = fetch_run(run_id)
            if not isinstance(run, dict):
                return "unreadable"
            verdicts.append(_marker_verdict(artifact, repo, run, now))
        return "tested" if "tested" in verdicts else verdicts[0]
    except (ValueError, TypeError, AttributeError, OSError, subprocess.SubprocessError):
        return "unreadable"


def gh_api(gh: str, path: str) -> object:
    done = subprocess.run([gh, "api", path], capture_output=True, text=True, check=True, timeout=60)
    return json.loads(done.stdout)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Has this tree been tested by a green pull request run (the `tested` job).")
    parser.add_argument("--repo", required=True, help="owner/name")
    parser.add_argument("--tree", required=True, help="the tree of the checked-out commit")
    parser.add_argument("--gh", default="gh", help="the GitHub CLI")
    args = parser.parse_args(argv)
    now = datetime.datetime.now(datetime.timezone.utc)
    try:
        artifacts = gh_api(args.gh, f"repos/{args.repo}/actions/artifacts?name=tested-{args.tree}&per_page=100")
        verdict = decide(args.repo, args.tree, artifacts, lambda run_id: gh_api(args.gh, f"repos/{args.repo}/actions/runs/{run_id}"), now)
    except (ValueError, OSError, subprocess.SubprocessError):
        verdict = "unreadable"
    print(json.dumps({"verdict": verdict}))
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as handle:
            handle.write(f"tested={'true' if verdict == 'tested' else 'false'}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
