#!/usr/bin/env bash
# CI cache preparation (.github/workflows/test.yml and build-native.yml, the step before `Save cargo cache`): leave only the dependencies' build in `target/`.
#
# The workspace crates' own build output is most of `target/` (the test binaries alone are 60% of a debug build) and is rebuilt on every change
# of the crates anyway; a cache that held it would also keep every earlier version's test binaries, which cargo never cleans up. This removes the
# build output of every workspace package (`cargo clean -p`, which knows the test binaries of the integration test files). Measured on 1.100.0
# (2026-11-17): nothing that carries a workspace crate's name is left in `target/` after it.
#
# Usage: scripts/ci-strip-workspace-build.sh [--release] [--target <triple>]
# `--release` cleans `target/release` instead of `target/debug`. `--target <triple>` is for a build made with `cargo build --target <triple>`: that
# build has two halves, the host's (build scripts and proc-macros, in `target/release`) and the target's (`target/<triple>/release`), and both are
# cleaned. When GITHUB_STEP_SUMMARY is set, the sizes of what is saved are written there (CACHE_KEY names the key it is saved under).
set -euo pipefail
cd "$(dirname "$0")/.."

profile=()
triple=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --release) profile=(--release); shift ;;
    --target)
      [ "$#" -ge 2 ] || { echo "--target needs a triple" >&2; exit 2; }
      triple="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
target="${CARGO_TARGET_DIR:-target}"

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
if [ -n "$triple" ]; then
  cargo clean --locked --offline ${profile[@]+"${profile[@]}"} --target "$triple" "${packages[@]}"
fi

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
