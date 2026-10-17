#!/usr/bin/env python3
"""0.19 D8: inspect recorded tool activity, never execute transcript content."""

import argparse
import ast
from collections import Counter
from datetime import datetime
import json
import posixpath
from pathlib import Path
import re
import shlex
import sys


SHELLS = {"sh", "bash", "zsh", "dash"}
READ_ONLY = {"status", "context", "retrieve", "report", "dashboard", "help", "version"}
READ_ACTIONS = {"check", "show", "list", "doctor", "verify", "diff", "explain"}


def text(content):
    if isinstance(content, str):
        return content
    return "\n".join(c.get("text", "") for c in content or [] if isinstance(c, dict))


def timestamp(value):
    if isinstance(value, (int, float)):
        return value / 1000
    if value:
        try:
            return datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp()
        except ValueError:
            pass
    return None


def exit_code(message):
    for obj in (message, message.get("details", {})):
        for key in ("exit_code", "exitCode"):
            if isinstance(obj.get(key), int):
                return obj[key]
    output = text(message.get("content"))
    match = re.search(r"(?:Process exited with code|Command exited with (?:code|status))\s+(\d+)", output)
    if match:
        return int(match[1])
    details = message.get("details", {})
    if details.get("async", {}).get("state") == "running":
        return None
    # OMP's synchronous bash result omits exitCode on success.
    if message.get("isError") is False:
        return 0
    return None


def discover(inputs):
    """Preserve input order (Codex resumed threads), deduplicate sibling discovery."""
    found = {}
    for value in inputs:
        path = Path(value).expanduser().resolve()
        if not path.exists():
            raise ValueError(f"input does not exist: {path}")
        candidates = sorted(path.rglob("*.jsonl")) if path.is_dir() else [path]
        for candidate in candidates:
            found[candidate] = None
            sibling = candidate.with_suffix("")
            if sibling.is_dir():
                for child in sorted(sibling.rglob("*.jsonl")):
                    found[child] = None
    if not found:
        raise ValueError("no jsonl session files found")
    return list(found)


def read_sessions(paths):
    sessions = {}
    file_sessions = {}
    for path in paths:
        rows = []
        with path.open(encoding="utf-8") as stream:
            for line, raw in enumerate(stream, 1):
                if not raw.strip():
                    continue
                try:
                    row = json.loads(raw)
                except json.JSONDecodeError as exc:
                    raise ValueError(f"{path}:{line}: invalid JSON: {exc.msg}") from exc
                if not isinstance(row, dict):
                    raise ValueError(f"{path}:{line}: expected a JSON object")
                rows.append((line, row))
        header = next((r for _, r in rows if r.get("type") in ("session", "thread.started")), None)
        if header is None:
            raise ValueError(f"{path}: missing omp session or Codex thread.started header")
        sid = header.get("thread_id") or header.get("id")
        if not sid:
            raise ValueError(f"{path}: missing session id")
        provider = "codex" if header["type"] == "thread.started" else "omp"
        session = sessions.setdefault(sid, {
            "id": sid, "provider": provider, "files": [], "cwd": header.get("cwd", ""),
            "parent_file": header.get("parentSession"), "events": [], "turn": 0,
        })
        session["files"].append(str(path))
        file_sessions[str(path)] = sid
        pending = {}
        seen = set()

        def event(kind, line, row, **values):
            item = {"kind": kind, "line": line, "file": str(path),
                    "turn": max(1, session["turn"]),
                    "time": timestamp(row.get("timestamp")), **values}
            session["events"].append(item)
            return item

        for line, row in rows:
            kind = row.get("type")
            message = row.get("message", {})
            role = message.get("role")
            if kind == "turn.started" or role == "user":
                # OMP's IRC delivery is a notification, not a new human turn.
                if role == "user" and "<irc " in text(message.get("content")):
                    continue
                session["turn"] += 1
                event("turn", line, row)
            if role == "assistant":
                content = message.get("content", [])
                calls = [c for c in content if isinstance(c, dict) and c.get("type") == "toolCall"] if isinstance(content, list) else []
                for call in calls:
                    cid = call.get("id", f"line-{line}")
                    if cid in seen:
                        continue
                    seen.add(cid)
                    args = call.get("arguments", {})
                    if isinstance(args, str):
                        try:
                            args = json.loads(args)
                        except json.JSONDecodeError:
                            args = {"input": args}
                    pending[cid] = event("tool", line, row, name=call.get("name", "").split(".")[-1],
                                         args=args, result=None)
                if not calls and message.get("stopReason") in ("stop", "end_turn"):
                    event("stop", line, row)
            elif role == "toolResult":
                call = pending.get(message.get("toolCallId"))
                if call is not None:
                    call["result"] = message
            elif kind in ("turn.completed", "turn.failed"):
                event("stop", line, row)
            elif kind in ("item.started", "item.completed"):
                item = row.get("item", {})
                cid = item.get("id", f"line-{line}")
                if item.get("type") not in ("command_execution", "file_change"):
                    continue
                if cid not in pending:
                    pending[cid] = event("tool", line, row,
                                         name="bash" if item["type"] == "command_execution" else "file_change",
                                         args={"command": item.get("command", ""), "changes": item.get("changes", [])},
                                         result=None)
                if kind == "item.completed":
                    pending[cid]["result"] = {"exit_code": item.get("exit_code"),
                                              "isError": item.get("status") == "failed",
                                              "content": item.get("aggregated_output", "")}
                    pending[cid]["args"]["changes"] = item.get("changes", [])
        if not session["events"]:
            raise ValueError(f"{path}: no recognized session events")
    for session in sessions.values():
        parent = session.pop("parent_file")
        if parent:
            session["parent"] = file_sessions.get(str(Path(parent).resolve()))
        else:
            session["parent"] = None
            for file in session["files"]:
                candidate = str(Path(file).parent.with_suffix(".jsonl"))
                if candidate in file_sessions and file_sessions[candidate] != session["id"]:
                    session["parent"] = file_sessions[candidate]
                    break
    return sessions


def python_writes(source):
    """Resolve literal Python file writes, including simple path variables."""
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return [], True
    variables = {}
    writes = []
    unknown = False

    def value(node):
        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            return node.value
        if isinstance(node, ast.Name):
            return variables.get(node.id)
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == "Path" and node.args:
            return value(node.args[0])
        return None

    for node in ast.walk(tree):
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name):
                    variables[target.id] = value(node.value)
        if not isinstance(node, ast.Call):
            continue
        if isinstance(node.func, ast.Name) and node.func.id == "open" and node.args:
            mode = value(node.args[1]) if len(node.args) > 1 else next((value(k.value) for k in node.keywords if k.arg == "mode"), "r")
            if mode and any(c in mode for c in "wax+"):
                path = value(node.args[0])
                if path:
                    writes.append(path)
                else:
                    unknown = True
        elif isinstance(node.func, ast.Attribute) and node.func.attr in ("write_text", "write_bytes", "unlink", "touch", "mkdir"):
            path = value(node.func.value)
            if path:
                writes.append(path)
            else:
                unknown = True
    return writes, unknown


CODE_SUFFIXES = {
    ".c", ".cc", ".cpp", ".cxx", ".css", ".go", ".h", ".hpp", ".htm", ".html",
    ".java", ".js", ".jsx", ".json", ".m", ".mm", ".py", ".rb", ".rs", ".sh",
    ".sql", ".swift", ".ts", ".tsx", ".vue", ".yaml", ".yml",
}


def is_code_path(path):
    """Conservative classifier for implementation ownership, separate from write checks.

    Only repository files count: a scratch script outside the repo (a
    reviewer's own determinism harness under /tmp, say) implements nothing.
    """
    candidate = path.split("?", 1)[0].lower()
    if outside_repo(candidate):
        return False
    if any(part in candidate.split("/") for part in ("reports", "screens", "fixtures", ".canon")):
        return False
    return Path(candidate).suffix in CODE_SUFFIXES


def shell_commands(command):
    """Tokenize shell syntax, keeping quoted prose out of command discovery."""
    lexer = shlex.shlex(command, posix=True, punctuation_chars=";&|<>()")
    lexer.whitespace_split = True
    lexer.commenters = "#"
    tokens = list(lexer)
    if tokens and posixpath.basename(tokens[0]) in SHELLS:
        for i, token in enumerate(tokens[1:], 1):
            if token.startswith("-") and "c" in token and i + 1 < len(tokens):
                return shell_commands(tokens[i + 1])
    # Newlines are shell separators; tokenize one line at a time after unwrapping.
    if "\n" in command:
        result = []
        for line in command.splitlines():
            result.extend(shell_commands(line))
        return result
    result, part = [], []
    for token in tokens:
        if token in (";", "&&", "||", "|", "&", "(", ")"):
            if part:
                result.append((part, token))
                part = []
        else:
            part.append(token)
    if part:
        result.append((part, ""))
    return result


def shell_activity(command, cwd, result):
    writes, calls, warnings = [], [], []
    # Heredoc bodies are data, not shell commands. Inspect Python bodies separately.
    lines = command.splitlines(keepends=True)
    shell = []
    i = 0
    while i < len(lines):
        line = lines[i]
        match = re.search(r"<<-?\s*['\"]?(\w+)['\"]?", line)
        shell.append(line)
        i += 1
        if match:
            body = []
            while i < len(lines) and lines[i].strip() != match[1]:
                body.append(lines[i])
                i += 1
            i += 1
            if re.search(r"\bpython[\d.]*\b", line):
                paths, unknown = python_writes("".join(body))
                writes.extend((p, cwd) for p in paths)
                if unknown:
                    warnings.append("unresolved Python write target")
    try:
        parts = shell_commands("".join(shell))
    except ValueError:
        return writes, calls, ["unparsed shell syntax"]
    code = exit_code(result) if result else None
    all_and = all(separator == "&&" for _, separator in parts[:-1])
    current_cwd = cwd
    for index, (tokens, separator) in enumerate(parts):
        for j, token in enumerate(tokens[:-1]):
            if token in (">", ">>", ">|") and tokens[j + 1] not in ("&", "1", "2", "/dev/null"):
                writes.append((tokens[j + 1], current_cwd))
        # Remove redirections and their operands before inspecting argv.
        argv = []
        skip = False
        for token in tokens:
            if skip:
                skip = False
                continue
            if token in (">", ">>", ">|", "<", "<<", "<<-", "<<<", ">&", "<&"):
                if argv and argv[-1].isdigit():
                    argv.pop()
                skip = True
            else:
                argv.append(token)
        while argv and (re.match(r"^[A-Za-z_]\w*=", argv[0]) or argv[0] in ("env", "command", "then", "do")):
            argv.pop(0)
        if not argv:
            continue
        exe = posixpath.basename(argv[0])
        args = argv[1:]
        if exe == "cd" and args:
            current_cwd = posixpath.normpath(posixpath.join(current_cwd, args[0]))
        elif exe == "canon":
            # A pipeline's exit status belongs to its final command, not canon.
            known = code if ((all_and and code == 0 and separator != "|") or index == len(parts) - 1) else None
            words = [a for a in args if not a.startswith("-")]
            verb = words[0] if words else "<bare>"
            action = words[1] if len(words) > 1 else ""
            calls.append({"verb": verb, "action": action, "exit_code": known,
                          "command": shlex.join(argv),
                          # `canon review add --help` reads usage; it records nothing.
                          "help": "--help" in args or "-h" in args})
            if "--help" not in args and "--version" not in args and verb not in READ_ONLY and action not in READ_ACTIONS:
                if not (verb in ("brief", "finding", "subject", "skills", "gate") and not action):
                    writes.append((f".canon/<{verb}{' ' + action if action else ''}>", current_cwd))
            if verb == "init":
                writes.extend((p, current_cwd) for p in ("canon.yaml", "AGENTS.md"))
            if verb == "skills" and action == "install":
                writes.append(("<provider skill files>", current_cwd))
            if verb == "change" and action == "new" and known == 0:
                writes.append((".canon/changes/<new>/brief.md", current_cwd))
        elif exe in ("touch", "mkdir", "rm", "rmdir"):
            writes.extend((a, current_cwd) for a in args if not a.startswith("-"))
        elif exe in ("cp", "mv", "install") and args:
            writes.append((args[-1], current_cwd))
            if exe == "mv":
                writes.extend((a, current_cwd) for a in args[:-1] if not a.startswith("-"))
        elif exe == "tee":
            writes.extend((a, current_cwd) for a in args if not a.startswith("-"))
        elif exe in ("sed", "perl") and any(a.startswith("-i") for a in args) and args:
            writes.append((args[-1], current_cwd))
        elif exe == "git" and args and args[0] in ("rm", "restore"):
            writes.extend((a, current_cwd) for a in args[1:] if not a.startswith("-"))
        elif exe.startswith("python") and "-c" in args:
            offset = args.index("-c") + 1
            if offset < len(args):
                paths, unknown = python_writes(args[offset])
                writes.extend((p, current_cwd) for p in paths)
                if unknown:
                    warnings.append("unresolved Python write target")
    return writes, calls, warnings


def tool_activity(event, cwd):
    name, args, result = event["name"], event["args"], event["result"]
    writes, calls, warnings = [], [], []
    cwd = args.get("cwd") or cwd
    if name in ("bash", "shell", "exec_command"):
        return shell_activity(args.get("command", args.get("cmd", "")), cwd, result)
    if result and result.get("isError"):
        return writes, calls, warnings
    if name == "write" and args.get("path"):
        writes.append((args["path"], cwd))
    elif name in ("edit", "apply_patch"):
        if args.get("path"):
            writes.append((args["path"], cwd))
        patch = args.get("input", args.get("patch", ""))
        paths = re.findall(r"^\[(.+)#[A-Fa-f0-9]{4}\]$", patch, re.M)
        paths += re.findall(r"^\*\*\* (?:Add|Update|Delete) File: (.+)$", patch, re.M)
        paths += re.findall(r"^(?:MV |\*\*\* Move to: )(.+)$", patch, re.M)
        writes.extend((p.strip('"'), cwd) for p in paths)
        if not writes:
            warnings.append("unparsed edit targets")
    elif name == "file_change":
        writes.extend((c["path"], cwd) for c in args.get("changes", []) if c.get("path"))
    elif name == "eval":
        code = args.get("code", "")
        # canon run from a cell (``Bun.$`canon review add …` ``, subprocess):
        # counted with an unknown exit code, since the cell's result is not
        # the command's.
        calls.extend(eval_canon_calls(code))
        if args.get("language") in ("py", "python"):
            paths, unknown = python_writes(code)
            writes.extend((p, cwd) for p in paths)
            if unknown:
                warnings.append("unresolved Python write target")
        else:
            warnings.append("eval side effects are not statically resolved")
    return writes, calls, warnings


EVAL_CANON = re.compile(r"""(?:^|[\s`'"(\[,])canon\s+((?:--?[\w-]+(?:\s+(?!-)[\w.-]+)?\s+)*)([a-z][a-z-]*)(?:\s+([a-z][a-z-]*))?(?P<rest>[^`'"\n]*)""")


def eval_canon_calls(code):
    """canon commands spelled out in an eval cell's code."""
    calls = []
    for match in EVAL_CANON.finditer(code):
        verb, action, rest = match.group(2), match.group(3) or "", match.group("rest")
        if action.startswith("-"):
            action = ""
        calls.append({"verb": verb, "action": action, "exit_code": None,
                      "command": match.group(0).strip(" `'\"([,"),
                      "help": "--help" in rest or " -h" in rest})
    return calls
    return writes, calls, warnings


def outside_repo(path):
    """A path that leaves the repository: above its root, absolute, or under
    an unexpanded shell variable (`$TMPDIR/x`, `${HOME}/x`). Scratch outside
    the repo neither implements the change nor breaks the brief stop."""
    return path.startswith(("../", "/", "$")) or path == ".."


def normalize_path(path, cwd, root):
    if "://" in path:
        if path.startswith("local://"):
            return path, False
        return None, False  # tool devices / messages are not filesystem writes
    absolute = posixpath.normpath(posixpath.join(cwd, path))
    relative = posixpath.relpath(absolute, root) if root else absolute
    if not root and absolute.startswith("/"):
        # Codex exec streams have no cwd; recognize the canonical path segment.
        relative = absolute[absolute.index("/.canon/") + 1:] if "/.canon/" in absolute else absolute
    return relative, relative == ".canon" or relative.startswith(".canon/")


def metrics(calls):
    counts = Counter(c["verb"] for c in calls)
    first = {}
    recoveries = []
    for index, call in enumerate(calls):
        if call["verb"] not in first:
            first[call["verb"]] = {"valid": None if call["exit_code"] is None else call["exit_code"] == 0,
                                    "exit_code": call["exit_code"], "line": call["line"], "file": call["file"]}
        if call["exit_code"] not in (None, 0):
            success = next((j for j in range(index + 1, len(calls)) if calls[j]["exit_code"] == 0), None)
            recoveries.append({"failed_line": call["line"], "failed_file": call["file"],
                               "next_success_line": calls[success]["line"] if success is not None else None,
                               "calls_between": success - index - 1 if success is not None else None})
    return {"canon_call_count": dict(sorted(counts.items())), "first_command_validity": first,
            "recovery_depth": recoveries}


def analyze(inputs, question_turns=()):
    sessions = read_sessions(discover(inputs))
    brief_times = []
    for session in sessions.values():
        session["writes"], session["calls"], session["warnings"] = [], [], []
        session["brief_stop"] = None
        brief_written = False
        for ordinal, event in enumerate(session["events"]):
            if event["kind"] == "stop" and brief_written and session["brief_stop"] is None:
                session["brief_stop"] = {**event, "ordinal": ordinal}
            if event["kind"] != "tool":
                continue
            writes, calls, warnings = tool_activity(event, session["cwd"])
            location = {k: event[k] for k in ("line", "file", "turn", "time")}
            for path, cwd in writes:
                path, inside = normalize_path(path, cwd, session["cwd"])
                if path is None:
                    continue
                session["writes"].append({**location, "path": path, "inside_canon": inside,
                                          "code": (not inside and is_code_path(path)),
                                          "ordinal": ordinal})
                if inside and (path.endswith("/brief.md") or path == ".canon/brief.md"):
                    brief_written = True
                    brief_times.append((session, event["time"]))
            session["calls"].extend({**location, **call} for call in calls)
            session["warnings"].extend({**location, "message": w} for w in warnings)

    def root_session(session):
        seen = set()
        while session.get("parent") in sessions:
            if session["id"] in seen:
                raise ValueError("cyclic session parents")
            seen.add(session["id"])
            session = sessions[session["parent"]]
        return session

    # A brief written by a subagent (a planner the orchestrator spawned) stops
    # its root: the root's first stop after that write is the brief stop.
    for writer, at in sorted(brief_times, key=lambda item: item[1] or 0):
        root = root_session(writer)
        if root["brief_stop"] is not None or at is None:
            continue
        for ordinal, event in enumerate(root["events"]):
            if event["kind"] == "stop" and event["time"] is not None and event["time"] >= at:
                root["brief_stop"] = {**event, "ordinal": ordinal}
                break

    failures, reports = [], []
    implementers, reviewers = [], []
    for sid, session in sessions.items():
        root = root_session(session)
        stop = root["brief_stop"]
        turns = {}
        for event in session["events"]:
            turns.setdefault(event["turn"], {"turn": event["turn"], "writes": [], "calls": []})
        for write in session["writes"]:
            if sid == root["id"]:
                before = stop is None or write["ordinal"] < stop["ordinal"]
                root_turn = write["turn"]
            else:
                before = stop is None or write["time"] is None or stop["time"] is None or write["time"] < stop["time"]
                starts = [e for e in root["events"] if e["kind"] == "turn" and e["time"] is not None and write["time"] is not None and e["time"] <= write["time"]]
                root_turn = starts[-1]["turn"] if starts else None
            write["root_turn"] = root_turn
            write["before_brief_stop"] = before
            write["question_only"] = root_turn in question_turns
            turns[write["turn"]]["writes"].append(write)
            for rule, failed in (("write_before_brief_stop", before and not write["inside_canon"] and not outside_repo(write["path"])),
                                 ("write_in_question_turn", write["question_only"])):
                if failed:
                    failures.append({"check": rule, "session": sid, **write})
        for call in session["calls"]:
            turns[call["turn"]]["calls"].append(call)
        implements = any(w["code"] for w in session["writes"])
        reviews = any(c["verb"] == "review" and c["action"] == "add" and not c["help"] for c in session["calls"])
        if implements:
            implementers.append(sid)
        if reviews:
            reviewers.append(sid)
        if implements and reviews:
            failures.append({"check": "reviewer_is_implementer", "session": sid})
        reports.append({"id": sid, "provider": session["provider"], "files": session["files"],
                        "parent": session["parent"], "root_session": root["id"], "brief_stop": stop,
                        "implementer": implements, "reviewer": reviews,
                        "code_writes": [w for w in session["writes"] if w["code"]],
                        "warnings": session["warnings"],
                        "turns": [{**t, **metrics(t["calls"])} for t in turns.values()],
                        **metrics(session["calls"])})
    counts = Counter()
    for report in reports:
        counts.update(report["canon_call_count"])
    return {"hard_checks_passed": not failures, "failures": failures,
            "implementer_sessions": sorted(implementers), "reviewer_sessions": sorted(reviewers),
            "canon_call_count": dict(sorted(counts.items())), "sessions": reports}


def render(report):
    lines = [f"Hard checks: {'PASS' if report['hard_checks_passed'] else 'FAIL'} ({len(report['failures'])} violation(s))",
             "Canon calls: " + json.dumps(report["canon_call_count"], sort_keys=True),
             "Implementer sessions: " + (", ".join(report["implementer_sessions"]) or "none observed"),
             "Reviewer sessions: " + (", ".join(report["reviewer_sessions"]) or "none observed")]
    for session in report["sessions"]:
        stop = session["brief_stop"]
        lines.append(f"Session {session['id']} ({session['provider']}): brief stop " + (f"turn {stop['turn']}, line {stop['line']}" if stop else "not observed"))
        for turn in session["turns"]:
            outside = [w for w in turn["writes"] if not w["inside_canon"]]
            before = sum(w["before_brief_stop"] for w in outside)
            question = sum(w["question_only"] for w in turn["writes"])
            lines.append(f"  Turn {turn['turn']}: {len(outside)} outside write(s), {before} before brief stop, {question} question-only write(s); canon {json.dumps(turn['canon_call_count'], sort_keys=True)}")
        validity = {verb: value["valid"] for verb, value in session["first_command_validity"].items()}
        lines.append("  First-command validity: " + json.dumps(validity, sort_keys=True))
        lines.append("  Recovery depth (intervening canon calls): " + json.dumps(session["recovery_depth"]))
        if session["warnings"]:
            lines.append(f"  Coverage warnings: {len(session['warnings'])} (details in --json)")
    for failure in report["failures"]:
        lines.append(f"FAIL {failure['check']}: {failure['session']}" + (f" turn {failure['turn']} line {failure['line']}: {failure['path']}" if "path" in failure else ""))
    lines.append("Scope: recorded explicit writes and canon invocations; null exit/validity means not attributable from the trace, not success. Arbitrary subprocess side effects are not a filesystem audit.")
    return "\n".join(lines)


def positive_turn(value):
    turn = int(value)
    if turn < 1:
        raise argparse.ArgumentTypeError("turn numbers start at 1")
    return turn


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", nargs="+", help="JSONL files or session directories; includes sibling subagent files")
    parser.add_argument("--question-turn", action="append", type=positive_turn, default=[], metavar="N",
                        help="mark root user turn N question-only (repeatable; applies to each root)")
    parser.add_argument("--json", action="store_true", help="emit a machine-readable report")
    args = parser.parse_args(argv)
    try:
        report = analyze(args.inputs, args.question_turn)
    except (OSError, ValueError) as exc:
        if args.json:
            print(json.dumps({"error": str(exc)}))
        else:
            print(f"trace_check: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(report, indent=2, ensure_ascii=False) if args.json else render(report))
    return 0 if report["hard_checks_passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
