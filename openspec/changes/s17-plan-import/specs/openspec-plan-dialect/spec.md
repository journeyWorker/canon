## ADDED Requirements

### Requirement: An openspec change directory maps to exactly one Change record
The openspec dialect adapter SHALL map each `openspec/changes/<slug>/`
directory to ONE `Change` record: `change_id` = the directory basename,
VERBATIM, constructed via `ChangeId::parse` (a basename failing the grammar
skips the whole directory, counted malformed, siblings unaffected);
`title` = the slug; `summary` = the first paragraph under proposal.md's
`## Why` heading, whitespace-normalized (an absent `## Why` heading yields
an empty summary plus a diagnostic, never invented prose). A change
directory with no readable proposal.md SHALL be skipped and counted
malformed — it is not a valid openspec change. A directory with
proposal.md but no tasks.md SHALL still import (a legitimate
proposal-stage change): one `Change` with `status: proposed`, zero `Task`
records, zero diagnostics.

#### Scenario: A well-formed change dir imports its Change record
- **WHEN** the adapter parses a fixture `openspec/changes/s17-plan-import/`
  carrying a proposal.md whose `## Why` opens with a summary paragraph
- **THEN** exactly one `Change` candidate is emitted with
  `change_id = s17-plan-import`, `title = s17-plan-import`, and `summary`
  equal to that first paragraph, whitespace-normalized

#### Scenario: A proposal-only change imports as proposed with zero tasks
- **WHEN** a change dir carries proposal.md but no tasks.md
- **THEN** one `Change` with `status: proposed` is emitted, zero `Task`
  candidates are emitted, and the pass records zero diagnostics for that
  dir

#### Scenario: A dir whose basename is not a valid ChangeId is skipped whole
- **WHEN** a directory named `Bad_Slug!` sits beside well-formed change
  dirs
- **THEN** that directory produces no records, increments the malformed
  count, and every sibling imports normally

### Requirement: ChangeStatus derives deterministically from checkbox tallies and archive location
The adapter SHALL derive `ChangeStatus` as a pure function of the source
snapshot: a change dir under `changes/archive/` → `archived`,
unconditionally; otherwise zero parseable checkbox rows → `proposed`; all
rows done (≥1) → `completed`; ≥1 done and ≥1 open → `in_progress`; none
done → `proposed`. A `**DEFERRED**`/`**DROPPED**`-annotated row counts by
its CHECKBOX state alone — the annotation never invents a status.

#### Scenario: An archived change imports as archived regardless of tallies
- **WHEN** a change dir sits under `openspec/changes/archive/` with every
  checkbox row still open
- **THEN** its `Change` record carries `status: archived` — the archive
  location wins unconditionally

#### Scenario: Checkbox tallies drive the live statuses
- **WHEN** three fixture change dirs carry (a) all rows checked, (b) a mix
  of checked and unchecked rows, and (c) no rows checked
- **THEN** their `Change` records carry `completed`, `in_progress`, and
  `proposed` respectively, and re-parsing the same bytes yields the same
  three statuses

### Requirement: tasks.md checkbox rows map to Task records with task_id parity against the S4 verdict adapter
Each parseable `- [ ]`/`- [x] <n> …` row SHALL map to one `Task` record:
`task_id` = `<change_id>#<n>` via `TaskId::parse` — byte-identical to the
derivation S4's `openspec-task` verdict adapter performs over the same
file, so plan rows and verdict trajectories join on the spine; `status` =
`open`/`done` from the checkbox verbatim; `title` = the row text after the
id token and any annotation, with the evidence suffix excluded;
`evidence_note` = the ` — ✅ <evidence>` suffix when present, else the
`**DEFERRED to §<to>**`/`**DROPPED**` annotation text when present, else
absent. A row outside the base checkbox shape SHALL be ignored as prose
(never counted malformed); a row whose `<n>` fails the task-number grammar
SHALL be skipped and counted.

#### Scenario: task_id derivation is byte-identical to the verdict adapter for every co-emitted row
- **WHEN** the plan adapter and the S4 `openspec-task` verdict adapter
  both parse the same fixture `tasks.md` under the same change dir
- **THEN** for every row the verdict adapter emits an event for, the plan
  adapter's `task_id` for that SAME row is byte-identical — both derive it
  through the one shared `<change_id>#<n>` function — so the verdict
  adapter's emitted `task_id` set is a SUBSET of the plan adapter's (the
  plan side additionally emits untouched-open rows the verdict adapter
  deliberately skips via its `NotApplicable` path), and a join on
  `task_id` matches every row both adapters produce

#### Scenario: A checked row with evidence imports as done with its note
- **WHEN** a row reads `- [x] 2.1 wire the driver — ✅ crates/canon-cli
  tests green`
- **THEN** the emitted `Task` carries `status: done`, `title` = "wire the
  driver", and `evidence_note` = "crates/canon-cli tests green"

#### Scenario: A DEFERRED row keeps its checkbox status and carries the annotation
- **WHEN** a row reads `- [ ] 3.2 **DEFERRED to §4.1** cursor gate`
- **THEN** the emitted `Task` carries `status: open` (the checkbox state,
  verbatim) with the deferral annotation preserved in `evidence_note` —
  the importer never converts a scheduling annotation into a status

### Requirement: Spec-delta scenarios and design prose are dropped with named diagnostics, never imported
The adapter SHALL NOT emit `Scenario` records — `canon inventory sync`
remains the ONLY `Scenario` producer, and an openspec spec-delta
`#### Scenario:` block fails the `scenario_id` grammar and has no core
record an s16 overlay could attach to. Spec-delta scenario blocks SHALL
increment a `spec-delta-scenario` drop count; a `design.md` SHALL
increment a `design-doc` drop count; both appear NAMED in the pass
summary. Proposal prose beyond the first `## Why` paragraph is a
deliberate partial read (the authored document stays the source of
truth), not a drop — no diagnostic.

#### Scenario: Spec deltas produce no Scenario records and a named count
- **WHEN** a change dir carries `specs/some-cap/spec.md` with three
  `#### Scenario:` blocks
- **THEN** the pass emits zero `Scenario` candidates, and the summary's
  `spec-delta-scenario` drop count increments by three — visible, never
  silent

#### Scenario: A design.md is dropped with its own named count
- **WHEN** a change dir carries a design.md
- **THEN** no record is derived from it and the summary's `design-doc`
  drop count increments — the import never guesses a mapping for design
  prose

### Requirement: One openspec checkbox grammar serves both the plan adapter and the S4 verdict adapter
`canon-ingest` SHALL keep exactly ONE openspec checkbox-row grammar
definition: the mirror currently local to the S4 `openspec-task` verdict
adapter is extracted into a shared module consumed by both adapters —
code motion with ZERO behavior change to the verdict adapter, pinned by
its existing tests. `canon-gate::checkbox` remains canon's format
AUTHORITY for the row shape; canon-ingest SHALL still take no canon-gate
dependency.

#### Scenario: The verdict adapter is byte-identical through the refactor
- **WHEN** the S4 verdict adapter's existing test suite runs after the
  grammar extraction
- **THEN** every test passes unchanged — same events, same evidence
  classification, same malformed counts — proving the extraction was code
  motion, not a behavior change

#### Scenario: A grammar fix lands once and serves both consumers
- **WHEN** the shared module's row parsing is corrected or extended
- **THEN** both the plan adapter and the verdict adapter observe the
  change from the single shared definition — there is no second in-crate
  copy left to drift

