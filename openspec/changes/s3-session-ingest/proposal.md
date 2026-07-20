## Why

Agent-CLI session data (omp/pi, Claude Code, Codex) lives in disconnected, per-tool
local transcript stores with no shared join key back to canon's other artifacts.
canon needs a first ingest surface — `canon-ingest` — that normalizes these raw
transcripts into the S1 state model (`Session`/`Run`/`Event` + token/cost rows keyed
by `session_id`, the vendored upstream launcher's join key) so cost, evidence, and trajectory records can
all join against one canonical session identity, and so the cost numbers agents and
operators already trust from the vendored upstream launcher carry over unchanged.

## What Changes

- New `canon-ingest` Rust crate implementing an **adapter registry** (mirroring the
  vendored upstream launcher's path-resolution + usage-parsing shape — MIT-attributed
  port: adapt the launcher's logic with ported-from-upstream provenance
  comments, operator directive 2026-07-10):
  each adapter declares its source root, relative scan path, file
  pattern, and a parser function; the registry enumerates the configured sources.
- First three adapters ship, in this order: **omp/pi** (`~/.omp/agent/sessions/`),
  **Claude Code** (`~/.claude/projects/`), **Codex** (`~/.codex/sessions/`).
- Each adapter normalizes its raw transcript format into canon-model's
  `Session`/`Run`/`Event` records (envelope `{schema, kind, at, actor}`, S1) plus
  token/cost rows keyed by `session_id`.
- **Incremental ingest** via a per-source watermark (last-seen cursor/mtime per
  scanned root) — a re-run only reads data newer than the watermark.
- **Idempotent ingest** via a per-record content digest — a full re-scan (watermark
  reset, `--watch` restart, concurrent run) never double-writes or double-counts a
  record already stored.
- New CLI command `canon ingest sessions [--watch]` (`--watch` polls the configured
  roots on an interval instead of exiting after one pass).
- Malformed session data is **skipped as a violation, never a crash** (design §7
  error-handling principle) — a corrupt transcript
  file degrades that file's records, not the whole ingest run.

## Capabilities

### New Capabilities

- `session-adapter-registry`: the adapter registration/enumeration contract plus the
  omp/claude/codex adapters that normalize raw transcripts into canon-model
  Session/Run/Event + token/cost rows.
- `session-ingest-idempotence`: watermark-based incremental scanning, content-digest
  idempotence, and cost parity with the vendored upstream launcher's numbers on the same fixture input.

### Modified Capabilities

_None — canon has no existing specs yet (S3 is part of the W1 wave; S0–S2 foundation
land in parallel)._

## Impact

- New crate `crates/canon-ingest` (Rust workspace, S0 scaffold).
- Depends on `canon-model` (S1: envelope + Session/Run/Event/EvidenceRecord types,
  join-spine `session_id` grammar) and `canon-store` (S2: git/pg/r2 tier write path)
  — both foundation-wave crates canon-ingest writes through, not around.
- New CLI surface: `canon ingest sessions [--watch]` on `canon-cli`.
- New fixture corpora: sanitized real transcript samples from omp/pi, Claude Code,
  and Codex, plus the vendored upstream launcher's own output on the same input for the cost-parity check.
- Companion skill (design §5 cross-cutting deliverable, decision 9): a `canon`
  ingest-usage skill under `canon/skills/` shipped in this same change.
