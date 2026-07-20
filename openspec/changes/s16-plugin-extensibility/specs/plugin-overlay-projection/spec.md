## ADDED Requirements

### Requirement: A plugin-aware read projects declared overlay fields onto the core view without mutating core records
A plugin-aware read SHALL resolve the plugin snapshot, read the core
index records and the relevant overlay records, and project the
overlay's declared field(s) onto each matching core record's in-memory
representation via its join key — this projection SHALL NEVER write
to, or otherwise mutate, any on-disk core record. The projection SHALL
be deterministic: the SAME core+overlay record set projects to the
SAME output on every run. In s16 this projection concretely targets
`core_kind: scenario` only, matching `plugin-overlay-registry`'s
`core_kind: scenario` restriction — a generic projection capable of
targeting ANY `RecordKind` is explicit FUTURE work, a non-goal of s16.

#### Scenario: A core record with a matching overlay record projects the declared fields
- **WHEN** `canon query --kind scenario --plugin porting` runs against
  a project where `porting`'s `coverage` overlay carries `covered:
  true, surface_ref: [...]` for `(project_id, scenario_id)` matching an
  existing `Scenario` record
- **THEN** that record's projected output carries `covered: true` and
  the `surface_ref` list, alongside its own native fields, unchanged

#### Scenario: Projecting never rewrites the core record on disk
- **WHEN** the same `canon query --kind scenario --plugin porting` run
  above completes
- **THEN** the underlying `Scenario` record's on-disk file (bytes,
  digest, path) is byte-identical to what it was before the query ran

#### Scenario: The same core+overlay pair projects identically across repeated runs
- **WHEN** `canon query --kind scenario --plugin porting` runs twice
  in a row with no intervening writes
- **THEN** both runs produce byte-identical projected output

### Requirement: Projection is fail-soft when a plugin or an overlay record is absent
A plugin-aware read SHALL degrade to the unmodified core view — never
a panic, never a hard error — whenever: the named plugin has no
installed manifest; the plugin's manifest resolves but declares no
overlay for the queried core kind; or a specific core record has no
matching overlay record for its join key. A malformed overlay record
present on disk (fails the CURRENT manifest's field schema) SHALL be
skipped with a diagnostic, without aborting projection for any other
record.

#### Scenario: An unnamed/uninstalled plugin degrades to the unmodified core view
- **WHEN** `canon query --kind scenario --plugin does-not-exist` runs
- **THEN** the queried `Scenario` records are returned exactly as
  `canon query --kind scenario` (no `--plugin`) would return them,
  plus a diagnostic naming the unresolved plugin — never a process
  error or non-zero exit

#### Scenario: A core record with no overlay record projects unmodified
- **WHEN** a `Scenario` record's `(project_id, scenario_id)` has no
  corresponding `porting.coverage` overlay record on disk
- **THEN** that record's projected output carries none of `porting`'s
  declared fields — its native fields are unchanged, and no default or
  guessed value is invented

#### Scenario: A malformed overlay record is skipped, not fatal to the whole projection
- **WHEN** one `porting.coverage` overlay record on disk no longer
  matches the CURRENT manifest's field schema (e.g. `covered` is a
  string, not a bool) while every other overlay record is well-formed
- **THEN** projection skips ONLY that malformed record (with a
  diagnostic) — every other core record's projection, including ones
  with well-formed overlays, completes normally

### Requirement: Core neither hardcodes nor precludes overlay-projected fields
`canon-model`'s core record types SHALL declare no field reserved for,
or excluded because of, any specific overlay's projected data — the
projected view is assembled ENTIRELY by the plugin-aware read path,
outside `canon-model`, so an overlay's field set can grow or a new
overlay kind can attach to a core kind without any `canon-model`
change.

#### Scenario: A second overlay plugin attaching to the same core kind requires no core change
- **WHEN** a second, hypothetical plugin declares an overlay ALSO
  attached to `core_kind: scenario` on the same join key, with a
  DIFFERENT field (e.g. `priority: number`)
- **THEN** both plugins' overlays project independently onto the same
  `Scenario` record via `--plugin <id>` — `canon_model::Scenario`'s own
  struct definition requires no change to support either
