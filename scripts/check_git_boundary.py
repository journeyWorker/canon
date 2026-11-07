#!/usr/bin/env python3
"""Fails (exit 1, one `file:line` per hit) when crate source starts `git` anywhere but the one path.

Every git call in canon goes through `canon_store::git::GitCall` (docs/design/rust-coding-rules.md, "One git path"):
it clears the environment, pins the configuration and bounds time and output. A `Command::new("git")` anywhere else
in `crates/*/src` skips that, so this script rejects it.

Scope: `crates/*/src/**/*.rs`, `#[cfg(test)]` modules included (a fixture in a source file uses the path too).
`crates/*/tests` are integration tests of the binary and are not scanned. Comments are ignored; an argument on the
line after `Command::new(` is found. A program name held in a variable is not seen: review is the second defence.

Usage: check_git_boundary.py [ROOT]    (ROOT defaults to the repository this script is in)
"""
import re
import sys
from pathlib import Path

# Files that may start git. The path itself, and the session-ingest code that the 0.39 ingest removal deletes
# (its git calls are out of scope of the one-path change).
ALLOWED = {
    "crates/canon-store/src/git.rs",
    "crates/canon-cli/src/ingest.rs",
}

# `Command::new(` (also `std::process::Command::new(`, `tokio::process::Command::new(`), optionally `OsStr::new(`,
# then a string literal, plain or raw, whose last path segment is `git` or `git.exe`.
LAUNCH = re.compile(
    r"""Command\s*::\s*new\s*\(\s*(?:(?:std::ffi::)?OsStr\s*::\s*new\s*\(\s*)?
        (?:r(?P<hashes>\#*)")?(?(hashes)|")
        (?:[^"\\\n]*[/\\])?git(?:\.exe)?
        (?(hashes)"(?P=hashes)|")""",
    re.VERBOSE,
)
COMMENT = re.compile(r"//[^\n]*|/\*.*?\*/", re.DOTALL)


def blank_comments(text: str) -> str:
    """The text with every comment replaced by spaces, newlines kept so line numbers hold."""
    return COMMENT.sub(lambda m: re.sub(r"[^\n]", " ", m.group(0)), text)


def scan(root: Path) -> list[str]:
    hits = []
    for source in sorted(root.glob("crates/*/src/**/*.rs")):
        relative = source.relative_to(root).as_posix()
        if relative in ALLOWED:
            continue
        text = blank_comments(source.read_text(encoding="utf-8", errors="replace"))
        for match in LAUNCH.finditer(text):
            line = text.count("\n", 0, match.start()) + 1
            hits.append(f"{relative}:{line}: starts git directly; use canon_store::git::GitCall")
    return hits


def main() -> int:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent
    hits = scan(root)
    if hits:
        print("\n".join(hits), file=sys.stderr)
        print(
            "check_git_boundary: every git call goes through canon_store::git::GitCall (rust-coding-rules.md, one git path)",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
