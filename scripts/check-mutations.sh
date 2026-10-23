#!/usr/bin/env bash
# T15 (0.28 plan): seeded violations each fail the grammar test that guards against them.
#
# Copies the workspace sources to a scratch directory, applies one violation at a time (a dropped
# metadata row or option help, a hand-built JSON print, a runnable group, a duplicate option name
# with another meaning, a dropped vocabulary row) and checks that the guarding test fails on an
# assertion there. See crates/canon-cli/tests/grammar_t15_mutation.rs. The first run compiles the
# workspace under the scratch path; CARGO_TARGET_DIR (default: ./target) keeps the dependencies.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test -p canon-cli --test grammar_t15_mutation -- --ignored --nocapture "$@"
