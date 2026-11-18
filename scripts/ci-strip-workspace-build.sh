#!/usr/bin/env bash
# CI cache preparation (.github/workflows/test.yml, the step before `Save cargo cache`): leave only the dependencies' build in `target/`.
#
# The workspace crates' own build output is most of `target/` (the test binaries alone are 60% of a debug build) and is rebuilt on every change
# of the crates anyway; a cache that held it would also keep every earlier version's test binaries, which cargo never cleans up. This removes the
# build output of every workspace package (`cargo clean -p`, which knows the test binaries of the integration test files). Measured on 1.100.0
# (2026-11-17): nothing that carries a workspace crate's name is left in `target/` after it.
#
# Usage: scripts/ci-strip-workspace-build.sh [--release]
# `--release` cleans `target/release` instead of `target/debug`. When GITHUB_STEP_SUMMARY is set, the sizes of what is saved are written there
# (CACHE_KEY names the key it is saved under).
set -euo pipefail
cd "$(dirname "$0")/.."

profile=()
target="${CARGO_TARGET_DIR:-target}"
if [ "${1:-}" = "--release" ]; then
  profile=(--release)
fi

size() { if [ -e "$1" ]; then du -sh "$1" | cut -f1; else echo "none"; fi; }
before="$(size "$target")"

names=()
while read -r name; do
  names+=("$name")
done < <(cargo metadata --no-deps --locked --offline --format-version 1 | jq -r '.packages[].name')
[ "${#names[@]}" -gt 0 ] || { echo "no workspace package found" >&2; exit 1; }

packages=()
for name in "${names[@]}"; do
  packages+=(-p "$name")
done
cargo clean --locked --offline ${profile[@]+"${profile[@]}"} "${packages[@]}"

echo "saved cache: target/ $(size "$target") (was $before), ~/.cargo $(size "$HOME/.cargo")"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "### cargo cache (saved)"
    echo "- key: \`${CACHE_KEY:-}\`"
    echo "- target/ before the strip: $before"
    echo "- target/: $(size "$target")"
    echo "- ~/.cargo: $(size "$HOME/.cargo")"
  } >> "$GITHUB_STEP_SUMMARY"
fi
