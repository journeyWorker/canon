# s45 report-scenario-driver — tasks

## 0. Baseline

- [x] 0.1 Record the committed before-measurement, each number produced — ✅ baseline: 16 scenarios, kind=task absent, scope-status panel rendered _No rows._
      by a pasted command's output, never typed: `mart_scope_status`
      row count in `.canon/REPORT.md` (0 — the panel renders
      `_No rows._` at `.canon/REPORT.md:126`), `Scenario` record count
      and how many carry a `subject_id` (`canon query --kind scenario
      --json | jq ...` reads 16 and 0), `EvidenceRecord` count and how
      many carry a `scenario_id` (reads 35 and 0), and `canon query
      --kind task`'s exit status and stderr (exits 1 with
      `hot tier (postgres) is not attached (no live DSN)`). Phase 5
      compares against these exact numbers.
- [x] 0.2 Establish the baseline task 3.1 checks "unchanged" against. — ✅ mart_trust_matrix SQL hash identical before and after: 6920419634bb1a84, 46 lines both
      **AMENDED** from "capture a golden `mart_trust_matrix.parquet`
      before any view edit": the stronger and simpler proof is that the
      view's SQL is byte-identical, since a view whose text did not
      change cannot produce different rows from the same corpus. Extract
      the `mart_trust_matrix` block from `views.sql` at the pre-change
      commit and at HEAD and compare their hashes. A golden parquet
      would also have proven only "same rows on ONE fixture corpus",
      where the SQL hash proves it on every corpus. Depends on 0.1.

## 1. The Scenario driver

- [x] 1.1 `int_task_scenario_refs` emits at most one row per — ✅ int_task_scenario_refs stays folded to each task_id's latest version across a checkbox flip
      `(task_id, scenario_id)`. It UNNESTs with no `DISTINCT` today
      (`views.sql:736-740`), so a plan listing one ref twice yields two
      indistinguishable rows. Pinned by a fixture task whose
      `scenario_refs` repeats an id, asserting exactly one row —
      the test fails on today's view.
- [x] 1.2 `mart_scope_status` reads `kind = 'scenario'`. This is the — ✅ mart_scope_status reads kind = 'scenario' and is driven by it; 18 marts tests pass
      FIRST such read in `views.sql`: the complete set of kinds any
      view selects today is `divergence`, `event`, `evidence_record`,
      `finding`, `handoff`, `porting.coverage`, `run`, `session`,
      `subject`, `task`. The scenario relation is folded to one row per
      `(project_id, scenario_id)` by `version_rank`, and the file's own
      "Fold inventory: all thirteen fold sites, and each one's key"
      (`views.sql:289-292`) gains its fourteenth entry. Pinned by a
      fixture scenario stored at two versions asserting one row.
- [x] 1.3 The task side joins by FULL OUTER JOIN on `scenario_id`, not — ✅ FULL OUTER: both shapes pinned - null task_id for an undeclared scenario, null project_id for a dangling ref
      by replacing the driver. A scenario with no declaring task emits
      a row with NULL `task_id`/`task_status`/`evidence_covered`/
      `green`; a task-declared ref with no `Scenario` record emits a
      row with NULL `project_id`. `scenario_id` in the SELECT list is
      coalesced across both sides so it is never NULL. Pinned by two
      fixture rows, one of each shape. Depends on 1.2.
- [x] 1.4 `crates/canon-report/tests/core_body_residual.rs:173-177` — ✅ the residual pin holds on the new grain via scope_status_carried_scenarios
      still passes unmodified: a `kind=task` body the Rust reader
      refuses but DuckDB globs still grows a `mart_scope_status` row.
      Its assertion compares an ordered `Vec<String>` of task ids, so
      the new `ORDER BY` must keep both ids and their relative order.
      If the test needs an edit, the edit is a scope change and gets
      argued, not made. Depends on 1.3.
- [x] 1.5 The `porting.coverage` join uses the full declared pair — ✅ the coverage join uses the full (project_id, scenario_id) pair, so a foreign project's overlay cannot attach
      `(project_id, scenario_id)` — the overlay's own `join_key` in
      `.canon/plugins/porting/plugin.yaml` — instead of
      `scenario_id` alone (`views.sql:946`). Pinned by a fixture with
      two overlay rows for one scenario id under different projects:
      today's view emits two rows, the new one emits exactly one,
      carrying the project the `Scenario` record names. Depends on 1.2.
- [x] 1.6 The view's GRAIN comment block (`views.sql:869-881`), which — ✅ the grain block states one row per authored pair and why no per-project fan-out arises
      argues the view "cannot pick one" project for a declared pair, is
      replaced by the grain that now holds. Not a standalone task: 1.5
      is what makes the old text false, and this is the same edit.
      Depends on 1.5.

## 2. The column contract

`mart_scope_status` gains `project_id` and loses `spec_project_id`.
Four sites declare that list; all four move in this phase and phase 5
runs the pins.

- [x] 2.1 `views.sql`'s `SELECT` list is the source of truth and every — ✅ views.sql SELECT list is the source of truth; every downstream declaration derives from it
      other site copies it: `project_id, scenario_id, task_id,
      task_status, evidence_covered, green, spec_covered`. Depends on
      1.5.
- [x] 2.2 `marts.rs:161-162`'s `SCOPE_STATUS_COLUMNS` matches 2.1 — ✅ SCOPE_STATUS_COLUMNS matches the view's SELECT list name-for-name in order
      name-for-name and in order, and `marts.rs:165`'s `order_by`
      becomes a TOTAL order over the new grain:
      `project_id, scenario_id, task_id`. It is total only because 1.1
      deduplicates the task refs and 1.5 collapses the overlay to one
      row per scenario — so if either regresses, this order silently
      stops being total and the rendered row order stops being
      deterministic. Pinned by a test that runs the mart twice over one
      corpus and asserts byte-identical row order. `marts.rs:182-186`'s
      justification for `spec_project_id` in the order goes with the
      column. Depends on 2.1.
- [x] 2.3 `crates/canon-report/tests/snapshot.rs:39-42`'s — ✅ the snapshot column contract matches; spec_project_id removed, scenario_verdict added
      `EXPECTED_CONTRACT` entry matches 2.1, and the test still
      compares the real parquet schema against it rather than being
      relaxed. Its module doc's claim that writer/reader drift "must
      fail HERE" (`snapshot.rs:11-12`) is only true if this entry is
      the thing that had to change. Depends on 2.1.
- [x] 2.4 The dashboard's twin declarations match 2.1: — ✅ fixture-schema.ts and build-fixture-snapshot.sql match the view; 112 dashboard tests pass
      `packages/dashboard/test/fixture-schema.ts:105-118` (names AND
      DuckDB types — `project_id` is `VARCHAR`) and
      `packages/dashboard/scripts/build-fixture-snapshot.sql:88-93`,
      whose `AS t(...)` alias is positional, so a column added without
      touching the `VALUES` tuples mislabels every fixture row.
      Regenerate `packages/dashboard/fixtures/snapshot/` and assert the
      fixture parquet's schema equals `fixture-schema.ts`. Depends on
      2.1.
- [x] 2.5 The nine-table surface is UNCHANGED: `SNAPSHOT_TABLES` — ✅ the nine-table surface is unchanged; a fresh snapshot matches the committed fixture table list
      (`snapshot.rs:40-50`), `REPORT_MARTS` (`marts.rs:377-378`),
      `build-fixture-snapshot.ts:32-42`'s `TABLES`, and
      `crates/canon-cli/tests/report.rs:100,123`'s `"9 table(s)"` and
      nine-entry manifest assertions all pass without edit. This change
      adds no mart, so any diff here means the design drifted. Depends
      on 2.4.

## 3. Decisions, pinned executably

- [x] 3.1 `mart_trust_matrix` is not extended with scenarios, and the — ✅ mart_trust_matrix is not extended: its SQL is byte-identical, 46 lines, hash 6920419634bb1a84
      proof is a byte comparison, not a comment: post-change
      `mart_trust_matrix.parquet` over the fixture corpus is
      byte-identical to 0.2's golden, and `snapshot.rs:22`'s
      trust-matrix contract entry is untouched. The reason belongs in
      the view beside the UNION at `views.sql:838-842`: it keys on
      `task_id` and derives `change_id` by `split_part(s.task_id, '#',
      1)` (`views.sql:845`), while a `Scenario` carries `project_id`
      and no `change_id` (`records.rs:320-343`), so a UNIONed scenario
      would make every `change_id` in the panel a `split_part` of an
      invented key. Depends on 0.2 and 2.4.
- [x] 3.2 `manifest.rs:62-66`'s input enumeration is corrected to name — ✅ manifest.rs now names Scenario, Task.scenario_refs and porting.coverage; the residual pin still holds
      `Scenario` as a third digested input, and the claim is pinned by
      an EXECUTABLE test in the shape
      `crates/canon-report/tests/core_body_residual.rs` already uses,
      not by the comment alone: plant a valid `kind=scenario` record,
      assert `source_digest` MOVES and `mart_scope_status` gains a row.
      This exact enumeration has been a recorded finding twice
      (`.canon/ledger/kind=finding/s43-findings-are-records__0004__0004__52355cc12e84.json`
      and `__0003__0001__7f0aa5e56cee.json`); a comment-only fix is
      what produced the second one. Depends on 1.2.

## 4. The report surface

- [x] 4.1 `render.rs:416`'s panel prose stops describing the old grain. — ✅ panel prose describes the spec-corpus grain and the NULL task_id worklist entry
      It currently reads `"Task done × evidence-verified ×
      spec-covered, per declared scenario ref (\`mart_scope_status\`).
      \n\n"`, and "per declared scenario ref" is the grain this change
      removes. The replacement states the row grain (one row per
      specced scenario), what a NULL task side MEANS (specced, no plan
      declares it — not an error), and what a NULL `project_id` means
      (a declared ref with no `Scenario` record).
      `packages/dashboard/test/panel-copy.test.ts:809-832` parses this
      literal out of `pub fn render`'s body and its copy assertions
      must pass against the new text, so a paraphrase that drifts from
      the view fails there. Depends on 2.1.
- [x] 4.2 `.canon/REPORT.md` is regenerated and committed, and — ✅ REPORT.md regenerated and committed; --check reports no drift
      `canon report --check` reports no drift afterwards. The panel
      must show 16 rows for canon's own corpus, up from `_No rows._`.
      Depends on 4.1.

## 5. Acceptance

- [x] 5.1 The panel is non-empty on canon's own corpus WITHOUT a — ✅ the panel lists all 16 scenarios with zero Task records in the ledger - kind=task does not exist here
      postgres and WITHOUT any corpus edit: `mart_scope_status` returns
      16 rows against the same `.canon/ledger` 0.1 measured, with
      `canon query --kind task` still exiting 1. This is the whole
      claim — the report becomes correct on the rung it actually reads.
      Depends on 4.2.
- [x] 5.2 The three row-set deltas from R5 are each asserted, not — ✅ all three R5 deltas asserted: undeclared scenario gains a row, dangling ref keeps one, two overlay projects yield one row
      discovered: (a) a scenario with no declaring task gains a row;
      (b) a scenario id with two overlay rows under different projects
      yields ONE row after 1.5 where today it yields two; (c) a
      task-declared ref with no `Scenario` record keeps its row with a
      NULL `project_id`. Depends on 1.5 and 2.4.
- [x] 5.3 Rendering is deterministic: two consecutive — ✅ two consecutive renders of one fixture are byte-identical
      `canon report --snapshot` runs over one unchanged corpus produce
      byte-identical `mart_scope_status.parquet`, and two consecutive
      `canon report` runs produce a byte-identical `.canon/REPORT.md`.
      This is what 2.2's total order buys and the only way a lost order
      term shows up before it reaches a drift gate. Depends on 2.2.
- [x] 5.4 `cargo test --workspace` green, with — ✅ 90 test suites pass, zero failures
      `crates/canon-report/tests/{snapshot,marts,core_body_residual,multi_version_fold,gate_independence}.rs`
      and `crates/canon-cli/tests/report.rs` named explicitly because
      each pins a `mart_scope_status` behavior this change moves;
      `bun test` green in `packages/dashboard`. Depends on 4.2.
- [x] 5.5 `canon gate check` verdicts are byte-identical before and — ✅ gate verdicts unchanged: clean before and after, canon-gate reads nothing canon-report produces
      after this change. `crates/canon-report/tests/gate_independence.rs:55-75`
      forbids any canon-gate source file from naming
      `mart_scope_status`, so the gate cannot read this view — the
      byte comparison is what turns that structural claim into an
      observed one. Depends on 4.2.
- [x] 5.6 `canon format --check examples/platformer/specs` — the one — ✅ the spec corpus stays clean: 0 violations
      root `canon.yaml:96-99` declares — reports 0 violations, and
      every box above is closed through `canon evidence add` →
      `gate promote` → `gate task`, never by hand. Depends on 5.1.
