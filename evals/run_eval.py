#!/usr/bin/env python3
"""Run or grade Canon maintenance evals without claiming unobserved quality.

The runner is dependency-free and treats the corpus, evaluator probes, task
contract, evidence bindings, and resource limits as evaluator-owned state.
"""
from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import pathlib
import selectors
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_CORPUS = pathlib.Path(__file__).with_name("tasks.json")
CORPUS_DIGEST_FILE = pathlib.Path(__file__).with_name("corpus.sha256")
PROBE_DIR = pathlib.Path(__file__).with_name("probes")
GRADER_PROBE = PROBE_DIR / "grader.py"
GATE_PROBE = PROBE_DIR / "gate.py"
# These values are intentionally pinned. A workspace cannot replace the evaluator
# probe without changing the runner itself (which is outside the submitted tree).
TRUSTED_PROBE_SHA256: dict[str, str] = {
    "grader.py": "520fce91e63058fb5c1a8643d27bba8768a28c285640be50871294585c68d89e",
    "gate.py": "a507e7fb6ee58a203bf101964b9c0f2b0db0412856b71e1cc48b92ad3be20952",
}
SCHEMA_VERSION = 2
PINNED_TASK_COUNT = 40


class EvalError(Exception):
    pass


def canonical(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def bytes_digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def load_json(path: pathlib.Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise EvalError(f"cannot read JSON {path}: {exc}") from exc


def command_text(command: Any) -> str:
    if isinstance(command, list):
        return " ".join(shlex.quote(str(x)) for x in command)
    return str(command)


def as_command(command: Any) -> list[str]:
    if isinstance(command, list) and command and all(isinstance(x, str) for x in command):
        return command
    if isinstance(command, str) and command.strip():
        return shlex.split(command)
    raise EvalError("commands must be a non-empty argv list or shell-free string")


def command_was_exercised(result: dict[str, Any]) -> bool:
    import re
    command = result.get("command", [])
    if command and command[0] == "cargo" and "test" in command:
        return bool(re.search(r"running [1-9][0-9]* tests?", result.get("stdout", "")))
    return result.get("exit_code") is not None and not result.get("timed_out", False) and not result.get("output_limited", False)


def remaining(deadline: float | None) -> float:
    if deadline is None:
        return 10**9
    return max(0.001, deadline - time.monotonic())


def _kill_group(proc: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except (OSError, ProcessLookupError):
        try:
            proc.kill()
        except OSError:
            pass


def run_command(command: Any, cwd: pathlib.Path, timeout: float, max_output: int, env: dict[str, str], deadline: float | None = None) -> dict[str, Any]:
    argv = as_command(command)
    end = min(time.monotonic() + max(timeout, 0.001), deadline) if deadline is not None else time.monotonic() + max(timeout, 0.001)
    started = time.monotonic()
    proc: subprocess.Popen[bytes] | None = None
    streams: dict[int, bytearray] = {1: bytearray(), 2: bytearray()}
    output_limited = False
    timed_out = False
    try:
        proc = subprocess.Popen(argv, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        assert proc.stdout is not None and proc.stderr is not None
        selector = selectors.DefaultSelector()
        selector.register(proc.stdout, selectors.EVENT_READ, 1)
        selector.register(proc.stderr, selectors.EVENT_READ, 2)
        while selector.get_map():
            wait_for = max(0.001, min(0.1, end - time.monotonic()))
            if wait_for <= 0:
                timed_out = True
                _kill_group(proc)
            for key, _ in selector.select(wait_for):
                chunk = key.fileobj.read(65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                stream = streams[key.data]
                if len(stream) + len(chunk) > max_output:
                    stream.extend(chunk[: max_output - len(stream)])
                    output_limited = True
                    _kill_group(proc)
                else:
                    stream.extend(chunk)
            if proc.poll() is not None and not selector.get_map():
                break
            if time.monotonic() >= end and proc.poll() is None:
                timed_out = True
                _kill_group(proc)
        try:
            proc.wait(timeout=0.2)
        except subprocess.TimeoutExpired:
            _kill_group(proc)
            proc.wait(timeout=0.2)
        returncode = None if timed_out else proc.returncode
    except (OSError, ValueError) as exc:
        returncode = None
        streams[2].extend(f"{type(exc).__name__}: {exc}".encode())
    finally:
        if proc is not None and proc.poll() is None:
            _kill_group(proc)
            try:
                proc.wait(timeout=0.2)
            except subprocess.TimeoutExpired:
                pass
    return {
        "command": argv,
        "command_text": command_text(argv),
        "exit_code": returncode,
        "timed_out": timed_out,
        "output_limited": output_limited,
        "latency_ms": round((time.monotonic() - started) * 1000, 3),
        "stdout": bytes(streams[1]).decode(errors="replace"),
        "stderr": bytes(streams[2]).decode(errors="replace"),
    }


def git(cwd: pathlib.Path, *args: str) -> tuple[int, str, str]:
    try:
        proc = subprocess.run(["git", *args], cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    except OSError as exc:
        return (None, "", f"{type(exc).__name__}: {exc}")  # type: ignore[return-value]
    return proc.returncode, proc.stdout, proc.stderr


def commit_at(workspace: pathlib.Path, ref: str = "HEAD") -> str:
    code, out, err = git(workspace, "rev-parse", "--verify", f"{ref}^{{commit}}")
    if code != 0:
        raise EvalError(f"commit is not resolvable: {err.strip()}")
    return out.strip()


def changed_paths(workspace: pathlib.Path, base_commit: str = "HEAD", forbidden: list[str] | None = None) -> list[str]:
    code, out, err = git(workspace, "diff", "--name-only", base_commit, "--")
    if code != 0:
        raise EvalError(f"git diff failed: {err.strip()}")
    paths = {line.strip() for line in out.splitlines() if line.strip()}
    code, out, err = git(workspace, "ls-files", "--others", "--exclude-standard")
    if code != 0:
        raise EvalError(f"git ls-files failed: {err.strip()}")
    paths.update(line.strip() for line in out.splitlines() if line.strip())
    # .canon/eval is evaluator-owned setup, not a candidate edit.
    paths = {path for path in paths if not (path == ".canon/eval" or path.startswith(".canon/eval/"))}
    if forbidden:
        code, out, err = git(workspace, "ls-files", "--others")
        if code != 0:
            raise EvalError(f"git ls-files (ignored) failed: {err.strip()}")
        paths.update(line.strip() for line in out.splitlines() if line.strip() and not line.startswith("target/") and matches(line.strip(), forbidden))
    return sorted(paths)


def matches(path: str, patterns: list[str]) -> bool:
    return any(fnmatch.fnmatch(path, pattern) or pathlib.PurePosixPath(path).match(pattern) for pattern in patterns)


def corpus_digest(path: pathlib.Path) -> str:
    try:
        return bytes_digest(path.read_bytes())
    except OSError as exc:
        raise EvalError(f"cannot read corpus {path}: {exc}") from exc


def _read_pinned_digest() -> str:
    try:
        value = CORPUS_DIGEST_FILE.read_text(encoding="ascii").strip()
    except OSError as exc:
        raise EvalError(f"cannot read pinned corpus digest: {exc}") from exc
    if len(value) != 64 or any(char not in "0123456789abcdef" for char in value):
        raise EvalError("pinned corpus digest is invalid")
    return value


def load_tasks(path: pathlib.Path, allow_unpinned: bool = False) -> dict[str, dict[str, Any]]:
    actual_digest = corpus_digest(path)
    pinned = _read_pinned_digest()
    if path.resolve() == DEFAULT_CORPUS.resolve() and actual_digest != pinned:
        raise EvalError("repository corpus does not match its pinned SHA-256")
    if path.resolve() != DEFAULT_CORPUS.resolve() and not allow_unpinned:
        raise EvalError("custom corpus requires --allow-unpinned-corpus")
    corpus = load_json(path)
    if corpus.get("schema_version") != 1 or not isinstance(corpus.get("tasks"), list):
        raise EvalError("corpus schema_version/tasks are invalid")
    tasks: dict[str, dict[str, Any]] = {}
    diff_shapes: set[tuple[str, ...]] = set()
    for task in corpus["tasks"]:
        task_id = task.get("id")
        if not isinstance(task_id, str) or not task_id or task_id in tasks:
            raise EvalError(f"invalid or duplicate task id: {task_id!r}")
        for field in ("instructions", "setup_seed", "expected_contract", "allowed_diff_paths", "forbidden_side_effects", "grader", "required_evidence", "gate"):
            if field not in task:
                raise EvalError(f"task {task_id} missing {field}")
        grader_command = as_command(task["grader"].get("command"))
        if "/" in grader_command[0] or "\\" in grader_command[0]:
            raise EvalError(f"task {task_id} grader executable must not be workspace path")
        shape = tuple(task["allowed_diff_paths"])
        if shape in diff_shapes:
            raise EvalError(f"duplicate allowed diff shape for task {task_id}")
        diff_shapes.add(shape)
        tasks[task_id] = task
    if path.resolve() == DEFAULT_CORPUS.resolve() and len(tasks) != PINNED_TASK_COUNT:
        raise EvalError(f"pinned corpus has {len(tasks)} tasks; expected {PINNED_TASK_COUNT}")
    if len(tasks) < 1:
        raise EvalError("corpus has no tasks")
    return tasks


def _safe_seed_path(relative: str) -> pathlib.PurePosixPath:
    path = pathlib.PurePosixPath(relative)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        raise EvalError(f"unsafe setup seed path: {relative}")
    return path


def prepare_contract(task: dict[str, Any], workspace: pathlib.Path, corpus_hash: str, base_commit: str) -> tuple[pathlib.Path, dict[str, str]]:
    canon_root = workspace / ".canon"
    root = canon_root / "eval"
    seed_root = root / "seed"
    if canon_root.is_symlink() or root.is_symlink() or seed_root.is_symlink():
        raise EvalError("evaluator contract directory cannot be a symlink")
    seed_root.mkdir(parents=True, exist_ok=True)
    seed_root_resolved = seed_root.resolve()
    seed_hashes: dict[str, str] = {}
    for item in task.get("setup_seed", {}).get("files", []):
        relative = _safe_seed_path(str(item["path"]))
        target = seed_root / pathlib.Path(relative)
        if target.is_symlink() or not target.resolve().is_relative_to(seed_root_resolved):
            raise EvalError(f"unsafe setup seed target: {relative}")
        target.parent.mkdir(parents=True, exist_ok=True)
        content = str(item.get("content", "")).encode()
        target.write_bytes(content)
        target.chmod(0o444)
        seed_hashes[str(relative)] = bytes_digest(content)
    contract = {"schema_version": SCHEMA_VERSION, "task_id": task["id"], "instructions": task["instructions"], "setup_seed": {"files": [{"path": key, "sha256": value} for key, value in sorted(seed_hashes.items())]}, "corpus_digest": corpus_hash, "task_digest": digest(task), "base_commit": base_commit}
    path = root / "task.json"
    if path.is_symlink():
        raise EvalError("authoritative task contract cannot be a symlink")
    path.write_text(json.dumps(contract, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    path.chmod(0o444)
    root.chmod(0o755)
    return path, seed_hashes


def verify_contract(task: dict[str, Any], workspace: pathlib.Path, contract_path: pathlib.Path, seed_hashes: dict[str, str], corpus_hash: str, base_commit: str) -> None:
    try:
        raw = contract_path.read_bytes()
    except OSError as exc:
        raise EvalError(f"authoritative task contract missing: {exc}") from exc
    contract = load_json(contract_path)
    if contract_path.stat().st_mode & 0o222:
        raise EvalError("authoritative task contract is writable")
    expected = {"schema_version": SCHEMA_VERSION, "task_id": task["id"], "instructions": task["instructions"], "setup_seed": {"files": [{"path": key, "sha256": value} for key, value in sorted(seed_hashes.items())]}, "corpus_digest": corpus_hash, "task_digest": digest(task), "base_commit": base_commit}
    if contract != expected or bytes_digest(raw) != bytes_digest((json.dumps(expected, indent=2, sort_keys=True) + "\n").encode()):
        raise EvalError("authoritative task contract was modified")
    for relative, expected_hash in seed_hashes.items():
        target = workspace / ".canon" / "eval" / "seed" / pathlib.Path(relative)
        if not target.is_file() or target.is_symlink() or target.stat().st_mode & 0o222 or bytes_digest(target.read_bytes()) != expected_hash:
            raise EvalError(f"setup seed was modified: {relative}")
    seed_root = workspace / ".canon" / "eval" / "seed"
    observed = {str(path.relative_to(seed_root)) for path in seed_root.rglob("*") if path.is_file()}
    if observed != set(seed_hashes):
        raise EvalError("setup seed contents were modified")


def _trusted_probe(path: pathlib.Path) -> None:
    expected = TRUSTED_PROBE_SHA256.get(path.name)
    if not expected:
        raise EvalError(f"evaluator probe is not pinned: {path}")
    try:
        actual = bytes_digest(path.read_bytes())
    except OSError as exc:
        raise EvalError(f"cannot read evaluator probe: {exc}") from exc
    if actual != expected:
        raise EvalError(f"evaluator probe digest mismatch: {path.name}")


def run_probe(path: pathlib.Path, args: list[str], cwd: pathlib.Path, timeout: float, max_output: int, env: dict[str, str], deadline: float | None) -> dict[str, Any]:
    _trusted_probe(path)
    return run_command([sys.executable, str(path), *args], cwd, timeout, max_output, env, deadline)

def _validate_grader_command(command: list[str], workspace: pathlib.Path) -> None:
    workspace_root = workspace.resolve()
    for token in command[1:]:
        if token.startswith("-"):
            continue
        candidate = (workspace / token).resolve()
        if candidate.is_file() and candidate.is_relative_to(workspace_root):
            raise EvalError("grader command references a workspace-modifiable file")


def run_authoritative(command: Any, task_id: str, workspace: pathlib.Path, timeout: float, max_output: int, env: dict[str, str], deadline: float | None = None) -> dict[str, Any]:
    command_argv = as_command(command)
    _validate_grader_command(command_argv, workspace)
    raw = json.dumps(command_argv, separators=(",", ":"))
    outer = run_probe(
        GRADER_PROBE,
        [
            "--task-id",
            task_id,
            "--workspace",
            str(workspace),
            "--command-json",
            raw,
            "--timeout",
            str(remaining(deadline)),
            "--max-output",
            str(max_output),
        ],
        workspace,
        timeout,
        max_output * 2 + 8192,
        env,
        deadline,
    )
    fallback = {
        "command": command_argv,
        "command_text": command_text(command_argv),
        "exit_code": None,
        "timed_out": bool(outer.get("timed_out", False)),
        "output_limited": bool(outer.get("output_limited", False)),
        "latency_ms": outer.get("latency_ms", 0),
        "stdout": outer.get("stdout", ""),
        "stderr": outer.get("stderr", ""),
    }
    if outer.get("exit_code") != 0 or fallback["timed_out"] or fallback["output_limited"]:
        return fallback
    try:
        value = json.loads(outer["stdout"])
        if not isinstance(value, dict) or value.get("protocol") != "canon-eval-grader-v1":
            raise ValueError("invalid probe response")
        if value.get("task_id", task_id) != task_id:
            raise ValueError("probe task id mismatch")
        if value.get("command") != command_argv:
            raise ValueError("probe command mismatch")
        required = ("exit_code", "timed_out", "output_limited", "stdout", "stderr")
        if any(key not in value for key in required):
            raise ValueError("incomplete probe response")
        value.pop("protocol", None)
        value["latency_ms"] = outer["latency_ms"]
        return value
    except (KeyError, TypeError, ValueError, json.JSONDecodeError) as exc:
        fallback["stderr"] = f"invalid evaluator response: {exc}"
        return fallback


def _validate_bound_evidence(value: Any, task: dict[str, Any], evidence: dict[str, Any], corpus_hash: str, base_commit: str, current_commit: str, command_hash: str, raw_hash: str) -> bool:
    if not isinstance(value, dict):
        return False
    expected = {"task_id": task["id"], "corpus_digest": corpus_hash, "task_digest": digest(task), "base_commit": base_commit, "current_commit": current_commit, "command_result_sha256": command_hash}
    return all(value.get(key) == expected_value for key, expected_value in expected.items()) and value.get("evidence_sha256") == raw_hash and isinstance(value.get("provenance"), dict) and value["provenance"].get("bound_by") == "canon-eval-runner"


def check_contract(task: dict[str, Any], workspace: pathlib.Path, changed: list[str], timeout: float, max_output: int, env: dict[str, str], reused_command: dict[str, Any] | None = None, *, corpus_hash: str = "", base_commit: str = "", current_commit: str = "", deadline: float | None = None) -> tuple[list[dict[str, Any]], list[dict[str, Any]], bool]:
    contract = task["expected_contract"]
    checks: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    allowed = list(task["allowed_diff_paths"]) + [".canon/evidence/**"]
    forbidden = task.get("forbidden_side_effects", {}).get("paths", [])
    bad_allowed = [path for path in changed if not matches(path, allowed)]
    bad_forbidden = [path for path in changed if matches(path, forbidden)]
    for name, passed, details in (("allowed_diff_paths", not bad_allowed, bad_allowed), ("forbidden_side_effects", not bad_forbidden, bad_forbidden)):
        check = {"name": name, "passed": passed, "details": details}
        checks.append(check)
        if not passed:
            failures.append(check)
    for item in contract.get("checks", []):
        name = item.get("name", item.get("path", item.get("contains", "contract-check")))
        passed = False
        details: Any = None
        kind = item["kind"]
        if kind == "path_exists":
            target = workspace / item["path"]
            passed, details = target.is_file() or target.is_dir(), item["path"]
        elif kind == "text_contains":
            target = workspace / item["path"]
            try:
                text = target.read_text(encoding="utf-8")
                passed, details = all(token in text for token in item["contains"]), {"path": item["path"], "contains": item["contains"]}
            except OSError as exc:
                details = f"{type(exc).__name__}: {exc}"
        elif kind == "json_fields":
            try:
                value = load_json(workspace / item["path"])
                passed, details = all(value.get(key) == expected for key, expected in item["equals"].items()), {"path": item["path"], "equals": item["equals"]}
            except (EvalError, AttributeError) as exc:
                details = str(exc)
        elif kind == "command":
            argv = as_command(item["command"])
            result = reused_command if reused_command is not None and reused_command.get("command") == argv else run_authoritative(argv, task["id"], workspace, remaining(deadline), max_output, env, deadline)
            passed = result["exit_code"] == item.get("exit_code", 0) and command_was_exercised(result)
            details = result
        else:
            details = f"unsupported check kind: {kind}"
        check = {"name": name, "kind": kind, "passed": passed, "details": details}
        checks.append(check)
        if not passed:
            failures.append(check)
    command_hash = digest(reused_command or {})
    for evidence in task.get("required_evidence", []):
        path = workspace / evidence["path"]
        valid = False
        details: Any = evidence["path"]
        if path.is_file() and evidence.get("format") == "json":
            try:
                raw = path.read_bytes()
                value = load_json(path)
                required_keys = evidence.get("required_keys", [])
                valid = isinstance(value, dict) and all(key in value for key in required_keys)
                if valid:
                    bound = dict(value)
                    source_hash = bytes_digest(raw)
                    bound.update({"task_id": task["id"], "corpus_digest": corpus_hash, "task_digest": digest(task), "base_commit": base_commit, "current_commit": current_commit, "command_result_sha256": command_hash, "evidence_sha256": source_hash, "provenance": {"bound_by": "canon-eval-runner", "source_sha256": source_hash}})
                    if path.is_symlink() or not path.resolve().is_relative_to(workspace.resolve()):
                        raise EvalError("evidence path is outside workspace")
                    try:
                        path.chmod(0o644)
                        path.write_text(json.dumps(bound, indent=2, sort_keys=True) + "\n", encoding="utf-8")
                    except OSError as exc:
                        raise EvalError(f"cannot write runner evidence binding: {exc}") from exc
                details = {"path": evidence["path"], "bound": valid, "command_result_sha256": command_hash}
            except EvalError as exc:
                details = str(exc)
            except OSError as exc:
                details = f"{type(exc).__name__}: {exc}"
        check = {"name": f"evidence:{evidence['path']}", "kind": "evidence", "passed": valid, "details": details}
        checks.append(check)
        if not valid:
            failures.append(check)
    return checks, failures, not failures


def finalize(payload: dict[str, Any]) -> dict[str, Any]:
    unsigned = dict(payload)
    unsigned["integrity"] = {"schema_version": SCHEMA_VERSION}
    unsigned["integrity"]["sha256"] = digest(unsigned)
    return unsigned


def verify_payload(payload: dict[str, Any]) -> None:
    integrity = payload.get("integrity", {})
    if integrity.get("schema_version") != SCHEMA_VERSION:
        raise EvalError("result has unsupported integrity schema")
    supplied = integrity.get("sha256")
    if not supplied:
        raise EvalError("result is missing integrity.sha256")
    unsigned = dict(payload)
    unsigned["integrity"] = {"schema_version": integrity.get("schema_version", SCHEMA_VERSION)}
    if supplied != digest(unsigned):
        raise EvalError("tampered result: integrity.sha256 does not match canonical payload")


def _gate(task: dict[str, Any], workspace: pathlib.Path, checks: list[dict[str, Any]], corpus_hash: str, base_commit: str, current_commit: str, command_hash: str, deadline: float | None, max_output: int, env: dict[str, str]) -> dict[str, Any]:
    input_path = workspace / ".canon" / "eval" / "gate-input.json"
    if input_path.is_symlink():
        raise EvalError("evaluator gate input cannot be a symlink")
    input_path.write_text(json.dumps({"task_id": task["id"], "corpus_digest": corpus_hash, "task_digest": digest(task), "base_commit": base_commit, "current_commit": current_commit, "command_result_sha256": command_hash, "checks": checks}, sort_keys=True) + "\n", encoding="utf-8")
    response = run_probe(GATE_PROBE, ["--input", str(input_path)], workspace, remaining(deadline), max_output, env, deadline)
    if response.get("exit_code") != 0 or response.get("timed_out") or response.get("output_limited"):
        raise EvalError("independent evaluator gate command failed")
    try:
        gate = json.loads(response["stdout"])
    except json.JSONDecodeError as exc:
        raise EvalError(f"invalid evaluator gate response: {exc}") from exc
    gate["generated_by"] = "canon-eval-runner"
    gate["task_id"] = task["id"]
    gate["corpus_digest"] = corpus_hash
    gate["base_commit"] = base_commit
    gate["current_commit"] = current_commit
    gate["command_result_sha256"] = command_hash
    gate_path = task.get("gate", {}).get("evidence_path")
    if not isinstance(gate_path, str) or pathlib.PurePosixPath(gate_path).is_absolute() or ".." in pathlib.PurePosixPath(gate_path).parts:
        raise EvalError("task gate path is unsafe")
    destination = workspace / gate_path
    if destination.is_symlink() or not destination.resolve().is_relative_to(workspace.resolve()):
        raise EvalError("runner gate path is outside workspace")
    try:
        destination.parent.mkdir(parents=True, exist_ok=True)
        parent = destination.parent
        ancestor = workspace
        for part in parent.relative_to(workspace).parts:
            ancestor = ancestor / part
            if ancestor.is_symlink() or not ancestor.is_dir():
                raise EvalError("runner gate parent path is unsafe")
        temporary = tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=parent,
            prefix=f".{destination.name}.",
            suffix=".tmp",
            delete=False,
        )
        temporary_path = pathlib.Path(temporary.name)
        try:
            with temporary:
                temporary.write(json.dumps(gate, indent=2, sort_keys=True) + "\n")
                temporary.flush()
                os.fsync(temporary.fileno())
            os.replace(temporary_path, destination)
            destination.chmod(0o644)
        finally:
            temporary_path.unlink(missing_ok=True)
    except OSError as exc:
        raise EvalError(f"cannot write runner gate: {exc}") from exc
    return gate


def grade(task: dict[str, Any], workspace: pathlib.Path, agent_run: dict[str, Any] | None, task_seed: str, timeout: float, max_output: int, extra_metrics: dict[str, Any] | None = None, base_commit: str = "HEAD", *, corpus_hash: str = "", corpus_path: str | None = None, deadline: float | None = None) -> dict[str, Any]:
    env = os.environ.copy()
    seed_path = pathlib.Path(task_seed).resolve()
    eval_root = workspace / ".canon" / "eval"
    eval_root.mkdir(parents=True, exist_ok=True)
    env.update({"CANON_EVAL_TASK_ID": task["id"], "CANON_EVAL_TASK_FILE": str(eval_root / "task.json"), "CANON_EVAL_SEED_DIR": str(seed_path)})
    test_run = run_authoritative(task["grader"]["command"], task["id"], workspace, remaining(deadline), max_output, env, deadline)
    current_commit = commit_at(workspace)
    changed = changed_paths(workspace, base_commit, task.get("forbidden_side_effects", {}).get("paths", []))
    command_hash = digest(test_run)
    checks, failures, contract_passed = check_contract(task, workspace, changed, remaining(deadline), max_output, env, test_run, corpus_hash=corpus_hash, base_commit=base_commit, current_commit=current_commit, deadline=deadline)
    semantic = extra_metrics.pop("_semantic", None) if extra_metrics else None
    semantic_required = task.get("semantic_grader", {}).get("required", False)
    if semantic_required and (semantic is None or semantic.get("decision") != "pass"):
        failure = {"name": "semantic_judgment", "passed": False, "details": "explicit independent judged evidence missing or failed"}
        checks.append(failure)
        failures.append(failure)
    objective_passed = test_run["exit_code"] == task["grader"].get("exit_code", 0) and command_was_exercised(test_run) and contract_passed and (not semantic_required or semantic is not None and semantic["decision"] == "pass")
    claimed = agent_run is not None and agent_run.get("exit_code") == 0 and not agent_run.get("timed_out", False)
    gate = _gate(task, workspace, checks, corpus_hash, base_commit, current_commit, command_hash, deadline, max_output, env)
    gate_passed = gate.get("status") == "pass" and gate.get("task_id") == task["id"] and gate.get("command_result_sha256") == command_hash
    if not gate_passed:
        gate_metadata = {key: value for key, value in gate.items() if key != "failures"}
        failures.append({"name": "final_gate", "passed": False, "details": gate_metadata})
    gate["failures"] = failures
    gate_path = workspace / task["gate"]["evidence_path"]
    gate_path.write_text(json.dumps(gate, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    metrics = {key: value for key, value in (extra_metrics or {}).items() if key != "_semantic"}
    for key in ("latency_ms", "cost_usd", "input_tokens", "output_tokens"):
        metrics.setdefault(key, None)
    overall_passed = objective_passed and gate_passed
    payload = {
        "schema_version": SCHEMA_VERSION,
        "task_id": task["id"],
        "changed_paths": changed,
        "passed": overall_passed,
        "provenance": {"corpus": corpus_path or str(DEFAULT_CORPUS), "corpus_digest": corpus_hash, "task_digest": digest(task), "workspace": str(workspace), "base_commit": base_commit, "current_commit": current_commit, "agent_command": agent_run.get("command") if agent_run else None},
        "agent": agent_run,
        "diff": {"changed_paths": changed, "allowed": task["allowed_diff_paths"], "forbidden": task.get("forbidden_side_effects", {}).get("paths", [])},
        "tests": test_run,
        "evidence": {"required": [item["path"] for item in task["required_evidence"]], "present": [item["path"] for item in task["required_evidence"] if (workspace / item["path"]).is_file()], "checks": checks},
        "gate": gate,
        "semantic": semantic,
        "outcome": {"passed": overall_passed, "agent_claimed_success": claimed, "false_pass": bool(claimed and not overall_passed), "regression": None},
        "metrics": metrics,
        "integrity": {"schema_version": SCHEMA_VERSION},
    }
    return finalize(payload)


def _result_metadata(item: dict[str, Any], tasks: dict[str, dict[str, Any]], corpus_hash: str) -> tuple[str, str, str]:
    task_id = item.get("task_id")
    if task_id not in tasks:
        raise EvalError(f"scoreboard contains unknown task {task_id!r}")
    provenance = item.get("provenance", {})
    if provenance.get("corpus_digest") != corpus_hash or provenance.get("task_digest") != digest(tasks[task_id]):
        raise EvalError(f"result provenance mismatch for {task_id}")
    base = provenance.get("base_commit")
    current = provenance.get("current_commit")
    if not isinstance(base, str) or not base or not isinstance(current, str) or not current:
        raise EvalError(f"result base/current provenance missing for {task_id}")
    return task_id, base, current


def scoreboard(baseline: list[dict[str, Any]], candidate: list[dict[str, Any]], tasks: dict[str, dict[str, Any]] | None = None, corpus_hash: str | None = None) -> dict[str, Any]:
    if tasks is None:
        raise EvalError("scoreboard requires the pinned corpus")
    if corpus_hash is None:
        raise EvalError("scoreboard requires corpus digest")
    base_ids = [item.get("task_id") for item in baseline]
    cand_ids = [item.get("task_id") for item in candidate]
    if len(set(base_ids)) != len(base_ids) or len(set(cand_ids)) != len(cand_ids):
        raise EvalError("scoreboard rejects duplicate task IDs")
    if set(base_ids) != set(cand_ids) or set(base_ids) != set(tasks):
        raise EvalError("scoreboard task sets must equal the pinned corpus")
    base_meta = [_result_metadata(item, tasks, corpus_hash) for item in baseline]
    cand_meta = [_result_metadata(item, tasks, corpus_hash) for item in candidate]
    if len({item[1] for item in base_meta}) != 1 or len({item[1] for item in cand_meta}) != 1:
        raise EvalError("scoreboard base commit provenance is inconsistent")
    base = {item["task_id"]: item for item in baseline}
    cand = {item["task_id"]: item for item in candidate}
    rows = []
    for task_id in sorted(tasks):
        left, right = base[task_id], cand[task_id]
        baseline_passed = bool(left.get("outcome", {}).get("passed"))
        candidate_passed = bool(right.get("outcome", {}).get("passed"))
        rows.append({"task_id": task_id, "baseline_passed": baseline_passed, "candidate_passed": candidate_passed, "regression": baseline_passed and not candidate_passed, "baseline_result_sha256": left["integrity"]["sha256"], "candidate_result_sha256": right["integrity"]["sha256"]})
    def aggregate(items: list[dict[str, Any]]) -> dict[str, Any]:
        known = [item for item in items if isinstance(item.get("outcome", {}).get("passed"), bool)]
        return {"tasks": len(items), "success_at_1": (sum(item["outcome"]["passed"] for item in known) / len(known)) if known else None, "false_pass": sum(bool(item.get("outcome", {}).get("false_pass")) for item in items), "latency_ms": [item["metrics"]["latency_ms"] for item in items if item.get("metrics", {}).get("latency_ms") is not None] or None, "cost_usd": [item["metrics"]["cost_usd"] for item in items if item.get("metrics", {}).get("cost_usd") is not None] or None}
    return {"schema_version": SCHEMA_VERSION, "provenance": {"corpus_digest": corpus_hash, "baseline_base_commits": sorted({item[1] for item in base_meta}), "candidate_base_commits": sorted({item[1] for item in cand_meta}), "baseline_result_digests": [item["integrity"]["sha256"] for item in baseline], "candidate_result_digests": [item["integrity"]["sha256"] for item in candidate]}, "baseline": aggregate(baseline), "candidate": aggregate(candidate), "rows": rows, "regressions": sum(row["regression"] for row in rows), "note": "Metrics remain null when no run supplied them; this scoreboard does not infer quality from agent text."}


def load_judged_evidence(path: pathlib.Path, task_id: str, corpus_hash: str | None = None, base_commit: str | None = None) -> dict[str, Any]:
    value = load_json(path)
    required = ("task_id", "decision", "evidence_refs", "provenance")
    if value.get("task_id") != task_id or value.get("decision") not in {"pass", "fail"} or any(key not in value for key in required) or not isinstance(value["evidence_refs"], list) or not value["provenance"]:
        raise EvalError("judged evidence requires task_id, decision, evidence_refs, and provenance")
    if corpus_hash is not None and value["provenance"].get("corpus_digest") != corpus_hash:
        raise EvalError("judged evidence corpus provenance mismatch")
    if base_commit is not None and value["provenance"].get("base_commit") != base_commit:
        raise EvalError("judged evidence base provenance mismatch")
    return value


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=pathlib.Path, default=DEFAULT_CORPUS)
    parser.add_argument("--allow-unpinned-corpus", action="store_true")
    parser.add_argument("--task-id")
    parser.add_argument("--repo", type=pathlib.Path, default=ROOT)
    parser.add_argument("--base-commit")
    parser.add_argument("--workspace", type=pathlib.Path)
    parser.add_argument("--agent-cmd")
    parser.add_argument("--judged-evidence", type=pathlib.Path)
    parser.add_argument("--cost-usd", type=float)
    parser.add_argument("--input-tokens", type=int)
    parser.add_argument("--output-tokens", type=int)
    parser.add_argument("--timeout", type=float, default=900)
    parser.add_argument("--max-output-bytes", type=int, default=1_000_000)
    parser.add_argument("--result", type=pathlib.Path)
    parser.add_argument("--baseline", type=pathlib.Path, nargs="*")
    parser.add_argument("--candidate", type=pathlib.Path, nargs="*")
    parser.add_argument("--scoreboard", type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        if args.timeout <= 0 or args.max_output_bytes <= 0:
            raise EvalError("timeout and max-output-bytes must be positive")
        tasks = load_tasks(args.corpus, args.allow_unpinned_corpus)
        corpus_hash = corpus_digest(args.corpus)
        if args.baseline is not None or args.candidate is not None:
            if args.baseline is None or args.candidate is None:
                raise EvalError("--baseline and --candidate must be supplied together")
            baseline, candidate = [], []
            for path in args.baseline:
                item = load_json(path); verify_payload(item); baseline.append(item)
            for path in args.candidate:
                item = load_json(path); verify_payload(item); candidate.append(item)
            output = scoreboard(baseline, candidate, tasks, corpus_hash)
            destination = args.scoreboard or pathlib.Path("scoreboard.json")
            destination.write_text(json.dumps(output, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
            print(json.dumps(output, indent=2, ensure_ascii=False)); return 0
        if not args.task_id or args.task_id not in tasks:
            raise EvalError("--task-id must name a corpus task")
        if bool(args.agent_cmd) == bool(args.workspace):
            raise EvalError("choose exactly one of --agent-cmd or --workspace")
        task = tasks[args.task_id]
        deadline = time.monotonic() + args.timeout
        worktree: pathlib.Path | None = None
        agent_run = None
        workspace: pathlib.Path
        if args.workspace:
            workspace = args.workspace.resolve()
            if not args.base_commit:
                raise EvalError("--workspace requires --base-commit")
            base_commit = commit_at(workspace, args.base_commit)
        else:
            source = args.repo.resolve()
            worktree = pathlib.Path(tempfile.mkdtemp(prefix="canon-eval-worktree-")); worktree.rmdir()
            code, _, err = git(source, "worktree", "add", "--detach", str(worktree), "HEAD")
            if code != 0:
                raise EvalError(f"could not create isolated worktree: {err.strip()}")
            workspace = worktree; base_commit = commit_at(workspace)
        contract_path, seed_hashes = prepare_contract(task, workspace, corpus_hash, base_commit)
        env = os.environ.copy(); env.update({"CANON_EVAL_TASK_ID": task["id"], "CANON_EVAL_TASK_FILE": str(contract_path), "CANON_EVAL_SEED_DIR": str(workspace / ".canon" / "eval" / "seed")})
        if args.agent_cmd:
            agent_run = run_command(args.agent_cmd, workspace, remaining(deadline), args.max_output_bytes, env, deadline)
        verify_contract(task, workspace, contract_path, seed_hashes, corpus_hash, base_commit)
        judged = load_judged_evidence(args.judged_evidence, task["id"], corpus_hash, base_commit) if args.judged_evidence else None
        supplied: dict[str, Any] = {"_semantic": judged} if judged else {}
        if agent_run is not None: supplied["latency_ms"] = agent_run["latency_ms"]
        for name in ("cost_usd", "input_tokens", "output_tokens"):
            value = getattr(args, name)
            if value is not None: supplied[name] = value
        result = grade(task, workspace, agent_run, str(workspace / ".canon" / "eval" / "seed"), args.timeout, args.max_output_bytes, supplied, base_commit, corpus_hash=corpus_hash, corpus_path=str(args.corpus), deadline=deadline)
        text = json.dumps(result, indent=2, ensure_ascii=False) + "\n"
        if args.result: args.result.write_text(text, encoding="utf-8")
        print(text, end=""); return 0 if result["outcome"]["passed"] else 1
    except EvalError as exc:
        print(f"eval runner: {exc}", file=sys.stderr); return 2
    finally:
        if 'worktree' in locals() and worktree is not None:
            git(args.repo.resolve(), "worktree", "remove", "--force", str(worktree))
            if worktree.exists(): shutil.rmtree(worktree, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
