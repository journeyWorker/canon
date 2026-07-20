## ADDED Requirements

### Requirement: Scenario/Review/Divergence layout uses the composite project-prefixed key
`GitTier` SHALL validate, on both write and read, that
`Scenario`/`Review`/`Divergence` file paths match their kind's
partition template using the COMPOSITE `<project_id>__…` natural key
(`partition::resolve_partition`'s project-prefixed keys) — a record at
a path derived from the OLD, unprefixed natural key SHALL be rejected
as a layout violation, exactly as any other path-shape mismatch is.

#### Scenario: A Scenario record written under the composite key round-trips through read
- **WHEN** a `Scenario` record with `project_id`/`scenario_id` is
  written via `GitTier::write`
- **THEN** its resolved path uses
  `<project_id>__<scenario_id>__<digest12>` as the filename stem, and a
  subsequent `GitTier::read` scan accepts that exact path as conforming

#### Scenario: A path built from the pre-s15, unprefixed natural key is rejected as a layout violation
- **WHEN** `GitTier::read` scans a `Scenario`/`Review`/`Divergence`
  file whose path was derived from the OLD natural key, with no
  `<project_id>__` prefix
- **THEN** the scan reports a layout violation for that file and
  excludes it from the returned record set, mirroring the existing
  "misfiled = malformed = violation" contract

### Requirement: An unrecognized kind=<x>/ directory is skipped and reported as foreign-namespace
A corpus-wide scan SHALL skip a `kind=<x>/` directory's contents and
report it as a foreign-namespace notice whenever `<x>` does not match
any of `RecordKind`'s twelve closed core kinds — it SHALL NEVER be
classified as a malformed core-evidence violation. This is the
forward-compatibility seam that lets a future s16 plugin kind coexist
without breaking an s15 consumer.

#### Scenario: An unknown kind directory is skipped, not treated as malformed
- **WHEN** a corpus scan walks a root containing
  `kind=plugin-widget/some-record.json` alongside the twelve
  recognized `kind=<x>/` directories
- **THEN** the scan skips `kind=plugin-widget/`'s contents entirely,
  reports it as a foreign-namespace notice, and reports ZERO
  malformed-evidence violations for anything under it

#### Scenario: The twelve core kinds are scanned exactly as before
- **WHEN** the same scan walks a root containing only recognized
  `kind=<x>/` directories — one of the twelve `RecordKind::ALL` values
- **THEN** every file under those directories is validated exactly as
  it was before the foreign-namespace rule existed — the new rule
  changes behavior ONLY for unrecognized kind directories

### Requirement: migrate_write, the sanctioned-overwrite exception, is removed
`GitTier` SHALL NOT expose a `migrate_write` (or any other
sanctioned-overwrite) method; the append-only `write` path SHALL be the
ONLY way to place a record in the git tier — a write to an existing
path SHALL always be rejected, with no exception route, sanctioned or
otherwise.

#### Scenario: No sanctioned-overwrite path exists on GitTier
- **WHEN** `GitTier`'s public API surface is inspected
- **THEN** it exposes no `migrate_write` method and no other method
  that overwrites an existing on-disk record — `write`/`read`/`age`
  are the only mutation-adjacent operations

#### Scenario: A write to an existing path is unconditionally rejected
- **WHEN** `GitTier::write` targets a path that already holds a record
- **THEN** the write is rejected — there is no "sole exception" caller
  (a `canon migrate` command or otherwise) that can force the overwrite
