## ADDED Requirements

### Requirement: `canon ingest artifacts` drives every registered `ArtifactAdapter`
`canon ingest artifacts` SHALL run every registered `canon-ingest`
`ArtifactAdapter`, feeding each its source per its own `ArtifactSourceKind`
— a `Path`-source adapter through the existing config-driven scan,
unmodified; the `Records`-source `handoff` adapter by reading canon's own
`Handoff` records off `canon-store`'s `Tier` and supplying them directly.

#### Scenario: A path-source adapter's configured root is scanned
- **WHEN** `canon.yaml`'s `artifacts.ledger_root` names a directory
  containing a well-formed `code-review` finding record
- **THEN** `canon ingest artifacts` reports the `ledger` adapter with
  `status: "read"` and `events_parsed >= 1`

#### Scenario: The handoff records-source adapter is actually driven
- **WHEN** a real `Handoff` row exists in the tier `canon.yaml`'s
  `routing.handoff` names, and `canon ingest artifacts` runs
- **THEN** the `handoff` adapter's summary entry reports `status: "read"`
  and `events_parsed > 0` — never the same shape an unconfigured or
  unreachable source would report, and never
  `ArtifactDispatchOutcome::UnsupportedSource`'s reason text

### Requirement: An unreachable records source degrades only its own adapter
`canon ingest artifacts` SHALL report that ONE adapter's summary entry as
`status: "unavailable"` with a non-empty reason, and SHALL still complete
every other adapter's scan and persist whatever verdicts were derived,
when the records-source read step cannot resolve (no live tier attached
for the routed kind, the kind is unrouted, or `canon.yaml` cannot be
loaded).

#### Scenario: `handoff` is unrouted while `ledger` is still configured
- **WHEN** `canon.yaml` configures `artifacts.ledger_root` but has no
  `routing.handoff` entry
- **THEN** the `handoff` summary reports `status: "unavailable"` with a
  reason, the `ledger` summary still reports `status: "read"` with its
  parsed events, and any verdicts the `ledger` adapter derived are still
  persisted

### Requirement: Derived verdicts persist into canon-learn's trajectory store
`canon ingest artifacts` SHALL fold every `ArtifactEvent` that derives a
verdict (via `derive_verdict` + `attach_regime_key`, unmodified) into a
regime-keyed `Trajectory`, and SHALL persist it via
`canon_learn::store_trajectory` into the `canon.yaml`-configured
`ParquetTrajectoryStore` — the same store `canon retrieve` and
`canon-report`'s marts read.

#### Scenario: A derived verdict is readable back from the learn store
- **WHEN** `canon ingest artifacts` derives at least one verdict and
  completes successfully
- **THEN** `ParquetTrajectoryStore::query_by_regime_key` for that verdict's
  `regime_key`, opened at the SAME `canon.yaml`-configured learn root,
  returns at least one `Trajectory` carrying that verdict

#### Scenario: An unregistered role is skipped, not fatal
- **WHEN** a derived verdict's role is not registered in this repo's
  `RoleRegistry` (`canon.yaml` `learn.roles`, or the built-in set)
- **THEN** that one trajectory is skipped and counted
  (`trajectories_skipped_unregistered_role`), and every other regime's
  trajectory still persists

### Requirement: Persisted trajectories are distilled so role-memory marts render
After persisting a regime's trajectories, `canon ingest artifacts` SHALL
call `canon_learn::rebuild_namespace` for that regime, so its distilled
`StrategyItem`s land in the SAME store `canon-report`'s `mart_role_memory`
reads.

#### Scenario: `mart_role_memory` and `mart_flywheel_funnel` render non-empty after ingest
- **WHEN** `canon ingest artifacts` completes at least one successful
  trajectory persist against a repo whose `canon.yaml` also configures
  `canon-report`'s tiers
- **THEN** `canon_report::marts::fetch_role_memory` and
  `canon_report::marts::fetch_flywheel_funnel`, queried against that
  repo's `Roots`, both return at least one row for the ingested role
