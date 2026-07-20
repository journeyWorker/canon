## ADDED Requirements

### Requirement: Two-layer trust spine evaluation
The system SHALL evaluate every gated artifact through two independent
checks: a static coverage check (does required evidence exist per policy)
and a dynamic verdict-ledger check (did the evidence pass, by whom, how
stale). A record satisfying coverage but never green, or green but not
covered, SHALL never be reported as a single collapsed "done" fact.

#### Scenario: Required cell with no evidence fails coverage
- **WHEN** the gate evaluates an artifact that a policy-derived rule marks
  as requiring evidence, and no evidence record exists for that cell
- **THEN** the gate emits an `uncovered-cell` violation and the artifact is
  never reported green, regardless of any other artifact's state

#### Scenario: Covered cell with a failing verdict is not green
- **WHEN** an evidence record exists for a required cell but its latest
  matching verdict is a failure
- **THEN** the gate reports the cell as covered but not green, and the
  failure is visible as its own fact distinct from the coverage fact

### Requirement: Trust ladder lifecycle enforcement
The system SHALL enforce a trust-level lifecycle — `draft`, `reviewed`,
`ratified` — with `flagged` as an orthogonal, human-only overlay. A
`reviewed` promotion SHALL require an accompanying review-record; its
absence SHALL be reported as `unreviewed-promotion`. `draft` artifacts
SHALL never count as green. `flagged` artifacts SHALL never count as green
regardless of any passing evidence.

#### Scenario: draft is never green
- **WHEN** an artifact carries the `draft` lifecycle tag
- **THEN** the gate never reports that artifact as green, even if a
  matching evidence record exists and passed

#### Scenario: reviewed without a review-record is a violation
- **WHEN** an artifact carries the `reviewed` lifecycle tag but the ledger
  contains no matching review-record for it
- **THEN** the gate emits an `unreviewed-promotion` violation

#### Scenario: flagged overrides passing evidence
- **WHEN** an artifact carries the `flagged` tag and also has a passing,
  non-stale evidence record
- **THEN** the gate reports the artifact as not green, and the violation
  identifies `flagged` as the reason

#### Scenario: clearing flagged requires a human-attributed clear-record
- **WHEN** a `flagged` artifact's tag is removed without a matching,
  human-attributed clear-record staged in the same commit
- **THEN** the gate rejects the clear and the artifact remains `flagged`

#### Scenario: agent-originated clear-record is rejected
- **WHEN** a clear-record's actor field identifies an agent (not a human)
- **THEN** the gate refuses to honor it as a valid clear and the artifact
  remains `flagged`

### Requirement: Policy-derived requirement routing
The system SHALL derive required-evidence cells from a versioned
`policy.yaml` file, never from per-artifact judgment encoded in code.
Tightening required coverage SHALL be expressible as a `policy.yaml` diff
alone, with no change to the artifact corpus.

#### Scenario: Policy change alone tightens coverage
- **WHEN** `policy.yaml` is edited to add a new required-cell rule for an
  existing artifact class
- **THEN** artifacts of that class that previously passed coverage now
  report `uncovered-cell` for the newly required cell, with zero edits to
  the artifacts themselves

#### Scenario: Severity below the required trust level at release
- **WHEN** an artifact's severity requires `human` trust per
  `policy.yaml`'s `trust_required`, but the artifact currently sits at
  `green@agent`
- **THEN** the gate emits `trust-below-required` scoped to the release
  check only — it does not block ordinary (non-release) evaluation

### Requirement: Staleness detection
The system SHALL degrade a passing evidence record to stale when the
surface it covers has changed since the record was produced. When the
record declares a surface ref, staleness SHALL be surface-scoped
(git-diff against that ref); otherwise a policy-configured
`max_commits_behind` ceiling SHALL apply.

#### Scenario: Surface-scoped staleness on a declared ref
- **WHEN** an evidence record declares a surface ref, and a later commit
  changes a file under that surface without a new evidence record
- **THEN** the existing record is reported `stale-evidence`, never counted
  green

#### Scenario: Ceiling staleness with no declared ref
- **WHEN** an evidence record declares no surface ref, and HEAD is more
  than `policy.yaml`'s `max_commits_behind` commits ahead of the record's
  commit
- **THEN** the record is reported `stale-evidence`

### Requirement: Staging-to-promote monotonic run_seq
The system SHALL accept reviewer-written evidence records under an
unordered `_staging/` area with no `run_seq`, and SHALL promote them to
committed records only through a serialized integrator step that assigns a
monotonic `run_seq` per (role, surface), re-validates each candidate with
the same checks the gate applies, and refuses (without consuming a
`run_seq`) any record that fails re-validation.

#### Scenario: Promotion assigns a monotonic run_seq
- **WHEN** two staging records for the same (role, surface) are promoted
  in the same invocation
- **THEN** they receive strictly increasing `run_seq` values with no gaps

#### Scenario: A malformed staging record is refused without consuming a seq
- **WHEN** a staging record fails the gate's own validation during promote
- **THEN** the promote step exits non-zero, the record is never written to
  the committed location, and no `run_seq` is consumed for that failure

#### Scenario: Dry-run prints the plan without writing
- **WHEN** `canon gate promote --dry-run` is invoked over one or more
  staging records
- **THEN** the command prints each record's target path and the `run_seq`
  it would receive, and writes or deletes nothing

### Requirement: Stable failure-class strings
The system SHALL expose every gate failure as one of a fixed, documented
set of failure-class strings. A failure class SHALL never be renamed
without updating every fixture and hook that depends on it in the same
change.

#### Scenario: A violation carries a known failure-class string
- **WHEN** the gate reports any violation
- **THEN** the violation's failure-class field is a member of the
  documented `FAILURE_CLASSES` set

### Requirement: Fixture-corpus selftest
The system SHALL ship a fixture corpus with rebindable roots and an
EXPECTED-violations file per fixture, covering every failure class, and a
`canon gate selftest` command that runs every fixture and fails on any
mismatch between actual and expected violations.

#### Scenario: selftest passes on an unmodified fixture corpus
- **WHEN** `canon gate selftest` runs against the shipped fixture corpus
  with no local modification
- **THEN** every fixture's actual violations match its EXPECTED-violations
  file exactly, and the command exits zero

#### Scenario: selftest fails when a fixture's expectations regress
- **WHEN** a fixture's EXPECTED-violations file is edited to omit a
  violation the gate still produces
- **THEN** `canon gate selftest` reports the mismatch and exits non-zero
