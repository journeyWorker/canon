# Tasks — s20 plan↔corpus join (task-scenario join + self-hosted plan import)

Sequencing follows design.md: **P1 (join field + row grammar) lands
before P2 (openspec dialect mapping), which lands before P3 (SQL
unification views)** — the field and its on-disk grammar must exist
before a dialect can populate it, and populated data must exist before
a view can join over it; **P4 (self-hosting) depends on P1-P3** being
present so canon's own `canon ingest plans` run actually exercises the
new field. Mirrors s17's own identity-before-producers discipline.

## 1. join field + row grammar (P1)

- [x] 1.1 `canon-model`: add `Task.scenario_refs: Vec<ScenarioId>`
      (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`),
      mirroring `EvidenceRecord.surface_ref`'s additive-field shape;
      `Task::new`'s existing signature stays unchanged (a `with_
      scenario_refs` builder, mirroring `EvidenceRecord`'s `with_*`
      pattern).
- [x] 1.2 `canon-gate::checkbox`: extend `TaskRow`/`parse_line`/
      `format_line` with the trailing `[covers: <scenario_id>[,
      <scenario_id>]*]` segment (design D2); round-trip byte-identity
      for rows with no segment is a pinned regression test; a malformed
      token is dropped from the parsed list with a `malformed-scenario-
      ref` diagnostic naming the row's task fragment, the row's other
      refs and its own import still succeed.
- [x] 1.3 `canon-ingest::openspec_rows`: mirror the same grammar
      addition in `parse_row` (design D5 restated) — independently
      implemented, byte-for-byte agreeing with 1.2 on which rows carry a
      `covers` segment and what it contains.
- [x] 1.4 Tests: `canon-gate::checkbox`'s full existing round-trip suite
      stays green unchanged; new round-trip tests cover a covers-only
      row, a covers+DEFERRED row, a malformed-token row, and an
      unbalanced/empty-bracket row (left as title prose); the S4
      verdict adapter's (`artifact_adapters/openspec_task.rs`) full
      existing test suite passes unchanged against the extended shared
      module.

## 2. openspec dialect mapping (P2 — after P1)

- [x] 2.1 `plan_adapters/openspec.rs`: map a row's parsed `covers` list
      onto the imported `Task.scenario_refs`; zero change to
      `change_id`/`task_id` derivation or `ChangeStatus` derivation
      (D6).
- [x] 2.2 Tests: a fixture change dir with covers-bearing rows
      round-trips `Task.scenario_refs` correctly; `task_id` parity
      against a covers-free fixture of the same row is byte-identical;
      the malformed-scenario-ref diagnostic surfaces in the pass
      summary named per row.

## 3. SQL unification views (P3 — after P2)

- [x] 3.1 `canon-store/sql/views.sql`: add `int_task_scenario_refs`
      (one row per declared `(task_id, scenario_id)` pair, `UNNEST`ing
      `Task.scenario_refs` from `stg_records`).
- [x] 3.2 Add `mart_scope_status`, joining `int_task_scenario_refs`
      against `mart_trust_matrix` (evidence-presence) and the
      `porting.coverage` overlay rows already visible in `stg_records`
      (spec-authorship) — `LEFT JOIN` on both sides so an absent
      evidence record or absent overlay row surfaces as an honest
      `NULL`, never a dropped row.
- [x] 3.3 Verification: a DuckDB smoke query over a fixture/dummy corpus
      (or the najun-art dummy, if still present under
      `target/usage-review/`) demonstrates a task declaring a covered,
      evidenced scenario resolving `evidence_covered = true, green =
      true, spec_covered = true` in ONE query, and a task declaring an
      unauthored scenario resolving `spec_covered = NULL` rather than a
      missing row.

## 4. self-hosting (P4 — after P1-P3)

- [x] 4.1 Add a `plans:` section to canon's own root `canon.yaml`:
      `sources: [{dialect: openspec, root: openspec/changes}]` (design
      D4 — the direct-changes-dir root, never `root: openspec`).
- [x] 4.2 Run `canon ingest plans` against canon's own repo; verify the
      pass reports one `Change` per live `openspec/changes/<slug>/` dir
      (N = the on-disk count at run time, zero `malformed`, zero
      `duplicate-change-id`) and `canon/ledger/kind=change/` gains the
      corresponding files. DONE: 23 live change dirs on disk == 23
      `changes_persisted`, 0 `duplicate_change_id`; the 2 `malformed`
      entries are pre-existing `invalid-task-number-grammar` rows in
      `s3-session-ingest/tasks.md` (unrelated to change-dir parsing,
      s18-added named diagnostics working as designed).
- [x] 4.3 Verify the `Task` outcome matches the live environment:
      persisted into `canon_v1.task` if `CANON_PG_DSN` is reachable, or
      reported via the documented `unwritten` seam (non-fatal, `Change`
      writes unaffected) if not — either is acceptable acceptance
      evidence per design D-notes; record which case applied. DONE:
      `CANON_PG_DSN` unset in this environment -> all 498 parsed Task
      candidates reported via the `unwritten` seam (non-fatal); all 23
      `Change` records persisted to the git tier regardless.
- [x] 4.4 Re-run `canon ingest plans` immediately after 4.2/4.3 with no
      source-tree edits; verify zero new records (cursor-digest match,
      self-referential exclusion of `canon/ledger`/`canon/ingest/
      cursors` confirmed — the pass does not self-churn). DONE at the
      storage layer: `canon/ledger/kind=change/` file count stayed at
      23 across both passes (git tier's own content-digest dedup, S17's
      foundational write-identity guarantee — zero NEW files). NOTE
      (deviation, environment-caused not s20-caused): `cursor_advanced`
      reports `false` on BOTH passes here because `CANON_PG_DSN` is
      unreachable in this sandbox — per the self-hosted-plan-import
      spec's own "the source's watermark cursor is NOT advanced in the
      unwritten case," which the same pg-unreachability triggers on
      every pass, so `skipped_unchanged` never flips true in this
      environment. The self-exclusion of `canon/ledger`/`canon/ingest/
      cursors` is unaffected by this (that root is `openspec/changes`,
      disjoint from `canon/ledger` either way) and the git-tier dedup
      above is the load-bearing idempotence proof independent of the
      cursor.
- [x] 4.5 Run `canon gate check` before and after the 4.2 import pass;
      verify every verdict is byte-identical. DONE: `canon gate check`
      exits 0 clean before, after the first import pass, and after the
      second (idempotent) pass — `diff`'d byte-identical all three
      times.

## 5. Verification

- [x] 5.1 `cargo build --workspace` + `cargo clippy --workspace
      --all-targets -- -D warnings` + `cargo test --workspace
      --no-fail-fast` (bare, no pipe masking) all green. DONE (Wave-1
      re-verification): all three commands re-run clean after the
      Wave-1 review fixes landed (byte-preserving malformed `[covers:
      …]` round-trip in `gate_task`/`format_line` + task_id-scoped
      `malformed-scenario-ref` diagnostic).
- [x] 5.2 `bunx openspec validate --strict s20-plan-corpus-join` green.
      DONE (Wave-1 re-verification).
- [x] 5.3 `canon selftest` all suites green, including the existing
      plan-import fixture corpus (unaffected by the additive field).
      DONE (Wave-1 re-verification): `plan-import (3 check(s))` green
      alongside every other registered suite.
- [x] 5.4 Structural invariants re-asserted green: `RecordKind::ALL.len()
      == 12` at all three assertion sites; no canon-gate/canon-learn
      source reference to `mart_scope_status` or `Task.scenario_refs`.
      DONE: `RecordKind::ALL.len() == 12` tests pass at all three sites
      (canon-model, canon-store fixture); grepped canon-gate's
      DECISION logic (gate_check/trust/trust_ladder/promote/dispatch)
      and canon-learn — zero references (the only `scenario_refs`
      matches in canon-gate are `checkbox.rs`'s own row-GRAMMAR field,
      the format authority this change's own task 1.2 adds, never gate
      decision logic).
</content>
