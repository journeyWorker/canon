# s35 gate-plan-dialect-seam — tasks

## 1. canon-ingest — grammar unification (D2)

- [x] 1.1 New dialect-neutral `task_rows` module: OWNED `TaskRow` +
      `Annotation` (+ `marker_text`), `parse_line`/`format_line`
      (byte-identical round-trip incl. `covers_raw`),
      `is_task_number`/`task_id_for`. Deletes `openspec_rows`; every
      round-trip + parse test from BOTH former grammars (canon-gate's
      `checkbox` + canon-ingest's `openspec_rows`) preserved.
      — ✅ `crates/canon-ingest/src/task_rows.rs`; `cargo test -p
      canon-ingest task_rows` green (unit tests incl. malformed-covers
      round-trip).
- [x] 1.2 Migrate the three readers off the borrowed `ParsedRow`/
      `parse_row` onto the owned `TaskRow`/`parse_line`:
      `artifact_adapters::openspec_task` (S4), `plan_adapters::openspec`,
      `plan_adapters::superpowers`; module docs + test names updated.
      — ✅ `cargo test -p canon-ingest` green (136 passed), no
      `openspec_rows`/`ParsedRow`/`parse_row` refs remain.

## 2. canon-ingest — `PlanWriteBack` seam (D1)

- [x] 2.1 `plan_writeback` module: `PlanWriteBack` trait
      (`locate_task`/`flip_task`/`typed_atoms_path`), `PlanTaskLocation`,
      `FlipDocOutcome`, typed `WriteBackError { RowNotFound, Unsupported }`.
      Re-exported from the crate root.
      — ✅ `crates/canon-ingest/src/plan_writeback.rs`.
- [x] 2.2 `plan_registry::PlanAdapterEntry` gains
      `write_back: Option<&'static dyn PlanWriteBack>`; both entries
      carry `Some(&STATIC)`; `find` re-exported as `find_plan_adapter`.
      — ✅ `crates/canon-ingest/src/plan_registry.rs`; registry tests
      green.
- [x] 2.3 openspec dialect implements all three: `locate_task` (FILE
      existence of `<root>/openspec/changes/<id>/tasks.md`), `flip_task`
      (delegates to `format_line`, idempotent, `RowNotFound`),
      `typed_atoms_path` (`tasks.vocab.yaml` sibling).
      — ✅ `crates/canon-ingest/src/plan_adapters/openspec.rs`.
- [x] 2.4 superpowers dialect: `locate_task` (by slugified stem →
      `ChangeId`), `flip_task` → loud `WriteBackError::Unsupported`,
      `typed_atoms_path` → `None`.
      — ✅ `crates/canon-ingest/src/plan_adapters/superpowers.rs`.
- [x] 2.5 Seam tests: openspec locate/flip round-trip + idempotent +
      RowNotFound + typed-atoms-path; superpowers locate-then-Unsupported
      + no typed-atoms.
      — ✅ `crates/canon-ingest/tests/plan_writeback_seam.rs`, 6 passed.

## 3. canon-gate — pure dialect-free decision (D3)

- [x] 3.1 `checkbox::gate_task` refactored to
      `gate_task(task_id, evidence, notes) -> TaskFlipDecision`
      (`Approved { evidence_note } | Blocked { violations }`); all
      document parsing/formatting removed. Fail-closed / `Divergent` /
      `scan_fake_markers` / `unevidenced-flip` / `fabricated-evidence`
      semantics preserved EXACTLY; decision tests adapted.
      — ✅ `crates/canon-gate/src/checkbox.rs`; lib exports + selftest
      taskflip runner updated.
- [x] 3.2 canon-gate is dialect-free: `grep -rn 'openspec'
      crates/canon-gate/src` → 0 (comments included; incidental
      pre-existing mentions in coverage/promote/trust_ladder/lib
      reworded).
      — ✅ verified: 0 matches.
- [x] 3.3 `cargo test -p canon-gate` green incl. `selftest`
      (`unevidenced-flip` + `fabricated-evidence` fixtures still fire).
      — ✅ 110 passed.

## 4. canon-cli — `gate.rs::run_task` orchestration (D4/D5/D6)

- [x] 4.1 Delete hardcoded `repo.join("openspec")…` paths; resolve plan
      sources via `plans::load_plan_sources_for_gate`; first source whose
      dialect `locate_task`s the task wins; none → loud exit 2 naming the
      sources consulted; typed-atoms file via `typed_atoms_path` from the
      same winning source; decision → `flip_task` → write.
      — ✅ `crates/canon-cli/src/gate.rs`; `grep -rn 'join("openspec")'
      crates/canon-cli/src` → 0.
- [x] 4.2 Compat default: absent `plans:` → `[{openspec, root: repo}]`
      (`load_plan_sources_for_gate`, reusing the ingest loader).
      — ✅ `crates/canon-cli/src/plans.rs`.
- [x] 4.3 Existing `tests/gate.rs` passes UNMODIFIED (incl. the
      unknown-task-id "no matching row" exit-1 assertion, satisfied by
      `WriteBackError::RowNotFound`'s Display).
      — ✅ 18 pre-existing gate tests green with no assertion edits.
- [x] 4.4 New CLI tests: compat default (canon.yaml present, no
      `plans:`), multi-source resolution order (task in a later source),
      first-source-wins (task in both), no-source-located (exit 2).
      — ✅ 4 new tests in `tests/gate.rs`, 22 total green.

## 5. Docs

- [x] 5.1 Module docs across all three crates state the new boundary
      ("plan dialects are adapter territory; the gate is dialect-free"):
      `task_rows`, `plan_writeback`, `plan_registry`, both plan adapters,
      `openspec_task`; canon-gate `checkbox`; canon-cli `gate` +
      `plans`.
      — ✅ dense doc comments citing s35 D1–D6 on each.
- [x] 5.2 `design.md` (seam shape D1, grammar unification D2, gate
      shedding D3, orchestration D4, compat default D5, superpowers
      deferral D6) + this `tasks.md`.
      — ✅ this change dir.

## 6. Verification

- [x] 6.1 `cargo test -p canon-ingest -p canon-gate -p canon-cli` green
      for every s35-owned surface (grammar, seam, gate decision, CLI
      orchestration).
      — ✅ canon-ingest 136 + seam 6; canon-gate 110; canon-cli gate 22
      — all green. (The two `canon-cli/tests/context.rs` failures are
      s36's canon-model `subject` kind addition raising the kind count
      12→13, not an s35 surface — coordinated with the model agent.)
