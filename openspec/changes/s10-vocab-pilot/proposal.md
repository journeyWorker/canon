## Why

canon's typed authoring vocabulary (S10, design.md D4) resolves a task
atom's `evidence.kind` domain from this repo's own `canon/policy.yaml` —
a file this repo had never actually authored (`crates/canon-vocab/tests/
canon_core_selftest.rs`'s own module doc: "`canon/policy.yaml` does not
exist at the repo root yet"). Every canon-vocab/canon-gate test to date
supplies its OWN fixture copy, never the real one. This pilot change
authors the real file, proving S10 part2's typed task-atom mechanism end
to end on a genuine (non-canon-vocab-fixture) consumer change: the atom
in `tasks.vocab.yaml` validates against the REAL `canon/vocab/canon.core/`
manifest plus this change's own new `canon/policy.yaml`, compiles to the
S1 `Task` model, round-trips, and gates for real via `canon gate task
s10-vocab-pilot#1` (S10 design.md D4's closing-the-loop-with-S5 bar).

## What Changes

- Add `<repo>/canon/policy.yaml`, declaring the evidence-kind domain
  (`test-run`, `manual-review`) canon-vocab's `Type::Evidence` (D4)
  resolves against for this repo.
- Track that addition as ONE typed task atom (`tasks.vocab.yaml`,
  additive alongside the freeform `tasks.md` this change also carries —
  S10 design.md Non-Goals: the typed format never replaces or migrates
  an existing change's checkbox grammar) using canon.core's own `task`
  directive (D1) — no new `canon/vocab/<pilot>/` plugin needed for this
  pilot.

## Impact

- New capability: none — S10 itself is already landed; this is a
  consumer change AUTHORED WITH it, not a change to the vocabulary
  system.
- Affected: `<repo>/canon/policy.yaml` (new), this change's own
  `tasks.md`/`tasks.vocab.yaml`.
