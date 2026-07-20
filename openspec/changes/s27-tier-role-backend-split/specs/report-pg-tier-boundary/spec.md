## MODIFIED Requirements

### Requirement: `canon report` renders a config-derived boundary note for kinds routed to a non-offline-file-readable backend
`canon report` SHALL compute, from `canon.yaml`'s STATIC `TierPolicy
.routing` table alone (NEVER a live database connection or row
count), the set of `RecordKind`s whose ROUTED RUNG's configured
`Backend` is NOT `offline_file_readable()`. When that set is
non-empty, `canon report` SHALL render a `## Tiers not reflected`
section in the generated report, placed after `## Inputs (digest)`
and before `## Trust matrix`, naming every such kind and pointing at
`canon query --kind <kind>` as the read path for that data. When the
set is empty (no `canon.yaml`, or a `canon.yaml` whose every routed
rung resolves to an offline-file-readable backend), `canon report`
SHALL render NO such section at all. This supersedes this
capability's original derivation, which filtered on `TierKind::Pg`
identity directly — the derivation now keys on BACKEND CAPABILITY
(`Backend::offline_file_readable()`), which for today's default
rung↔backend pairing (`local`→git, `hot`→postgres, `cold`→s3)
selects the IDENTICAL kind set the original `TierKind::Pg` filter did,
but stays correct under any future non-default pairing (e.g. a `cold`
rung ever backed by `postgres` would be excluded here, where an
identity-keyed filter would wrongly include it).

#### Scenario: A multi-rung repo renders the boundary note naming the non-offline-readable kinds
- **WHEN** `canon report` runs against a repo whose `canon.yaml`
  routes at least one `RecordKind` (e.g. `task`) to a rung whose
  configured backend is `postgres` (not offline-file-readable)
- **THEN** the rendered report contains a `## Tiers not reflected`
  section naming `task` (and every other kind routed to a
  non-offline-readable-backend rung), and pointing at `canon query
  --kind <kind>` as the way to read that data
- **AND** the section does NOT name any kind routed to a rung whose
  backend is `git` or `s3`

#### Scenario: An all-offline-readable-backend repo renders no boundary section
- **WHEN** `canon report` runs against a repo with no `canon.yaml`
  present, or a `canon.yaml` whose every routed rung resolves to a
  `git`- or `s3`-backed rung only
- **THEN** the rendered report contains no `## Tiers not reflected`
  section at all — not an empty section, not a placeholder, nothing

#### Scenario: A cold rung backed by postgres is excluded, proving the derivation keys on backend capability, not rung identity
- **WHEN** `canon report` runs against a repo whose `canon.yaml`
  routes a kind (e.g. `trajectory`) to the `cold` rung, and
  `tiers.cold.backend` is configured as `postgres` (a non-default,
  but valid, pairing)
- **THEN** the rendered report's `## Tiers not reflected` section
  names `trajectory` — demonstrating the derivation excludes it
  because its backend (`postgres`) is not offline-file-readable,
  REGARDLESS of `trajectory` being routed to the `cold` rung, which a
  rung-identity-keyed derivation would have wrongly treated as always
  report-visible

#### Scenario: A malformed canon.yaml degrades to the same no-note behavior as no canon.yaml
- **WHEN** `canon report` runs against a repo whose `canon.yaml`
  exists but fails to parse as a valid `TierPolicy` (e.g. an unknown
  `routing` key, or a legacy backend name used as a rung value)
- **THEN** `canon report` still succeeds (does not fail or panic on
  this account) and renders no `## Tiers not reflected` section,
  exactly as it would for a repo with no `canon.yaml` at all

### Requirement: The boundary note is deterministic and drift-checkable, matching every existing panel's discipline
The `## Tiers not reflected` section's content SHALL be timestamp-free
and SHALL list kinds in a fixed, sorted order (ascending by the
kind's wire string) rather than any unordered iteration. Rendering
`canon report` twice against an unchanged `canon.yaml` SHALL produce a
byte-identical `## Tiers not reflected` section (or its byte-identical
absence), regardless of whether a live connection to any
non-offline-readable backend is reachable at either run — `canon
report --check`'s existing byte-diff drift gate SHALL cover this
section with no separate mechanism.

#### Scenario: Two renders of an unchanged multi-rung config produce a byte-identical boundary section
- **WHEN** `canon report` runs twice in succession against a repo
  whose `canon.yaml` is unchanged between runs and routes at least one
  kind to a non-offline-readable-backend rung
- **THEN** the `## Tiers not reflected` section of both renders is
  byte-for-byte identical, and neither render's section contains a
  wall-clock-derived value or a live row count
- **AND** this holds identically whether or not a live database
  connection is reachable for either run

#### Scenario: `canon report --check` reports no drift solely from the boundary note's presence
- **GIVEN** a committed `canon/REPORT.md` was generated against the
  current, unchanged `canon.yaml`
- **WHEN** `canon report --check` is run afterward with no change to
  `canon.yaml`, the corpus, or the ledger
- **THEN** the check reports no drift — the boundary note's presence
  or absence never causes spurious drift on an otherwise-unchanged
  input

### Requirement: `canon report` emits a stderr warning naming the same non-offline-readable-backed kinds as the rendered note
`canon report` (the CLI) SHALL print one stderr line, prefixed `canon
report: WARN `, naming the identical (config-derived, sorted) set of
`RecordKind`s the rendered `## Tiers not reflected` section names,
computed via the same backend-capability derivation so the two can
never disagree. This SHALL happen for every `canon report` invocation
mode (the flagless write, `--check`, and `--snapshot <dir>`). A repo
with every routed rung backed by an offline-file-readable backend
SHALL produce no such stderr line.

#### Scenario: A multi-rung repo's canon report run emits a stderr WARN matching the report's own note
- **WHEN** `canon report --repo <dir>` runs against a repo whose
  `canon.yaml` routes `task`, `session`, and `event` to rungs backed
  by `postgres`
- **THEN** the command's stderr contains a line starting `canon
  report: WARN` that names `task`, `session`, and `event`
- **AND** the written `canon/REPORT.md`'s `## Tiers not reflected`
  section names the exact same three kinds

#### Scenario: An all-offline-readable-backend repo's canon report run emits no WARN line
- **WHEN** `canon report --repo <dir>` runs against a repo with no
  `canon.yaml`, or one whose every routed rung resolves to a `git`- or
  `s3`-backed rung
- **THEN** the command's stderr contains no `WARN` line naming any
  record kind

### Requirement: The boundary note and warning add no live-database dependency and change no gate/query behavior
`canon report` SHALL NOT gain, via the `## Tiers not reflected` section
or its stderr warning, a live database connection, a new `stg_*` view
over a non-offline-readable backend, or any other live-connection read
path into `canon-report` or `canon-store`'s DuckDB view layer. `canon
query --kind <kind>` for a kind routed to any rung SHALL be unaffected
by this change. `canon gate check` SHALL continue to read nothing
produced by `canon-report`, and its verdicts SHALL be byte-identical
for any corpus, before and after this change.

#### Scenario: canon query for a non-offline-readable-backed kind is unaffected
- **WHEN** `canon query --kind task` runs against a repo whose
  `canon.yaml` routes `task` to a `postgres`-backed rung, both before
  and after this change lands
- **THEN** the command's behavior (rows returned, exit code) is
  identical in both cases — this change touches no `canon query` code
  path

#### Scenario: Gate verdicts are unaffected by the new boundary note's existence
- **WHEN** `canon gate check` runs against a corpus both before and
  after `canon report`'s `## Tiers not reflected` section is
  rekeyed to backend capability
- **THEN** `canon gate check`'s verdicts are byte-identical in both
  cases — no `canon-gate` source file reads `canon-report`'s output
  or the boundary note
