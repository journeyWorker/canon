## ADDED Requirements

### Requirement: Exactly one shared per-tier degrade-or-propagate implementation backs every lenient tier build in canon-cli
`canon-cli` SHALL implement, in exactly one place (`tiers.rs`), the
logic that decides — per tier (`pg`/`r2`) — whether an attach failure
degrades to `None` (a bare `StoreError::TierUnavailable`) or propagates
loud (any other error); both the whole-policy builder (`canon ingest
plans`) and the kind-scoped builder (`canon query`) SHALL call that one
implementation, never carrying an independent copy of the decision.

#### Scenario: ingest plans and query degrade an unset pg DSN identically
- **WHEN** `canon ingest plans` and `canon query --kind <a pg-routed
  kind that also has a git/r2 candidate to persist/read>` each run
  against a `canon.yaml` with `tiers.pg`'s `dsn_env` unset
- **THEN** both commands' `pg` tier degrades to unattached via the
  identical `StoreError::TierUnavailable`-degrades-to-`None` path — the
  same function, not two independently written equivalents

#### Scenario: A malformed tiers.pg.schema propagates identically from both callers
- **WHEN** `canon ingest plans` and `canon query` each run against a
  `canon.yaml` whose `tiers.pg.schema` fails `validate_schema_ident`
- **THEN** both commands fail loud with the identical schema-validation
  error text — proving the "propagate anything other than
  `TierUnavailable`" branch is the same code path for both callers

### Requirement: canon ingest plans's own observable behavior is unchanged by the relocation
This relocation SHALL NOT change `canon ingest plans`'s existing error
text, exit codes, or persisted/unwritten counts for any existing
fixture: moving the shared lenient-attach logic out of `plans.rs` into
`tiers.rs` only relocates and generalizes WHERE the logic lives, never
WHAT it decides for its pre-existing caller.

#### Scenario: ingest plans's existing degradation fixtures pass unmodified
- **WHEN** `canon ingest plans`'s existing test suite (covering an
  unset `dsn_env`, an unreachable `r2` bucket, and a malformed
  `tiers.pg.schema`) is re-run after the relocation
- **THEN** every existing assertion (persisted counts, `unwritten`
  bodies, error text, exit codes) passes unmodified — no test needed
  updating to accommodate the relocation

### Requirement: A kind-scoped attachment variant sits beside the existing whole-policy variant, sharing the same per-tier core
`canon-cli::tiers` SHALL expose both a whole-policy lenient builder
(attempts every tier `canon.yaml` declares, each independently lenient —
`canon ingest plans`'s shape, which cannot scope to one kind because a
single pass may persist several kinds) and a kind-scoped lenient builder
(attempts only the tiers one specific `RecordKind` needs — `canon
query`'s shape) — both calling the same per-tier attach-or-degrade
functions, differing only in WHICH tiers each one attempts.

#### Scenario: The whole-policy variant still attempts every declared tier
- **WHEN** `canon ingest plans` runs against a `canon.yaml` declaring
  `git`+`pg`+`r2`, regardless of which kinds the current pass happens to
  persist
- **THEN** the whole-policy builder attempts to attach all three
  (each independently degrading on unavailability) — it is never
  narrowed to a subset of tiers based on the pass's discovered kinds

#### Scenario: The kind-scoped variant attempts only the tiers one kind needs
- **WHEN** `canon query --kind change` (git-routed only) runs against
  the same `canon.yaml`
- **THEN** the kind-scoped builder attempts only `git` — `pg` and `r2`
  are never attempted, distinguishing its behavior from the whole-policy
  builder's "attempt everything declared" contract above
