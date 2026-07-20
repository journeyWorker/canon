//! The static `ArtifactAdapter` registry seam (S4 FOUNDATION wave) —
//! mirrors [`crate::registry`]'s `SessionAdapter` registry shape (a
//! static, declaration-ordered table; no dynamic plugin loading, S3
//! design D1) over [`crate::artifact_adapter::ArtifactAdapter`] instead.
//!
//! FOUNDATION shipped this registry EMPTY; wave-2 appends four entries
//! — `ledger`, `divergence`, `handoff`, `openspec-task` (S4 tasks
//! groups 1-4), each landing independently. S15 P4 (design D7) appends
//! two more — `review`, `divergence-native`, the NATIVE verdict
//! records-source adapters over canon's OWN `Review`/`Divergence`
//! tiers — WITHOUT touching
//! [`ArtifactAdapter`](crate::artifact_adapter::ArtifactAdapter),
//! [`ArtifactEvent`](crate::artifact_adapter::ArtifactEvent), or
//! [`crate::verdict::VerdictRow`] (the frozen contract this module,
//! `crate::artifact_adapter`, and `crate::verdict` together document).

use crate::artifact_adapter::{ArtifactAdapter, ArtifactParseOutcome, ArtifactSourceConfig};
use crate::artifact_adapters::divergence::DivergenceAdapter;
use crate::artifact_adapters::handoff::HandoffAdapter;
use crate::artifact_adapters::ledger::LedgerAdapter;
use crate::artifact_adapters::native_divergence::NativeDivergenceFlywheelAdapter;
use crate::artifact_adapters::openspec_task::OpenspecTaskAdapter;
use crate::artifact_adapters::review::ReviewFlywheelAdapter;

/// Whether an adapter's [`ArtifactAdapter::resolve_source`] expects to
/// find its input under the generic, `canon.yaml`-sourced
/// [`ArtifactSourceConfig`] path fields (`Path` — `ledger`/
/// `divergence`/`openspec-task`), or is fed an already-resolved
/// [`crate::artifact_adapter::ArtifactSourceHandle::Records`] batch by
/// a driver living OUTSIDE this crate (`Records` — `handoff`, whose
/// production driver resolves canon's own Postgres-tier `Handoff`
/// table through `canon-store::Tier::read`; that driver is a DEFERRED
/// residual — the future `canon ingest` artifact-ingest CLI wiring —
/// not shipped anywhere in this workspace yet).
///
/// This is registry-LOCAL bookkeeping, never on the frozen
/// `ArtifactAdapter` trait itself (S4 FOUNDATION froze that trait):
/// [`resolve_and_parse`] reads it below to route a `Records`-kind
/// entry to an explicit [`ArtifactDispatchOutcome::UnsupportedSource`]
/// diagnostic instead of calling `resolve_source` (contractually
/// always `None` for a `Records`-kind adapter — see `ArtifactAdapter`'s
/// own trait doc) and folding that `None` into an empty parse outcome
/// indistinguishable from "configured but nothing found" — the exact
/// P1 silent-drop `ReviewS4Full` flagged (`resolve_and_parse` used to
/// return `ArtifactParseOutcome::empty()` for `handoff` with no signal
/// at all that its production driver simply doesn't exist yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactSourceKind {
    /// Resolved from `ArtifactSourceConfig`'s path fields; safe to run
    /// through the config-driven scan below.
    Path,
    /// Fed a pre-resolved `Records` handle by an out-of-crate driver;
    /// the config-driven scan below cannot supply this and must say
    /// so explicitly, never silently.
    Records,
}

/// One registered artifact-ingest adapter.
pub struct ArtifactAdapterEntry {
    pub adapter: &'static dyn ArtifactAdapter,
    pub source_kind: ArtifactSourceKind,
    /// Registry-LOCAL bookkeeping (like `source_kind` above), never on
    /// the frozen `ArtifactAdapter` trait itself — `true` for the two
    /// S15 P4 NATIVE verdict records-source adapters (`review`,
    /// `divergence-native`, design D7), `false` for the four S4
    /// adapters (including `handoff`, which is `Records`-kind but NOT
    /// a native-verdict source). `canon-cli::artifact_ingest::run`
    /// reads this to gate a `native_verdict: true` entry behind
    /// `ArtifactSourceConfig::native_records` (design D7's XOR
    /// switch) — driven ONLY when that switch is on; a `false` entry
    /// (including `handoff`) is UNAFFECTED by the switch and always
    /// runs.
    pub native_verdict: bool,
}

impl ArtifactAdapterEntry {
    pub fn adapter_id(&self) -> &'static str {
        self.adapter.adapter_id()
    }
}

/// The static registry, in declaration order (mirrors
/// `crate::registry::registry`'s "deterministic order, never
/// `HashMap`-iteration-order dependent"). FOUNDATION shipped zero
/// entries; wave-2 appended `ledger`/`divergence`/`handoff`/
/// `openspec-task`, each tagged with its `ArtifactSourceKind`
/// (`handoff` is the sole S4 `Records`-kind entry — see that type's
/// doc comment). S15 P4 (design D7) appends `review`/
/// `divergence-native` — also `Records`-kind, but additionally tagged
/// `native_verdict: true` (see that field's own doc comment).
pub fn registry() -> &'static [ArtifactAdapterEntry] {
    static DIVERGENCE: DivergenceAdapter = DivergenceAdapter;
    static DIVERGENCE_NATIVE: NativeDivergenceFlywheelAdapter = NativeDivergenceFlywheelAdapter;
    static HANDOFF: HandoffAdapter = HandoffAdapter;
    static LEDGER: LedgerAdapter = LedgerAdapter;
    static OPENSPEC_TASK: OpenspecTaskAdapter = OpenspecTaskAdapter;
    static REVIEW: ReviewFlywheelAdapter = ReviewFlywheelAdapter;
    const REGISTRY: &[ArtifactAdapterEntry] = &[
        ArtifactAdapterEntry { adapter: &DIVERGENCE, source_kind: ArtifactSourceKind::Path, native_verdict: false },
        ArtifactAdapterEntry { adapter: &HANDOFF, source_kind: ArtifactSourceKind::Records, native_verdict: false },
        ArtifactAdapterEntry { adapter: &LEDGER, source_kind: ArtifactSourceKind::Path, native_verdict: false },
        ArtifactAdapterEntry { adapter: &OPENSPEC_TASK, source_kind: ArtifactSourceKind::Path, native_verdict: false },
        ArtifactAdapterEntry { adapter: &REVIEW, source_kind: ArtifactSourceKind::Records, native_verdict: true },
        ArtifactAdapterEntry { adapter: &DIVERGENCE_NATIVE, source_kind: ArtifactSourceKind::Records, native_verdict: true },
    ];
    REGISTRY
}

/// Look up one registered adapter by `adapter_id()` — the natural seam
/// for a future `canon ingest artifacts --adapter <id>` selection
/// (S4's own CLI subcommand is explicitly deferred past this
/// foundation wave).
pub fn find(adapter_id: &str) -> Option<&'static ArtifactAdapterEntry> {
    registry().iter().find(|entry| entry.adapter_id() == adapter_id)
}

/// One [`resolve_and_parse`] dispatch's result — either a completed
/// parse (`Parsed`, possibly an empty outcome when a `Path`-kind
/// adapter's own config field is simply unset) or an explicit
/// diagnostic naming a `Records`-kind adapter this config-driven scan
/// path structurally cannot drive (see [`ArtifactSourceKind`]).
/// `UnsupportedSource` must NEVER collapse into
/// `Parsed(ArtifactParseOutcome::empty())` — that collapse is the
/// exact silent-drop this type exists to prevent.
#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactDispatchOutcome {
    Parsed(ArtifactParseOutcome),
    UnsupportedSource { adapter_id: &'static str, reason: &'static str },
}

/// One adapter's full resolve+parse pass — mirrors
/// `crate::registry::scan_and_parse`'s shape over the artifact-adapter
/// trait, generalized to also cover a `Records`-kind adapter's
/// contractual absence from this config-driven scan path (P1,
/// `ReviewS4Full`). A `Path`-kind entry resolves+parses exactly as
/// before, `Parsed(ArtifactParseOutcome::empty())` when `resolve_source`
/// finds nothing configured (an unconfigured source is never scanned,
/// never a hardcoded fallback). A `Records`-kind entry (`handoff`)
/// NEVER reaches `resolve_source` at all — this dispatch path cannot
/// supply the pre-fetched `Records` handle it needs (that supply is
/// the DEFERRED `canon ingest` CLI-wiring driver's job — see
/// `crate::artifact_adapters::handoff`'s module doc — resolving
/// canon's own Postgres-tier `Handoff` table through
/// `canon_store::Tier::read` and calling `parse` with the result
/// directly, entirely outside this registry-scan seam), so it returns
/// an explicit [`ArtifactDispatchOutcome::UnsupportedSource`] instead
/// of a silently empty outcome.
pub fn resolve_and_parse(entry: &ArtifactAdapterEntry, config: &ArtifactSourceConfig) -> ArtifactDispatchOutcome {
    if entry.source_kind == ArtifactSourceKind::Records {
        return ArtifactDispatchOutcome::UnsupportedSource {
            adapter_id: entry.adapter_id(),
            reason: "requires a records source (canon-store Tier::read), not available in the config-driven scan path — awaiting the deferred canon-ingest CLI driver",
        };
    }
    match entry.adapter.resolve_source(config) {
        Some(source) => ArtifactDispatchOutcome::Parsed(entry.adapter.parse(&source)),
        None => ArtifactDispatchOutcome::Parsed(ArtifactParseOutcome::empty()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_adapter::ArtifactSourceHandle;

    #[test]
    fn registry_contains_the_divergence_adapter() {
        // Wave-2 lands its four adapters independently, so this check
        // is count-agnostic (single-owner test, per S4Foundation/Main
        // coordination: whoever lands first makes it so, subsequent
        // adapters do NOT touch this test) — it asserts `divergence`
        // is registered and lookup-able, not the registry's total
        // size or declaration order relative to its siblings.
        assert!(find("divergence").is_some());
        assert_eq!(find("divergence").unwrap().adapter_id(), "divergence");
        assert!(registry().iter().any(|e| e.adapter_id() == "divergence"));
    }

    #[test]
    fn registry_contains_the_handoff_adapter() {
        // Own coverage, not the single-owner count-agnostic test above
        // (S4Divergence landed that one first; per protocol I don't
        // touch it, just add my own `find` assertion).
        assert!(find("handoff").is_some());
        assert_eq!(find("handoff").unwrap().adapter_id(), "handoff");
        assert!(registry().iter().any(|e| e.adapter_id() == "handoff"));
    }

    #[test]
    fn handoff_is_tagged_records_and_every_other_s4_adapter_is_tagged_path() {
        let handoff = find("handoff").expect("handoff registered");
        assert_eq!(handoff.source_kind, ArtifactSourceKind::Records);
        for id in ["divergence", "ledger", "openspec-task"] {
            let entry = find(id).unwrap_or_else(|| panic!("{id} registered"));
            assert_eq!(entry.source_kind, ArtifactSourceKind::Path, "{id} must be tagged Path-source");
        }
    }

    #[test]
    fn review_and_divergence_native_are_registered_records_kind_and_native_verdict() {
        for id in ["review", "divergence-native"] {
            let entry = find(id).unwrap_or_else(|| panic!("{id} registered"));
            assert_eq!(entry.source_kind, ArtifactSourceKind::Records, "{id} must be tagged Records-source");
            assert!(entry.native_verdict, "{id} must be tagged native_verdict: true");
        }
    }

    #[test]
    fn every_s4_adapter_is_tagged_native_verdict_false() {
        for id in ["divergence", "ledger", "openspec-task", "handoff"] {
            let entry = find(id).unwrap_or_else(|| panic!("{id} registered"));
            assert!(!entry.native_verdict, "{id} must be tagged native_verdict: false");
        }
    }

    #[test]
    fn records_source_adapter_is_never_silently_emptied_by_the_config_driven_scan() {
        // The exact regression this P1 fix prevents (ReviewS4Full):
        // a registered records-source adapter (`handoff`) run through
        // the config-driven `resolve_and_parse` dispatch must surface
        // the unsupported-source diagnostic, never collapse into
        // `Parsed(ArtifactParseOutcome::empty())` — which looks
        // identical to "configured but nothing found" to any caller
        // inspecting the outcome, silently hiding the fact that this
        // adapter's production driver (`canon-store::Tier::read`,
        // outside this crate) doesn't exist yet.
        let handoff_entry = find("handoff").expect("handoff is registered");
        let outcome = resolve_and_parse(handoff_entry, &ArtifactSourceConfig::default());
        match outcome {
            ArtifactDispatchOutcome::UnsupportedSource { adapter_id, reason } => {
                assert_eq!(adapter_id, "handoff");
                assert!(!reason.is_empty());
            }
            ArtifactDispatchOutcome::Parsed(_) => {
                panic!("a records-source adapter must never silently parse through the config-driven scan path")
            }
        }
    }

    struct StubAdapter;

    impl ArtifactAdapter for StubAdapter {
        fn adapter_id(&self) -> &'static str {
            "stub"
        }

        fn resolve_source(&self, config: &ArtifactSourceConfig) -> Option<ArtifactSourceHandle> {
            crate::artifact_adapter::resolve_path_source(&config.ledger_root)
        }

        fn parse(&self, _source: &ArtifactSourceHandle) -> ArtifactParseOutcome {
            ArtifactParseOutcome::empty()
        }
    }

    #[test]
    fn resolve_and_parse_is_empty_outcome_when_unconfigured() {
        static STUB: StubAdapter = StubAdapter;
        let entry = ArtifactAdapterEntry { adapter: &STUB, source_kind: ArtifactSourceKind::Path, native_verdict: false };
        let outcome = resolve_and_parse(&entry, &ArtifactSourceConfig::default());
        assert_eq!(outcome, ArtifactDispatchOutcome::Parsed(ArtifactParseOutcome::empty()));
    }
}
