## ADDED Requirements

### Requirement: Typed task atoms carry a structured evidence requirement
A typed task atom SHALL declare its evidence requirement as a structured
`evidence` attribute of `{kind, ref}`, where `kind` is validated against the
evidence kinds S5's resolved `policy.yaml` recognizes for that atom's tags. A
typed task atom SHALL NOT accept a free-text evidence string in place of the
structured `evidence` attribute.

#### Scenario: Evidence kind outside the policy-derived domain is rejected
- **GIVEN** S5's resolved policy recognizes evidence kinds `test-run` and
  `review-record` for a task atom's tags
- **WHEN** a typed task atom declares `evidence: { kind: manual-note, ref: "..." }`
- **THEN** validation reports a diagnostic naming `manual-note` as invalid and
  listing `test-run`, `review-record` as the expected kinds

#### Scenario: Valid evidence kind passes validation
- **GIVEN** S5's resolved policy recognizes evidence kind `test-run` for a task
  atom's tags
- **WHEN** a typed task atom declares `evidence: { kind: test-run, ref: "cargo
  test -p canon-report" }`
- **THEN** validation reports no evidence-kind diagnostic for that atom

### Requirement: Typed task atoms compile to the S1 Task model
A validated typed task atom SHALL compile to an S1 `Task` record carrying the
atom's `id`, description, status, and evidence requirement. Compiling an invalid
atom (one that failed vocabulary validation) SHALL NOT produce a `Task` record.

#### Scenario: A valid atom compiles to a Task record
- **GIVEN** a typed task atom that passes vocabulary validation
- **WHEN** the atom is compiled
- **THEN** an S1 `Task` record is produced carrying the atom's id, description,
  status, and evidence requirement

#### Scenario: An invalid atom does not compile
- **GIVEN** a typed task atom that fails vocabulary validation (e.g. missing a
  required attribute)
- **WHEN** compilation is attempted
- **THEN** no `Task` record is produced
- **AND** the same validation diagnostics that failed the atom are surfaced

### Requirement: Compiled task atoms round-trip
The system SHALL, on compiling a valid typed task atom to an S1 `Task` record and
then decompiling that record, reproduce an atom that is equivalent to the
original (same `id`, `tag`, and `attrs` values) and that itself passes vocabulary
validation.

#### Scenario: Compile then decompile reproduces an equivalent atom
- **GIVEN** a typed task atom that passes vocabulary validation
- **WHEN** the atom is compiled to a `Task` record and then decompiled back to an
  atom
- **THEN** the decompiled atom has the same `id`, `tag`, and `attrs` values as
  the original
- **AND** the decompiled atom passes vocabulary validation

### Requirement: `canon gate task` accepts a typed evidence path
`canon gate task <task_id>` SHALL accept a typed task atom's structured evidence
requirement as an alternative to a free-string `--verify-via` argument: given a
task compiled from a typed atom, the gate SHALL check for an `EvidenceRecord`
matching the atom's declared `evidence.kind`/`ref` before allowing the task's
checkbox to flip.

#### Scenario: Gate passes with a matching evidence record
- **GIVEN** a task compiled from a typed atom declaring `evidence: { kind:
  test-run, ref: "cargo test -p canon-report" }`
- **AND** a matching `EvidenceRecord` of kind `test-run` with that ref exists
- **WHEN** `canon gate task <task_id>` runs
- **THEN** the task's checkbox is permitted to flip to done

#### Scenario: Gate blocks without a matching evidence record
- **GIVEN** a task compiled from a typed atom declaring `evidence: { kind:
  test-run, ref: "cargo test -p canon-report" }`
- **AND** no matching `EvidenceRecord` exists
- **WHEN** `canon gate task <task_id>` runs
- **THEN** the task's checkbox flip is blocked with a stable failure-class
  message
