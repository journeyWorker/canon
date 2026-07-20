# s35 — gate-plan-dialect-seam

## Why

Canon's stated boundary is: **plan dialects (openspec, superpowers, …) are
adapter territory — nothing outside `canon ingest` may depend on any one
dialect's on-disk shape.** Today two surfaces violate that boundary:

1. `canon gate task <task_id>` (`crates/canon-cli/src/gate.rs`) hardcodes
   `<repo>/openspec/changes/<change_id>/tasks.md` and
   `<repo>/openspec/changes/<change_id>/tasks.vocab.yaml`. The trust spine
   itself is coupled to one dialect's directory layout — a consumer whose
   plans are superpowers docs (or anything else) cannot use the
   evidence-gated flip at all.
2. `canon-gate/src/checkbox.rs` frames itself as "the openspec checkbox
   grammar" and is the gate crate's own reader/writer for that dialect's
   file rows, duplicating the row grammar `canon-ingest::openspec_rows`
   already owns.

## What Changes

- **New seam:** `canon-ingest::plan_adapter::PlanWriteBack` — an optional
  per-dialect capability alongside `PlanAdapter`:
  - `locate_task(root, task_id) -> Option<PlanTaskLocation>` (file + row)
  - `flip_task(root, task_id, evidence_note) -> FlipOutcome` (fail-closed)
  - `typed_atoms_for(root, change_id) -> Option<Vec<AtomRecord>>` (the
    S10 typed-tasks file is also dialect-owned layout)
  The `openspec` dialect implements all three (absorbing the row grammar
  currently split between `canon-gate::checkbox` and
  `canon-ingest::openspec_rows`); `superpowers` implements
  `locate_task`/`flip_task` against its own checkbox sections.
- **`canon gate task` becomes dialect-agnostic:** it resolves the task's
  plan source from `canon.yaml`'s `plans:` sources (first source whose
  dialect locates the `task_id` wins), runs the UNCHANGED pure
  `canon_gate::gate_task` verdict logic, and delegates the file mutation
  to the resolved dialect's `PlanWriteBack`.
- **Compat default:** a repo with no `plans:` section behaves as
  `plans: { sources: [{ dialect: openspec, root: . }] }` — existing
  consumers keep working; the dependence moves from hardcoded to
  configured-default.
- **`canon-gate` sheds dialect knowledge:** `checkbox.rs`'s grammar moves
  into the openspec plan adapter; `canon-gate` keeps only the pure
  verdict/flip decision (`gate_task`) operating on a dialect-neutral
  `TaskRow` view supplied by the caller.
- Docs/skills updated: trust-spine pages describe "the task's plan file,
  resolved via the configured plan dialect".

## Impact

- Affected: `canon-ingest` (plan_adapter, openspec + superpowers
  adapters), `canon-gate` (checkbox.rs removal/motion), `canon-cli`
  (gate.rs), website trust-spine/cli docs, `trust-spine-gate` +
  `canon-plan-import` skills.
- Non-goals: no change to verdict logic, evidence requirements, hook
  seams, or the `task_id = <change_id>#<n>` join-spine grammar (which is
  dialect-neutral and stays).
