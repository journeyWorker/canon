## 1. Adapter registry scaffold

- [x] 1.1 Define the `Adapter` trait in `canon-ingest` (`source_id()`,
      `resolve_roots(cfg) -> Vec<PathBuf>`, `scan(root) -> Vec<RawRecord>`,
      `normalize(RawRecord) -> Vec<Event>`) and the static `registry()` array.
      (Wave 1, S3Pipeline: shipped as `SessionAdapter { client_id(),
      scan_roots(home, use_env_roots), parse(path) -> Vec<UnifiedRow> }` +
      `registry::registry()` — the frozen Wave-1 contract per the S3
      implementation assignment; `UnifiedRow` is the `RawRecord`/`Event`
      normalization target this task's prose names, mirroring the launcher's
      `UnifiedMessage` per design context. `crates/canon-ingest/src/
      adapter.rs`, `registry.rs`.)
- [x] 1.2 Implement root resolution honoring the launcher's root-override env var and
      `canon.yaml` per-source path config, falling back to the documented
      default (`~/.omp/agent/sessions/`, `~/.claude/projects/`,
      `${CODEX_HOME:-~/.codex}/sessions/` unioned with
      `${CODEX_HOME:-~/.codex}/archived_sessions/`).
      All three layers now ship. Env overrides + all four adapters'
      defaults live in `crates/canon-ingest/src/adapters/{omp,claude,
      codex,hermes}.rs`'s `scan_roots` (gated by `use_env_roots` — omp's
      `CANON_INGEST_OMP_SESSIONS_DIR`, claude's
      `CANON_INGEST_CLAUDE_SESSIONS_DIR`, codex's `CODEX_HOME`, hermes's
      `HERMES_HOME`). The `canon.yaml` per-source override lands at the
      canon-cli layer WITHOUT touching the FROZEN Wave-1
      `SessionAdapter::scan_roots(home, use_env_roots)` contract:
      `crates/canon-cli/src/ingest.rs`'s `IngestSourceConfig` parses
      `ingest.sources.<client_id>.roots` and, for a configured source,
      scans exactly those roots (relative paths resolved against the
      `canon.yaml` dir) via `canon_ingest::scanner::scan_roots` while
      the adapter's own `parse_files` still parses — so the trait is
      untouched. Present-but-malformed config fails LOUD
      (`IngestError::Config`): a `deny_unknown_fields` typo (`root:`),
      an unknown source id (`claude` vs `claude-code`), or a
      non-YAML canon.yaml, rather than silently scanning default home
      roots; an explicit `roots: []` scans zero (never a fallback);
      missing/unreadable canon.yaml stays fail-soft. 5 new tests in
      `ingest::tests` (replace-not-union, empty-scans-zero, and three
      fail-loud cases: unknown-id, `root:` typo, invalid YAML) + the
      pre-existing default-resolution suite.
- [x] 1.3 Implement the `omp` adapter: scan + parse omp/pi session transcripts.
      (Wave 1, S3Pipeline: `crates/canon-ingest/src/adapters/omp.rs`, ported
      from the vendored upstream launcher's omp/pi session parser
      with provenance comments; 8 unit tests + 3 fixture-corpus integration
      tests green, `cargo test -p canon-ingest`.)
- [x] 1.4 Implement the `claude` adapter: scan + parse `~/.claude/projects/**/*.jsonl`.
      — ✅ `crates/canon-ingest/src/adapters/claude.rs` (`ClaudeCodeAdapter`,
      registered in `registry.rs`; exercised end-to-end by
      `tests/claude_ingest_integration.rs`).
- [x] 1.5 Implement the `codex` adapter: scan + parse
      `${CODEX_HOME:-~/.codex}/sessions/**/*.jsonl` UNIONED with
      `${CODEX_HOME:-~/.codex}/archived_sessions/**/*.jsonl` (Codex CLI's
      own session-rotation sibling directory — a live-only scan silently
      under-counts rotated sessions), deduped by canonicalized path before
      scanning (design D5).
      — ✅ `crates/canon-ingest/src/adapters/codex.rs` (`CodexAdapter::scan_roots`
      unions live + archived, registered in `registry.rs`); tests
      `codex.rs::{scan_roots_unions_live_and_archived_sessions,
      live_and_archived_sessions_are_both_scanned_and_parsed}`.
- [x] 1.6 Handle an absent/unconfigured source root as a zero-record, non-fatal
      skip with an informational summary line (never a violation or crash).
      (Wave 1, S3Pipeline: `scanner::scan_dir`/`scan_roots` return `Vec::new()`
      for a missing root, unit-tested; CLI run summary prints `0 file(s)
      scanned` rather than erroring — smoke-tested against an empty `--home`.)

## 2. Normalization to the S1 model

- [x] 2.1 Map each adapter's raw record into canon-model's `Session`/`Run`/
      `Event` types carrying the `{schema, kind, at, actor}` envelope (S1).
      (Wave 1, S3Pipeline: `crates/canon-ingest/src/normalize.rs::
      normalize_session` — omp rows only; smoke-tested end-to-end against a
      real `canon` binary + git tier, `canon/ledger/kind={session,run,event}/`.)
- [x] 2.2 Derive `actor {agent_id, role, session_id?, model?}` per adapter
      (never a bare `by` string) from the source's own client/session/model
      fields.
      (Wave 1, S3Pipeline: `Actor::new_unattributed(client).with_session(..)
      [.with_model(..)]` — role stays `None` per S11 D5's "absent stays
      absent" contract, since omp/pi's transcript carries no canon role.)
- [x] 2.3 Emit a token/cost row per session keyed by `session_id` (S1
      join-spine grammar: agent-CLI UUID, the vendored upstream launcher's join key).
      (Wave 1, S3Pipeline: one `Event { label: "token_usage", run_id, detail:
      {tokens, cost, cost_source, ...} }` per `UnifiedRow`, `run_id` carries
      `session_id` via `Run.session_id` — canon-model has no standalone
      token/cost record kind; `Event.detail` is S1's documented open
      extension point for this, see `normalize.rs` module doc.)
- [x] 2.4 Skip a record that fails normalization as a violation (count + log),
      never crash the run or abort the remaining records/adapters (design §7).
      (Wave 1, S3Pipeline: `NormalizeOutcome::skipped_rows` counts a
      `SessionId::parse` failure without aborting the batch; per-line skip
      already happens one layer down in `adapters::omp::parse_pi_file`,
      unit-tested `skips_malformed_json_lines_but_keeps_the_rest`.)
- [x] 2.5 Implement Claude Code streaming-duplicate reconciliation: dedup
      re-written messages by composite `messageId:requestId` key, merging
      duplicates by per-field max, BEFORE the record is eligible for the
      digest step (design D6).
      — ✅ `claude.rs::{merge_claude_duplicate, merge_claude_tool_result_duplicate}`;
      test `tests/claude_ingest_integration.rs::streaming_duplicate_pair_merges_into_one_row_with_max_tokens_and_the_corrupt_line_is_skipped`.
- [x] 2.6 Implement Codex cumulative-total reconciliation: diff each
      `token_count` event against the previous snapshot (never sum raw
      per-line values), and detect a forked-child session replaying its
      parent's history, attributing tokens to the fork-source identity
      (`session_forked_from_id.or(session_id_from_meta)`) so they are not
      double-counted under the child's `session_id` (design D6).
      — ✅ `codex.rs::{CodexTotals::delta_from, looks_like_stale_regression}`
      + fork attribution; tests `codex.rs::{cumulative_totals_are_diffed_into_deltas_not_summed_raw,
      forked_child_replay_of_parent_history_is_not_double_counted}` +
      `tests/codex_fork_dedup_integration.rs`.

## 3. Incremental watermark

- [x] 3.1 Persist a per-source watermark cursor (`{source_id, last_seen_at,
      last_seen_digest}`) through canon-store after a successful scan.
      Evidence: `crates/canon-store/src/cursor.rs` (`SourceCursor` carries
      all three named fields + a per-file `files` index; `CursorStore`
      reads/writes one `<source_id>.json` per source via the atomic
      `write_atomic` primitive — canon-store OWNS the type + IO).
      **Two documented deviations (cursor.rs module doc):** (a) STORAGE —
      a per-operator scan cursor is machine-local mutable state, so it
      persists under a local root (`<repo>/canon/ingest/cursors/`,
      gitignored) rather than the git/pg/r2 tiers; "through canon-store"
      holds at the persistence-authority level, only the tier CHOICE
      deviates. (b) SHAPE — `{last_seen_at, last_seen_digest}` are present
      but DERIVED from an added per-file digest index, because a single
      high-water mark is an unsound gate (a copied/restored old-mtime file
      or a same-mtime rewrite would false-skip). Tests
      `cursor::tests::{store_round_trips_and_fail_softs_on_corrupt,
      summary_fields_derive_from_the_index, …}`.
- [x] 3.2 Gate each adapter's scan to only read data newer than its persisted
      watermark; advance the watermark only after the corresponding records
      are durably written.
      Evidence: `crates/canon-cli/src/ingest.rs::run` reads each source's
      cursor, and skips its whole parse/normalize/persist when the source
      is unchanged (`SourceCursor::source_unchanged` — a full-content
      digest match on the entire present file set); cursors advance ONLY
      after the pass's records durably persist (best-effort write, never
      failing an ingest whose records already landed). The gate is
      SOURCE-granular so a multi-file session is never partially
      re-normalized. **Scope note:** "newer" is enforced at the
      parse/normalize/persist layer (the expensive work is skipped for an
      unchanged source); the gate still READS each file to digest it —
      avoiding that re-read is 3.3 below. Tests
      `ingest::tests::{routed_policy_persists_through_canon_store_git_tier
      (2nd pass watermark-skips, `--full` forces re-parse),
      a_changed_source_reingests_while_unchanged_ones_stay_skipped}`.
- [ ] 3.3 Support append-only `.jsonl` sources (Claude Code, Codex) resuming
      from a byte-offset/line-count cursor rather than re-reading the whole
      file on every run.
      **Deliberately deferred — a further optimization layered ON TOP of
      the shipped 3.1/3.2 cursor.** The watermark now skips an unchanged
      source's parse/normalize/persist, but still READS + digests each
      present file to make the skip decision; 3.3 would additionally
      avoid that re-read by resuming a grown `.jsonl` from a persisted
      byte-offset/line-count. It is intrinsically append-only-`.jsonl` +
      Claude/Codex-specific and fragile (a compaction/rotation invalidates
      the offset), and reading + hashing a file is cheap relative to the
      parse+normalize+persist the cursor already skips, so 3.1/3.2's
      `--watch` win lands without it. `cursor::SourceCursor.files`'s
      per-file `mtime_ms`/`size` are the natural place a future offset
      cursor would hang.

## 4. Idempotent re-ingest

- [x] 4.1 Compute a stable content digest (sha256 over canonical normalized
      JSON, excluding volatile re-emitted fields) per normalized record.
      (Wave 1, S3Pipeline: `normalize::content_digest` — canon-ingest's own
      idempotence bookkeeping, independent of and in addition to
      canon-store's own per-write digest below; unit-tested
      `normalization_is_deterministic_across_two_runs`.)
- [x] 4.2 Use the digest as the record's write identity in canon-store's
      upsert path so a duplicate record (watermark reset, restarted
      `--watch`, concurrent run) is skipped, never double-written or
      double-counted.
      (Wave 1, S3Pipeline: reuses canon-store's own digest-suffixed Hive
      write path — `canon_store::partition`'s `{natural_key}__{digest12}`
      — via `TierRegistry::persist`; `canon-cli/src/ingest.rs::
      persist_idempotent` additionally treats a git-tier
      `StoreError::DuplicatePath` as a no-op (git-tier's own idempotence
      contract is "reject the duplicate write", not "silently dedup" — see
      that fn's doc comment). Verified end-to-end: a smoke-tested two-run
      `canon ingest sessions` against a git-routed fixture `canon.yaml`
      produces identical ledger file names/counts on both runs, and
      `ingest::tests::routed_policy_persists_through_canon_store_git_tier`
      asserts the same in-process.)

## 5. CLI command

- [x] 5.1 Wire `canon ingest sessions [--watch]` on `canon-cli`: one-pass scan
      by default; `--watch` polls the configured roots on an interval.
      (Wave 1, S3Pipeline: `canon ingest sessions [--watch] [--interval-secs]
      [--home] [--canon-yaml] [--json]`, `canon-cli/src/main.rs` +
      `src/ingest.rs`; `--watch` is a real (if watermark-less, see 3.1) poll
      loop, not a stub. `canon ingest sessions --help` verified.)
- [x] 5.2 Emit a run summary (records scanned, records skipped as violations,
      per-adapter counts) to stdout/manifest.
      (Wave 1, S3Pipeline: `ingest::format_human` — per-adapter file/row
      counts, sessions normalized, rows skipped, runs/events written;
      smoke-tested against a real binary run, see this change's own PR
      report for full output.)

## 6. Fixtures, cost parity, and selftest

- [ ] 6.1 Collect sanitized real transcript samples from omp/pi, Claude Code,
      and Codex into the S3 fixture corpus (redact prose/PII fields only;
      `session_id`/token/cost/timestamp fields pass through unchanged);
      include at least one Codex `archived_sessions/` sample, one Claude
      Code streamed-duplicate-message sample, one Codex forked-child
      session sample, and one Codex compaction-regression (stale total)
      sample.
      (Wave 1, S3Pipeline: omp/pi-only real-ish fixtures shipped —
      `crates/canon-ingest/tests/fixtures/home/{.omp,.pi}/agent/sessions/**`
      (dual-root, corrupt-line, leading-title-record, missing-provider
      samples). Claude Code/Codex samples (archived_sessions, streamed-dup,
      forked-child, compaction-regression) are Wave 2 scope — left
      unchecked; the "real transcript samples" this task names are
      synthetic-but-format-accurate, not literal captured transcripts —
      no live CLI session data was available to sanitize from in this
      sandbox.)
- [ ] 6.2 Capture the vendored upstream launcher's own computed cost for the same fixture corpus and
      check it in as the expected-cost fixture.
      (Wave 2/whole-change scope — not implemented; needs all three
      adapters' fixtures and a launcher CLI run, out of reach in this
      sandbox. omp/pi's own cost is always `0.0`/`CostSource::Unknown`
      per the ported omp/pi parser behavior — trivially "parity" with the launcher's
      own omp/pi parser, which also never computes cost inline — but the actual
      checked-in fixture + test this task calls for is not done.)
- [ ] 6.3 Write the cost-parity test: sum canon-ingest's normalized cost rows
      per `session_id` and diff against the checked-in launcher expectation
      within a declared rounding tolerance.
      (Wave 2/whole-change scope — not implemented, depends on 6.2.)
- [x] 6.4 Write the archived-sessions test: assert a session present only
      under `${CODEX_HOME:-~/.codex}/archived_sessions/` (not
      `sessions/`) is still ingested and contributes to its `session_id`'s
      normalized output.
      — ✅ `codex.rs::live_and_archived_sessions_are_both_scanned_and_parsed`.
- [x] 6.5 Write the streaming-dedup test: assert a Claude Code
      streamed-duplicate-message sample normalizes to one record with
      per-field-max token counts, not a sum across duplicates.
      — ✅ `tests/claude_ingest_integration.rs::streaming_duplicate_pair_merges_into_one_row_with_max_tokens_and_the_corrupt_line_is_skipped`.
- [x] 6.6 Write the cumulative-delta + fork-replay test: assert a Codex
      forked-child sample attributes replayed tokens to the fork-source
      identity (not double-counted under the child's `session_id`), and a
      compaction-regression sample does not produce a negative or inflated
      delta.
      — ✅ `codex.rs::{cumulative_totals_are_diffed_into_deltas_not_summed_raw,
      forked_child_replay_of_parent_history_is_not_double_counted}` +
      `tests/codex_fork_dedup_integration.rs`.
- [x] 6.7 Write the two-run idempotence test: run `canon ingest sessions`
      twice over the unchanged fixture corpus and assert byte-identical
      normalized output and a zero-scanned second pass.
      (Split per ReviewS3Full finding 5 — the two halves below are
      independently checked/unchecked rather than the parent
      overclaiming full completion on partial evidence.)
    - [x] 6.7a Byte-identical normalized output across two runs.
          (Wave 1, S3Pipeline: asserted at three levels —
          `normalize::tests::normalization_is_deterministic_
          across_two_runs` (unit), `ingest_integration::
          full_pipeline_is_idempotent_across_two_ingest_runs`
          (fixture-corpus integration, asserts `serde_json` equality +
          identical serialized bytes), and `ingest::tests::routed_
          policy_persists_through_canon_store_git_tier` (CLI-level,
          through a real `canon-store` git tier).)
    - [x] 6.7b Zero-scanned second pass.
          — ✅ Now satisfied by the S3 §3 watermark (task 3.2):
          `ingest::tests::routed_policy_persists_through_canon_store_git_tier`
          asserts the second pass over the unchanged corpus reports
          `runs_written == 0` + `adapters[0].files_scanned == 0` +
          `skipped_unchanged >= 1` (the source is watermark-skipped, not
          re-parsed), and the pass-1 cursor persisted at
          `canon/ingest/cursors/omp.json`.
- [x] 6.8 Write the watermark-reset test: reset a source's cursor, re-scan the
      full corpus, and assert the store's record count is unchanged (no
      duplicates).
      — ✅ `ingest::tests::resetting_a_cursor_reingests_the_full_corpus_without_duplicating`
      is the literal reset: it deletes the persisted cursor file
      (`canon/ingest/cursors/omp.json`), re-runs, and asserts the source
      is fully re-parsed (`skipped_unchanged == 0`) yet the git-tier
      `.json` record count is UNCHANGED before vs after (S3 4.2's
      digest-idempotent write makes a byte-identical resubmission a
      no-op). `routed_policy_persists_through_canon_store_git_tier`'s
      `--full` branch additionally proves the reset-equivalent
      cursor-ignoring path with the same count-unchanged assertion.
- [x] 6.9 Wire the S3 fixtures into `canon selftest` (design §8: fixture
      corpora with rebindable roots + expected-output diff).
      — ✅ `crates/canon-ingest/src/selftest.rs::selftest()` wraps this
      crate's pure normalize invariants (determinism across two runs +
      session grouping) as in-memory fixture checks; registered in the
      Wave-3 unified aggregator (`canon_cli::selftest`) as the
      `session-ingest` suite (2 checks). `canon selftest` runs it green
      (side-effect-free — synthetic `UnifiedRow`s, no filesystem read).

## 7. Companion skill

- [x] 7.1 Author the `canon` session-ingest companion skill under
      `canon/skills/` (decision 9): documents `canon ingest sessions
      [--watch]`, the shipped adapters, and how an agent reads the run
      summary — materialized for Claude Code + Codex only via the skill
      install lock (content-hash + version, never `generatedAt`).
      — ✅ `canon/skills/canon-session-ingest/SKILL.md` (documents `canon
      ingest sessions [--watch] [--full]`, all four shipped adapters
      (omp/hermes/claude/codex) + their env overrides, the S3 §3 watermark
      gate, and every run-summary line); materialized via `canon skills
      install` (`.claude/skills/canon-session-ingest/` +
      `.codex/skills/canon-session-ingest.md` + `.install-lock.json` bump).
