## Why

The `najun-art` usability review (`target/usage-review/najun-art-dummy/
SYNTHESIS.md`) ran canon's full author→ingest→gate loop against a real
consumer repo and returned four independent **conditional-adopt**
verdicts. Two of the five ranked gaps are addressed here, verbatim from
the synthesis:

**B3 — the plan↔coverage join does not exist (GAP, undermines the core
pitch; Planner's #1 ask, corroborated by Plan-agent).** s17's own
proposal pitches "after one `canon ingest plans` + `canon ingest
artifacts` pass over the same repo, `task_id` joins plan-side `Task`
records to verdict-side trajectories with no schema work"
(`s17-plan-import/proposal.md:205-209`). Traced against the live ledger
and the SQL marts, that pitch is only **2/3** true:

- `task_id` DOES join `Task` ↔ `EvidenceRecord` — `mart_trust_matrix`
  (`crates/canon-store/sql/views.sql:189-226`) reads `int_task_evidence`
  (`:143-150`) and defines `covered = evidence_count > 0` per `task_id`.
  `EvidenceRecord` itself even carries BOTH `task_id: Option<TaskId>` and
  `scenario_id: Option<ScenarioId>` (`crates/canon-model/src/
  records.rs:551,553`) — but only when one evidence-authoring run
  happens to populate both on the SAME row; nothing DECLARES the
  relationship ahead of time.
- s16's coverage overlay (`porting.coverage`, `crates/canon-plugin/src/
  overlay.rs:47-50`, `project.rs:188-217`) keys on `(project_id,
  scenario_id)` ONLY — `attaches_to.core_kind: scenario` is the ONLY
  core kind s16 supports (`crates/canon-plugin/src/manifest/
  resolve.rs:36-38`), and it carries no `task_id` field at all.
- `canon_model::records::Task` (`records.rs:82-90`) has NO field that
  names which `Scenario`(s) it should satisfy. There is no record and
  no view linking a `Task` to the `Scenario`(s) it is meant to cover.

"Covered" therefore means two unrelated things in two marts with zero
cross-reference: `mart_trust_matrix.covered` is evidence-PRESENCE for a
`task_id`; `porting.coverage.covered` is spec-authorship for a
`scenario_id`. The planner's single most important question — *is this
scope both DONE and VERIFIED* — cannot be answered by one canon query
(Planner; corroborated by Plan-agent: "dropped plan constructs are
count-only, so the plan↔corpus loop 'the plan promised 3 scenarios —
were they authored?' cannot close").

**B4 — canon does not dogfood its own plan-import (GAP, credibility;
Planner).** Canon's own repo carries 19 live openspec change dirs
(`openspec/changes/{s0..s17,s10-vocab-pilot}/`, verified by directory
listing; `archive/` is empty) built across exactly the multi-wave,
many-agent tracking s17's `plan-import-connector` exists to serve — yet
canon's own root `canon.yaml` (verified, `tiers:`/`routing:`/`aging:`
only) has **no `plans:` section**, and `canon/ledger/kind=change/` does
not exist on disk. The surface built to give a planner a queryable
multi-change scope view has never been run on the one project big
enough to stress it.

**Both gaps are closeable without reopening s17's shipped shape.**
`crates/canon-cli/src/plans.rs`'s own module doc already documents a
self-referential root exclusion ("a source root that CONTAINS [the git
ledger root / cursor tree] (`--source .` / `root: .`) would otherwise
self-churn forever... excluded by a canonicalized `starts_with` check
computed once per run", `plans.rs:37-54`) — the self-hosting case was
engineered for, just never wired into canon's own config. And s17's own
D3 already establishes the overlay-eligibility test this proposal
reuses: "an s16 overlay attaches to an EXISTING core record —
concretely `attaches_to.core_kind: scenario` only... if a future dialect
carries per-change data that genuinely wants projection onto Change/Task
views, that is an s16 plugin-manifest EXTENSION... never importer-private
logic" (`s17-plan-import/design.md:131-148`). This change is that
extension, scoped to the ONE join the review flagged as missing.

## What Changes

- **`Task` gains an optional, declaratively-authored join field:
  `scenario_refs: Vec<ScenarioId>`** (`canon-model`), empty by default
  (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`, mirroring
  `EvidenceRecord.surface_ref`'s own additive-field precedent) — the
  PLAN side's declaration of which `Scenario`(s) a task is meant to
  satisfy. Never required, never inferred from prose matching (see
  non-goals): a task with no declared refs behaves exactly as it does
  today.
- **A new `[covers: <scenario_id>[, <scenario_id>]*]` trailing grammar
  segment on `tasks.md` checkbox rows**, positioned after the title and
  before the ` — ✅ <evidence>` suffix (or at end-of-line if no evidence
  suffix), landing in BOTH of the row grammar's two homes per s17's own
  D5 discipline: `canon-gate::checkbox` (the format AUTHORITY — read AND
  write) and `canon-ingest::openspec_rows` (the shared read-only mirror
  the S4 verdict adapter and s17's plan adapter both already consume). A
  malformed individual scenario token inside the bracket is dropped from
  `scenario_refs` and counted under a NAMED `malformed-scenario-ref`
  diagnostic scoped to that row — the row's OTHER well-formed refs, and
  the row itself, still import normally (mirrors s17's own per-construct
  soft-fail discipline).
- **s17's openspec plan adapter maps the new grammar onto
  `Task.scenario_refs`** — one additive rule in the existing "Dialect →
  RecordKind mapping" table (`s17-plan-import/design.md`'s own table),
  zero change to `change_id`/`task_id` derivation, zero change to
  `ChangeStatus` derivation (D6), zero change to the S4 verdict adapter's
  emitted `ArtifactEvent`s beyond the shared grammar's title extraction
  now also stripping a `[covers: …]` suffix when one is present (a
  no-op for every row that doesn't use it — pinned by the verdict
  adapter's existing test suite staying green).
- **Two new SQL views layer the unification, `canon-store/sql/
  views.sql`-only — zero canon-store Rust, zero canon-gate change**:
  `int_task_scenario_refs` (one row per `(task_id, scenario_id)` pair,
  `UNNEST`ing `task.scenario_refs` from `stg_records`) and
  `mart_scope_status` (one row per declared `(task_id, scenario_id)`
  pair, joining `mart_trust_matrix` for evidence-presence coverage
  against `porting.coverage` overlay rows — which already appear in
  `stg_records` today, since that view's `kind=*/**/*.json` glob
  (`views.sql:54`) is kind-string-generic and already includes
  namespaced overlay records on disk, verified against
  `GitTier::write_namespaced`'s target root). One query now answers "is
  this task DONE (checkbox), VERIFIED (evidence-covered), and
  SPEC-COVERED (scenario-covered)".
- **canon's own root `canon.yaml` gains a `plans:` section**
  (`sources: [{dialect: openspec, root: openspec/changes}]` — the
  direct-changes-dir root shape s17's `discover_change_dirs` already
  tolerates, deliberately NOT the `root: openspec` near-miss shape a
  sibling change (`s18-uniform-root-and-loud-import`) is separately
  hardening against), and `canon ingest plans` is run against canon's
  own repo as an acceptance step, proving the connector survives the one
  corpus large enough to stress it (19 change dirs, mixed
  proposed/in_progress/completed status, zero archived).

### Added Capabilities

- `task-scenario-join`: `Task.scenario_refs` (optional, additive), the
  `[covers: …]` tasks.md grammar in both canon-gate (authority) and
  canon-ingest (shared mirror), the openspec dialect's mapping rule, the
  per-reference soft-fail diagnostic, and the `int_task_scenario_refs` /
  `mart_scope_status` SQL views that answer "done AND verified AND
  spec-covered" in one query — connector-never-authority preserved
  throughout (`canon gate check` byte-identical before/after; the mart
  is read-only reporting, never a gate/promotion input).
- `self-hosted-plan-import`: canon's own root `canon.yaml` `plans:`
  section pointed at its own `openspec/changes`, plus the acceptance run
  proving `canon ingest plans` imports all 19 of canon's own change dirs
  idempotently (a second pass writes zero new records) without the
  self-referential root triggering runaway re-scan churn.

### Explicit non-goals

- **No 13th `RecordKind`.** `scenario_refs` is an additive field on the
  EXISTING `Task` kind; the 12-member closure
  (`RecordKind::ALL.len() == 12`, three independent assertion sites,
  `s17-plan-import/design.md`'s own acceptance bar) is unchanged and
  re-asserted green as part of this change's own acceptance.
- **No heuristic/text-matching derivation of task↔scenario links.** A
  regex or substring scan over task titles for scenario-id-shaped tokens
  was considered (design.md's Decision 1, option (b)) and rejected: a
  false-positive match silently mis-joins two unrelated records, which
  is a WORSE failure than the join simply not existing, and violates the
  "malformed evidence is no evidence" fail-loud posture every other
  s15-s17 connector holds itself to. The link is always an EXPLICIT,
  operator-authored declaration.
- **No `porting.coverage`-shaped overlay for the task↔scenario link
  itself.** Considered (design.md's Decision 1, option (c)) and
  rejected: s16 overlays enrich an EXTERNAL donor project's view of an
  EXISTING core `Scenario` record; the task↔scenario link is intrinsic
  PLAN content (authored by whoever writes `tasks.md`, at plan-authoring
  time, by the SAME actor who writes the checkbox row it lives on) —
  routing it through a separate plugin-manifest-declared overlay would
  require a THIRD-PARTY plugin.yaml for what is really core plan
  metadata, and would misplace authority the same way an evidence_note
  living outside its own task row would.
- **No sidecar file (e.g. a hand-authored `task-scenarios.yaml`) as the
  reference's home.** Considered and rejected: it is a SECOND corpus
  that can drift from `tasks.md` the way the SYNTHESIS's own B-adjacent
  friction note already warns against ("Coverage rests on hand-authored
  inventory YAML that can drift... an unvalidated claim, not verified
  coverage"). The reference lives inline in the row it describes,
  identically to `evidence_note`, so a `tasks.md` diff shows exactly
  what changed.
- **No change to `change_id`/`task_id` derivation, `ChangeStatus`
  derivation (D6), or the S4 verdict adapter's `ArtifactEvent` shape.**
  This change adds ONE optional field and ONE additive grammar rule; the
  join-spine identity work s17 already closed is untouched.
- **No change to coverage/promotion/gate AUTHORITY.** `canon-gate`'s
  `uncovered-cell` check, S5's trust ladder, and S7's promotion read
  NOTHING introduced here; `mart_scope_status` is read-only SQL reporting
  layered on existing `stg_records`, never a second Rust-side
  aggregation (mirrors S9 design D1's own "`canon-report` renders these,
  it does not recompute them" discipline) and never a gate input —
  `canon gate check` verdicts are byte-identical before and after this
  change lands (an acceptance test).
- **No fix for B1 (the `root:` near-miss exiting 0) or B2 (`canon query`
  breaking from a subdirectory).** Those are separately tracked
  (`s18-uniform-root-and-loud-import` already exists for B1); this
  change's own `plans:` config deliberately uses the CORRECT
  `openspec/changes` root specifically so it does not depend on B1
  landing first, and does not re-trigger the same near-miss on canon's
  own repo.
- **No `--watch`, no new storage tier, no dashboard/report-mart wiring
  beyond the two new SQL views.** `canon-report`'s S9 panels are
  untouched; a future change MAY surface `mart_scope_status` as a
  dashboard panel, but that is additive CLI/UI surface, not this
  change's concern.

## Impact

- **`canon-model`**: `Task.scenario_refs: Vec<ScenarioId>` (additive,
  default empty) on `records.rs`; no new `RecordKind`, no change to
  `TaskId`/`ScenarioId` grammars.
- **`canon-gate`**: `checkbox.rs`'s `TaskRow`/`parse_line`/`format_line`
  extended with the `[covers: …]` segment — the format AUTHORITY change;
  round-trip tests extended, existing round-trip tests for rows without
  the segment stay byte-identical.
- **`canon-ingest`**: `openspec_rows.rs`'s shared `parse_row` mirrors the
  same grammar addition (D5's "one grammar, two consumers" discipline,
  restated); `plan_adapters/openspec.rs` maps the parsed refs onto
  `Task.scenario_refs`; `artifact_adapters/openspec_task.rs` (S4) is
  pinned unchanged by its existing test suite (title extraction is the
  only shared surface, and only rows that adopt `[covers: …]` see a
  different title).
- **`canon-store`**: `sql/views.sql` gains `int_task_scenario_refs` and
  `mart_scope_status` — SQL-only, no Rust, no new storage primitive.
- **`canon.yaml` (canon's own repo root)**: new `plans:` section.
- **`canon-cli`**: none — `canon ingest plans` and its config parsing
  are unchanged; canon's own repo simply becomes a configured source.
- **UNCHANGED**: `canon-plugin` (the `porting.coverage` overlay
  declaration and s16's `attaches_to.core_kind: scenario` restriction are
  read, never modified), `canon-learn`, S7 promotion, `canon inventory
  sync` (still the sole `Scenario` producer).
</content>
