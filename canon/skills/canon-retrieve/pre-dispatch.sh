#!/bin/sh
# canon-retrieve-pre-dispatch.sh — S8's generic pre-dispatch hook
# (`s8-retrieve-before-task`, design.md decision 4, tasks.md task 3.1):
# fires at a PreToolUse-equivalent point, generalized to any
# `task`-shaped dispatch (Claude Code's `Task` tool / Codex's
# equivalent subagent-dispatch tool call), mirroring a donor
# harness's `pre-edit-pattern-lookup.ts` fail-soft, advisory-only
# PreToolUse contract (design.md Context) — applied here to
# role-scoped strategy retrieval instead of edit-pattern lookup.
#
# WIRING (design decision 4 — reuses S5's install-hooks mechanism
# verbatim, invents no second convention): install the standard
# `{matcher, hooks: [{type: "command", command, timeout}]}` entry via
# the ALREADY-GENERIC `canon gate install-hooks` (S5, `s5-trust-spine-
# gate`) — this script adds NO new installer code:
#
#   canon gate install-hooks --repo . --event PreToolUse \
#     --matcher Task --command "sh /path/to/pre-dispatch.sh" \
#     --timeout 15
#
# Place this file wherever your repo keeps materialized hook scripts
# (e.g. `.claude/hooks/canon-retrieve-pre-dispatch.sh`, following the
# manual-placement convention for materialized hook scripts)
# and point `--command` at that path — same manual-
# placement discipline canon-gate's own `PRE_COMMIT_SCRIPT` documents
# ("install it directly as `.git/hooks/pre-commit`, or invoke it from
# a lefthook/husky `run:` line").
#
# FAIL-SOFT, ADVISORY-ONLY — this script NEVER blocks the dispatch and
# NEVER emits a `permissionDecision`; it only ever adds
# `additionalContext` (Claude Code's `PreToolUse` hook output
# contract) or stays silent. Every one of the following degrades to a
# SILENT no-op, never a nonzero exit:
#   - `canon` (or `jq`) not on PATH.
#   - the dispatched tool is not task-shaped (`tool_name != "Task"`) —
#     re-checked here even though the install recipe above already
#     scopes the hook via `--matcher Task`, so this script stays a
#     safe no-op if installed with a broader/no matcher.
#   - no role derivable from the tool input, or empty/malformed stdin.
#   - `canon retrieve` returns empty guidance — it is ALREADY
#     fail-soft (S8Core's `retrieve_guidance` never errors; canon-
#     cli's own CLI boundary never exits nonzero for a store outage,
#     `crates/canon-cli/src/retrieve.rs`'s own module doc) — this
#     script never adds a SECOND failure mode on top of that.
#
# ROLE/REGIME DERIVATION (an explicit design.md Open Question this
# script resolves, pinned here): Claude Code's `Task` tool call
# carries a `subagent_type` field naming which specialist role is
# about to run — read directly as `--role` (kebab-cased). `--regime`'s
# `<role>/<repo>/<area>/<hash>` is NOT fully recoverable from a bare
# tool-call payload (S6's `area`/`hash` taxonomy is repo-specific, not
# something a generic script can infer) — this ships a CONSERVATIVE
# default (`area=general`, a stable hash of that area) a repo is
# expected to override once it adopts a real regime taxonomy:
#   - `CANON_RETRIEVE_ROLE`    overrides the auto-derived role.
#   - `CANON_RETRIEVE_AREA`    overrides the default `general` area.
#   - `CANON_RETRIEVE_HASH`    overrides the derived hash segment.
#   - `CANON_RETRIEVE_REGIME`  overrides the whole assembled
#                              `regime_key`, bypassing all of the above.
set -eu

command -v canon >/dev/null 2>&1 || exit 0
command -v jq >/dev/null 2>&1 || exit 0

INPUT="$(cat 2>/dev/null || true)"
[ -n "$INPUT" ] || exit 0

TOOL_NAME="$(printf '%s' "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null || true)"
[ "$TOOL_NAME" = "Task" ] || exit 0

# `slugify` is used ONLY for the ROLE segment: `RoleId`'s grammar is
# strict kebab (`[a-z0-9]+(-[a-z0-9]+)*`), and a kebab string is a
# fixed point of `regime_key`'s own segment canonicalizer, so the
# derived `--role` still equals the assembled regime_key's leading
# segment. The repo/area segments are NOT slugified here — they are
# canonicalized by `canon regime-key` below (the write-path serializer).
slugify() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9' '-' | sed -E 's/-+/-/g; s/^-//; s/-$//'
}

ROLE="${CANON_RETRIEVE_ROLE:-}"
if [ -z "$ROLE" ]; then
  ROLE="$(printf '%s' "$INPUT" | jq -r '.tool_input.subagent_type // empty' 2>/dev/null || true)"
fi
ROLE="$(slugify "$ROLE")"
[ -n "$ROLE" ] || exit 0

REGIME="${CANON_RETRIEVE_REGIME:-}"
if [ -z "$REGIME" ]; then
  REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
  REPO_RAW="$(basename "$REPO_ROOT")"
  AREA_RAW="${CANON_RETRIEVE_AREA:-general}"
  HASH="${CANON_RETRIEVE_HASH:-}"
  if [ -z "$HASH" ]; then
    HASH="$(printf '%s' "$AREA_RAW" | sha256sum 2>/dev/null | cut -c1-12)"
    [ -n "$HASH" ] || HASH="$(printf '%s' "$AREA_RAW" | shasum -a 256 2>/dev/null | cut -c1-12)"
  fi
  [ -n "$HASH" ] || exit 0
  # Assemble the regime_key through canon's OWN serializer (`canon
  # regime-key` -> `canon_model::ids::regime_key`), the identical
  # normalizer the S4/S6/S14 WRITE path uses — never a second shell
  # derivation. A prior local `slugify` of the repo segment mapped a
  # written `my_repo` to a queried `my-repo`, so `retrieve_guidance`
  # fail-softed to empty and silently missed the namespace. `--role`
  # is the already-kebab ROLE (a fixed point of that canonicalizer, so
  # it still equals the assembled key's leading segment). Malformed
  # segments make `canon regime-key` exit nonzero -> `|| true` -> empty
  # REGIME -> the silent no-op just below.
  REGIME="$(canon regime-key --role "$ROLE" --repo "$REPO_RAW" --area "$AREA_RAW" --hash "$HASH" 2>/dev/null || true)"
  [ -n "$REGIME" ] || exit 0
fi

GUIDANCE_JSON="$(canon retrieve --role "$ROLE" --regime "$REGIME" --json 2>/dev/null || true)"
[ -n "$GUIDANCE_JSON" ] || exit 0

COUNT="$(printf '%s' "$GUIDANCE_JSON" | jq 'length' 2>/dev/null || echo 0)"
[ "$COUNT" -gt 0 ] 2>/dev/null || exit 0

CONTEXT="$(printf '%s' "$GUIDANCE_JSON" | jq -r --arg role "$ROLE" '
  (["[canon-retrieve] guidance for role " + $role + ":"]
   + (map("- " + .title + ": " + .content))) | join("\n")
' 2>/dev/null || true)"
[ -n "$CONTEXT" ] || exit 0

jq -n --arg ctx "$CONTEXT" '{hookSpecificOutput: {hookEventName: "PreToolUse", additionalContext: $ctx}}'
exit 0
