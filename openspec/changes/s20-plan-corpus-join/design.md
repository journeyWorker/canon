# Design — s20 plan↔corpus join (task-scenario join + self-hosted plan import)

## Current state (accurate baseline, verified)

- **The join that already exists.** `EvidenceRecord` (`crates/
  canon-model/src/records.rs:544-567`) carries BOTH `task_id:
  Option<TaskId>` (:551) and `scenario_id: Option<ScenarioId>` (:553) on
  the SAME row — but only an evidence-authoring run that happens to
  populate both fields creates that pairing; nothing declares the
  relationship in advance, and `mart_trust_matrix` (`crates/canon-store/
  sql/views.sql:189-226`) only ever reads the `task_id` half through
  `int_task_evidence` (:143-150).
- **The join that s16 built and its own boundary.** `porting.coverage`
  is an overlay: `(namespace, kind) = (porting, coverage)`,
  `attaches_to.core_kind: scenario`, `join_key: [project_id,
  scenario_id]` (`crates/canon-plugin/src/overlay.rs:279-282`,
  `project.rs:188-217`). s16 restricts `attaches_to.core_kind` to
  `scenario` ONLY (`crates/canon-plugin/src/manifest/resolve.rs:36-38`,
  `SUPPORTED_CORE_KIND`) — a generic projection over `task` or any other
  core kind is explicit FUTURE work the resolver rejects today
  (`E-PLUGIN-CORE-KIND`, `diagnostic.rs:29-32`). An overlay record is an
  ordinary namespaced JSON file under the git tier's `kind=<namespace>.
  <kind>/` partition — verified against `GitTier::write_namespaced`'s
  target root, which is the SAME root `stg_git_records`' glob
  (`views.sql:54`, `kind=*/**/*.json`) already walks. `stg_records.kind`
  is read straight off the body's own `kind` string (:59), never
  filtered to the 12-member `RecordKind` enum — so overlay rows are
  ALREADY visible to every `stg_*`/`int_*`/`mart_*` view in this file,
  today, without any store-side change. Nothing currently reads them.
- **The row grammar's two homes (s17 D5, restated).** `canon-gate::
  checkbox` (`crates/canon-gate/src/checkbox.rs:1-107`) is the format
  AUTHORITY: `<indent>- [ ] <id> <title>` /
  `<indent>- [x] <id> <title> — ✅ <evidence>`, with an OPTIONAL
  `**DEFERRED to §<to>**` / `**DROPPED**` annotation immediately after
  `<id>`. `canon-ingest::openspec_rows` (`crates/canon-ingest/src/
  openspec_rows.rs:1-73`) is an independently-maintained, read-only
  MIRROR of the exact same shape, consumed by BOTH the S4 verdict
  adapter (`artifact_adapters/openspec_task.rs`) and s17's plan adapter
  (`plan_adapters/openspec.rs`) — "one grammar, two consumers" (D5), by
  design never a `canon-gate` dependency for `canon-ingest`.
- **`Task`'s current shape.** `envelope, task_id, title, status,
  evidence_note` (`records.rs:82-90`) — no field naming which
  `Scenario`(s) the task is meant to satisfy.
- **Canon's own self-hosting gap.** `canon.yaml` (repo root) has
  `handoff_templates:`/`tiers:`/`routing:`/`aging:` only — no `plans:`
  section, verified by direct read. `openspec/changes/` holds 19 change
  dirs (`s0`–`s17` + `s10-vocab-pilot`; `archive/` is empty), verified
  by directory listing. `crates/canon-cli/src/plans.rs`'s own module doc
  (:37-54) already documents a canonicalized `starts_with` exclusion so
  a source root CONTAINING the importer's own git-ledger root or cursor
  tree (`root: .`) "would otherwise self-churn forever" — engineered,
  never exercised on canon's own repo. `task: pg` routing (`canon.yaml`
  `routing:`) means a self-import's `Task` records need a live
  `CANON_PG_DSN` to persist; s17's documented `unwritten` seam (R6)
  degrades this non-fatally when absent — `Change` records (routed
  `git`) persist regardless.

## Decision 1 — Join mechanism: an optional `Task.scenario_refs` field, not a derivation or an overlay

Three shapes were weighed for "how does a `Task` know which
`Scenario`(s) it should satisfy":

**(a) — CHOSEN. A plan-import `Task` row carries an optional declared
scenario reference: `scenario_refs: Vec<ScenarioId>`, empty by
default.** The reference is authored EXPLICITLY, at plan-authoring time,
by whoever writes the `tasks.md` row it lives on — the same actor, the
same file, the same commit that already carries `evidence_note`. It
requires no inference and no cross-file lookup: `canon query`'s existing
fan-out over `Task` bodies sees it exactly like any other field.

**(b) — REJECTED. A documented derivation/adapter function (mirroring
`task_id_for`/`regime_key`'s "single-function join derivation"
precedent), inferring the link from existing data — e.g. scanning a
task's title text for `<area>.<surface>.<NN>`-shaped substrings.**
Rejected on two grounds. First, correctness: `task_id_for`/`regime_key`
are derivations over data that is ALREADY a reliable, structured key
(a dir basename, a role+repo pair) — a task's free-text title is prose,
not a key; a substring match risks both false positives (an unrelated
number sequence that happens to parse as a `ScenarioId`) and false
negatives (a real reference phrased slightly differently), and a
SILENT mis-join is a strictly worse failure than the join simply not
existing — it violates the "malformed evidence is no evidence" fail-loud
posture every s15-s17 connector holds itself to. Second, precedent:
every other join-spine key in this codebase (`task_id`, `scenario_id`,
`session_id`, `regime_key`) is either a GRAMMAR-VALIDATED identity
minted at construction time or a pure structural decomposition of one
(`TaskId::change_id()`) — never a heuristic scrape of unstructured
prose. There is no comparable derivation to mirror; inventing one here
would be the first.

**(c) — REJECTED. An s16-style overlay** (e.g. a `plan.task-scope`
namespace attaching to `core_kind: scenario` with a `task_id` field,
`join_key: [project_id, scenario_id]`), the SAME shape `porting.coverage`
already uses. Rejected because it inverts WHO owns the data. An overlay
exists so a THIRD PARTY (a donor project's own coverage tooling, in
`porting.coverage`'s case) can enrich canon's view of a core record it
does not otherwise author — s16's own design frames this as "foreign
NAMESPACED kinds beside core dirs, projection at read time"
(`s17-plan-import/design.md:58-59`, restating s16). The task↔scenario
link is not foreign data about a `Scenario`; it is core PLAN content —
the same author, in the same `tasks.md` row, already writes
`evidence_note` inline. Routing it through a plugin-manifest-declared
overlay would require a `plugin.yaml` for what is really a plan-import
mapping rule, and — per s17's own D3, which already checked this exact
question for spec-delta scenarios — "if a future dialect carries
per-change data that genuinely wants projection onto Change/Task views,
that is an s16 plugin-manifest EXTENSION (widening
`attaches_to.core_kind`)... never importer-private logic"
(`design.md:145-148`). Widening `attaches_to.core_kind` to admit `task`
is exactly the kind of s16-authority change D3 correctly deferred; (a)
needs none of it.

**Consequence:** the closed 12-`RecordKind` set is untouched
(`scenario_refs` is an additive field on the EXISTING `Task` kind, never
a 13th kind, never an overlay); `Task`'s own schema-registry binding
(S13 CEL policy surface) picks the new field up the same way it already
picks up `evidence_note`, with no registry code change (S13's binding is
generated from the Rust type, per S1/S13's own discipline).

## Decision 2 — Row grammar: a trailing `[covers: …]` segment, both format homes updated in lockstep

The reference needs an on-disk home in `tasks.md` — the row IS the
unit of authorship a task's other metadata (`evidence_note`) already
lives on. Two shapes were considered:

- **A trailing bracket segment** (chosen):
  `<title> [covers: <scenario_id>[, <scenario_id>]*] — ✅ <evidence>` (or
  end-of-line if no evidence suffix). Positioned AFTER the title and
  BEFORE the evidence marker — the one open slot in the existing row
  shape that collides with neither the leading `**DEFERRED…**`/
  `**DROPPED**` annotation (which occupies the position immediately
  after `<id>`) nor the evidence suffix (which is always the row's
  final segment). A malformed token inside the brackets (fails
  `ScenarioId::parse`, the 3-dot-segment `<area>.<surface>.<NN>`
  grammar) is dropped from `scenario_refs` and counted under a NAMED
  `malformed-scenario-ref` diagnostic scoped to that row's `task_id` —
  the row's other well-formed refs, and the row itself, still import;
  mirrors D4's "a bad `<n>` skipped + counted" and D3's per-construct
  drop-diagnostic discipline, applied at reference granularity instead
  of row granularity because a `covers` list is itself a list of
  independent references, not one atomic construct. A bracket that does
  not parse as a comma-separated list at all (unbalanced brackets,
  empty brackets) is left as ordinary title prose — never guessed at,
  mirroring canon-gate's "malformed input becomes absence" discipline
  for the DEFERRED/DROPPED forms.
- **A leading annotation slot, reusing/extending `Annotation`**
  (rejected): `Annotation::Deferred`/`Dropped` are semantically
  MUTUALLY EXCLUSIVE scheduling states of the row itself (a row is
  either deferred, dropped, or neither) — `covers` is an orthogonal,
  independently-present fact (a task can be `**DEFERRED to §4.2**` AND
  declare `[covers: …]` on the same row). Overloading one slot to carry
  two independent concerns would force a combinator grammar
  (`**DEFERRED to §4.2, covers: …**`) that is harder to read and breaks
  round-trip byte-identity for every EXISTING annotated row the moment
  the writer touches that slot's format.

Both format homes are updated in lockstep, per s17's own D5 discipline
restated: `canon-gate::checkbox` (`TaskRow`, `parse_line`, `format_line`
— the format AUTHORITY, read AND write, since `canon-gate` can also
flip a checkbox and rewrite the row) and `canon-ingest::openspec_rows`
(`parse_row` — the shared READ-ONLY mirror both the S4 verdict adapter
and s17's plan adapter already consume). `canon-ingest` gains no new
dependency (`ScenarioId` is already `canon_model`, already imported by
`openspec_rows.rs:40`). The S4 verdict adapter's title extraction now
also strips a `[covers: …]` suffix when present — a no-op for every row
that doesn't use the new segment, pinned by that adapter's existing
test suite staying green (the same "code motion / shared-grammar
change, zero verdict-layer behavior change" bar s17's own P1 held
itself to).

## Decision 3 — Unifying "covered": two new SQL-only views, no new Rust aggregation

`mart_trust_matrix.covered` (evidence-presence, keyed `task_id`) and
`porting.coverage.covered` (spec-authorship, keyed `scenario_id`) stay
exactly what they are — this change does not redefine either. What was
missing is the JOIN TABLE between them, which `Task.scenario_refs` now
supplies structurally. Two views, both read-only over `stg_records`
(already kind-string-generic, `views.sql:38-86`), mirroring S9 design
D1's "`canon-report` renders these, it does not recompute them" posture
— no second Rust-side aggregation, exactly the discipline every existing
`mart_*` in this file already holds itself to:

```sql
-- int_task_scenario_refs: one row per declared (task_id, scenario_id)
-- pair, unnesting Task.scenario_refs.
CREATE OR REPLACE VIEW int_task_scenario_refs AS
SELECT
    body ->> '$.task_id' AS task_id,
    trim(both '"' from ref.value::VARCHAR) AS scenario_id
FROM stg_records, UNNEST(from_json(body -> '$.scenario_refs', '["JSON"]')) AS ref
WHERE kind = 'task' AND (body -> '$.scenario_refs') IS NOT NULL;

-- mart_scope_status: DONE (checkbox) x VERIFIED (evidence-presence,
-- mart_trust_matrix) x SPEC-COVERED (porting.coverage overlay), one row
-- per declared (task_id, scenario_id) pair. A task with no scenario_refs
-- never appears here (nothing to unify) but still appears in
-- mart_trust_matrix unchanged -- this view is additive, not a
-- replacement.
CREATE OR REPLACE VIEW mart_scope_status AS
SELECT
    r.task_id,
    r.scenario_id,
    tm.task_status,
    tm.covered   AS evidence_covered,
    tm.green,
    cov.covered  AS spec_covered
FROM int_task_scenario_refs r
LEFT JOIN mart_trust_matrix tm ON tm.task_id = r.task_id
LEFT JOIN (
    SELECT body ->> '$.scenario_id' AS scenario_id,
           (body ->> '$.covered')::BOOLEAN AS covered
    FROM stg_records
    WHERE kind = 'porting.coverage'
) cov ON cov.scenario_id = r.scenario_id
ORDER BY r.task_id, r.scenario_id;
```

`porting.coverage` rows are read generically by `kind` string — this
view does not depend on the `porting` plugin being installed; a repo
with no coverage overlay simply gets `spec_covered = NULL` (honestly
absent, matching `mart_trust_matrix`'s own `LEFT JOIN` posture for a
task with no evidence). This is an interim, explicitly-named coupling to
ONE overlay identity (`porting.coverage`) — the same "explicit STUB,
never silently load-bearing" posture `int_evidence_verdicts` already
establishes for its own S5-shaped stand-in; a repo using a DIFFERENT
overlay identity for spec-coverage gets no `spec_covered` signal from
this view until a follow-up generalizes the join to `canon.yaml`-declared
overlay identities (named non-goal).

**Why SQL-only, no canon-gate change:** `canon gate check`'s
`uncovered-cell` authority reads `Scenario`/overlay records directly in
Rust (S5), never this view; `mart_scope_status` is `canon-report`/
dashboard-facing REPORTING, the same authority boundary S9's five
existing panels already hold. The acceptance test asserting `canon gate
check` verdicts stay byte-identical before/after this change lands is
the same test shape s17's own R1 mitigation used.

## Decision 4 — Self-hosting root: `openspec/changes`, not `openspec` or `.`

For B4, canon's own `plans:` source root has three candidate shapes:

- **`root: openspec/changes`** (chosen) — the DIRECT-changes-dir shape
  `discover_change_dirs` already tolerates (s17 design's own dialect
  table, "a repo root containing `openspec/changes/`, a direct changes
  dir, or a fixture tree"). Scans exactly the plan tree, nothing else;
  needs no self-exclusion logic at all, since `canon/ledger` and
  `canon/ingest/cursors` are not under `openspec/changes`.
- **`root: openspec`** (rejected) — this is the EXACT near-miss B1
  documents (`plans.sources[].root: openspec` "makes `discover_change_
  dirs` treat `openspec/changes` itself as one malformed change dir...
  and exit 0"). Using it here would both fail to import anything AND
  quietly reproduce the bug this proposal's sibling change
  (`s18-uniform-root-and-loud-import`) is separately hardening against.
  Rejected explicitly so this change's acceptance step does not
  accidentally depend on B1 landing first.
- **`root: .`** (rejected as the DEFAULT, though it is what
  `plans.rs`'s self-exclusion comment was written to make safe) —
  functionally correct (the canonicalized `starts_with` exclusion
  keeps `canon/ledger`/`canon/ingest/cursors` out of the digest), but
  scans the ENTIRE monorepo (every crate, every doc) on every cursor
  check for a source whose actual content of interest is one directory.
  `plans.rs`'s own module doc already recommends scoping a source's
  `root:` to the real plan tree for exactly this reason, "the same
  reason `crate::ingest`'s own module doc recommends scoping session
  `roots:` to real client home dirs." Kept as a documented FALLBACK if a
  future consumer's plan tree genuinely has no stable subdirectory to
  root at — not this change's choice.

## Risks

- **R1 — cross-crate format-authority change lands late.** Extending
  `canon-gate::checkbox` (a SHIPPED, frozen s5/s17-consumed grammar)
  carries more blast radius than an s17-shaped purely-additive change.
  Mitigated the same way s17's own D5 grammar extraction was: the new
  segment is OPTIONAL and ADDITIVE (an absent `[covers: …]` round-trips
  byte-identically to today's grammar), and the acceptance bar requires
  the S4 verdict adapter's FULL existing test suite green unchanged,
  the same bar D5's original extraction held itself to.
- **R2 — `mart_scope_status` silently under-reports if `Task.
  scenario_refs` isn't adopted.** A task authored before this change (or
  by a dialect that doesn't emit `covers`) simply has an empty
  `scenario_refs` and never appears in `mart_scope_status` — this is the
  HONEST behavior (nothing was declared, nothing is unified), not a bug;
  `mart_trust_matrix` alone still answers "is this task evidence-covered"
  exactly as it does today. Adoption is a documentation/authoring-habit
  concern, out of scope for this change's Rust/SQL surface.
- **R3 — the `porting.coverage`-identity coupling in `mart_scope_status`
  is a named, interim proxy** (Decision 3) — a repo whose spec-coverage
  overlay uses a different plugin identity gets no `spec_covered` signal.
  Accepted and documented, mirroring `int_evidence_verdicts`'s own STUB
  posture; a generalization to arbitrary `canon.yaml`-declared overlay
  identities is explicit follow-up work, not silently promised here.
- **R4 — self-hosting `canon ingest plans` on a live, in-progress
  monorepo writes real ledger records into `canon/ledger/kind=change/`
  and (DSN permitting) `canon_v1.task` in pg.** This is the POINT of
  B4's acceptance step, not a side effect to avoid — but it does mean
  the acceptance run is not perfectly side-effect-free on canon's own
  repo state the way a fixture-rooted selftest is. Mitigated by s17's
  existing idempotence guarantee (a second pass over an unchanged
  `openspec/changes` tree writes zero new records) and by scoping the
  acceptance run's root to `openspec/changes` (Decision 4) so it never
  touches anything outside the plan tree it is meant to import.

## Sequencing

- **P1 — join field + row grammar (canon-model, canon-gate,
  canon-ingest).** `Task.scenario_refs`; `[covers: …]` in
  `canon-gate::checkbox` (authority) and `canon-ingest::openspec_rows`
  (shared mirror); per-reference malformed diagnostic; S4 verdict
  adapter's existing suite re-verified green.
- **P2 — openspec dialect mapping (canon-ingest, after P1).** s17's
  `plan_adapters/openspec.rs` maps the parsed `covers` list onto
  `Task.scenario_refs`; fixture tests extended for the malformed-token
  and multi-ref cases.
- **P3 — unification views (canon-store, after P1-P2 land in a repo with
  real data to query against).** `int_task_scenario_refs` and
  `mart_scope_status` added to `sql/views.sql`; a DuckDB smoke query
  over a fixture corpus (or the najun-art dummy) demonstrates the
  DONE×VERIFIED×SPEC-COVERED triple resolving in one query.
- **P4 — self-hosting (canon.yaml + acceptance run, after P1-P3).**
  `plans:` section added to canon's own root `canon.yaml`; `canon
  ingest plans` run against canon's own `openspec/changes`; idempotent
  re-run verified; `canon gate check` byte-identical before/after.
</content>
