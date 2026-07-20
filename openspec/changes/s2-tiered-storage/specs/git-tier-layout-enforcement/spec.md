## ADDED Requirements

### Requirement: Misfiled git-tier records are detected as layout violations
`GitTier` SHALL validate, on both write and read, that every record's file
path matches its kind's declared partition template (generalized from
`tools/parity.py::_ledger_layout_problem`); a record at the wrong path is
malformed evidence — reported as a layout violation, never silently
accepted or reparsed from an inferred "correct" location.

#### Scenario: A record filed at the wrong path is rejected on write
- **WHEN** `GitTier::write` is asked to place a record whose target path
  does not match its kind's `partition_template()`
- **THEN** the write is rejected with a `FailureClass::Malformed` `layout`
  violation naming the expected template and the rejected path.

#### Scenario: A pre-existing misfiled record is flagged on read, not skipped silently
- **WHEN** `GitTier::read` scans the git tier and encounters a file whose
  path does not match its kind's partition template (e.g. a flat file
  where a Hive-nested `area=<area>/` directory is required)
- **THEN** the scan reports a layout violation for that file and excludes
  it from the returned record set — mirroring `_ledger_layout_problem`'s
  "misfiled = malformed = violation" contract exactly, never trusting a
  misfiled record's content.

#### Scenario: Area-scoped and non-area-scoped kinds use distinct templates
- **WHEN** a kind declares an area-scoped partition template
  (`kind=<kind>/area=<area>/<id>.json`) versus a kind declaring a flat
  template (`kind=<kind>/<file>.json`)
- **THEN** `GitTier` validates each kind's files against its own declared
  template — an area-scoped kind's record is never accepted at a flat
  path and vice versa.

### Requirement: The git tier never rewrites existing records
`GitTier::write` SHALL be append-only: writing a record never overwrites
an existing file at the same path; corrections are new records, and
`canon migrate` is the only sanctioned rewriter (design §7).

#### Scenario: A write to an existing path is rejected
- **WHEN** `GitTier::write` targets a path that already holds a record
- **THEN** the write is rejected rather than silently overwriting the
  existing file.

#### Scenario: `canon migrate` is the sole exception, and leaves a quarantine report
- **WHEN** `canon migrate` rewrites a git-tier record to a corrected
  layout or schema version
- **THEN** the rewrite is explicitly attributed to the migration path (not
  `GitTier::write`'s normal append-only path) and any record it cannot
  safely upgrade is left in place with an entry in a quarantine report,
  never silently dropped or force-overwritten.
