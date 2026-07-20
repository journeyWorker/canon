## ADDED Requirements

### Requirement: Dashboard loads a Parquet snapshot into in-memory DuckDB-Wasm
The dashboard SHALL load a `canon report --snapshot` output directory by reading
its `manifest.json`, then for each listed `{table, file}` entry registering the
Parquet file and creating a table of the same name in an in-memory DuckDB-Wasm
database. The dashboard SHALL NOT attempt to enumerate the snapshot directory.

#### Scenario: Snapshot loads into an in-memory database
- **GIVEN** a fixture snapshot directory with a valid `manifest.json` and matching
  Parquet files
- **WHEN** the dashboard app starts against that directory
- **THEN** every table listed in `manifest.json` is queryable in the in-memory
  DuckDB-Wasm database

### Requirement: DuckDB-Wasm assets are self-hosted, not fetched from a runtime CDN
The dashboard SHALL bundle its DuckDB-Wasm and Apache Arrow assets at build time and
serve them from the app's own origin. The dashboard SHALL NOT fetch these assets
from a third-party CDN at runtime.

#### Scenario: Dashboard loads with no third-party network requests
- **WHEN** the dashboard app is loaded in a browser with all third-party hosts
  blocked
- **THEN** the app still initializes DuckDB-Wasm successfully using only
  same-origin assets

### Requirement: Freshness banner surfaces snapshot provenance
The dashboard SHALL render a freshness banner showing the loaded snapshot's
`source_git_sha` and `generated_at` from `manifest.json`.

#### Scenario: Banner reflects the loaded snapshot's provenance
- **WHEN** the dashboard loads a snapshot whose `manifest.json` has
  `source_git_sha: "abc1234"` and a `generated_at` value
- **THEN** the rendered freshness banner displays `abc1234` and that
  `generated_at` value

### Requirement: Dashboard never re-derives a mart
Every dashboard panel query SHALL be a SELECT/filter/aggregate over tables already
present in the loaded snapshot. The dashboard SHALL NOT implement mart-derivation
logic (joins across raw source tables that `canon report` itself performs) in
application code.

#### Scenario: A mart-logic change requires no dashboard code change
- **GIVEN** a `canon report`-side mart definition changes and a new snapshot is
  exported
- **WHEN** the dashboard loads the new snapshot
- **THEN** the updated panel numbers render correctly with no dashboard code change

### Requirement: Five panels render from one fixture snapshot
The dashboard SHALL render five panels — change/task trust matrix, session costs,
role memory, flywheel health funnel, and review-feedback burn-down — each
populated entirely from the loaded snapshot.

#### Scenario: All panels render from a fixture snapshot
- **GIVEN** a fixture snapshot covering all five panels' backing tables
- **WHEN** the dashboard loads that snapshot
- **THEN** the trust matrix, session costs, role memory, flywheel funnel, and
  review burn-down panels each render without error

### Requirement: The donor monorepo's OpenSpec rollup endpoint is an optional trust-matrix data source
`canon report`'s ingest phase SHALL, when a repo's `.canon/canon.yaml` sets
`dashboardRollupUrl`, call `GET <dashboardRollupUrl>/changes` with the required
`X-User-Id` header and fold the returned `RolledChange[]` rows
(`worktree`, `branch`, `slug`, `route`, `created`, `proposalTitle`) into the
trust-matrix panel's backing mart, tagged by source. A failure calling this
endpoint SHALL NOT block report generation or dashboard rendering.

#### Scenario: Rollup rows appear in the trust matrix when configured
- **GIVEN** `canon.yaml` sets `dashboardRollupUrl` to a reachable endpoint
  returning a valid `RolledChange[]` payload
- **WHEN** `canon report` runs
- **THEN** the trust-matrix panel's data includes rows sourced from that endpoint

#### Scenario: Endpoint failure degrades gracefully
- **GIVEN** `canon.yaml` sets `dashboardRollupUrl` to an endpoint that returns 401
  or is unreachable
- **WHEN** `canon report` runs
- **THEN** report generation completes successfully
- **AND** the trust-matrix panel renders using only canon's own git-tier change
  scan, with no rollup-sourced rows
