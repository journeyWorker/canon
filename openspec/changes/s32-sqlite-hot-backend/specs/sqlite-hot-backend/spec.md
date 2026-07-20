## ADDED Requirements

### Requirement: Sqlite is a hot-class backend
`canon.yaml` SHALL accept `backend: sqlite` wherever the s28 class
check accepts a `LiveDb` backend, configured by a `path:` field
(relative paths resolved against the canon.yaml directory) with no
environment-variable indirection; a `tiers.<rung>` sqlite entry
missing `path:` SHALL fail loud at parse with a hint naming the
field.

#### Scenario: Hot rung on sqlite passes the class check
- **GIVEN** `tiers.hot: { backend: sqlite, path: canon/hot.db }`
- **WHEN** the policy parses
- **THEN** parsing succeeds and the hot rung resolves to a sqlite
  tier at that path

#### Scenario: Sqlite on a file-class rung still fails
- **GIVEN** `tiers.local: { backend: sqlite, path: x.db }`
- **WHEN** the policy parses
- **THEN** parsing fails with the s28 class-mismatch error naming
  the expected local-file class

### Requirement: SqliteTier honors the store contract
`SqliteTier` SHALL implement the append-only `records_history`
contract — a byte-identical resubmission is a deduped no-op, batch
and looped writes are equivalent, reads serve `TierQuery` — with WAL
journal mode and a busy timeout applied at connect.

#### Scenario: Re-ingest is a no-op
- **GIVEN** a corpus persisted to a sqlite hot tier
- **WHEN** the same corpus is force-re-ingested
- **THEN** the row count is unchanged

#### Scenario: Ingest works with zero services
- **GIVEN** a repo whose hot rung is sqlite and NO `CANON_*` env vars
  and no running database
- **WHEN** `canon ingest sessions` runs
- **THEN** session/run/event records persist and `canon query
  --kind session` returns them

### Requirement: New repos scaffold sqlite by default
`canon init` SHALL write a canon.yaml whose hot rung is
`backend: sqlite` with `path: canon/hot.db`, SHALL gitignore the db
file (and its WAL/SHM siblings), and SHALL keep the Postgres stanza
present as a commented same-class swap; `canon init --check-config`
SHALL validate a sqlite config exactly as it validates postgres.

#### Scenario: Fresh init ingests without docker
- **GIVEN** a fresh `canon init` repo
- **WHEN** `canon ingest sessions` runs with no services and no env
- **THEN** the pass persists records into `canon/hot.db`

#### Scenario: Existing configs unaffected
- **GIVEN** a canon.yaml whose hot rung is postgres
- **WHEN** any canon command parses it
- **THEN** behavior is byte-identical to pre-s32
