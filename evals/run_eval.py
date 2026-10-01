#!/usr/bin/env python3
"""Run or grade Canon maintenance evals without claiming unobserved quality.

The runner is deliberately dependency-free. It creates a detached git worktree for
an explicitly supplied agent command, or grades an explicitly supplied completed
workspace. All outcome fields are derived from git, subprocesses, and evidence
files; agent prose is never treated as proof.
"""
from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import pathlib
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_CORPUS = pathlib.Path(__file__).with_name("tasks.json")
SCHEMA_VERSION = 1


class EvalError(Exception):
    pass


def canonical(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


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
    """Cargo filters that run no tests are not verification."""
    import re
    command = result.get("command", [])
    if command and command[0] == "cargo" and "test" in command:
        return bool(re.search(r"running [1-9][0-9]* tests?", result.get("stdout", "")))
    return result.get("exit_code") is not None and not result.get("timed_out", False)


def run_command(command: Any, cwd: pathlib.Path, timeout: float, max_output: int, env: dict[str, str]) -> dict[str, Any]:
    argv = as_command(command)
    started = time.monotonic()
    try:
        proc = subprocess.run(
            argv,
            cwd=cwd,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
        timed_out = False
        returncode = proc.returncode
        stdout, stderr = proc.stdout, proc.stderr
    except subprocess.TimeoutExpired as exc:
        timed_out = True
        returncode = None
        stdout = exc.stdout or ""
        stderr = exc.stderr or ""
    except (OSError, ValueError) as exc:
        timed_out = False
        returncode = None
        stdout, stderr = "", f"{type(exc).__name__}: {exc}"
    elapsed = round((time.monotonic() - started) * 1000, 3)
    stdout = str(stdout)[-max_output:]
    stderr = str(stderr)[-max_output:]
    return {
        "command": argv,
        "command_text": command_text(argv),
        "exit_code": returncode,
        "timed_out": timed_out,
        "latency_ms": elapsed,
        "stdout": stdout,
        "stderr": stderr,
    }


def git(cwd: pathlib.Path, *args: str) -> tuple[int, str, str]:
    try:
        proc = subprocess.run(["git", *args], cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    except OSError as exc:
        return (None, "", f"{type(exc).__name__}: {exc}")  # type: ignore[return-value]
    return proc.returncode, proc.stdout, proc.stderr


def changed_paths(workspace: pathlib.Path, base_commit: str = "HEAD", forbidden: list[str] | None = None) -> list[str]:
    code, out, err = git(workspace, "diff", "--name-only", base_commit, "--")
    if code != 0:
        raise EvalError(f"git diff failed: {err.strip()}")
    paths = {line.strip() for line in out.splitlines() if line.strip()}
    code, out, err = git(workspace, "ls-files", "--others", "--exclude-standard")
    if code != 0:
        raise EvalError(f"git ls-files failed: {err.strip()}")
    paths.update(line.strip() for line in out.splitlines() if line.strip())
    if forbidden:
        code, out, err = git(workspace, "ls-files", "--others")
        if code != 0:
            raise EvalError(f"git ls-files (ignored) failed: {err.strip()}")
        paths.update(
            line.strip()
            for line in out.splitlines()
            if line.strip() and not line.startswith("target/") and matches(line.strip(), forbidden)
        )
    return sorted(paths)


def matches(path: str, patterns: list[str]) -> bool:
    return any(fnmatch.fnmatch(path, pattern) or pathlib.PurePosixPath(path).match(pattern) for pattern in patterns)


def seed_directory(task: dict[str, Any]) -> tempfile.TemporaryDirectory[str]:
    directory: tempfile.TemporaryDirectory[str] = tempfile.TemporaryDirectory(prefix="canon-eval-seed-")
    root = pathlib.Path(directory.name)
    for item in task.get("setup_seed", {}).get("files", []):
        relative = pathlib.PurePosixPath(item["path"])
        if relative.is_absolute() or ".." in relative.parts:
            directory.cleanup()
            raise EvalError(f"unsafe setup seed path: {relative}")
        target = root / pathlib.Path(relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(str(item.get("content", "")), encoding="utf-8")
    (root / "task.json").write_text(json.dumps(task, indent=2) + "\n", encoding="utf-8")
    return directory


def check_contract(task: dict[str, Any], workspace: pathlib.Path, changed: list[str], timeout: float, max_output: int, env: dict[str, str], reused_command: dict[str, Any] | None = None) -> tuple[list[dict[str, Any]], list[dict[str, Any]], bool]:
    contract = task["expected_contract"]
    checks: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []

    allowed = list(task["allowed_diff_paths"]) + [".canon/evidence/**"]
    forbidden = task.get("forbidden_side_effects", {}).get("paths", [])
    bad_allowed = [path for path in changed if not matches(path, allowed)]
    bad_forbidden = [path for path in changed if matches(path, forbidden)]
    allowed_check = {"name": "allowed_diff_paths", "passed": not bad_allowed, "details": bad_allowed}
    forbidden_check = {"name": "forbidden_side_effects", "passed": not bad_forbidden, "details": bad_forbidden}
    checks.extend([allowed_check, forbidden_check])
    if not allowed_check["passed"]:
        failures.append(allowed_check)
    if not forbidden_check["passed"]:
        failures.append(forbidden_check)

    for item in contract.get("checks", []):
        name = item.get("name", item.get("path", item.get("contains", "contract-check")))
        passed = False
        details: Any = None
        kind = item["kind"]
        if kind == "path_exists":
            target = workspace / item["path"]
            passed = target.is_file() or target.is_dir()
            details = item["path"]
        elif kind == "text_contains":
            target = workspace / item["path"]
            try:
                text = target.read_text(encoding="utf-8")
                passed = all(token in text for token in item["contains"])
                details = {"path": item["path"], "contains": item["contains"]}
            except OSError as exc:
                details = f"{type(exc).__name__}: {exc}"
        elif kind == "json_fields":
            target = workspace / item["path"]
            try:
                value = load_json(target)
                passed = all(value.get(key) == expected for key, expected in item["equals"].items())
                details = {"path": item["path"], "equals": item["equals"]}
            except (EvalError, AttributeError) as exc:
                details = str(exc)
        elif kind == "command":
            argv = as_command(item["command"])
            result = reused_command if reused_command is not None and reused_command.get("command") == argv else run_command(argv, workspace, timeout, max_output, env)
            passed = result["exit_code"] == item.get("exit_code", 0) and command_was_exercised(result)
            details = result
        else:
            details = f"unsupported check kind: {kind}"
        check = {"name": name, "kind": kind, "passed": passed, "details": details}
        checks.append(check)
        if not passed:
            failures.append(check)

    for evidence in task.get("required_evidence", []):
        path = workspace / evidence["path"]
        present = path.is_file()
        valid = False
        details: Any = evidence["path"]
        if present and evidence.get("format") == "json":
            try:
                value = load_json(path)
                valid = all(key in value for key in evidence.get("required_keys", []))
                details = {"path": evidence["path"], "required_keys": evidence.get("required_keys", [])}
            except EvalError as exc:
                details = str(exc)
        else:
            valid = present
        check = {"name": f"evidence:{evidence['path']}", "kind": "evidence", "passed": valid, "details": details}
        checks.append(check)
        if not valid:
            failures.append(check)

    gate = task.get("gate", {})
    gate_file = gate.get("evidence_path")
    if gate_file:
        path = workspace / gate_file
        gate_ok = False
        details: Any = gate_file
        if path.is_file():
            try:
                value = load_json(path)
                gate_ok = value.get("status") == "pass" and all(key in value for key in gate.get("required_keys", ["status", "checks"]))
                details = {"status": value.get("status"), "required_keys": gate.get("required_keys", ["status", "checks"])}
            except EvalError as exc:
                details = str(exc)
        check = {"name": "final_gate", "kind": "gate", "passed": gate_ok, "details": details}
        checks.append(check)
        if not gate_ok:
            failures.append(check)

    return checks, failures, not failures and not bad_allowed and not bad_forbidden


def verify_payload(payload: dict[str, Any]) -> None:
    integrity = payload.get("integrity", {})
    supplied = integrity.get("sha256")
    if not supplied:
        raise EvalError("result is missing integrity.sha256")
    unsigned = dict(payload)
    unsigned["integrity"] = {"schema_version": integrity.get("schema_version", SCHEMA_VERSION)}
    expected = digest(unsigned)
    if supplied != expected:
        raise EvalError("tampered result: integrity.sha256 does not match canonical payload")


def finalize(payload: dict[str, Any]) -> dict[str, Any]:
    unsigned = dict(payload)
    unsigned["integrity"] = {"schema_version": SCHEMA_VERSION}
    unsigned["integrity"]["sha256"] = digest(unsigned)
    return unsigned

def load_tasks(path: pathlib.Path) -> dict[str, dict[str, Any]]:
    corpus = load_json(path)
    if corpus.get("schema_version") != SCHEMA_VERSION or not isinstance(corpus.get("tasks"), list):
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
        shape = tuple(task["allowed_diff_paths"])
        if shape in diff_shapes:
            raise EvalError(f"duplicate allowed diff shape for task {task_id}")
        diff_shapes.add(shape)
        tasks[task_id] = task
    if len(tasks) < 30:
        raise EvalError(f"corpus has {len(tasks)} tasks; at least 30 are required")
    return tasks



def grade(task: dict[str, Any], workspace: pathlib.Path, agent_run: dict[str, Any] | None, task_seed: str, timeout: float, max_output: int, extra_metrics: dict[str, Any] | None = None, base_commit: str = "HEAD") -> dict[str, Any]:
    env = os.environ.copy()
    env.update({"CANON_EVAL_TASK_ID": task["id"], "CANON_EVAL_SEED_DIR": task_seed})
    test_run = run_command(task["grader"]["command"], workspace, timeout, max_output, env)
    changed = changed_paths(workspace, base_commit, task.get("forbidden_side_effects", {}).get("paths", []))
    checks, failures, contract_passed = check_contract(task, workspace, changed, timeout, max_output, env, test_run)
    semantic = extra_metrics.pop("_semantic", None) if extra_metrics else None
    semantic_required = task.get("semantic_grader", {}).get("required", False)
    if semantic_required and semantic is None:
        failure = {"name": "semantic_judgment", "passed": False, "details": "explicit independent judged evidence missing"}
        checks.append(failure)
        failures.append(failure)
    objective_passed = (
        test_run["exit_code"] == task["grader"].get("exit_code", 0)
        and command_was_exercised(test_run)
        and contract_passed
        and (not semantic_required or semantic is not None and semantic["decision"] == "pass")
    )
    claimed = agent_run is not None and agent_run.get("exit_code") == 0 and not agent_run.get("timed_out", False)
    evidence = {
        "required": [item["path"] for item in task["required_evidence"]],
        "present": [item["path"] for item in task["required_evidence"] if (workspace / item["path"]).is_file()],
        "checks": checks,
    }
    metrics = {key: value for key, value in (extra_metrics or {}).items() if key != "_semantic"}
    for key in ("latency_ms", "cost_usd", "input_tokens", "output_tokens"):
        if key not in metrics:
            metrics[key] = None
    payload = {
        "schema_version": SCHEMA_VERSION,
        "task_id": task["id"],
        "changed_paths": changed,
        "passed": objective_passed,
        "provenance": {"corpus": str(DEFAULT_CORPUS), "corpus_digest": digest(task), "workspace": str(workspace), "base_commit": base_commit, "agent_command": agent_run.get("command") if agent_run else None},
        "agent": agent_run,
        "diff": {"changed_paths": changed, "allowed": task["allowed_diff_paths"], "forbidden": task.get("forbidden_side_effects", {}).get("paths", [])},
        "tests": test_run,
        "evidence": evidence,
        "gate": {"passed": objective_passed, "failures": failures},
        "semantic": semantic,
        "outcome": {"passed": objective_passed, "agent_claimed_success": claimed, "false_pass": bool(claimed and not objective_passed), "regression": None},
        "metrics": metrics,
        "integrity": {"schema_version": SCHEMA_VERSION},
    }
    return finalize(payload)


def scoreboard(baseline: list[dict[str, Any]], candidate: list[dict[str, Any]]) -> dict[str, Any]:
    base = {item["task_id"]: item for item in baseline}
    cand = {item["task_id"]: item for item in candidate}
    task_ids = sorted(set(base) | set(cand))
    rows = []
    for task_id in task_ids:
        left, right = base.get(task_id), cand.get(task_id)
        baseline_passed = left is not None and bool(left.get("outcome", {}).get("passed"))
        candidate_passed = right is not None and bool(right.get("outcome", {}).get("passed"))
        rows.append({"task_id": task_id, "baseline_passed": baseline_passed, "candidate_passed": candidate_passed, "regression": baseline_passed and not candidate_passed, "baseline_result_sha256": left.get("integrity", {}).get("sha256") if left else None, "candidate_result_sha256": right.get("integrity", {}).get("sha256") if right else None})
    def aggregate(items: list[dict[str, Any]]) -> dict[str, Any]:
        known = [item for item in items if isinstance(item.get("outcome", {}).get("passed"), bool)]
        return {"tasks": len(items), "success_at_1": (sum(item["outcome"]["passed"] for item in known) / len(known)) if known else None, "false_pass": sum(bool(item.get("outcome", {}).get("false_pass")) for item in items), "latency_ms": [item["metrics"]["latency_ms"] for item in items if item.get("metrics", {}).get("latency_ms") is not None] or None, "cost_usd": [item["metrics"]["cost_usd"] for item in items if item.get("metrics", {}).get("cost_usd") is not None] or None}
    return {"schema_version": SCHEMA_VERSION, "provenance": {"baseline_result_digests": [item.get("integrity", {}).get("sha256") for item in baseline], "candidate_result_digests": [item.get("integrity", {}).get("sha256") for item in candidate]}, "baseline": aggregate(baseline), "candidate": aggregate(candidate), "rows": rows, "regressions": sum(row["regression"] for row in rows), "note": "Metrics remain null when no run supplied them; this scoreboard does not infer quality from agent text."}

def load_judged_evidence(path: pathlib.Path, task_id: str) -> dict[str, Any]:
    value = load_json(path)
    required = ("task_id", "decision", "evidence_refs", "provenance")
    if value.get("task_id") != task_id or value.get("decision") not in {"pass", "fail"}:
        raise EvalError("judged evidence has wrong task_id or decision")
    if any(key not in value for key in required) or not isinstance(value["evidence_refs"], list):
        raise EvalError("judged evidence requires task_id, decision, evidence_refs, and provenance")
    if not value["provenance"]:
        raise EvalError("judged evidence provenance must be explicit")
    return value


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=pathlib.Path, default=DEFAULT_CORPUS)
    parser.add_argument("--task-id")
    parser.add_argument("--repo", type=pathlib.Path, default=ROOT)
    parser.add_argument("--base-commit", help="required immutable base for a supplied completed workspace")
    parser.add_argument("--workspace", type=pathlib.Path, help="grade this completed workspace instead of running an agent")
    parser.add_argument("--agent-cmd", help="explicit argv (shell-free) for the external agent")
    parser.add_argument("--judged-evidence", type=pathlib.Path, help="optional externally supplied semantic judgment JSON; never inferred")
    parser.add_argument("--cost-usd", type=float, help="optional externally measured agent cost; omitted means null")
    parser.add_argument("--input-tokens", type=int, help="optional externally measured input tokens")
    parser.add_argument("--output-tokens", type=int, help="optional externally measured output tokens")
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
        tasks = load_tasks(args.corpus)
        if args.baseline is not None or args.candidate is not None:
            if args.baseline is None or args.candidate is None:
                raise EvalError("--baseline and --candidate must be supplied together")
            baseline, candidate = [], []
            for path in args.baseline:
                item = load_json(path)
                verify_payload(item)
                baseline.append(item)
            for path in args.candidate:
                item = load_json(path)
                verify_payload(item)
                candidate.append(item)
            output = scoreboard(baseline, candidate)
            destination = args.scoreboard or pathlib.Path("scoreboard.json")
            destination.write_text(json.dumps(output, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
            print(json.dumps(output, indent=2, ensure_ascii=False))
            return 0
        if not args.task_id or args.task_id not in tasks:
            raise EvalError("--task-id must name a corpus task")
        if bool(args.agent_cmd) == bool(args.workspace):
            raise EvalError("choose exactly one of --agent-cmd or --workspace")
        task = tasks[args.task_id]
        seed = seed_directory(task)
        worktree: pathlib.Path | None = None
        agent_run = None
        try:
            if args.workspace:
                workspace = args.workspace.resolve()
                if not args.base_commit:
                    raise EvalError("--workspace requires --base-commit so committed edits cannot evade grading")
                base_commit = args.base_commit
                code, _, err = git(workspace, "rev-parse", "--show-toplevel")
                if code != 0:
                    raise EvalError(f"workspace is not a git checkout: {err.strip()}")
                code, resolved, err = git(workspace, "rev-parse", "--verify", f"{base_commit}^{{commit}}")
                if code != 0:
                    raise EvalError(f"base commit is not resolvable: {err.strip()}")
                base_commit = resolved.strip()
            else:
                source = args.repo.resolve()
                worktree = pathlib.Path(tempfile.mkdtemp(prefix="canon-eval-worktree-"))
                worktree.rmdir()
                code, _, err = git(source, "worktree", "add", "--detach", str(worktree), "HEAD")
                if code != 0:
                    raise EvalError(f"could not create isolated worktree: {err.strip()}")
                workspace = worktree
                code, base_commit, err = git(workspace, "rev-parse", "HEAD")
                if code != 0:
                    raise EvalError(f"could not record base commit: {err}")
                base_commit = base_commit.strip()
                env = os.environ.copy()
                env.update({"CANON_EVAL_TASK_ID": task["id"], "CANON_EVAL_SEED_DIR": seed.name})
                agent_run = run_command(args.agent_cmd, workspace, args.timeout, args.max_output_bytes, env)
            judged = load_judged_evidence(args.judged_evidence, task["id"]) if args.judged_evidence else None
            supplied = {"_semantic": judged} if judged else {}
            if agent_run is not None:
                supplied["latency_ms"] = agent_run["latency_ms"]
            if args.cost_usd is not None:
                supplied["cost_usd"] = args.cost_usd
            if args.input_tokens is not None:
                supplied["input_tokens"] = args.input_tokens
            if args.output_tokens is not None:
                supplied["output_tokens"] = args.output_tokens
            result = grade(task, workspace, agent_run, seed.name, args.timeout, args.max_output_bytes, supplied, base_commit)
            text = json.dumps(result, indent=2, ensure_ascii=False) + "\n"
            if args.result:
                args.result.write_text(text, encoding="utf-8")
            print(text, end="")
            return 0 if result["outcome"]["passed"] else 1
        finally:
            if worktree is not None:
                git(args.repo.resolve(), "worktree", "remove", "--force", str(worktree))
                if worktree.exists():
                    shutil.rmtree(worktree, ignore_errors=True)
            seed.cleanup()
    except EvalError as exc:
        print(f"eval runner: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
