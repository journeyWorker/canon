//! The `PlanAdapter` trait + `PlanParseOutcome` normalization target
//! (s17 P1 FOUNDATION, design D1/D5 — frozen for P2's openspec dialect
//! adapter and any deferred dialect after it).
//!
//! Distinct from [`crate::artifact_adapter::ArtifactAdapter`] (S4): an
//! `ArtifactAdapter` reads a review/CI/handoff/task-state ARTIFACT and
//! derives a verdict EVENT keyed by the S1 join spine; a `PlanAdapter`
//! reads a foreign PLAN dialect (an openspec change dir, s17's
//! reference dialect) and normalizes it into `Change`/`Task` record
//! CANDIDATES — plan STATE, not verdict events. Both families sit
//! beside `SessionAdapter`/`registry` (S3) as canon-ingest's third
//! trait-plus-static-registry connector pair (design D1): a static
//! table, no dynamic plugin loading, and — the load-bearing boundary
//! this crate holds for all three families — no `canon-store`
//! dependency. `canon-ingest` is pure scan/parse/normalize domain
//! logic; `canon-cli`'s driver (P3) is the one place a `PlanAdapter`'s
//! output meets a validated tiered write.
//!
//! # `PlanSourceConfig`/`PlanSourceHandle`: config vs. resolved source
//! Mirrors [`crate::artifact_adapter::ArtifactSourceConfig`]/
//! [`crate::artifact_adapter::ArtifactSourceHandle`]'s split, adapted
//! to how `canon.yaml`'s `plans:` section is actually shaped (design
//! D2): a LIST of `{dialect, root}` pairs, not one named path field per
//! known dialect. Dialect selection happens externally — the driver
//! (P3) looks up [`crate::plan_registry::find`] by a configured
//! source's `dialect` string BEFORE calling `resolve_source`, so
//! [`PlanSourceConfig`] carries only the one root that particular call
//! is about. `resolve_source` returning `None` means "this call's root
//! is unconfigured" — never scanned, never a hardcoded fallback,
//! exactly like the artifact family's own discipline.
//!
//! # `PlanParseOutcome`: candidates + named per-construct drop counts
//! Every dialect maps foreign constructs onto ONLY `Change`/`Task`
//! (design D3) — the closed 12-member `RecordKind` set is the
//! acceptance bar, never a 13th kind. A foreign construct with no home
//! among the twelve (e.g. openspec's `#### Scenario:` spec-delta
//! blocks, or a `design.md`) is dropped with a NAMED diagnostic count,
//! keyed by a stable construct-name string
//! (`unmapped.get("spec-delta-scenario")`) rather than a fixed struct
//! field — this is deliberate: the spec's own "adding a dialect
//! requires exactly one registry entry plus one adapter module — no
//! change to the trait, the outcome type, the driver, or any other
//! adapter" (`plan-import-connector` spec) rules out a struct with
//! openspec-specific field names, since a second dialect's unmapped
//! constructs would then force a breaking change to this shared type.
//! `malformed` is a Vec of NAMED entries (s18 `loud-plan-import-
//! diagnostics` spec) for a construct that is not merely unmappable but
//! structurally broken (an unreadable `proposal.md`, a change dir whose
//! basename fails `ChangeId::parse`) — each entry carries the
//! construct's relative path, a reason drawn from a fixed per-adapter
//! vocabulary, and an optional actionable hint (e.g. the openspec
//! dialect's root-one-level-too-high `changes`-basename signature) —
//! "malformed evidence is no evidence," never a crash (spec "Malformed
//! plan sources fail soft per construct"), and never an anonymous
//! increment with no way to identify WHICH construct failed or WHY.

use std::collections::BTreeMap;
use std::path::PathBuf;

use canon_model::records::{Change, Task};
use serde::{Deserialize, Serialize};

/// The generic, `canon.yaml`-sourced configuration one
/// [`PlanAdapter::resolve_source`] call resolves its source location
/// from — one already dialect-selected root (design D2: the driver
/// picks the adapter via [`crate::plan_registry::find`] BEFORE
/// building this config, so there is no per-dialect field to choose
/// between, unlike [`crate::artifact_adapter::ArtifactSourceConfig`]'s
/// several named fields). `root` defaults `None` — an unconfigured
/// source is simply not scanned, never silently defaulted to a
/// hardcoded path. Derives `Deserialize` for the same reason
/// `ArtifactSourceConfig` does: a `canon.yaml` `plans.sources[]` entry
/// round-trips through this shape with no bespoke parser.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSourceConfig {
    /// The root this call should scan, already resolved relative to
    /// the `canon.yaml` directory (P3 CLI wiring) — a repo root
    /// containing `openspec/changes/`, a changes dir directly, or a
    /// fixture tree; each dialect's own `resolve_source`/`parse` decide
    /// how much of that shape tolerance they need.
    #[serde(default)]
    pub root: Option<PathBuf>,
}

/// What one [`PlanAdapter::parse`] call actually reads — resolved from
/// [`PlanSourceConfig`]. A single `Path` variant today (every plan
/// dialect canon ships — openspec, superpowers — or defers —
/// donor-JSON — is a file-tree source, design.md's architecture
/// diagram); kept as an enum rather than a bare `PathBuf` so a future
/// non-path-based dialect can add a variant without a breaking change
/// to [`PlanAdapter::parse`]'s signature, mirroring
/// [`crate::artifact_adapter::ArtifactSourceHandle`]'s own
/// config-vs-handle split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanSourceHandle {
    /// A filesystem root this adapter scans directly.
    Path(PathBuf),
}

/// One structurally-broken construct a [`PlanAdapter::parse`] call
/// skipped (s18 `loud-plan-import-diagnostics` spec's "Every malformed
/// plan-import construct is named by path and reason"): a NAMED entry
/// carrying the construct's relative path and a reason drawn from a
/// fixed, per-adapter reason vocabulary — never an anonymous increment
/// to a bare count with no way to identify WHICH construct failed or
/// WHY. `hint` carries an additional actionable hint for a specific
/// near-miss signature (e.g. the openspec dialect's root-one-level-
/// too-high `changes`-basename hint) — `None` for every ordinary
/// malformed entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MalformedEntry {
    /// The construct's path (relative to the source root when
    /// derivable — never an absolute path leaking the host filesystem
    /// layout into a persisted/printed diagnostic).
    pub path: String,
    /// A stable reason string drawn from the adapter's own fixed
    /// vocabulary (e.g. the openspec dialect's `"missing-proposal-md"`,
    /// `"invalid-change-id-grammar"`, `"unreadable-directory"`).
    pub reason: String,
    /// An additional actionable hint for a specific near-miss
    /// signature — `None` for every ordinary malformed entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// The result of one [`PlanAdapter::parse`] call: the `Change`/`Task`
/// record candidates it extracted, per-construct NAMED unmapped-drop
/// counts (design D3, see module doc comment), and a `malformed` list
/// of [`MalformedEntry`] values for every structurally broken construct
/// (s18 `loud-plan-import-diagnostics` spec). Never a `canon-store`
/// type — a candidate is an ordinary `canon_model::records` value the
/// P3 CLI driver persists through `TierRegistry::persist`, exactly like
/// any other adapter's normalized output.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PlanParseOutcome {
    pub changes: Vec<Change>,
    pub tasks: Vec<Task>,
    /// Per-construct unmapped-drop counts, keyed by a stable construct
    /// name (e.g. `"spec-delta-scenario"`, `"design-doc"` for the
    /// openspec dialect, design D3). A `BTreeMap`, never a `HashMap` —
    /// deterministic iteration order for any rendered pass summary,
    /// this crate's established "never HashMap-iteration-order
    /// dependent" discipline (mirrors `plan_registry`'s own ordering
    /// guarantee).
    pub unmapped: BTreeMap<String, usize>,
    /// Individual constructs (one change dir, one row) skipped because
    /// they were unreadable or grammar-invalid, never because they were
    /// merely unmappable (that's `unmapped`, above) — skip AND count,
    /// each one NAMED by path + reason (s18), never a bare count, never
    /// a crash, never a silent drop. `.len()` is the count a caller
    /// that only needs the scalar tally reaches for.
    pub malformed: Vec<MalformedEntry>,
}

impl PlanParseOutcome {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Increment the named unmapped-drop count for `construct`,
    /// inserting a fresh entry on first occurrence — the one mutator
    /// every dialect adapter should call instead of touching `unmapped`
    /// directly, so the "keyed by a stable construct name" discipline
    /// (design D3) has a single call site to audit.
    pub fn record_unmapped(&mut self, construct: &str) {
        *self.unmapped.entry(construct.to_string()).or_insert(0) += 1;
    }

    /// Record one malformed construct at `path` with `reason`, no hint
    /// — the one mutator every dialect adapter should call instead of
    /// pushing to `malformed` directly (s18 `loud-plan-import-
    /// diagnostics` spec's "named by path and reason").
    pub fn record_malformed(&mut self, path: impl Into<String>, reason: impl Into<String>) {
        self.malformed.push(MalformedEntry { path: path.into(), reason: reason.into(), hint: None });
    }

    /// Record one malformed construct at `path` with `reason` PLUS an
    /// additional actionable `hint` — the near-miss variant (e.g. the
    /// openspec dialect's root-one-level-too-high `changes`-basename
    /// hint).
    pub fn record_malformed_with_hint(&mut self, path: impl Into<String>, reason: impl Into<String>, hint: impl Into<String>) {
        self.malformed.push(MalformedEntry { path: path.into(), reason: reason.into(), hint: Some(hint.into()) });
    }
}

/// One plan-dialect adapter (s17 P1 FOUNDATION, frozen for P2's
/// openspec adapter and any dialect after it — mirrors
/// [`crate::artifact_adapter::ArtifactAdapter`]'s "trait + static
/// table" shape, itself mirroring `SessionAdapter`'s S3 design D1).
/// `dialect_id()` names the dialect (`"openspec"`, the reference
/// dialect s17 shipped; `"superpowers"`, s30's `plan-dialect-
/// superpowers` shipped against the superpowers `writing-plans`
/// skill's grammar; `"donor-json"` stays deferred, design.md's
/// architecture diagram); `resolve_source` turns
/// the generic, `canon.yaml`-sourced [`PlanSourceConfig`] into this
/// adapter's own [`PlanSourceHandle`] (returning `None` when `root` is
/// unset — an unconfigured source is never scanned); `parse` converts
/// one resolved handle into a [`PlanParseOutcome`].
pub trait PlanAdapter: Send + Sync {
    /// The dialect's stable identity — the string a `canon.yaml`
    /// `plans.sources[].dialect` entry and `canon ingest plans
    /// --dialect <id>` both name, and [`crate::plan_registry::find`]
    /// looks up.
    fn dialect_id(&self) -> &'static str;

    /// Resolve this adapter's source from the generic config surface.
    /// `None` when `config.root` is unset — never a hardcoded fallback
    /// path.
    fn resolve_source(&self, config: &PlanSourceConfig) -> Option<PlanSourceHandle>;

    /// Parse one already-resolved source into a [`PlanParseOutcome`].
    /// A malformed/unreadable individual construct is skipped AND
    /// counted (design D3), never a crash; a genuinely unmappable
    /// construct is dropped with a NAMED `unmapped` diagnostic, never
    /// an invented mapping onto `Change`/`Task`.
    fn parse(&self, source: &PlanSourceHandle) -> PlanParseOutcome;
}

/// Trivial accessor mirroring
/// [`crate::artifact_adapter::resolve_path_source`]'s convenience for
/// a path-based [`PlanSourceConfig::root`] field — wraps a configured
/// root into the [`PlanSourceHandle::Path`] shape [`PlanAdapter::parse`]
/// expects, or `None` when unconfigured.
pub fn resolve_path_source(root: &Option<PathBuf>) -> Option<PlanSourceHandle> {
    root.as_ref().map(|p| PlanSourceHandle::Path(p.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_outcome_has_no_candidates_and_zero_counts() {
        let outcome = PlanParseOutcome::empty();
        assert!(outcome.changes.is_empty());
        assert!(outcome.tasks.is_empty());
        assert!(outcome.unmapped.is_empty());
        assert!(outcome.malformed.is_empty());
    }

    #[test]
    fn record_unmapped_accumulates_by_construct_name() {
        let mut outcome = PlanParseOutcome::empty();
        outcome.record_unmapped("spec-delta-scenario");
        outcome.record_unmapped("spec-delta-scenario");
        outcome.record_unmapped("design-doc");
        assert_eq!(outcome.unmapped.get("spec-delta-scenario"), Some(&2));
        assert_eq!(outcome.unmapped.get("design-doc"), Some(&1));
        assert_eq!(outcome.unmapped.len(), 2, "unrelated construct names never collide");
    }

    #[test]
    fn record_malformed_names_the_path_and_reason() {
        let mut outcome = PlanParseOutcome::empty();
        outcome.record_malformed("openspec/changes/bad-dir", "missing-proposal-md");
        assert_eq!(outcome.malformed.len(), 1);
        assert_eq!(outcome.malformed[0].path, "openspec/changes/bad-dir");
        assert_eq!(outcome.malformed[0].reason, "missing-proposal-md");
        assert_eq!(outcome.malformed[0].hint, None);
    }

    #[test]
    fn record_malformed_with_hint_carries_the_actionable_hint() {
        let mut outcome = PlanParseOutcome::empty();
        outcome.record_malformed_with_hint("changes", "missing-proposal-md", "root: may point at the changes dir's parent");
        assert_eq!(outcome.malformed.len(), 1);
        assert_eq!(outcome.malformed[0].hint.as_deref(), Some("root: may point at the changes dir's parent"));
    }

    #[test]
    fn resolve_path_source_is_none_when_root_unconfigured() {
        assert_eq!(resolve_path_source(&None), None);
    }

    #[test]
    fn resolve_path_source_wraps_a_configured_root() {
        let root = PathBuf::from("/tmp/some-plan-source");
        assert_eq!(resolve_path_source(&Some(root.clone())), Some(PlanSourceHandle::Path(root)));
    }

    struct StubAdapter;

    impl PlanAdapter for StubAdapter {
        fn dialect_id(&self) -> &'static str {
            "stub"
        }

        fn resolve_source(&self, config: &PlanSourceConfig) -> Option<PlanSourceHandle> {
            resolve_path_source(&config.root)
        }

        fn parse(&self, _source: &PlanSourceHandle) -> PlanParseOutcome {
            PlanParseOutcome::empty()
        }
    }

    #[test]
    fn a_plan_adapter_implementation_resolves_and_parses_through_the_trait_object() {
        let adapter: &dyn PlanAdapter = &StubAdapter;
        assert_eq!(adapter.dialect_id(), "stub");
        assert!(adapter.resolve_source(&PlanSourceConfig::default()).is_none());
        let config = PlanSourceConfig { root: Some(PathBuf::from("/tmp/x")) };
        let source = adapter.resolve_source(&config).expect("configured root resolves");
        assert_eq!(adapter.parse(&source), PlanParseOutcome::empty());
    }
}
