# s35 gate-plan-dialect-seam — design

The trust spine (`canon gate task`) hardcoded ONE plan dialect's
directory layout in two places, and the checkbox ROW GRAMMAR was
duplicated across two crates. This change moves all plan-dialect
on-disk knowledge behind a per-dialect adapter capability and unifies
the grammar into a single dialect-neutral module, leaving `canon-gate`
a pure, dialect-free evidence decision.

## D1 — `PlanWriteBack`: the per-dialect write-back capability

A new trait `canon_ingest::plan_writeback::PlanWriteBack`, registered
ALONGSIDE `PlanAdapter` on each `plan_registry::PlanAdapterEntry` as
`write_back: Option<&'static dyn PlanWriteBack>` (the SAME unit-struct
adapter implements both traits, so the field is `Some(&STATIC)` for
each shipped dialect; a future import-only dialect registers `None`).
Three methods, all layout/grammar, never evidence:

- `locate_task(root, &TaskId) -> Option<PlanTaskLocation>` — WHICH
  document carries the task's row. Deliberately a FILE-existence
  question, not a row-existence one: whether the specific `<n>` row is
  present in the located document is `flip_task`'s concern
  (`RowNotFound`). This preserves the pre-s35 precedence exactly — a
  located-but-rowless task stays a gate-red "no matching row" (exit 1),
  never a "no source located it" usage error (exit 2).
- `flip_task(document, &TaskId, evidence_note) -> Result<FlipDocOutcome,
  WriteBackError>` — the dialect-owned document mutation. Idempotent
  no-op on an already-`[x]` row (`flipped: false`, document byte-
  identical); typed `WriteBackError::RowNotFound` for an absent row;
  typed `WriteBackError::Unsupported { dialect }` for a dialect that
  cannot round-trip its own docs.
- `typed_atoms_path(root, &ChangeId) -> Option<PathBuf>` — WHERE the
  S10 `tasks.vocab.yaml` typed-task file lives for this dialect; `None`
  for a dialect with no typed-vocabulary convention.

The trait never sees an `EvidenceRecord`, a verdict, or a policy. The
crate dependency rule is preserved: `canon-gate` and `canon-ingest`
both depend only on `canon-model`; `canon-cli` (which already depends
on both) is the ONE place they meet. No `canon-gate`↔`canon-ingest`
edge is introduced.

## D2 — Grammar unification: one `task_rows`, formerly two

The checkbox row grammar existed twice: `canon-gate::checkbox` owned
the canonical reader+WRITER (`TaskRow`/`parse_line`/`format_line`, with
the byte-identical `covers_raw` round-trip), and
`canon-ingest::openspec_rows` was an independently-maintained READ-ONLY
mirror (`ParsedRow`/`parse_row`) the S4 verdict adapter and the s17
plan adapters read. These collapse into ONE module:
`canon-ingest::task_rows`, dialect-neutral (it is canon's own row
format, not any dialect's).

- The stricter WRITER semantics win: the unified `TaskRow` is OWNED
  (mutable + re-emittable), carries `covers_raw`, and `format_line`
  round-trips byte-identically — including a `[covers: …]` segment that
  mixes a malformed token between well-formed ones (the s20 review
  finding). The former borrowed `ParsedRow` is deleted; its consumers
  (`artifact_adapters::openspec_task`, `plan_adapters::{openspec,
  superpowers}`) now read the owned `TaskRow` via `parse_line`. The
  join-key derivation (`task_id_for`/`is_task_number`) and
  `Annotation::marker_text()` move with it. Every round-trip test from
  BOTH former modules is preserved in the unified module.
- The `openspec` plan adapter owns only the DIRECTORY layout
  (`<root>/openspec/changes/<change_id>/{tasks.md,tasks.vocab.yaml}`);
  the row grammar it reads is dialect-neutral.

## D3 — `canon-gate` sheds all dialect/markdown knowledge

`canon_gate::checkbox::gate_task` was a document-parsing flip
(`document, task_id, evidence, notes -> Result<GateTaskOutcome,
GateTaskError>`). It becomes a PURE evidence decision:

```
gate_task(task_id, evidence, notes) -> TaskFlipDecision
    Approved { evidence_note: String }  // the ` — ✅ ` suffix text
    Blocked  { violations: Vec<Violation> }
```

The fail-closed semantics are unchanged EXACTLY: only a matching,
non-`Divergent` `EvidenceRecord` approves; a paired `EvidenceNote`
failing `scan_fake_markers` blocks with `fabricated-evidence`; a
missing/divergent record blocks with `unevidenced-flip`;
`NotApplicable` passes alongside `Faithful`. What moved OUT is every
document concern: locating the row, detecting an already-flipped/absent
row, and applying the flip. `canon-gate/src` contains ZERO `openspec`
mentions (grep-clean, comments included) — the crate is dialect-free.

## D4 — `canon gate task` orchestration (canon-cli)

`gate.rs::run_task` deletes the hardcoded `repo.join("openspec")…`
paths. New flow:

1. Resolve plan sources via `plans::load_plan_sources_for_gate` (reuses
   `canon ingest plans`' own `load_plan_sources_from_config`).
2. Locate the task: for each source in config order, look up its
   dialect in `plan_registry`, call `PlanWriteBack::locate_task`;
   first hit wins. No source locating it → loud error naming the
   sources consulted, exit 2.
3. Resolve the typed-atoms file via `typed_atoms_path` from the SAME
   winning source (never a second hardcoded path).
4. Run the pure `canon_gate::gate_task` decision, then delegate the
   mutation to the winning dialect's `flip_task`.

Row-state precedence is preserved by passing `flip_task` the approved
note when the decision is `Approved` and an empty note when `Blocked`
(whose mutated document is then DISCARDED, never written): a missing
row is `RowNotFound` and an already-done row is a no-op success
REGARDLESS of the evidence verdict — exactly the pre-s35 behavior.

## D5 — Compat default

`canon ingest plans` treats an ABSENT `plans:` section as ZERO sources
(a clean no-op scan). `canon gate task` cannot: a pre-s35 consumer with
no `plans:` section, whose `tasks.md` lives at
`<repo>/openspec/changes/<change_id>/`, must keep working. So
`load_plan_sources_for_gate` falls back to the documented default
`[{ dialect: openspec, root: <repo> }]` when config yields zero
sources. The dependence moved from hardcoded to configured-default,
never removed. A PRESENT-but-malformed `plans:` section still fails
loud, exactly as for `canon ingest plans`.

## D6 — superpowers write-back deferral

The superpowers dialect CAN locate a task's plan doc (by slugified
filename stem → `ChangeId` identity, design s30 D2) but its
`### Task N:` sections carry `**Step N:**` checkbox lines with no
canonical per-row evidence-suffix convention to round-trip. So its
`flip_task` returns a loud, typed `WriteBackError::Unsupported {
dialect: "superpowers" }` — never a silent no-op an operator would
mistake for a landed flip — and `typed_atoms_path` is `None`. Adding
per-row evidence write-back for that dialect is a future change, gated
on a grammar authority for the suffix.

## Non-goals

No change to the verdict logic, evidence requirements, hook seams, or
the `task_id = <change_id>#<n>` join-spine grammar (dialect-neutral,
stays in canon-model). The typed-evidence path (S10 part2 D4) is
behaviorally unchanged — only its file-path resolution moved behind the
seam.
