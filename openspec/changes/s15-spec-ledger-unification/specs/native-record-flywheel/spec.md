## ADDED Requirements

### Requirement: Per-kind handle-based records-source adapters feed native verdicts into Trajectory
canon-ingest SHALL register ONE handle-based records-source adapter per
verdict-bearing native kind — `Review` and `Divergence` — each a
single-`RecordKind` registry entry matching the existing
`record_kind_for_records_adapter` one-adapter-one-kind dispatch
(`canon-cli::artifact_ingest`), mirroring the `Handoff` adapter's
`source_kind = Records` handle-based shape (never a path scan). Each
normalizes its record into an `ArtifactEvent`, derives a verdict, and persists
it into `canon_learn::Trajectory` via `store_trajectory`. `Scenario` is NOT a
flywheel source — it is a no-verdict INDEX materialized by `canon inventory
sync`, never fed through verdict derivation.

#### Scenario: A Review and a Divergence in the same run both produce trajectories
- **WHEN** one ingest run reads a `Review` record attesting a scenario AND a
  `Divergence` record resolving another, via their respective per-kind records
  adapters
- **THEN** BOTH verdict-bearing records are folded into `Trajectory` records
  persisted via `store_trajectory`, each reachable the same way an S4
  raw-artifact-derived `Trajectory` is — neither kind is dropped because a
  single adapter could read only one kind

#### Scenario: Each adapter is handle-based, like Handoff, never path-configured
- **WHEN** either per-kind records-source adapter's registry entry is inspected
- **THEN** its `source_kind` is `Records` — handle-based, fed an
  already-resolved `Vec<RawRecord>` by the caller — it never resolves a
  filesystem path itself, mirroring the `Handoff` adapter

### Requirement: Regime derivation for the records-source adapter
The records-source adapter SHALL derive `regime_key`'s inputs itself —
`role` from the record's `envelope.actor.role`, `area` from
`scenario_id.area()`, and `repo` from the configured root — mirroring
`attach_regime_key`, since `canon_learn::Trajectory` is keyed only by
`regime_key` (no `scenario_id`).

#### Scenario: regime_key is derived from actor role, scenario area, and root
- **WHEN** the adapter processes a `Review` record with
  `actor.role = "dev"`, `scenario_id = "world.firstbuy-hotdeal.26"`
  (area `world`), read from root `"canon"`
- **THEN** the resulting verdict's `regime_key` is derived with
  `role = dev`, `area = world`, `repo = canon` — the same
  `attach_regime_key` shape the S4 raw-artifact adapters already use

### Requirement: A native-records config switch, XOR against a raw-artifact path
`canon.yaml`'s `artifacts:` section SHALL gain a `native_records: bool`
(default false) switch that enables the native verdict records adapters
(`Review`/`Divergence`) against canon's OWN tiers. This switch is XOR-exclusive
with the raw-artifact path fields
(`ledger_root`/`divergences_root`/`openspec_root`): config validation SHALL
reject a configuration that sets `native_records: true` together with ANY
raw-artifact path, before any read happens, because the two paths'
near-identical verdict rows (which don't share a `trajectory_content_digest`,
so they would NOT dedupe) would double-count the same underlying evidence. The
driver SHALL run the native verdict adapters ONLY when `native_records: true`.
This switch scopes ONLY the new native verdict adapters — the existing
`Handoff` records-source adapter (also `source_kind = Records`, but not a native
verdict source) is UNAFFECTED and keeps its current behavior.

#### Scenario: native_records with a raw-artifact path fails config validation
- **WHEN** `canon.yaml`'s `artifacts:` section sets `native_records: true` AND
  also sets a raw-artifact path (e.g. `ledger_root`)
- **THEN** config validation rejects the configuration before any ingest read
  runs, reporting the XOR conflict

#### Scenario: native_records alone validates and runs the native verdict adapters
- **WHEN** `artifacts.native_records: true` is set with NO raw-artifact path
- **THEN** config validation passes and the ingest run drives the
  `Review`/`Divergence` native verdict adapters against canon's own tiers

#### Scenario: The handoff adapter is unaffected by the native_records switch
- **WHEN** `native_records` is false or unset and the `Handoff` records-source
  adapter is otherwise configured
- **THEN** the `Handoff` adapter runs exactly as before — the `native_records`
  switch gates only the `Review`/`Divergence` native verdict adapters, never
  `Handoff`
