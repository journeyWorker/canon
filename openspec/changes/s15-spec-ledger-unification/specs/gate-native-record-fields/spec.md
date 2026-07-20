## ADDED Requirements

### Requirement: EvidenceRecord defines native lifecycle/trust/staleness fields
`canon_model::EvidenceRecord` SHALL define `lifecycle: Option<TrustLifecycle>`,
`flagged: Option<FlaggedOverlay>`, `evidence_sha: Option<Sha>`,
`run_seq: Option<TotalOrder>`, and `surface_ref: Vec<String>` (a `Vec`
defaulting to empty when absent — the documented exception to the `Option<T>`
shape the other four use) as its OWN native, typed fields, each
optional-with-documented-default (see the absent-default requirement
below) — canon-gate SHALL read these directly off `ctx.evidence`, never
through a canon-gate-owned companion type or a second, independent
raw-JSON re-scan of the ledger.

#### Scenario: The gate reads all five fields natively off EvidenceRecord
- **WHEN** `canon gate check` loads a `GateContext` whose
  `EvidenceRecord`s carry
  `lifecycle`/`flagged`/`evidence_sha`/`surface_ref`/`run_seq`
- **THEN** every check that needs these fields — trust ladder,
  staleness, promotion — reads them directly off the already-typed
  `ctx.evidence` records, and no second `GitTier` construction or raw
  JSON re-scan occurs to recover them

### Requirement: Absent native fields take the documented safe default (three-way read)
The read of the five native fields SHALL be THREE-way, preserving the
pre-s15 `trust.rs` semantics: a legitimately ABSENT field reads as its
documented safe default (absent `lifecycle` = `draft`; absent `flagged`
= unflagged; absent `evidence_sha` = staleness-UNRESOLVABLE, never
assumed stale; absent `surface_ref` = empty; absent `run_seq` = none), a
PRESENT well-formed field reads as its typed value, and only a PRESENT
field that fails to parse is `malformed-evidence`. An absent field SHALL
NEVER be reported as malformed — so a pre-s15 `EvidenceRecord` that
never declared any of these still reads as a legitimate `draft`,
unflagged record.

#### Scenario: An EvidenceRecord with no native trust fields reads as draft/unflagged, not malformed
- **WHEN** the gate loads an `EvidenceRecord` carrying NONE of the five
  native fields (e.g. a record promoted before this change, or evidence
  that simply never declared them)
- **THEN** it reads as `lifecycle = draft`, unflagged,
  staleness-unresolvable, empty `surface_ref`, no `run_seq` — a
  legitimate record that is never classified as `malformed-evidence`

### Requirement: A present-but-malformed native field fails the record into violations
An `EvidenceRecord` SHALL fail into `GateContext.violations` as
malformed evidence when any of its
`lifecycle`/`flagged`/`evidence_sha`/`surface_ref`/`run_seq` fields is
PRESENT but does not match its expected shape; this SHALL NEVER be
implemented via `#[serde(default)]` (or any equivalent mechanism) that
would silently collapse a malformed value into an absent one.

#### Scenario: A malformed flagged value lands the record in violations, not green
- **WHEN** an `EvidenceRecord`'s `flagged` field is present but its
  shape does not parse (e.g. a garbage/partial object where
  `FlaggedOverlay` is expected)
- **THEN** the record fails into `GateContext.violations` as
  `malformed-evidence` — it never evaluates as green, and it never
  silently reads as if `flagged` were absent

#### Scenario: A malformed lifecycle/evidence_sha/surface_ref/run_seq value fails the same way
- **WHEN** any of `lifecycle`, `evidence_sha`, `surface_ref`, or
  `run_seq` is present but malformed on an `EvidenceRecord`
- **THEN** that field's malformation fails the record into violations
  exactly as a malformed `flagged` does — none of the five fields has
  a silent-default escape path

### Requirement: The human-only flagged ratchet is unchanged
Moving `flagged` onto `EvidenceRecord` as a native field SHALL NOT
alter the flag-clear ratchet's enforcement — only a human-attributed
actor may set or clear `flagged`, and clearing remains one-way: an
agent-attributed actor's attempt to clear a flag SHALL still be
rejected.

#### Scenario: An agent-attributed actor still cannot clear a flag
- **WHEN** an agent-attributed actor (not human) attempts to clear a
  `flagged` `EvidenceRecord` now carrying `flagged` as a native field
- **THEN** the clear attempt is still rejected (`FlagClearRejected`),
  identically to its pre-s15 behavior against the companion type

#### Scenario: A human-attributed actor can still clear a flag
- **WHEN** a human-attributed actor attempts to clear a `flagged`
  `EvidenceRecord`
- **THEN** the clear succeeds, exactly as before this change

### Requirement: The gate's review index matches by (project_id, scenario_id)
`canon-gate`'s review index SHALL match a `Review` to an `EvidenceRecord` by
the COMPOSITE `(project_id, scenario_id)` whenever the evidence carries
`Some(project_id)` (`trust.rs::review_index`, consumed by `TrustLadderCheck`'s
`unreviewed-promotion` decision) — a `Review` for one project SHALL NOT satisfy
the reviewed requirement for a DIFFERENT project's evidence sharing the same
`scenario_id`. An `EvidenceRecord` with `project_id = None` (pre-s15/legacy)
SHALL fall back to the prior bare-`scenario_id` match, so no existing gate
behavior regresses. Because `Review.project_id` is REQUIRED, every review
carries a concrete project to match against.

#### Scenario: A review for one project does not satisfy another project's same-scenario_id evidence
- **WHEN** a `Review` exists for `(project_id: app-a, scenario_id: X)` and an
  `EvidenceRecord` carries `(project_id: Some(app-b), scenario_id: X)`
- **THEN** the app-b evidence is NOT counted as reviewed — it remains
  `unreviewed-promotion` — because no review matches its `(app-b, X)` composite
  key

#### Scenario: A legacy evidence record with no project_id matches by bare scenario_id
- **WHEN** an `EvidenceRecord` carries `project_id = None` (pre-s15) and a
  `Review` exists for that `scenario_id` in any project
- **THEN** the legacy evidence is counted as reviewed by the bare-`scenario_id`
  fallback, exactly as before s15
