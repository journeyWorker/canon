#!/usr/bin/env python3
"""Decide whether build-native.yml builds: the `scope` job's one script.

A pull request builds when its change touches what the binary is made of, and
always builds when it is a release (head branch `release/...`) or a manual run.
GitHub's `paths:` filter cannot do this: past 3,000 changed files it looks only
at the first 3,000 and skips the workflow when none of them matches (0.42's
release PR changed 4,043 files and the first match was the 3,638th).

The changed files are `git diff --no-renames --name-only` between the first
parent of the checked-out merge commit (the base) and the merge commit; a
rename lists both its old and new path. When the list cannot be had, the answer
is to build. The answer is the job output `skip` (`true` only when nothing is to
be built), written to $GITHUB_OUTPUT.

    build-scope.py --event <name> --head-ref <branch> [--repo <dir>]
"""

from __future__ import annotations

import argparse
import os
import pathlib
import subprocess
import sys

# The one list of what a build depends on: a path is in when it equals an entry or lies below an entry ending in `/`.
BUILD_PATHS = (
    "crates/",
    "packages/dashboard/",
    ".cargo/",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".github/workflows/",
    ".github/actions/",
    "scripts/",
)
RELEASE_PREFIX = "release/"


def touches_build(path: str) -> bool:
    return any(path.startswith(entry) if entry.endswith("/") else path == entry for entry in BUILD_PATHS)


def changed_files(repo: pathlib.Path) -> list[str] | None:
    """The paths the checked-out merge commit changes against its base, or None when git cannot say."""
    try:
        parents = subprocess.run(["git", "rev-list", "--parents", "-n", "1", "HEAD"], cwd=repo, capture_output=True, text=True, check=True).stdout.split()
        if len(parents) < 3:
            return None
        diff = subprocess.run(
            ["git", "diff", "--no-renames", "--name-only", "-z", f"{parents[1]}", "HEAD"],
            cwd=repo,
            capture_output=True,
            check=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    return [name.decode("utf-8", "surrogateescape") for name in diff.split(b"\0") if name]


def builds(event: str, head_ref: str, files: list[str] | None) -> bool:
    if event != "pull_request":
        return True
    if head_ref.startswith(RELEASE_PREFIX):
        return True
    if files is None:
        return True
    return any(touches_build(path) for path in files)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Decide whether build-native.yml builds (the `scope` job).")
    parser.add_argument("--event", required=True, help="the GitHub event name")
    parser.add_argument("--head-ref", default="", help="the pull request's head branch")
    parser.add_argument("--repo", default=".", help="the checked-out repository")
    args = parser.parse_args(argv)
    needs_files = args.event == "pull_request" and not args.head_ref.startswith(RELEASE_PREFIX)
    files = changed_files(pathlib.Path(args.repo)) if needs_files else None
    build = builds(args.event, args.head_ref, files)
    print(f"scope: {'build' if build else 'skip'} (event {args.event or '-'}, head {args.head_ref or '-'}, {'changed files unknown' if files is None else f'{len(files)} changed files'})")
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as handle:
            handle.write(f"skip={'false' if build else 'true'}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
