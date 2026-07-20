## 1. `canon.yaml` wiring

- [x] 1.1 Parse `canon.yaml`'s `artifacts:` top-level section
      (`ledger_root`/`divergences_root`/`openspec_root`) into
      `canon_ingest::ArtifactSourceConfig`, resolving every configured
      path relative to `--repo` (never the process CWD). A missing
      `artifacts:` section, or an unreadable/unparseable `canon.yaml`,
      degrades to `ArtifactSourceConfig::default()` (no source scanned),
      never an error. Evidence: `crates/canon-cli/src/
      artifact_ingest.rs::load_artifact_source_config` +
      `artifact_ingest::tests::load_artifact_source_config_*`.

## 2. Path-source adapter driving

- [x] 2.1 Run every `ArtifactSourceKind::Path` registry entry through the
      existing, UNCHANGED `canon_ingest::artifact_registry::
      resolve_and_parse` — no new logic, no adapter change. Evidence:
      `artifact_ingest.rs::run`'s `ArtifactSourceKind::Path` arm.

## 3. Records-source (`handoff`) adapter driving

- [x] 3.1 Read canon's own `Handoff` records off `canon-store`'s `Tier`
      via `canon_cli::tiers::build_tiers` +
      `canon_store::registry::TierRegistry::query(&TierQuery::kind
      (RecordKind::Handoff))` — the SAME read path `canon query` uses —
      and hand the resulting `Vec<RawRecord>` to `HandoffAdapter::parse`
      as `ArtifactSourceHandle::Records`. `canon-ingest` gains no new
      dependency (`cargo tree -p canon-ingest -e no-dev` still shows no
      `canon-store`). Evidence: `artifact_ingest.rs::read_records_for` +
      `artifact_ingest.rs::run`'s `ArtifactSourceKind::Records` arm.
- [x] 3.2 A records-source read failure (no live PG DSN, `handoff`
      unrouted, malformed `canon.yaml`) reports `status: "unavailable"`
      with an explicit reason on that adapter's own summary entry —
      never a silent zero-events collapse, never aborts the rest of the
      pass. Evidence: `artifact_ingest.rs::ArtifactAdapterSummary` +
      integration test
      `an_unrouted_handoff_source_degrades_to_unavailable_without_aborting_the_rest_of_the_pass`
      (`crates/canon-cli/tests/artifact_ingest.rs`).

## 4. Verdict derivation and trajectory persistence

- [x] 4.1 Fold every collected `ArtifactEvent` through
      `canon_ingest::verdict::derive_verdict` +
      `canon_ingest::verdict::attach_regime_key` (unchanged, S4's own
      frozen table) — `regime_key`'s `<hash>` component is
      `canon_ingest::normalize::content_digest` of the source event's own
      join key (design.md decision 3). Evidence: `artifact_ingest.rs::run`'s
      verdict-derivation loop + `artifact_ingest::tests::regime_hash_*`.
- [x] 4.2 Group derived verdicts by `regime_key` into
      `canon_learn::Trajectory`s and persist each via
      `canon_learn::store_trajectory` into the `canon.yaml`-configured
      `ParquetTrajectoryStore` (`LearnConfig::root`) — the SAME store
      `canon retrieve` and `canon report`'s marts already read. No new
      `canon-learn` API. Evidence: `artifact_ingest.rs::run`'s
      persistence loop.
- [x] 4.3 A trajectory whose regime role is not registered in this repo's
      `RoleRegistry` is skipped and counted
      (`trajectories_skipped_unregistered_role`), never a fatal error for
      the rest of the batch. Evidence: `artifact_ingest.rs::run`'s
      `Err(LearnError::UnregisteredRole(_))` arm.
- [x] 4.4 After each successful persist, call
      `canon_learn::rebuild_namespace` for that regime so the distilled
      `StrategyItem` tier (`stg_strategy_items`) is populated too —
      without this, `mart_role_memory` stays empty even after a
      successful ingest. Evidence: `artifact_ingest.rs::run`'s
      `rebuild_namespace` call, `strategy_items_rebuilt` outcome field.

## 5. CLI surface

- [x] 5.1 `canon ingest artifacts [--repo <dir>] [--watch]
      [--interval-secs <n>] [--json]` — `IngestCommand::Artifacts` in
      `crates/canon-cli/src/main.rs`, dispatching to
      `canon_cli::artifact_ingest::run`. Human summary by default;
      `--json` for the full machine-readable outcome. Evidence:
      `main.rs`'s `IngestCommand::Artifacts` variant + `run_ingest_artifacts`
      + `ingest_artifacts_help_smoke` integration test.

## 6. End-to-end proof

- [x] 6.1 Integration test seeding BOTH source shapes in one fixture repo
      — a PATH-source `code-review` ledger finding AND a RECORDS-source
      `Handoff` row planted straight into canon-store's git tier — then
      running the real `canon` binary and asserting all three: (a) the
      handoff adapter's `status` is `"read"` with non-zero events (no
      `ArtifactDispatchOutcome::UnsupportedSource`-shaped silent drop);
      (b) a trajectory parquet row is readable back through
      `canon_learn::ParquetTrajectoryStore::query_by_regime_key`; (c)
      `canon_report::marts::fetch_role_memory` AND
      `fetch_flywheel_funnel`, run against the real `duckdb` CLI, both
      render non-empty rows keyed to the freshly-ingested regime/role.
      Evidence: `crates/canon-cli/tests/artifact_ingest.rs::
      ingest_artifacts_drives_both_source_shapes_persists_trajectories_and_feeds_real_marts`.

## 7. Honesty reconciliation

- [x] 7.1 Update `openspec/changes/s4-artifact-ingest/tasks.md` task
      3.1's honesty note to point at this change as the now-shipped
      driver it named as deferred.
- [x] 7.2 Update `openspec/changes/s6-role-strategy-memory/tasks.md` task
      5.1's evidence note the same way.
