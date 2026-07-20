## MODIFIED Requirements

### Requirement: canon query attaches only the rung(s) the requested --kind actually needs
`canon query` (both `run` and `run_with_plugin`) SHALL attach the
`hot`/`cold` rungs' configured backends only when the requested
`--kind`'s own routing (`TierPolicy.routing`) or its aging destination
(`TierPolicy.aging[kind].to`) names that RUNG (supersedes this
capability's original `TierKind`-keyed scoping) — never every rung
`canon.yaml` happens to declare. The `local` rung's backend, when
configured, SHALL always be attached unconditionally, independent of
the requested `--kind`'s routing (it is, by convention, a local
directory handle with no connection to fail).

#### Scenario: A local-routed kind's query never attempts to reach hot or cold
- **WHEN** `canon query --kind change` runs against a `canon.yaml`
  declaring `tiers.local`+`tiers.hot`+`tiers.cold`, where `change`
  routes to `local` and neither the hot rung's `dsn_env` nor the
  cold rung's `bucket_env` is set
- **THEN** the command succeeds and returns the `change` records — no
  attempt to read the hot rung's DSN env var or connect to the cold
  rung's bucket is made at all

#### Scenario: A hot-routed, cold-aged kind's query attempts both rungs
- **WHEN** `canon query --kind handoff` runs against a `canon.yaml`
  where `handoff` routes to `hot` and ages to `cold` after 30 days
- **THEN** the command attempts to attach BOTH the hot and cold
  rungs' configured backends — never the hot rung alone — because a
  `handoff` record may currently live in either rung

#### Scenario: --plugin's git-tree resolution is unaffected by hot/cold scoping
- **WHEN** `canon query --kind task --plugin <id>` runs against a
  `canon.yaml` where `task` routes to `hot` (so the local rung is
  not among the rungs `task`'s read fan-out needs) and the hot rung's
  backend DSN is live
- **THEN** the plugin manifest/overlay resolution still succeeds — the
  local rung's backend was attached unconditionally, never scoped
  out by the queried kind's own routing

### Requirement: A query whose own routed rung is unavailable fails, naming the rung and backend — never silently and never generically
`canon query` SHALL fail with an error naming which `Rung` was needed
and, whenever the rung was configured with a known backend, which
`Backend` was behind it and why it is unattached (the `StoreError::
TierUnavailable { rung, backend, reason }` shape, reached through
`TierRegistry`'s existing per-kind resolution) whenever the rung a
queried `--kind` is ACTUALLY routed (or aged) to is unattached
(degraded to `None` per the scoping requirement above) — never a
silent empty result, and never an error that omits which rung or
which kind was responsible. This supersedes this capability's original
`StoreError::TierUnavailable { tier: TierKind, reason }` shape, which
named only a backend.

#### Scenario: A hot-routed kind's query fails naming the rung and backend
- **WHEN** `canon query --kind task` runs against a `canon.yaml`
  where `task` routes to `hot`, `tiers.hot.backend` is `postgres`, and
  `CANON_PG_DSN` is unset
- **THEN** the command fails with an error whose text names the `hot`
  rung, the `postgres` backend behind it, and the "no live DSN"
  reason — e.g. `"hot tier (postgres) is not attached (no live DSN)"`,
  reached rather than pre-empted

#### Scenario: A cold-routed kind's query fails naming the rung and backend, distinctly from a hot failure
- **WHEN** `canon query --kind trajectory` runs against a `canon.yaml`
  where `trajectory` routes to `cold` and `tiers.cold.backend`'s
  `bucket_env` is unset
- **THEN** the command fails naming the `cold` rung, its `s3` backend,
  and its own "no live bucket" reason — distinguishable in the error
  text from the hot-rung failure scenario above, never a single
  generic "a tier is unavailable" message

#### Scenario: A routed rung with no tiers.<rung> block at all fails naming the rung alone
- **WHEN** `canon query --kind task` runs against a `canon.yaml` whose
  `routing.task` is `hot`, but `canon.yaml`'s `tiers:` section declares
  no `hot` entry at all (never configured, no backend known)
- **THEN** the command fails with an error naming the `hot` rung and
  stating it is not configured — the error never fabricates or guesses
  a backend name it was never told

#### Scenario: The command's exit code is non-zero for an own-rung-unavailable failure
- **WHEN** any query fails per the scenarios above
- **THEN** `canon query` exits non-zero — the failure is process-
  visible, not merely a warning printed alongside a fabricated empty
  success

### Requirement: An unreachable rung that the queried kind does not need never fails the query
`canon query` SHALL degrade a rung it attempts (per the scoping
requirement above) to unattached (`None`) when that rung's configured
backend is unreachable ONLY because it lacks live credentials or a
live connection (`StoreError::TierUnavailable`) — rather than failing
the whole command, exactly as `canon ingest plans` already degrades a
per-backend `TierUnavailable`. A malformed BACKEND CONFIGURATION
(e.g. `tiers.hot.schema` failing identifier validation, or any
`StoreError` other than `TierUnavailable`) SHALL still fail the whole
command loud — "lenient" describes reachability, never configuration
correctness. This supersedes this capability's original scenario text,
which cited the pre-migration `tiers.pg.schema` path.

#### Scenario: A multi-rung canon.yaml no longer categorically blocks every --kind
- **WHEN** `canon query` runs, with any `--kind`, against a
  `canon.yaml` declaring a `hot` rung whose `dsn_env` is unset and a
  `cold` rung whose `bucket_env` is unset
- **THEN** a `--kind` routed to `local` succeeds; only a `--kind`
  routed (or aged) to `hot`/`cold` is affected by that rung's
  unavailability — the presence of an unreachable `hot`/`cold` block
  in `canon.yaml` never, by itself, breaks a query for a kind that
  does not need it

#### Scenario: A malformed hot-rung schema still fails loud regardless of DSN presence
- **WHEN** `canon query --kind task` runs against a `canon.yaml` whose
  `tiers.hot.schema` fails `validate_schema_ident` (e.g. contains an
  uppercase letter), whether or not `CANON_PG_DSN` is set
- **THEN** the command fails with the schema-validation error — this
  is never masked by, or confused with, an unset-DSN degrade
