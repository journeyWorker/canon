## MODIFIED Requirements

### Requirement: An unrecognized kind=<x>/ directory is skipped and reported as foreign-namespace
A corpus-wide scan SHALL skip a `kind=<x>/` directory's contents and
report it as a foreign-namespace notice whenever `<x>` does not match
any of `RecordKind`'s twelve closed core kinds — it SHALL NEVER be
classified as a malformed core-evidence violation. This is the
forward-compatibility seam s16's plugin-overlay write/read primitives
(`write_namespaced`/`scan_namespaced_kind`) are the FIRST real
consumer of — a `kind=<namespace>.<kind>/` overlay directory a
ledger-overlay plugin owns is exactly one such foreign namespace, and
canon's own corpus scan treats it identically to any other
unrecognized kind.

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
  it was before the foreign-namespace rule existed — the rule changes
  behavior ONLY for unrecognized kind directories

#### Scenario: A porting overlay directory is a real, working instance of the foreign-namespace seam
- **WHEN** a root's git tier carries both core `kind=scenario/`
  records AND `kind=porting.coverage/` overlay records (written via
  `write_namespaced`)
- **THEN** a core `scan_corpus` (s15's own, unmodified) reports
  `kind=porting.coverage/` as a foreign-namespace notice and skips its
  contents, while `scan_namespaced_kind("porting.coverage")` (s16's
  plugin-aware read) reads that SAME directory's contents normally —
  proving the seam serves both a core reader (which must ignore it)
  and a plugin reader (which must read it) without any core code
  change

#### Scenario: A plugin overlay's foreign-namespace directory never collides with a core RecordKind directory
- **WHEN** `write_namespaced` is asked to write under a
  namespaced-kind string equal to a core `RecordKind::as_str()` value
- **THEN** the write is rejected (per the `plugin-overlay-records`
  capability) — the foreign-namespace seam only ever coexists with
  core directories, it never aliases one
