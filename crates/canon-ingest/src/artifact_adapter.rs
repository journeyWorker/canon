//! The `ArtifactAdapter` trait + `ArtifactEvent` normalization target
//! (S4 FOUNDATION wave, frozen for Wave 2's ledger/divergence/handoff/
//! openspec-task adapters).
//!
//! Distinct from [`crate::adapter::SessionAdapter`] (S3): a
//! `SessionAdapter` normalizes billable-model-call token rows keyed by
//! `session_id`; an `ArtifactAdapter` is a **verdict-deriving** adapter
//! — it reads a review/CI/handoff/task-state artifact and normalizes it
//! into an [`ArtifactEvent`] keyed by the S1 join spine's
//! `scenario_id`/`handoff_id`/`task_id`, which `crate::verdict` then
//! folds (a pure, table-driven step, never per-adapter logic — design
//! D5) into an optional `{role, polarity, becomes}` verdict.
//!
//! **Rescope (operator directive, 2026-07-11): every adapter's source
//! root is `canon.yaml`-configured, GENERIC — never a hardcoded
//! sibling-repo path or a live hosted-Postgres connection.**
//! [`ArtifactSourceConfig`] carries that configuration surface;
//! [`ArtifactSourceHandle`] is what an adapter's `parse` call actually
//! reads (a filesystem root for the ledger/divergence/openspec-task
//! adapters, or already-fetched [`RawRecord`]s for the handoff adapter
//! — `canon-ingest` has no `canon-store` dependency, so a DB-backed
//! adapter never opens its own connection; its wave-2 driver resolves
//! the live query through `canon-store::Tier::read` and hands the rows
//! in here).

use std::path::PathBuf;

use canon_model::evidence::RawRecord;
use canon_model::ids::{HandoffId, RoleId, ScenarioId, TaskId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The generic, `canon.yaml`-sourced configuration every
/// `ArtifactAdapter` resolves its source location from. Every path
/// field defaults to `None` — an unconfigured source is simply not
/// scanned, NEVER silently defaulted to a hardcoded sibling repo (the
/// exact violation this rescope removes: the S4 design as originally
/// authored pointed the ledger/divergence adapters at a hardcoded
/// donor-consumer-repo `spec/**` path and the handoff adapter at a
/// prior session/event store's live hosted-Postgres `handoffs`
/// table). The one non-path field,
/// `native_records` (S15 P4, design D7), defaults `false` for the
/// same reason — a native source is never silently scanned either.
///
/// Parsing this struct out of a repo's actual `canon.yaml` file is
/// wave-2/CLI wiring (mirrors how `SessionAdapter::scan_roots` takes an
/// already-resolved `home: &Path` rather than reading YAML itself) —
/// this type is the frozen SHAPE that wiring populates, deriving
/// `Deserialize` so a future `serde_yaml::from_str::<ArtifactSourceConfig>`
/// (or a field nested inside a larger `canon.yaml` document) needs no
/// bespoke parser.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSourceConfig {
    /// Root of a Hive-partitioned ledger tree
    /// (`kind=<kind>/[area=<area>/]*.json`, S4 design D1). The donor
    /// consumer repo's `spec/ledger/` is the reference donor and
    /// fixture-corpus origin — never a compiled-in default.
    #[serde(default)]
    pub ledger_root: Option<PathBuf>,
    /// Root of a Hive-partitioned divergence tree
    /// (`lane=<l>/area=<a>/surface=<s>/*.jsonl`, S4 design D2).
    #[serde(default)]
    pub divergences_root: Option<PathBuf>,
    /// Root under which `openspec/changes/*/tasks.md` files are scanned
    /// (S4 design D4). Ordinarily the consumer repo's own root.
    #[serde(default)]
    pub openspec_root: Option<PathBuf>,
    /// Enables the S15 P4 NATIVE verdict records-source adapters
    /// (`review`/`divergence-native`, design D7) against canon's OWN
    /// tiers. XOR-exclusive with `ledger_root`/`divergences_root`/
    /// `openspec_root`: the two source families' verdict rows differ
    /// slightly, so `trajectory_content_digest` (canon-cli) would not
    /// dedupe them, silently double-counting the same underlying
    /// evidence — `canon-cli::artifact_ingest`'s config-load step
    /// rejects a config that sets both before any read runs (spec
    /// `native-record-flywheel` Requirement 3). Defaults `false`, like
    /// every other field here — never silently on.
    #[serde(default)]
    pub native_records: bool,
}

/// What one [`ArtifactAdapter::parse`] call actually reads — resolved
/// from [`ArtifactSourceConfig`] (path-based sources) or supplied
/// directly by a caller that already ran its own query (handle-based
/// sources, e.g. the handoff adapter's `Tier::read` result).
#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactSourceHandle {
    /// A filesystem root (a directory to walk, or — for the openspec
    /// adapter — a single `tasks.md` file) this adapter scans directly.
    Path(PathBuf),
    /// Already-fetched raw candidate records (e.g. rows a `Tier::read`
    /// call against canon's own Postgres-tier `Handoff` table already
    /// resolved). `canon-ingest` never opens the connection itself —
    /// see this module's doc comment.
    Records(Vec<RawRecord>),
}

/// The S1 join-spine identifier an [`ArtifactEvent`] is keyed by — the
/// three key kinds design §5 S4 names (`scenario_id`, `handoff_id`,
/// `task_id`; `change_id` is not listed separately because
/// `TaskId::change_id()` already decomposes it, join-spine spec).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArtifactJoinKey {
    Scenario(ScenarioId),
    Handoff(HandoffId),
    Task(TaskId),
}

/// The closed classification an [`ArtifactEvent`] carries — the ONLY
/// vocabulary [`crate::verdict::derive_verdict`] reads (S4 design §5,
/// reproduced verbatim in `specs/review-verdict-mapping/spec.md`).
/// Every wave-2 adapter's job is to map its own raw record shape onto
/// one of these variants; `derive_verdict` never sees adapter-specific
/// JSON.
///
/// The seven `*Finding`/`*Promotion`/`*Resolved`/`*Revert`/`*Merge`
/// variants are exactly the design table's seven rows. `NonVerdict`
/// collapses every explicit non-verdict case the design names: a
/// divergence manifest line (D2), a ledger `run`/`drill` record (D1), a
/// handoff state transition alone (D3 — "a handoff is management
/// plumbing, not a review/CI/merge signal"), and an openspec task flip
/// with no parseable merge/CI evidence or a `**DEFERRED**`/`**DROPPED**`
/// rewrite (D4 — "malformed evidence is no evidence… at the verdict
/// layer").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactEventKind {
    /// Ledger `kind=code-review`, `verdict` absent or not `faithful` —
    /// an open/still-divergent finding (table row 1).
    CodeReviewFinding,
    /// Ledger `kind=design-review`, same verdict condition (row 2).
    DesignReviewFinding,
    /// Ledger `kind=review` promoting a scenario to `@reviewed` (row 3).
    ReviewPromotion,
    /// Ledger `kind=clear` clearing a previously `@flagged` scenario
    /// (row 4).
    ClearAfterFlagged,
    /// Divergence `type=remediation` followed by a `resolved` status
    /// (row 5).
    RemediationResolved,
    /// A CI failure or PR revert observed via the openspec/handoff-
    /// joined event stream (row 6).
    CiFailOrPrRevert,
    /// A PR merge with no revert recorded within the configured revert
    /// window (row 7).
    PrMergeNoRevert,
    /// Every explicit non-verdict case (see variant-group doc above) —
    /// `derive_verdict` always returns `None` for this variant.
    NonVerdict,
}

/// The shared normalization target every [`ArtifactAdapter::parse`]
/// call emits — mirrors [`crate::adapter::UnifiedRow`]'s role for
/// `SessionAdapter`, one event per artifact-ingest record.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactEvent {
    /// The emitting adapter's stable identity (`"ledger"` |
    /// `"divergence"` | `"handoff"` | `"openspec-task"`, wave-2).
    pub adapter_id: &'static str,
    /// The S1 join-spine key this event is keyed by.
    pub join_key: ArtifactJoinKey,
    /// What kind of thing happened — the only field
    /// `crate::verdict::derive_verdict` inspects.
    pub kind: ArtifactEventKind,
    /// The role that authored the underlying artifact, when the source
    /// record makes it derivable (required for `ReviewPromotion`'s "the
    /// authoring role of the scenario"; `None` for every other kind).
    pub authoring_role: Option<RoleId>,
    /// The source artifact's area/severity tag (ledger `scenario_id`'s
    /// `<area>` component, divergence `area=`/`surface=` partition
    /// keys, …) — folded into a verdict's `regime_key` by the emitting
    /// adapter (task 5.2), never recomputed inside `derive_verdict`.
    pub area: Option<String>,
    /// A passthrough trust-level tag (`@reviewed`/`@ratified` where
    /// applicable, task 5.3) — carried on the event so the adapter can
    /// copy it onto the emitted verdict without a second source lookup.
    pub trust_level: Option<String>,
    /// When the source record itself was authored/observed.
    pub at: DateTime<Utc>,
    /// The full normalized detail this event carries — mirrors
    /// `canon_model::records::Event.detail`'s deliberately open
    /// `serde_json::Value` shape; the field this event's eventual
    /// `canon_model::records::Event` conversion copies verbatim.
    pub detail: serde_json::Value,
}

/// The result of one [`ArtifactAdapter::parse`] call: the events it
/// successfully extracted, plus a count of records it could not parse
/// at all — mirrors [`crate::adapter::ParseOutcome`]'s "malformed
/// evidence is no evidence" discipline (design §7): a record this
/// adapter's format doesn't recognize is skipped AND counted, never
/// silently dropped, never a panic.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArtifactParseOutcome {
    pub events: Vec<ArtifactEvent>,
    pub skipped: usize,
}

impl ArtifactParseOutcome {
    pub fn empty() -> Self {
        Self::default()
    }
}

/// One artifact-ingest adapter (S4 FOUNDATION, frozen for Wave 2's
/// ledger/divergence/handoff/openspec-task adapters — mirrors
/// `SessionAdapter`'s "trait + static table" shape, S3 design D1,
/// generalized to a source that is sometimes a path and sometimes an
/// already-fetched record batch). `adapter_id()` names the adapter;
/// `resolve_source` turns the generic, `canon.yaml`-sourced
/// [`ArtifactSourceConfig`] into this adapter's own
/// [`ArtifactSourceHandle`] (returning `None` when this adapter's
/// config field is unset — an unconfigured source is never scanned);
/// `parse` converts one resolved handle into an [`ArtifactParseOutcome`].
///
/// A handle-based adapter (the handoff adapter) has no meaningful
/// `resolve_source` output from `ArtifactSourceConfig` alone — its
/// wave-2 driver constructs an `ArtifactSourceHandle::Records(..)`
/// directly (after resolving canon's own Postgres-tier `Handoff` table
/// through `canon-store::Tier::read`, entirely outside this crate) and
/// calls `parse` with it, skipping `resolve_source`.
pub trait ArtifactAdapter: Send + Sync {
    /// The adapter's stable identity (`"ledger"` | `"divergence"` |
    /// `"handoff"` | `"openspec-task"`, wave-2).
    fn adapter_id(&self) -> &'static str;

    /// Resolve this adapter's source from the generic config surface.
    /// `None` when this adapter's config field is unset, or when this
    /// adapter is handle-based (see trait doc comment) — never a
    /// hardcoded fallback path.
    fn resolve_source(&self, config: &ArtifactSourceConfig) -> Option<ArtifactSourceHandle>;

    /// Parse one already-resolved source into an [`ArtifactParseOutcome`].
    /// Malformed/unparseable content is skipped AND counted (design
    /// §7), never a crash.
    fn parse(&self, source: &ArtifactSourceHandle) -> ArtifactParseOutcome;
}

/// Trivial accessor mirroring `SessionAdapter::scan_roots`'s
/// `home`-join convenience for a path-based [`ArtifactSourceConfig`]
/// field — joins a configured root against nothing (it is already
/// absolute or repo-root-relative, unlike `SessionAdapter`'s
/// `home`-relative roots) and simply clones it into an owned
/// [`PathBuf`], the shape [`ArtifactSourceHandle::Path`] wraps.
pub fn resolve_path_source(root: &Option<PathBuf>) -> Option<ArtifactSourceHandle> {
    root.as_ref().map(|p| ArtifactSourceHandle::Path(p.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_source_config_defaults_to_unconfigured() {
        // No field defaults to a hardcoded donor-repo path — an
        // unconfigured source stays `None`; `native_records` stays
        // `false` (never silently on).
        let config = ArtifactSourceConfig::default();
        assert_eq!(config.ledger_root, None);
        assert_eq!(config.divergences_root, None);
        assert_eq!(config.openspec_root, None);
        assert!(!config.native_records);
    }

    #[test]
    fn native_records_deserializes_and_defaults_false() {
        let bare = serde_json::json!({});
        let config: ArtifactSourceConfig = serde_json::from_value(bare).unwrap();
        assert!(!config.native_records);

        let on = serde_json::json!({"native_records": true});
        let config: ArtifactSourceConfig = serde_json::from_value(on).unwrap();
        assert!(config.native_records);
    }

    #[test]
    fn artifact_source_config_deserializes_from_json() {
        let json = serde_json::json!({
            "ledger_root": "canon/fixtures/ledger",
            "divergences_root": "canon/fixtures/divergences",
        });
        let config: ArtifactSourceConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.ledger_root, Some(PathBuf::from("canon/fixtures/ledger")));
        assert_eq!(config.divergences_root, Some(PathBuf::from("canon/fixtures/divergences")));
        assert_eq!(config.openspec_root, None);
    }

    #[test]
    fn resolve_path_source_is_none_when_unconfigured() {
        assert_eq!(resolve_path_source(&None), None);
        assert_eq!(resolve_path_source(&Some(PathBuf::from("a/b"))), Some(ArtifactSourceHandle::Path(PathBuf::from("a/b"))));
    }

    /// A minimal fixture adapter proving `ArtifactAdapter` is
    /// dyn-compatible and that a path-based `resolve_source` +
    /// `parse` round trip works end to end — the shape wave-2's real
    /// adapters implement against.
    struct FixtureAdapter;

    impl ArtifactAdapter for FixtureAdapter {
        fn adapter_id(&self) -> &'static str {
            "fixture"
        }

        fn resolve_source(&self, config: &ArtifactSourceConfig) -> Option<ArtifactSourceHandle> {
            resolve_path_source(&config.ledger_root)
        }

        fn parse(&self, source: &ArtifactSourceHandle) -> ArtifactParseOutcome {
            match source {
                ArtifactSourceHandle::Path(p) if p.exists() => ArtifactParseOutcome { events: Vec::new(), skipped: 0 },
                _ => ArtifactParseOutcome { events: Vec::new(), skipped: 1 },
            }
        }
    }

    #[test]
    fn artifact_adapter_is_dyn_compatible_and_round_trips_config() {
        let adapter: &dyn ArtifactAdapter = &FixtureAdapter;
        assert_eq!(adapter.adapter_id(), "fixture");

        let unconfigured = ArtifactSourceConfig::default();
        assert!(adapter.resolve_source(&unconfigured).is_none());

        let dir = tempfile::tempdir().unwrap();
        let configured = ArtifactSourceConfig { ledger_root: Some(dir.path().to_path_buf()), ..Default::default() };
        let source = adapter.resolve_source(&configured).unwrap();
        let outcome = adapter.parse(&source);
        assert_eq!(outcome.skipped, 0);
    }

    #[test]
    fn artifact_source_handle_records_variant_carries_raw_records() {
        let raw = RawRecord(serde_json::json!({"id": "20260710-1432-fix-a1b2"}));
        let handle = ArtifactSourceHandle::Records(vec![raw.clone()]);
        match handle {
            ArtifactSourceHandle::Records(rows) => assert_eq!(rows, vec![raw]),
            ArtifactSourceHandle::Path(_) => panic!("expected Records variant"),
        }
    }

}
