//! `canon ingest artifacts [--repo <dir>] [--watch]` (S14
//! `s14-artifact-ingest-cli`): the artifact/verdict half of canon's
//! join-spine driver — the exact "future `canon ingest` artifact-ingest
//! CLI wiring" `crates/canon-ingest/src/artifact_registry.rs`'s own doc
//! comment and `openspec/changes/{s4-artifact-ingest,s6-role-strategy-
//! memory}/tasks.md`'s honesty notes name as a DEFERRED residual —
//! mirrors [`crate::ingest`]'s already-proven `canon ingest sessions`
//! shape (adapters -> normalize/derive -> persist, with a documented
//! seam for whatever couldn't be persisted), generalized from
//! `SessionAdapter`/canon-store to `ArtifactAdapter`/canon-learn.
//!
//! # The S4 dependency boundary (frozen, never crossed here)
//! `canon-ingest` stays canon-store-free at runtime (S4 task 0.1) —
//! this module is where the two meet, exactly like `crate::ingest` is
//! for `SessionAdapter`/canon-store. Two adapter shapes, fed two
//! different ways:
//! - **`Path`-source adapters** (`ledger`/`divergence`/`openspec-task`)
//!   resolve their own root from the `canon.yaml`-configured
//!   [`canon_ingest::ArtifactSourceConfig`] and scan it directly —
//!   [`canon_ingest::artifact_registry::resolve_and_parse`]'s existing
//!   config-driven scan path, unchanged, is the ONLY thing this module
//!   calls for them.
//! - **`Records`-source adapters** (today, only `handoff`) cannot be
//!   driven by that config-driven scan at all — `resolve_and_parse`
//!   returns [`canon_ingest::artifact_registry::ArtifactDispatchOutcome::UnsupportedSource`]
//!   for them, by design (the P1 silent-drop `ReviewS4Full` fixed).
//!   THIS driver is what `artifact_registry`'s own doc comment names as
//!   the missing piece: it reads canon's own `Handoff`/`Review`/
//!   `Divergence` records straight off `canon-store`'s `Tier` (via a
//!   [`crate::tiers::build_lenient_tiers_for_kinds`] built ONCE, up
//!   front, over the union of every `Records`-source adapter's own
//!   `RecordKind` this pass will actually run — s29 design D6, before
//!   which this module called the strict [`crate::tiers::build_tiers`]
//!   PER adapter — + [`canon_store::registry::TierRegistry::query`],
//!   the same read path `canon query` uses) and hands the resulting
//!   `Vec<RawRecord>` to [`canon_ingest::ArtifactAdapter::parse`]
//!   directly as [`canon_ingest::ArtifactSourceHandle::Records`] —
//!   `canon-ingest` itself never touches `canon-store`.
//!
//! Every adapter's contribution (or its absence, and why) is reported
//! in [`ArtifactIngestOutcome::adapters`] — a `Records`-source adapter
//! whose read failed degrades to `status: "unavailable"` with the
//! reason, it is NEVER folded into a silent zero-events outcome
//! indistinguishable from "nothing to report" (the exact collapse
//! `ArtifactDispatchOutcome::UnsupportedSource` exists to prevent one
//! layer down — this driver extends that same discipline to its own
//! records-source read step).
//!
//! # Verdict derivation and persistence
//! Every collected [`canon_ingest::ArtifactEvent`] is folded through
//! [`canon_ingest::verdict::derive_verdict`] +
//! [`canon_ingest::verdict::attach_regime_key`] (S4's own frozen,
//! table-driven mapping — this module adds no verdict logic of its
//! own), grouped by the resulting `regime_key` into
//! [`canon_learn::Trajectory`]s, and persisted via
//! [`canon_learn::store_trajectory`] into the SAME
//! `ParquetTrajectoryStore` (under the `canon.yaml`-configured
//! `learn.root`, `canon_learn::LearnConfig::root`) that `canon
//! retrieve` and `canon-report`'s marts already read — no second store,
//! no new seam in `canon-learn` was needed (`store_trajectory`/
//! `Trajectory::new`/`rebuild_namespace` are already public API,
//! `crates/canon-learn/tests/fixture_round_trip.rs` already proves the
//! exact store->distill->rebuild->search round trip this module drives
//! with SYNTHETIC data; this module is the real-data caller that test's
//! own doc comment names as the deferred residual). Immediately after a
//! successful persist, [`canon_learn::rebuild_namespace`] re-derives
//! that regime's distilled `StrategyItem`s too — without this, S9's
//! `mart_role_memory` (which reads ONLY the distilled tier,
//! `stg_strategy_items`) would stay empty even after a successful
//! ingest, defeating this whole change's purpose.
//!
//! `regime_key`'s `<hash>` component is
//! [`canon_ingest::normalize::content_digest`] of the source event's
//! own join key (`scenario:<id>` / `handoff:<id>` / `task:<id>`) — the
//! SAME digest primitive S3 session-ingest already uses for its own
//! write-identity, reused here (not a new hashing scheme) to give
//! related events sharing one join key (e.g. an open review finding and
//! its later remediation) the identical `regime_key`, folding them onto
//! ONE trajectory exactly as [`canon_learn::Trajectory`]'s own doc
//! comment describes. Write-time idempotence (S4 tasks.md group 6) IS
//! enforced here: before persisting a regime's trajectory, [`run`]
//! checks whether an existing trajectory already recorded under the
//! EXACT SAME `regime_key` carries the identical (`regime_key` + the
//! ordered `VerdictRow` contents) digest ([`trajectory_content_digest`],
//! reusing the SAME [`content_digest`] primitive `regime_key`'s own
//! `<hash>` component uses — not a new hashing scheme) and skips the
//! persist (counted,
//! `ArtifactIngestOutcome::trajectories_skipped_duplicate`) rather than
//! double-writing — a repeat `canon ingest artifacts` pass over an
//! UNCHANGED corpus re-derives the identical digest
//! (`canon_ingest::scanner::scan_dir`'s deterministic byte-lexical file
//! order means the SAME `VerdictRow` sequence every pass) and persists
//! nothing new. A genuinely CHANGED corpus (a new/different verdict
//! folded onto that same `regime_key`) still persists a FRESH
//! trajectory alongside the untouched prior rows — the raw tier stays
//! append-only (design decision 3), never overwritten or deduped away.
//!
//! # Documented seam
//! A `Records`-source adapter's read genuinely CAN fail — no live
//! `tiers.pg` DSN (s29 design D6: the printed reason now names the
//! configured `dsn_env`/`bucket_env`, never a bare guess), `canon.yaml`
//! missing/unreadable, or `handoff`/`review`/`divergence-native`
//! simply unrouted — that adapter alone degrades to zero events
//! (reported, never silent, see above) while every `Path`-source
//! adapter and the persistence step still run normally. A genuinely
//! MALFORMED `canon.yaml` (bad YAML/policy syntax, an invalid pg
//! schema, a non-forward aging rule, …) is NOT this per-adapter
//! degrade, though (s29 design D6): it fails the WHOLE `canon ingest
//! artifacts` command loud, exactly like `crate::ingest`'s own
//! contract — "lenient" describes rung reachability only, config
//! correctness always stays loud. `canon-learn`'s own parquet store
//! has no analogous "unreachable" failure mode (`ParquetTrajectoryStore::open`
//! never fails — it is a bare `PathBuf`, directories are created lazily
//! on write), so unlike `crate::ingest`'s whole-batch unwritten
//! fallback, this driver's persistence step degrades per-trajectory: an
//! unregistered role (`canon.yaml` `learn.roles`) is skipped and
//! counted (`ArtifactIngestOutcome::trajectories_skipped_unregistered_role`),
//! never a fatal error for the rest of the batch.
//!
//! # S15 P4: native verdict adapters (design D7)
//! Two more `Records`-source adapters, `review`/`divergence-native`,
//! read canon's OWN `Review`/`Divergence` tiers (never a raw
//! ledger/divergence-manifest artifact) into verdicts — tagged
//! [`canon_ingest::artifact_registry::ArtifactAdapterEntry::native_verdict`]
//! `true` (`handoff` stays `false`: `Records`-kind but not a native
//! verdict source). They are driven ONLY when
//! `ArtifactSourceConfig::native_records` is `true` — [`run`] validates
//! ([`validate_artifact_source_config`]) that this switch is
//! XOR-exclusive with `ledger_root`/`divergences_root`/`openspec_root`
//! BEFORE any adapter read runs (spec `native-record-flywheel`
//! Requirement 3: the raw and native paths' verdict rows differ
//! slightly, so [`trajectory_content_digest`] would not dedupe them,
//! silently double-counting the same evidence). A `native_verdict:
//! true` entry with the switch off reports `status: "disabled"`
//! (never `"unavailable"`, reserved for a genuine read failure) and is
//! skipped entirely — `handoff` is UNAFFECTED and always runs. Their
//! events carry `detail["native_kind"]` (`"review"` |
//! `"divergence"`, plus `"status"` for the latter) instead of a
//! `derive_verdict`-mapped [`canon_ingest::ArtifactEventKind`] (always
//! `NonVerdict` for these two) — [`derive_verdict_for_event`] reads
//! that tag to dispatch to
//! [`canon_ingest::verdict::derive_native_review_verdict`]/
//! [`canon_ingest::verdict::derive_native_divergence_verdict`] instead
//! of [`canon_ingest::verdict::derive_verdict`], then rejoins the SAME
//! `attach_regime_key` + grouping + `trajectory_content_digest` +
//! `store_trajectory` + `rebuild_namespace` path every other event
//! already uses.

use std::collections::BTreeMap;
use std::path::Path;

use canon_ingest::artifact_adapter::{ArtifactEvent, ArtifactJoinKey, ArtifactSourceConfig, ArtifactSourceHandle};
use canon_ingest::artifact_registry::{ArtifactDispatchOutcome, ArtifactSourceKind};
use canon_ingest::normalize::content_digest;
use canon_ingest::verdict::{VerdictRow, attach_regime_key, derive_native_divergence_verdict, derive_native_review_verdict, derive_verdict};
use canon_learn::{LearnConfig, LearnError, ParquetStrategyStore, ParquetTrajectoryStore, RoleRegistry, Trajectory, TrajectoryId, TrajectoryStore, rebuild_namespace, store_trajectory};
use canon_model::envelope::RecordKind;
use canon_model::evidence::RawRecord;
use canon_model::ids::RegimeKey;
use canon_model::records::DivergenceStatus;
use canon_store::fold_latest_by_key;
use canon_store::policy::{BackendConfig, Rung, TierPolicy};
use canon_store::registry::TierRegistry;
use canon_store::tier::{StoreError, TierQuery};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::context::resolve_repo_root;
use crate::tiers::{self, TierCliError};

#[derive(Debug, thiserror::Error)]
pub enum ArtifactIngestError {
    #[error(transparent)]
    Learn(#[from] LearnError),
    #[error(transparent)]
    RegimeKey(#[from] canon_model::ids::JoinKeyError),
    /// s29 design D6: the up-front kind-scoped lenient tier build
    /// failed with something OTHER than "canon.yaml missing/
    /// unreadable" (handled as a graceful per-adapter degrade, see the
    /// module doc's "documented seam") — a genuinely malformed config
    /// (bad YAML/policy syntax, an invalid pg schema, a non-forward
    /// aging rule, …), surfaced by `main.rs` as a nonzero exit exactly
    /// like `Learn`/`RegimeKey`.
    #[error(transparent)]
    Tiers(#[from] TierCliError),
    /// `artifacts.native_records: true` configured together with ANY
    /// raw-artifact path field (`ledger_root`/`divergences_root`/
    /// `openspec_root`) — design D7's XOR, rejected by
    /// [`validate_artifact_source_config`] BEFORE any adapter read
    /// runs (spec `native-record-flywheel` Requirement 3), surfaced by
    /// `main.rs` as a nonzero exit exactly like `Learn`/`RegimeKey`.
    #[error("canon.yaml artifacts config: {0}")]
    ConfigXor(String),
}

/// One registered adapter's contribution to this pass — mirrors
/// `crate::ingest::AdapterSummary`'s per-adapter shape, generalized
/// with a `status` field so a `Records`-source read failure is a
/// visible, distinct outcome (module doc's "documented seam"), never
/// collapsed into the same zero-events shape an unconfigured
/// `Path`-source adapter reports.
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactAdapterSummary {
    pub adapter_id: &'static str,
    /// `"path"` | `"records"` (mirrors [`ArtifactSourceKind`]).
    pub source_kind: &'static str,
    /// `"read"` (this adapter's source was actually reached, whether
    /// or not it was configured/had records) | `"unavailable"` (a
    /// `Records`-source read failed before `parse` ever ran) |
    /// `"disabled"` (a `native_verdict: true` entry, S15 P4, whose
    /// `ArtifactSourceConfig::native_records` switch is off — never
    /// `"unavailable"`, which is reserved for a genuine read failure).
    pub status: &'static str,
    pub events_parsed: usize,
    pub malformed: usize,
    /// `Some(reason)` only when `status == "unavailable"`.
    pub unavailable_reason: Option<String>,
}

/// One regime-keyed trajectory this pass persisted.
#[derive(Debug, Clone, Serialize)]
pub struct PersistedTrajectory {
    pub regime_key: String,
    pub verdict_count: usize,
}

/// One `canon ingest artifacts` pass's outcome.
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactIngestOutcome {
    pub adapters: Vec<ArtifactAdapterSummary>,
    pub verdicts_derived: usize,
    pub trajectories_persisted: Vec<PersistedTrajectory>,
    /// A trajectory whose `regime_key` role is not registered in this
    /// repo's `canon_learn::RoleRegistry` (`canon.yaml` `learn.roles`,
    /// or the built-in set) — skipped and counted (module doc's
    /// "documented seam"), never a fatal error for the rest of the
    /// batch.
    pub trajectories_skipped_unregistered_role: usize,
    /// A trajectory whose (`regime_key` + ordered `VerdictRow`
    /// contents) digest already matches a trajectory this exact
    /// `regime_key` already holds ([`trajectory_content_digest`]) —
    /// skipped and counted (module doc's write-time idempotence),
    /// never a double-write of an unchanged corpus.
    pub trajectories_skipped_duplicate: usize,
    /// Sum of every regime's freshly-distilled `StrategyItem` count
    /// (`rebuild_namespace`'s return value) — the count that actually
    /// lands in `stg_strategy_items`, i.e. what makes S9's
    /// `mart_role_memory` non-empty.
    pub strategy_items_rebuilt: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ArtifactsSectionManifest {
    #[serde(default)]
    artifacts: ArtifactSourceConfig,
}

/// Parse `canon.yaml`'s `artifacts:` top-level section into an
/// [`ArtifactSourceConfig`] — the CLI-local wiring
/// `crate::artifact_adapter`'s own doc comment names as the future step
/// ("a future `serde_yaml::from_str::<ArtifactSourceConfig>` ... needs
/// no bespoke parser"); this IS that wiring, mirroring
/// `crate::tiers::build_tiers`'s "wire canon.yaml to a live type" role
/// for `canon-store`'s `TierPolicy`. A missing `artifacts:` section, or
/// an unreadable/unparseable `canon.yaml`, degrades to
/// `ArtifactSourceConfig::default()` (every field `None`, no source
/// scanned) rather than an error — matches every field's own "an
/// unconfigured source is never scanned" contract, and
/// `crate::retrieve::open_strategy_store`'s identical degrade-to-
/// default posture for this same file. Every configured path is
/// resolved relative to `repo` (never the process CWD), mirroring
/// `build_tiers`'s `tiers.git.root` resolution.
fn load_artifact_source_config(repo: &Path, canon_yaml_text: &str) -> ArtifactSourceConfig {
    let parsed = serde_yaml::from_str::<ArtifactsSectionManifest>(canon_yaml_text).map(|m| m.artifacts).unwrap_or_default();
    ArtifactSourceConfig {
        ledger_root: parsed.ledger_root.map(|p| repo.join(p)),
        divergences_root: parsed.divergences_root.map(|p| repo.join(p)),
        openspec_root: parsed.openspec_root.map(|p| repo.join(p)),
        native_records: parsed.native_records,
    }
}

/// The design D7 XOR: `native_records: true` together with ANY
/// raw-artifact path field is rejected BEFORE any adapter read runs
/// (spec `native-record-flywheel` Requirement 3 — the raw and native
/// paths' verdict rows differ slightly, so
/// [`trajectory_content_digest`] would not dedupe them, silently
/// double-counting the same underlying evidence). [`run`] calls this
/// immediately after [`load_artifact_source_config`], before touching
/// any adapter.
fn validate_artifact_source_config(config: &ArtifactSourceConfig) -> Result<(), ArtifactIngestError> {
    if config.native_records && (config.ledger_root.is_some() || config.divergences_root.is_some() || config.openspec_root.is_some()) {
        return Err(ArtifactIngestError::ConfigXor(
            "artifacts.native_records: true is XOR-exclusive with ledger_root/divergences_root/openspec_root — set at most one of them".to_string(),
        ));
    }
    Ok(())
}

/// The `canon-store` `RecordKind` a `Records`-source adapter reads —
/// `handoff` (S4), `review`/`divergence-native` (S15 P4). A future
/// `Records`-source adapter this registry gains without a matching arm
/// here reports itself `"unavailable"` with an explicit reason (never
/// a silent zero), so growing the registry can never regress into the
/// same silent-drop `ArtifactDispatchOutcome::UnsupportedSource`
/// prevents one layer down.
fn record_kind_for_records_adapter(adapter_id: &str) -> Result<RecordKind, String> {
    match adapter_id {
        "handoff" => Ok(RecordKind::Handoff),
        "review" => Ok(RecordKind::Review),
        "divergence-native" => Ok(RecordKind::Divergence),
        other => Err(format!("no canon-store RecordKind mapping registered in canon-cli for records-source adapter `{other}`")),
    }
}

/// Reads every `RawRecord` of `adapter_id`'s mapped `RecordKind` off
/// `store`/`policy` -- a [`canon_store::registry::TierRegistry`] +
/// its own [`canon_store::policy::TierPolicy`] built ONCE, up front,
/// by [`run`]'s own kind-scoped [`crate::tiers::build_lenient_tiers_for_kinds`]
/// call (s29 design D6) -- the module doc's "records-source adapters
/// ... fed by THIS canon-cli driver" step. `Err` (a `String` reason,
/// never a panic) when `store` is `None` (`canon.yaml` missing/
/// unreadable -- `missing_config_reason` names why), this adapter has
/// no `RecordKind` mapping, or the mapped kind's routed rung is
/// unrouted/unattached -- the caller reports this as `status:
/// "unavailable"` rather than a fatal whole-pass error.
///
/// `unavailable_reasons` (from the SAME up-front build) is checked
/// BEFORE `TierRegistry::query` runs: a routed rung that was
/// ATTEMPTED and degraded carries its build-time reason (the
/// configured `dsn_env`/`bucket_env` name) there, which is a more
/// SPECIFIC message than `TierRegistry::query`'s own generic
/// `Backend::default_unattached_reason()` fallback (s29 design D6) --
/// reusing `StoreError::tier_unavailable`'s canonical Display so the
/// wording matches the rest of the codebase. An UNROUTED kind (no
/// entry in `unavailable_reasons` at all, since the build step never
/// attempted a rung for it) falls through to `TierRegistry::query`'s
/// own `StoreError::UnroutedKind`, unchanged.
///
/// s21 P4 (design.md D5): `handoff` is routed to `PgTier`, whose `read`
/// now returns every retained historical version (s21 P3), not one row
/// per `handoff_id` — folded here, BEFORE `HandoffAdapter::parse` ever
/// sees the records, via the SAME shared `fold_latest_by_key` every
/// other multi-version reader applies, so `HandoffAdapter`'s own
/// idempotence contract ("one snapshot, several transitions" — its
/// module doc) keeps holding: it still receives exactly one CURRENT row
/// per `handoff_id`, never N historical rows misread as N independent
/// current ones. `review`/`divergence-native` are intentionally NOT
/// folded — both are git-routed, multi-row-per-key BY DESIGN (S15 P4's
/// native-verdict contract), the opposite of `handoff`'s contract.
fn read_records_for(
    adapter_id: &str,
    store: Option<&TierRegistry>,
    policy: Option<&TierPolicy>,
    unavailable_reasons: &BTreeMap<Rung, String>,
    missing_config_reason: Option<&str>,
) -> Result<Vec<RawRecord>, String> {
    let kind = record_kind_for_records_adapter(adapter_id)?;
    let Some(store) = store else {
        return Err(missing_config_reason.map(str::to_string).unwrap_or_else(|| "canon.yaml is missing or unreadable — no live tiers configured".to_string()));
    };
    let policy = policy.expect("policy is always Some whenever store is Some -- built together in run()");
    if let Ok(rung) = policy.tier_for(kind) {
        if let Some(reason) = unavailable_reasons.get(&rung) {
            let backend = policy.tiers.get(&rung).map(BackendConfig::backend);
            return Err(StoreError::tier_unavailable(rung, backend, reason.clone()).to_string());
        }
    }
    let records = store.query(&TierQuery::kind(kind)).map(|result| result.records).map_err(|e| e.to_string())?;
    Ok(if adapter_id == "handoff" { fold_handoff_records(kind, records) } else { records })
}

/// The `handoff`-only fold [`read_records_for`] applies — see its own
/// doc comment. Kept as a small, separately-named function (mirroring
/// `canon-cli::query::fold_pg_routed_kind`'s identical shape) rather
/// than inlined, so the ONE adapter this applies to stays visibly
/// distinct from `review`/`divergence-native`, which never call it.
fn fold_handoff_records(kind: RecordKind, records: Vec<RawRecord>) -> Vec<RawRecord> {
    struct Candidate {
        key: String,
        at: DateTime<Utc>,
        digest: String,
        record: RawRecord,
    }
    let candidates = records.into_iter().map(|record| {
        let key = canon_store::partition::resolve_partition(kind, &record.0).map(|p| p.natural_key).unwrap_or_default();
        let at = canon_store::tier::raw_record_at(&record);
        let digest = canon_store::partition::content_digest12(&record.0);
        Candidate { key, at, digest, record }
    });
    fold_latest_by_key(candidates, |c| c.key.clone(), |c| c.at, |c| c.digest.as_str()).into_values().map(|c| c.record).collect()
}

/// The resolved repo's `regime_key` `<repo>` segment — its directory
/// basename, canonicalized downstream by `canon_model::ids::regime_key`
/// itself (lowercased, whitespace/`/` collapsed to `-`), so this needs
/// no normalization of its own.
fn repo_label(repo: &Path) -> String {
    repo.file_name().and_then(|s| s.to_str()).unwrap_or("repo").to_string()
}

/// A stable, source-kind-tagged identity string for one
/// [`ArtifactJoinKey`] — the input to [`regime_hash`]'s digest, never
/// itself the `regime_key` hash (module doc: two events sharing a join
/// key fold onto the SAME trajectory).
fn join_key_identity(key: &ArtifactJoinKey) -> String {
    match key {
        ArtifactJoinKey::Scenario(id) => format!("scenario:{}", id.as_str()),
        ArtifactJoinKey::Handoff(id) => format!("handoff:{}", id.as_str()),
        ArtifactJoinKey::Task(id) => format!("task:{}", id.as_str()),
    }
}

/// `regime_key`'s `<hash>` component (module doc): reuses S3's
/// `content_digest` primitive over the join key's own identity string,
/// never a new hashing scheme.
fn regime_hash(key: &ArtifactJoinKey) -> String {
    content_digest(&serde_json::json!(join_key_identity(key)))
}

/// A stable content digest for one regime-keyed trajectory candidate —
/// `regime_key` + the ORDERED `VerdictRow` contents (`role`/`polarity`/
/// `becomes`, the only fields the type carries) — reusing the SAME
/// [`content_digest`] primitive [`regime_hash`] already uses (not a new
/// hashing scheme). [`run`]'s existence check calls this identically
/// for a freshly-derived candidate and for every already-persisted
/// trajectory under the same `regime_key`: an unchanged corpus
/// re-derives the identical digest (module doc's write-time
/// idempotence), a genuinely changed one a different one.
fn trajectory_content_digest(regime_key: &RegimeKey, verdicts: &[VerdictRow]) -> String {
    let verdict_json: Vec<serde_json::Value> =
        verdicts.iter().map(|v| serde_json::json!({"role": v.role.as_str(), "polarity": v.polarity.as_str(), "becomes": v.becomes.as_str()})).collect();
    content_digest(&serde_json::json!({"regime_key": regime_key.as_str(), "verdicts": verdict_json}))
}

/// Derives one `VerdictRow` from an `ArtifactEvent` — dispatching to
/// the S15 P4 native helpers
/// ([`derive_native_review_verdict`]/[`derive_native_divergence_verdict`],
/// design D7) for an event emitted by the native `review`/
/// `divergence-native` records-source adapters, identified by the
/// ADAPTER-CONTROLLED `event.adapter_id` (`"review"` |
/// `"divergence-native"`); every other event (the four S4 adapters,
/// including `handoff`) still goes through the frozen [`derive_verdict`]
/// table, UNCHANGED.
///
/// Dispatch is gated on `adapter_id`, NEVER on `detail["native_kind"]`
/// alone (ReviewP4): the S4 ledger/divergence raw-path adapters copy
/// raw artifact JSON verbatim into `detail`, so a raw record that
/// happens to carry a stray `native_kind` field must NOT hijack the
/// native branch and silently drop the verdict the frozen S4 table
/// would have scored. `adapter_id` is a `&'static str` each adapter
/// sets on its own events (never copied from source content), so it
/// cannot be spoofed by artifact data. The `divergence-native` arm
/// still reads the record's own `detail["status"]` (an
/// adapter-emitted payload, not a routing key) to recover the typed
/// `DivergenceStatus`.
///
/// The role is ALWAYS `event.authoring_role` (`envelope.actor.role`)
/// for a native event — never `derive_verdict`'s hard-coded
/// constants (spec `native-record-flywheel` Requirement 2). An event
/// with no derivable `authoring_role`, or a `divergence-native` event
/// whose `detail["status"]` fails to round-trip through
/// `DivergenceStatus` (a caller-contract violation, never expected
/// from `crate::artifact_ingest`'s own adapters), yields `None` —
/// skipped, never a fabricated role or status.
fn derive_verdict_for_event(event: &ArtifactEvent) -> Option<VerdictRow> {
    match event.adapter_id {
        "review" => event.authoring_role.as_ref().map(derive_native_review_verdict),
        "divergence-native" => {
            let role = event.authoring_role.as_ref()?;
            let status = event.detail.get("status").and_then(|v| serde_json::from_value::<DivergenceStatus>(v.clone()).ok())?;
            derive_native_divergence_verdict(&status, role)
        }
        _ => derive_verdict(event.kind, event.authoring_role.as_ref()),
    }
}

/// One scan -> derive-verdict -> persist pass over every registered
/// `ArtifactAdapter` (module doc).
pub fn run(repo: &Path) -> Result<ArtifactIngestOutcome, ArtifactIngestError> {
    let repo = resolve_repo_root(repo);
    let canon_yaml_path = repo.join("canon.yaml");
    let canon_yaml_text = std::fs::read_to_string(&canon_yaml_path).unwrap_or_default();

    let artifact_config = load_artifact_source_config(&repo, &canon_yaml_text);
    validate_artifact_source_config(&artifact_config)?;
    // `LearnConfig::from_manifest`'s own contract (crates/canon-learn/
    // src/config.rs): a genuinely ABSENT `learn:` section (or an empty
    // `canon.yaml`) already resolves to `Ok(LearnConfig::default())`
    // inside `from_manifest` itself — that clean-default case is NOT
    // touched here. Only a malformed `learn:` section (bad YAML, an
    // invalid `roles:`/`promotion:` kebab-slug `RoleId`, a non-positive
    // `promotion.<role>.n_min`/`window_days`) reaches `Err`, and that
    // MUST fail this whole pass loud (`?` into `ArtifactIngestError::
    // Learn`, surfaced by `main.rs` as a nonzero exit) rather than
    // silently falling back to `LearnConfig::default()` and persisting
    // this run's trajectories into `<repo>/.canon/learn` — the wrong
    // store whenever the repo configured a different `learn.root`.
    let learn_config = LearnConfig::from_manifest(&canon_yaml_text)?;

    // s29 design D6: build ONE kind-scoped lenient tier set up front,
    // covering the union of every `Records`-source adapter's mapped
    // `RecordKind` that will actually run this pass (a `native_verdict:
    // true` entry with the switch off needs no live tier at all) --
    // malformed config (bad YAML/policy syntax, an invalid pg schema,
    // a non-forward aging rule, …) fails the WHOLE command loud here,
    // matching `crate::ingest`'s own contract; `canon.yaml` missing/
    // unreadable degrades every `Records`-source adapter to
    // "unavailable" (the pre-existing "documented seam" posture); an
    // individually unrouted kind (e.g. `handoff` with no
    // `routing.handoff`) is untouched by this step and surfaces via
    // `TierRegistry::query`'s own `UnroutedKind`, exactly as before.
    let records_kinds: Vec<RecordKind> = canon_ingest::artifact_registry::registry()
        .iter()
        .filter(|entry| entry.source_kind == ArtifactSourceKind::Records)
        .filter(|entry| !(entry.native_verdict && !artifact_config.native_records))
        .filter_map(|entry| record_kind_for_records_adapter(entry.adapter_id()).ok())
        .collect();

    let (store, policy_for_reason, unavailable_reasons, missing_config_reason): (Option<TierRegistry>, Option<TierPolicy>, BTreeMap<Rung, String>, Option<String>) =
        match tiers::build_lenient_tiers_for_kinds(&canon_yaml_path, &records_kinds) {
            Ok(loaded) => {
                let policy = loaded.policy.clone();
                let reasons = loaded.unavailable_reasons.clone();
                (Some(TierRegistry::new(loaded.policy, loaded.git, loaded.pg, loaded.r2, loaded.sqlite)), Some(policy), reasons, None)
            }
            Err(TierCliError::ReadCanonYaml { path, source }) => (None, None, BTreeMap::new(), Some(format!("reading `{path}`: {source}"))),
            Err(other) => return Err(other.into()),
        };

    let mut adapters = Vec::new();
    let mut all_events: Vec<ArtifactEvent> = Vec::new();

    for entry in canon_ingest::artifact_registry::registry() {
        match entry.source_kind {
            ArtifactSourceKind::Path => match canon_ingest::artifact_registry::resolve_and_parse(entry, &artifact_config) {
                ArtifactDispatchOutcome::Parsed(parsed) => {
                    adapters.push(ArtifactAdapterSummary {
                        adapter_id: entry.adapter_id(),
                        source_kind: "path",
                        status: "read",
                        events_parsed: parsed.events.len(),
                        malformed: parsed.skipped,
                        unavailable_reason: None,
                    });
                    all_events.extend(parsed.events);
                }
                // Never reachable for a `Path`-kind entry today
                // (`resolve_and_parse` only returns this for
                // `Records`-kind entries) — handled anyway so a future
                // registry change can never silently regress into the
                // exact zero-events collapse this type exists to
                // prevent.
                ArtifactDispatchOutcome::UnsupportedSource { adapter_id, reason } => {
                    adapters.push(ArtifactAdapterSummary {
                        adapter_id,
                        source_kind: "path",
                        status: "unavailable",
                        events_parsed: 0,
                        malformed: 0,
                        unavailable_reason: Some(reason.to_string()),
                    });
                }
            },
            ArtifactSourceKind::Records => {
                if entry.native_verdict && !artifact_config.native_records {
                    // A native-verdict adapter (`review`/`divergence-native`,
                    // S15 P4) with the switch off is DISABLED, not
                    // unavailable — `"unavailable"` is reserved for a
                    // genuine read failure below.
                    adapters.push(ArtifactAdapterSummary {
                        adapter_id: entry.adapter_id(),
                        source_kind: "records",
                        status: "disabled",
                        events_parsed: 0,
                        malformed: 0,
                        unavailable_reason: None,
                    });
                    continue;
                }
                match read_records_for(entry.adapter_id(), store.as_ref(), policy_for_reason.as_ref(), &unavailable_reasons, missing_config_reason.as_deref()) {
                    Ok(raws) => {
                        let parsed = entry.adapter.parse(&ArtifactSourceHandle::Records(raws));
                        adapters.push(ArtifactAdapterSummary {
                            adapter_id: entry.adapter_id(),
                            source_kind: "records",
                            status: "read",
                            events_parsed: parsed.events.len(),
                            malformed: parsed.skipped,
                            unavailable_reason: None,
                        });
                        all_events.extend(parsed.events);
                    }
                    Err(reason) => {
                        adapters.push(ArtifactAdapterSummary {
                            adapter_id: entry.adapter_id(),
                            source_kind: "records",
                            status: "unavailable",
                            events_parsed: 0,
                            malformed: 0,
                            unavailable_reason: Some(reason),
                        });
                    }
                }
            }
        }
    }

    let label = repo_label(&repo);
    let mut verdicts: Vec<(RegimeKey, VerdictRow, DateTime<Utc>)> = Vec::new();
    for event in &all_events {
        let Some(row) = derive_verdict_for_event(event) else { continue };
        let area = event.area.clone().unwrap_or_else(|| "unscoped".to_string());
        let hash = regime_hash(&event.join_key);
        let verdict = attach_regime_key(row, event.join_key.clone(), &label, &area, &hash, event.trust_level.clone())?;
        verdicts.push((verdict.regime_key, verdict.row, event.at));
    }
    let verdicts_derived = verdicts.len();

    let mut by_regime: BTreeMap<RegimeKey, (Vec<VerdictRow>, DateTime<Utc>)> = BTreeMap::new();
    for (regime_key, row, at) in verdicts {
        let bucket = by_regime.entry(regime_key).or_insert_with(|| (Vec::new(), at));
        bucket.0.push(row);
        if at > bucket.1 {
            bucket.1 = at;
        }
    }

    let learn_root = repo.join(&learn_config.root);
    let trajectory_store = ParquetTrajectoryStore::open(learn_root.join("trajectories"));
    let strategy_store = ParquetStrategyStore::open(learn_root.join("strategies"));
    let role_registry = RoleRegistry::from_config(&learn_config);

    let mut trajectories_persisted = Vec::new();
    let mut trajectories_skipped_unregistered_role = 0usize;
    let mut trajectories_skipped_duplicate = 0usize;
    let mut strategy_items_rebuilt = 0usize;
    for (regime_key, (rows, at)) in by_regime {
        let verdict_count = rows.len();
        let digest = trajectory_content_digest(&regime_key, &rows);
        let already_persisted = trajectory_store
            .query_by_regime_key(&regime_key)?
            .iter()
            .any(|existing| trajectory_content_digest(&existing.regime_key, &existing.verdicts) == digest);
        if already_persisted {
            trajectories_skipped_duplicate += 1;
            continue;
        }
        let task = format!("{verdict_count} verdict(s) derived from canon-ingest artifact adapters for regime {regime_key}");
        let context =
            format!("canon ingest artifacts: {verdict_count} VerdictRow(s) folded onto regime_key {regime_key} by the S14 artifact-ingest driver");
        let trajectory = Trajectory::new(TrajectoryId::new(), regime_key.clone(), task, context, rows, at, vec!["artifact-ingest".to_string()])?;

        match store_trajectory(&role_registry, &trajectory_store, &trajectory) {
            Ok(()) => {
                trajectories_persisted.push(PersistedTrajectory { regime_key: regime_key.as_str().to_string(), verdict_count });
                let items = rebuild_namespace(&trajectory_store, &strategy_store, &regime_key)?;
                strategy_items_rebuilt += items.len();
            }
            Err(LearnError::UnregisteredRole(_)) => {
                trajectories_skipped_unregistered_role += 1;
            }
            Err(err) => return Err(err.into()),
        }
    }

    Ok(ArtifactIngestOutcome {
        adapters,
        verdicts_derived,
        trajectories_persisted,
        trajectories_skipped_unregistered_role,
        trajectories_skipped_duplicate,
        strategy_items_rebuilt,
    })
}

/// Human-readable run summary — one line per adapter (its `status`
/// spelled out, never silently folded into a bare count), then the
/// verdict/persistence tallies.
pub fn format_human(outcome: &ArtifactIngestOutcome) -> String {
    let mut out = String::new();
    for adapter in &outcome.adapters {
        match adapter.status {
            "read" => out.push_str(&format!(
                "{} ({}): {} event(s) parsed, {} malformed\n",
                adapter.adapter_id, adapter.source_kind, adapter.events_parsed, adapter.malformed
            )),
            "disabled" => out.push_str(&format!(
                "{} ({}): disabled — artifacts.native_records is off\n",
                adapter.adapter_id, adapter.source_kind
            )),
            _ => out.push_str(&format!(
                "{} ({}): records source unavailable — {}\n",
                adapter.adapter_id,
                adapter.source_kind,
                adapter.unavailable_reason.as_deref().unwrap_or("unknown reason")
            )),
        }
    }
    out.push_str(&format!("verdicts derived: {}\n", outcome.verdicts_derived));
    out.push_str(&format!("trajectories persisted: {}\n", outcome.trajectories_persisted.len()));
    for trajectory in &outcome.trajectories_persisted {
        out.push_str(&format!("  - {} ({} verdict(s))\n", trajectory.regime_key, trajectory.verdict_count));
    }
    out.push_str(&format!("trajectories skipped (unregistered role): {}\n", outcome.trajectories_skipped_unregistered_role));
    out.push_str(&format!("trajectories skipped (duplicate, already persisted): {}\n", outcome.trajectories_skipped_duplicate));
    out.push_str(&format!("strategy items rebuilt (distilled): {}\n", outcome.strategy_items_rebuilt));
    out
}

/// `--json`: the full outcome, machine-readable.
pub fn format_json(outcome: &ArtifactIngestOutcome) -> String {
    serde_json::to_string_pretty(outcome).expect("ArtifactIngestOutcome always serializes")
}

/// canon artifact-ingest's shared-contract selftest entry point (Wave-3
/// `canon selftest` aggregator, per-crate registration — unblocks S4
/// 7.4). Wraps this driver's pure write-identity invariants — `regime_hash`
/// (12-hex, deterministic, join-key-sensitive) and
/// `trajectory_content_digest` (deterministic, regime-key-sensitive,
/// verdict-content-and-order-sensitive) — as in-memory checks over
/// synthetic keys/verdicts. No filesystem or network read,
/// side-effect-free against the real repo.
///
/// `Ok(n)` = checks passed; `Err(_)` = one line per failure, never panics.
pub fn selftest() -> Result<usize, Vec<String>> {
    use canon_ingest::verdict::{Becomes, Polarity};
    use canon_model::ids::{RoleId, ScenarioId};

    let mut passed = 0;
    let mut failures = Vec::new();

    let scen_a = ArtifactJoinKey::Scenario(ScenarioId::parse("world.firstbuy-hotdeal.26").expect("valid scenario id"));
    let scen_b = ArtifactJoinKey::Scenario(ScenarioId::parse("world.firstbuy-hotdeal.27").expect("valid scenario id"));
    let h = regime_hash(&scen_a);
    if h.len() == 12 && h == regime_hash(&scen_a) && regime_hash(&scen_a) != regime_hash(&scen_b) {
        passed += 1;
    } else {
        failures.push("regime-hash: not a deterministic 12-hex digest sensitive to the join key".to_string());
    }

    let key = RegimeKey::parse(canon_model::ids::regime_key("dev", "acme-repo", "world", "abc123")).expect("valid regime key");
    let key2 = RegimeKey::parse(canon_model::ids::regime_key("dev", "acme-repo", "world", "def456")).expect("valid regime key");
    let vr = |role: &str, p: Polarity, b: Becomes| VerdictRow { role: RoleId::parse(role).expect("valid role"), polarity: p, becomes: b };

    if trajectory_content_digest(&key, &[vr("dev", Polarity::Failure, Becomes::GuardrailCandidate)])
        == trajectory_content_digest(&key, &[vr("dev", Polarity::Failure, Becomes::GuardrailCandidate)])
        && trajectory_content_digest(&key, &[vr("dev", Polarity::Failure, Becomes::GuardrailCandidate)])
            != trajectory_content_digest(&key2, &[vr("dev", Polarity::Failure, Becomes::GuardrailCandidate)])
    {
        passed += 1;
    } else {
        failures.push("trajectory-digest-determinism: not deterministic or not regime-key-sensitive".to_string());
    }

    let content_differs = trajectory_content_digest(&key, &[vr("dev", Polarity::Failure, Becomes::GuardrailCandidate)])
        != trajectory_content_digest(&key, &[vr("dev", Polarity::Success, Becomes::StrategyCandidate)]);
    let order_differs = trajectory_content_digest(&key, &[vr("dev", Polarity::Failure, Becomes::GuardrailCandidate), vr("dev", Polarity::Success, Becomes::StrategyCandidate)])
        != trajectory_content_digest(&key, &[vr("dev", Polarity::Success, Becomes::StrategyCandidate), vr("dev", Polarity::Failure, Becomes::GuardrailCandidate)]);
    if content_differs && order_differs {
        passed += 1;
    } else {
        failures.push("trajectory-digest-sensitivity: not sensitive to verdict content or order".to_string());
    }

    if failures.is_empty() { Ok(passed) } else { Err(failures) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_label_falls_back_to_repo_when_basename_is_unavailable() {
        assert_eq!(repo_label(Path::new("/")), "repo");
        assert_eq!(repo_label(Path::new("/tmp/acme-repo")), "acme-repo");
    }

    #[test]
    fn regime_hash_is_a_twelve_char_lowercase_hex_digest_and_is_deterministic() {
        let key = ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap());
        let hash = regime_hash(&key);
        assert_eq!(hash.len(), 12);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_eq!(hash, regime_hash(&key), "same join key must always hash to the same regime_key hash component");
    }

    #[test]
    fn regime_hash_differs_across_distinct_join_keys() {
        let a = ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap());
        let b = ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse("world.firstbuy-hotdeal.27").unwrap());
        assert_ne!(regime_hash(&a), regime_hash(&b));
    }

    #[test]
    fn load_artifact_source_config_resolves_configured_paths_against_repo_and_leaves_others_unconfigured() {
        let repo = Path::new("/tmp/some-repo");
        let yaml = "artifacts:\n  ledger_root: fixtures/ledger\n";
        let config = load_artifact_source_config(repo, yaml);
        assert_eq!(config.ledger_root, Some(repo.join("fixtures/ledger")));
        assert_eq!(config.divergences_root, None);
        assert_eq!(config.openspec_root, None);
    }

    #[test]
    fn load_artifact_source_config_degrades_to_default_when_artifacts_section_is_absent() {
        let repo = Path::new("/tmp/some-repo");
        let config = load_artifact_source_config(repo, "handoff_templates:\n  - foo\n");
        assert_eq!(config, ArtifactSourceConfig::default());
    }

    fn verdict_row(role: &str, polarity: canon_ingest::verdict::Polarity, becomes: canon_ingest::verdict::Becomes) -> VerdictRow {
        VerdictRow { role: canon_model::ids::RoleId::parse(role).unwrap(), polarity, becomes }
    }

    fn regime(area: &str, hash: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key("dev", "acme-repo", area, hash)).unwrap()
    }

    #[test]
    fn trajectory_content_digest_is_deterministic_for_identical_input() {
        let key = regime("world", "abc123");
        let rows = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate)];
        assert_eq!(trajectory_content_digest(&key, &rows), trajectory_content_digest(&key, &rows));
    }

    #[test]
    fn trajectory_content_digest_differs_across_distinct_regime_keys() {
        let rows = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate)];
        let a = regime("world", "abc123");
        let b = regime("world", "def456");
        assert_ne!(trajectory_content_digest(&a, &rows), trajectory_content_digest(&b, &rows));
    }

    #[test]
    fn trajectory_content_digest_differs_when_verdict_contents_differ() {
        let key = regime("world", "abc123");
        let a = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate)];
        let b = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Success, canon_ingest::verdict::Becomes::StrategyCandidate)];
        assert_ne!(trajectory_content_digest(&key, &a), trajectory_content_digest(&key, &b));
    }

    #[test]
    fn trajectory_content_digest_is_sensitive_to_verdict_order() {
        let key = regime("world", "abc123");
        let first = verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate);
        let second = verdict_row("dev", canon_ingest::verdict::Polarity::Success, canon_ingest::verdict::Becomes::StrategyCandidate);
        let forward = vec![first.clone(), second.clone()];
        let reversed = vec![second, first];
        assert_ne!(
            trajectory_content_digest(&key, &forward),
            trajectory_content_digest(&key, &reversed),
            "the digest folds ORDERED verdict contents — module doc's write-time idempotence relies on \
             `scan_dir`'s deterministic file order producing the SAME sequence every pass, so two different \
             orderings must never collide"
        );
    }

    #[test]
    fn a_spoofed_native_kind_in_s4_detail_does_not_hijack_the_s4_verdict_path() {
        // ReviewP4 regression: the S4 raw-path adapters copy raw
        // artifact JSON verbatim into `detail`, so a raw record could
        // carry a stray `native_kind` field. Dispatch is gated on the
        // adapter-controlled `adapter_id`, NOT the detail tag — so this
        // S4 (`ledger`) event still derives its normal S4 verdict and is
        // never silently dropped into the native (`None`-role) branch.
        let event = ArtifactEvent {
            adapter_id: "ledger",
            join_key: ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap()),
            kind: canon_ingest::artifact_adapter::ArtifactEventKind::CodeReviewFinding,
            authoring_role: None,
            area: Some("world".to_string()),
            trust_level: None,
            at: Utc::now(),
            detail: serde_json::json!({"native_kind": "review"}),
        };
        let via_dispatch = derive_verdict_for_event(&event);
        assert_eq!(
            via_dispatch,
            derive_verdict(canon_ingest::artifact_adapter::ArtifactEventKind::CodeReviewFinding, None),
            "a spoofed `native_kind` in S4 `detail` must NOT hijack dispatch — the frozen S4 table still applies"
        );
        assert!(via_dispatch.is_some(), "the CodeReviewFinding verdict is derived, never silently skipped by the native branch");
    }

    #[test]
    fn a_native_review_event_routes_by_adapter_id_even_with_nonverdict_kind() {
        // The native adapters set `kind = NonVerdict` (derive_verdict
        // would return `None`), so routing MUST come from `adapter_id`,
        // not `kind`/`detail`: a `review`-adapter event still derives a
        // native verdict whose role is the record's own actor.role.
        let role = canon_model::ids::RoleId::parse("content").unwrap();
        let event = ArtifactEvent {
            adapter_id: "review",
            join_key: ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap()),
            kind: canon_ingest::artifact_adapter::ArtifactEventKind::NonVerdict,
            authoring_role: Some(role.clone()),
            area: Some("world".to_string()),
            trust_level: None,
            at: Utc::now(),
            detail: serde_json::json!({"native_kind": "review"}),
        };
        let row = derive_verdict_for_event(&event).expect("a native review event derives a verdict via adapter_id routing");
        assert_eq!(row, derive_native_review_verdict(&role), "role is the record's actor.role, routed by adapter_id despite NonVerdict kind");
    }
}
