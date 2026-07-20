## ADDED Requirements

### Requirement: A release build never attaches S3 with defaulted credentials
A release build (`cfg!(not(debug_assertions))`) SHALL require
`CANON_S3_ENDPOINT`,
`CANON_S3_ACCESS_KEY`, and `CANON_S3_SECRET_KEY` to be set; a missing
one SHALL fail attachment with a `StoreError::BackendUnattached`
whose reason names EVERY unset variable. Debug builds SHALL keep the
zero-env docker-compose MinIO defaults. Plain-HTTP transport SHALL be
permitted only when the resolved endpoint itself starts with
`http://`.

#### Scenario: Release build with bucket but no credentials fails loud
- **WHEN** a strict-mode S3 attach resolves a bucket name but
  `CANON_S3_ENDPOINT`, `CANON_S3_ACCESS_KEY`, and
  `CANON_S3_SECRET_KEY` are all unset
- **THEN** attachment fails with `BackendUnattached` naming all three
  unset variables — it SHALL NOT build a client pointed at
  `http://127.0.0.1:59000` with `canon`/`canoncanon`

#### Scenario: Debug build keeps the zero-env local dev path
- **WHEN** a non-strict (debug-profile) S3 attach resolves a bucket
  name and no `CANON_S3_*` variable is set
- **THEN** attachment succeeds against the compose MinIO defaults,
  byte-identical to pre-s29 behavior

### Requirement: Aging rules must move strictly forward
`TierPolicy::from_yaml` SHALL reject any `aging.<kind>` entry whose
kind has no `routing.<kind>` entry, or whose `to` rung is not
strictly colder than the kind's routed rung under the total order
`local < hot < cold`. The `PolicyError` SHALL name the kind, the
routed rung, the target rung, and the ordering rule.

#### Scenario: Same-rung aging is rejected instead of deleting records
- **WHEN** `TierPolicy::from_yaml` parses `routing.task: hot` with
  `aging.task: { after: 30d, to: hot }`
- **THEN** parsing fails with a `PolicyError` naming `task`, `hot`,
  and the forward-only rule — the registry SHALL never receive the
  same tier as aging source and destination

#### Scenario: Backward aging is rejected instead of silently dead
- **WHEN** `TierPolicy::from_yaml` parses `routing.scenario: cold`
  with `aging.scenario: { after: 30d, to: hot }`
- **THEN** parsing fails with the same `PolicyError` class

### Requirement: Aging durations parse totally
Aging-duration parsing SHALL reject negative magnitudes and SHALL map
out-of-range magnitudes to a `PolicyError` naming the offending
literal, never panicking.

#### Scenario: Negative and overflow durations are policy errors
- **WHEN** `TierPolicy::from_yaml` parses `after: -1d`, or `after:
  9223372036854775807d`
- **THEN** each fails with a `PolicyError` naming the literal — no
  future-dated cutoff, no panic

### Requirement: R2 reads degrade malformed rows to violations
`R2Tier::read` SHALL validate each decoded row's envelope (kind, id,
digest, RFC3339 `at`) before constructing records; a malformed row or
object SHALL append an `EvidenceViolation` naming the object path and
SHALL NOT panic or abort the read of remaining objects.

#### Scenario: A tampered parquet body yields a violation, not a panic
- **WHEN** an s3-backed rung holds an object whose parquet `body`
  decodes to `{}`
- **THEN** `read` returns the remaining valid records plus one
  violation naming that object — the process SHALL NOT panic

### Requirement: S3 existence checks propagate non-NotFound errors
The R2 write path's existence probe SHALL treat only
`object_store::Error::NotFound` as absence; any other HEAD failure
SHALL propagate as `StoreError::ObjectStore`.

#### Scenario: A denied HEAD does not masquerade as a fresh write
- **WHEN** the object store answers a pre-write HEAD with an
  authorization error
- **THEN** the write fails with `StoreError::ObjectStore` — it SHALL
  NOT re-PUT the object and report `deduped: false`

### Requirement: Ingest builds only the rungs its kinds route to, and reports degrade reasons
`canon ingest sessions` and `canon ingest artifacts` SHALL construct
tiers for exactly the union of rungs their record kinds route (or
age) to; a malformed `canon.yaml` SHALL fail the command loud; an
unavailable needed rung SHALL degrade records to unwritten AND the
printed outcome SHALL carry the build-time reason including the
configured env-var name.

#### Scenario: An unrelated unset cold bucket no longer blocks session persistence
- **WHEN** `canon ingest sessions` runs with `session`/`run`/`event`
  routed to a reachable rung while an unrelated kind's `cold` rung
  credentials are unset
- **THEN** sessions persist normally — the unrelated rung is never
  attempted

#### Scenario: A degraded ingest names the variable an operator must set
- **WHEN** `canon ingest sessions` degrades because the routed hot
  rung's `dsn_env`-named variable is unset
- **THEN** the printed outcome names that variable (e.g.
  `CANON_PG_DSN`) — never a bare "tiers unreachable" guess

### Requirement: A Postgres connection outage classifies as unavailability
`PgTier::connect`'s initial pool-connection failure SHALL map to
`StoreError::TierUnavailable` (backend `postgres`, reason carrying
the driver display); post-connection DDL/query failures SHALL keep
mapping to `StoreError::Sql`. Configuration validation SHALL precede
credential resolution in BOTH strict and lenient builders, and `canon
init --check-config` SHALL reject a `tiers.pg.schema` failing
`validate_schema_ident`.

#### Scenario: An unreachable Postgres degrades lenient paths instead of hard-failing
- **WHEN** a lenient tier build resolves a DSN whose host accepts no
  connections
- **THEN** the pg rung degrades to unavailable (query-time error
  stays the named, non-silent kind) — the build SHALL NOT abort

#### Scenario: check-config catches a malformed schema
- **WHEN** `canon init --check-config` reads a `canon.yaml` whose
  `tiers.hot.schema` is `Bad-Schema`
- **THEN** the command fails naming the schema — it SHALL NOT print
  `[PASS] tiers/routing/aging`
