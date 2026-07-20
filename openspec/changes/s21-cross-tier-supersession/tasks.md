# Tasks — s21 cross-tier supersession + deterministic gate clock

Sequencing follows design.md: **P1 (fold digest tie-break) lands before P3 (PgTier history table), which lands before P4 (reader migration)**; P2 (GitTier sorted scan) and P5-P6 (gate clock injection + its test) are independent and may interleave. P7 (closure) depends on all of P1-P6.

## 1. `fold_latest_by_key` total order (P1)

- [x] 1.1 `canon-store/src/fold.rs`: add a `digest: impl Fn(&T) -> &str` parameter to `fold_latest_by_key`; change the tie-break from "later-iterated item wins on equal `at`" to "greatest `(at, digest)` pair wins," compared as data — remove the iteration-order dependency from the doc comment and the implementation both.
- [x] 1.2 Update every existing call site to pass a digest closure: `canon-gate::ledger::latest_verdicts`, `canon-gate::staleness::fold_latest_green_cells`, the divergence fold, the flywheel fold (enumerate via `grep fold_latest_by_key` across the workspace — compiler enforces completeness per design.md R4).
- [x] 1.3 Tests: rename/extend `fold.rs`'s existing tie-break test (`ties_broken_by_iteration_order_the_later_item_wins`) to assert the NEW digest-based outcome; add a test proving the SAME two same-`at` items fold to the SAME winner regardless of which is iterated first (construction-order-independence, the actual machine-independence property).

## 2. `GitTier` sorted scan (P2 — independent of P1/P3/P4)

- [x] 2.1 `canon-store/src/git_tier.rs::scan_kind_where`: add `.sort_by_file_name()` to the `WalkDir` builder.
- [x] 2.2 Test: a fixture with kind-directory entries constructed/named such that unsorted filesystem order would differ from sorted order (e.g. write in reverse-lexicographic name order) asserts `TierReadResult.records`' order is sorted, byte-stable across two separate `GitTier::read` calls.

## 3. PgTier `records_history` (P3 — after P1)

- [x] 3.1 `canon-store/src/pg_tier.rs`: replace `create_table_sql`/`create_index_sql` with `records_history(kind TEXT, id TEXT, at TIMESTAMPTZ, digest TEXT, body JSONB, PRIMARY KEY (kind, id, digest))` + an index on `(kind, id)` for per-key grouping — drop the old `records` table's DDL entirely (design.md R1: no table left behind ambiguously).
- [x] 3.2 `upsert_sql` → `insert_sql`: `INSERT INTO {schema}.records_history (kind, id, at, digest, body) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (kind, id, digest) DO NOTHING RETURNING …` (or an equivalent "did this insert land" signal) — `write_row` sets `WriteReceipt.deduped` from whether the row was actually inserted, preserving the existing `deduped` contract.
- [x] 3.3 `select_sql`/`select_older_than_sql`: point at `records_history`; remove any implicit "one row per key" assumption from the query (no `DISTINCT ON`/`GROUP BY` — the raw multi-version read is the point, design D1/D5).
- [x] 3.4 `PgTier::read` return type/contract: confirm (and doc-comment) it now returns every historical row for a kind, matching `GitTier`/`R2Tier`'s existing `Tier::read` contract — no behavior branch inside `PgTier` that tries to pre-fold.
- [x] 3.5 `PgTier::age`: adapt the `delete_sql`/candidate-read path to the renamed table; confirm per-row `at < cutoff` aging (unchanged semantics) now ages superseded versions independently of whichever version is currently "latest" for their key (design.md R2 — accepted, not a regression).
- [x] 3.6 Tests (extend `pg_tier.rs`'s existing pure-SQL unit tests, offline, no live Postgres): `insert_sql` conflicts on `(kind, id, digest)` and never updates; `select_sql`/`select_older_than_sql` carry no dedup clause; schema-ident validation, DSN-resolution-cascade tests untouched. Live-tier tests (`live-pg` feature, `tests/pg_tier_live.rs`): write two versions of the same `(kind, id)` with different `at`s out of order (older arriving second) → both rows persist, `PgTier::read` returns both, `fold_latest_by_key` over the result picks the newer `at` regardless of arrival order; a same-digest resubmission is a no-op (`deduped: true`, row count unchanged).

## 4. PgTier-routed-kind reader migration (P4 — after P3)

- [x] 4.1 Inventory every reader of a pg-routed kind (`task`, `handoff`, `session`, `run`, `event` per `canon.yaml:38-43`) — every `TierQuery::kind(RecordKind::{Task,Handoff,Session,Run,Event})` call site across `canon-cli`, S9 marts, `canon query` — and classify each as "already applies `fold_latest_by_key`" or "assumes one row per key." (`canon-cli::query::{run,run_with_plugin}` and `canon-cli::artifact_ingest::read_records_for` assumed one row per key; S9's `canon-report/sql/views.sql` unions git+r2 only, never PgTier — out of scope, confirmed by inspection.)
- [x] 4.2 Add `fold_latest_by_key` to every site classified "assumes one row per key" in 4.1 — no site exempted (design.md D5/R3). (`query::fold_pg_routed_kind` gated on `Task|Handoff|Session|Run|Event`, applied in both `run`/`run_with_plugin` before `apply_scope`/`rollup_for`; `artifact_ingest::fold_handoff_records` applied in `read_records_for` for the `handoff` adapter only — `review`/`divergence-native` stay unfolded, per their own git-routed multi-row-by-design contract.)
- [x] 4.3 Acceptance test: for a fixture corpus unchanged by this migration (one write per key, no supersession), every migrated reader returns the IDENTICAL row set — count and content — before and after the migration (row-count parity, design.md R3's explicit mitigation). (`canon-cli/tests/query.rs::fold_is_a_noop_for_an_unsuperseded_{task,handoff}_corpus`.)
- [x] 4.4 Acceptance test: for a fixture corpus with a superseded `Task`/`Handoff` (two writes, same key, second carries a newer `at`), a migrated reader returns exactly the newer version — proving the migration actually closes the correctness gap, not merely preserves the old (buggy-but-lucky) behavior. (`canon-cli/tests/query.rs::fold_resolves_to_the_newer_version_for_a_superseded_{task,handoff}`, both write the newer version FIRST and the older SECOND to prove arrival order is irrelevant.)

## 5. Gate-authority clock injection (P5 — independent of P1-P4)

- [x] 5.1 `canon-gate/src/context.rs`: add `now: DateTime<Utc>` to `GateContext`; `GateContext::load(ctx: GateCtx, registry: &SchemaRegistry, now: DateTime<Utc>) -> Result<Self, GateContextError>` stores it.
- [x] 5.2 `canon-gate/src/staleness.rs::StalenessCheck::run`: replace `let now = Utc::now();` with `let now = ctx.now;`.
- [x] 5.3 `canon-gate/src/trust.rs::ReleaseTrustCheck::run`: replace `let now = Utc::now();` with `let now = ctx.now;`.
- [x] 5.4 `canon-cli/src/gate.rs`: `run_gate_check`/`run_gate_task` each call `Utc::now()` exactly once, at the dispatch boundary (mirroring `scaffold.rs`'s `run_scenario_new`/`run_feature_new` idiom, cited in design.md), and pass it to `GateContext::load`.
- [x] 5.5 Update every in-crate test call site of `GateContext::load` (`canon-gate/src/context.rs`, `dispatch.rs`, `selftest.rs`, `staleness.rs`, `trust.rs` — every site the assignment's grep enumerated) to pass an explicit fixed `now` (a named UTC constant, never `Utc::now()` — design.md R5) rather than defaulting to the production clock.

## 6. Time-bearing-policy determinism test (P6 — after P5)

- [x] 6.1 New fixture: a `policy.yaml` with a `staleness.max_commits_behind` (or `release.trust_required`) CEL predicate over `age_days(...)`, plus a ledger evidence record with a fixed `at`.
- [x] 6.2 Determinism test: two independent `GateContext::load(ctx, &registry, FIXED_NOW)` calls at the SAME injected `now`, followed by `check_set(..).run(&ctx)` — assert the two gate reports are byte-identical (proves the CEL evaluation is genuinely pure given `now`, closing the exact coverage gap the SYNTHESIS/Plan-agent review named).
- [x] 6.3 Sensitivity test: same fixture, two `GateContext::load` calls with DIFFERENT injected `now` values straddling the policy's `age_days` threshold — assert the verdict changes in the expected direction (evidence ages from clean to `stale-evidence`/`trust-below-required`), proving the injected clock is load-bearing, not inert.

## 7. Closure

- [x] 7.1 `cargo build --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace --no-fail-fast` (bare, no pipe masking) all green. (1277 tests passed, 0 failed, 0 ignored; clippy zero warnings.)
- [x] 7.2 `bunx openspec validate --strict s21-cross-tier-supersession` green.
- [x] 7.3 Re-run (unmodified expectations) `canon-cli/tests/plugin_sync.rs`'s and `plans_ingest.rs`'s byte-identity acceptance tests — confirm gate verdicts stay byte-identical with/without a plugin sync or plan import, proving this change altered WHERE the fold/clock inputs come from without altering WHAT the gate decides. (`canon_gate_check_verdicts_are_byte_identical_with_and_without_a_prior_plan_import` + `gate_check_verdicts_are_byte_identical_with_and_without_a_porting_sync_run`, both green, unmodified.)
- [x] 7.4 Structural invariants re-asserted green: `RecordKind::ALL.len() == 12` at all three assertion sites; no canon-gate/canon-learn source reference to anything importer/plugin-specific (connector-never-authority, unaffected by this change but re-checked as a regression guard).
- [x] 7.5 `canon selftest` all suites green. (11/11 suites ok, exit 0.)
