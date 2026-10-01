#!/usr/bin/env python3
"""Cross-workflow release-safety / drift checker.

Adapted from the vendored upstream release pipeline's own cross-workflow
safety checker (recommended action #1) and RE-DERIVED against canon's own
two-workflow layout: canon inherits the upstream project's exact "two YAML
matrices that must stay
identical, with no native cross-file consistency check" risk the moment
`.github/workflows/build-native.yml` (test matrix) and `publish.yml`
(release matrix) both declare a per-target `matrix.settings` table.

This script fails CI the instant one matrix (or the shared env, or a
platform package's `package.json` name, or `@journeykit/canon`'s
`optionalDependencies` set) drifts from the others — closing the drift
hole BEFORE a broken release rather than discovering it via a failed
publish.

It also performs deterministic supply-chain checks over tracked textual release
surfaces, excluding binary assets and fixture artifacts. External GitHub
Actions MUST be pinned to full commit SHAs; downloaded executables MUST
have a nearby SHA-256 verification before
extraction or execution; Compose images MUST use digest references; and
dependency installs MUST use a lockfile-safe mode. Local actions/workflows
and human-readable version comments remain allowed.

Dependency-free by design (stdlib `argparse`/`json`/`re`/`sys`/`pathlib`
only — no PyYAML, no `yq`), so it runs as the FIRST step of both workflows
on a bare runner. YAML matrix rows are extracted with an
indentation-aware line scanner (the upstream project's own no-PyYAML
approach), not a full parser: it only needs the handful of scalar fields
each `settings:` row carries.

Run `python3 scripts/check-release-workflow-safety.py --help` for the
complete list of checks. The fixture-only supply-chain API is intentionally
small (`supply_chain_errors`) so its reject/accept shapes can be tested
without a Git checkout or network access.

Coverage note (the upstream lesson): a
drift checker only guards what it is told to compare. This one compares
the build/publish matrices, the shared `CARGO_*` env, and the package
manifests; it deliberately does NOT assert `setup-bun` version parity
(canon pins bun in one workflow only today) — add that comparison here
if a second workflow ever pins its own bun version.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BUILD_WF = ROOT / ".github/workflows/build-native.yml"
PUBLISH_WF = ROOT / ".github/workflows/publish.yml"

# The env vars both workflows MUST declare identically (a mismatched
# CARGO_INCREMENTAL/… between the test build and the release build means
# the release binary was built under different flags than CI proved).
REQUIRED_ENV = ("CARGO_TERM_COLOR", "CARGO_INCREMENTAL")

# The scalar fields every `matrix.settings` row carries that must agree
# across the two workflows (publish additionally carries `package_name`,
# checked separately against the manifests).
SHARED_MATRIX_FIELDS = ("host", "target", "package_dir")


def read(path: pathlib.Path) -> str:
    if not path.is_file():
        fail(f"missing workflow file: {path.relative_to(ROOT)}")
    return path.read_text(encoding="utf-8")


def top_level_env(text: str) -> dict[str, str]:
    """The top-level `env:` block's key: value pairs (2-space indent)."""
    out: dict[str, str] = {}
    lines = text.splitlines()
    in_env = False
    for line in lines:
        if line.rstrip() == "env:":
            in_env = True
            continue
        if in_env:
            m = re.match(r"^  (\w+): (.+)$", line)
            if m:
                out[m.group(1)] = m.group(2).strip()
            elif line and not line.startswith("  "):
                break  # dedented out of the env block
    return out


def matrix_rows(text: str) -> list[dict[str, str]]:
    """Every `matrix.settings` row as a {field: value} dict.

    Indentation-aware scan: each row starts at a `- host:` list item;
    subsequent deeper-indented `key: value` lines belong to that row
    until the next `- ` or a dedent.
    """
    rows: list[dict[str, str]] = []
    current: dict[str, str] | None = None
    row_indent = None
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip())
        item = re.match(r"^\s*- (\w+): (.+)$", line)
        if item:
            # a new list item begins a new row
            if current is not None:
                rows.append(current)
            current = {}
            row_indent = indent
            current[item.group(1)] = item.group(2).strip().strip('"')
            continue
        if current is not None and row_indent is not None and indent > row_indent:
            kv = re.match(r"^\s*(\w+): (.+)$", line)
            if kv:
                current[kv.group(1)] = kv.group(2).strip().strip('"')
        elif current is not None and indent <= row_indent:
            rows.append(current)
            current = None
            row_indent = None
    if current is not None:
        rows.append(current)
    # keep only rows that actually look like build-matrix settings
    return [r for r in rows if "target" in r and "host" in r]


ERRORS: list[str] = []


def check(cond: bool, msg: str) -> None:
    if not cond:
        ERRORS.append(msg)


def fail(msg: str) -> None:
    print(f"check-release-workflow-safety: FATAL: {msg}", file=sys.stderr)
    sys.exit(2)


FULL_SHA = re.compile(r"^[0-9a-fA-F]{40}$")
SHA256 = re.compile(r"@sha256:[0-9a-fA-F]{64}(?:$|[\s\"'])")
ACTION_USE = re.compile(r"^\s*(?:-\s*)?uses:\s*([^\s#]+)")
DOWNLOAD_TOOL = re.compile(r"\b(curl|wget)\b")
CHECKSUM_TOOL = re.compile(
    r"\b(?:sha256sum|shasum\s+-a\s+256|openssl\s+dgst\s+-sha256)\b"
)
INSTALL_COMMAND = re.compile(
    r"\b(?:npm|bun|pnpm|yarn)\s+(?:install|i|ci|add|update|upgrade)\b"
)
FLOATING_CHANNEL = re.compile(
    r"@[^\s\"']*(?:latest|stable|next|beta|canary|nightly)(?:[\s\"']|$)",
    re.IGNORECASE,
)
FLOATING_RANGE = re.compile(r"@[^\s\"']*(?:[\^~*]|>=|<=|>|<|[0-9]+[.]x)(?:[\s\"']|$)")
LOCKFILE_FLAG = re.compile(r"(?:--frozen-lockfile|--locked|--immutable)\b")

# Never decode binary assets, even when their names resemble release surfaces.
BINARY_SUFFIXES = frozenset(
    {
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".bmp", ".tiff",
        ".parquet", ".wasm", ".zip", ".tar", ".gz", ".bz2", ".xz", ".7z",
        ".bin", ".exe", ".dll", ".so", ".dylib", ".a", ".o", ".pdf",
        ".woff", ".woff2", ".ttf", ".otf", ".mp3", ".mp4", ".wav",
        ".sqlite", ".db", ".pyc",
    }
)


EXACT_SEMVER = re.compile(
    r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$"
)


def _has_floating_package_spec(command: str) -> bool:
    """Reject partial/range/tag package versions, while allowing scopes."""
    for token in re.findall(r"""[^\s;&|"'`]+""", command):
        if token.startswith("-") or "@" not in token:
            continue
        if token.startswith("@"):
            at = token.find("@", 1)
        else:
            at = token.rfind("@")
        if at <= 0 or at + 1 >= len(token):
            continue
        version = token[at + 1 :].rstrip(",)")
        if version.startswith(("file:", "git+", "http:", "https:")):
            continue
        if not EXACT_SEMVER.fullmatch(version):
            return True
    return False


def _source_line(line: str) -> str:
    """Return a source line with full-line and trailing comments removed."""
    if line.lstrip().startswith("#"):
        return ""
    return line.split("#", 1)[0]


def _logical_lines(lines: list[str]) -> list[tuple[int, str]]:
    """Join shell-style continuation lines while retaining the first line number."""
    out: list[tuple[int, str]] = []
    index = 0
    while index < len(lines):
        line = _source_line(lines[index])
        start = index
        while line.rstrip().endswith("\\") and index + 1 < len(lines):
            index += 1
            line = f"{line.rstrip()[:-1]} {_source_line(lines[index]).strip()}"
        out.append((start, line))
        index += 1
    return out


def _relative(path: pathlib.Path, root: pathlib.Path) -> str:
    return str(path.relative_to(root))


def _is_workflow(path: pathlib.Path) -> bool:
    return (
        len(path.parts) >= 3
        and path.parts[0:2] == (".github", "workflows")
        and path.suffix in {".yml", ".yaml"}
    )


def _is_compose(path: pathlib.Path) -> bool:
    name = path.name.lower()
    return (
        path.suffix in {".yml", ".yaml"}
        and ("compose" in name or name in {"docker.yml", "docker.yaml"})
    )


def _is_release_script(path: pathlib.Path) -> bool:
    """Select tracked root scripts, excluding checker tests and fixture data."""
    if not path.parts or path.parts[0] != "scripts":
        return False
    if any(part in {"test-fixtures", "fixtures"} for part in path.parts):
        return False
    if path.name.startswith("test-") or path.name == pathlib.Path(__file__).name:
        return False
    return path.suffix in {".sh", ".bash", ".py", ".js", ".ts"}


def _scan_action_refs(path: pathlib.Path, text: str, root: pathlib.Path) -> list[str]:
    errors: list[str] = []
    for number, line in enumerate(text.splitlines(), 1):
        source = _source_line(line)
        match = ACTION_USE.match(source)
        if not match:
            continue
        ref = match.group(1).strip("\"'")
        # Local actions and reusable workflows are resolved from this checkout;
        # they have no external mutable ref to pin.
        if ref.startswith("./") or ref.startswith("../"):
            continue
        at = ref.rfind("@")
        revision = ref[at + 1 :] if at >= 0 else ""
        if at < 0 or not FULL_SHA.fullmatch(revision):
            errors.append(
                f"{_relative(path, root)}:{number}: GitHub uses ref {ref!r} "
                "must be a full 40-hex commit SHA (version comments are allowed)"
            )
    return errors


def _download_target(command: str) -> str | None:
    match = re.search(
        r"(?:--output(?:=|\s+)|--output-document(?:=|\s+)|-o\s+|-O\s+)"
        r"[\"']?([^\\\s\"';&|]+)",
        command,
    )
    if match:
        return match.group(1)
    # Redirection is also a downloaded file, while a pipe is an executable
    # download and deliberately has no acceptable target-less form.
    match = re.search(r">\s*[\"']?([^\\\s\"';&|]+)", command)
    return match.group(1) if match else None


def _download_is_executable(command: str) -> bool:
    return "|" in command or bool(
        re.search(r"\b(?:chmod\s+\+x|exec|source)\b", command)
    )


def _checksum_mentions_target(line: str, target: str | None) -> bool:
    if target is None or re.search(r"\bsha256sum\s+-c\b", line):
        return True
    basename = pathlib.PurePosixPath(target.strip("\"'")).name
    return target in line or (basename and basename in line)


def _checksum_mentions_target(line: str, target: str | None) -> bool:
    if target is None:
        return True
    basename = pathlib.PurePosixPath(target.strip("\"'")).name
    return target in line or (basename and basename in line)


def _has_nearby_checksum(lines: list[str], start: int, target: str | None) -> bool:
    """Require verification after download and before first use of its artifact."""
    end = min(len(lines), start + 24)
    for index in range(start + 1, end):
        line = _source_line(lines[index])
        if not line:
            continue
        if CHECKSUM_TOOL.search(line) and _checksum_mentions_target(line, target):
            return True
        if target and target in line and re.search(
            r"\b(?:unzip|tar|7z|gzip|gunzip|unxz|chmod|install|exec|source)\b",
            line,
        ):
            return False
    return False


def _scan_downloads(path: pathlib.Path, text: str, root: pathlib.Path) -> list[str]:
    errors: list[str] = []
    lines = text.splitlines()
    for number, command in _logical_lines(lines):
        if not DOWNLOAD_TOOL.search(command) or not re.search(r"https?://", command):
            continue
        target = _download_target(command)
        executable = _download_is_executable(command)
        # A curl health probe has neither an output target nor an executable
        # pipe. wget without -O still writes a file and is therefore checked.
        tool = DOWNLOAD_TOOL.search(command)
        if target is None and not executable and tool and tool.group(1) == "curl":
            continue
        # A compact shell command may verify and extract on one line; honor
        # the ordering in that command rather than requiring a new line.
        checksum_at = CHECKSUM_TOOL.search(command)
        extraction_at = re.search(
            r"\b(?:unzip|tar|7z|gzip|gunzip|unxz|chmod|install|exec|source)\b",
            command,
        )
        if checksum_at and (
            extraction_at is None or checksum_at.start() < extraction_at.start()
        ) and _checksum_mentions_target(command, target):
            continue
        if not _has_nearby_checksum(lines, number, target):
            errors.append(
                f"{_relative(path, root)}:{number + 1}: {tool.group(1)} download "
                "must have a nearby SHA-256 verification before extraction or execution"
            )
    return errors


def _scan_compose_images(path: pathlib.Path, text: str, root: pathlib.Path) -> list[str]:
    errors: list[str] = []
    for number, line in enumerate(text.splitlines(), 1):
        source = _source_line(line)
        match = re.match(r"^\s*image:\s*([^\s]+)", source)
        if match and not SHA256.search(match.group(1)):
            errors.append(
                f"{_relative(path, root)}:{number}: Compose image {match.group(1)!r} "
                "must use an @sha256: digest"
            )
    return errors


def _scan_installs(path: pathlib.Path, text: str, root: pathlib.Path) -> list[str]:
    errors: list[str] = []
    lines = text.splitlines()
    for number, command in _logical_lines(lines):
        source = command.strip()
        match = INSTALL_COMMAND.search(source)
        if not match:
            continue
        if (
            FLOATING_CHANNEL.search(source)
            or FLOATING_RANGE.search(source)
            or _has_floating_package_spec(source)
        ):
            errors.append(
                f"{_relative(path, root)}:{number + 1}: package install uses a "
                "floating npm/channel version"
            )
        # npm ci is explicitly lockfile-backed. Global tool upgrades have no
        # project lockfile to honor, but still undergo the floating-channel
        # check above.
        global_install = bool(re.search(r"(?:^|\s)(?:-g|--global)(?:\s|$)", source))
        is_ci = bool(re.search(r"\b(?:npm\s+ci)\b", source))
        if not global_install and not is_ci and not LOCKFILE_FLAG.search(source):
            errors.append(
                f"{_relative(path, root)}:{number + 1}: package install must use "
                "--frozen-lockfile, --locked, or an equivalent lockfile-safe mode"
            )
    return errors


def supply_chain_errors(
    root: pathlib.Path, tracked_files: list[pathlib.Path] | list[str]
) -> list[str]:
    """Scan only the supplied tracked paths; deterministic and network-free.

    ``tracked_files`` is explicit to make fixture tests independent of Git.
    Production callers pass the output of ``git ls-files``.
    """
    errors: list[str] = []
    for item in sorted(tracked_files, key=str):
        relative = pathlib.Path(item)
        if relative.suffix.lower() in BINARY_SUFFIXES:
            continue
        if not (
            _is_workflow(relative)
            or _is_compose(relative)
            or _is_release_script(relative)
        ):
            continue
        path = relative if relative.is_absolute() else root / relative
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as error:
            errors.append(f"{_relative(path, root)}: cannot read as UTF-8: {error}")
            continue
        if _is_workflow(relative):
            errors.extend(_scan_action_refs(path, text, root))
            errors.extend(_scan_downloads(path, text, root))
            errors.extend(_scan_installs(path, text, root))
        elif _is_compose(relative):
            errors.extend(_scan_compose_images(path, text, root))
        elif _is_release_script(relative):
            errors.extend(_scan_downloads(path, text, root))
            errors.extend(_scan_installs(path, text, root))
    return errors


def tracked_paths(root: pathlib.Path) -> list[pathlib.Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=root,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        return []
    return [
        pathlib.Path(item)
        for item in result.stdout.decode("utf-8").split("\0")
        if item
    ]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Check release matrix/package drift and deterministic supply-chain "
            "policy for tracked workflows, Compose files, and release scripts."
        ),
        epilog=(
            "Supply-chain policy: external uses: refs require 40-hex commit "
            "SHAs; curl/wget executable downloads require nearby SHA-256 "
            "verification; Compose images require @sha256: digests; npm/bun/"
            "pnpm/yarn installs require frozen lockfiles and reject floating "
            "channels. Local actions/workflows are allowed."
        ),
    )
    parser.parse_args(argv)
    ERRORS.clear()
    supply_files = tracked_paths(ROOT)
    if not supply_files:
        ERRORS.append("cannot list tracked files for supply-chain checks")
    else:
        ERRORS.extend(supply_chain_errors(ROOT, supply_files))

    build_text = read(BUILD_WF)
    publish_text = read(PUBLISH_WF)

    # 1. Shared env parity.
    build_env = top_level_env(build_text)
    publish_env = top_level_env(publish_text)
    for key in REQUIRED_ENV:
        check(key in build_env, f"build-native.yml env missing {key}")
        check(key in publish_env, f"publish.yml env missing {key}")
        if key in build_env and key in publish_env:
            check(
                build_env[key] == publish_env[key],
                f"env {key} drift: build-native={build_env[key]!r} vs publish={publish_env[key]!r}",
            )

    # 2. Matrix parity on the shared fields.
    build_rows = matrix_rows(build_text)
    publish_rows = matrix_rows(publish_text)
    check(len(build_rows) >= 1, "build-native.yml has no matrix rows")
    check(
        len(build_rows) == len(publish_rows),
        f"matrix row count drift: build-native={len(build_rows)} vs publish={len(publish_rows)}",
    )

    def key_of(row: dict[str, str]) -> tuple[str, ...]:
        return tuple(row.get(f, "<missing>") for f in SHARED_MATRIX_FIELDS)

    build_keys = sorted(key_of(r) for r in build_rows)
    publish_keys = sorted(key_of(r) for r in publish_rows)
    check(
        build_keys == publish_keys,
        f"matrix (host/target/package_dir) drift:\n  build-native={build_keys}\n  publish     ={publish_keys}",
    )

    # 3. Every publish row's package_name matches its platform package's
    #    package.json `name`, and its package_dir exists.
    platform_names: set[str] = set()
    for row in publish_rows:
        pkg_dir = row.get("package_dir")
        pkg_name = row.get("package_name")
        check(pkg_name is not None, f"publish matrix row {row.get('target')} lacks package_name")
        if not pkg_dir:
            continue
        manifest = ROOT / "packages" / pkg_dir / "package.json"
        if not manifest.is_file():
            ERRORS.append(f"publish row {row.get('target')}: packages/{pkg_dir}/package.json is missing")
            continue
        actual = json.loads(manifest.read_text(encoding="utf-8")).get("name")
        check(
            actual == pkg_name,
            f"package name drift for {pkg_dir}: matrix={pkg_name!r} vs package.json={actual!r}",
        )
        if pkg_name:
            platform_names.add(pkg_name)

    # 4. @journeykit/canon's optionalDependencies keys are EXACTLY the set of
    #    platform package names the publish matrix ships (no orphan dep,
    #    no unshipped platform).
    cli_manifest = ROOT / "packages/cli/package.json"
    if cli_manifest.is_file():
        opt = json.loads(cli_manifest.read_text(encoding="utf-8")).get("optionalDependencies", {})
        opt_keys = set(opt.keys())
        check(
            opt_keys == platform_names,
            f"@journeykit/canon optionalDependencies drift:\n  optionalDependencies={sorted(opt_keys)}\n  publish matrix names ={sorted(platform_names)}",
        )
    else:
        ERRORS.append("packages/cli/package.json is missing")

    # 5. Every platform package the publish matrix ships is actually
    #    `npm publish`ed by a step in publish.yml (no built-but-unpublished
    #    platform, and no publish of a package not in the matrix).
    # A publish step is a `working-directory: packages/<dir>` whose step
    # body (up to the next step / working-directory) contains an `npm
    # publish` invocation — matches both a bare `run: npm publish` and a
    # `run: |` multiline block with a guarded/idempotent `npm publish`.
    published: set[str] = set()
    pub_lines = publish_text.splitlines()
    for i, line in enumerate(pub_lines):
        m = re.match(r"\s*working-directory: packages/([\w./-]+)\s*$", line)
        if not m:
            continue
        pkg_dir = m.group(1)
        for follow in pub_lines[i + 1:]:
            if re.match(r"\s*- name:", follow) or re.match(r"\s*working-directory:", follow):
                break
            if "npm publish" in follow:
                published.add(pkg_dir)
                break
    expected_dirs = {r.get("package_dir") for r in publish_rows if r.get("package_dir")}
    expected_dirs.add("cli")  # the wrapper is always published
    check(
        published == expected_dirs,
        f"npm-publish step drift:\n  publishes={sorted(published)}\n  expected ={sorted(expected_dirs)}",
    )

    # 6. Every package.json in the repo carries the Cargo workspace version.
    #    publish.yml rewrites the three PUBLISHED manifests from
    #    `[workspace.package].version` at release time, so a stale committed
    #    version never reaches npm — which is exactly why it can rot
    #    unnoticed in the tree (found at v0.2.1 with all three still on
    #    0.1.0, and two more numbers across the private packages). A
    #    committed manifest that disagrees with the tag being cut is a
    #    reader-facing lie, so it is asserted here rather than left to the
    #    rewrite. Private packages are included deliberately: one repo, one
    #    version, one rule to check.
    cargo_text = read(ROOT / "Cargo.toml")
    m = re.search(r'\[workspace\.package\](?:(?!^\[).|\n)*?^version = "([^"]+)"', cargo_text, re.MULTILINE)
    if not m:
        fail("Cargo.toml has no [workspace.package] version")
    workspace_version = m.group(1)
    # Scan from ROOT, not just packages/ — the first cut missed tracked
    # `examples/platformer/package.json`, so the rule claimed "every
    # package.json" while a manifest sat at 0.0.0 and the check passed.
    # Excluded paths are dependency/build output, never source.
    excluded = {"node_modules", "dist", ".astro", "target", "vendors", ".git"}
    manifests = sorted(
        p for p in ROOT.rglob("package.json") if excluded.isdisjoint(p.parts)
    )
    check(bool(manifests), "no package.json found to version-check — the scan is broken, not the tree")
    for manifest in manifests:
        rel = manifest.relative_to(ROOT)
        try:
            found = json.loads(manifest.read_text()).get("version")
        except (OSError, json.JSONDecodeError) as e:
            # An unreadable manifest must FAIL, never pass vacuously.
            ERRORS.append(f"cannot read {rel} for the version check: {e}")
            continue
        check(
            found == workspace_version,
            f"version drift in {rel}: package.json={found!r} vs Cargo.toml [workspace.package]={workspace_version!r}",
        )

    # 7. Repo-root hygiene. An agent-authored patch fragment landed at the
    #    repo root in s43 round 7 and was committed by a `git add -A`: a file
    #    literally named `ord.split(.-.).all(digits),|true => ...`. Nothing
    #    caught it — not the gate, not this checker, not any test — and it
    #    would have shipped in the v0.5.0 source archive. The root is a small,
    #    slow-moving set, so an allow-list is cheap and the failure is loud.
    #    A NEW legitimate root file is one line here; a stray one is a red
    #    build.
    allowed_root = {
        "ARCHITECTURE.md",
        ".gitignore",
        "Cargo.lock",
        "Cargo.toml",
        "README.md",
        "bun.lock",
        "canon.yaml",
        "docker-compose.yml",
        "package.json",
    }
    # `check=False` + reading only stdout would pass VACUOUSLY: in a source
    # tarball with git installed, `git ls-files` exits 128 with empty stdout,
    # so `tracked_root` would be empty and every stray file would look absent.
    # A hygiene check that reports success when it could not look is worse
    # than no check at all.
    ls = subprocess.run(
        ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=False
    )
    if ls.returncode != 0:
        ERRORS.append(
            "cannot list tracked files for the repo-root check: "
            f"`git ls-files` exited {ls.returncode} ({ls.stderr.strip() or 'no stderr'})"
        )
        tracked_root: set[str] = set()
    else:
        tracked_root = {
            line for line in ls.stdout.splitlines() if line and "/" not in line
        }
    for stray in sorted(tracked_root - allowed_root):
        ERRORS.append(
            f"unexpected tracked file at the repo root: {stray!r} — "
            "delete it, or add it to `allowed_root` if it is genuinely part of the repo"
        )

    if ERRORS:
        print("check-release-workflow-safety: DRIFT DETECTED\n", file=sys.stderr)
        for e in ERRORS:
            print(f"  - {e}", file=sys.stderr)
        return 1
    print("check-release-workflow-safety: OK — build/publish matrices, env, and manifests are coherent")
    return 0


if __name__ == "__main__":
    sys.exit(main())
