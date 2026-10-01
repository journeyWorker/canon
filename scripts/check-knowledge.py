#!/usr/bin/env python3
"""Validate Canon's repository knowledge map without network access.

The checker intentionally treats ``canon/knowledge-index.json`` as a small
manifest, not as a second documentation system.  It verifies that every
mapped path exists, that projections point at canonical sources, that primary
Markdown links resolve within the checkout, and that active entries have a
review date no older than the configured window.  ``--as-of`` makes freshness
checks reproducible in CI and offline fixture tests.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
from pathlib import Path, PurePosixPath
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_INDEX = Path("canon/knowledge-index.json")
DEFAULT_AS_OF = dt.date(2026, 10, 1)
DEFAULT_MAX_AGE_DAYS = 180
KINDS = {"source", "projection", "empirical-memory"}
ACTIVE_STATUSES = {"active"}
MARKDOWN_LINK = re.compile(r"!?\[[^\]]*\]\(\s*<?([^\s>]+)>?(?:\s+['\"][^'\"]*['\"])?\s*\)")


def _date(value: Any, label: str) -> dt.date | None:
    if not isinstance(value, str):
        return None
    try:
        return dt.date.fromisoformat(value)
    except ValueError:
        return None


def _safe_relative(value: Any, label: str, errors: list[str]) -> Path | None:
    if not isinstance(value, str) or not value:
        errors.append(f"{label}: path must be a non-empty string")
        return None
    normalized = value.replace("\\", "/")
    candidate = PurePosixPath(normalized)
    if candidate.is_absolute() or re.match(r"^[A-Za-z]:/", normalized):
        errors.append(f"{label}: path must be repository-relative: {value!r}")
        return None
    if any(part in {".."} for part in candidate.parts):
        errors.append(f"{label}: path contains a traversal segment: {value!r}")
        return None
    return Path(*candidate.parts)


def _read_index(index_path: Path) -> tuple[dict[str, Any] | None, list[str]]:
    try:
        raw = json.loads(index_path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None, [f"missing knowledge index: {index_path}"]
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        return None, [f"cannot read knowledge index {index_path}: {exc}"]
    if not isinstance(raw, dict):
        return None, ["knowledge index root must be an object"]
    return raw, []


def _markdown_link_errors(root: Path, entry: dict[str, Any], relative: Path) -> list[str]:
    if relative.suffix.lower() not in {".md", ".mdx"}:
        return []
    path = root / relative
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        return [f"{relative}: cannot read primary document: {exc}"]
    errors: list[str] = []
    in_fence = False
    for number, line in enumerate(lines, 1):
        if line.lstrip().startswith("```") or line.lstrip().startswith("~~~"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        for target in MARKDOWN_LINK.findall(line):
            target = target.strip()
            if not target or target.startswith(("#", "/", "http://", "https://", "mailto:", "tel:")):
                continue
            target = target.split("#", 1)[0].split("?", 1)[0]
            if not target:
                continue
            link_relative = _safe_relative(target, f"{relative}:{number}", errors)
            if link_relative is None:
                continue
            resolved = (root / relative.parent / link_relative).resolve()
            try:
                resolved.relative_to(root.resolve())
            except ValueError:
                errors.append(f"{relative}:{number}: link escapes repository: {target!r}")
                continue
            if not resolved.exists():
                errors.append(f"{relative}:{number}: broken primary link: {target!r}")
    return errors


def check_index(
    root: Path,
    index_path: Path | None = None,
    *,
    as_of: dt.date | None = None,
    max_age_days: int | None = None,
) -> list[str]:
    """Return deterministic validation errors for a knowledge index."""
    root = root.resolve()
    index_path = (root / DEFAULT_INDEX if index_path is None else index_path)
    if not index_path.is_absolute():
        index_path = root / index_path
    index, errors = _read_index(index_path)
    if index is None:
        return errors
    if index.get("schema_version") != 1:
        errors.append("knowledge index schema_version must be 1")
    entries = index.get("entries")
    if not isinstance(entries, list):
        return errors + ["knowledge index entries must be an array"]
    as_of = as_of or _date(index.get("as_of"), "as_of") or DEFAULT_AS_OF
    max_age = max_age_days if max_age_days is not None else index.get("freshness_days", DEFAULT_MAX_AGE_DAYS)
    if not isinstance(max_age, int) or max_age < 0:
        errors.append("freshness_days/max_age_days must be a non-negative integer")
        max_age = DEFAULT_MAX_AGE_DAYS

    ids: dict[str, int] = {}
    paths: dict[str, str] = {}
    by_id: dict[str, dict[str, Any]] = {}
    relative_paths: dict[str, Path] = {}
    for number, entry in enumerate(entries, 1):
        label = f"entries[{number}]"
        if not isinstance(entry, dict):
            errors.append(f"{label}: entry must be an object")
            continue
        entry_id = entry.get("id")
        if not isinstance(entry_id, str) or not entry_id:
            errors.append(f"{label}: id must be a non-empty string")
            continue
        if entry_id in ids:
            errors.append(f"{label}: duplicate id {entry_id!r} (also entries[{ids[entry_id]}])")
        else:
            ids[entry_id] = number
        by_id[entry_id] = entry
        kind = entry.get("kind")
        if kind not in KINDS:
            errors.append(f"{label} {entry_id!r}: kind must be one of {sorted(KINDS)}")
        relative = _safe_relative(entry.get("path"), f"{label} {entry_id!r}", errors)
        if relative is not None:
            path_key = relative.as_posix()
            if path_key in paths:
                errors.append(f"{label} {entry_id!r}: duplicate path {path_key!r} (also {paths[path_key]!r})")
            else:
                paths[path_key] = entry_id
            relative_paths[entry_id] = relative
            if not (root / relative).exists():
                errors.append(f"{label} {entry_id!r}: missing path {path_key!r}")
        status = entry.get("status")
        if status in ACTIVE_STATUSES:
            if not isinstance(entry.get("owner"), str) or not entry["owner"].strip():
                errors.append(f"{label} {entry_id!r}: active entry requires owner")
            criteria = entry.get("review_criteria")
            if not isinstance(criteria, list) or not criteria or not all(isinstance(item, str) and item.strip() for item in criteria):
                errors.append(f"{label} {entry_id!r}: active entry requires non-empty review_criteria")
            review_date = _date(entry.get("review_date"), f"{label} {entry_id!r}.review_date")
            if review_date is None:
                errors.append(f"{label} {entry_id!r}: active entry requires review_date YYYY-MM-DD")
            elif review_date > as_of:
                errors.append(f"{label} {entry_id!r}: review_date {review_date.isoformat()} is after as_of {as_of.isoformat()}")
            elif (as_of - review_date).days > max_age:
                errors.append(f"{label} {entry_id!r}: stale review_date {review_date.isoformat()} at as_of {as_of.isoformat()} (max age {max_age}d)")
        source_id = entry.get("source_id")
        if kind in {"projection", "empirical-memory"} and (not isinstance(source_id, str) or not source_id):
            errors.append(f"{label} {entry_id!r}: {kind} entry requires source_id")

    for entry_id, entry in by_id.items():
        source_id = entry.get("source_id")
        if source_id is None:
            continue
        source = by_id.get(source_id)
        if source is None:
            errors.append(f"entry {entry_id!r}: source_id {source_id!r} does not name an entry")
        elif source.get("kind") != "source":
            errors.append(f"entry {entry_id!r}: source_id {source_id!r} must name a source entry")
    for entry_id, relative in relative_paths.items():
        entry = by_id[entry_id]
        if entry.get("primary") is True:
            if entry.get("kind") != "source":
                errors.append(f"entry {entry_id!r}: only source entries may be primary")
            errors.extend(_markdown_link_errors(root, entry, relative))

    return sorted(set(errors))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root (default: script parent)")
    parser.add_argument("--index", type=Path, default=DEFAULT_INDEX, help="knowledge index path, relative to root")
    parser.add_argument("--as-of", dest="as_of", type=lambda value: dt.date.fromisoformat(value), help="date used for freshness checks (YYYY-MM-DD)")
    parser.add_argument("--max-age-days", type=int, help="maximum active-document review age")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    errors = check_index(root, args.index, as_of=args.as_of, max_age_days=args.max_age_days)
    if errors:
        for error in errors:
            print(f"check-knowledge: ERROR: {error}", file=sys.stderr)
        return 1
    print(f"check-knowledge: OK ({(root / args.index).as_posix()})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
