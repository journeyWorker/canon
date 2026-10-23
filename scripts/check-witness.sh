#!/usr/bin/env bash
# T9 (0.28 plan): every output the generated spec declares was emitted at least once by the test suite.
#
# A debug build of `canon` records each JSON document it prints (command, ok, report type) into
# $CANON_WITNESS_DIR when that variable is set. This runs the whole workspace suite that way,
# then the check in crates/canon-cli/tests/grammar_t9_witness.rs, which compares what was
# recorded with the spec's `outputs`. What the suite cannot reach is listed, with a reason, in
# that file's UNWITNESSED allowlist, which only shrinks.
#
# Usage: scripts/check-witness.sh [extra `cargo test --workspace` arguments]
# CANON_WITNESS_DIR may be set to keep the recording; by default a temporary directory is used.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -z "${CANON_WITNESS_DIR:-}" ]; then
  CANON_WITNESS_DIR="$(mktemp -d)"
  trap 'rm -rf "$CANON_WITNESS_DIR"' EXIT
else
  mkdir -p "$CANON_WITNESS_DIR"
fi
export CANON_WITNESS_DIR

cargo test --workspace --no-fail-fast "$@"
cargo test -p canon-cli --test grammar_t9_witness -- --ignored
