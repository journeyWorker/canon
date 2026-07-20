## MODIFIED Requirements

### Requirement: `canon report` renders a config-derived boundary note for kinds not read directly by the report
`canon report` SHALL compute, from `canon.yaml`'s STATIC `TierPolicy
.routing` table alone (NEVER a live connection or row count), the set
of `RecordKind`s whose ROUTED RUNG's configured `Backend` is NOT
`read_directly_by_report()`. When that set is non-empty, `canon
report` SHALL render a `## Kinds not read directly` section in the
generated report, placed after `## Inputs (digest)` and before `##
Trust matrix`, naming every such kind and pointing at `canon query
--kind <kind>` as the read path for that data. When the set is empty
(no `canon.yaml`, or a `canon.yaml` whose every routed rung resolves
to a directly-read backend), `canon report` SHALL render NO such
section at all. This supersedes this capability's s27 derivation,
which keyed on `Backend::offline_file_readable()` — that predicate
wrongly returned `true` for `S3`, treating an S3-backed rung's data as
always report-visible when `canon report` never opens the live S3
bucket directly (it only ever scans a LOCAL `canon/r2` parquet
mirror, which nothing keeps automatically synced with the bucket).
`Backend::read_directly_by_report()` corrects this: `true` ONLY for
`git` (the git ledger IS one of `canon-report`'s local read roots);
`false` for both `postgres` and `s3`.

#### Scenario: A multi-rung repo renders the boundary note naming the kinds not read directly
- **WHEN** `canon report` runs against a repo whose `canon.yaml`
  routes at least one `RecordKind` (e.g. `task`) to a rung whose
  configured backend is `postgres`
- **THEN** the rendered report contains a `## Kinds not read directly`
  section naming `task` (and every other kind routed to a
  not-directly-read-backend rung), and pointing at `canon query --kind
  <kind>` as the way to read that data
- **AND** the section does NOT name any kind routed to a rung whose
  backend is `git`

#### Scenario: An all-directly-read-backend repo renders no boundary section
- **WHEN** `canon report` runs against a repo with no `canon.yaml`
  present, or a `canon.yaml` whose every routed rung resolves to a
  `git`-backed rung only
- **THEN** the rendered report contains no `## Kinds not read
  directly` section at all — not an empty section, not a placeholder,
  nothing

#### Scenario: A cold rung backed by s3 now appears in the boundary section, correcting s27's silent omission
- **WHEN** `canon report` runs against a repo whose `canon.yaml`
  routes a kind (e.g. `trajectory`) to the `cold` rung, and
  `tiers.cold.backend` is configured as `s3` (today's default,
  class-correct pairing, `rung-backend-capability` design D1)
- **THEN** the rendered report's `## Kinds not read directly` section
  names `trajectory` — because `canon report` never opens the live S3
  bucket directly, REGARDLESS of `canon.yaml`'s `CANON_R2_ROOT`
  possibly holding an unrelated, separately-materialized local mirror
- **AND** this is a CHANGE from s27's behavior, under which this exact
  configuration silently rendered no boundary entry for `trajectory`
  at all (`Backend::offline_file_readable()`'s S3 overclaim)

#### Scenario: A malformed canon.yaml degrades to the same no-note behavior as no canon.yaml
- **WHEN** `canon report` runs against a repo whose `canon.yaml`
  exists but fails to parse as a valid `TierPolicy` (e.g. an unknown
  `routing` key, a legacy backend name used as a rung value, or a
  class-mismatched `tiers.<rung>` entry)
- **THEN** `canon report` still succeeds (does not fail or panic on
  this account) and renders no `## Kinds not read directly` section,
  exactly as it would for a repo with no `canon.yaml` at all

### Requirement: The boundary note is deterministic, drift-checkable, and states its own limits truthfully
The `## Kinds not read directly` section's content SHALL be
timestamp-free and SHALL list kinds in a fixed, sorted order (ascending
by the kind's wire string) rather than any unordered iteration.
Rendering `canon report` twice against an unchanged `canon.yaml` SHALL
produce a byte-identical `## Kinds not read directly` section (or its
byte-identical absence), regardless of whether a live connection to
any not-directly-read backend is reachable at either run — `canon
report --check`'s existing byte-diff drift gate SHALL cover this
section with no separate mechanism. The section's own explanatory
sentence SHALL NOT assert that a listed kind's data is unconditionally
absent from the report — a locally-materialized mirror of that
backend's data MAY still exist — it SHALL instead state that
`canon report` reads its local roots directly, that the listed kinds
route to a backend whose own store is not one of them, and that their
data appears only if separately materialized into the local report
roots (which may be incomplete or stale).

#### Scenario: Two renders of an unchanged multi-rung config produce a byte-identical boundary section
- **WHEN** `canon report` runs twice in succession against a repo
  whose `canon.yaml` is unchanged between runs and routes at least one
  kind to a not-directly-read-backend rung
- **THEN** the `## Kinds not read directly` section of both renders is
  byte-for-byte identical, and neither render's section contains a
  wall-clock-derived value or a live row count
- **AND** this holds identically whether or not a live connection to
  any not-directly-read backend is reachable for either run

#### Scenario: canon report --check reports no drift solely from the boundary note's presence
- **GIVEN** a committed `canon/REPORT.md` was generated against the
  current, unchanged `canon.yaml`
- **WHEN** `canon report --check` is run afterward with no change to
  `canon.yaml`, the corpus, or the ledger
- **THEN** the check reports no drift — the boundary note's presence
  or absence never causes spurious drift on an otherwise-unchanged
  input

#### Scenario: The boundary sentence never claims an absolute "not reflected"
- **WHEN** an operator reads the `## Kinds not read directly`
  section's explanatory sentence for a repo whose `cold` rung is
  s3-backed
- **THEN** the sentence states that `canon report` reads its local
  roots directly and that the listed kind's backend's own store is not
  one of them, and that the data appears only if materialized into the
  local report roots — it does NOT assert the data is unconditionally
  missing from the report

### Requirement: `canon report` emits a stderr warning naming the same not-directly-read kinds as the rendered note
`canon report` (the CLI) SHALL print one stderr line, prefixed `canon
report: WARN `, naming the identical (config-derived, sorted) set of
`RecordKind`s the rendered `## Kinds not read directly` section names,
computed via the same `read_directly_by_report`-keyed derivation so
the two can never disagree. This SHALL happen for every `canon report`
invocation mode (the flagless write, `--check`, and `--snapshot
<dir>`). A repo with every routed rung backed by a directly-read
backend SHALL produce no such stderr line.

#### Scenario: A multi-rung repo's canon report run emits a stderr WARN matching the report's own note
- **WHEN** `canon report --repo <dir>` runs against a repo whose
  `canon.yaml` routes `task`, `session`, and `event` to rungs backed
  by `postgres`
- **THEN** the command's stderr contains a line starting `canon
  report: WARN` that names `task`, `session`, and `event`
- **AND** the written `canon/REPORT.md`'s `## Kinds not read directly`
  section names the exact same three kinds

#### Scenario: An all-directly-read-backend repo's canon report run emits no WARN line
- **WHEN** `canon report --repo <dir>` runs against a repo with no
  `canon.yaml`, or one whose every routed rung resolves to a
  `git`-backed rung
- **THEN** the command's stderr contains no `WARN` line naming any
  record kind

### Requirement: The boundary note and warning add no live-connection dependency and change no gate/query behavior
`canon report` SHALL NOT gain, via the `## Kinds not read directly`
section or its stderr warning, a live database connection, a live
object-store (S3) client, a new `stg_pg_records`/`stg_s3_records` view,
or any other live-connection read path into `canon-report` or
`canon-store`'s DuckDB view layer. `canon query --kind <kind>` for a
kind routed to any rung SHALL be unaffected by this change. `canon
gate check` SHALL continue to read nothing produced by `canon-report`,
and its verdicts SHALL be byte-identical for any corpus, before and
after this change.

#### Scenario: canon query for a not-directly-read-backend kind is unaffected
- **WHEN** `canon query --kind task` runs against a repo whose
  `canon.yaml` routes `task` to a `postgres`-backed rung, both before
  and after this change lands
- **THEN** the command's behavior (rows returned, exit code) is
  identical in both cases — this change touches no `canon query` code
  path

#### Scenario: Gate verdicts are unaffected by the boundary note's rekeyed predicate
- **WHEN** `canon gate check` runs against a corpus both before and
  after `canon report`'s `## Kinds not read directly` section is
  rekeyed from `offline_file_readable` to `read_directly_by_report`
- **THEN** `canon gate check`'s verdicts are byte-identical in both
  cases — no `canon-gate` source file reads `canon-report`'s output
  or the boundary note
