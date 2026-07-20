# Tasks — s24 scope-status surface

Sequencing follows design.md: **P1 (marts.rs fetch) before P2 (render.rs panel) before P3 (lib.rs wiring)**; P4 (snapshot.rs) is independent and may land in any order relative to P1-P3; P5 (fixtures + tests) depends on P1-P4; P6 (closure) depends on all.

## 1. `mart_scope_status` fetch (P1)

- [x] 1.1 `crates/canon-report/src/marts.rs`: add `pub const SCOPE_STATUS_COLUMNS: &[&str] = &["task_id", "scenario_id", "task_status", "evidence_covered", "green", "spec_covered"];` — exactly `mart_scope_status`'s own `SELECT` list (`crates/canon-store/sql/views.sql:271-287`), no renaming or reordering.
- [x] 1.2 `crates/canon-report/src/marts.rs`: add `pub fn fetch_scope_status(roots: &Roots) -> Result<MartResult, ReportError>` calling the existing private `fetch(roots, "mart_scope_status", "task_id, scenario_id", SCOPE_STATUS_COLUMNS)` helper — the identical pattern every other `fetch_*` in this file already uses.

## 2. Render panel (P2 — after 1)

- [x] 2.1 `crates/canon-report/src/render.rs`: add `pub scope_status: MartResult` as the LAST field of `ReportMarts` (design D2 — append, never insert ahead of an existing panel).
- [x] 2.2 `crates/canon-report/src/render.rs::render`: after the existing `## Review burn-down` block, append a `## Scope status` block following the identical shape every existing panel uses — one `out.push_str("## Scope status\n\n")`, one prose line naming the source view and what it answers (e.g. "Task done × evidence-verified × spec-covered, per declared scenario ref (`mart_scope_status`)."), then `render_table(&mut out, &marts.scope_status)`. No new heading/prose/table shape invented — reuse `render_table` unmodified.

## 3. Wire into `report()` (P3 — after 1, 2)

- [x] 3.1 `crates/canon-report/src/lib.rs::report`: add `scope_status: marts::fetch_scope_status(&inputs.roots)?,` as the last field of the `ReportMarts` literal, alongside the existing 5 `fetch_*` calls.

## 4. Snapshot export (P4 — independent of 1-3, after 1 for the constant to exist if sequenced together)

- [x] 4.1 `crates/canon-report/src/snapshot.rs:24-25`: extend `SNAPSHOT_TABLES` to `&["mart_trust_matrix", "mart_session_costs", "mart_role_memory", "mart_flywheel_funnel", "mart_review_burndown", "mart_scope_status"]` — append last (design D2), existing 5 entries and their order untouched.
- [x] 4.2 Update the doc comment immediately above `SNAPSHOT_TABLES` (currently "The five S9-owned marts…") to reflect six panels and name `mart_scope_status`'s s20/s24 provenance, matching this file's existing "every constant/fn doc-comments its own provenance" convention.

## 5. Fixtures + tests (P5 — after 1-4)

- [x] 5.1 `crates/canon-report/fixtures/corpus.rs`: extend the shared fixture corpus with (a) one `Task` carrying a non-empty `scenario_refs` (`[covers: …]`-declared, per `Task::scenario_refs`'s existing field shape) referencing a scenario ID, and (b) one `porting.coverage` overlay record for that scenario ID with a known `covered` boolean — enough for `mart_scope_status` to produce at least one non-NULL, known-value row. Export the fixture's known expected values as named constants in a `corpus::scope_status` module, mirroring `corpus::trust_matrix`'s existing constant-export convention (`TASK_1_COVERED_GREEN` etc.).
- [x] 5.2 `crates/canon-report/tests/marts.rs`: add `scope_status_matches_the_fixture_corpus_exactly` — fetches `marts::fetch_scope_status`, asserts row count and every column value against `corpus::scope_status`'s constants, mirroring the file's existing five `*_matches_the_fixture_corpus_exactly` tests exactly (skip-if-no-duckdb guard included).
- [x] 5.3 `crates/canon-report/tests/byte_stability.rs`: no new test needed — the existing `rendering_the_same_fixture_twice_is_byte_identical` and `digest_header_reflects_the_repo_state_the_corpus_was_built_at` tests exercise the FULL rendered report (now including the new panel) unmodified; confirm both stay green with the fixture extended per 5.1 (byte-identity + timestamp-free assertions cover the new panel automatically since they assert over the whole `report()` output string).
- [x] 5.4 `crates/canon-report/tests/fresh_repo.rs`: change `assert_eq!(content.matches("_No rows._").count(), 5, …)` to `6`, and update the assertion's failure message/doc comment to name 6 panels — proving the new panel inherits `Roots::ensure_seeded`'s existing git-only tolerance (design.md "Current state") with zero new seeding code, on a completely fresh/git-only repo.
- [x] 5.5 `crates/canon-report/tests/snapshot.rs`: extend `EXPECTED_CONTRACT` with `("mart_scope_status", SCOPE_STATUS_COLUMNS-equivalent list)` as the 6th entry (matching `marts::SCOPE_STATUS_COLUMNS` exactly), and update every hardcoded `5` count assertion (`manifest.tables.len()`, on-disk `*.parquet` file count, parsed-JSON `tables.len()`) to `6`.
- [x] 5.6 New test in `crates/canon-report/tests/marts.rs` or `snapshot.rs` (whichever fits the file's existing convention better): a task with NO declared `scenario_refs` contributes NO row to `mart_scope_status` (proves the view's additive-only posture is preserved end-to-end through the new Rust fetch, not just at the SQL layer already covered by `crates/canon-store/tests/e2e_scope_status_duckdb.rs`).

## 5b. Cross-crate snapshot-contract ripples (P5 — the 6th mart's blast radius beyond canon-report)

The SHARED SNAPSHOT CONTRACT (S9, "locked over IRC with the `packages/dashboard` sibling") has consumers OUTSIDE canon-report that hardcode the mart count/list; growing `SNAPSHOT_TABLES` 5->6 ripples to each (ReviewS24 [important] — originally under-scoped in this change's Impact):

- [x] 5b.1 `crates/canon-cli/tests/report.rs::snapshot_writes_*_parquet_files_and_a_manifest`: rename `five`->`six`, `"5 table(s)"`->`"6 table(s)"`, append `"mart_scope_status"` to the asserted table-name list, and `manifest["tables"].len()` `5`->`6`.
- [x] 5b.2 `crates/canon-cli/tests/selftest_fixture.rs::fresh_snapshot_matches_the_committed_dashboard_fixture_contract`: the committed-fixture table-count assertion `5`->`6` (the fresh-vs-committed table-list + per-table column diffs are count-agnostic and need no edit).
- [x] 5b.3 `packages/dashboard/fixtures/snapshot/`: add `mart_scope_status.parquet` (schema `task_id,scenario_id,task_status VARCHAR × evidence_covered,green,spec_covered BOOLEAN`, from a fresh `canon report --snapshot`) and append its `manifest.json` `tables` entry last; frozen `generated_at`/`source_git_sha`/`source_digest` unchanged (the cross-check diffs table list + column schema, never those metadata fields).
- [x] 5b.4 `packages/dashboard/test/fixture-schema.ts::EXPECTED_MART_SCHEMA`: append the `mart_scope_status` column list so `fixture-schema.test.ts` actually validates the new mart's committed-fixture schema (ReviewS24 [nit] — it iterates the static list, so an unlisted mart is silently unchecked). `test/smoke.test.ts` needs no change: it asserts a fixed 5-panel `rowCount>0` + the (unchanged) banner digests, none of which the additive 6th table disturbs.

## 6. Closure

- [ ] 6.1 `cargo build --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace --no-fail-fast` (bare, no pipe masking) all green.
- [ ] 6.2 `bunx openspec validate --strict s24-scope-status-surface` green.
- [x] 6.3 Manually run `canon report --repo <a git-only fixture repo with a Task.scenario_refs + porting.coverage corpus>` and confirm a `## Scope status` section renders with the expected row(s), and `canon report --snapshot <dir>` produces `mart_scope_status.parquet` + a `manifest.json` listing it — the exact end-to-end proof the round-2 findings asked for. (Verified via a throwaway `canon-report` example harness calling the crate's own `report()`/`snapshot()` — the same entry points the bin/future `canon-cli` arm call — against the fixture corpus; output confirmed the panel + both parquet/manifest artifacts, then the harness was removed, not part of the deliverable.)
- [ ] 6.4 Structural invariants re-asserted green: `RecordKind::ALL.len() == 12` at all three assertion sites; `canon gate check` byte-identity acceptance tests (`canon-cli/tests/plugin_sync.rs`, `plans_ingest.rs`) unmodified and still green — this change touches no `canon-gate` file, confirming the "read-only reporting, never a gate input" posture `mart_scope_status`'s own SQL comment already declares.
