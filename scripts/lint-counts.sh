#!/usr/bin/env bash
# Count clippy/rustc warnings per lint. Run from the workspace root.
#
#   scripts/lint-counts.sh [OUT_DIR] [-- extra rustc/clippy flags...]
#
# With no flags, three passes run, and the lints are whatever the workspace
# config enables ([workspace.lints] in Cargo.toml, clippy.toml) plus clippy's
# default groups:
#   all   cargo clippy --workspace --all-targets
#   live  the same with `-p canon-store --features live-pg,live-r2`
#   prod  cargo clippy --workspace --lib --bins -- -D clippy::unwrap_used
#         (production targets only; counts unwrap_used)
# Extra flags after `--` replace the `prod` pass and are passed to the `all`
# and `live` passes after `--`.
#
# Writes OUT_DIR/all.tsv (pass, lint, file, line, col; de-duplicated) and prints
# a per-lint summary sorted by count. Uses its own target dir under OUT_DIR so
# the repo's target/ is untouched; delete OUT_DIR afterwards.
set -euo pipefail

out="${1:-${TMPDIR:-/tmp}/lint-counts}"
shift || true
[ "${1:-}" = "--" ] && shift
mkdir -p "$out"

extra=("$@")
: > "$out/raw.json"

run_pass() {
  # run_pass NAME -- cargo-clippy-args...
  local name="$1"
  shift
  CARGO_TARGET_DIR="$out/target" cargo clippy --message-format=json "$@" 2>/dev/null \
    | python3 -c '
import json, sys
name = sys.argv[1]
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") == "compiler-message":
        m["pass"] = name
        print(json.dumps(m))
' "$name" >> "$out/raw.json" || true
}

if [ ${#extra[@]} -eq 0 ]; then
  run_pass all --workspace --all-targets
  run_pass live -p canon-store --all-targets --features live-pg,live-r2
  run_pass prod --workspace --lib --bins -- -W clippy::unwrap_used
else
  run_pass all --workspace --all-targets -- "${extra[@]}"
  run_pass live -p canon-store --all-targets --features live-pg,live-r2 -- "${extra[@]}"
fi

python3 -c '
import json, sys
seen = set()
rows = []
for line in open(sys.argv[1]):
    m = json.loads(line)
    msg = m["message"]
    code = (msg.get("code") or {}).get("code")
    if not code or msg.get("level") != "warning":
        continue
    spans = [s for s in msg.get("spans", []) if s.get("is_primary")]
    if not spans:
        continue
    s = spans[0]
    key = (code, s["file_name"], s["line_start"], s["column_start"])
    if key in seen:
        continue
    seen.add(key)
    rows.append((m["pass"],) + key)
for r in sorted(rows):
    print("\t".join(map(str, r)))
' "$out/raw.json" > "$out/all.tsv"

echo "total $(wc -l < "$out/all.tsv") warnings -> $out/all.tsv"
cut -f2 "$out/all.tsv" | sort | uniq -c | sort -rn
