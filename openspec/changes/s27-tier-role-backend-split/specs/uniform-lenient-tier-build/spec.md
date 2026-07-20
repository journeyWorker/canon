## MODIFIED Requirements

### Requirement: Exactly one shared per-backend degrade-or-propagate implementation backs every lenient tier build in canon-cli
`canon-cli` SHALL implement, in exactly one place (`tiers.rs`), the
logic that decides — per BACKEND (`postgres`/`s3`, keyed by whichever
rung a `canon.yaml` assigns them to) — whether an attach failure
degrades to `None` (a bare `StoreError::TierUnavailable`) or
propagates loud (any other error); both the whole-policy builder
(`canon ingest plans`) and the rung-scoped builder (`canon query`)
SHALL call that one implementation, never carrying an independent
copy of the decision. This supersedes this capability's original
`pg`/`r2`-named framing (`attach_pg`/`attach_r2`) — the shared
functions rename to `attach_postgres`/`attach_s3` to match `Backend`'s
vocabulary; the shared decision logic itself is unchanged.

#### Scenario: ingest plans and query degrade an unset hot-rung DSN identically
- **WHEN** `canon ingest plans` and `canon query --kind <a hot-routed
  kind that also has a local/cold candidate to persist/read>` each
  run against a `canon.yaml` whose hot rung's `dsn_env` is unset
- **THEN** both commands' hot-rung attach degrades to unattached via
  the identical `StoreError::TierUnavailable`-degrades-to-`None`
  path — the same function, not two independently written equivalents

#### Scenario: A malformed hot-rung schema propagates identically from both callers
- **WHEN** `canon ingest plans` and `canon query` each run against a
  `canon.yaml` whose `tiers.hot.schema` fails `validate_schema_ident`
- **THEN** both commands fail loud with the identical schema-
  validation error text — proving the "propagate anything other than
  `TierUnavailable`" branch is the same code path for both callers

### Requirement: canon ingest plans's own observable behavior is unchanged by the rung/backend rekey
This rekey SHALL NOT change `canon ingest plans`'s existing error
text (beyond the rung/backend vocabulary substitution itself), exit
codes, or persisted/unwritten counts for any existing fixture: moving
the shared lenient-attach logic from `TierKind`-keying to `Rung`-
keying only changes WHICH VOCABULARY the logic is expressed in, never
WHAT it decides for its pre-existing caller.

#### Scenario: ingest plans's existing degradation fixtures pass with rung-vocabulary fixtures
- **WHEN** `canon ingest plans`'s existing test suite (covering an
  unset hot-rung `dsn_env`, an unreachable cold-rung bucket, and a
  malformed `tiers.hot.schema`) is re-run, with its `canon.yaml`
  fixtures migrated to the rung/backend shape, after the rekey
- **THEN** every existing assertion (persisted counts, `unwritten`
  bodies, exit codes) passes with only the fixture's YAML vocabulary
  changed — no assertion's persisted/unwritten SEMANTICS change

### Requirement: A rung-scoped attachment variant sits beside the existing whole-policy variant, sharing the same per-backend core
`canon-cli::tiers` SHALL expose both a whole-policy lenient builder
(attempts every rung `canon.yaml` declares, each independently
lenient — `canon ingest plans`'s shape, which cannot scope to one
kind because a single pass may persist several kinds) and a rung-
scoped lenient builder (attempts only the rungs one specific
`RecordKind` needs — `canon query`'s shape) — both calling the same
per-backend attach-or-degrade functions, differing only in WHICH
rungs each one attempts. Supersedes this capability's original
kind-scoped/`TierKind`-scoped framing.

#### Scenario: The whole-policy variant still attempts every declared rung
- **WHEN** `canon ingest plans` runs against a `canon.yaml` declaring
  `local`+`hot`+`cold`, regardless of which kinds the current pass
  happens to persist
- **THEN** the whole-policy builder attempts to attach all three
  rungs' backends (each independently degrading on unavailability) —
  it is never narrowed to a subset of rungs based on the pass's
  discovered kinds

#### Scenario: The rung-scoped variant attempts only the rungs one kind needs
- **WHEN** `canon query --kind change` (local-routed only) runs
  against the same `canon.yaml`
- **THEN** the rung-scoped builder attempts only the local rung's
  backend — the hot and cold rungs' backends are never attempted,
  distinguishing its behavior from the whole-policy builder's "attempt
  everything declared" contract above
