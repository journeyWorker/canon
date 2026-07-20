## ADDED Requirements

### Requirement: canon-model owns versioned schemas for the whole artifact family
canon-model SHALL define versioned JSON-schemas covering run/review/clear/
drill ledger records, divergence manifest/review/remediation events,
inventory entries, policy, trajectories, strategies, and sessions.

#### Scenario: A schema-registered kind validates against its own schema
- **WHEN** `canon fmt --check` reads a `kind=review` ledger record
- **THEN** it validates the record against the `review` kind's registered
  JSON-schema, including the envelope fields and the kind-specific fields

### Requirement: ONE canonical Hive partition grammar, declared per kind
Every artifact kind SHALL be governed by a declared `LayoutDescriptor`
(partition keys + leaf filename grammar) under one general Hive rule
(`kind=<kind>/[key=value/]*<leaf>`); layout violations SHALL be detected as
a generalized form of the ledger's existing `_ledger_layout_problem` check.

#### Scenario: features/ layout violation is detected
- **WHEN** `canon fmt --check` finds a `.feature` file outside the
  `kind=feature/area=<area>/` partition grammar
- **THEN** it reports a layout violation for that file, citing the expected
  partition grammar

#### Scenario: A kind with zero partition keys is not flagged for missing area=
- **WHEN** `canon fmt --check` reads a `kind=run` or `kind=drill` ledger
  record filed flat under `kind=<kind>/` with no `area=` segment
- **THEN** it does not report a layout violation, because `run`/`drill` are
  declared with zero partition keys in the registry

### Requirement: `canon fmt --check` reports exactly the audited gaps over local fixtures
`canon fmt --check` SHALL, run over `canon-fmt`'s LOCAL fixture corpus
(`crates/canon-fmt/fixtures/consumer-corpus/`, reproducing the donor consumer repo's
real, audited drift shapes — never a live the donor consumer repo checkout), report
exactly the gaps identified in the 2026-07-10 artifact audit (design §5
S11 table): third partition grammar and missing authoring provenance in
`features/`; missing schema envelope, missing at/actor, and
partition-key-smeared filenames in `inventory/`; missing actor/session/
cost/duration and free-text/`;`-joined refs in `ledger/`; abbreviated
`app_sha` and one-way back-refs in `divergences/`; and missing session/
actor/change/task identity cross-family.

#### Scenario: fmt --check over the fixture corpus matches the audit
- **WHEN** `canon fmt --check` runs over `canon-fmt`'s local fixture corpus
- **THEN** its violation report matches the audited gap list, with no
  unaudited gap reported and no audited gap missed
