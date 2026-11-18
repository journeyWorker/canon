#!/usr/bin/env python3
"""Tests of scripts/release-build.py: the verdict and exit value for API answers and downloaded files given as input."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("release_build", ROOT / "release-build.py")
assert SPEC and SPEC.loader
RB = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RB)

REPO = "owner/canon"
TREE = "1" * 40
SHA = "a" * 40
PARENT = "b" * 40
WORKFLOW = ".github/workflows/build-native.yml"


def artifact(run_id: int, created: str, expired: bool = False) -> dict:
    return {"name": f"build-manifest-{TREE}", "expired": expired, "created_at": created, "workflow_run": {"id": run_id}}


def run(run_id: int, **fields) -> dict:
    base = {"id": run_id, "path": WORKFLOW, "conclusion": "success", "head_sha": SHA, "head_repository": {"full_name": REPO}}
    return {**base, **fields}


class FakeApi:
    """GitHub's answers by path prefix; a path nobody answers raises like a failed call."""

    def __init__(self, artifacts: list[dict], runs: dict[int, dict] | None = None, by_sha: dict[str, list[dict]] | None = None) -> None:
        self.artifacts = artifacts
        self.runs = runs or {}
        self.by_sha = by_sha or {}

    def __call__(self, path: str) -> object:
        if "/actions/artifacts?" in path:
            return {"artifacts": self.artifacts}
        if "/actions/runs/" in path:
            return self.runs[int(path.rsplit("/", 1)[1])]
        if "/workflows/" in path:
            sha = path.split("head_sha=")[1].split("&")[0]
            return {"workflow_runs": self.by_sha.get(sha, [])}
        raise RB.Unreadable(path)


def find(api: FakeApi) -> dict:
    return RB.find(api, REPO, TREE, [SHA, PARENT])


class FindTests(unittest.TestCase):
    def test_a_valid_candidate_passes_with_its_run(self) -> None:
        answer = find(FakeApi([artifact(7, "2026-11-18T01:00:00Z")], {7: run(7)}))
        self.assertEqual((answer["verdict"], answer["run_id"]), ("pass", 7))

    def test_the_newest_valid_candidate_wins(self) -> None:
        api = FakeApi([artifact(1, "2026-11-17T01:00:00Z"), artifact(2, "2026-11-18T01:00:00Z")], {1: run(1), 2: run(2)})
        self.assertEqual(find(api)["run_id"], 2)

    def test_a_valid_older_candidate_passes_when_the_newest_is_not_green(self) -> None:
        api = FakeApi([artifact(1, "2026-11-17T01:00:00Z"), artifact(2, "2026-11-18T01:00:00Z")], {1: run(1), 2: run(2, conclusion="failure")})
        self.assertEqual(find(api)["run_id"], 1)

    def test_no_candidate_and_no_run_is_no_run(self) -> None:
        self.assertEqual(find(FakeApi([]))["verdict"], "no_run")

    def test_a_run_of_the_commit_that_is_still_going_is_in_progress(self) -> None:
        api = FakeApi([], by_sha={SHA: [run(9, status="in_progress", conclusion=None)]})
        self.assertEqual(find(api)["verdict"], "in_progress")

    def test_a_running_run_of_a_parent_is_in_progress(self) -> None:
        api = FakeApi([], by_sha={PARENT: [run(9, status="queued", conclusion=None)]})
        self.assertEqual(find(api)["verdict"], "in_progress")

    def test_a_green_run_of_the_commit_that_built_another_tree_is_tree_mismatch(self) -> None:
        api = FakeApi([], by_sha={SHA: [run(9, status="completed")]})
        self.assertEqual(find(api)["verdict"], "tree_mismatch")

    def test_only_failed_runs_of_the_commit_are_no_run(self) -> None:
        api = FakeApi([], by_sha={SHA: [run(9, status="completed", conclusion="failure")]})
        self.assertEqual(find(api)["verdict"], "no_run")

    def test_expired_candidates_are_expired(self) -> None:
        self.assertEqual(find(FakeApi([artifact(1, "2026-11-01T01:00:00Z", expired=True)]))["verdict"], "expired")

    def test_a_run_that_is_not_green_is_run_not_green(self) -> None:
        self.assertEqual(find(FakeApi([artifact(1, "x")], {1: run(1, conclusion="failure")}))["verdict"], "run_not_green")

    def test_a_fork_run_is_foreign_repo(self) -> None:
        self.assertEqual(find(FakeApi([artifact(1, "x")], {1: run(1, head_repository={"full_name": "mallory/canon"})}))["verdict"], "foreign_repo")

    def test_another_workflow_is_wrong_workflow(self) -> None:
        self.assertEqual(find(FakeApi([artifact(1, "x")], {1: run(1, path=".github/workflows/publish.yml")}))["verdict"], "wrong_workflow")

    def test_an_unreadable_api_raises_for_exit_two(self) -> None:
        def broken(path: str) -> object:
            raise RB.Unreadable("down")

        with self.assertRaises(RB.Unreadable):
            RB.find(broken, REPO, TREE, [SHA])


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class Download:
    """A directory shaped like fetch-build's download, with its manifest."""

    def __init__(self, directory: pathlib.Path) -> None:
        self.dir = directory
        self.pages = {"man1/canon.1": b"page"}
        self.binaries = {target: f"binary of {target}".encode() for target in RB.TARGETS}
        for target, name in RB.TARGETS.items():
            folder = directory / f"canon-binary-{target}"
            folder.mkdir(parents=True)
            (folder / name).write_bytes(self.binaries[target])
            (folder / f"{name}.sha256").write_text(f"{sha(self.binaries[target])}  {name}\n", encoding="utf-8")
        for page, data in self.pages.items():
            (directory / "man-pages" / page).parent.mkdir(parents=True, exist_ok=True)
            (directory / "man-pages" / page).write_bytes(data)
        self.manifest = {
            "schema": RB.SCHEMA,
            "tree": TREE,
            "commit": SHA,
            "event": "pull_request",
            "run": {"id": 7, "attempt": 1},
            "binaries": {target: sha(data) for target, data in self.binaries.items()},
            "man": {page: sha(data) for page, data in self.pages.items()},
        }
        self.write_manifest()

    def write_manifest(self) -> None:
        (self.dir / "manifest").mkdir(exist_ok=True)
        (self.dir / "manifest" / RB.MANIFEST_FILE).write_text(json.dumps(self.manifest), encoding="utf-8")


def attested(run_id: int):
    def attest(path: pathlib.Path, repo: str) -> list:
        return [{"verificationResult": {"signature": {"certificate": {"runInvocationURI": f"https://github.com/{REPO}/actions/runs/{run_id}/attempts/1"}}}}]

    return attest


def nothing_attested(path: pathlib.Path, repo: str) -> list:
    return []


class VerifyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.download = Download(pathlib.Path(self.temp.name))

    def verdict(self, attest=None, run_id: int = 7, tree: str = TREE) -> str:
        return RB.verify(self.download.dir, REPO, run_id, tree, attest or attested(7))["verdict"]

    def test_a_matching_download_passes(self) -> None:
        self.assertEqual(self.verdict(), "pass")

    def test_a_manifest_of_another_tree_is_tree_mismatch(self) -> None:
        self.assertEqual(self.verdict(tree="2" * 40), "tree_mismatch")

    def test_a_manifest_without_a_target_is_incomplete(self) -> None:
        del self.download.manifest["binaries"]["x86_64-pc-windows-msvc"]
        self.download.write_manifest()
        self.assertEqual(self.verdict(), "incomplete")

    def test_a_missing_binary_is_incomplete(self) -> None:
        (self.download.dir / "canon-binary-aarch64-apple-darwin" / "canon").unlink()
        self.assertEqual(self.verdict(), "incomplete")

    def test_a_missing_manifest_is_incomplete(self) -> None:
        (self.download.dir / "manifest" / RB.MANIFEST_FILE).unlink()
        self.assertEqual(self.verdict(), "incomplete")

    def test_a_changed_byte_is_checksum_mismatch(self) -> None:
        binary = self.download.dir / "canon-binary-x86_64-unknown-linux-musl" / "canon"
        binary.write_bytes(binary.read_bytes() + b"x")
        self.assertEqual(self.verdict(), "checksum_mismatch")

    def test_a_changed_pair_alone_is_checksum_mismatch(self) -> None:
        pair = self.download.dir / "canon-binary-x86_64-unknown-linux-musl" / "canon.sha256"
        pair.write_text(f"{sha(b'other')}  canon\n", encoding="utf-8")
        self.assertEqual(self.verdict(), "checksum_mismatch")

    def test_a_changed_man_page_is_checksum_mismatch(self) -> None:
        (self.download.dir / "man-pages" / "man1" / "canon.1").write_bytes(b"changed")
        self.assertEqual(self.verdict(), "checksum_mismatch")

    def test_a_man_page_the_manifest_does_not_list_is_incomplete(self) -> None:
        (self.download.dir / "man-pages" / "extra.1").write_bytes(b"x")
        self.assertEqual(self.verdict(), "incomplete")

    def test_no_attestation_is_attestation_invalid(self) -> None:
        self.assertEqual(self.verdict(nothing_attested), "attestation_invalid")

    def test_an_attestation_of_another_run_is_attestation_invalid(self) -> None:
        self.assertEqual(self.verdict(attested(8)), "attestation_invalid")


class CommandTests(unittest.TestCase):
    def run_cli(self, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run([sys.executable, str(ROOT / "release-build.py"), *args], capture_output=True, text=True, check=False)

    def test_a_bad_argument_exits_two(self) -> None:
        self.assertEqual(self.run_cli("find").returncode, 2)

    def test_an_api_that_cannot_be_read_exits_two(self) -> None:
        # No `gh` on the PATH: the answer cannot be given, which is not a pass.
        done = subprocess.run(
            [sys.executable, str(ROOT / "release-build.py"), "find", "--repo", REPO, "--tree", TREE, "--sha", SHA],
            capture_output=True,
            text=True,
            check=False,
            env={"PATH": ""},
        )
        self.assertEqual(done.returncode, 2)

    def test_record_writes_a_manifest_that_verify_accepts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            download = Download(pathlib.Path(directory) / "d")
            out = pathlib.Path(directory) / "m.json"
            done = self.run_cli("record", "--tree", TREE, "--commit", SHA, "--event", "pull_request", "--run-id", "7", "--run-attempt", "1", "--artifacts", str(download.dir), "--man", str(download.dir / "man-pages"), "--out", str(out))
            self.assertEqual(done.returncode, 0, done.stderr)
            (download.dir / "manifest" / RB.MANIFEST_FILE).write_text(out.read_text(encoding="utf-8"), encoding="utf-8")
            self.assertEqual(RB.verify(download.dir, REPO, 7, TREE, attested(7))["verdict"], "pass")

    def test_record_refuses_a_binary_that_differs_from_its_pair(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            download = Download(pathlib.Path(directory) / "d")
            binary = download.dir / "canon-binary-aarch64-apple-darwin" / "canon"
            binary.write_bytes(b"changed")
            out = pathlib.Path(directory) / "m.json"
            done = self.run_cli("record", "--tree", TREE, "--commit", SHA, "--event", "pull_request", "--run-id", "7", "--run-attempt", "1", "--artifacts", str(download.dir), "--man", str(download.dir / "man-pages"), "--out", str(out))
            self.assertEqual(done.returncode, 1)
            self.assertEqual(json.loads(done.stdout)["verdict"], "checksum_mismatch")
            self.assertFalse(out.exists())


if __name__ == "__main__":
    unittest.main()
