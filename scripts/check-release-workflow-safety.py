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

It also requires every `npm publish` step of publish.yml to split prereleases
from stable versions: a hyphenated version (`0.35.0-rc.1`) publishes with
`--tag next`, never `latest`, and a stable version keeps the plain command
(`prerelease_publish_errors`).

Coverage note (the upstream lesson): a
drift checker only guards what it is told to compare. This one compares
the build/publish matrices, the shared `CARGO_*` env, and the package
manifests, and that every workflow pinning `setup-bun` pins the same
version (test.yml proves the launcher under the bun publish.yml ships it
with).
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
REQUIRED_ENV = ("CARGO_TERM_COLOR", "CARGO_INCREMENTAL", "MACOSX_DEPLOYMENT_TARGET")

# The macOS deployment target both workflows declare (the binary's `minos`, checked by check-release-binary.py).
MACOS_DEPLOYMENT_TARGET = "15.0"

# The only runner labels a release job may use: the newest image, its label written down, never `-latest`.
ALLOWED_RUNNERS = ("macos-26", "ubuntu-26.04", "ubuntu-26.04-arm", "windows-2025")

# The scalar fields every `matrix.settings` row carries that must agree
# across the two workflows (publish additionally carries `package_name`,
# checked separately against the manifests).
SHARED_MATRIX_FIELDS = ("host", "target", "package_dir", "lg_page", "alpine_image")


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


DASHBOARD_BUILD_CMD = "bun run --cwd packages/dashboard build"


def _job_block(text: str, job: str) -> str:
    """The lines of a top-level job (2-space-indented key under `jobs:`)."""
    lines = text.splitlines()
    start = next((i for i, line in enumerate(lines) if line == f"  {job}:"), None)
    if start is None:
        return ""
    end = len(lines)
    for i in range(start + 1, len(lines)):
        if re.match(r"^  [\w-]+:\s*$", lines[i]):
            end = i
            break
    return "\n".join(lines[start:end])


def _step_block(job: str, step: str) -> str:
    """The text of the step named `step` inside a job block."""
    lines = job.splitlines()
    start = next((i for i, line in enumerate(lines) if line.strip() == f"- name: {step}"), None)
    if start is None:
        return ""
    end = len(lines)
    for i in range(start + 1, len(lines)):
        if re.match(r"^\s*- (name|uses):", lines[i]):
            end = i
            break
    return "\n".join(lines[start:end])


def dashboard_embed_errors(text: str, workflow: str) -> list[str]:
    """`canon dashboard` runs in any repo because every platform binary embeds
    the same dashboard app (0.25 D2): the app is built by exactly one job,
    and each build row downloads that build and hands it to `cargo build`
    as CANON_DASHBOARD_DIST. A row that skips the hand-off ships a binary
    whose dashboard says "not built".
    """
    errors: list[str] = []
    builds = text.count(DASHBOARD_BUILD_CMD)
    if builds != 1:
        errors.append(f"{workflow}: the dashboard app must be built by exactly one job ({DASHBOARD_BUILD_CMD!r} appears {builds} times)")
    build = _job_block(text, "build")
    if not build:
        return errors + [f"{workflow}: no `build` job"]
    needs = re.search(r"^    needs:\s*(.+)$", build, re.MULTILINE)
    if not needs or "dashboard" not in needs.group(1):
        errors.append(f"{workflow}: the build job must `needs` the dashboard job")
    if "name: dashboard-app" not in build:
        errors.append(f"{workflow}: the build job must download the `dashboard-app` artifact")
    cargo = _step_block(build, "Build canon-cli binary")
    if "CANON_DASHBOARD_DIST:" not in cargo or "cargo build" not in cargo:
        errors.append(f"{workflow}: the `Build canon-cli binary` step must set CANON_DASHBOARD_DIST for `cargo build`")
    dashboard = _job_block(text, "dashboard")
    if "name: dashboard-app" not in dashboard or "bun install --frozen-lockfile" not in dashboard:
        errors.append(f"{workflow}: the dashboard job must install with the frozen lockfile and upload `dashboard-app`")
    return errors


STABLE_PUBLISH = 'npm publish "$TARBALL" --provenance --access public'
PRERELEASE_PUBLISH = 'npm publish "$TARBALL" --provenance --access public --tag next'
PRERELEASE_GUARD = re.compile(r'^(?:el)?if \[\[ "\$VERSION" == \*-\* \]\]; then$')
GITHUB_RELEASE = re.compile(r"\bgh\s+release\b|action-gh-release|create-release")
PRERELEASE_TEST = re.compile(r"\*-\*|contains\([^)]*'-'[^)]*\)")


def _steps(job: str) -> list[str]:
    """Every step of a job block, each as its own text."""
    steps: list[str] = []
    current: list[str] = []
    for line in job.splitlines():
        if re.match(r"^\s*- (name|uses|run):", line):
            if current:
                steps.append("\n".join(current))
            current = []
        current.append(line)
    if current:
        steps.append("\n".join(current))
    return steps


def prerelease_publish_errors(text: str, workflow: str) -> list[str]:
    """A hyphenated version (`0.35.0-rc.1`) is a prerelease and publishes under
    the `next` dist-tag, never `latest`; a stable version keeps the one
    `npm publish "$TARBALL" --provenance --access public` it always ran. Every `npm
    publish` step carries the split, so no package lands on `latest` alone
    (npm refuses a prerelease without `--tag`, and a `--tag latest` would
    defeat the point). A GitHub release, if the workflow creates one, is
    marked prerelease for a hyphenated version.
    """
    errors: list[str] = []
    job = _job_block(text, "publish")
    if not job:
        return [f"{workflow}: no `publish` job"]
    if re.search(r"--tag[ =]latest\b", job):
        errors.append(f"{workflow}: the publish job must never pass `--tag latest`; a stable version publishes with no tag")
    publish_steps = [step for step in _steps(job) if "npm publish" in step]
    if not publish_steps:
        errors.append(f"{workflow}: the publish job has no `npm publish` step")
    for step in publish_steps:
        name = step.splitlines()[0].strip()
        lines = [
            line.strip()
            for line in step.splitlines()
            if line.strip() and not line.strip().startswith("#")
        ]
        publishes = [i for i, line in enumerate(lines) if line.startswith("npm publish")]
        stable = [i for i in publishes if lines[i] == STABLE_PUBLISH]
        pre = [i for i in publishes if lines[i] == PRERELEASE_PUBLISH]
        if len(stable) != 1 or len(pre) != 1 or len(publishes) != 2:
            errors.append(
                f"{workflow}: `{name}` must run exactly `{STABLE_PUBLISH}` and `{PRERELEASE_PUBLISH}`, "
                f"found {[lines[i] for i in publishes]}"
            )
            continue
        if not PRERELEASE_GUARD.match(lines[pre[0] - 1]):
            errors.append(
                f"{workflow}: `{name}`: `{PRERELEASE_PUBLISH}` must sit directly under "
                "`elif [[ \"$VERSION\" == *-* ]]; then` (a hyphenated version is a prerelease)"
            )
        if lines[stable[0] - 1] != "else" or stable[0] < pre[0]:
            errors.append(
                f"{workflow}: `{name}`: `{STABLE_PUBLISH}` must be the `else` branch after the prerelease branch"
            )
        if not re.search(r"^\s*VERSION:\s*\S", step, re.MULTILINE):
            errors.append(f"{workflow}: `{name}` must set VERSION in its env, or the prerelease test reads nothing")
    for step in _steps(text):
        if GITHUB_RELEASE.search(step) and not (
            "prerelease" in step and PRERELEASE_TEST.search(step)
        ):
            name = step.splitlines()[0].strip()
            errors.append(f"{workflow}: `{name}` creates a GitHub release and must mark a hyphenated version prerelease")
    return errors


# ---------------------------------------------------------------------------
# 0.42 release portability: the floor each release binary declares, the four
# packages, the smokes before a publish, the dry run, the time limits.
# Every function takes the text it reads (a workflow, a manifest, a source
# file), so the tests feed it a copy with one rule broken.
# ---------------------------------------------------------------------------

EXPECTED_TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "x86_64-pc-windows-msvc",
)
LG_PAGE_TARGET = "aarch64-unknown-linux-musl"
LG_PAGE = "16"
SMOKE_JOBS = {"smoke-binary": "binary", "smoke-install": "install", "smoke-network": "network"}
BINARY_CHECK_STEP = "Check the binary keeps the floor of its platform"
RUNNER_REF = re.compile(r"^\$\{\{\s*matrix\.settings\.(\w+)\s*\}\}$")


def _job_names(text: str) -> list[str]:
    """The keys directly under `jobs:` (2-space indent), in order."""
    names: list[str] = []
    in_jobs = False
    for line in text.splitlines():
        if line.rstrip() == "jobs:":
            in_jobs = True
            continue
        if in_jobs:
            match = re.match(r"^  ([\w-]+):\s*$", line)
            if match:
                names.append(match.group(1))
            elif line and not line.startswith(" "):
                break
    return names


def _scan_rows(text: str) -> list[dict[str, str]]:
    """Every `- key: value` list item with its deeper `key: value` lines, as a dict (matrix rows, and steps)."""
    rows: list[dict[str, str]] = []
    current: dict[str, str] | None = None
    row_indent = 0
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip())
        item = re.match(r"^\s*- (\w+): (.+)$", line)
        if item:
            if current is not None:
                rows.append(current)
            current = {item.group(1): item.group(2).strip().strip('"')}
            row_indent = indent
            continue
        if current is not None and indent > row_indent:
            kv = re.match(r"^\s*(\w+): (.+)$", line)
            if kv:
                current[kv.group(1)] = kv.group(2).strip().strip('"')
        elif current is not None:
            rows.append(current)
            current = None
    if current is not None:
        rows.append(current)
    return rows


def _field(job: str, key: str) -> str | None:
    """A job's own scalar key (4-space indent)."""
    match = re.search(rf"^    {re.escape(key)}:[ \t]*(.*?)[ \t]*$", job, re.MULTILINE)
    return match.group(1) if match else None


def _needs(job: str) -> set[str]:
    """The jobs a job waits for: `needs: x`, `needs: [x, y]` or a block list."""
    lines = job.splitlines()
    for index, line in enumerate(lines):
        match = re.match(r"^    needs:\s*(.*)$", line)
        if not match:
            continue
        value = match.group(1).strip()
        if value.startswith("["):
            return {item.strip() for item in value.strip("[]").split(",") if item.strip()}
        if value:
            return {value}
        found: set[str] = set()
        for follow in lines[index + 1 :]:
            item = re.match(r"^      - (\S+)\s*$", follow)
            if not item:
                break
            found.add(item.group(1))
        return found
    return set()


def runner_label_errors(text: str, workflow: str) -> list[str]:
    """Every job runs on a pinned runner image from ALLOWED_RUNNERS, a matrix row's included: `-latest` follows
    the newest image and moves the floor of a binary with no change here."""
    errors: list[str] = []
    for name in _job_names(text):
        job = _job_block(text, name)
        runs_on = _field(job, "runs-on")
        if runs_on is None:
            errors.append(f"{workflow}: job `{name}` has no `runs-on`")
            continue
        labels: list[str] = []
        reference = RUNNER_REF.match(runs_on)
        if reference:
            key = reference.group(1)
            labels = [row[key] for row in _scan_rows(job) if key in row]
            if not labels:
                errors.append(f"{workflow}: job `{name}` runs on matrix.settings.{key} and no matrix row has it")
        else:
            labels = [runs_on.strip("\"'")]
        for label in labels:
            if label not in ALLOWED_RUNNERS:
                reason = " (`-latest` is the newest image, not a floor)" if label.endswith("-latest") else ""
                errors.append(
                    f"{workflow}: job `{name}` runs on `{label}`{reason}; the allowed runner labels are {', '.join(ALLOWED_RUNNERS)}"
                )
    return errors


def build_row_errors(rows: list[dict[str, str]], workflow: str) -> list[str]:
    """The four build rows: macOS arm64, and Linux x64 and arm64 built as musl targets on their own host
    (no job `container:`; the aarch64 row runs a digest-pinned `rust:alpine` image in a step, see
    `alpine_build_errors`), Windows x64; `lg_page` and `alpine_image` on the aarch64 musl row alone."""
    errors: list[str] = []
    by_target = {row.get("target", ""): row for row in rows}
    for target in EXPECTED_TARGETS:
        if target not in by_target:
            errors.append(f"{workflow}: the build matrix has no row for {target}")
    for row in rows:
        host, target = row.get("host", ""), row.get("target", "")
        if host.startswith("ubuntu-"):
            if not target.endswith("-unknown-linux-musl"):
                errors.append(f"{workflow}: the Linux row {host}/{target} must build a musl target (a static binary has no glibc floor)")
            elif (target.startswith("aarch64"), host.endswith("-arm")) not in {(True, True), (False, False)}:
                errors.append(f"{workflow}: {target} must build on its own architecture: x86_64 on ubuntu-26.04, aarch64 on ubuntu-26.04-arm (found {host})")
        elif host.startswith("macos-"):
            if target != "aarch64-apple-darwin":
                errors.append(f"{workflow}: the macOS row {host} builds {target}; macOS is arm64 only (aarch64-apple-darwin)")
        elif host.startswith("windows-"):
            if target != "x86_64-pc-windows-msvc":
                errors.append(f"{workflow}: the Windows row {host} builds {target}; Windows is x64 only (x86_64-pc-windows-msvc)")
        if "gnu" in target:
            errors.append(f"{workflow}: row {target} is a gnu target; releases are musl (Linux), msvc (Windows) and darwin")
        has_page = "lg_page" in row
        if target == LG_PAGE_TARGET and row.get("lg_page") != LG_PAGE:
            errors.append(f"{workflow}: the {LG_PAGE_TARGET} row must set `lg_page: \"{LG_PAGE}\"` (JEMALLOC_SYS_WITH_LG_PAGE={LG_PAGE}, 64K pages), found {row.get('lg_page')!r}")
        elif target != LG_PAGE_TARGET and has_page:
            errors.append(f"{workflow}: only the {LG_PAGE_TARGET} row sets `lg_page`; {target} has it (x86_64 is 4K pages, other targets have no jemalloc)")
        image = row.get("alpine_image")
        if target == LG_PAGE_TARGET and not (image and re.match(r"^rust:[\w.]+-alpine[\w.]*@sha256:", image)):
            errors.append(f"{workflow}: the {LG_PAGE_TARGET} row must set `alpine_image: rust:<toolchain>-alpine<version>@sha256:<digest>` (Ubuntu's musl-gcc cannot compile jemalloc there), found {image!r}")
        elif target != LG_PAGE_TARGET and image is not None:
            errors.append(f"{workflow}: only the {LG_PAGE_TARGET} row sets `alpine_image`; {target} has it (the other rows build on their host)")
    return errors


def jemalloc_errors(text: str, workflow: str, cargo_text: str, main_text: str) -> list[str]:
    """jemalloc is the allocator of musl builds only: the dependency hangs on `cfg(target_env = "musl")`, the
    global allocator too, and the page size reaches the build through the one `Build canon-cli binary` step."""
    errors: list[str] = []
    deps = re.search(r"^\[dependencies\]\n(.*?)(?=^\[)", cargo_text, re.MULTILINE | re.DOTALL)
    if deps and "tikv-jemallocator" in deps.group(1):
        errors.append("crates/canon-cli/Cargo.toml: tikv-jemallocator is in [dependencies]; it belongs under [target.'cfg(target_env = \"musl\")'.dependencies] only")
    musl = re.search(r"^\[target\.'cfg\(target_env = \"musl\"\)'\.dependencies\]\n(.*?)(?=^\[|\Z)", cargo_text, re.MULTILINE | re.DOTALL)
    if not musl or "tikv-jemallocator" not in musl.group(1):
        errors.append("crates/canon-cli/Cargo.toml: tikv-jemallocator must be a dependency of [target.'cfg(target_env = \"musl\")'.dependencies]")
    if not re.search(r'#\[cfg\(target_env = "musl"\)\]\s*#\[global_allocator\]', main_text):
        errors.append("crates/canon-cli/src/main.rs: the jemalloc #[global_allocator] must sit under #[cfg(target_env = \"musl\")]")
    build = _step_block(_job_block(text, "build"), "Build canon-cli binary")
    if "JEMALLOC_SYS_WITH_LG_PAGE: ${{ matrix.settings.lg_page }}" not in build:
        errors.append(f"{workflow}: the `Build canon-cli binary` step must set `JEMALLOC_SYS_WITH_LG_PAGE: ${{{{ matrix.settings.lg_page }}}}` (the aarch64 musl row's 64K pages)")
    for number, line in enumerate(text.splitlines(), 1):
        source = _source_line(line)
        if "JEMALLOC_SYS_WITH_LG_PAGE" in source and "matrix.settings.lg_page" not in source and "unset" not in source and '[ -n "$JEMALLOC' not in source and "-e JEMALLOC_SYS_WITH_LG_PAGE" not in source:
            errors.append(f"{workflow}:{number}: JEMALLOC_SYS_WITH_LG_PAGE may only be set from the matrix row's `lg_page`")
    return errors


def alpine_build_errors(text: str, workflow: str) -> list[str]:
    """The aarch64 musl row builds in its `alpine_image`: the one `Build canon-cli binary` step reads the row's image
    and runs it with `docker run` (the workspace mounted, the dashboard and the page size passed in), and the
    toolchain inside is checked against rust-toolchain.toml; the host `Install Rust` step skips that row."""
    errors: list[str] = []
    build = _job_block(text, "build")
    step = _step_block(build, "Build canon-cli binary")
    for needle, why in (
        ("ALPINE_IMAGE: ${{ matrix.settings.alpine_image }}", "read the row's `alpine_image` into ALPINE_IMAGE"),
        ('docker run --rm -i', "run the image with `docker run --rm -i`"),
        ('"$ALPINE_IMAGE" sh -s', 'run the image `"$ALPINE_IMAGE"` (the row\'s, nothing else) with `sh -s`'),
        ('-v "$GITHUB_WORKSPACE:$GITHUB_WORKSPACE"', "mount the workspace"),
        ("-e CANON_DASHBOARD_DIST", "pass CANON_DASHBOARD_DIST into the container"),
        ("-e JEMALLOC_SYS_WITH_LG_PAGE", "pass JEMALLOC_SYS_WITH_LG_PAGE into the container"),
        ('"$got" != "$want"', "compare the image's rustc with rust-toolchain.toml"),
        ("cargo build --locked --release -p canon-cli", "run `cargo build --locked --release -p canon-cli`"),
    ):
        if needle not in step:
            errors.append(f"{workflow}: the `Build canon-cli binary` step must {why}")
    install = _step_block(build, "Install Rust")
    if "if: matrix.settings.alpine_image == ''" not in install:
        errors.append(f"{workflow}: the `Install Rust` step must skip the rows that build in an `alpine_image` (`if: matrix.settings.alpine_image == ''`)")
    return errors


def macos_target_errors(env: dict[str, str], workflow: str) -> list[str]:
    value = env.get("MACOSX_DEPLOYMENT_TARGET", "").strip("\"'")
    if value != MACOS_DEPLOYMENT_TARGET:
        return [f"{workflow}: env MACOSX_DEPLOYMENT_TARGET must be \"{MACOS_DEPLOYMENT_TARGET}\" (found {value or 'nothing'}): the macOS binary's minos is the floor"]
    return []


def crt_static_errors(cargo_config: str, workflows: dict[str, str]) -> list[str]:
    """msvc links the C runtime statically from the one cargo config file; a workflow `RUSTFLAGS` would replace it."""
    errors: list[str] = []
    section = re.search(r"^\[target\.[^\]\n]*msvc[^\]\n]*\]\n(.*?)(?=^\[|\Z)", cargo_config, re.MULTILINE | re.DOTALL)
    if not section or "target-feature=+crt-static" not in section.group(1):
        errors.append(".cargo/config.toml: a [target.…msvc…] table must set rustflags with `target-feature=+crt-static`")
    for name, text in workflows.items():
        for number, line in enumerate(text.splitlines(), 1):
            if re.search(r"\w*RUSTFLAGS\w*", _source_line(line)):
                errors.append(f"{name}:{number}: a workflow must not set RUSTFLAGS: it replaces the flags of .cargo/config.toml (the msvc crt-static)")
    return errors


def binary_check_errors(text: str, workflow: str, rows: list[dict[str, str]], checker_text: str) -> list[str]:
    """Every build row reads its binary: the `build` job's check step runs scripts/check-release-binary.py with the row's target,
    and that script knows every row's target."""
    errors: list[str] = []
    build = _job_block(text, "build")
    step = _step_block(build, BINARY_CHECK_STEP)
    if not step:
        errors.append(f"{workflow}: the build job has no step `{BINARY_CHECK_STEP}` (scripts/check-release-binary.py on every row)")
    elif "scripts/check-release-binary.py" not in step or '--target "${{ matrix.settings.target }}"' not in step:
        errors.append(f"{workflow}: step `{BINARY_CHECK_STEP}` must run scripts/check-release-binary.py --target \"${{{{ matrix.settings.target }}}}\"")
    if re.search(r"^\s+if:", step, re.MULTILINE):
        errors.append(f"{workflow}: step `{BINARY_CHECK_STEP}` must run on every row, with no `if:`")
    known = set(re.findall(r'^    "([\w-]+)": \{"format"', checker_text, re.MULTILINE))
    for row in rows:
        if row.get("target") not in known:
            errors.append(f"{workflow}: scripts/check-release-binary.py does not know the target {row.get('target')} of a build row")
    if re.search(r"^\s+container:", build, re.MULTILINE):
        errors.append(f"{workflow}: the build job must not run in a `container:`; the aarch64 musl row runs its image with `docker run` in a step, the other Linux rows link on the host")
    return errors


def launcher_errors(index_ts: str, rows: list[dict[str, str]], manifests: dict[str, dict]) -> list[str]:
    """The launcher resolves linux/x64 and linux/arm64 whatever the host libc (no libc detection), exactly the
    packages the matrix ships; the Linux packages carry no `libc` filter; every package declares its own os/cpu."""
    errors: list[str] = []
    for needle in ("CANON_LIBC", "detectLibcKind", "ldd --version", "glibc", "-linux-gnu"):
        if needle in index_ts:
            errors.append(f"packages/cli/src/index.ts: libc detection is gone from the launcher; `{needle}` is still there")
    function = re.search(r"function resolveTargetPackageName\(\)[^{]*\{(.*?)\n\}\n", index_ts, re.DOTALL)
    if not function:
        errors.append("packages/cli/src/index.ts: no resolveTargetPackageName function")
        return errors
    resolved = set(re.findall(r'"(core-[\w-]+)"', function.group(1)))
    shipped = {row["package_dir"] for row in rows if row.get("package_dir")}
    for missing in sorted(shipped - resolved):
        errors.append(f"packages/cli/src/index.ts: the launcher cannot resolve the matrix package {missing}")
    for extra in sorted(resolved - shipped):
        errors.append(f"packages/cli/src/index.ts: the launcher resolves {extra}, which no matrix row ships")
    for directory, manifest in sorted(manifests.items()):
        if "libc" in manifest:
            errors.append(f"packages/{directory}/package.json: no `libc` filter (a static binary runs on any libc)")
        match = re.fullmatch(r"core-(darwin|linux|win32)-(arm64|x64)", directory)
        if match:
            if manifest.get("os") != [match.group(1)] or manifest.get("cpu") != [match.group(2)]:
                errors.append(
                    f"packages/{directory}/package.json: os/cpu are {manifest.get('os')}/{manifest.get('cpu')}, "
                    f"expected [{match.group(1)!r}]/[{match.group(2)!r}]"
                )
    return errors


def _tokens(text: str, pattern: str) -> set[str]:
    return set(re.findall(pattern, text, re.MULTILINE))


def package_set_errors(
    publish_text: str,
    rows: list[dict[str, str]],
    launcher_name: str,
    bun_lock: str,
    optional_keys: set[str],
) -> list[str]:
    """The platform packages are one set everywhere: build, download, checksum, executable bit, version, pack, the
    pre-flight, the publish steps, `optionalDependencies`, bun.lock. A place that lacks one names it."""
    errors: list[str] = []
    dirs = {row["package_dir"] for row in rows if row.get("package_dir")}
    names = {row["package_name"] for row in rows if row.get("package_name")}
    package = _job_block(publish_text, "package")
    publish = _job_block(publish_text, "publish")

    def compare(place: str, got: set[str], want: set[str]) -> None:
        if got != want:
            errors.append(f"{place}: package set drift: missing {sorted(want - got)}, extra {sorted(got - want)}")

    def step(job: str, name: str, place: str) -> str:
        block = _step_block(job, name)
        if not block:
            errors.append(f"publish.yml: no step `{name}` ({place})")
        return block

    downloads = re.findall(r"name: canon-binary-([\w-]+)\s*\n\s*path: packages/([\w-]+)/bin", package)
    compare("publish.yml package job: binary downloads", {d for _, d in downloads}, dirs)
    by_dir = {row["package_dir"]: row.get("target") for row in rows if row.get("package_dir")}
    for target, directory in downloads:
        if by_dir.get(directory) != target:
            errors.append(f"publish.yml package job: packages/{directory}/bin downloads canon-binary-{target}, the matrix builds {by_dir.get(directory)} for it")
    block = step(package, "Verify binary checksum handoff", "checksum verification")
    compare("publish.yml: checksum verification", _tokens(block, r"cd packages/([\w-]+)/bin && sha256sum -c"), dirs)
    block = step(package, "Stage binaries executable", "executable bit")
    compare("publish.yml: executable bit", _tokens(block, r"chmod \+x packages/([\w-]+)/bin/"), dirs)
    block = step(package, "Apply the workspace version to every package.json", "version")
    version_line = re.search(r"for pkg in (.*); do", block)
    compare("publish.yml: version application", _tokens(version_line.group(1) if version_line else "", r"packages/([\w-]+)"), dirs | {"cli"})
    block = step(package, "Pack every package into tarballs/", "pack")
    pack_line = re.search(r"for pkg in (.*); do", block)
    compare("publish.yml: npm pack", _tokens(pack_line.group(1) if pack_line else "", r"packages/([\w-]+)"), dirs | {"cli"})
    block = _step_block(publish, "Pre-flight npm release state (fresh-release gate + recovery + first-publish token)")
    if not block:
        errors.append("publish.yml: no pre-flight step in the publish job")
    preflight_line = re.search(r"for pkg_dir in (.*); do", block)
    compare("publish.yml: pre-flight", set((preflight_line.group(1) if preflight_line else "").split()), dirs | {"cli"})
    published: list[str] = []
    for item in _steps(publish):
        if "npm publish" in item:
            package_env = re.search(r'^\s*PACKAGE:\s*"?([^"\s]+)"?\s*$', item, re.MULTILINE)
            published.append(package_env.group(1) if package_env else "<no PACKAGE>")
    compare("publish.yml: npm publish steps", set(published), names | {launcher_name})
    if len(published) != len(set(published)):
        errors.append(f"publish.yml: a package is published by two steps: {published}")
    arm = next((row["package_name"] for row in rows if row.get("package_dir") == "core-linux-arm64"), None)
    if published and (published[0] != arm or published[-1] != launcher_name):
        errors.append(
            f"publish.yml: publish order must put {arm} first (its first publish needs the token, so a failure stops before any other "
            f"package goes out) and the launcher {launcher_name} last, found {published}"
        )
    compare("packages/cli optionalDependencies", optional_keys, names)
    compare("bun.lock workspaces", _tokens(bun_lock, r'^    "packages/(core-[\w-]+)": \{'), dirs)
    for name in sorted(dirs | {"cli"}):
        if not (ROOT / "packages" / name / "package.json").is_file():
            errors.append(f"packages/{name}/package.json is missing")
    return errors


IMAGE_KEY = re.compile(r"^\s*(?:-\s*)?((?:[\w-]*_)?(?:image|container|IMAGE|CONTAINER)):[ \t]*(\S+)")


def image_digest_errors(text: str, workflow: str) -> list[str]:
    """An image a workflow names (`image:`, `container:`, `POSTGRES_IMAGE:`) is pinned by digest: a tag moves."""
    errors: list[str] = []
    for number, line in enumerate(text.splitlines(), 1):
        match = IMAGE_KEY.match(_source_line(line))
        if not match:
            continue
        value = match.group(2).strip("\"'")
        if value.startswith("${{") or value in ("|", ">"):
            continue
        if not re.search(r"@sha256:[0-9a-fA-F]{64}$", value):
            errors.append(f"{workflow}:{number}: image {value!r} must be pinned with an @sha256: digest")
    return errors


def _image_family(image: str) -> str:
    name = image.split("@", 1)[0]
    if name.startswith("debian:12") or name.endswith("-bookworm-slim"):
        return "glibc"
    if name.startswith("alpine:") or name.endswith("-alpine"):
        return "musl"
    return "other"


def smoke_errors(publish_text: str, build_text: str, other_texts: dict[str, str]) -> list[str]:
    """Three smokes run in publish.yml, on x64 and arm64 each in a glibc image (debian:12, glibc 2.36: the 0.41
    floor killed it) and a musl one, and `publish` waits for all three; no other workflow has them."""
    errors: list[str] = []
    names = set(_job_names(publish_text))
    publish = _job_block(publish_text, "publish")
    needs = _needs(publish)
    for job_name, mode in SMOKE_JOBS.items():
        if job_name not in names:
            errors.append(f"publish.yml: no `{job_name}` job (the pre-publish smoke of the {mode} path)")
            continue
        job = _job_block(publish_text, job_name)
        if f"release-smoke.sh {mode} " not in job:
            errors.append(f"publish.yml: `{job_name}` must run scripts/release-smoke.sh {mode}")
        if job_name not in needs:
            errors.append(f"publish.yml: the publish job must wait for `{job_name}` (needs: {sorted(needs)})")
        rows = [row for row in _scan_rows(job) if "runner" in row]
        for runner in ("ubuntu-26.04", "ubuntu-26.04-arm"):
            families = {_image_family(row.get("image", "")) for row in rows if row["runner"] == runner}
            if families != {"glibc", "musl"}:
                errors.append(f"publish.yml: `{job_name}` must run on {runner} in a debian:12 (glibc) image and an alpine (musl) image, found {sorted(families)}")
    if "package" not in needs:
        errors.append("publish.yml: the publish job must wait for `package` (the tarballs it publishes)")
    for name, text in {"build-native.yml": build_text, **other_texts}.items():
        for job_name in _job_names(text):
            if job_name in SMOKE_JOBS:
                errors.append(f"{name}: the smoke job `{job_name}` belongs to publish.yml only (not on pull requests or every push)")
        if "release-smoke" in text:
            errors.append(f"{name}: scripts/release-smoke.sh runs from publish.yml only")
    triggers = publish_text.split("\npermissions:", 1)[0]
    if re.search(r"^  (?:pull_request|pull_request_target|schedule):|^    branches:", triggers, re.MULTILINE):
        errors.append("publish.yml: runs on tags and by hand only; its smokes must not run on pull requests or every push")
    for job_name in ("smoke-install", "publish"):
        job = _job_block(publish_text, job_name)
        if job and "name: npm-tarballs" not in job:
            errors.append(f"publish.yml: `{job_name}` must download the `npm-tarballs` that `package` packed")
    if re.search(r"\bnpm pack(?!\w)", "\n".join(_source_line(line) for line in publish.splitlines())):
        errors.append("publish.yml: the publish job must not `npm pack` again; it publishes the tarballs the smoke installed")
    for line in publish_text.splitlines():
        if "npm publish" in _source_line(line) and 'npm publish "$TARBALL"' not in line:
            errors.append(f"publish.yml: `{line.strip()}` must publish the packed tarball (npm publish \"$TARBALL\" ...)")
    return errors


# The GitHub Actions expression subset a publish step's `if:` may use, evaluated the way Actions does
# (null, boolean, number, string; `==` on two different types compares them as numbers).
_COND_TOKEN = re.compile(r"\s*(?:(?P<str>'(?:[^']|'')*')|(?P<num>\d+(?:\.\d+)?)|(?P<op>\|\||&&|==|!=|!|\(|\))|(?P<id>[A-Za-z_][\w.\-]*))")


class _Cond:
    def __init__(self, text: str, context: dict[str, object]) -> None:
        self.context = context
        self.tokens: list[tuple[str, str]] = []
        position = 0
        text = text.strip()
        while position < len(text):
            match = _COND_TOKEN.match(text, position)
            if not match or match.end() == position:
                raise ValueError(f"cannot read `{text[position:]}`")
            kind = match.lastgroup or ""
            self.tokens.append((kind, match.group(kind)))
            position = match.end()
        self.index = 0

    def peek(self) -> tuple[str, str] | None:
        return self.tokens[self.index] if self.index < len(self.tokens) else None

    def take(self) -> tuple[str, str]:
        token = self.peek()
        if token is None:
            raise ValueError("the condition ends early")
        self.index += 1
        return token

    def parse(self) -> object:
        value = self.or_expr()
        leftover = self.peek()
        if leftover is not None:
            raise ValueError(f"unexpected `{leftover[1]}`")
        return value

    def or_expr(self) -> object:
        value = self.and_expr()
        while self.peek() == ("op", "||"):
            self.take()
            right = self.and_expr()
            value = value if _truthy(value) else right
        return value

    def and_expr(self) -> object:
        value = self.eq_expr()
        while self.peek() == ("op", "&&"):
            self.take()
            right = self.eq_expr()
            value = right if _truthy(value) else value
        return value

    def eq_expr(self) -> object:
        value = self.unary()
        while self.peek() in (("op", "=="), ("op", "!=")):
            operator = self.take()[1]
            right = self.unary()
            equal = _loose_equal(value, right)
            value = equal if operator == "==" else not equal
        return value

    def unary(self) -> object:
        if self.peek() == ("op", "!"):
            self.take()
            return not _truthy(self.unary())
        return self.primary()

    def primary(self) -> object:
        kind, text = self.take()
        if (kind, text) == ("op", "("):
            value = self.or_expr()
            if self.take() != ("op", ")"):
                raise ValueError("a `(` is not closed")
            return value
        if kind == "str":
            return text[1:-1].replace("''", "'")
        if kind == "num":
            return float(text)
        if kind == "id":
            if text in ("true", "false"):
                return text == "true"
            if text == "null":
                return None
            if text not in self.context:
                raise ValueError(f"`{text}` is not a context this check knows (github.event_name, inputs.dry_run, github.event.inputs.dry_run)")
            return self.context[text]
        raise ValueError(f"unexpected `{text}`")


def _truthy(value: object) -> bool:
    if value is None:
        return False
    if isinstance(value, bool):
        return value
    if isinstance(value, float):
        return value != 0 and value == value
    return value != ""


def _number(value: object) -> float:
    if value is None:
        return 0.0
    if isinstance(value, bool):
        return 1.0 if value else 0.0
    if isinstance(value, float):
        return value
    text = str(value).strip()
    if text == "":
        return 0.0
    try:
        return float(text)
    except ValueError:
        return float("nan")


def _loose_equal(left: object, right: object) -> bool:
    if type(left) is type(right):
        if isinstance(left, str) and isinstance(right, str):
            return left.lower() == right.lower()
        return left == right
    return _number(left) == _number(right)


DRY_RUN_SCENARIOS = (
    ("a tag push (no inputs)", {"github.event_name": "push", "inputs.dry_run": None, "github.event.inputs.dry_run": None}, True),
    ("a manual run with dry_run=true", {"github.event_name": "workflow_dispatch", "inputs.dry_run": True, "github.event.inputs.dry_run": "true"}, False),
    ("a manual run with dry_run=false", {"github.event_name": "workflow_dispatch", "inputs.dry_run": False, "github.event.inputs.dry_run": "false"}, True),
)


def _step_condition(step: str) -> str | None:
    match = re.search(r"^ {8}if:[ \t]*(.*?)[ \t]*$", step, re.MULTILINE)
    if not match:
        return None
    value = match.group(1)
    wrapped = re.fullmatch(r"\$\{\{\s*(.*?)\s*\}\}", value)
    return wrapped.group(1) if wrapped else value


def dry_run_errors(text: str, workflow: str) -> list[str]:
    """A manual run is a dry run unless told otherwise, and a dry run skips the five `npm publish` steps and nothing else.
    The condition is read as Actions reads it: it must publish on a tag push (no inputs), skip on dry_run=true and publish on false."""
    errors: list[str] = []
    inputs = re.search(r"^      dry_run:\n((?:        .*\n)+)", text, re.MULTILINE)
    if not inputs or not re.search(r"^        type: boolean$", inputs.group(1), re.MULTILINE) or not re.search(r"^        default: true$", inputs.group(1), re.MULTILINE):
        errors.append(f"{workflow}: workflow_dispatch needs a boolean input `dry_run` whose default is true")
    for name in _job_names(text):
        job = _job_block(text, name)
        job_if = _field(job, "if")
        if job_if is not None and "dry_run" in job_if:
            errors.append(f"{workflow}: job `{name}` has an `if:` on dry_run; only the `npm publish` steps may be skipped by it")
        publishes = 0
        for step in _steps(job):
            title = step.splitlines()[0].strip()
            condition = _step_condition(step)
            if "npm publish" in step:
                publishes += 1
                if condition is None:
                    errors.append(f"{workflow}: `{title}` has no `if:` that skips it on a dry run")
                    continue
                for label, context, want in DRY_RUN_SCENARIOS:
                    try:
                        got = _truthy(_Cond(condition, context).parse())
                    except ValueError as error:
                        errors.append(f"{workflow}: `{title}`: the condition `{condition}` cannot be read ({error})")
                        break
                    if got != want:
                        verb = "publish" if want else "skip"
                        note = " (a tag push has no inputs: it is always a real release)" if want and "tag" in label else ""
                        errors.append(f"{workflow}: `{title}`: the condition `{condition}` must {verb} for {label}{note}")
            elif condition is not None and "dry_run" in condition:
                errors.append(f"{workflow}: `{title}` is not a publish step; an `if:` on dry_run belongs to the `npm publish` steps only")
        if publishes and name != "publish":
            errors.append(f"{workflow}: `npm publish` belongs to the `publish` job, found in `{name}`")
    return errors


def preflight_errors(text: str, workflow: str) -> list[str]:
    """The pre-flight runs in a dry run too; `npm whoami` only while some package is not on npm; a recovery with
    dry_run fails early; the already-published message names `recovery=true` and `dry_run=false`."""
    errors: list[str] = []
    publish = _job_block(text, "publish")
    step = _step_block(publish, "Pre-flight npm release state (fresh-release gate + recovery + first-publish token)")
    if not step:
        return [f"{workflow}: the publish job has no pre-flight step"]
    if _step_condition(step) is not None:
        errors.append(f"{workflow}: the pre-flight is read-only and runs in a dry run: it has no `if:`")
    if "npm view" not in step:
        errors.append(f"{workflow}: the pre-flight must ask npm (`npm view`) whether each package exists")
    stack: list[tuple[str, bool]] = []  # (condition, in its then-branch)
    whoami = 0
    unguarded = 0
    for raw in step.splitlines():
        line = raw.strip()
        if line.startswith("#"):
            continue
        # Every call counts: bare, `if [!] npm whoami`, `$(npm whoami)`, `VAR=$(npm whoami)`, after `;`/`&&`/`||`/`then`.
        # An `echo "...npm whoami..."` message is text, not a call.
        if re.search(r"(?:^|[;&|(!`]|\bif|\bthen|\bdo|\belse)\s*npm whoami\b", line):
            whoami += 1
            if not any(cond == '[ -n "$ABSENT" ]' and then for cond, then in stack):
                unguarded += 1
        if re.match(r"^if .*; then$", line):
            stack.append((line[3:-6].strip(), True))
        elif line.startswith("elif ") or line == "else":
            if stack:
                stack[-1] = (stack[-1][0], False)
        elif line == "fi" and stack:
            stack.pop()
    if whoami == 0:
        errors.append(f"{workflow}: the pre-flight must run `npm whoami` while a package is missing from npm (a first publish needs the token)")
    elif unguarded:
        errors.append(f"{workflow}: every `npm whoami` in the pre-flight must run only under `if [ -n \"$ABSENT\" ]; then` (a package of the set is not on npm); once all are, trusted publishing needs no token ({unguarded} of {whoami} do not)")
    if "recovery=true and dry_run=false" not in step:
        errors.append(f"{workflow}: the already-published message must name `recovery=true and dry_run=false`")
    read_version = _job_block(text, "read-version")
    refuse = next((s for s in _steps(read_version) if "DRY_RUN" in s and "RECOVERY" in s), "")
    if not refuse or "exit 1" not in refuse or '"${RECOVERY:-false}" = "true"' not in refuse or '"${DRY_RUN:-false}" = "true"' not in refuse:
        errors.append(f"{workflow}: read-version must fail early on recovery=true with dry_run=true (a step reading RECOVERY and DRY_RUN and exiting 1)")
    return errors


def timeout_errors(text: str, workflow: str) -> list[str]:
    """Every job has `timeout-minutes`, a positive multiple of 5 (three times the longest measured run, rounded up to 5)."""
    errors: list[str] = []
    for name in _job_names(text):
        value = _field(_job_block(text, name), "timeout-minutes")
        if value is None:
            errors.append(f"{workflow}: job `{name}` has no `timeout-minutes` (the default holds a runner for 6 hours)")
        elif not value.isdigit() or int(value) <= 0 or int(value) % 5 or int(value) > 360:
            errors.append(f"{workflow}: job `{name}` has `timeout-minutes: {value}`; the rule is a positive multiple of 5")
    return errors


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
    """Workflow files and composite actions: both carry `uses:` refs and `run:` steps."""
    if path.suffix not in {".yml", ".yaml"}:
        return False
    return (len(path.parts) >= 3 and path.parts[0:2] == (".github", "workflows")) or (
        len(path.parts) >= 4 and path.parts[0:2] == (".github", "actions")
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
        if item and not item.startswith("scripts/test-fixtures/")
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
            "channels. Local actions/workflows are allowed. Release policy: "
            "every npm publish step of publish.yml publishes a hyphenated "
            "(prerelease) version with --tag next, never latest."
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
    build_rows = matrix_rows(_job_block(build_text, "build"))
    publish_rows = matrix_rows(_job_block(publish_text, "build"))
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

    # 5. The platform packages are one set in every place a release names them
    #    (download, checksum, executable bit, version, pack, pre-flight, the
    #    publish steps in order, optionalDependencies, bun.lock); see
    #    `package_set_errors`, run with the other 0.42 rules below.

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

    # 8. Every workflow that pins bun pins the SAME bun. test.yml runs the
    #    launcher's exit-status test and publish.yml builds and publishes
    #    it; a launcher proven under one bun and shipped under another is
    #    the drift this file exists to catch.
    bun_pins: dict[str, set[str]] = {}
    for workflow in sorted((ROOT / ".github/workflows").glob("*.yml")):
        for pin in re.findall(r"^\s*bun-version:\s*[\"']?([^\s\"'#]+)", read(workflow), re.MULTILINE):
            bun_pins.setdefault(pin, set()).add(workflow.name)
    check(
        len(bun_pins) <= 1,
        "setup-bun version drift across workflows: "
        + ", ".join(f"{v} in {sorted(files)}" for v, files in sorted(bun_pins.items())),
    )

    # 9. Every platform row embeds the one dashboard build, in both workflows.
    ERRORS.extend(dashboard_embed_errors(build_text, "build-native.yml"))
    ERRORS.extend(dashboard_embed_errors(publish_text, "publish.yml"))
    # 10. Every `npm publish` step splits stable from prerelease: a hyphenated
    #    version publishes `--tag next`, never `latest`.
    ERRORS.extend(prerelease_publish_errors(publish_text, "publish.yml"))

    # 11. 0.42 release portability: the floor each binary declares and the
    #     checks that keep it (see each function's docstring).
    other_texts = {name: read(ROOT / ".github/workflows" / name) for name in ("test.yml", "pr-guard.yml")}
    workflow_texts = {"build-native.yml": build_text, "publish.yml": publish_text, **other_texts}
    for name, text in workflow_texts.items():
        ERRORS.extend(timeout_errors(text, name))
    for name in ("build-native.yml", "publish.yml"):
        ERRORS.extend(runner_label_errors(workflow_texts[name], name))
        ERRORS.extend(image_digest_errors(workflow_texts[name], name))
        ERRORS.extend(macos_target_errors(top_level_env(workflow_texts[name]), name))
    ERRORS.extend(build_row_errors(build_rows, "build-native.yml"))
    ERRORS.extend(build_row_errors(publish_rows, "publish.yml"))
    cargo_toml = read(ROOT / "crates/canon-cli/Cargo.toml")
    main_rs = read(ROOT / "crates/canon-cli/src/main.rs")
    binary_checker = read(ROOT / "scripts/check-release-binary.py")
    for name in ("build-native.yml", "publish.yml"):
        ERRORS.extend(jemalloc_errors(workflow_texts[name], name, cargo_toml, main_rs))
    for name in ("build-native.yml", "publish.yml"):
        ERRORS.extend(alpine_build_errors(workflow_texts[name], name))
    ERRORS.extend(binary_check_errors(build_text, "build-native.yml", build_rows, binary_checker))
    ERRORS.extend(binary_check_errors(publish_text, "publish.yml", publish_rows, binary_checker))
    ERRORS.extend(crt_static_errors(read(ROOT / ".cargo/config.toml"), {"build-native.yml": build_text, "publish.yml": publish_text, **other_texts}))
    platform_manifests = {
        directory.name: json.loads((directory / "package.json").read_text(encoding="utf-8"))
        for directory in sorted((ROOT / "packages").glob("core-*"))
        if (directory / "package.json").is_file()
    }
    ERRORS.extend(launcher_errors(read(ROOT / "packages/cli/src/index.ts"), publish_rows, platform_manifests))
    launcher_name = json.loads(cli_manifest.read_text(encoding="utf-8")).get("name", "") if cli_manifest.is_file() else ""
    optional_keys = set(json.loads(cli_manifest.read_text(encoding="utf-8")).get("optionalDependencies", {})) if cli_manifest.is_file() else set()
    ERRORS.extend(package_set_errors(publish_text, publish_rows, launcher_name, read(ROOT / "bun.lock"), optional_keys))
    ERRORS.extend(smoke_errors(publish_text, build_text, other_texts))
    ERRORS.extend(dry_run_errors(publish_text, "publish.yml"))
    ERRORS.extend(preflight_errors(publish_text, "publish.yml"))

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
        # Workspace clippy configuration (rust-coding-rules.md §1); clippy reads it from the root.
        "clippy.toml",
        # The one Rust toolchain pin; rustup and cargo read it from the root.
        "rust-toolchain.toml",
        "README.md",
        # The release notes (README.md only introduces the project); read by the docs tests and the README's Changelog link.
        "CHANGELOG.md",
        "bun.lock",
        "canon.yaml",
        "docker-compose.yml",
        # The contributor command recipes (`just --list`); each mirrors a CI step.
        "justfile",
        # The Orca "Forca" workspace recipe (#66); Orca reads it from the root.
        "orca.yaml",
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
