#!/usr/bin/env python3
"""Tests of scripts/build-scope.py: which pull requests build, on a real merge commit like the one Actions checks out."""

from __future__ import annotations

import importlib.util
import os
import pathlib
import subprocess
import sys
import tempfile
import json
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
    def mode(self, event: str, head: str, files: list[str] | None) -> str:
        return SCOPE.mode_of(event, head, files)

    def test_a_manual_run_is_a_release(self) -> None:
        self.assertEqual(self.mode("workflow_dispatch", "", []), "release")
        self.assertEqual(self.mode("workflow_dispatch", "", None), "release")

    def test_a_release_branch_is_a_release_whatever_changed(self) -> None:
        self.assertEqual(self.mode("pull_request", "release/0.44.0", ["docs/a.md"]), "release")
        self.assertEqual(self.mode("pull_request", "release/0.44.0", []), "release")
        self.assertEqual(self.mode("pull_request", "release/0.44.0", ["crates/a/lib.rs"]), "release")

    def test_a_branch_that_only_looks_like_a_release_is_not(self) -> None:
        self.assertEqual(self.mode("pull_request", "feat/release/0.44.0", ["docs/a.md"]), "skip")
        self.assertEqual(self.mode("pull_request", "release", ["docs/a.md"]), "skip")

    def test_a_change_to_crates_only_is_quick(self) -> None:
        self.assertEqual(self.mode("pull_request", "feat/x", ["crates/a/lib.rs"]), "quick")
        self.assertEqual(self.mode("pull_request", "feat/x", ["docs/a.md", "scripts/x", ".github/workflows/test.yml"]), "quick")

    def test_the_lockfile_a_build_script_and_the_cargo_config_are_native(self) -> None:
        for path in ("Cargo.lock", "crates/canon-cli/build.rs", "crates/x/y/build.rs", "build.rs", ".cargo/config.toml"):
            self.assertEqual(self.mode("pull_request", "feat/x", ["crates/a/lib.rs", path]), "native", path)

    def test_a_file_that_only_looks_like_one_of_them_is_quick(self) -> None:
        for path in ("crates/foo/build.rs.bak", "crates/foo/xbuild.rs", "crates/Cargo.lockx"):
            self.assertEqual(self.mode("pull_request", "feat/x", [path]), "quick", path)

    def test_a_change_with_no_build_path_skips(self) -> None:
        self.assertEqual(self.mode("pull_request", "docs/x", ["docs/a.md", "README.md"]), "skip")
        self.assertEqual(self.mode("pull_request", "docs/x", []), "skip")

    def test_a_change_that_cannot_be_listed_is_a_release(self) -> None:
        self.assertEqual(self.mode("pull_request", "docs/x", None), "release")

    def test_only_the_quick_mode_leaves_rows_out(self) -> None:
        self.assertTrue(SCOPE.exclude_for("quick"))
        for mode in ("release", "native", "skip", "", "no-such-mode"):
            self.assertEqual(SCOPE.exclude_for(mode), [], mode)


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

    def run_scope(self, change, event: str = "pull_request", head_ref: str = "feat/x") -> dict[str, str]:
        with tempfile.TemporaryDirectory() as directory:
            repo = pathlib.Path(directory) / "repo"
            repo.mkdir()
            pull_request(repo, change)
            return self.outputs(repo, event, head_ref)

    def outputs(self, repo: pathlib.Path, event: str, head_ref: str) -> dict[str, str]:
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "output"
            env = {**os.environ, "GITHUB_OUTPUT": str(output)}
            subprocess.run(
                [sys.executable, str(ROOT / "build-scope.py"), "--event", event, "--head-ref", head_ref, "--repo", str(repo)],
                env=env,
                check=True,
                capture_output=True,
            )
            return dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())

    def test_four_thousand_changed_files_with_one_build_file_last_builds(self) -> None:
        # The 0.42 release PR: 4,043 files, the first in the filter the 3,638th.
        def change(repo: pathlib.Path) -> None:
            write(repo, {f".canon/archive/f{index:04d}.json": "y" for index in range(4000)})
            write(repo, {"scripts/zz-last": "y"})

        got = self.run_scope(change)
        self.assertEqual((got["skip"], got["mode"]), ("false", "quick"))

    def test_a_rename_out_of_a_build_path_builds(self) -> None:
        def change(repo: pathlib.Path) -> None:
            (repo / "tools").mkdir()
            (repo / "scripts" / "x").rename(repo / "tools" / "x")

        self.assertEqual(self.run_scope(change)["skip"], "false")

    def test_a_change_to_crates_is_quick_and_leaves_the_other_rows_out(self) -> None:
        got = self.run_scope(lambda repo: write(repo, {"crates/a/lib.rs": "y"}))
        self.assertEqual((got["skip"], got["mode"]), ("false", "quick"))
        self.assertTrue(json.loads(got["exclude"]))

    def test_a_change_to_the_lockfile_is_native_with_every_row(self) -> None:
        got = self.run_scope(lambda repo: write(repo, {"Cargo.lock": "y"}))
        self.assertEqual((got["skip"], got["mode"], json.loads(got["exclude"])), ("false", "native", []))

    def test_a_change_to_documents_only_skips(self) -> None:
        got = self.run_scope(lambda repo: write(repo, {"README.md": "y", "docs/a.md": "y"}))
        self.assertEqual(got["skip"], "true")

    def test_a_release_branch_is_a_release_though_only_documents_changed(self) -> None:
        got = self.run_scope(lambda repo: write(repo, {"README.md": "y"}), head_ref="release/0.44.0")
        self.assertEqual((got["skip"], got["mode"], json.loads(got["exclude"])), ("false", "release", []))

    def test_a_manual_run_is_a_release(self) -> None:
        got = self.run_scope(lambda repo: write(repo, {"README.md": "y"}), event="workflow_dispatch", head_ref="")
        self.assertEqual((got["skip"], got["mode"]), ("false", "release"))

    def test_a_checkout_that_cannot_list_the_change_is_a_release(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            got = self.outputs(pathlib.Path(directory), "pull_request", "docs/x")
        self.assertEqual((got["skip"], got["mode"]), ("false", "release"))


if __name__ == "__main__":
    unittest.main()
