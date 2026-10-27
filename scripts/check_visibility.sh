#!/usr/bin/env bash
# Fails (exit 1, one file:line per hit) when crate source uses the restricted
# visibility forms `pub(super)` or `pub(in <path>)` (rust-coding-rules.md §3).
# Scope: crates/*/src/**/*.rs. Lines whose first non-blank characters are `//`
# (line, doc and `//!` comments) are skipped, so prose may name the forms;
# a trailing comment after code is not skipped. Needs only bash and grep.
set -euo pipefail

cd "$(dirname "$0")/.."

hits=$(grep -rnE --include='*.rs' 'pub\((super\)|in )' crates/*/src \
  | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' || true)

if [ -n "$hits" ]; then
  printf '%s\n' "$hits" >&2
  echo "check_visibility: pub(super) / pub(in ..) is banned in crate sources" >&2
  exit 1
fi
