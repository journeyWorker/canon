#!/usr/bin/env bash
# CI cache preparation (.github/workflows/test.yml, the step before `Save cargo cache`): leave only the dependencies' build in `target/`.
#
# The workspace crates' own build output is most of `target/` (the test binaries alone are 60% of a debug build) and is rebuilt on every change
# of the crates anyway; a cache that held it would also keep every earlier version's test binaries, which cargo never cleans up. This removes the
# build output of every workspace package (`cargo clean -p`, which knows the test binaries of the integration test files), then the files
# that still carry a workspace crate's name: the units of another path's copy of the workspace (the job `grammar-mutation` builds one) have other
# package ids and are not found by `cargo clean -p`.
#
# Usage: scripts/ci-strip-workspace-build.sh [--release]
# `--release` cleans `target/release` instead of `target/debug`. When GITHUB_STEP_SUMMARY is set, the sizes of what is saved are written there
# (CACHE_KEY names the key it is saved under).
set -euo pipefail
cd "$(dirname "$0")/.."

profile=()
target="${CARGO_TARGET_DIR:-target}"
dir="$target/debug"
if [ "${1:-}" = "--release" ]; then
  profile=(--release)
  dir="$target/release"
fi

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

for name in "${names[@]}"; do
  under="${name//-/_}"
  find "$dir/deps" -maxdepth 1 \( -name "$under-*" -o -name "lib$under-*" \) -delete 2>/dev/null || true
  find "$dir/.fingerprint" "$dir/build" -maxdepth 1 -name "$name-*" -exec rm -rf {} + 2>/dev/null || true
done

# nextest's reports and run store (`target/nextest/`) are results of this run, not dependencies: a restored report of an older run would look like one of this.
rm -rf "$target/nextest"

size() { if [ -e "$1" ]; then du -sh "$1" | cut -f1; else echo "none"; fi; }
echo "saved cache: target/ $(size "$target"), ~/.cargo $(size "$HOME/.cargo")"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "### cargo cache (saved)"
    echo "- key: \`${CACHE_KEY:-}\`"
    echo "- target/: $(size "$target")"
    echo "- ~/.cargo: $(size "$HOME/.cargo")"
  } >> "$GITHUB_STEP_SUMMARY"
fi
