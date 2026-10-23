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
# 0.28 D9: whether a canon call writes comes from the command spec's `effect`, not a verb list.
SPEC_PATH = Path(__file__).resolve().parents[2] / "docs" / "cli" / "canon.cli.json"


def load_effects(path=SPEC_PATH):
    """`{command path tuple: effect}` for every leaf of the generated spec (`read`, `write` or `destructive`)."""
    with open(path, encoding="utf-8") as stream:
        spec = json.load(stream)
    return {tuple(node["path"]): node["effect"] for node in spec["commands"] if node["nodeKind"] == "leaf"}


EFFECTS = load_effects()


def leaf_of(words):
    """The longest leaf command path that starts `words`, or `()` for a spelling the spec does not know."""
    for size in range(min(len(words), 3), 0, -1):
        if tuple(words[:size]) in EFFECTS:
            return tuple(words[:size])
    return ()


def text(content):
    if isinstance(content, str):
        return content
    return "\n".join(c.get("text", "") for c in content or [] if isinstance(c, dict))


ANSI = re.compile(r"\x1b\[[0-9;]*m")
RENAMED = re.compile(r"(?:--[\w-]+(?: \S+)?) was (?:renamed (?:--[\w-]+(?: \S+)?)|removed) in [\d.]+; use ")


def error_code(output):
    """The canon error code in a result text: JSON `"code":"X"` or human `canon <path>: X —` / `error[X]`."""
    for pattern in (r'"error"\s*:\s*\{[^{}]*?"code"\s*:\s*"([A-Z][A-Z0-9_]+)"',
                    r"canon [^:\n]+: ([A-Z][A-Z0-9_]+) \u2014", r"error\[([A-Z][A-Z0-9_]+)\]"):
        match = re.search(pattern, output)
        if match:
            return match[1]
    return None


def classify_failure(output):
    """`naming` for a wrong-shape call caused by a name, else `other`."""
    output = ANSI.sub("", output)
    if RENAMED.search(output):
        return "naming"  # a spelling the CLI removed, refused with the name that replaced it
    if re.search(r"invalid value '[^']*' for '--kind\b", output):
        return "naming"
    for flag in re.findall(r"unexpected argument '(--[\w-]+)'", output):
        if re.search(r"similar argument exists", output):
            return "naming"
        if re.search(re.escape(flag) + r"-id\b", output.replace(f"'{flag}'", "")):
            return "naming"
    if error_code(output) == "USAGE":
        for flag in re.findall(r"(?:unknown|unexpected)[^\n]*?(--[\w-]+)", output):
            if re.search(re.escape(flag) + r"-id\b|" + re.escape(flag) + r"_record\b", output.replace(flag, "", 1)):
                return "naming"
    return "other"


# Leaves that record a human's decision: an agent never runs them, with or without its role.
HUMAN_ONLY = {("brief", "approve"), ("policy", "approve"), ("divergence", "accept")}
ROLE_VARIABLE = "CANON_ROLE"
# `error.code`s of a call whose shape the CLI refused: a wrong or unknown option, operand or input key.
SHAPE_CODES = {"USAGE", "UNKNOWN_FIELD"}
NO_ENV = {"assigned": {}, "unset": set(), "clear": False}


def is_shape_code(code):
    return code in SHAPE_CODES or (code or "").startswith("INPUT_")


def failure_envelopes(output):
    """Every 0.28 JSON failure envelope (`{"ok":false,"error":{...},"meta":{...}}`) in a result text.

    A document cut short by `head -c` or `cut -c` is still read: its code, message, hint and
    `meta.command` are pulled out by pattern, so a truncated refusal is not lost."""
    found, decoder = [], json.JSONDecoder()
    for match in re.finditer(r'\{\s*"ok"\s*:\s*false\b', output):
        try:
            doc, _ = decoder.raw_decode(output, match.start())
            error = doc.get("error") if isinstance(doc, dict) else None
            if isinstance(error, dict):
                meta = doc.get("meta") if isinstance(doc.get("meta"), dict) else {}
                found.append({"code": error.get("code"), "message": error.get("message") or "",
                              "hint": error.get("hint") or "", "command": meta.get("command")})
                continue
        except json.JSONDecodeError:
            pass
        rest = output[match.start():match.start() + 4000]

        def pick(key):
            hit = re.search(r'"' + key + r'"\s*:\s*"((?:[^"\\]|\\.)*)"', rest)
            return hit[1] if hit else ""
        found.append({"code": pick("code") or None, "message": pick("message"), "hint": pick("hint"),
                      "command": pick("command") or None})
    return found


def classify_envelope(envelope):
    """`naming` when the refusal says a spelling was removed or renamed, else `other`."""
    said = f"{envelope['message']} {envelope['hint']}"
    if re.search(r"\bwas (?:removed|renamed)\b", said):
        return "naming"
    return classify_failure(json.dumps({"error": {"code": envelope["code"], "message": envelope["message"], "hint": envelope["hint"]}}))


def usage_path(output):
    """The command a clap usage error names (`Usage: canon subject new …`), as a leaf path tuple."""
    match = re.search(r"^Usage: canon ((?:[a-z][a-z-]* ?)+)", output, re.M)
    return leaf_of(match[1].split()) if match else ()


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


def landed(root, path):
    """Whether `path` ever landed in the repository's git history: code that
    was committed implements the change; a scratch file written and thrown
    away (a reviewer's diagnostic spec, a harness under the repo's `tmp/`)
    does not. When `root` is not a git work tree (a fixture, a deleted repo),
    every repository path counts, as before."""
    import subprocess
    try:
        inside = subprocess.run(["git", "-C", root, "rev-parse", "--is-inside-work-tree"], capture_output=True, text=True, timeout=10)
        if inside.returncode != 0:
            return True
        found = subprocess.run(["git", "-C", root, "log", "--all", "--format=%H", "-n", "1", "--", path], capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return True
    return found.returncode != 0 or bool(found.stdout.strip())


def newline_separators(command):
    """`command` with every newline outside quotes turned into `;`, so a quoted
    multi-line argument (`python3 -c "…\n…"`) stays one word."""
    out, quote, escaped = [], None, False
    for ch in command:
        if escaped:
            escaped = False
        elif ch == "\\" and quote != "'":
            escaped = True
        elif quote:
            quote = None if ch == quote else quote
        elif ch in "'\"":
            quote = ch
        elif ch == "\n":
            ch = " ; "
        out.append(ch)
    return "".join(out)


def shell_commands(command):
    """Tokenize shell syntax, keeping quoted prose out of command discovery."""
    lexer = shlex.shlex(newline_separators(command), posix=True, punctuation_chars=";&|<>()")
    lexer.whitespace_split = True
    lexer.commenters = "#"
    tokens = [token.lstrip("`") for token in lexer]
    tokens = [token for token in tokens if token]
    if tokens and posixpath.basename(tokens[0]) in SHELLS:
        for i, token in enumerate(tokens[1:], 1):
            if token.startswith("-") and "c" in token and i + 1 < len(tokens):
                return shell_commands(tokens[i + 1])
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


def strip_prefix(argv):
    """Peel what runs `argv` without being the command: `VAR=x`, `env [-u NAME] [-i] [VAR=x]`,
    `command`, `time`, `timeout N`, loop and `then` keywords. Returns the rest plus the environment
    it set: `{"assigned": {NAME: value}, "unset": {NAME}, "clear": bool}`."""
    argv, env = list(argv), {"assigned": {}, "unset": set(), "clear": False}
    while argv:
        word = argv[0]
        assignment = re.match(r"^([A-Za-z_]\w*)=(.*)$", word)
        base = posixpath.basename(word)
        if assignment:
            env["assigned"][assignment[1]] = assignment[2]
            argv.pop(0)
        elif base == "env":
            argv.pop(0)
            while argv and argv[0].startswith("-"):
                flag = argv.pop(0)
                if flag in ("-u", "--unset") and argv:
                    env["unset"].add(argv.pop(0))
                elif flag.startswith("--unset="):
                    env["unset"].add(flag.split("=", 1)[1])
                elif flag in ("-i", "--ignore-environment", "-"):
                    env["clear"] = True
                elif flag in ("-C", "--chdir", "-S", "--split-string", "-P") and argv:
                    argv.pop(0)
                elif flag == "--":
                    break
        elif base == "timeout":
            argv.pop(0)
            while argv and argv[0].startswith("-"):
                flag = argv.pop(0)
                if flag in ("-k", "-s", "--kill-after", "--signal") and argv:
                    argv.pop(0)
            if argv:
                argv.pop(0)
        elif word in ("command", "then", "do", "else", "time", "exec", "nohup", "!", "{", "builtin") or base in ("nohup", "time"):
            argv.pop(0)
        else:
            break
    return argv, env


def merged(shell_env, env):
    return {"assigned": {**shell_env["assigned"], **env["assigned"]},
            "unset": shell_env["unset"] | env["unset"], "clear": shell_env["clear"] or env["clear"]}


def shell_activity(command, cwd, result, inherited=None):
    writes, calls, warnings = [], [], []
    # Variables a `VAR=x`, `export` or `unset` earlier in this command (or in the shell string it sits in) left behind.
    shell_env = merged(inherited or NO_ENV, NO_ENV)
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
                calls.extend(python_canon_calls("".join(body)))
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
        argv, env = strip_prefix(argv)
        if not argv:
            # `VAR=x` alone sets it for the rest of this shell string.
            shell_env["assigned"].update(env["assigned"])
            for name in env["assigned"]:
                shell_env["unset"].discard(name)
            continue
        exe = posixpath.basename(argv[0])
        args = argv[1:]
        if exe == "cd" and args:
            current_cwd = posixpath.normpath(posixpath.join(current_cwd, args[0]))
        elif exe == "export":
            for arg in args:
                name, equals, value = arg.partition("=")
                if equals:
                    shell_env["assigned"][name] = value
                    shell_env["unset"].discard(name)
        elif exe == "unset":
            for name in (a for a in args if not a.startswith("-")):
                shell_env["unset"].add(name)
                shell_env["assigned"].pop(name, None)
        elif exe in SHELLS and any(a.startswith("-") and not a.startswith("--") and "c" in a for a in args):
            # `bash -c "canon …"` anywhere in a chain: the string is a shell of its own.
            at = next(i for i, a in enumerate(args) if a.startswith("-") and not a.startswith("--") and "c" in a)
            if at + 1 < len(args):
                more_writes, more_calls, more_warnings = shell_activity(args[at + 1], current_cwd, None, merged(shell_env, env))
                writes.extend(more_writes)
                calls.extend(more_calls)
                warnings.extend(more_warnings)
        elif exe == "canon":
            # A pipeline's exit status belongs to its final command, not canon.
            known = code if ((all_and and code == 0 and separator != "|") or index == len(parts) - 1) else None
            words = [a for a in args if not a.startswith("-")]
            verb = words[0] if words else "<bare>"
            action = words[1] if len(words) > 1 else ""
            calls.append(canon_call(args, words, shlex.join(argv), known, merged(shell_env, env)))
            leaf = leaf_of(words)
            if "--help" not in args and "--version" not in args and leaf and EFFECTS[leaf] != "read":
                writes.append((f".canon/<{' '.join(leaf)}>", current_cwd))
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
                calls.extend(python_canon_calls(args[offset]))
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
# `["canon", "review", "scenario", …]` in a subprocess call.
ARGV_LIST = re.compile(r"""\[\s*(?:\w+\s*\+\s*)?['"]canon['"]\s*((?:,\s*(?:f?['"][^'"]*['"]|\w+)\s*)+)\]""")
SUBPROCESS = re.compile(r"\b(?:subprocess|os\.(?:system|popen)|Popen|check_output|check_call|execvp?)\b")
STRIPS_ROLE = re.compile(r"""env\s+-u\s+CANON_ROLE|pop\(\s*['"]CANON_ROLE|del\s+[\w.]*\[\s*['"]CANON_ROLE""")


def declared_role(args, env):
    """The role a call declares: `--as <role>`, else a `CANON_ROLE=<role>` assignment; `None` for neither."""
    for i, arg in enumerate(args):
        if arg == "--as" and i + 1 < len(args):
            return args[i + 1]
        if arg.startswith("--as="):
            return arg.split("=", 1)[1]
    if ROLE_VARIABLE in env["unset"] or env["clear"]:
        return None
    return env["assigned"].get(ROLE_VARIABLE) or None


def canon_call(args, words, command, exit_code, env):
    """One recorded canon invocation: `args` are its arguments after `canon`, `words` the ones that
    are not options, `env` what the shell around it set (`assigned`, `unset`, `clear`)."""
    verb = words[0] if words else "<bare>"
    # `canon review add --help` reads usage; it records nothing.
    help_flag = "--help" in args or "-h" in args or verb == "help"
    return {"verb": verb, "action": words[1] if len(words) > 1 else "", "exit_code": exit_code,
            "command": command, "help": help_flag, "probe": help_flag or verb == "schema",
            "leaf": " ".join(leaf_of(words)), "role": declared_role(args, env),
            "role_stripped": ROLE_VARIABLE in env["unset"] or env["clear"] or env["assigned"].get(ROLE_VARIABLE) == ""}


def eval_canon_calls(code):
    """canon commands spelled out in an eval cell's code."""
    calls = []
    stripped = bool(STRIPS_ROLE.search(code))
    for match in EVAL_CANON.finditer(code):
        verb, action, rest = match.group(2), match.group(3) or "", match.group("rest")
        if action.startswith("-"):
            action = ""
        rest_args = rest.split()
        words = [verb] + ([action] if action else []) + [a for a in rest_args if not a.startswith("-")]
        call = canon_call(rest_args, words, match.group(0).strip(" `'\"([,"), None, NO_ENV)
        call["help"] = "--help" in rest or " -h" in rest
        call["probe"] = call["help"] or verb == "schema"
        call["role_stripped"] = stripped
        calls.append(call)
    for match in ARGV_LIST.finditer(code):
        args = [w.strip().strip("'\"") for w in match.group(1).split(",") if w.strip()]
        words = [a for a in args if not a.startswith("-")]
        if words:
            call = canon_call(args, words, "canon " + " ".join(args), None, NO_ENV)
            call["role_stripped"] = stripped
            calls.append(call)
    return calls


def python_canon_calls(source):
    """canon calls a Python source runs through a subprocess; nothing else in it is a call."""
    return eval_canon_calls(source) if SUBPROCESS.search(source) else []


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


def wrong_shape_item(event, call, code, kind, message=""):
    return {"file": event["file"], "line": event["line"], "command": call["command"],
            "exit_code": call["exit_code"], "code": code, "class": kind, "message": message}


CLAP_USAGE = re.compile(r"^error: (?:unexpected argument|the following required arguments? w|invalid value|unrecognized subcommand|a value is required|unknown option)", re.M)


def classify_calls(calls, output, event):
    """Attribute the failures in one tool result to its canon calls (`calls` may grow).

    A 0.28 JSON failure envelope names its command (`meta.command`), so it is matched to the call
    with that leaf, and counts as a wrong shape when its code is a shape code (`USAGE`,
    `UNKNOWN_FIELD`, `INPUT_*`); any other code is a refusal, not a mis-shaped call. An envelope no
    call explains (a canon run the parser cannot see) becomes a call of its own. Without an
    envelope, a clap usage error or a non-zero exit the call is known to own is a wrong shape."""
    wrong, refusals, matched = [], [], set()
    for envelope in failure_envelopes(output):
        path = envelope["command"] or ""
        index = next((i for i, c in enumerate(calls) if i not in matched and c["leaf"] == path), None)
        if index is None and len(calls) == 1 and not matched and not path:
            index = 0
        if index is None:
            index = len(calls)
            calls.append(canon_call([], path.split(), "canon " + (path or "<unseen>"), None, NO_ENV))
        matched.add(index)
        call = calls[index]
        call["error_code"] = envelope["code"]
        if is_shape_code(envelope["code"]):
            wrong.append(wrong_shape_item(event, call, envelope["code"], classify_envelope(envelope), envelope["message"]))
        else:
            refusals.append({"file": event["file"], "line": event["line"], "command": call["command"], "code": envelope["code"]})
    for i, call in enumerate(calls):
        if i in matched:
            continue
        if call["exit_code"] not in (None, 0):
            wrong.append(wrong_shape_item(event, call, error_code(output), classify_failure(output)))
        elif call["exit_code"] is None and CLAP_USAGE.search(output):
            owner = usage_path(output)
            if (owner and " ".join(owner) == call["leaf"]) or (not owner and len(calls) == 1):
                wrong.append(wrong_shape_item(event, call, "USAGE", classify_failure(output)))
    return {"wrong_shape": wrong, "refusals": refusals}


def probe_counts(calls):
    """How often the agent asked the CLI how to call it: `--help` / `-h` / `canon help` and `canon schema …`."""
    help_calls = sum(1 for c in calls if c["help"])
    schema = sum(1 for c in calls if c["verb"] == "schema" and not c["help"])
    return {"help": help_calls, "schema": schema, "total": help_calls + schema}


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
            "recovery_depth": recoveries, "probes": probe_counts(calls)}


def analyze(inputs, question_turns=()):
    sessions = read_sessions(discover(inputs))
    brief_times = []
    for session in sessions.values():
        session["writes"], session["calls"], session["warnings"] = [], [], []
        session["wrong_shape"], session["refusals"] = [], []
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
                                          "code": (not inside and is_code_path(path) and landed(session["cwd"], path)),
                                          "ordinal": ordinal})
                if inside and (path.endswith("/brief.md") or path == ".canon/brief.md"):
                    brief_written = True
                    brief_times.append((session, event["time"]))
            output = text(event["result"].get("content")) if event["result"] else ""
            if event["name"] in ("bash", "shell", "exec_command", "eval") and output:
                found = classify_calls(calls, ANSI.sub("", output), event)
                session["wrong_shape"].extend(found["wrong_shape"])
                session["refusals"].extend(found["refusals"])
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
        declared = False
        for call in session["calls"]:
            declared = declared or bool(call.get("role"))
            if tuple(call["leaf"].split()) in HUMAN_ONLY and not call["help"]:
                refused = call.get("error_code") == "HUMAN_ONLY"
                if call["role_stripped"] or (declared and not call.get("role") and not refused):
                    failures.append({"check": "human_only_role_stripped", "session": sid, "command": call["command"],
                                     "turn": call["turn"], "line": call["line"], "file": call["file"]})
        implements = any(w["code"] for w in session["writes"])
        reviews = any(c["verb"] == "review" and c["action"] in ("scenario", "change") and not c["help"] for c in session["calls"])
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
    counts, probes = Counter(), Counter()
    for report in reports:
        counts.update(report["canon_call_count"])
        probes.update(report["probes"])
    wrong = [item for session in sessions.values() for item in session["wrong_shape"]]
    return {"hard_checks_passed": not failures, "failures": failures,
            "implementer_sessions": sorted(implementers), "reviewer_sessions": sorted(reviewers),
            "canon_call_count": dict(sorted(counts.items())),
            "probes": {"help": probes["help"], "schema": probes["schema"], "total": probes["total"],
                       "per_session": {r["id"]: r["probes"]["total"] for r in reports}},
            "refusals": [item for session in sessions.values() for item in session["refusals"]],
            "wrong_shape_calls": {"naming": sum(i["class"] == "naming" for i in wrong),
                                  "other": sum(i["class"] == "other" for i in wrong), "items": wrong},
            "sessions": reports}


def render(report):
    lines = [f"Hard checks: {'PASS' if report['hard_checks_passed'] else 'FAIL'} ({len(report['failures'])} violation(s))",
             "Canon calls: " + json.dumps(report["canon_call_count"], sort_keys=True),
             "Wrong-shape canon calls: {naming} naming, {other} other".format(**{k: report["wrong_shape_calls"][k] for k in ("naming", "other")}),
             "Probes (--help/-h, canon schema): {total} ({help} help, {schema} schema)".format(**{k: report["probes"][k] for k in ("total", "help", "schema")}),
             "Implementer sessions: " + (", ".join(report["implementer_sessions"]) or "none observed"),
             "Reviewer sessions: " + (", ".join(report["reviewer_sessions"]) or "none observed")]
    for item in report["wrong_shape_calls"]["items"]:
        lines.append(f"  wrong shape ({item['class']}, {item['code']}): {item['command']}")
    for session in report["sessions"]:
        stop = session["brief_stop"]
        lines.append(f"Session {session['id']} ({session['provider']}): brief stop " + (f"turn {stop['turn']}, line {stop['line']}" if stop else "not observed"))
        for turn in session["turns"]:
            outside = [w for w in turn["writes"] if not w["inside_canon"]]
            before = sum(w["before_brief_stop"] for w in outside)
            question = sum(w["question_only"] for w in turn["writes"])
            lines.append(f"  Turn {turn['turn']}: {len(outside)} outside write(s), {before} before brief stop, {question} question-only write(s); canon {json.dumps(turn['canon_call_count'], sort_keys=True)}")
        validity = {verb: value["valid"] for verb, value in session["first_command_validity"].items()}
        lines.append(f"  Probes: {session['probes']['total']} ({session['probes']['help']} help, {session['probes']['schema']} schema)")
        lines.append("  First-command validity: " + json.dumps(validity, sort_keys=True))
        lines.append("  Recovery depth (intervening canon calls): " + json.dumps(session["recovery_depth"]))
        if session["warnings"]:
            lines.append(f"  Coverage warnings: {len(session['warnings'])} (details in --json)")
    for failure in report["failures"]:
        lines.append(f"FAIL {failure['check']}: {failure['session']}" + (f" turn {failure['turn']} line {failure['line']}: {failure.get('path') or failure.get('command')}" if "path" in failure or "command" in failure else ""))
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
