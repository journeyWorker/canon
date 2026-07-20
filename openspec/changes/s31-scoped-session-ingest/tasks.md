# s31 scoped-session-ingest — tasks

## 1. canon-store (D1 gate helper + D2 batch)

- [x] 1.1 D2: `Tier::write_batch` (provided default = loop `write`);
      `PgTier` override with 500-row chunked multi-row insert, one
      transaction per chunk, receipts per row; `TierRegistry::persist`
      groups by (kind → rung) and calls `write_batch`.
- [x] 1.2 D1: `SourceCursor::diff(present) -> {changed_or_new, unchanged}`
      per-file partition helper beside (not replacing) the struct;
      unit tests: growing file, new file, deleted file, unchanged set.
- [x] 1.3 Tests: batch no-op on byte-identical resubmission (record
      count unchanged); batch vs loop write equivalence on the same
      corpus.

## 2. canon-ingest (D4 directives + workspace on Session)

- [x] 2.1 `DirectiveRow` type + `ParseOutcome.directives` (default
      empty, additive).
- [x] 2.2 Adapters emit user-role directive rows: omp/pi, claude-code,
      codex; hermes emits per the same rule or documents the format
      gap in its module doc.
- [x] 2.3 `normalize_rows` (or a sibling entry point) maps directive
      rows to `Event { label: "user_directive", detail.text }` with
      deterministic `seq`; `Session` gains optional `workspace_key`/
      `workspace_label`/`project_key`; JSON schema regenerated.
- [x] 2.4 Tests: fixture transcripts with user turns → directive
      events in order, verbatim text; re-parse of a grown fixture
      re-emits byte-identical earlier events.

## 3. canon-cli (D1 wiring + D3 scope + surface)

- [x] 3.1 Replace the all-or-nothing gate with the per-file diff:
      parse ONLY changed/new files; summary line gains a reparsed
      count alongside `skipped unchanged`.
- [x] 3.2 D3: project-set resolution (main root + `git worktree list
      --porcelain`, fail-soft), root pruning for omp/claude-code,
      row-level workspace filter for codex/hermes, `--all-workspaces`
      flag, scope named in the summary; `project_key` stamped onto
      normalized sessions.
- [x] 3.3 Stale-doc fixes: drop "Wave 1: `omp` only" from the command
      help; document the new flag.
- [x] 3.4 Integration tests: fixture home with two "projects" →
      default pass ingests only the configured project (worktree
      included), `--all-workspaces` ingests both; steady-state second
      pass parses zero files.

## 4. Docs

- [x] 4.1 `canon/skills/canon-session-ingest/SKILL.md`: project scope
      default, `--all-workspaces`, file-granular watermark, directive
      events (+ materialized `.claude`/`.codex` copies via
      `canon skills install`).
- [x] 4.2 Website `cli.mdx` (EN+KR): `ingest sessions` row — scope
      default + new flag + directive capture.

## 5. Verification

- [x] 5.1 `cargo test --workspace` green offline (no live services, no
      `CANON_*` env), `canon selftest` + `canon gate selftest` green.
- [x] 5.2 Live smoke on this repo: full project-scoped pass, then a
      steady-state pass completing in seconds with `parsed 1` (the
      live transcript) or `0`; directive events queryable.
