#!/usr/bin/env python3
"""The release binaries are built once, by build-native.yml; publish.yml fetches them.

Three commands share one manifest schema (`build-manifest.json`, below):

    release-build.py record --tree T --commit C --event E --run-id N --run-attempt N --artifacts DIR --man DIR --out FILE
    release-build.py find   --repo OWNER/NAME [--tree T] [--sha SHA]...
    release-build.py verify --repo OWNER/NAME --run-id N --dir DIR [--tree T]

`record` (build-native.yml `manifest` job) writes the manifest of the tree the run built: the checked-out merge ref's
`HEAD^{tree}`, the sha256 of the four binaries and of every man page. `find` (publish.yml `fetch-build`) looks for the
manifest artifact `build-manifest-<tree>` of the tree being published, and answers with the build-native run that
uploaded it. `verify` matches what was downloaded to the manifest and checks the manifest's attestation.

The answer of `find` and `verify` is one JSON line on stdout: `verdict` (`pass`, or a rejection code) and, for a
rejection, `reason`. The exit value is 0 for a pass, 1 for a rejection, 2 when no answer can be given (the API cannot be
read, bad arguments). A rejection also writes the command that fixes it to stderr: its wording is not a contract, the
verdict is. Standard library only; it runs on the runner's `python3` with the runner's `gh`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
from typing import Callable

SCHEMA = 1
MANIFEST_FILE = "build-manifest.json"
MANIFEST_ARTIFACT = "build-manifest-"
WORKFLOW = ".github/workflows/build-native.yml"
# target -> the binary's file name in its `canon-binary-<target>` artifact.
TARGETS = {
    "aarch64-apple-darwin": "canon",
    "x86_64-unknown-linux-musl": "canon",
    "aarch64-unknown-linux-musl": "canon",
    "x86_64-pc-windows-msvc": "canon.exe",
}
MAN_ARTIFACT = "man-pages"
RUNNING = ("queued", "in_progress", "waiting", "pending", "requested")

# Verdict -> the command that fixes it (a hint on stderr).
PRESCRIPTION = {
    "no_run": "gh workflow run build-native.yml --ref <branch or tag>, then gh run rerun <publish run> --failed",
    "in_progress": "wait for the build-native run of this commit, then gh run rerun <publish run> --failed",
    "tree_mismatch": "merge the base into the branch and rerun its pull request, or gh workflow run build-native.yml --ref <ref>",
    "expired": "gh workflow run build-native.yml --ref <ref>",
    "run_not_green": "fix the failure of the build-native run",
    "foreign_repo": "investigate: the run is from another repository (a fork's pull request)",
    "wrong_workflow": "investigate: the run is not this repository's build-native.yml",
    "incomplete": "gh workflow run build-native.yml --ref <ref> to build the same tree again",
    "checksum_mismatch": "investigate, then build again",
    "attestation_invalid": "build again with gh workflow run build-native.yml --ref <ref>",
}


class Unreadable(Exception):
    """No answer can be given (the API or a tool cannot be read): exit 2."""


def reject(verdict: str, reason: str) -> dict:
    return {"verdict": verdict, "reason": reason}


def sha256_of(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def pair_digest(path: pathlib.Path) -> str | None:
    """The hash a `<file>.sha256` pair names (`<hex>  <name>`), or None when it is missing or unreadable."""
    try:
        first = path.read_text(encoding="utf-8").split()
    except OSError:
        return None
    return first[0].lower() if first and re.fullmatch(r"[0-9a-fA-F]{64}", first[0]) else None


def man_files(directory: pathlib.Path) -> dict[str, str]:
    return {p.relative_to(directory).as_posix(): sha256_of(p) for p in sorted(directory.rglob("*")) if p.is_file()}


# ---------------------------------------------------------------- record


def record(tree: str, commit: str, event: str, run_id: int, attempt: int, artifacts: pathlib.Path, man: pathlib.Path) -> dict:
    """The manifest of the downloaded artifacts, or a rejection when one is missing or differs from its `.sha256` pair."""
    binaries: dict[str, str] = {}
    for target, name in TARGETS.items():
        binary = artifacts / f"canon-binary-{target}" / name
        if not binary.is_file():
            return reject("incomplete", f"no binary for {target}")
        actual = sha256_of(binary)
        if pair_digest(binary.with_name(name + ".sha256")) != actual:
            return reject("checksum_mismatch", f"{target}: the binary differs from its .sha256 pair")
        binaries[target] = actual
    pages = man_files(man) if man.is_dir() else {}
    if not pages:
        return reject("incomplete", "no man pages")
    return {
        "verdict": "pass",
        "manifest": {
            "schema": SCHEMA,
            "tree": tree,
            "commit": commit,
            "event": event,
            "run": {"id": run_id, "attempt": attempt},
            "binaries": binaries,
            "man": pages,
        },
    }


# ---------------------------------------------------------------- find

Api = Callable[[str], object]


def gh_api(path: str) -> object:
    try:
        done = subprocess.run(["gh", "api", path], capture_output=True, text=True, check=False)
    except OSError as error:
        raise Unreadable(f"gh cannot run: {error}") from error
    if done.returncode != 0:
        raise Unreadable(f"gh api {path} failed: {done.stderr.strip()[:300]}")
    try:
        return json.loads(done.stdout)
    except json.JSONDecodeError as error:
        raise Unreadable(f"gh api {path} answered no JSON") from error


def _run_path(run: dict) -> str:
    return str(run.get("path", "")).split("@", 1)[0]


def _run_verdict(run: dict, repo: str) -> dict | None:
    """The rejection of one build-native candidate run, or None when it may supply the binaries."""
    head = (run.get("head_repository") or {}).get("full_name", "")
    if head.lower() != repo.lower():
        return reject("foreign_repo", f"run {run.get('id')} comes from {head or 'an unknown repository'}")
    if _run_path(run) != WORKFLOW:
        return reject("wrong_workflow", f"run {run.get('id')} is {_run_path(run) or 'an unknown workflow'}")
    if run.get("conclusion") != "success":
        return reject("run_not_green", f"run {run.get('id')} concluded {run.get('conclusion')}")
    return None


def find(api: Api, repo: str, tree: str, shas: list[str]) -> dict:
    """The build-native run that built `tree`, or why there is none. Never waits."""
    listing = api(f"repos/{repo}/actions/artifacts?name={MANIFEST_ARTIFACT}{tree}&per_page=100")
    candidates = [a for a in (listing.get("artifacts", []) if isinstance(listing, dict) else []) if a.get("name") == f"{MANIFEST_ARTIFACT}{tree}"]
    candidates.sort(key=lambda a: str(a.get("created_at", "")), reverse=True)
    first: dict | None = None
    for artifact in candidates:
        if artifact.get("expired"):
            first = first or reject("expired", f"the manifest of tree {tree} has expired")
            continue
        run_id = (artifact.get("workflow_run") or {}).get("id")
        run = api(f"repos/{repo}/actions/runs/{run_id}")
        if not isinstance(run, dict):
            raise Unreadable(f"run {run_id} answered no object")
        problem = _run_verdict(run, repo)
        if problem is None:
            return {"verdict": "pass", "run_id": run_id, "tree": tree, "commit": run.get("head_sha", "")}
        first = first or problem
    if first is not None:
        return first
    runs: list[dict] = []
    for sha in dict.fromkeys(shas):
        answer = api(f"repos/{repo}/actions/workflows/{pathlib.PurePosixPath(WORKFLOW).name}/runs?head_sha={sha}&per_page=100")
        runs.extend(answer.get("workflow_runs", []) if isinstance(answer, dict) else [])
    if any(r.get("status") in RUNNING for r in runs):
        return reject("in_progress", "a build-native run of this commit is still running")
    if any(r.get("conclusion") == "success" for r in runs):
        return reject("tree_mismatch", f"a build-native run of this commit built another tree than {tree}")
    return reject("no_run", f"no build-native run built tree {tree}")


# ---------------------------------------------------------------- verify


def gh_attestation(manifest: pathlib.Path, repo: str) -> list:
    """`gh attestation verify` of the manifest: the attestations it accepts, or an empty list when it refuses."""
    try:
        done = subprocess.run(
            ["gh", "attestation", "verify", str(manifest), "--repo", repo, "--signer-workflow", f"{repo}/{WORKFLOW}", "--format", "json"],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        raise Unreadable(f"gh cannot run: {error}") from error
    if done.returncode != 0:
        return []
    try:
        result = json.loads(done.stdout)
    except json.JSONDecodeError:
        return []
    return result if isinstance(result, list) else []


def _invocation_runs(node: object) -> set[int]:
    """Every run number a `runInvocationURI` in an attestation result names."""
    found: set[int] = set()
    if isinstance(node, dict):
        for key, value in node.items():
            if key == "runInvocationURI" and isinstance(value, str):
                match = re.search(r"/actions/runs/(\d+)", value)
                if match:
                    found.add(int(match.group(1)))
            else:
                found |= _invocation_runs(value)
    elif isinstance(node, list):
        for item in node:
            found |= _invocation_runs(item)
    return found


Attest = Callable[[pathlib.Path, str], list]


def verify(directory: pathlib.Path, repo: str, run_id: int, tree: str, attest: Attest = gh_attestation) -> dict:
    """The download in `directory` (`canon-binary-<target>/`, `man-pages/`, `manifest/build-manifest.json`) against its manifest."""
    manifest_path = directory / "manifest" / MANIFEST_FILE
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return reject("incomplete", "the manifest is missing or unreadable")
    if not isinstance(manifest, dict) or manifest.get("schema") != SCHEMA:
        return reject("incomplete", "the manifest has no known schema")
    if manifest.get("tree") != tree:
        return reject("tree_mismatch", f"the manifest names tree {manifest.get('tree')}, not {tree}")
    binaries = manifest.get("binaries")
    pages = manifest.get("man")
    if not isinstance(binaries, dict) or not isinstance(pages, dict) or not pages:
        return reject("incomplete", "the manifest lacks its binaries or man pages")
    for target, name in TARGETS.items():
        if target not in binaries:
            return reject("incomplete", f"the manifest has no {target}")
        binary = directory / f"canon-binary-{target}" / name
        if not binary.is_file():
            return reject("incomplete", f"no downloaded binary for {target}")
        pair = pair_digest(binary.with_name(name + ".sha256"))
        if pair is None:
            return reject("incomplete", f"no .sha256 pair for {target}")
        actual = sha256_of(binary)
        if not (actual == pair == str(binaries[target]).lower()):
            return reject("checksum_mismatch", f"{target}: the binary, its .sha256 pair and the manifest do not agree")
    man_dir = directory / MAN_ARTIFACT
    have = man_files(man_dir) if man_dir.is_dir() else {}
    if set(have) != set(pages):
        return reject("incomplete", "the downloaded man pages are not the manifest's list")
    if any(have[name] != str(pages[name]).lower() for name in have):
        return reject("checksum_mismatch", "a man page differs from the manifest")
    attestations = attest(manifest_path, repo)
    if run_id not in _invocation_runs(attestations):
        return reject("attestation_invalid", f"no attestation of this manifest from build-native run {run_id}")
    return {"verdict": "pass", "tree": tree, "run_id": run_id}


# ---------------------------------------------------------------- command line


def emit(answer: dict) -> int:
    print(json.dumps(answer, sort_keys=True))
    if answer["verdict"] == "pass":
        return 0
    hint = PRESCRIPTION.get(answer["verdict"])
    if hint:
        print(f"release-build: {answer['verdict']}: {answer.get('reason', '')}\n  fix: {hint}", file=sys.stderr)
    return 1


def append_env_file(variable: str, text: str) -> None:
    path = os.environ.get(variable)
    if path:
        with open(path, "a", encoding="utf-8") as handle:
            handle.write(text)


def git_shas() -> list[str]:
    """The checked-out commit and its parents (the merge commit of a release is built from its parents' runs)."""
    try:
        done = subprocess.run(["git", "rev-list", "--parents", "-n", "1", "HEAD"], capture_output=True, text=True, check=True)
    except (OSError, subprocess.CalledProcessError) as error:
        raise Unreadable(f"git cannot list the checked-out commit: {error}") from error
    return done.stdout.split()


def git_tree() -> str:
    try:
        return subprocess.run(["git", "rev-parse", "HEAD^{tree}"], capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise Unreadable(f"git cannot read the tree: {error}") from error


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Record, find and verify the manifest of a build-native run.")
    commands = parser.add_subparsers(dest="command", required=True)

    rec = commands.add_parser("record", help="write the manifest of the downloaded artifacts")
    rec.add_argument("--tree", required=True)
    rec.add_argument("--commit", required=True)
    rec.add_argument("--event", required=True)
    rec.add_argument("--run-id", type=int, required=True)
    rec.add_argument("--run-attempt", type=int, required=True)
    rec.add_argument("--artifacts", type=pathlib.Path, required=True, help="the directory holding canon-binary-<target>/")
    rec.add_argument("--man", type=pathlib.Path, required=True, help="the man pages directory")
    rec.add_argument("--out", type=pathlib.Path, required=True)

    fnd = commands.add_parser("find", help="the build-native run that built this tree")
    fnd.add_argument("--repo", required=True)
    fnd.add_argument("--tree", help="default: HEAD^{tree} of the checkout")
    fnd.add_argument("--sha", action="append", default=[], help="a commit whose runs to look for (default: HEAD and its parents)")

    ver = commands.add_parser("verify", help="match the download to its manifest and attestation")
    ver.add_argument("--repo", required=True)
    ver.add_argument("--run-id", type=int, required=True)
    ver.add_argument("--dir", type=pathlib.Path, required=True)
    ver.add_argument("--tree", help="default: HEAD^{tree} of the checkout")

    args = parser.parse_args(argv)
    try:
        if args.command == "record":
            answer = record(args.tree, args.commit, args.event, args.run_id, args.run_attempt, args.artifacts, args.man)
            if answer["verdict"] != "pass":
                return emit(answer)
            args.out.write_text(json.dumps(answer["manifest"], indent=2, sort_keys=True) + "\n", encoding="utf-8")
            return emit({"verdict": "pass"})
        if args.command == "find":
            tree = args.tree or git_tree()
            answer = find(gh_api, args.repo, tree, args.sha or git_shas())
            if answer["verdict"] == "pass":
                append_env_file("GITHUB_OUTPUT", f"run_id={answer['run_id']}\ntree={tree}\n")
                append_env_file("GITHUB_STEP_SUMMARY", f"Binaries come from build-native run {answer['run_id']} (commit {answer['commit']}, tree {tree}).\n")
            return emit(answer)
        return emit(verify(args.dir, args.repo, args.run_id, args.tree or git_tree()))
    except Unreadable as error:
        print(json.dumps({"verdict": "unreadable", "reason": str(error)}))
        return 2


if __name__ == "__main__":
    sys.exit(main())
