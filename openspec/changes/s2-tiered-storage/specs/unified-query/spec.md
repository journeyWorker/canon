## ADDED Requirements

### Requirement: `canon query` fans out across tiers and merges by identity
`canon query` SHALL resolve a record kind's tier(s) from `TierPolicy`,
issue a native read against each resolved tier, and merge the results
ordered by the record envelope's `at` field into one output stream.

#### Scenario: A kind split across hot and cold tiers merges correctly
- **WHEN** `canon query --kind handoff` runs against a fixture where some
  `handoff` records are still in the pg tier and others have already aged
  to the r2 tier
- **THEN** the output contains every record from both tiers exactly once,
  ordered by `at`, with no duplicate and no gap.

#### Scenario: A tier-scoped query filters correctly
- **WHEN** `canon query --kind session --since <timestamp>` runs
- **THEN** every returned record's `at` is at or after `<timestamp>` and
  no record from a different kind is included.

### Requirement: DuckDB stg_/int_/mart_ views over the git and r2 tiers
The repo SHALL ship DuckDB views layered as `stg_*` (thin, source-shaped),
`int_*` (gate-equivalent derivations mirroring `canon-gate`), and `mart_*`
(persona-facing), reading the git tier's Hive files and the r2 tier's
parquet exports directly, never duplicating or caching tier data.

#### Scenario: Views open against a fixture corpus
- **WHEN** the DuckDB view file is loaded (`duckdb -init <views.sql>`)
  against a fixture repo containing git-tier Hive files and r2-tier
  parquet exports
- **THEN** every `stg_*`, `int_*`, and `mart_*` view opens without error
  and returns rows matching the fixture's known content.

#### Scenario: A mart never contradicts the gate's own derivation
- **WHEN** an `int_*` view's derivation logic is compared against
  `canon-gate`'s equivalent Rust logic for the same fixture input
- **THEN** both produce the same verdict for every fixture record — the
  SQL view is a mirror, never an independent re-derivation that could
  diverge.
