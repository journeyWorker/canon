#!/usr/bin/env python3
"""Decide whether and how much build-native.yml builds: the `scope` job's one script.

A pull request builds when its change touches what the binary is made of, and
always builds everything when it is a release (head branch `release/...`) or a manual run.
GitHub's `paths:` filter cannot do this: past 3,000 changed files it looks only
at the first 3,000 and skips the workflow when none of them matches (0.42's
release PR changed 4,043 files and the first match was the 3,638th).

The changed files are `git diff --no-renames --name-only` between the first
parent of the checked-out merge commit (the base) and the merge commit; a
rename lists both its old and new path. When the list cannot be had, the answer
is the most there is to build. The answers are the job outputs, written to $GITHUB_OUTPUT:

    skip     `true` only when nothing is to be built
    mode     `release` (not a pull request, a `release/` head, or a change that cannot be listed: every row, the man pages
             and the manifest), `native` (a pull request that touches `Cargo.lock`, a `build.rs` or `.cargo/`: every row, no man
             pages), `quick` (any other pull request that touches a build path: the x86_64 musl row alone), `skip`
    exclude  the matrix `exclude` list as JSON: the rows a `quick` run leaves out (`[]` for every other mode)

    build-scope.py --event <name> --head-ref <branch> [--repo <dir>]
"""

from __future__ import annotations

import argparse
import json
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
    "canon/skills/",
    "bun.lock",
    "package.json",
    "scripts/",
)
RELEASE_PREFIX = "release/"
# What makes a pull request build every row, though it is not a release: the lockfile, a build script anywhere, the cargo config.
NATIVE_FILES = ("Cargo.lock",)
NATIVE_NAMES = ("build.rs",)
NATIVE_DIRS = (".cargo/",)
# The matrix field that tells the one row a `quick` run builds (`quick: "true"`) from the others (`quick: "false"`), in build-native.yml.
QUICK_FIELD = "quick"
QUICK_EXCLUDE = [{"settings": {QUICK_FIELD: "false"}}]


def touches_build(path: str) -> bool:
    return any(path.startswith(entry) if entry.endswith("/") else path == entry for entry in BUILD_PATHS)


def touches_native(path: str) -> bool:
    return path in NATIVE_FILES or path.rsplit("/", 1)[-1] in NATIVE_NAMES or any(path.startswith(entry) for entry in NATIVE_DIRS)


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


def mode_of(event: str, head_ref: str, files: list[str] | None) -> str:
    """`release`, `native`, `quick` or `skip`."""
    if event != "pull_request" or head_ref.startswith(RELEASE_PREFIX) or files is None:
        return "release"
    if any(touches_native(path) for path in files):
        return "native"
    if any(touches_build(path) for path in files):
        return "quick"
    return "skip"


def exclude_for(mode: str) -> list[dict]:
    """The rows left out of the matrix: all but the quick row for `quick`, none for any other value (an unknown mode builds everything)."""
    return QUICK_EXCLUDE if mode == "quick" else []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Decide whether build-native.yml builds (the `scope` job).")
    parser.add_argument("--event", required=True, help="the GitHub event name")
    parser.add_argument("--head-ref", default="", help="the pull request's head branch")
    parser.add_argument("--repo", default=".", help="the checked-out repository")
    args = parser.parse_args(argv)
    needs_files = args.event == "pull_request" and not args.head_ref.startswith(RELEASE_PREFIX)
    files = changed_files(pathlib.Path(args.repo)) if needs_files else None
    mode = mode_of(args.event, args.head_ref, files)
    print(f"scope: {mode} (event {args.event or '-'}, head {args.head_ref or '-'}, {'changed files unknown' if files is None else f'{len(files)} changed files'})")
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as handle:
            handle.write(f"skip={'true' if mode == 'skip' else 'false'}\n")
            handle.write(f"mode={mode}\n")
            handle.write(f"exclude={json.dumps(exclude_for(mode), separators=(',', ':'))}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
