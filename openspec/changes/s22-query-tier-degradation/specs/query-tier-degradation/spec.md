## ADDED Requirements

### Requirement: canon query attaches only the tier(s) the requested --kind actually needs
`canon query` (both `run` and `run_with_plugin`) SHALL attach `tiers.pg`/
`tiers.r2` only when the requested `--kind`'s own routing (`TierPolicy
.routing`) or its aging destination (`TierPolicy.aging[kind].to`) names
that tier — never every tier `canon.yaml` happens to declare. `tiers.git`,
when configured, SHALL always be attached unconditionally, independent of
the requested `--kind`'s routing (it is a local directory handle with no
connection to fail).

#### Scenario: A git-routed kind's query never attempts to reach pg or r2
- **WHEN** `canon query --kind change` runs against a `canon.yaml`
  declaring `tiers.git`+`tiers.pg`+`tiers.r2`, where `change` routes to
  `git` and neither `pg`'s `dsn_env` nor `r2`'s `bucket_env` is set
- **THEN** the command succeeds and returns the `change` records — no
  attempt to read `pg`'s DSN env var or connect to `r2` is made at all

#### Scenario: A pg-routed, r2-aged kind's query attempts both tiers
- **WHEN** `canon query --kind handoff` runs against a `canon.yaml`
  where `handoff` routes to `pg` and ages to `r2` after 30 days
- **THEN** the command attempts to attach BOTH `pg` and `r2` — never
  `pg` alone — because a `handoff` record may currently live in either
  tier

#### Scenario: --plugin's git-tree resolution is unaffected by pg/r2 scoping
- **WHEN** `canon query --kind task --plugin <id>` runs against a
  `canon.yaml` where `task` routes to `pg` (so `git` is not among the
  tiers `task`'s read fan-out needs) and `tiers.pg`'s DSN is live
- **THEN** the plugin manifest/overlay resolution still succeeds — `git`
  was attached unconditionally, never scoped out by the queried kind's
  own routing

### Requirement: An unreachable tier that the queried kind does not need never fails the query
`canon query` SHALL degrade a tier it attempts (per the scoping
requirement above) to unattached (`None`) when that tier is unreachable
ONLY because it lacks live credentials or a live connection
(`StoreError::TierUnavailable`) — rather than failing the whole command,
exactly as `canon ingest plans` already degrades a per-tier
`TierUnavailable`. A malformed tier CONFIGURATION (e.g. a
`tiers.pg.schema` failing identifier validation, or any `StoreError`
other than `TierUnavailable`) SHALL still fail the whole command loud —
"lenient" describes reachability, never configuration correctness.

#### Scenario: A multi-tier canon.yaml no longer categorically blocks every --kind
- **WHEN** `canon query` runs, with any `--kind`, against a `canon.yaml`
  declaring `tiers.pg` whose `dsn_env` is unset and `tiers.r2` whose
  `bucket_env` is unset
- **THEN** a `--kind` routed to `git` succeeds; only a `--kind` routed
  (or aged) to `pg`/`r2` is affected by that tier's unavailability — the
  presence of an unreachable `pg`/`r2` block in `canon.yaml` never, by
  itself, breaks a query for a kind that does not need it

#### Scenario: A malformed pg schema still fails loud regardless of DSN presence
- **WHEN** `canon query --kind task` runs against a `canon.yaml` whose
  `tiers.pg.schema` fails `validate_schema_ident` (e.g. contains an
  uppercase letter), whether or not `CANON_PG_DSN` is set
- **THEN** the command fails with the schema-validation error — this is
  never masked by, or confused with, an unset-DSN degrade

### Requirement: A query whose own routed tier is unavailable fails, named — never silently and never generically
`canon query` SHALL fail with an error naming which `TierKind` was
needed and why (the existing `StoreError::TierUnavailable { tier,
reason }`, reached through `TierRegistry`'s existing per-kind
resolution) whenever the tier a queried `--kind` is ACTUALLY routed (or
aged) to is unattached (degraded to `None` per the requirement above) —
never a silent empty result, and never an error that omits which tier or
which kind was responsible.

#### Scenario: A pg-routed kind's query fails naming the tier and reason
- **WHEN** `canon query --kind task` runs against a `canon.yaml` where
  `task` routes to `pg` and `CANON_PG_DSN` is unset
- **THEN** the command fails with an error whose text names `pg` (the
  tier) and the "no live DSN" reason — the same
  `StoreError::TierUnavailable` text `TierRegistry::handle` already
  produces for every other caller, reached rather than pre-empted

#### Scenario: An r2-routed kind's query fails naming r2, distinctly from a pg failure
- **WHEN** `canon query --kind trajectory` runs against a `canon.yaml`
  where `trajectory` routes to `r2` and `CANON_R2_BUCKET` is unset
- **THEN** the command fails naming `r2` and its own "no live bucket"
  reason — distinguishable in the error text from the `pg`-tier failure
  scenario above, never a single generic "a tier is unavailable" message

#### Scenario: The command's exit code is non-zero for an own-tier-unavailable failure
- **WHEN** any query fails per the two scenarios above
- **THEN** `canon query` exits non-zero — the failure is process-visible,
  not merely a warning printed alongside a fabricated empty success
