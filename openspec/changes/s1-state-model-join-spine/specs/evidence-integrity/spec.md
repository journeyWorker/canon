## ADDED Requirements

### Requirement: Malformed evidence is no evidence
`canon-model` SHALL provide a validation function that, given a raw
`EvidenceRecord` candidate, either accepts it or returns a structured
violation — never panics and never silently counts a malformed record as
valid evidence (mirrors `tools/parity.py`'s `_load_ledger`/`_ledger_problem`
skip-not-crash contract).

#### Scenario: A malformed record is skipped and reported, not crashed on
- **WHEN** `canon-model::validate_evidence` receives a record missing a
  required envelope field (e.g. no `actor`)
- **THEN** it returns an `EvidenceViolation` describing the missing field
  and the caller's batch-read loop continues to the next record — the
  process never panics or aborts the batch on one malformed record.

#### Scenario: A well-formed record is accepted and never flagged
- **WHEN** `canon-model::validate_evidence` receives a record with a
  complete, correctly-typed envelope and all kind-specific required fields
  present
- **THEN** it returns success with no violation.

### Requirement: Stable failure-class strings
`canon-model` SHALL define failure classes as a fixed, named set (a
`FailureClass` enum with a stable `as_str()` mapping) that every crate
raising `EvidenceViolation`s reuses, rather than each crate inventing its
own ad hoc strings.

#### Scenario: A failure class string never changes across a patch release
- **WHEN** any two `canon-model` versions within the same major version are
  compared
- **THEN** every `FailureClass::as_str()` value present in both versions is
  byte-identical — fixtures and hooks that grep these strings never break
  on a non-breaking upgrade.

#### Scenario: Renaming a failure class requires a coordinated migration
- **WHEN** a `FailureClass` variant's string value is changed
- **THEN** the change is a `canon-model` `schema` version bump (D2) and
  ships together with updated fixtures referencing the old string — never
  a silent rename in an otherwise-unrelated change.
