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
//!
//! # s39: the antecedent join (`joined-evidence-grounding`)
//! The verdict derivation above is a FILTER. An event
//! [`derive_verdict_for_event`] scores `None` for — most plainly an
//! `open`/`deferred` native divergence, which
//! [`canon_ingest::verdict::derive_native_divergence_verdict`] maps to
//! `None` CORRECTLY, because an open finding is not an outcome yet —
//! never reaches the accumulator, and its prose goes with it. On
//! canon's own corpus that discards the richest evidence the repo
//! holds: the `open` ship-blocker findings, each a file-and-line
//! locator plus a symptom and a remedy, while what survives is the bare
//! resolution narrative ("Fixed and independently re-verified+attested
//! at `<sha>`"). canon distilled THAT it got fixed and threw away WHAT
//! was broken — and both halves were already parsed, in the same batch,
//! in the same `Vec`, one loop before the drop.
//!
//! s39 (`joined-evidence-grounding`) closes that read-side, over the
//! join spine, with no new record kind and no extra tier read:
//! [`index_antecedents`] indexes every prose-bearing NON-verdict event
//! by its [`join_key_identity`] before the accumulation loop runs, and
//! each verdict-bearing event absorbs ([`antecedents_before`]) the
//! indexed events on its OWN join key at or before it in time. A
//! `resolved` divergence therefore distills together with the `open`
//! findings it closed, rendered as a visibly distinct block in the
//! trajectory's `context` ([`RegimeEvidence::trajectory_text`]).
//!
//! Nothing about verdict derivation moves. An open finding still mints
//! no verdict, no reward and no trajectory of its own — it contributes
//! evidence TEXT to a trajectory some OTHER event's verdict created.

use std::collections::BTreeMap;
use std::path::Path;

use canon_ingest::artifact_adapter::{ArtifactEvent, ArtifactJoinKey, ArtifactSourceConfig, ArtifactSourceHandle};
use canon_ingest::artifact_registry::{ArtifactDispatchOutcome, ArtifactSourceKind};
use canon_ingest::normalize::content_digest;
use canon_ingest::verdict::{VerdictRow, attach_regime_key, derive_native_divergence_verdict, derive_native_review_verdict, derive_verdict};
use canon_learn::{LearnConfig, LearnError, ParquetStrategyStore, ParquetTrajectoryStore, RewardRegistry, RoleRegistry, Trajectory, TrajectoryId, TrajectoryStore, VerdictOutcome, mark_trajectory_verdict, rebuild_namespace, store_trajectory};
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
    /// Trajectories whose covering verdict+reward was RESOLVED and
    /// written back in this same pass
    /// ([`canon_learn::RewardRegistry::compute_for_trajectory`] +
    /// [`canon_learn::mark_trajectory_verdict`]). Every freshly
    /// constructed `Trajectory` starts `VerdictOutcome::Pending` (S7
    /// design D2's two-phase reward-write model), and S7's promotion
    /// gates read `verdict_record.outcome`, NOT the raw `verdicts`
    /// list — so a trajectory left `Pending` is invisible to
    /// `OccurrencePromotionGate`/`CrnPromotionGate` and can never
    /// corroborate a promotion. This counter is what makes the
    /// artifact-ingest half of the flywheel observable.
    pub trajectories_marked: usize,
    /// Trajectories deliberately LEFT `Pending`: the role's own
    /// `RewardFn` resolved to [`VerdictOutcome::Pending`], which
    /// `mark_trajectory_verdict` rejects outright
    /// ([`canon_learn::LearnError::CannotMarkVerdictPending`]) because
    /// `Pending` is the unset default, never a covering-verdict write.
    /// The `dev` role reaches this legitimately: `compute_dev_reward`'s
    /// additive triad stays `Pending` below a full `1.0` (a partial
    /// `pr-merged` + `ci-pass` without `no-rollback`), waiting for the
    /// S7 webhook receiver's no-rollback timer to resolve it. Counted,
    /// never an error.
    pub trajectories_left_pending: usize,
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
    let records = read_kind_records(kind, store, policy, unavailable_reasons, missing_config_reason)?;
    Ok(if adapter_id == "handoff" { fold_handoff_records(kind, records) } else { records })
}

/// The kind-level read core [`read_records_for`] is a thin,
/// adapter-keyed wrapper around — extracted by s39
/// (`joined-evidence-grounding`) so its second reader, the
/// [`ScenarioTitleIndex`] join, reuses the EXACT same read path
/// (up-front `unavailable_reasons` precheck, then
/// [`TierRegistry::query`]) instead of growing a second one beside it
/// with its own subtly different degrade rules. Every mechanic and
/// every failure mode is documented on [`read_records_for`] above; the
/// only thing that moved down here is the part that never depended on
/// an `adapter_id` at all.
fn read_kind_records(
    kind: RecordKind,
    store: Option<&TierRegistry>,
    policy: Option<&TierPolicy>,
    unavailable_reasons: &BTreeMap<Rung, String>,
    missing_config_reason: Option<&str>,
) -> Result<Vec<RawRecord>, String> {
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
    store.query(&TierQuery::kind(kind)).map(|result| result.records).map_err(|e| e.to_string())
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
        // s38 (`evidence-bearing-memory`): the record's own format
        // generation, the rung `fold_latest_by_key` now compares between
        // `at` and `digest`. Read through `raw_record_schema` rather than
        // a typed envelope because these candidates are bare `RawRecord`
        // JSON at this point; a malformed record missing `schema`
        // degrades to `0`, which sorts strictly below every real
        // generation (kinds start at `1`), so it can never out-rank a
        // well-formed row on a tie.
        schema: u32,
        digest: String,
        record: RawRecord,
    }
    let candidates = records.into_iter().map(|record| {
        let key = canon_store::partition::resolve_partition(kind, &record.0).map(|p| p.natural_key).unwrap_or_default();
        let at = canon_store::tier::raw_record_at(&record);
        let schema = canon_store::tier::raw_record_schema(&record);
        let digest = canon_store::partition::content_digest12(&record.0);
        Candidate { key, at, schema, digest, record }
    });
    fold_latest_by_key(candidates, |c| c.key.clone(), |c| c.at, |c| c.schema, |c| c.digest.as_str()).into_values().map(|c| c.record).collect()
}

/// The char cap [`ScenarioTitleIndex`] applies to an indexed scenario
/// title (s39 `joined-evidence-grounding`), mirroring the role
/// `canon_ingest::artifact_adapter`'s own `EVIDENCE_TEXT_MAX_CHARS`
/// plays for mined `detail` prose: a joined title lands in a
/// trajectory's `task`, which becomes a distilled
/// `canon_learn::StrategyItem`'s TITLE, and several of those are
/// injected into a dispatched agent's context at once — so one
/// pathological record must not be able to bloat every retrieval that
/// touches its regime.
///
/// 160 is measured, not guessed: the 16 `Scenario` records in this
/// repo's own `.canon/ledger/kind=scenario` carry titles of 33–63 chars
/// (one Gherkin `Scenario:` line), so 160 leaves every genuine title
/// INTACT with better than 2x headroom while bounding a pasted
/// paragraph to roughly two terminal lines. Deliberately far tighter
/// than the 512 chars evidence prose gets: this is a TITLE, and the
/// full narrative already has a home in `context`.
const SCENARIO_TITLE_MAX_CHARS: usize = 160;

/// `(project_id, scenario_id) -> title`, read ONCE per `canon ingest
/// artifacts` pass off the `Scenario` ledger index (s39
/// `joined-evidence-grounding`).
///
/// This exists because canon's join spine means a record does not have
/// to carry every fact about itself: `canon_model::records::Review` is
/// exactly `{envelope, project_id, scenario_id, reviewer, pin,
/// provenance_ref}` — genuinely no prose field — while `Scenario`
/// carries a real sentence in `title`. A review attests
/// `(project_id, scenario_id)`, so joining that pair is what lets its
/// trajectory say WHAT was attested instead of only naming the
/// scenario id and its pin sha.
///
/// Keyed scenario-id-OUTER / project-id-inner, which looks inverted
/// against the `(project_id, scenario_id)` join pair and is deliberate:
/// an [`ArtifactEvent`]'s `join_key` carries the scenario id ALONE
/// (`ArtifactJoinKey::Scenario`), and its `project_id` is only
/// recoverable from adapter-emitted `detail` ([`event_project_id`]),
/// which not every adapter populates. Scenario-first therefore makes
/// the always-available half the lookup's first level, and keeps the
/// project level a real part of the key rather than dropping it: a
/// scenario id that two projects both define resolves only when the
/// event names its project, and otherwise contributes NO title (see
/// [`Self::title_for`]) instead of printing another project's sentence.
///
/// `BTreeMap` at both levels, so the index — and anything derived from
/// it — iterates in one total, data-derived order regardless of the
/// order the tier handed the records over.
#[derive(Debug, Default)]
struct ScenarioTitleIndex {
    titles: BTreeMap<String, BTreeMap<String, IndexedScenarioTitle>>,
}

/// One indexed title plus the `at` that earned it its slot — see
/// [`ScenarioTitleIndex::absorb_record`]'s newest-wins rule.
#[derive(Debug)]
struct IndexedScenarioTitle {
    at: DateTime<Utc>,
    title: String,
}

impl ScenarioTitleIndex {
    /// Indexes every well-formed `Scenario` record, skipping the rest.
    ///
    /// A record missing `scenario_id`/`project_id`/`title`, or carrying
    /// a blank one, contributes NO entry — a strategy title must never
    /// be enriched with an empty sentence, and a partially-formed record
    /// is not evidence (the same "malformed evidence is no evidence"
    /// discipline `canon_ingest::ArtifactParseOutcome` applies one layer
    /// out). Reads the fields off the bare `RawRecord` JSON rather than
    /// deserializing `canon_model::records::Scenario`, so one
    /// unparseable record — or a future schema generation this build
    /// does not know — degrades to a missing title rather than aborting
    /// the whole index.
    fn from_records(records: &[RawRecord]) -> Self {
        let mut index = Self::default();
        for record in records {
            index.absorb_record(record);
        }
        index
    }

    /// Folds one record in, newest `at` winning its
    /// `(project_id, scenario_id)` slot.
    ///
    /// The ledger's own file naming (`<project>__<scenario>__<digest>`)
    /// means a RETITLED scenario lands as a SECOND record beside its
    /// predecessor rather than replacing it, and a `LiveDb`-class rung
    /// retains historical versions outright (s21 P3) — so "which title
    /// is current" is a real question, not a hypothetical. Comparing
    /// `at` answers it, and makes this index independent of the order
    /// the tier yielded records in: a strictly newer record replaces the
    /// slot, an equal-or-older one leaves it alone. Determinism is
    /// load-bearing here — the title reaches `Trajectory::task`, and the
    /// write-time idempotence skip only fires when a second pass derives
    /// byte-identical text.
    ///
    /// Reads that timestamp through [`raw_record_at_or_min`] rather than
    /// `canon_store::tier::raw_record_at`, which `expect`s a
    /// well-formed `at` and would PANIC on a hand-edited ledger file —
    /// unacceptable for a join whose whole contract is that it can only
    /// ever fail to enrich, never fail a run.
    fn absorb_record(&mut self, record: &RawRecord) {
        let (Some(scenario_id), Some(project_id), Some(title)) =
            (raw_field(record, "scenario_id"), raw_field(record, "project_id"), raw_field(record, "title").map(compact_scenario_title))
        else {
            return;
        };
        let at = raw_record_at_or_min(record);
        match self.titles.entry(scenario_id.to_string()).or_default().entry(project_id.to_string()) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(IndexedScenarioTitle { at, title });
            }
            std::collections::btree_map::Entry::Occupied(mut slot) if at > slot.get().at => {
                slot.insert(IndexedScenarioTitle { at, title });
            }
            std::collections::btree_map::Entry::Occupied(_) => {}
        }
    }

    /// The title `join_key` names, or `None` — which is the ONLY
    /// outcome for a non-scenario join key (a `handoff:`/`task:` id has
    /// no scenario to name), an id this index never saw, or an id two
    /// projects both define that `project_id` does not disambiguate.
    ///
    /// `None` is not a degraded rendering, it is the s38 rendering:
    /// [`RegimeEvidence::trajectory_text`] then emits its pre-s39 `task`
    /// byte for byte, so this whole join is purely additive.
    fn title_for(&self, join_key: &ArtifactJoinKey, project_id: Option<&str>) -> Option<&str> {
        let ArtifactJoinKey::Scenario(scenario_id) = join_key else { return None };
        let by_project = self.titles.get(scenario_id.as_str())?;
        let indexed = match project_id {
            Some(project) => by_project.get(project)?,
            // Exactly one project defines this scenario id, so naming it
            // is unambiguous even though the event never said which
            // project it belongs to. Two or more and there is no honest
            // answer: pick nothing rather than another project's
            // sentence.
            None if by_project.len() == 1 => by_project.values().next()?,
            None => return None,
        };
        Some(indexed.title.as_str())
    }
}

/// One trimmed, non-blank string field off a bare `RawRecord`'s JSON —
/// the shape [`ScenarioTitleIndex::absorb_record`] needs for all three
/// of its fields, where a present-but-blank value must read exactly like
/// an absent one.
fn raw_field<'a>(record: &'a RawRecord, field: &str) -> Option<&'a str> {
    record.0.get(field)?.as_str().map(str::trim).filter(|value| !value.is_empty())
}

/// A bare `RawRecord`'s `at`, or `DateTime::<Utc>::MIN_UTC` when it is
/// absent or not RFC-3339 — the newest-wins discriminator
/// [`ScenarioTitleIndex::absorb_record`] compares.
///
/// `canon_store::tier::raw_record_at` is the shared accessor for this
/// field, but it `expect`s the parse to succeed ("already passed
/// validate_envelope_shape") and therefore PANICS on a record that
/// reached this index without that guarantee — a hand-edited ledger
/// file is enough. The scenario-title join must degrade, never abort a
/// `canon ingest artifacts` run, so it takes the same posture
/// `canon_store::tier::raw_record_schema` already takes for ITS field:
/// a malformed value falls back to a floor that sorts strictly below
/// every well-formed record, so it can never out-rank one.
fn raw_record_at_or_min(record: &RawRecord) -> DateTime<Utc> {
    record
        .0
        .get("at")
        .and_then(|value| value.as_str())
        .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
        .map(|at| at.with_timezone(&Utc))
        .unwrap_or(DateTime::<Utc>::MIN_UTC)
}

/// Collapses whitespace runs to single spaces, trims, and caps at
/// [`SCENARIO_TITLE_MAX_CHARS`] CHARS (never bytes — canon's corpora
/// carry Korean prose and a byte cut would split a codepoint), marking a
/// cut with a trailing `…`.
///
/// Deliberately a canon-cli-local twin of
/// `canon_ingest::artifact_adapter`'s private `compact_evidence_text`
/// rather than a new public export from that crate: the two caps answer
/// different questions (a strategy TITLE versus its CONTENT, see
/// [`SCENARIO_TITLE_MAX_CHARS`]), and widening `canon-ingest`'s API to
/// share fifteen lines would couple them into moving together.
fn compact_scenario_title(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(SCENARIO_TITLE_MAX_CHARS + 4));
    for word in raw.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if let Some((cut, _)) = out.char_indices().nth(SCENARIO_TITLE_MAX_CHARS) {
        out.truncate(cut);
        let trimmed = out.trim_end().len();
        out.truncate(trimmed);
        out.push('…');
    }
    out
}

/// The `project_id` an event's own adapter recorded in `detail`, if any
/// — the second half of [`ScenarioTitleIndex`]'s join key (s39
/// `joined-evidence-grounding`).
///
/// The native `review`/`divergence-native` adapters emit it explicitly,
/// and the raw `ledger`/`divergence` adapters copy their source JSON
/// verbatim, so it is usually present; `None` when it is not, which
/// [`ScenarioTitleIndex::title_for`] resolves through its
/// single-project fallback rather than failing. Read from `detail` —
/// artifact-supplied content — is safe here precisely because a wrong
/// value can only ever cost a title lookup a hit: unlike
/// `derive_verdict_for_event`'s `adapter_id` dispatch, nothing routes on
/// it, and no verdict, reward, or trajectory identity depends on it.
fn event_project_id(event: &ArtifactEvent) -> Option<&str> {
    event.detail.get("project_id")?.as_str().map(str::trim).filter(|value| !value.is_empty())
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

/// One derived verdict PLUS the salient evidence of the event it came
/// from (`s38-evidence-bearing-memory`).
///
/// [`run`]'s accumulator used to collapse to a bare
/// `(RegimeKey, VerdictRow, DateTime<Utc>)` tuple and drop the event on
/// the floor — so the only material left for the trajectory's
/// `task`/`context` (which become a distilled
/// `canon_learn::StrategyItem`'s TITLE and CONTENT, and from there the
/// guidance `canon retrieve` injects into a dispatched agent) was a
/// description of this driver's own plumbing. A named struct rather
/// than a wider tuple because six positional fields at one call site is
/// how the wrong two get swapped.
struct DerivedVerdict {
    regime_key: RegimeKey,
    row: VerdictRow,
    at: DateTime<Utc>,
    /// The event's [`ArtifactJoinKey::as_str`] — the CONCRETE artifact
    /// id (`platformer.hud.01`) the trajectory names.
    join_key: String,
    /// The joined `Scenario.title` for that id, when this pass's
    /// [`ScenarioTitleIndex`] holds one (s39 `joined-evidence-grounding`)
    /// — resolved in [`run`]'s accumulation loop, the one place that has
    /// both the index and the event's own `detail` `project_id` in hand.
    /// `None` for a non-scenario join key, an unindexed id, or an
    /// unreadable/unrouted scenario tier, and then the rendered `task`
    /// stays exactly what s38 produced.
    scenario_title: Option<String>,
    kind_label: &'static str,
    evidence_line: String,
    /// The prose of the non-verdict events that preceded this one on
    /// the same join key (s39 `joined-evidence-grounding`), already
    /// `(at, line)`-ordered and deduped by [`antecedents_before`].
    antecedents: Vec<String>,
}

/// One regime group's accumulated verdicts and evidence — the value
/// side of [`group_by_regime`]'s map (`s38-evidence-bearing-memory`).
struct RegimeEvidence {
    rows: Vec<VerdictRow>,
    /// The newest source-record timestamp in this group (the
    /// trajectory's `recorded_at`).
    latest_at: DateTime<Utc>,
    /// `Some(id)` while every absorbed verdict shared ONE join key —
    /// which is the invariant by construction, since `regime_key`'s
    /// `<hash>` component IS [`regime_hash`] of the join key, so one
    /// regime group can only ever hold one key. `None` is the honest
    /// degrade if that ever stops holding: [`Self::trajectory_text`]
    /// then names the regime key rather than picking one member's id
    /// and printing a wrong one.
    join_key: Option<String>,
    /// The joined `Scenario.title` this group's `join_key` names (s39
    /// `joined-evidence-grounding`) — the only field
    /// [`Self::trajectory_text`]'s `task` half gained since s38, and
    /// `None` whenever no title was joined, which renders the s38 text
    /// byte for byte.
    scenario_title: Option<String>,
    /// Deduped [`ArtifactEvent::display_label`]s in first-seen order —
    /// the SAME strings [`ArtifactEvent::evidence_line`] prefixes its
    /// lines with, so a trajectory's title and its content always name
    /// the same things.
    kind_labels: Vec<&'static str>,
    /// Deduped [`ArtifactEvent::evidence_line`]s in first-seen order.
    evidence_lines: Vec<String>,
    /// The antecedent findings this regime's verdicts absorbed (s39
    /// `joined-evidence-grounding`) — deduped, in the `(at, line)` order
    /// [`antecedents_before`] hands them over, and capped by
    /// [`MAX_ANTECEDENT_LINES`]/[`MAX_ANTECEDENT_CHARS`].
    antecedent_lines: Vec<String>,
    /// How many eligible antecedents those caps turned away. Rendered
    /// as one short marker line rather than vanishing: this module
    /// reports every degrade it performs (module doc's "documented
    /// seam"), and a strategy quoting three of five findings without
    /// saying so reads as though there were only three.
    antecedents_omitted: usize,
}

impl Default for RegimeEvidence {
    /// `latest_at` seeds `DateTime::<Utc>::MIN_UTC` so the first
    /// absorbed verdict always wins the running max — no `Option`
    /// dance, and no unreachable `unwrap` at the one place the
    /// timestamp is read. (`chrono` implements no `Default` for
    /// `DateTime<Utc>`, hence the hand-written impl.)
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            latest_at: DateTime::<Utc>::MIN_UTC,
            join_key: None,
            scenario_title: None,
            kind_labels: Vec::new(),
            evidence_lines: Vec::new(),
            antecedent_lines: Vec::new(),
            antecedents_omitted: 0,
        }
    }
}

impl RegimeEvidence {
    /// Folds one [`DerivedVerdict`] in, keeping the running max
    /// timestamp and both first-seen-ordered dedup lists.
    fn absorb(&mut self, derived: DerivedVerdict) {
        let DerivedVerdict { regime_key: _, row, at, join_key, scenario_title, kind_label, evidence_line, antecedents } = derived;
        if self.rows.is_empty() {
            self.join_key = Some(join_key);
        } else {
            // Unreachable by construction (see `join_key`'s doc): a
            // group's `regime_key` hash IS this key's digest. Asserted
            // in debug rather than assumed silently — and in release it
            // degrades to naming the regime key, never a wrong id.
            debug_assert_eq!(
                self.join_key.as_deref(),
                Some(join_key.as_str()),
                "a regime group must hold exactly one join key — regime_hash is content_digest of it"
            );
            if self.join_key.as_deref() != Some(join_key.as_str()) {
                self.join_key = None;
            }
        }
        // s39: every verdict in a group shares one join key, so they
        // resolve the SAME title — except one whose adapter recorded no
        // `project_id` in `detail`, which resolves `None` where its
        // siblings resolve `Some`. First non-`None` wins, so the
        // rendered title never depends on which member happened to be
        // absorbed first.
        if self.scenario_title.is_none() {
            self.scenario_title = scenario_title;
        }
        self.rows.push(row);
        if at > self.latest_at {
            self.latest_at = at;
        }
        push_first_seen(&mut self.kind_labels, kind_label);
        push_first_seen(&mut self.evidence_lines, evidence_line);
        // Strictly AFTER this verdict's own line lands in
        // `evidence_lines` — see `absorb_antecedents`' dedupe contract.
        self.absorb_antecedents(antecedents);
    }

    /// Folds one verdict's antecedent lines into the regime's shared
    /// block (s39 `joined-evidence-grounding`).
    ///
    /// Called from [`Self::absorb`] AFTER that verdict's own
    /// `evidence_line` is in `evidence_lines`, which is what lets the
    /// dedupe consult BOTH lists: a line already rendered as an outcome
    /// must not reappear under the "preceded by" heading, where it would
    /// read as a distinct earlier finding rather than the same sentence
    /// twice.
    ///
    /// The caps live here rather than in [`antecedents_before`] because
    /// what they bound is the RENDERED block, and that block is
    /// per-regime: a regime holding six verdicts would otherwise
    /// accumulate six separately-capped lists. Both bounds are checked
    /// before the push, so the block never transiently exceeds them, and
    /// a rejection is COUNTED rather than dropped.
    fn absorb_antecedents(&mut self, antecedents: Vec<String>) {
        for line in antecedents {
            if self.evidence_lines.contains(&line) || self.antecedent_lines.contains(&line) {
                continue;
            }
            // Recomputed rather than carried as a running field: at
            // `MAX_ANTECEDENT_LINES` entries this sums at most four
            // short strings, and a cached counter is one more piece of
            // state that can desync from the vector it describes.
            let budget: usize = self.antecedent_lines.iter().map(|kept| kept.chars().count()).sum::<usize>() + line.chars().count();
            if self.antecedent_lines.len() >= MAX_ANTECEDENT_LINES || budget > MAX_ANTECEDENT_CHARS {
                self.antecedents_omitted += 1;
                continue;
            }
            self.antecedent_lines.push(line);
        }
    }

    /// This group's `(task, context)` — the trajectory fields
    /// `canon_learn::distill_trajectory` turns into a strategy's TITLE
    /// and CONTENT (`s38-evidence-bearing-memory`):
    ///
    /// - `task` names the concrete artifact plus its deduped kind
    ///   labels (`platformer.hud.01: review promotion`), so a retrieved
    ///   `avoid: …` guardrail says what to avoid on WHAT — followed,
    ///   when s39's [`ScenarioTitleIndex`] joined one, by the
    ///   scenario's own `title` after an em dash
    ///   (`platformer.moving.01: review attestation — A moving platform
    ///   carries the standing player`). The id stays first and stays
    ///   verbatim: it is the join key a reader needs to find the
    ///   artifact, and the sentence after it is what makes the id mean
    ///   something. With no joined title the string is byte-identical
    ///   to s38's, so the enrichment is purely additive.
    /// - `context` is the deduped evidence lines, newline-joined — real
    ///   reviewer/divergence/task prose, never a `detail` blob (see
    ///   [`ArtifactEvent::evidence_line`]) — followed, when s39's
    ///   antecedent join found any, by a `preceded on this artifact by:`
    ///   heading and one bullet-indented line per finding. The heading
    ///   plus the indent are what make a finding tellable from the
    ///   outcome that closed it: both are one line of the same kind of
    ///   prose, and an undifferentiated list reads as N independent
    ///   outcomes rather than one outcome and its antecedents.
    ///
    /// Both are pure functions of the accumulated, first-seen-ordered
    /// input, and `canon_ingest::scanner::scan_dir` hands this driver a
    /// byte-lexically deterministic event order — so two passes over an
    /// unchanged corpus produce byte-identical strings and
    /// [`trajectory_content_digest`]'s duplicate skip still fires.
    fn trajectory_text(&self, regime_key: &RegimeKey) -> (String, String) {
        let subject = self.join_key.as_deref().unwrap_or_else(|| regime_key.as_str());
        let labelled = if self.kind_labels.is_empty() { subject.to_string() } else { format!("{subject}: {}", self.kind_labels.join(", ")) };
        // One line, plain text: this lands in a distilled strategy's
        // title, which `canon retrieve` injects into a dispatched
        // agent's context — no markup to render, nothing to parse.
        let task = match &self.scenario_title {
            Some(title) => format!("{labelled} — {title}"),
            None => labelled,
        };
        let mut context = self.evidence_lines.join("\n");
        if !self.antecedent_lines.is_empty() {
            if !context.is_empty() {
                context.push('\n');
            }
            context.push_str("preceded on this artifact by:");
            for line in &self.antecedent_lines {
                context.push_str("\n  - ");
                context.push_str(line);
            }
            if self.antecedents_omitted > 0 {
                context.push_str(&format!("\n  (+{} more omitted at the antecedent cap)", self.antecedents_omitted));
            }
        }
        (task, context)
    }
}

/// Appends `item` only if absent, preserving FIRST-SEEN order
/// (`s38-evidence-bearing-memory`). A `BTreeSet` would dedupe too, but
/// would re-sort the labels/lines alphabetically — and the order a
/// regime's evidence was observed in is the order that reads as a
/// narrative. Linear `contains` is right at this size: a regime group
/// holds a handful of events, and the allocation a hash set would cost
/// exceeds the scan it saves.
fn push_first_seen<T: PartialEq>(seen: &mut Vec<T>, item: T) {
    if !seen.contains(&item) {
        seen.push(item);
    }
}

/// The most antecedent findings one regime's rendered `context` block
/// carries (s39 `joined-evidence-grounding`).
///
/// Four is sized off the corpus this join exists for: canon's own
/// `.canon/ledger/kind=divergence` records carry at most two `open`
/// findings on a scenario before the `resolved` record that closed
/// them, so four leaves headroom for an artifact that took two review
/// rounds while keeping the block scannable at a glance. The bound
/// matters because `canon retrieve` injects SEVERAL strategies into one
/// dispatched agent's context at once — unbounded, a single
/// heavily-reviewed artifact would crowd out every other strategy
/// retrieved for that role.
const MAX_ANTECEDENT_LINES: usize = 4;

/// The char budget that SAME block gets, enforced independently of
/// [`MAX_ANTECEDENT_LINES`] (s39 `joined-evidence-grounding`).
///
/// A count cap alone does not bound length. `ArtifactEvent::evidence_line`
/// caps each line at 512 chars, so four of them is ~2 KiB — several
/// times a typical trajectory's entire content. 1536 (three full-cap
/// lines) passes the real corpus untouched, whose findings run 157–485
/// chars so four of them fit comfortably, while bounding a pathological
/// record — a pasted stack trace, a diff — to roughly three lines'
/// worth. Dropping is whole-line by construction: half a finding is
/// worse than none, because its locator and its remedy sit at opposite
/// ends of the sentence.
const MAX_ANTECEDENT_CHARS: usize = 1536;

/// One prose-bearing NON-verdict event reduced to exactly what the
/// antecedent join reads (s39 `joined-evidence-grounding`): its
/// timestamp, to decide whether it PRECEDES a given verdict, and its
/// already-rendered [`ArtifactEvent::evidence_line`].
struct AntecedentEvent {
    at: DateTime<Utc>,
    line: String,
}

/// Prose-bearing non-verdict events bucketed by [`join_key_identity`],
/// each bucket totally ordered by `(at, line)` and deduped by line —
/// [`index_antecedents`]'s output and [`antecedents_before`]'s input
/// (s39 `joined-evidence-grounding`).
///
/// Keyed by the SOURCE-KIND-TAGGED identity rather than the bare
/// [`ArtifactJoinKey::as_str`], so a `task:x` finding can never attach
/// itself to a `scenario:x` verdict. It is the same string
/// [`regime_hash`] digests, which makes "shares my join key" and "would
/// have folded onto my regime" one question instead of two that can
/// disagree. `BTreeMap` (not a hash map) because a bucket's contents
/// reach rendered, agent-facing text.
type AntecedentIndex = BTreeMap<String, Vec<AntecedentEvent>>;

/// Indexes the events [`run`]'s accumulation loop is about to DROP but
/// which still carry real evidence (s39 `joined-evidence-grounding`) —
/// extracted from `run` for the same reason [`group_by_regime`] was:
/// the join it drives is then unit-testable without a live store.
///
/// Two filters, both required:
///
/// - **No verdict.** An event that derives one becomes a trajectory
///   line in its own right; indexing it too would print it twice, once
///   as the outcome and once as its own antecedent.
/// - **Real prose**, via [`ArtifactEvent::has_salient_prose`]. s38's
///   `evidence_line` degrades to the bare `display_label` when a record
///   carries no narrative, and to a `status <token>` marker one step
///   before that; neither is evidence, both are canon's own vocabulary,
///   and padding every retrieved strategy with it is the exact
///   regression `s38-evidence-bearing-memory` removed. The predicate
///   lives beside `salient_prose` in `canon-ingest` so this filter
///   cannot drift from the field-priority list it depends on — and so
///   it is not approximated by `evidence_line() != display_label()`,
///   which silently passes the status-token line.
///
/// Each bucket is then sorted by `(at, line)` — a TOTAL, data-derived
/// order, never the adapter scan order — and deduped by line text
/// keeping the EARLIEST occurrence. Earliest is not a coin flip: it is
/// the only choice that keeps [`antecedents_before`]'s cutoff honest,
/// since if the earliest copy of a line is too late to attach, every
/// copy is.
fn index_antecedents(events: &[ArtifactEvent]) -> AntecedentIndex {
    let mut index = AntecedentIndex::new();
    for event in events {
        if derive_verdict_for_event(event).is_some() || !event.has_salient_prose() {
            continue;
        }
        index.entry(join_key_identity(&event.join_key)).or_default().push(AntecedentEvent { at: event.at, line: event.evidence_line() });
    }
    for bucket in index.values_mut() {
        bucket.sort_by(|a, b| (a.at, &a.line).cmp(&(b.at, &b.line)));
        let mut deduped: Vec<AntecedentEvent> = Vec::with_capacity(bucket.len());
        for candidate in std::mem::take(bucket) {
            if !deduped.iter().any(|kept| kept.line == candidate.line) {
                deduped.push(candidate);
            }
        }
        *bucket = deduped;
    }
    index
}

/// The antecedents one verdict-bearing event absorbs (s39
/// `joined-evidence-grounding`): the indexed events on its OWN join key
/// whose timestamp is at or before its own.
///
/// The cutoff is what makes this a JOIN rather than a bag. A finding
/// filed AFTER a resolution is a different, still-open problem that the
/// resolution demonstrably did not fix, so attaching it would distill a
/// claim the corpus does not support. Ties are inclusive on purpose: an
/// artifact whose finding and remediation were recorded in one batch
/// carries one timestamp for both, and excluding equality there would
/// lose exactly the pairing this join exists for.
///
/// The bucket is already `(at, line)`-ordered ascending, so the eligible
/// set is a prefix and a `take_while` suffices — no re-sort, and no
/// allocation at all for a key that has no antecedents.
fn antecedents_before(index: &AntecedentIndex, identity: &str, at: DateTime<Utc>) -> Vec<String> {
    let Some(bucket) = index.get(identity) else { return Vec::new() };
    bucket.iter().take_while(|antecedent| antecedent.at <= at).map(|antecedent| antecedent.line.clone()).collect()
}

/// Groups every derived verdict onto its `regime_key`
/// (`s38-evidence-bearing-memory`) — extracted from [`run`] so the
/// evidence accumulation this drives is unit-testable without a live
/// store. `BTreeMap` (not a hash map) keeps the persist loop's regime
/// order deterministic, as it was before.
fn group_by_regime(derived: Vec<DerivedVerdict>) -> BTreeMap<RegimeKey, RegimeEvidence> {
    let mut by_regime: BTreeMap<RegimeKey, RegimeEvidence> = BTreeMap::new();
    for verdict in derived {
        // `entry` needs an owned key while `absorb` consumes the rest
        // of the struct, so the key is cloned once per verdict — a
        // short `String`, and the alternative is a five-positional-
        // argument `absorb` whose call site invites exactly the
        // parameter mix-up `DerivedVerdict` exists to remove.
        by_regime.entry(verdict.regime_key.clone()).or_default().absorb(verdict);
    }
    by_regime
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

    // s39 (`joined-evidence-grounding`): the scenario-title join reads
    // `RecordKind::Scenario` too, so its own rung must be attached by
    // the SAME up-front build -- no adapter maps to that kind, so
    // without this the read would ask a `TierRegistry` that never
    // attempted the rung. Widening the kind set cannot change any
    // adapter's reported status: `unavailable_reasons` is keyed by RUNG,
    // and a rung some adapter's kind also routes to was already being
    // attempted for that adapter's sake. An UNROUTED `scenario` kind
    // contributes no rungs and is not an error here
    // (`build_lenient_tiers_for_kinds`' own contract).
    let tier_kinds: Vec<RecordKind> = records_kinds.iter().copied().chain([RecordKind::Scenario]).collect();

    let (store, policy_for_reason, unavailable_reasons, missing_config_reason): (Option<TierRegistry>, Option<TierPolicy>, BTreeMap<Rung, String>, Option<String>) =
        match tiers::build_lenient_tiers_for_kinds(&canon_yaml_path, &tier_kinds) {
            Ok(loaded) => {
                let policy = loaded.policy.clone();
                let reasons = loaded.unavailable_reasons.clone();
                (Some(TierRegistry::new(loaded.policy, loaded.git, loaded.pg, loaded.r2, loaded.sqlite)), Some(policy), reasons, None)
            }
            Err(TierCliError::ReadCanonYaml { path, source }) => (None, None, BTreeMap::new(), Some(format!("reading `{path}`: {source}"))),
            Err(other) => return Err(other.into()),
        };

    // s39: ONE read of the `Scenario` ledger index for the whole pass —
    // never one per event — through the exact read path every
    // `Records`-source adapter above uses ([`read_kind_records`]).
    //
    // A missing/unreadable `canon.yaml`, an unrouted `scenario` kind, or
    // an unreachable routed rung degrades to an EMPTY index and a
    // NORMAL run: this join only ever enriches text a trajectory would
    // have carried anyway, so its absence cannot add, drop, or alter a
    // single verdict or trajectory. That is what makes it different from
    // an adapter read failure, which loses evidence outright and
    // therefore MUST surface as `status: "unavailable"` in
    // `ArtifactIngestOutcome::adapters` (module doc's "documented
    // seam") — there is no adapter here to report, and inventing one
    // would misreport a read no registry entry owns. A genuinely
    // MALFORMED `canon.yaml` still fails the whole command loud, above,
    // before this line is ever reached.
    let scenario_titles = read_kind_records(RecordKind::Scenario, store.as_ref(), policy_for_reason.as_ref(), &unavailable_reasons, missing_config_reason.as_deref())
        .map(|records| ScenarioTitleIndex::from_records(&records))
        .unwrap_or_default();

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
    // Built BEFORE the accumulation loop over the SAME `all_events`:
    // that loop's `else { continue }` is precisely where a prose-bearing
    // non-verdict event would otherwise be dropped unread (s39
    // `joined-evidence-grounding`, module doc).
    let antecedent_index = index_antecedents(&all_events);
    let mut derived: Vec<DerivedVerdict> = Vec::new();
    for event in &all_events {
        let Some(row) = derive_verdict_for_event(event) else { continue };
        let area = event.area.clone().unwrap_or_else(|| "unscoped".to_string());
        let hash = regime_hash(&event.join_key);
        let verdict = attach_regime_key(row, event.join_key.clone(), &label, &area, &hash, event.trust_level.clone())?;
        derived.push(DerivedVerdict {
            regime_key: verdict.regime_key,
            row: verdict.row,
            at: event.at,
            join_key: event.join_key.as_str().to_string(),
            // s39: resolved HERE, the one place holding both the
            // once-read index and the event's own `detail` `project_id`.
            scenario_title: scenario_titles.title_for(&event.join_key, event_project_id(event)).map(str::to_string),
            kind_label: event.display_label(),
            evidence_line: event.evidence_line(),
            antecedents: antecedents_before(&antecedent_index, &join_key_identity(&event.join_key), event.at),
        });
    }
    let verdicts_derived = derived.len();
    let by_regime = group_by_regime(derived);

    let learn_root = repo.join(&learn_config.root);
    let trajectory_store = ParquetTrajectoryStore::open(learn_root.join("trajectories"));
    let strategy_store = ParquetStrategyStore::open(learn_root.join("strategies"));
    let role_registry = RoleRegistry::from_config(&learn_config);
    // S7's per-role reward table (`dev`'s ported weighted composite plus
    // the five provisional roles); an unregistered role falls back to
    // `default_reward_fn` rather than erroring, so every trajectory
    // always has SOME reward function.
    let reward_registry = RewardRegistry::builtin();

    let mut trajectories_persisted = Vec::new();
    let mut trajectories_skipped_unregistered_role = 0usize;
    let mut trajectories_skipped_duplicate = 0usize;
    let mut strategy_items_rebuilt = 0usize;
    let mut trajectories_marked = 0usize;
    let mut trajectories_left_pending = 0usize;
    for (regime_key, evidence) in by_regime {
        let verdict_count = evidence.rows.len();
        let digest = trajectory_content_digest(&regime_key, &evidence.rows);
        let already_persisted = trajectory_store
            .query_by_regime_key(&regime_key)?
            .iter()
            .any(|existing| trajectory_content_digest(&existing.regime_key, &existing.verdicts) == digest);
        if already_persisted {
            trajectories_skipped_duplicate += 1;
            continue;
        }
        let (task, context) = evidence.trajectory_text(&regime_key);
        let at = evidence.latest_at;
        let trajectory =
            Trajectory::new(TrajectoryId::new(), regime_key.clone(), task, context, evidence.rows, at, vec!["artifact-ingest".to_string()])?;

        match store_trajectory(&role_registry, &trajectory_store, &trajectory) {
            Ok(()) => {
                trajectories_persisted.push(PersistedTrajectory { regime_key: regime_key.as_str().to_string(), verdict_count });
                // Resolve the covering verdict+reward from the SAME
                // `VerdictRow`(s) this trajectory was just built from,
                // then write it back — closing S7 design D2's two-phase
                // reward-write model at the one call site that has both
                // halves in hand. `compute_for_trajectory` is documented
                // as "the shape `mark_trajectory_verdict`'s caller
                // typically wants" (`reward.rs`); before this wiring
                // existed every artifact-derived trajectory stayed
                // `Pending` forever, so no promotion gate could ever see
                // a corroborating sample. The role is known-registered
                // here — `store_trajectory` above already rejected an
                // unregistered one — so the `?` is a genuine
                // store/parse failure, not a routine miss.
                match reward_registry.compute_for_trajectory(&trajectory)? {
                    // `Pending` is the unset default; marking it is
                    // rejected by design. Leave the trajectory pending
                    // for a later covering signal (the S7 webhook
                    // receiver's PR/CI path) and count it.
                    (VerdictOutcome::Pending, _) => trajectories_left_pending += 1,
                    (outcome, reward) => {
                        // Marked BEFORE `rebuild_namespace` deliberately:
                        // `mark_verdict` is the ONLY path allowed to
                        // rewrite a stored trajectory, and
                        // `rebuild_namespace` must leave those bytes
                        // untouched (asserted by
                        // `parquet_trajectory`'s own round-trip test).
                        mark_trajectory_verdict(&trajectory_store, &trajectory.id, outcome, reward)?;
                        trajectories_marked += 1;
                    }
                }
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
        trajectories_marked,
        trajectories_left_pending,
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
    out.push_str(&format!("trajectories marked (covering verdict resolved): {}\n", outcome.trajectories_marked));
    out.push_str(&format!("trajectories left pending (awaiting a covering signal): {}\n", outcome.trajectories_left_pending));
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
    use canon_ingest::artifact_adapter::ArtifactEventKind as Kind;

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

    /// One `DerivedVerdict` built the way [`run`] builds it — through a
    /// real `ArtifactEvent`, so these tests exercise the actual
    /// `join_key`/`display_label`/`evidence_line` accessors rather than
    /// hand-written strings.
    fn derived_with(adapter_id: &'static str, regime: &RegimeKey, join_key: &str, kind: Kind, detail: serde_json::Value, at: &str) -> DerivedVerdict {
        let event = ArtifactEvent {
            adapter_id,
            join_key: ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse(join_key.to_string()).unwrap()),
            kind,
            authoring_role: Some(canon_model::ids::RoleId::parse("dev").unwrap()),
            area: Some("world".to_string()),
            trust_level: None,
            at: at.parse().unwrap(),
            detail,
        };
        DerivedVerdict {
            regime_key: regime.clone(),
            row: verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate),
            at: event.at,
            join_key: event.join_key.as_str().to_string(),
            kind_label: event.display_label(),
            evidence_line: event.evidence_line(),
            scenario_title: None,
            antecedents: Vec::new(),
        }
    }

    fn derived(regime: &RegimeKey, join_key: &str, kind: Kind, detail: serde_json::Value, at: &str) -> DerivedVerdict {
        derived_with("ledger", regime, join_key, kind, detail, at)
    }

    fn finding(text: &str) -> serde_json::Value {
        serde_json::json!({"detail": text})
    }

    #[test]
    fn a_regime_groups_every_verdict_and_keeps_the_newest_timestamp() {
        let key = regime("world", "abc123");
        let scenario = "world.firstbuy-hotdeal.14";
        let group = group_by_regime(vec![
            derived(&key, scenario, Kind::CodeReviewFinding, finding("reveal reads the client cart"), "2026-07-08T20:05:49Z"),
            derived(&key, scenario, Kind::RemediationResolved, finding("re-read the grant from the result"), "2026-07-09T10:15:00Z"),
        ]);
        let evidence = group.get(&key).expect("both verdicts fold onto the one regime");
        assert_eq!(evidence.rows.len(), 2);
        assert_eq!(evidence.latest_at, "2026-07-09T10:15:00Z".parse::<DateTime<Utc>>().unwrap(), "recorded_at is the newest source record");
    }

    #[test]
    fn trajectory_text_names_the_artifact_with_deduped_labels_and_evidence_in_first_seen_order() {
        // The whole point of s38-evidence-bearing-memory: the TITLE
        // names a real scenario and what happened to it, and the CONTENT
        // quotes the reviewer's actual findings — no describing the
        // ingest driver, no `detail` blob, no alphabetical re-sorting of
        // a narrative.
        let key = regime("world", "abc123");
        let scenario = "world.firstbuy-hotdeal.14";
        let group = group_by_regime(vec![
            derived(&key, scenario, Kind::CodeReviewFinding, finding("reveal reads the client cart"), "2026-07-08T20:05:49Z"),
            // Same kind AND same prose as the first: contributes neither
            // a duplicate label nor a duplicate line.
            derived(&key, scenario, Kind::CodeReviewFinding, finding("reveal reads the client cart"), "2026-07-08T21:00:00Z"),
            derived(&key, scenario, Kind::RemediationResolved, finding("re-read the grant from the result"), "2026-07-09T10:15:00Z"),
        ]);
        let (task, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(task, "world.firstbuy-hotdeal.14: code-review finding, remediation resolved");
        assert_eq!(context, "code-review finding: reveal reads the client cart\nremediation resolved: re-read the grant from the result");
        assert!(
            !context.contains("canon ingest artifacts") && !context.contains("VerdictRow"),
            "a strategy's content never describes the ingest driver: {context}"
        );
    }

    #[test]
    fn trajectory_text_is_byte_identical_across_two_passes_over_the_same_events() {
        // `trajectory_content_digest`'s duplicate skip only fires if the
        // text a second ingest pass derives matches the first exactly.
        let key = regime("world", "abc123");
        let scenario = "world.firstbuy-hotdeal.14";
        let events = || {
            vec![
                derived(&key, scenario, Kind::CodeReviewFinding, finding("reveal reads the client cart"), "2026-07-08T20:05:49Z"),
                derived(&key, scenario, Kind::ClearAfterFlagged, serde_json::json!({"pin": "9c93d024b"}), "2026-07-09T09:00:00Z"),
            ]
        };
        let first = group_by_regime(events());
        let second = group_by_regime(events());
        assert_eq!(first.get(&key).unwrap().trajectory_text(&key), second.get(&key).unwrap().trajectory_text(&key));
    }

    #[test]
    fn trajectory_text_degrades_to_the_regime_key_rather_than_printing_a_wrong_artifact_id() {
        // Unreachable by construction (`regime_hash` IS the join key's
        // digest, and `absorb` debug-asserts it), so this exercises the
        // release-build degrade directly: a group that somehow spanned
        // two keys names the regime, never one arbitrary member's id.
        let key = regime("world", "abc123");
        let mut evidence = RegimeEvidence::default();
        evidence.absorb(derived(&key, "world.firstbuy-hotdeal.14", Kind::CodeReviewFinding, finding("a finding"), "2026-07-08T20:05:49Z"));
        evidence.join_key = None;
        let (task, _) = evidence.trajectory_text(&key);
        assert_eq!(task, format!("{}: code-review finding", key.as_str()));
    }

    #[test]
    fn push_first_seen_dedupes_without_re_sorting() {
        let mut seen = Vec::new();
        for item in ["remediation resolved", "code-review finding", "remediation resolved"] {
            push_first_seen(&mut seen, item);
        }
        assert_eq!(seen, vec!["remediation resolved", "code-review finding"]);
    }

    #[test]
    fn a_native_verdict_regime_titles_itself_by_the_record_never_non_verdict() {
        // The shape `canon retrieve` returns for a repo configured
        // `artifacts.native_records: true` (this one): the native
        // adapters set `kind = NonVerdict` — their verdicts come from the
        // native derivation path — so labelling off `kind` alone would
        // title every strategy here "non-verdict".
        let key = regime("platformer", "41fdd8c5");
        let native = |status: &str, prose: &str, at: &str| {
            let detail = serde_json::json!({"native_kind": "divergence", "status": status, "detail": prose});
            derived_with("divergence-native", &key, "platformer.session.04", Kind::NonVerdict, detail, at)
        };
        let group = group_by_regime(vec![
            native("still_divergent", "SHIP-BLOCKER App.tsx:45 calls sim.setPaused inside a React updater", "2026-07-14T20:50:34Z"),
            native("resolved", "Fixed and re-verified at 505a668e", "2026-07-14T21:38:36Z"),
        ]);
        let (task, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(task, "platformer.session.04: still-divergent divergence, resolved divergence");
        assert!(!task.contains("non-verdict"), "a record that DID score must not be titled non-verdict: {task}");
        assert_eq!(
            context,
            "still-divergent divergence: SHIP-BLOCKER App.tsx:45 calls sim.setPaused inside a React updater\nresolved divergence: Fixed and re-verified at 505a668e"
        );
    }

    /// The two real `SHIP-BLOCKER` findings canon's own divergence
    /// corpus carries on `platformer.session.04`, and the resolution
    /// record that closed them — the exact shape s39
    /// (`joined-evidence-grounding`) exists to distill as ONE strategy.
    const APP_FINDING: &str = "SHIP-BLOCKER examples/platformer/src/App.tsx:45-46 calls sim.setPaused() inside a React updater";
    const SIM_FINDING: &str = "SHIP-BLOCKER examples/platformer/src/engine/simulation.ts:151 clears jumpBuffered after consuming it";
    const RESOLUTION: &str = "Fixed and independently re-verified+attested at 505a668e";

    /// One `ArtifactEvent` shaped the way the `divergence-native`
    /// adapter emits them. `status` drives BOTH the display label and
    /// whether a verdict is derived at all (`open` derives none), which
    /// is the exact split the antecedent join keys off; `prose: None`
    /// produces the label-only event that must never be absorbed.
    fn native_divergence(join_key: &str, status: &str, prose: Option<&str>, at: &str) -> ArtifactEvent {
        let mut detail = serde_json::json!({"native_kind": "divergence", "status": status});
        if let Some(text) = prose {
            detail["detail"] = serde_json::json!(text);
        }
        ArtifactEvent {
            adapter_id: "divergence-native",
            join_key: ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse(join_key.to_string()).unwrap()),
            kind: Kind::NonVerdict,
            authoring_role: Some(canon_model::ids::RoleId::parse("dev").unwrap()),
            area: Some("platformer".to_string()),
            trust_level: None,
            at: at.parse().unwrap(),
            detail,
        }
    }

    /// [`run`]'s own per-event accumulation step minus the live store —
    /// the same `join_key`/`display_label`/`evidence_line`/antecedent
    /// wiring, so these tests exercise the shipped join rather than
    /// hand-written strings.
    fn derived_from(index: &AntecedentIndex, regime: &RegimeKey, event: &ArtifactEvent) -> DerivedVerdict {
        DerivedVerdict {
            regime_key: regime.clone(),
            row: verdict_row("dev", canon_ingest::verdict::Polarity::Success, canon_ingest::verdict::Becomes::StrategyCandidate),
            at: event.at,
            join_key: event.join_key.as_str().to_string(),
            scenario_title: None,
            kind_label: event.display_label(),
            evidence_line: event.evidence_line(),
            antecedents: antecedents_before(index, &join_key_identity(&event.join_key), event.at),
        }
    }

    #[test]
    fn a_resolved_divergence_distills_together_with_the_open_findings_it_closed() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let events = vec![
            native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T20:50:34Z"),
            native_divergence(scenario, "open", Some(SIM_FINDING), "2026-07-14T20:51:02Z"),
            native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"),
        ];
        // s39 changes nothing about verdict derivation: the two findings
        // still score no verdict of their own, exactly as
        // `derive_native_divergence_verdict` maps `Open => None`.
        assert!(derive_verdict_for_event(&events[0]).is_none() && derive_verdict_for_event(&events[1]).is_none());

        let index = index_antecedents(&events);
        let group = group_by_regime(vec![derived_from(&index, &key, &events[2])]);
        let (_, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(
            context,
            format!("resolved divergence: {RESOLUTION}\npreceded on this artifact by:\n  - open divergence: {APP_FINDING}\n  - open divergence: {SIM_FINDING}"),
            "the outcome AND both findings it closed, the findings visibly marked as antecedents"
        );
    }

    #[test]
    fn a_finding_on_a_different_artifact_is_never_absorbed() {
        // The join key is the whole join. A concurrent finding on a
        // sibling scenario is somebody else's problem, and quoting it
        // here would attribute it to this resolution.
        let key = regime("platformer", "41fdd8c5");
        let events = vec![
            native_divergence("platformer.session.05", "open", Some(APP_FINDING), "2026-07-14T20:50:34Z"),
            native_divergence("platformer.session.04", "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"),
        ];
        let index = index_antecedents(&events);
        let group = group_by_regime(vec![derived_from(&index, &key, &events[1])]);
        let (_, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(context, format!("resolved divergence: {RESOLUTION}"));
        assert!(!context.contains("preceded"), "no heading at all when nothing attached: {context}");
    }

    #[test]
    fn a_finding_filed_after_the_resolution_is_not_absorbed_by_it() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let resolution = native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z");
        let later = native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-15T09:00:00Z");
        let index = index_antecedents(&[resolution.clone(), later]);
        let group = group_by_regime(vec![derived_from(&index, &key, &resolution)]);
        let (_, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(context, format!("resolved divergence: {RESOLUTION}"), "a later finding is a still-open problem this resolution did not fix");

        // The boundary is INCLUSIVE: a finding and its remediation
        // recorded in one batch share a timestamp, and that pairing is
        // exactly what this join exists for.
        let same_instant = native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T21:38:36Z");
        let index = index_antecedents(&[resolution.clone(), same_instant]);
        let group = group_by_regime(vec![derived_from(&index, &key, &resolution)]);
        let (_, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert!(context.contains(APP_FINDING), "an equal timestamp still attaches: {context}");
    }

    #[test]
    fn a_prose_free_non_verdict_event_contributes_no_antecedent() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let bare = native_divergence(scenario, "open", None, "2026-07-14T20:50:34Z");
        assert_eq!(bare.evidence_line(), "open divergence", "the label-only shape the prose filter exists for");

        let events = vec![bare, native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z")];
        let index = index_antecedents(&events);
        assert!(index.is_empty(), "a bare kind label is canon's own vocabulary, not evidence — it is never indexed");
        let group = group_by_regime(vec![derived_from(&index, &key, &events[1])]);
        let (_, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(context, format!("resolved divergence: {RESOLUTION}"));
    }

    #[test]
    fn a_finding_repeated_across_records_contributes_one_antecedent_line() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let events = vec![
            native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T20:50:34Z"),
            // The same finding re-filed in a later review round.
            native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T21:00:00Z"),
            native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"),
        ];
        let index = index_antecedents(&events);
        assert_eq!(index[&format!("scenario:{scenario}")].len(), 1, "deduped by line text at index time, earliest copy kept");
        let grouped = group_by_regime(vec![derived_from(&index, &key, &events[2])]);
        let evidence = grouped.get(&key).unwrap();
        assert_eq!(evidence.antecedent_lines.len(), 1);
        assert_eq!(evidence.antecedents_omitted, 0, "a dedupe is not a cap drop and must never be reported as one");
    }

    #[test]
    fn the_antecedent_block_is_capped_by_line_count_and_says_how_many_it_dropped() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let mut events: Vec<ArtifactEvent> =
            (0..7).map(|n| native_divergence(scenario, "open", Some(&format!("SHIP-BLOCKER finding {n}")), &format!("2026-07-14T20:5{n}:00Z"))).collect();
        events.push(native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"));

        let index = index_antecedents(&events);
        assert_eq!(index[&format!("scenario:{scenario}")].len(), 7, "all seven are indexed — the cap is an absorb-time bound on the RENDERED block");
        let grouped = group_by_regime(vec![derived_from(&index, &key, events.last().unwrap())]);
        let evidence = grouped.get(&key).unwrap();
        assert_eq!(evidence.antecedent_lines.len(), MAX_ANTECEDENT_LINES);
        assert_eq!(evidence.antecedents_omitted, 3);

        let (_, context) = evidence.trajectory_text(&key);
        assert!(context.ends_with("\n  (+3 more omitted at the antecedent cap)"), "a capped block says so rather than silently quoting four of seven: {context}");
        assert!(context.contains("finding 3") && !context.contains("finding 4"), "the four EARLIEST survive, in `(at, line)` order: {context}");
    }

    #[test]
    fn the_antecedent_char_budget_binds_before_the_line_cap_does() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        // Each renders as `"open divergence: "` plus s38's own 512-char
        // per-line cap, so three of them (~1590 chars) exceed
        // MAX_ANTECEDENT_CHARS while staying under MAX_ANTECEDENT_LINES.
        let mut events: Vec<ArtifactEvent> =
            (0..3).map(|n| native_divergence(scenario, "open", Some(&format!("{n}{}", "x".repeat(600))), &format!("2026-07-14T20:5{n}:00Z"))).collect();
        events.push(native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"));

        let index = index_antecedents(&events);
        let grouped = group_by_regime(vec![derived_from(&index, &key, events.last().unwrap())]);
        let evidence = grouped.get(&key).unwrap();
        assert_eq!(evidence.antecedent_lines.len(), 2, "two full-cap lines fit the budget, the third does not");
        assert!(evidence.antecedent_lines.len() < MAX_ANTECEDENT_LINES, "the CHAR budget bound this block, not the line count");
        assert_eq!(evidence.antecedents_omitted, 1);
        assert!(evidence.antecedent_lines.iter().map(|line| line.chars().count()).sum::<usize>() <= MAX_ANTECEDENT_CHARS);
        assert!(
            evidence.antecedent_lines.iter().all(|line| line.starts_with("open divergence: ")),
            "dropping is whole-line: the block budget never cuts a finding in half"
        );
    }

    #[test]
    fn trajectory_text_is_byte_identical_across_two_passes_with_antecedents_present() {
        // s38's idempotence guarantee has to survive the s39 join:
        // `trajectory_content_digest` folds only the verdict ROWS, so a
        // trajectory whose text wobbled between passes would be skipped
        // as a duplicate while carrying different content — silent drift
        // into `.canon/learn`.
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let pass = || {
            // Deliberately handed over in an order that is NOT time
            // order, so a scan-order-dependent render shows up here.
            let events = vec![
                native_divergence(scenario, "open", Some(SIM_FINDING), "2026-07-14T20:51:02Z"),
                native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T20:50:34Z"),
                native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"),
            ];
            let index = index_antecedents(&events);
            let grouped = group_by_regime(vec![derived_from(&index, &key, &events[2])]);
            grouped.get(&key).unwrap().trajectory_text(&key)
        };
        let first = pass();
        assert_eq!(first, pass(), "two passes over the same events must derive byte-identical task/context");

        let (_, context) = first;
        assert!(context.contains(APP_FINDING) && context.contains(SIM_FINDING), "both antecedents are present: {context}");
        assert!(
            context.find(APP_FINDING) < context.find(SIM_FINDING),
            "antecedents render in `(at, line)` order, never the adapter scan order they arrived in: {context}"
        );
    }

    // ── s39 `joined-evidence-grounding`: the scenario-title join ──

    /// One `Scenario` ledger record shaped exactly like the ones in this
    /// repo's `.canon/ledger/kind=scenario` — a bare `RawRecord`, which
    /// is what the tier read hands over.
    fn scenario_record(project_id: &str, scenario_id: &str, title: &str, at: &str) -> RawRecord {
        RawRecord(serde_json::json!({
            "actor": {"agent_id": "canon-inventory-sync"},
            "at": at,
            "kind": "scenario",
            "project_id": project_id,
            "scenario_id": scenario_id,
            "schema": 1,
            "title": title,
        }))
    }

    /// The one real record this repo's own corpus carries for
    /// `platformer.moving.01`, verbatim.
    fn moving_platform_index() -> ScenarioTitleIndex {
        ScenarioTitleIndex::from_records(&[scenario_record(
            "platformer",
            "platformer.moving.01",
            "A moving platform carries the standing player",
            "2026-07-14T19:27:35.016340Z",
        )])
    }

    fn scenario_key(id: &str) -> ArtifactJoinKey {
        ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse(id.to_string()).unwrap())
    }

    /// [`derived_from`] plus [`run`]'s own title resolution — the SAME
    /// [`ScenarioTitleIndex::title_for`] + [`event_project_id`] pair over
    /// a real [`ArtifactEvent`], so these tests exercise the shipped join
    /// rather than a hand-set field. `project_id` is written into
    /// `detail` exactly as the emitting adapters write it; omitting it is
    /// the shape that drives `title_for`'s single-project fallback. The
    /// antecedent index is empty here on purpose: this is the `task` half
    /// of s39, and the `context` half has its own tests above.
    fn derived_joined(
        titles: &ScenarioTitleIndex,
        adapter_id: &'static str,
        regime: &RegimeKey,
        join_key: ArtifactJoinKey,
        project_id: Option<&str>,
        kind: Kind,
        at: &str,
    ) -> DerivedVerdict {
        let mut detail = finding("reveal reads the client cart");
        if let Some(project) = project_id {
            detail["project_id"] = serde_json::Value::String(project.to_string());
        }
        let event = ArtifactEvent {
            adapter_id,
            join_key,
            kind,
            authoring_role: Some(canon_model::ids::RoleId::parse("dev").unwrap()),
            area: Some("platformer".to_string()),
            trust_level: None,
            at: at.parse().unwrap(),
            detail,
        };
        DerivedVerdict {
            scenario_title: titles.title_for(&event.join_key, event_project_id(&event)).map(str::to_string),
            ..derived_from(&AntecedentIndex::new(), regime, &event)
        }
    }

    /// The `task` half of the text a group of exactly these verdicts
    /// renders.
    fn rendered_task(key: &RegimeKey, verdicts: Vec<DerivedVerdict>) -> String {
        group_by_regime(verdicts).get(key).expect("the group is keyed by the regime it was built with").trajectory_text(key).0
    }

    #[test]
    fn a_scenario_keyed_trajectory_names_its_joined_title() {
        // The exact shape s39 exists for: `records::Review` carries no
        // prose field at all, so pre-s39 a review-derived strategy was
        // titled by the scenario id and its kind label — nothing about
        // what was attested. The joined `Scenario.title` is the sentence
        // that fixes it, and the id stays FIRST because it is the join
        // key a reader follows back to the artifact.
        let key = regime("platformer", "41fdd8c5");
        let task = rendered_task(
            &key,
            vec![derived_joined(
                &moving_platform_index(),
                "review",
                &key,
                scenario_key("platformer.moving.01"),
                Some("platformer"),
                Kind::NonVerdict,
                "2026-07-14T21:38:36Z",
            )],
        );
        assert_eq!(task, "platformer.moving.01: review attestation — A moving platform carries the standing player");
    }

    #[test]
    fn an_unknown_scenario_id_renders_the_s38_task_byte_for_byte() {
        let key = regime("platformer", "41fdd8c5");
        let joined = derived_joined(
            &moving_platform_index(),
            "review",
            &key,
            scenario_key("platformer.hud.02"),
            Some("platformer"),
            Kind::NonVerdict,
            "2026-07-14T21:38:36Z",
        );
        assert_eq!(joined.scenario_title, None, "an id the index never saw joins nothing");
        assert_eq!(rendered_task(&key, vec![joined]), "platformer.hud.02: review attestation");
    }

    #[test]
    fn a_non_scenario_join_key_renders_the_s38_task_byte_for_byte() {
        // A handoff- or task-keyed trajectory has no scenario to name, so
        // the join must not merely MISS — it is never attempted, and the
        // rendered text stays indistinguishable from s38's.
        let key = regime("platformer", "41fdd8c5");
        let titles = moving_platform_index();
        let handoff = ArtifactJoinKey::Handoff(canon_model::ids::HandoffId::parse("20260710-1432-fix-a1b2").unwrap());
        let task_key = ArtifactJoinKey::Task(canon_model::ids::TaskId::parse("frozen-fixture-change#1.4").unwrap());
        assert_eq!(titles.title_for(&handoff, Some("platformer")), None);
        assert_eq!(titles.title_for(&task_key, None), None);

        let joined = derived_joined(&titles, "handoff", &key, handoff, Some("platformer"), Kind::PrMergeNoRevert, "2026-07-14T21:38:36Z");
        assert_eq!(rendered_task(&key, vec![joined]), "20260710-1432-fix-a1b2: PR merge with no revert");
    }

    #[test]
    fn an_empty_index_leaves_every_task_byte_identical_to_s38() {
        // The degrade path: a missing/unreadable `canon.yaml`, an
        // unrouted `scenario` kind, and an unreachable routed rung all
        // resolve to `ScenarioTitleIndex::default()` and a NORMAL run —
        // this join can only ever fail to ENRICH.
        let key = regime("platformer", "41fdd8c5");
        let empty = ScenarioTitleIndex::default();
        let joined = derived_joined(&empty, "review", &key, scenario_key("platformer.moving.01"), Some("platformer"), Kind::NonVerdict, "2026-07-14T21:38:36Z");
        assert_eq!(joined.scenario_title, None);
        assert_eq!(rendered_task(&key, vec![joined]), "platformer.moving.01: review attestation");
        assert_eq!(empty.title_for(&scenario_key("platformer.moving.01"), None), None, "and no project-less fallback invents one either");
    }

    #[test]
    fn a_blank_or_missing_title_contributes_no_entry() {
        // A partially-formed record is not evidence: enriching a strategy
        // title with an empty sentence (`<id>: <label> — `) is strictly
        // worse than leaving the s38 text alone.
        let index = ScenarioTitleIndex::from_records(&[
            scenario_record("platformer", "platformer.moving.01", "   \n\t ", "2026-07-14T19:27:35Z"),
            RawRecord(serde_json::json!({"at": "2026-07-14T19:27:35Z", "kind": "scenario", "project_id": "platformer", "scenario_id": "platformer.moving.02"})),
            RawRecord(serde_json::json!({"at": "2026-07-14T19:27:35Z", "kind": "scenario", "scenario_id": "platformer.moving.03", "title": "No project_id at all"})),
        ]);
        assert!(index.titles.is_empty(), "a blank title, an absent title, and an absent project_id each contribute nothing: {index:?}");
    }

    #[test]
    fn a_malformed_at_never_panics_and_never_outranks_a_well_formed_record() {
        // `canon_store::tier::raw_record_at` would PANIC on this record.
        // A hand-edited ledger file must cost a title lookup at most, so
        // the join reads `at` leniently and floors a malformed one.
        let mut broken = scenario_record("platformer", "platformer.moving.01", "Indexed from a record with a broken timestamp", "2026-07-01T09:00:00Z");
        broken.0["at"] = serde_json::Value::String("not-a-timestamp".to_string());
        let good = scenario_record("platformer", "platformer.moving.01", "A moving platform carries the standing player", "2026-07-14T19:27:35Z");

        let broken_only = ScenarioTitleIndex::from_records(&[broken.clone()]);
        assert_eq!(
            broken_only.title_for(&scenario_key("platformer.moving.01"), Some("platformer")),
            Some("Indexed from a record with a broken timestamp"),
            "a broken `at` is not a broken title — the record still indexes"
        );
        let both = ScenarioTitleIndex::from_records(&[broken, good]);
        assert_eq!(
            both.title_for(&scenario_key("platformer.moving.01"), Some("platformer")),
            Some("A moving platform carries the standing player"),
            "the floored timestamp sorts below every real one, so it can never win the slot"
        );
    }

    #[test]
    fn a_retitled_scenario_indexes_its_newest_title_regardless_of_read_order() {
        // The ledger names files `<project>__<scenario>__<digest>`, so a
        // RETITLED scenario lands beside its predecessor rather than
        // replacing it, and a `LiveDb`-class rung retains history
        // outright (s21 P3). Whichever order the tier yields them in, the
        // index must resolve the same current title — otherwise two
        // passes over an unchanged corpus could derive different `task`
        // strings and defeat the duplicate skip.
        let old = scenario_record("platformer", "platformer.moving.01", "An older sentence", "2026-07-01T09:00:00Z");
        let new = scenario_record("platformer", "platformer.moving.01", "A moving platform carries the standing player", "2026-07-14T19:27:35Z");
        let forward = ScenarioTitleIndex::from_records(&[old.clone(), new.clone()]);
        let reversed = ScenarioTitleIndex::from_records(&[new, old]);
        let key = scenario_key("platformer.moving.01");
        assert_eq!(forward.title_for(&key, Some("platformer")), Some("A moving platform carries the standing player"));
        assert_eq!(forward.title_for(&key, Some("platformer")), reversed.title_for(&key, Some("platformer")));
    }

    #[test]
    fn one_scenario_id_defined_by_two_projects_resolves_only_when_the_event_names_its_project() {
        // `ArtifactJoinKey::Scenario` carries the scenario id ALONE, so a
        // project-less event falls back to the id — honest only while
        // exactly one project defines it. With two, printing either
        // sentence would attribute one project's spec to another
        // project's strategy memory.
        let index = ScenarioTitleIndex::from_records(&[
            scenario_record("platformer", "platformer.moving.01", "A moving platform carries the standing player", "2026-07-14T19:27:35Z"),
            scenario_record("world", "platformer.moving.01", "A wholly different sentence", "2026-07-14T19:27:35Z"),
        ]);
        let key = scenario_key("platformer.moving.01");
        assert_eq!(index.title_for(&key, Some("world")), Some("A wholly different sentence"));
        assert_eq!(index.title_for(&key, None), None, "ambiguous without a project: name nothing rather than the wrong sentence");
        assert_eq!(index.title_for(&key, Some("absent-project")), None);
    }

    #[test]
    fn a_pathological_title_is_capped_before_it_reaches_a_strategy_title() {
        // Several distilled strategies are injected into a dispatched
        // agent's context at once, so one fat record must not be able to
        // bloat every retrieval that touches its regime. Multi-byte prose
        // (canon's corpora carry Korean) also proves the cap counts CHARS
        // — a byte cut would split a codepoint and panic in `truncate`.
        let fat = "가".repeat(SCENARIO_TITLE_MAX_CHARS + 40);
        let index = ScenarioTitleIndex::from_records(&[scenario_record("platformer", "platformer.moving.01", &fat, "2026-07-14T19:27:35Z")]);
        let key = regime("platformer", "41fdd8c5");
        let title = index.title_for(&scenario_key("platformer.moving.01"), Some("platformer")).expect("a long title is capped, never dropped");
        assert_eq!(title.chars().count(), SCENARIO_TITLE_MAX_CHARS + 1, "the cap plus its one-char cut marker");
        assert!(title.ends_with('…'), "a cut is marked, never silent");

        let joined = derived_joined(&index, "review", &key, scenario_key("platformer.moving.01"), Some("platformer"), Kind::NonVerdict, "2026-07-14T21:38:36Z");
        let task = rendered_task(&key, vec![joined]);
        assert!(
            task.chars().count() <= "platformer.moving.01: review attestation — ".chars().count() + SCENARIO_TITLE_MAX_CHARS + 1,
            "the cap bounds the RENDERED task, not just the index: {task}"
        );
    }

    #[test]
    fn a_joined_title_is_byte_identical_across_two_passes_over_the_same_events() {
        // s38's idempotence guarantee has to survive this join too:
        // `trajectory_content_digest` folds only the verdict ROWS, so text
        // that wobbled between passes would be skipped as a duplicate and
        // never corrected.
        let key = regime("platformer", "41fdd8c5");
        let titles = moving_platform_index();
        let pass = || {
            vec![
                derived_joined(&titles, "review", &key, scenario_key("platformer.moving.01"), Some("platformer"), Kind::NonVerdict, "2026-07-14T21:38:36Z"),
                // No `project_id` in `detail`: resolves through the
                // single-project fallback to the SAME title, so the
                // rendered title never depends on absorb order.
                derived_joined(&titles, "ledger", &key, scenario_key("platformer.moving.01"), None, Kind::CodeReviewFinding, "2026-07-14T22:00:00Z"),
            ]
        };
        assert_eq!(rendered_task(&key, pass()), rendered_task(&key, pass()));
        assert_eq!(
            rendered_task(&key, pass()),
            "platformer.moving.01: review attestation, code-review finding — A moving platform carries the standing player"
        );
    }
}
