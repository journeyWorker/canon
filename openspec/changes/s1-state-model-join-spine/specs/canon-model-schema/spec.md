## ADDED Requirements

### Requirement: Closed record-kind set with a shared envelope
`canon-model` SHALL define exactly twelve record kinds — `Change`, `Task`,
`Scenario`, `Session`, `Run`, `Event`, `Handoff`, `Review`, `Divergence`,
`Trajectory`, `StrategyItem`, `EvidenceRecord` — and every one of them SHALL
carry the envelope `{schema: <int>, kind, at, actor}` where `actor` is
`{agent_id, role, session_id?, model?}`, never a bare `by` string.

#### Scenario: Every record type round-trips through serde
- **WHEN** any of the twelve record kinds is constructed, serialized to
  JSON, and deserialized back
- **THEN** the result is equal to the original value (lossless round-trip)
  and the top-level JSON object contains `schema`, `kind`, `at`, and
  `actor` keys with `actor` itself an object containing at least
  `agent_id` and `role`.

#### Scenario: A bare `by` field is rejected at the type level
- **WHEN** a developer attempts to add a plain `by: String` field to any
  record type in `canon-model`
- **THEN** the change does not compile against the shared `Envelope`
  composition pattern (no record type defines its own ad hoc actor field
  outside `Envelope.actor`), enforced by the crate's own type structure,
  not by a lint that can be silenced.

### Requirement: Versioned JSON-schema export
`canon-model` SHALL export one versioned JSON-schema document per record
kind, generated from the same Rust type definitions used for
serialization, into a build output directory.

#### Scenario: Schema export matches the Rust types
- **WHEN** the schema-export step runs against the current `canon-model`
  source
- **THEN** it produces one `.schema.json` file per record kind, each
  declaring the `schema`/`kind`/`at`/`actor` envelope fields plus that
  kind's own fields, with no manual editing step between the Rust type and
  the emitted schema.

#### Scenario: A field addition is reflected without a second registration site
- **WHEN** a field is added to a record kind's Rust struct
- **THEN** re-running schema export changes only that kind's `.schema.json`
  output — no separate schema-authoring file exists to fall out of sync.
