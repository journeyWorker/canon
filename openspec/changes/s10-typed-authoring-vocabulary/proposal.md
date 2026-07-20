## Why

Per decision 10, the donor vocabulary project's plugin system — `plugin.yaml` + `directives/*.yaml` +
`enums.yaml` manifests, resolved into a single capability snapshot that feeds both
the checker and the donor CLI's context command — is adopted **nearly as-is** into canon (user-owned
code, no clean-room constraint) as S10, sequenced last but not dropped (decision 8).
Today, openspec `tasks.md` lines are freeform prose checkboxes with **zero
machine-checkable shape**: a task's evidence requirement, if stated at all, is a
free-text string a human interprets, and S5's evidence-gated flip
(`flipTaskDone`/`scanFakeMarkers`, the donor CLI)
can only validate that *some* `--verify-via` string was supplied, never that it
names evidence of the *right kind* for that task. S10 closes that loop: typed task
atoms declare their own evidence requirement in the same vocabulary system that
validates every other authored artifact, and the same resolution mechanism drives
`canon context` (S12) and, later, an LSP — one registration site, never a second
place a vocabulary rule can be defined and drift from what the checker enforces.

## What Changes

- New `plugin.yaml` + `directives/*.yaml` + `enums.yaml` manifest format, lifted
  from the donor vocabulary project's shape (`id`, `version`, `kind`, `depends`, `exports: {directives,
  enums, …}` at the plugin level; `name`, `attrs: [{name, type, required?,
  default?}]`, `semantics`, `lower` per directive) and pointed at canon's own
  authoring domain (task atoms, handoff body templates) rather than the donor vocabulary project's VN-scene
  vocabulary.
- New capability-snapshot resolution (`fold_env`/`build_input`/
  `resolve_document_snapshot` shape, the donor checker's fold step,
  the donor CLI's snapshot builder): ONE function resolving a project's active
  plugins + profile into a merged directive/enum/type index, consumed identically
  by the checker, by `canon context` (S12), and — later, not built in this change —
  an LSP, so no vocabulary rule can diverge between "what validates" and "what an
  agent is told exists."
- New checker diagnostics reusing the donor's checker crate's exact shape and message format:
  `E-UNKNOWN-DIRECTIVE`/`E-UNKNOWN-ATTR`/`E-MISSING-ATTR` plus the "expected one
  of: …" enum-violation message (the donor checker's enum-violation message) pulled
  from the same enum declarations `canon context` emits — a failed authoring
  attempt teaches the vocabulary from the same source of truth.
- New **typed task atom** schema: a task line's evidence requirement becomes a
  structured attribute (naming an S1 `EvidenceRecord` kind + S5 policy-derived
  requirement), not a free string. Typed task atoms compile to the S1 `Task` model
  and round-trip. **Target** (this change proves the mechanism on one consumer
  repo; it does not migrate every openspec change): replace freeform `tasks.md`
  checkbox lines with typed atoms.
- New **vocabulary-defined handoff body templates**: S1's per-domain handoff body
  template registry (기획/디자인/개발/테스트/…, referenced from `canon.yaml`) is
  itself declared through this same manifest system — a template is a directive
  whose attrs are the domain's required body fields, validated and surfaced by the
  same resolution and diagnostics as task atoms.
- **BREAKING** (scoped to this change's own new surface only, nothing existing):
  none — S10 is net-new; it introduces the vocabulary system and typed atoms
  alongside, not instead of, the freeform `tasks.md` grammar S5 already gates.
- The plugin-system **lift mechanism** (depend on the donor vocabulary project crates as a git/path
  dependency vs. import the source into canon's own workspace) is an explicit open
  question, tracked in design.md, decided at S10 kickoff per design doc §10 Q5 —
  not resolved by this proposal.

## Capabilities

### New Capabilities

- `typed-authoring-vocabulary`: the `plugin.yaml`/`directives/*.yaml`/`enums.yaml`
  manifest format, the capability-snapshot resolution feeding checker + `canon
  context` from one source, and the "expected one of: …" diagnostic shape.
- `typed-task-atoms`: the typed task-atom schema carrying its own evidence
  requirement, compiling to and round-tripping through the S1 `Task` model,
  proven on one real consumer-repo change.
- `vocabulary-defined-handoff-templates`: per-domain handoff body templates
  declared and validated through the same manifest/resolution system.

### Modified Capabilities

_(none — `openspec/specs/` has no prior capabilities for this change to modify.)_

## Impact

- New crate(s) under `crates/` for the manifest/resolution/checker machinery
  (lifted from the donor's manifest crate/the donor's checker crate's shapes), wired into `canon-cli` as
  new validation entry points and into `canon context` (S12) as its snapshot
  source.
- New `canon/vocab/` (or equivalent) home for canon's own core plugin (task-atom +
  handoff-template directives), analogous to the donor's core plugin.
- S1's `Task` model and handoff-body-template registry gain a compile/round-trip
  path from the new typed-atom authoring surface; S1's model itself is unchanged
  (typed atoms compile TO the existing `Task` shape, they do not replace it).
- S5's evidence-gated task flip gains a typed-evidence path alongside its existing
  free-string `--verify-via`; the free-string path is not removed by this change.
- New skill(s) under `canon/skills/` teaching agents to author with the typed
  vocabulary and to run `canon context` before authoring (decision 9, S12 tie-in).
- Depends on S1 (Task/EvidenceRecord model, join spine), S5 (policy-derived
  requirements + evidence-gated flip this closes the loop with), and S12 (`canon
  context`, the consumer of the capability snapshot) — S10 is sequenced last (wave
  W4) precisely so those land first.
