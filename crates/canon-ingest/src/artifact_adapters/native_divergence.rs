//! The native `Divergence` verdict records-source adapter (S15 P4,
//! design D7): normalizes canon's OWN `Divergence` records —
//! [`canon_model::records::Divergence`], read from canon's OWN storage
//! tier, never a raw divergence-manifest JSONL artifact — into one
//! [`ArtifactEvent`] per record, carrying its `status` in `detail` for
//! `canon-cli::artifact_ingest::run`'s driver to call
//! `crate::verdict::derive_native_divergence_verdict` directly,
//! entirely BYPASSING `crate::verdict::derive_verdict`'s frozen S4
//! table (a native `Divergence`'s role source is `envelope.actor.role`,
//! never `derive_verdict`'s `RemediationResolved` row's hard-coded
//! `dev` constant — spec `native-record-flywheel` Requirement 2).
//!
//! Distinct from [`crate::artifact_adapters::divergence`] (the S4
//! `Path`-kind adapter over a raw `lane=/area=/surface=/*.jsonl`
//! manifest tree, module name `divergence`, `adapter_id() ==
//! "divergence"`) — this adapter's `adapter_id()` is
//! `"divergence-native"` so both can be registered and driven
//! simultaneously without a collision, mirroring how `review` (this
//! wave) and any future ledger-sourced review adapter would coexist.
//!
//! **Handle-based, exactly like [`crate::artifact_adapters::handoff`]**
//! (that module's own doc comment, mirrored here):
//! [`resolve_source`](NativeDivergenceFlywheelAdapter::resolve_source)
//! always returns `None` — there is no `ArtifactSourceConfig` PATH
//! field this adapter resolves a source from (its gate is the boolean
//! `ArtifactSourceConfig::native_records` switch, read by the CLI
//! driver BEFORE it ever fetches records for this adapter — design
//! D7's XOR against the raw-artifact path fields), so
//! `crate::artifact_registry::resolve_and_parse`'s generic
//! config-driven scan path never calls
//! [`parse`](NativeDivergenceFlywheelAdapter::parse) for this adapter
//! either (`crate::artifact_registry::ArtifactSourceKind::Records`,
//! same as `handoff`). `canon-cli::artifact_ingest::run` resolves
//! canon's own `Divergence` table through `canon_store::Tier::read`,
//! converts each `RawRecord` it returns, and calls `parse` directly
//! with `ArtifactSourceHandle::Records(..)` — `canon-ingest` gains no
//! *production* `canon-store` dependency from this adapter either; this
//! crate's `canon-store` dev-dependency (already present for
//! `handoff.rs`'s own fixture) is reused, never duplicated, by this
//! module's own `#[cfg(test)]` fixtures — `cargo build -p canon-ingest`
//! never touches it.
//!
//! **This adapter never folds the `run_seq`/`round` fold-ordering
//! machinery** ([`crate::artifact_adapter`]'s [`canon_model::fold`]
//! sibling module, design D8) — that fold answers "what IS this
//! `(project_id, scenario_id)` group's CURRENT divergence state right
//! now", a different question from this adapter's "what verdict does
//! THIS ONE RECORD, taken alone, contribute to the flywheel". Every
//! `Divergence` record this adapter reads emits its own event,
//! independently — `canon-cli::artifact_ingest::run`'s driver derives
//! (up to) one verdict per record, exactly as every other
//! records-source adapter here does.
//!
//! `kind` on every emitted event is [`ArtifactEventKind::NonVerdict`]
//! (the frozen S4 vocabulary is NOT used for native verdict derivation).
//! `detail["native_kind"] == "divergence"` plus `detail["status"]`
//! (the record's own `DivergenceStatus`, serialized verbatim) are the
//! tags `canon-cli::artifact_ingest::run`'s driver reads (together with
//! `authoring_role`) to dispatch to
//! `crate::verdict::derive_native_divergence_verdict` instead of
//! `derive_verdict`.

use canon_model::envelope::RecordKind;
use canon_model::records::Divergence;

use crate::artifact_adapter::{
    ArtifactAdapter, ArtifactEvent, ArtifactEventKind, ArtifactJoinKey, ArtifactParseOutcome, ArtifactSourceConfig, ArtifactSourceHandle,
};

pub struct NativeDivergenceFlywheelAdapter;

impl NativeDivergenceFlywheelAdapter {
    /// One `Divergence` record normalizes to exactly one
    /// `ArtifactEvent` — this adapter does not replay a transition
    /// history the way `handoff.rs`'s `events_for` does; each stored
    /// `Divergence` row IS one review-round observation already.
    fn event(record: &Divergence) -> ArtifactEvent {
        let status = serde_json::to_value(&record.status).expect("DivergenceStatus always serializes");
        ArtifactEvent {
            adapter_id: "divergence-native",
            join_key: ArtifactJoinKey::Scenario(record.scenario_id.clone()),
            // Never a verdict via the frozen S4 table (module doc) —
            // the native derivation path reads `detail`/`authoring_role`
            // instead.
            kind: ArtifactEventKind::NonVerdict,
            authoring_role: record.envelope.actor.role.clone(),
            area: Some(record.scenario_id.area().to_string()),
            trust_level: None,
            at: record.envelope.at,
            detail: serde_json::json!({
                "native_kind": "divergence",
                "status": status,
                "scenario_id": record.scenario_id.as_str(),
                "project_id": record.project_id.as_str(),
                "run_seq": record.run_seq,
                "round": record.round,
                "reviewer": record.reviewer,
            }),
        }
    }
}

impl ArtifactAdapter for NativeDivergenceFlywheelAdapter {
    fn adapter_id(&self) -> &'static str {
        "divergence-native"
    }

    /// Handle-based adapter (module doc above): there is no
    /// `ArtifactSourceConfig` path field this adapter resolves a
    /// source from, so this always returns `None` — its CLI driver
    /// constructs an `ArtifactSourceHandle::Records(..)` directly and
    /// calls `parse` with it, skipping this method entirely.
    fn resolve_source(&self, _config: &ArtifactSourceConfig) -> Option<ArtifactSourceHandle> {
        None
    }

    /// Malformed/unparseable content is skipped AND counted, never a
    /// crash (design §7, mirrors `handoff.rs`). A candidate that fails
    /// to deserialize as a `Divergence`, or whose own `kind` field
    /// disagrees with `RecordKind::Divergence`, is one such skip —
    /// this adapter's source is always `ArtifactSourceHandle::Records`
    /// in production (see `resolve_source`'s doc comment); a `Path`
    /// handle reaching here at all is a caller-contract violation,
    /// handled the same never-a-crash way rather than treated as one
    /// malformed record.
    fn parse(&self, source: &ArtifactSourceHandle) -> ArtifactParseOutcome {
        let ArtifactSourceHandle::Records(raws) = source else {
            return ArtifactParseOutcome::empty();
        };

        let mut events = Vec::new();
        let mut skipped = 0usize;
        for raw in raws {
            match serde_json::from_value::<Divergence>(raw.0.clone()) {
                Ok(record) if record.envelope.kind == RecordKind::Divergence => events.push(Self::event(&record)),
                _ => skipped += 1,
            }
        }
        ArtifactParseOutcome { events, skipped }
    }
}

#[cfg(test)]
mod tests {
    use canon_model::envelope::{Actor, Envelope};
    use canon_model::evidence::RawRecord;
    use canon_model::ids::{ProjectId, RoleId, ScenarioId, Sha, TotalOrder};
    use canon_model::records::DivergenceStatus;

    use super::*;

    fn divergence(scenario: &str, role: Option<&str>, status: DivergenceStatus, run_seq: u64) -> Divergence {
        let at = "2026-07-01T09:00:00Z".parse().unwrap();
        let actor = match role {
            Some(r) => Actor::new("s15-fixture", RoleId::parse(r).unwrap()),
            None => Actor::new_unattributed("s15-fixture"),
        };
        Divergence {
            envelope: Envelope::new(1, RecordKind::Divergence, at, actor),
            project_id: ProjectId::parse("canon").unwrap(),
            scenario_id: ScenarioId::parse(scenario).unwrap(),
            sha: Sha::parse("a".repeat(40)).unwrap(),
            status,
            run_seq: TotalOrder::new(run_seq),
            round: 1,
            reviewer: "reviewer-1".to_string(),
            detail: String::new(),
        }
    }

    fn records(divergences: &[Divergence]) -> ArtifactSourceHandle {
        ArtifactSourceHandle::Records(divergences.iter().map(|d| RawRecord(serde_json::to_value(d).unwrap())).collect())
    }

    #[test]
    fn resolve_source_is_always_none() {
        assert_eq!(NativeDivergenceFlywheelAdapter.resolve_source(&ArtifactSourceConfig::default()), None);
    }

    #[test]
    fn path_handle_yields_empty_outcome_never_a_crash() {
        let outcome = NativeDivergenceFlywheelAdapter.parse(&ArtifactSourceHandle::Path("nope".into()));
        assert_eq!(outcome, ArtifactParseOutcome::empty());
    }

    #[test]
    fn a_resolved_divergence_emits_one_nonverdict_event_carrying_status_and_role() {
        let d = divergence("world.firstbuy-hotdeal.26", Some("dev"), DivergenceStatus::Resolved, 3);
        let outcome = NativeDivergenceFlywheelAdapter.parse(&records(&[d]));
        assert_eq!(outcome.skipped, 0);
        assert_eq!(outcome.events.len(), 1);
        let event = &outcome.events[0];
        assert_eq!(event.adapter_id, "divergence-native");
        assert_eq!(event.kind, ArtifactEventKind::NonVerdict);
        assert_eq!(event.join_key, ArtifactJoinKey::Scenario(ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap()));
        assert_eq!(event.area.as_deref(), Some("world"));
        assert_eq!(event.authoring_role.as_ref().map(|r| r.as_str()), Some("dev"));
        assert_eq!(event.detail["native_kind"], "divergence");
        assert_eq!(event.detail["status"], "resolved");
    }

    #[test]
    fn a_deferred_divergence_serializes_its_reason_and_expiry_into_status() {
        let expiry: chrono::DateTime<chrono::Utc> = "2026-08-01T00:00:00Z".parse().unwrap();
        let status = DivergenceStatus::Deferred { reason: "waiting on design".to_string(), expiry };
        let d = divergence("world.firstbuy-hotdeal.26", Some("design"), status, 1);
        let outcome = NativeDivergenceFlywheelAdapter.parse(&records(&[d]));
        let detail_status = &outcome.events[0].detail["status"];
        assert_eq!(detail_status["deferred"]["reason"], "waiting on design");
    }

    #[test]
    fn a_malformed_record_is_skipped_and_counted() {
        let raw = RawRecord(serde_json::json!({"not": "a divergence"}));
        let outcome = NativeDivergenceFlywheelAdapter.parse(&ArtifactSourceHandle::Records(vec![raw]));
        assert_eq!(outcome.skipped, 1);
        assert!(outcome.events.is_empty());
    }
}
