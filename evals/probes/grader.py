#!/usr/bin/env python3
"""Trusted evaluator-side command dispatcher; never loaded from the task workspace."""
from __future__ import annotations

import argparse
import json
import os
import selectors
import signal
import subprocess
import time


def kill_group(proc: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except (OSError, ProcessLookupError):
        try:
            proc.kill()
        except OSError:
            pass


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--task-id", required=True)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--command-json", required=True)
    parser.add_argument("--timeout", required=True, type=float)
    parser.add_argument("--max-output", required=True, type=int)
    args = parser.parse_args()
    command = json.loads(args.command_json)
    if not isinstance(command, list) or not command or not all(isinstance(item, str) for item in command):
        raise SystemExit("invalid command")
    started = time.monotonic()
    proc = subprocess.Popen(command, cwd=args.workspace, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    assert proc.stdout is not None and proc.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout, selectors.EVENT_READ, 1)
    selector.register(proc.stderr, selectors.EVENT_READ, 2)
    streams = {1: bytearray(), 2: bytearray()}
    timed_out = False
    limited = False
    deadline = started + max(args.timeout, 0.001)
    while selector.get_map():
        wait_for = max(0.001, min(0.1, deadline - time.monotonic()))
        if wait_for <= 0 and proc.poll() is None:
            timed_out = True
            kill_group(proc)
        for key, _ in selector.select(wait_for):
            chunk = key.fileobj.read(65536)
            if not chunk:
                selector.unregister(key.fileobj)
                key.fileobj.close()
                continue
            stream = streams[key.data]
            if len(stream) + len(chunk) > args.max_output:
                stream.extend(chunk[: args.max_output - len(stream)])
                limited = True
                kill_group(proc)
            else:
                stream.extend(chunk)
        if time.monotonic() >= deadline and proc.poll() is None:
            timed_out = True
            kill_group(proc)
    try:
        proc.wait(timeout=0.2)
    except subprocess.TimeoutExpired:
        kill_group(proc)
        proc.wait(timeout=0.2)
    code = None if timed_out or limited else proc.returncode
    print(json.dumps({"protocol": "canon-eval-grader-v1", "command": command, "command_text": " ".join(command), "exit_code": code, "timed_out": timed_out, "output_limited": limited, "stdout": bytes(streams[1]).decode(errors="replace"), "stderr": bytes(streams[2]).decode(errors="replace")}, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
