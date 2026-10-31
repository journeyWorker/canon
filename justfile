# Contributor recipes. Each runs the command a CI job runs, with the same
# options (.github/workflows/test.yml, build-native.yml, pr-guard.yml).
# CI itself does not call just.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Cargo's build output directory; the gate and bench recipes run the binaries from here.
target := env("CARGO_TARGET_DIR", "target")

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

# The workspace tests (test.yml `offline`, without the network ban).
test:
    cargo test --workspace --locked

# The three clippy runs of test.yml `clippy`.
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo clippy --workspace --lib --bins --locked -- -D warnings -D clippy::unwrap_used
    cargo clippy -p canon-store --all-targets --features live-pg,live-r2 --locked -- -D warnings

# Canon gates its own spec corpus: build, sync the scenario index, check.
gate:
    cargo build --locked -p canon-cli
    {{ target }}/debug/canon inventory sync
    {{ target }}/debug/canon gate check

# Rewrite the generated documentation blocks from the command spec.
docs:
    CANON_BLESS_DOCS=1 cargo test -p canon-cli --test docs_generated

# Rewrite the conformance expectations (review the diff before committing).
conformance:
    conformance/regenerate.sh

# The release-workflow safety check and its test.
release-check:
    python3 scripts/check-release-workflow-safety.py
    python3 scripts/test-release-workflow-safety.py

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

# atip check at L0 on the canon binary and on the checker itself (test.yml `atip-l0`).
atip:
    cargo build --locked -p canon-cli -p atip-check
    {{ target }}/debug/atip check --bin {{ target }}/debug/canon --level L0 --fixture crates/canon-cli/tests/atip/canon.fixture.json
    {{ target }}/debug/atip check --bin {{ target }}/debug/atip --level L0 --fixture crates/atip-check/atip.fixture.json

# Every CI check that needs no docker.
ci: release-check lint test gate launcher platformer atip
