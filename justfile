# Contributor recipes. Each runs the command a CI job runs, with the same
# options (.github/workflows/test.yml, build-native.yml, pr-guard.yml).
# CI itself does not call just.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Cargo's build output directory; the gate and bench recipes run the binaries from here.
target := env("CARGO_TARGET_DIR", "target")

# The checker `just atip-install` installs; the recipes call it by path, never from PATH.
atip_bin := "tools/atip-check/node_modules/.bin/atip"

# The live tier's environment contract (see the README).
live_env := "CANON_REQUIRE_LIVE=1 CANON_PG_DSN=postgres://canon:canon@127.0.0.1:55432/canon_v1 CANON_S3_ENDPOINT=http://127.0.0.1:59000 CANON_S3_BUCKET=canon CANON_S3_ACCESS_KEY=canon CANON_S3_SECRET_KEY=canoncanon"

# List the recipes.
default:
    @just --list

# Build the canon binary (debug).
build:
    cargo build --locked -p canon-cli

# Install canon from this checkout.
install:
    cargo install --path crates/canon-cli

# The workspace tests (test.yml `offline`, without the network ban). The checker (`atip check`) is the atip repo's: its tests are not run here (`just atip` runs it on canon).
test:
    cargo test --workspace --locked

# The unit tests of the workspace under three callers: none, an implementer with an actor, and an unknown role. A unit test that reads the declared role
# or actor from the environment (`closing_role` once did) answers differently in two of them and fails here; `tests/hermetic_env.rs` covers the tests that start the binary.
test-callers:
    env -u CANON_ROLE -u CANON_ACTOR cargo test --workspace --lib --locked
    env CANON_ROLE=implementer CANON_ACTOR=impl-someone cargo test --workspace --lib --locked
    env -u CANON_ACTOR CANON_ROLE=no-such-role cargo test --workspace --lib --locked

# The three clippy runs of test.yml `clippy`, and its git boundary scan (every git call goes through `canon_store::git`).
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo clippy --workspace --lib --bins --locked -- -D warnings -D clippy::unwrap_used
    cargo clippy -p canon-store --all-targets --features live-pg,live-r2 --locked -- -D warnings
    python3 scripts/check_git_boundary.py

# Canon gates its own spec corpus: build, sync the scenario index, check.
gate:
    cargo build --locked -p canon-cli
    {{ target }}/debug/canon inventory sync
    {{ target }}/debug/canon gate check

# Rewrite the generated documentation blocks from the command spec.
docs:
    CANON_BLESS_DOCS=1 cargo test -p canon-cli --test docs_generated

# Make the stored rule calls again (tests/atip/rule_calls.jsonl): what the search for a call that breaks or meets a rule finds, kept as data that
# `atip_check_order` and `arg_rules` read. Run it after a rule, an example, a value table or the searcher (tests/rule_support/search.rs) changes.
rule-calls:
    CANON_BLESS_RULE_CALLS=1 cargo test -p canon-cli --test arg_rules the_stored_rule_calls

# Make them again and fail when the table in the repository differs (stage the table first). Run it once before a release, like `atip-deep`: CI does not
# search on every push; the digest of each row and the re-evaluation of its call catch a changed input on every push.
rule-calls-check:
    just rule-calls
    git diff --exit-code crates/canon-cli/tests/atip/rule_calls.jsonl

# Rewrite the conformance expectations (review the diff before committing).
conformance:
    conformance/regenerate.sh

# The release-workflow safety check and its test, and the release binary check's test.
# On a binary: `python3 scripts/check-release-binary.py --target <triple> <canon>` (see its header).
release-check:
    python3 scripts/check-release-workflow-safety.py
    python3 scripts/test-release-workflow-safety.py
    python3 scripts/test-check-release-binary.py

# The npm launcher's tests (test.yml `launcher`).
launcher:
    bun install --frozen-lockfile
    cd packages/cli && bun test

# The example game's tests, one per scenario (test.yml `platformer`).
platformer:
    cd examples/platformer && bun install --frozen-lockfile
    cd examples/platformer && bun run test:junit

# Build the dashboard app (build-native.yml `dashboard`).
dashboard:
    bun install --frozen-lockfile
    bun run --cwd packages/dashboard build

# Generate the man pages into ./man-pages (build-native.yml `man`).
man:
    cargo run --locked --release -p canon-cli --features man --example canon-man -- man-pages

# The corpus-size cost-shape gate on the release binary (test.yml `bench-shape`). Linux only: the script reads /proc.
bench:
    cargo build --release --locked -p canon-cli --bin canon
    python3 scripts/bench/run.py --canon {{ target }}/release/canon --grid small --out {{ target }}/bench.tsv
    python3 scripts/bench/check_shape.py {{ target }}/bench.tsv --max-factor 20

# The live tier against docker-compose Postgres and MinIO (test.yml `live-tier`); needs docker.
live:
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'docker compose down -v' EXIT
    docker compose up -d --wait postgres minio
    docker compose up minio-init
    env {{ live_env }} cargo test -p canon-store --features live-pg,live-r2 --locked
    env {{ live_env }} cargo test -p canon-cli --test ledger_pg_live --locked

# Install the checker canon is held to: the pre-built `@journeykit/atip-check` (atip repo, `atip check`) from the lockfile in `tools/atip-check/`, which pins its
# version and the sha512 of its platform packages. The CI job `atip` runs the same command.
atip-install:
    cd tools/atip-check && bun install --frozen-lockfile

# atip check on the canon binary, interface checks only (seconds): L2, at the contract the fixture declares. Needs the checker: `just atip-install`.
atip:
    cargo build --locked -p canon-cli
    {{ atip_bin }} check --bin {{ target }}/debug/canon --level L2 --jobs 4 --fixture crates/canon-cli/tests/atip/canon.fixture.json

# The same with `--deep` (declaration against behaviour, about two minutes): what CI's `atip` job runs; run it once before a release.
atip-deep:
    cargo build --locked -p canon-cli
    {{ atip_bin }} check --bin {{ target }}/debug/canon --level L2 --jobs 4 --deep --fixture crates/canon-cli/tests/atip/canon.fixture.json

# Every CI check that needs no docker.
ci: release-check lint test test-callers gate launcher platformer atip-deep
