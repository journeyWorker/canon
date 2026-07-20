## Why

`canon ingest sessions` (S3) is fully wired end to end: session adapters ->
`normalize_rows` -> `canon_store::registry::TierRegistry::persist`. Its
sibling half is not. S4 (`s4-artifact-ingest`) shipped the
`ledger`/`divergence`/`handoff`/`openspec-task` `ArtifactAdapter`s, the
table-driven `derive_verdict`/`attach_regime_key` mapping, and the
`ArtifactDispatchOutcome::UnsupportedSource` diagnostic that names exactly
what is missing: "a driver living OUTSIDE this crate ... resolving canon's
own Postgres-tier `Handoff` table through `canon_store::Tier::read`" — "not
yet built anywhere in this workspace". S6 (`s6-role-strategy-memory`) proved
the store->distill->rebuild->search round trip, but only against SYNTHETIC
`VerdictRow` struct literals, its own tasks.md honesty note says so
verbatim: "the production `canon ingest` artifact-driver that would feed
real `VerdictRow`s into this crate end-to-end does not exist yet (deferred
residual)". S9 (`s9-unified-surface`) built `mart_role_memory` and
`mart_flywheel_funnel` over `canon-learn`'s own parquet store — and today
that store has never been written to by anything but a test fixture.
Three changes each honestly deferred the SAME missing piece: the driver
that actually connects S4's derived verdicts to S6's learn store, so S9's
panels have real data to render. This change ships that driver.

## What Changes

- New `canon ingest artifacts [--repo <dir>] [--watch] [--interval-secs
  <n>] [--json]` in `canon-cli`: resolves `--repo` (the same
  `resolve_repo_root` nearest-`canon.yaml`-ancestor walk `canon
  context`/`canon gate`/`canon retrieve` already use), runs every
  registered `ArtifactAdapter`, derives verdicts, folds them into
  regime-keyed `canon_learn::Trajectory`s, and persists them into the SAME
  `ParquetTrajectoryStore` `canon retrieve` (S8) and `canon report`'s marts
  (S9) already read.
- `Path`-source adapters (`ledger`/`divergence`/`openspec-task`) run
  through the existing, UNCHANGED config-driven scan
  (`canon_ingest::artifact_registry::resolve_and_parse`) — no change to
  that function or any adapter.
- The `Records`-source `handoff` adapter is driven for the first time: this
  new module reads canon's own `Handoff` records off `canon-store`'s
  `Tier` (`canon_store::registry::TierRegistry::query`, resolved via the
  SAME `canon_cli::tiers::build_tiers` helper `canon query` uses) and hands
  the resulting `Vec<RawRecord>` to `HandoffAdapter::parse` directly as
  `ArtifactSourceHandle::Records` — `canon-ingest` gains no new dependency;
  it still never imports `canon-store`.
- Every adapter's contribution (or the reason it has none) is reported in
  a structured outcome, printed as a human summary or `--json` — a
  records-source read failure (no live `tiers.pg` DSN, `handoff` unrouted)
  degrades to a visible `"unavailable"` status for that ONE adapter, never
  a silent zero indistinguishable from "nothing found," and never aborts
  the rest of the pass (path-source adapters and persistence continue).
- After persisting a regime's trajectories, this driver also calls
  `canon_learn::rebuild_namespace` for that regime — without this, S9's
  `mart_role_memory` (which reads ONLY the distilled `StrategyItem` tier)
  would stay empty even after a successful ingest, defeating the point of
  this change. No new `canon-learn` API was needed: `store_trajectory`,
  `Trajectory::new`, and `rebuild_namespace` were already public.
- `openspec/changes/s4-artifact-ingest/tasks.md` (task 3.1's honesty note)
  and `openspec/changes/s6-role-strategy-memory/tasks.md` (task 5.1's
  evidence note) are updated to point at this change as the now-shipped
  driver they each named as a deferred residual.

## Capabilities

### New Capabilities

- `artifact-ingest-cli-driver`: `canon ingest artifacts`, the records-
  source `handoff` read step this driver alone performs, the derive-
  verdict -> regime-keyed-trajectory -> persist -> rebuild pipeline, and
  the documented per-adapter degrade-and-report seam for an unreachable
  records source.

### Modified Capabilities

_None — this change is purely additive: no existing `canon-cli` subcommand,
`canon-ingest` adapter, or `canon-learn` store API changes shape._

## Impact

- New CLI surface `canon ingest artifacts [--repo][--watch]
  [--interval-secs][--json]` on `canon-cli`.
- New module `crates/canon-cli/src/artifact_ingest.rs` — the only place
  `canon-ingest`'s artifact adapters and `canon-store`'s `Tier` meet, and
  the only place `canon-ingest`'s verdicts and `canon-learn`'s store meet.
- New `canon.yaml` top-level `artifacts:` section
  (`ledger_root`/`divergences_root`/`openspec_root`, mirroring
  `ArtifactSourceConfig`'s own field names) — parsed entirely inside
  `canon-cli`, exactly as that type's own doc comment anticipated
  ("a future `serde_yaml::from_str::<ArtifactSourceConfig>` ... needs no
  bespoke parser").
- Zero changes to `crates/canon-ingest/**` or `crates/canon-learn/**` — every
  API this driver calls (`resolve_and_parse`, `ArtifactAdapter::parse`,
  `derive_verdict`, `attach_regime_key`, `Trajectory::new`,
  `store_trajectory`, `rebuild_namespace`) was already public. The S4
  canon-store-free boundary (`cargo tree -p canon-ingest -e no-dev` shows
  no `canon-store`) is unchanged.
- Depends on S4 (artifact adapters + verdict mapping), S6 (the learn
  store), S8 (retrieval reads the same store), and S9 (the marts this
  change is proven against), all already landed.
