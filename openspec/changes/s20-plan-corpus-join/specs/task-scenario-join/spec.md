## ADDED Requirements

### Requirement: Task carries an optional, declaratively-authored scenario-coverage reference
`canon_model::records::Task` SHALL carry an additive field
`scenario_refs: Vec<ScenarioId>`, empty by default
(`#[serde(default, skip_serializing_if = "Vec::is_empty")]`). The field
SHALL never be populated by inference over prose or any other heuristic
derivation — only an explicit, operator-authored declaration SHALL
populate it. A `Task` record with no declared references SHALL behave
identically, in every existing consumer, to a `Task` record from before
this change.

#### Scenario: A Task with no declared scenario refs is unchanged
- **WHEN** an existing `Task` record (or a newly imported one from a row
  with no `[covers: …]` segment) is read by `canon query` or any
  existing S9 mart
- **THEN** `scenario_refs` deserializes to an empty list and every
  existing field/behavior is byte-identical to before this change

#### Scenario: A well-formed scenario reference round-trips
- **WHEN** a `Task` is constructed with `scenario_refs: ["wall.render.01",
  "wall.render.02"]`
- **THEN** the record serializes and deserializes byte-identically, and
  every entry parses as a well-formed `ScenarioId`

### Requirement: tasks.md rows declare scenario coverage via a trailing `[covers: …]` segment, recognized by both grammar homes
`openspec/changes/<slug>/tasks.md` checkbox rows SHALL support an
OPTIONAL trailing `[covers: <scenario_id>[, <scenario_id>]*]` segment,
positioned after the row's title and before the ` — ✅ <evidence>`
suffix (or at end-of-line when no evidence suffix is present). This
segment SHALL be recognized in BOTH `canon-gate::checkbox` (the format
AUTHORITY, read and write) and `canon-ingest::openspec_rows` (the shared
read-only mirror the S4 verdict adapter and the plan adapter both
consume) — the two SHALL stay byte-for-byte in agreement on which rows
carry a `covers` segment and what it contains. A row with no `[covers:
…]` segment SHALL round-trip byte-identically to its pre-existing
grammar.

#### Scenario: A row with a covers segment parses its scenario refs
- **WHEN** a tasks.md row reads
  `- [x] 3.2 Implement the widget renderer [covers: wall.render.01,
  wall.render.02] — ✅ crates/app/src/widget.rs`
- **THEN** both `canon-gate::checkbox` and `canon-ingest::openspec_rows`
  parse `scenario_refs = ["wall.render.01", "wall.render.02"]`, the
  title excludes the bracket segment, and the evidence suffix is
  unaffected

#### Scenario: A row without a covers segment is unaffected
- **WHEN** a tasks.md row carries no `[covers: …]` segment
- **THEN** `format_line`/`parse_line` (canon-gate) and `parse_row`
  (canon-ingest) produce byte-identical output to their pre-existing
  behavior — the S4 verdict adapter's full existing test suite passes
  unchanged

#### Scenario: A covers segment coexists with a DEFERRED/DROPPED annotation
- **WHEN** a row reads
  `- [ ] 4.1 **DEFERRED to §5** Wire the audio bus [covers:
  wall.audio.03]`
- **THEN** both the leading `Deferred { to: "5" }` annotation and the
  trailing `scenario_refs = ["wall.audio.03"]` parse independently on
  the same row — neither grammar position interferes with the other

### Requirement: A malformed scenario token degrades per-reference, never per-row
A `[covers: …]` token that fails `ScenarioId::parse` SHALL be dropped from `scenario_refs` and counted under a NAMED
`malformed-scenario-ref` diagnostic scoped to that row's `task_id`
(the `<area>.<surface>.<NN>` grammar governs what "fails to parse"
means) — never a silent drop, and never sinking the
row's OTHER well-formed references or the row's own `Task` import. A
bracket segment that does not parse as a comma-separated list at all
(unbalanced brackets, an empty `[covers: ]`) SHALL be left as ordinary
title prose, never guessed at as a partial `covers` declaration.

#### Scenario: One malformed token among several does not sink the others
- **WHEN** a row declares `[covers: wall.render.01, not-a-scenario-id,
  wall.render.02]`
- **THEN** the imported `Task.scenario_refs` is
  `["wall.render.01", "wall.render.02"]`, and the pass's diagnostic
  summary counts exactly one `malformed-scenario-ref` naming that row's
  `task_id`

#### Scenario: An unrecognized bracket shape is left as title prose
- **WHEN** a row's title text contains `[covers: ]` (empty) or an
  unbalanced `[covers: wall.render.01` (no closing bracket)
- **THEN** no `scenario_refs` entry and no `malformed-scenario-ref`
  diagnostic are produced for that row — the bracket text remains part
  of the ordinary title, unrecognized rather than partially parsed

### Requirement: The openspec plan dialect maps `covers` onto `Task.scenario_refs` with zero change to task_id/change_id/status derivation
s17's openspec plan adapter (`plan_adapters/openspec.rs`) SHALL map a
row's parsed `covers` list onto the imported `Task.scenario_refs` field.
This mapping SHALL NOT alter `change_id`/`task_id` derivation
(`TaskId::parse`, `<change_id>#<n>`), `ChangeStatus` derivation (design
D6), or any other existing field of the imported `Change`/`Task`
records.

#### Scenario: A covers-bearing row imports task_id-parity-preserving
- **WHEN** a fixture change dir's `tasks.md` row 2.1 declares
  `[covers: world.hotdeal.01]`
- **THEN** the imported `Task.task_id` is byte-identical to what it
  would be without the `covers` segment, and `Task.scenario_refs =
  ["world.hotdeal.01"]`

### Requirement: One SQL query answers DONE, VERIFIED, and SPEC-COVERED for a declared task-scenario pair
`canon-store/sql/views.sql` SHALL expose `int_task_scenario_refs` (one
row per declared `(task_id, scenario_id)` pair, unnesting `Task.
scenario_refs`) and `mart_scope_status` (joining `int_task_scenario_refs`
against `mart_trust_matrix`'s evidence-presence `covered`/`green`
columns and the `porting.coverage` overlay's spec-authorship `covered`
column). A single query over `mart_scope_status` SHALL answer, for any
declared task-scenario pair, whether the task is checkbox-DONE,
evidence-VERIFIED, and spec-COVERED — without a caller separately
querying `mart_trust_matrix` and the coverage overlay and cross-
referencing by hand.

#### Scenario: A task declaring a covered, evidenced scenario resolves fully green
- **WHEN** `mart_scope_status` is queried for a `task_id` whose declared
  `scenario_id` has both a `faithful` evidence record and a
  `covered: true` `porting.coverage` overlay row
- **THEN** the returned row shows `task_status = done` (or whatever the
  checkbox state is), `evidence_covered = true`, `green = true`, and
  `spec_covered = true` — all in one query result row

#### Scenario: A task declaring an unauthored scenario surfaces the gap
- **WHEN** `mart_scope_status` is queried for a `task_id` whose declared
  `scenario_id` has no `porting.coverage` overlay row at all
- **THEN** the returned row shows `spec_covered = NULL` (honestly
  absent, matching `mart_trust_matrix`'s own `LEFT JOIN` posture for a
  task with no evidence) rather than a false `false` or a missing row

#### Scenario: A task with no declared scenario refs is absent from the join mart but unchanged in mart_trust_matrix
- **WHEN** a `Task` has an empty `scenario_refs`
- **THEN** it produces no row in `int_task_scenario_refs`/
  `mart_scope_status`, and its row in `mart_trust_matrix` is unaffected
  — the new views are additive, never a replacement for the existing
  evidence-presence mart

### Requirement: The task-scenario join preserves connector-never-authority and the closed 12-kind set
No gate, coverage, or promotion code path SHALL read `Task.
scenario_refs`, the `[covers: …]` grammar, or `mart_scope_status`.
`RecordKind::ALL` SHALL remain exactly 12 members; `scenario_refs` is an
additive field on the EXISTING `Task` kind, never a 13th kind and never
an overlay.

#### Scenario: Gate verdicts are byte-identical before and after this change
- **WHEN** `canon gate check` runs before and after this change lands
  (including against a repo whose `Task` records carry populated
  `scenario_refs`)
- **THEN** every gate verdict is byte-identical — `mart_scope_status` is
  read-only reporting, never a gate input

#### Scenario: The kind closure survives the join field
- **WHEN** this change lands
- **THEN** `RecordKind::ALL.len() == 12` at all existing assertion
  sites, unchanged, and no code path constructs a record kind outside
  the twelve on account of `scenario_refs`
</content>
