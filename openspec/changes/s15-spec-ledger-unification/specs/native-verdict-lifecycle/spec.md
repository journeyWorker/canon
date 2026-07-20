## ADDED Requirements

### Requirement: canon review add writes a native, attributed Review
`canon review add` SHALL construct and write a native
`canon_model::Review` record, attributed to the invoking actor via the
envelope, with its `provenance_ref` enforced to be present — exactly
one of `upstream_ref`/`original_spec_ref` — a review lacking a resolvable
provenance ref SHALL be refused, never written with an empty or
synthesized ref. Without a native producer, `Review`/`Divergence` have
no way to exist on the ledger at all and canon-gate's review index
reads an eternally-empty set (every cell `unreviewed-promotion`
forever) — `canon review add` is the producer that closes that gap for
`Review`.

#### Scenario: canon review add writes an attributed Review with a valid provenance ref
- **WHEN** `canon review add` runs with a scenario_id, reviewer, pin,
  and a valid `--upstream-ref` (or `--original-spec-ref`)
- **THEN** a `Review` record is written whose `envelope.actor`
  identifies the invoking actor and whose `provenance_ref` matches the
  supplied ref

#### Scenario: A missing provenance ref is refused, not defaulted
- **WHEN** `canon review add` runs with neither `--upstream-ref` nor
  `--original-spec-ref` supplied
- **THEN** the command refuses to write a `Review` record and exits
  non-zero

### Requirement: canon divergence stage/promote assigns the monotonic run_seq
`canon divergence stage` SHALL write an unordered staging candidate
carrying no `run_seq`; `canon divergence promote` SHALL assign each
staged candidate a monotonic `run_seq` at promotion time; a refused
candidate SHALL NOT consume a `run_seq` — the next successfully
promoted candidate SHALL receive the sequence number the refusal would
otherwise have consumed. Without this native producer,
`canon-gate`'s divergence fold has no source of `Divergence` records at
all, leaving the S9 divergence burn-down permanently empty.

#### Scenario: Stage then promote assigns a monotonic run_seq
- **WHEN** two `Divergence` candidates are staged in sequence and then
  promoted together
- **THEN** each promoted `Divergence` record carries a `run_seq`, and
  the second promoted record's `run_seq` is strictly greater than the
  first's

#### Scenario: A refused candidate consumes no run_seq
- **WHEN** a staged `Divergence` candidate fails re-validation at
  promote time (refused) alongside a second, valid candidate
- **THEN** the refused candidate is left in staging with no `run_seq`
  assigned, and the valid candidate's assigned `run_seq` is exactly
  what it would have received had the refused candidate never been
  staged

### Requirement: Staging→promote extended to Divergence, partition axis (project_id, role, surface)
`canon-gate::promote` SHALL accept `RecordKind::Divergence`
candidates, not only `EvidenceRecord` (its prior hardcoded target),
computing each candidate's `run_seq`-partition key as
`(project_id, role, surface)` — `role` taken from the writing actor,
`surface` derived from the divergence's own `scenario_id`.

#### Scenario: Divergence promotion partitions run_seq by (project_id, role, surface)
- **WHEN** two `Divergence` candidates share the same
  `(project_id, role, surface)` partition and are promoted together
- **THEN** they receive monotonically increasing `run_seq` values
  within that partition, independent of any other partition's
  `run_seq` sequence

#### Scenario: A candidate with no derivable partition key is refused
- **WHEN** a staged `Divergence` candidate carries neither a resolvable
  `role` nor a `surface` derivable from its `scenario_id`
- **THEN** promotion refuses the candidate as malformed evidence,
  consuming no `run_seq`

### Requirement: DivergenceStatus gains StillDivergent and Deferred
`DivergenceStatus` SHALL be extended, additively, with
`StillDivergent` and `Deferred { reason: String, expiry: DateTime<Utc> }`
variants alongside the existing `Open`/`Resolved`; a pre-existing
`{open, resolved}`-only fixture SHALL continue to deserialize
unchanged.

#### Scenario: A pre-s15 open/resolved fixture still parses
- **WHEN** a `Divergence` record serialized with only
  `status: "open"` (or `"resolved"`) from before this change is
  deserialized
- **THEN** it parses successfully into the corresponding
  `DivergenceStatus::Open`/`Resolved` variant, unaffected by the new
  variants

#### Scenario: canon divergence resolve/defer write the new statuses
- **WHEN** `canon divergence defer` runs with a reason and expiry for
  one divergence, and separately `canon divergence resolve` runs for a
  still-open divergence whose binding has NOT changed
- **THEN** the deferred divergence's status is
  `Deferred{reason, expiry}` and the resolved one's is `Resolved`

### Requirement: Pure fold_to_current_state: run_seq primary, round tiebreak-only
canon-model SHALL provide a PURE function
`fold_to_current_state(records, live_bindings, as_of)` that groups
`Divergence` records by `(project_id, scenario_id)`, ranks records
within a group by `run_seq` as the SOLE primary ordering key, uses
`round` only as a tiebreak among equal `run_seq` values — never as an
independent `Ord` axis — and honors `as_of` when evaluating `Deferred`
expiry.

#### Scenario: A lower run_seq at a higher round still folds before a higher run_seq
- **WHEN** a group contains one record with `(run_seq: 3, round: 9)`
  and another with `(run_seq: 4, round: 1)`
- **THEN** the fold treats the `run_seq: 4` record as the LATER
  (winning) one, regardless of its lower `round` — `round` never
  overrides `run_seq` ordering

#### Scenario: round is a pure tiebreak among equal run_seq values
- **WHEN** two records in the SAME group share the identical `run_seq`
- **THEN** the fold breaks the tie by comparing `round`, and only in
  that equal-`run_seq` case

#### Scenario: as_of governs Deferred expiry
- **WHEN** a group's winning record is `Deferred { reason, expiry }`
  and `fold_to_current_state` is called with `as_of` past `expiry`
- **THEN** the fold's output no longer treats that group as deferred
  as of `as_of`; calling it again with `as_of` before `expiry` still
  treats it as deferred

### Requirement: ResolvedInvalid is fold-derived, never persisted; no-TOCTOU re-check taken as input
`fold_to_current_state` SHALL emit a SEPARATE `FoldedState` output type
distinct from `DivergenceStatus`; a `FoldedState::ResolvedInvalid`
variant SHALL be derivable ONLY at fold time, from a live-binding
re-check — a comparison of the scenario's CURRENT app state (the `app_sha` the
caller reads off the latest live evidence) against the `sha` the `Divergence`
resolved against: the SOLE live-checkable axis (WHO/WHEN a divergence was
resolved are its own immutable provenance, and a superseding resolution is
handled by `run_seq` ranking; a `digest` axis is reserved in `BindingSnapshot`
for a future comparable source) — passed into the function as an INPUT
parameter, never computed
by re-fetching state internally (canon-model cannot depend on
canon-store; the caller owns fetching, eliminating TOCTOU). It SHALL
NEVER be persisted as a `DivergenceStatus` variant.

#### Scenario: A resolved binding whose live ledger record changed downgrades to ResolvedInvalid at fold time
- **WHEN** a group's winning record has `status: Resolved` but the
  supplied `live_bindings` re-check shows the scenario's CURRENT app `sha`
  no longer matches the `sha` the divergence resolved against
- **THEN** `fold_to_current_state` returns `FoldedState::ResolvedInvalid`
  for that group — the on-disk `Divergence` record's `status` field
  itself is never rewritten

#### Scenario: ResolvedInvalid never exists as a persisted DivergenceStatus
- **WHEN** `DivergenceStatus`'s variant set is inspected
- **THEN** no `ResolvedInvalid` variant exists there — it is
  exclusively a `FoldedState` output variant
