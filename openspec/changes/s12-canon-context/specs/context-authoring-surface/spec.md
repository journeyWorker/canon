## ADDED Requirements

### Requirement: `canon context` emits the project-resolved authoring surface
`canon context [--repo <dir>] [--json]` SHALL emit record kinds + envelope
fields, enum domains, join-key grammars, partition layout, policy-derived
requirements, and a capability version, resolved for the target repo.

#### Scenario: Default invocation emits the human outline
- **WHEN** `canon context` runs with no flags inside a repo with a
  `canon.yaml`
- **THEN** it emits a compact human outline listing kind names, enum names,
  join-key names, and the capability version

#### Scenario: --json emits the full machine-readable surface
- **WHEN** `canon context --json` runs against the same repo
- **THEN** it emits a JSON document whose `kinds`, `enums`, and `joinKeys`
  keys list the identical set of names the outline mode lists

#### Scenario: --repo resolves a target other than the working directory
- **WHEN** `canon context --repo <other-dir>` runs from outside that
  directory
- **THEN** the emitted surface reflects `<other-dir>`'s own `canon.yaml`
  and schema/policy state, not the invoking working directory's

### Requirement: Capability query never blocks on corpus diagnostics
`canon context` SHALL emit its full authoring surface and exit 0 even when
the target repo's corpus has `canon fmt`/`canon gate` diagnostics.

#### Scenario: Surface is emitted over a corpus with known violations
- **WHEN** `canon context` runs against a repo whose corpus currently fails
  `canon fmt --check`
- **THEN** `canon context` still exits 0 and emits the full surface,
  unaffected by the corpus's validation state

### Requirement: Same schema registry as the validator — no second registration site
`canon context` SHALL resolve its surface from the identical
`SchemaRegistry`/`PolicyResolution` API `canon fmt` and `canon gate` call;
no command SHALL maintain an independent copy of schema or policy data.

#### Scenario: A schema change reflects in both validator errors and context output
- **WHEN** an enum member is added to a schema-registered kind
- **THEN** both `canon context --json`'s `enums` entry for that kind AND
  `canon fmt`'s enum-mismatch diagnostic for that kind list the new member,
  from the single schema edit

### Requirement: Deterministic, byte-stable output
`canon context`'s output SHALL be deterministic: sorted maps, stable array
ordering, and byte-identical output across repeated runs over unchanged
input.

#### Scenario: Two consecutive runs over unchanged input are byte-identical
- **WHEN** `canon context --json` runs twice in succession against a
  fixture repo whose corpus and schema/policy state are unchanged
- **THEN** the two outputs are byte-identical

#### Scenario: JSON and outline modes agree on content
- **WHEN** `canon context --json` and `canon context` (outline) run against
  the same fixture repo
- **THEN** every kind, enum, and join-key name present in the JSON output
  also appears by name in the outline output

### Requirement: Validator errors embed "expected one of: …" from the same registry
canon-model's schema validator SHALL, on an enum-value mismatch, emit a
diagnostic in the form `` `<got>` is not a valid value for `<field>` of
`<kind>` (expected one of: <members>) ``, where `<members>` is produced by
the identical enum-domain lookup `canon context` uses to populate its
`enums` field.

#### Scenario: Invalid enum value produces an "expected one of" diagnostic
- **WHEN** an artifact is authored with an enum-typed field set to a value
  outside that field's registered domain
- **THEN** `canon fmt`/`canon gate` reports an error containing "expected
  one of: " followed by the exact member list `canon context --json` would
  report for that same field
