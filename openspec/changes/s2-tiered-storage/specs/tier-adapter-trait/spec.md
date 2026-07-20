## ADDED Requirements

### Requirement: One storage trait, three conforming adapters
`canon-store` SHALL define a single `Tier` trait with `write`, `read`, and
`age` operations, implemented by `GitTier`, `PgTier`, and `R2Tier`, such
that any caller (S3 ingest, S5 gate, S6 learn) writes and reads through the
trait without branching on which physical tier handles a given record
kind.

#### Scenario: A record write routes through the trait, not a tier-specific call
- **WHEN** a caller writes a record of a kind whose `TierPolicy` routing
  points at the git tier
- **THEN** the caller invokes `Tier::write` against the resolved adapter
  and never calls a `GitTier`-specific method directly from outside
  `canon-store`.

#### Scenario: Every adapter round-trips a written record
- **WHEN** any of `GitTier`, `PgTier`, or `R2Tier` writes a well-formed
  record and then reads it back by its identity
- **THEN** the read result is equal to the written record (write/read
  round-trip holds independently for all three adapters).

### Requirement: Write/read/age round-trips across all three tiers
The three adapters SHALL support write, read, and age operations that
round-trip correctly in a fixture environment covering all three tiers.

#### Scenario: A record ages from hot to cold and remains readable
- **WHEN** a record written to `PgTier` passes its `TierPolicy` aging
  threshold and `Tier::age` runs
- **THEN** the record becomes readable from `R2Tier` at the same logical
  identity and is no longer present in `PgTier`.

#### Scenario: Aging is idempotent under a duplicate run
- **WHEN** `Tier::age` runs twice in succession against the same
  already-aged record
- **THEN** the second run performs no duplicate write to the cold tier
  (digest-based idempotence) and reports zero newly-aged records for that
  entry.

### Requirement: Live tiers attach to a local docker-compose stack
`PgTier` and `R2Tier` SHALL each offer an env-configured `connect_live`
constructor attaching to a fully local docker-compose stack (a `postgres`
service backing `PgTier`, an S3-compatible `minio` service backing
`R2Tier`) — no cloud credentials required for local development or CI.
`PgTier::connect_live` needs zero exported env vars (its DSN defaults to the
compose Postgres). `R2Tier::connect_live` defaults its S3
endpoint/access-key/secret-key/region to the compose MinIO, but still reads
its `bucket_env`-named variable fail-loud (returning `TierUnavailable` when
unset), preserving the bucket_env contract — so the live R2 path requires
only that one bucket env var, never any endpoint/credential/region var.

#### Scenario: A record round-trips through the live MinIO/Postgres tiers
- **WHEN** `R2Tier::connect_live`/`PgTier::connect_live` attach to a
  locally running docker-compose MinIO/Postgres stack and a record is
  written and then read back
- **THEN** the read result reflects the written record, and a second
  identical write reports a digest-dedup no-op — exactly as the offline
  local-filesystem/ephemeral-Postgres substitutes already prove for the
  same adapters.

#### Scenario: An unreachable live stack skips cleanly, unless required
- **WHEN** a `live-pg`/`live-r2`-gated integration test probes its
  configured endpoint/DSN and finds it unreachable
- **THEN** the test reports a clean skip and passes, UNLESS
  `CANON_REQUIRE_LIVE=1` is set (the CI live-tier job's own env), in
  which case the same unreachable probe is a hard test failure instead —
  so a CI regression that silently loses connectivity to the compose
  stack cannot go green.
