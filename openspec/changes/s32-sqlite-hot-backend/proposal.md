# Why

The hot rung has exactly one backend (Postgres), so the first
`canon ingest sessions` on a fresh repo requires a running docker
compose stack — the heaviest prerequisite in the whole quick-start,
and pure friction for a single-operator repo. s28 built the seam for
this on purpose: `Rung::Hot` validates a `BackendClass` (`LiveDb`),
not a vendor name, and its design explicitly reserves "a second
live-database vendor for `hot`".

# What Changes

- `Backend::Sqlite` (class `LiveDb`, `read_directly_by_report:
  false`) — accepted anywhere `postgres` is accepted today by the s28
  class check.
- `SqliteTier` in canon-store: the `PgTier` contract (append-only
  `records_history`, digest-dedup `ON CONFLICT DO NOTHING`, s31
  chunked `write_batch`, `TierQuery` reads) over sqlx's sqlite
  driver; WAL journal mode + busy timeout at connect. No env
  indirection: a local db file carries no secret, so `canon.yaml`
  names a `path:` directly (relative paths resolve against the
  canon.yaml directory).
- `canon init` scaffolds NEW repos with `hot: { backend: sqlite,
  path: canon/hot.db }` (zero-dependency quick start) and gitignores
  the db file; the Postgres stanza stays in the template as the
  documented team-scale swap (same class, one-line change). Existing
  repos are untouched.
- Docs: tiered-storage skill + session-ingest skill + website
  tiered-storage page note the sqlite default, the pg upgrade path,
  and the single-writer caveat (WAL covers concurrent batch ingest;
  heavy multi-agent concurrency is what the pg swap is for).

# Impact

- Affected specs: `sqlite-hot-backend` (new capability).
- Affected code: `crates/canon-store` (policy + new tier),
  `crates/canon-cli` (tiers builder, init template, check-config),
  skills (`tiered-storage`, `canon-session-ingest`), website
  (`concepts/tiered-storage.mdx` EN/KR).
- Not affected: s28 class-validation model (this is its intended
  use), s29 S3/release strictness, report boundary (sqlite joins the
  "not read directly by report" set exactly like postgres).
