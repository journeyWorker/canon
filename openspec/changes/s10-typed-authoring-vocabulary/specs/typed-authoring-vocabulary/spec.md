## ADDED Requirements

### Requirement: Vocabulary is declared via plugin.yaml + directives/*.yaml + enums.yaml
A vocabulary plugin SHALL be declared by a `plugin.yaml` file with `id`, `version`,
`kind`, and an `exports` map naming a `directives` directory and an `enums` file.
Each file under the exported `directives` directory SHALL declare a list of
directives, each with a `name` and a list of `attrs`, where each attr has a `name`
and a `type` (a scalar, an inline enum, or a list of either). The exported `enums`
file SHALL declare a flat map of enum name to member list.

#### Scenario: A well-formed plugin resolves to a directive index
- **GIVEN** a `plugin.yaml` exporting one `directives/*.yaml` file with a `task`
  directive and one `enums.yaml` with a `status` enum
- **WHEN** the plugin is resolved
- **THEN** the resulting directive index contains a `task` entry with its
  declared attrs
- **AND** the resulting enum index contains the `status` enum with its declared
  members

### Requirement: One capability-snapshot resolution feeds the checker and `canon context`
There SHALL be exactly one resolution function that merges a project's active
plugins and profile into a capability snapshot (directive index, enum index).
The checker and `canon context` (S12) SHALL both call this same function; neither
SHALL independently re-derive a partial vocabulary view from raw manifest files.

#### Scenario: Checker and context agree on the same vocabulary
- **GIVEN** a project with a resolved capability snapshot containing directive
  `task` with a required attr `desc`
- **WHEN** the checker validates an atom missing `desc`
- **THEN** it reports `E-MISSING-ATTR` naming `desc`
- **AND** `canon context` run against the same project lists `task`'s `desc` attr
  as required, using the identical resolved snapshot

### Requirement: Unknown directive, unknown attribute, and missing required attribute are checker-validated
Validating an atom against the capability snapshot SHALL report `E-UNKNOWN-
DIRECTIVE` when the atom's tag does not resolve to any directive in the snapshot,
`E-UNKNOWN-ATTR` when a supplied attribute is not declared on the resolved
directive, and `E-MISSING-ATTR` when a declared required attribute is absent.

#### Scenario: Unknown directive tag
- **WHEN** an atom's tag does not match any directive in the resolved snapshot
- **THEN** validation reports `E-UNKNOWN-DIRECTIVE`

#### Scenario: Unknown attribute on a known directive
- **GIVEN** a resolved directive `task` that does not declare an attr named
  `priority`
- **WHEN** an atom tagged `task` supplies a `priority` attribute
- **THEN** validation reports `E-UNKNOWN-ATTR` naming `priority`

#### Scenario: Missing required attribute
- **GIVEN** a resolved directive `task` declaring `desc` as required
- **WHEN** an atom tagged `task` omits `desc`
- **THEN** validation reports `E-MISSING-ATTR` naming `desc`

### Requirement: Enum-typed attribute violations report "expected one of: …"
Validation SHALL, when a supplied attribute value does not match any member of
its declared enum type, report a diagnostic whose message states the supplied
value, the attribute name, the directive tag, and the literal phrase "expected
one of:" followed by the enum's declared members.

#### Scenario: Invalid enum value names the valid members
- **GIVEN** a resolved directive `task` with an attr `status` of type
  `{enum: [pending, in-progress, done]}`
- **WHEN** an atom tagged `task` sets `status: blocked`
- **THEN** validation reports a diagnostic whose message contains "expected one
  of: pending, in-progress, done"
