#!/usr/bin/env python3
"""Tests of scripts/build-scope.py: which pull requests build, on a real merge commit like the one Actions checks out."""

from __future__ import annotations

import importlib.util
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("build_scope", ROOT / "build-scope.py")
assert SPEC and SPEC.loader
SCOPE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCOPE)


def git(repo: pathlib.Path, *args: str) -> None:
    subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false", *args],
        cwd=repo,
        check=True,
        capture_output=True,
    )


def write(repo: pathlib.Path, files: dict[str, str]) -> None:
    for name, text in files.items():
        path = repo / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


def pull_request(repo: pathlib.Path, change) -> None:
    """A base commit, a topic branch that `change(repo)` edits, and the merge commit of the two checked out (what a
    `pull_request` run checks out), with the base moved on so the merge is a real one."""
    git(repo, "init", "-q", "-b", "main")
    write(repo, {"README.md": "x", "scripts/x": "x", "crates/a/lib.rs": "x"})
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "base")
    git(repo, "checkout", "-q", "-b", "topic")
    change(repo)
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "topic")
    git(repo, "checkout", "-q", "main")
    write(repo, {"elsewhere.txt": "base moved"})
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "base moves")
    git(repo, "merge", "-q", "--no-ff", "-m", "merge", "topic")


class PathTests(unittest.TestCase):
    def test_the_paths_a_build_reads_are_in(self) -> None:
        for path in (
            "crates/canon-cli/src/main.rs",
            "packages/dashboard/src/app.ts",
            ".cargo/config.toml",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".github/workflows/test.yml",
            ".github/actions/setup-rust/action.yml",
            "canon/skills/repo-scaffold/SKILL.md",
            "bun.lock",
            "package.json",
            "scripts/build-scope.py",
        ):
            self.assertTrue(SCOPE.touches_build(path), path)

    def test_everything_else_is_out(self) -> None:
        for path in (
            "README.md",
            "CHANGELOG.md",
            "docs/guide.md",
            ".canon/ledger/kind=finding/x.json",
            "packages/cli/src/index.ts",
            "packages/core-linux-x64/package.json",
            "examples/platformer/Cargo.toml",
            "tools/x",
            ".github/CODEOWNERS",
            "Cargo.tomlx",
            "cratesx/a.rs",
        ):
            self.assertFalse(SCOPE.touches_build(path), path)


class DecisionTests(unittest.TestCase):
    def test_a_manual_run_builds(self) -> None:
        self.assertTrue(SCOPE.builds("workflow_dispatch", "", []))
        self.assertTrue(SCOPE.builds("workflow_dispatch", "", None))

    def test_a_release_branch_builds_whatever_changed(self) -> None:
        self.assertTrue(SCOPE.builds("pull_request", "release/0.43.0", ["docs/a.md"]))
        self.assertTrue(SCOPE.builds("pull_request", "release/0.43.0", []))

    def test_a_branch_that_only_looks_like_a_release_does_not(self) -> None:
        self.assertFalse(SCOPE.builds("pull_request", "feat/release/0.43.0", ["docs/a.md"]))
        self.assertFalse(SCOPE.builds("pull_request", "release", ["docs/a.md"]))

    def test_a_change_with_no_build_path_skips_and_one_with_a_build_path_builds(self) -> None:
        self.assertFalse(SCOPE.builds("pull_request", "docs/x", ["docs/a.md", "README.md"]))
        self.assertFalse(SCOPE.builds("pull_request", "docs/x", []))
        self.assertTrue(SCOPE.builds("pull_request", "docs/x", ["docs/a.md", "crates/a/lib.rs"]))

    def test_a_change_that_cannot_be_listed_builds(self) -> None:
        self.assertTrue(SCOPE.builds("pull_request", "docs/x", None))


class MergeCommitTests(unittest.TestCase):
    """The changed files of a real merge commit: the base is its first parent."""

    def files(self, change) -> list[str] | None:
        with tempfile.TemporaryDirectory() as directory:
            repo = pathlib.Path(directory)
            pull_request(repo, change)
            files = SCOPE.changed_files(repo)
            return None if files is None else sorted(files)

    def test_lists_only_the_pull_requests_own_changes(self) -> None:
        self.assertEqual(self.files(lambda repo: write(repo, {"docs/a.md": "y"})), ["docs/a.md"])

    def test_a_rename_lists_the_old_path_and_the_new(self) -> None:
        def rename(repo: pathlib.Path) -> None:
            (repo / "tools").mkdir()
            (repo / "scripts" / "x").rename(repo / "tools" / "x")

        self.assertEqual(self.files(rename), ["scripts/x", "tools/x"])

    def test_a_path_with_odd_characters_is_listed_whole(self) -> None:
        self.assertEqual(self.files(lambda repo: write(repo, {"docs/ü ñ.md": "y"})), ["docs/ü ñ.md"])

    def test_a_directory_that_is_not_a_repository_cannot_be_listed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            self.assertIsNone(SCOPE.changed_files(pathlib.Path(directory)))

    def test_a_checkout_that_is_not_a_merge_commit_cannot_be_listed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = pathlib.Path(directory)
            git(repo, "init", "-q", "-b", "main")
            write(repo, {"a": "x"})
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "one")
            self.assertIsNone(SCOPE.changed_files(repo))


class EndToEndTests(unittest.TestCase):
    """The script as the `scope` job runs it, on pull requests of the shapes that went wrong or must be right."""

    def run_scope(self, change, event: str = "pull_request", head_ref: str = "feat/x") -> str:
        with tempfile.TemporaryDirectory() as directory:
            repo = pathlib.Path(directory) / "repo"
            repo.mkdir()
            pull_request(repo, change)
            output = pathlib.Path(directory) / "output"
            env = {**os.environ, "GITHUB_OUTPUT": str(output)}
            subprocess.run(
                [sys.executable, str(ROOT / "build-scope.py"), "--event", event, "--head-ref", head_ref, "--repo", str(repo)],
                env=env,
                check=True,
                capture_output=True,
            )
            return output.read_text(encoding="utf-8").strip()

    def test_four_thousand_changed_files_with_one_build_file_last_builds(self) -> None:
        # The 0.42 release PR: 4,043 files, the first in the filter the 3,638th.
        def change(repo: pathlib.Path) -> None:
            write(repo, {f".canon/archive/f{index:04d}.json": "y" for index in range(4000)})
            write(repo, {"scripts/zz-last": "y"})

        self.assertEqual(self.run_scope(change), "skip=false")

    def test_a_rename_out_of_a_build_path_builds(self) -> None:
        def change(repo: pathlib.Path) -> None:
            (repo / "tools").mkdir()
            (repo / "scripts" / "x").rename(repo / "tools" / "x")

        self.assertEqual(self.run_scope(change), "skip=false")

    def test_a_change_to_documents_only_skips(self) -> None:
        self.assertEqual(self.run_scope(lambda repo: write(repo, {"README.md": "y", "docs/a.md": "y"})), "skip=true")

    def test_a_release_branch_builds_though_only_documents_changed(self) -> None:
        change = lambda repo: write(repo, {"README.md": "y"})
        self.assertEqual(self.run_scope(change, head_ref="release/0.43.0"), "skip=false")

    def test_a_manual_run_builds(self) -> None:
        change = lambda repo: write(repo, {"README.md": "y"})
        self.assertEqual(self.run_scope(change, event="workflow_dispatch", head_ref=""), "skip=false")

    def test_a_checkout_that_cannot_list_the_change_builds(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "output"
            env = {**os.environ, "GITHUB_OUTPUT": str(output)}
            subprocess.run(
                [sys.executable, str(ROOT / "build-scope.py"), "--event", "pull_request", "--head-ref", "docs/x", "--repo", directory],
                env=env,
                check=True,
                capture_output=True,
            )
            self.assertEqual(output.read_text(encoding="utf-8").strip(), "skip=false")


if __name__ == "__main__":
    unittest.main()
