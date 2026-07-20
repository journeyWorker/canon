# s31 scoped-session-ingest — design

Review provenance: operator dogfood session 2026-07-14 (full first
pass 23m, steady-state pass 5m+ timeout, PG row counts, `sample`d
hot frames in `PgTier::write_row`). Finding numbers cite that run.

## D1 — File-granular watermark (supersedes S3 §3's source-granular gate)

`SourceCursor` already stores one digest PER FILE; only the gate
(`source_unchanged`) compares all-or-nothing. Replace the gate with a
per-file diff at the `canon-cli` pass layer:

- `unchanged`: present file whose digest equals the cursor's → skipped
  (counted in `skipped_unchanged`, never re-read into parse).
- `changed`/`new`: parsed this pass.
- `deleted`: absent from the fresh cursor (no tombstone work — records
  already persisted are append-only history).

S3 §3's stated reason for source granularity — "a multi-file session
is never partially re-normalized" — is vacuous for every registered
adapter: omp/pi, claude-code, codex, and hermes all derive a session
from exactly ONE transcript file (each adapter module documents this).
That single-file-session property becomes an explicit
`SessionAdapter` contract note; a future multi-file adapter must opt
back into source-granular gating (registry flag), not weaken D1.

Cursor write timing is unchanged (advance only after a durable pass).

## D2 — Batched persist

`Tier` gains `write_batch(&mut self, records) -> Result<Vec<WriteReceipt>>`
with a provided default that loops `write` (Git/local tiers keep it).
`PgTier` overrides: chunks of 500 rows per multi-row
`INSERT … ON CONFLICT (kind, id, digest) DO NOTHING`-equivalent
statement, one transaction per chunk, receipts reconstructed from the
statement's per-row outcome. Semantics identical to the s21
append-only contract — a byte-identical resubmission is a no-op.
`TierRegistry::persist` batches per (kind → rung) group instead of
per record.

## D3 — Project scope (the default corpus)

- Project identity: the repo's MAIN worktree root plus every linked
  root from `git worktree list --porcelain`, each passed through the
  existing `normalize_workspace_key`. Fail-soft: not a git repo (or
  git absent) → the repo root alone. `project_key` = the main
  worktree's normalized key.
- Root pruning: omp (`~/.omp/agent/sessions/<encoded-cwd>/`, same for
  `~/.pi`) and claude-code (`~/.claude/projects/<encoded-path>/`)
  partition by cwd — non-matching SUBDIRS are pruned at enumerate
  time (never read, never digested). Codex/hermes transcripts are not
  cwd-partitioned: parse, then drop rows whose `workspace_key` is
  outside the project set (dropped rows are ordinary filtering, not
  malformed).
- `--all-workspaces` restores today's machine-wide behaviour; the
  summary names the active scope either way.
- `Session` gains OPTIONAL `workspace_key`, `workspace_label`,
  `project_key` body fields (additive; regenerate the JSON schema per
  the state-model discipline). Sessions ingested pre-s31 simply lack
  the fields.

## D4 — User-directive capture

- `ParseOutcome` gains `directives: Vec<DirectiveRow>` (default
  empty): `{ session_id, timestamp_ms, text, workspace_key,
  workspace_label }`. Adapters emit one row per USER-role message
  (never system/tool/assistant): omp/pi `message.role == "user"`,
  claude-code user entries, codex user input items. Hermes: same rule
  if its format carries user turns; else document the gap in the
  adapter module and emit none.
- Normalize maps each to an `Event` on the session's run:
  `label: "user_directive"`, `detail: { text, workspace_key,
  workspace_label }`, deterministic `seq` (fold order), so re-parsing
  a grown file re-emits byte-identical earlier events → digest-deduped.
- Command/paste blobs are stored verbatim (they ARE the directive);
  no truncation this wave.
- OUT OF SCOPE (named follow-up): distilling directives into S6
  strategy memory; any `canon learn`/`canon retrieve` awareness.

## D5 — Wiring order

D1+D3 both cut the steady-state pass: D3 shrinks the corpus (~7k
files → the project's), D1 shrinks the re-parse set (project files →
the one growing transcript). With D2, the residual cost per pass is
one growing file's parse + one small batch. `--watch` stays shipped
and becomes usable again; no interval change.
