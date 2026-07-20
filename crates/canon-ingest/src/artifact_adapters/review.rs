//! The native `Review` verdict records-source adapter (S15 P4, design
//! D7): normalizes canon's OWN `Review` records —
//! [`canon_model::records::Review`], read from canon's OWN storage
//! tier, never a raw ledger artifact — into one [`ArtifactEvent`] per
//! record, carrying enough of the native record's own shape in
//! `detail` for `canon-cli::artifact_ingest::run`'s driver to call
//! `crate::verdict::derive_native_review_verdict` directly, entirely
//! BYPASSING `crate::verdict::derive_verdict`'s frozen S4 table (a
//! `Review` record's role source is `envelope.actor.role`, never a
//! ledger-adapter-derived "authoring role of the scenario" — spec
//! `native-record-flywheel` Requirement 2).
//!
//! **Handle-based, exactly like [`crate::artifact_adapters::handoff`]**
//! (that module's own doc comment, mirrored here):
//! [`resolve_source`](ReviewFlywheelAdapter::resolve_source) always
//! returns `None` — there is no `ArtifactSourceConfig` PATH field this
//! adapter resolves a source from (its gate is the boolean
//! `ArtifactSourceConfig::native_records` switch, read by the CLI
//! driver BEFORE it ever fetches records for this adapter — design
//! D7's XOR against the raw-artifact path fields), so
//! `crate::artifact_registry::resolve_and_parse`'s generic
//! config-driven scan path never calls
//! [`parse`](ReviewFlywheelAdapter::parse) for this adapter either
//! (`crate::artifact_registry::ArtifactSourceKind::Records`, same as
//! `handoff`). `canon-cli::artifact_ingest::run` resolves canon's own
//! `Review` table through `canon_store::Tier::read`, converts each
//! `RawRecord` it returns, and calls `parse` directly with
//! `ArtifactSourceHandle::Records(..)` — `canon-ingest` gains no
//! *production* `canon-store` dependency from this adapter either; this
//! crate's `canon-store` dev-dependency (already present for
//! `handoff.rs`'s own fixture) is reused, never duplicated, by this
//! module's own `#[cfg(test)]` fixtures — `cargo build -p canon-ingest`
//! never touches it.
//!
//! `kind` on every emitted event is [`ArtifactEventKind::NonVerdict`]
//! (the frozen S4 vocabulary is NOT used for native verdict derivation
//! — mirrors `handoff.rs`'s own "management plumbing, not a
//! review/CI/merge signal" posture, but for a different reason here: a
//! `Review` record ALREADY IS its own verdict signal, just derived by a
//! separate, explicit native path rather than `derive_verdict`'s
//! table). `detail["native_kind"] == "review"` is the tag
//! `canon-cli::artifact_ingest::run`'s driver reads (together with
//! `authoring_role`) to dispatch to
//! `crate::verdict::derive_native_review_verdict` instead of
//! `derive_verdict`.

use canon_model::envelope::RecordKind;
use canon_model::records::Review;

use crate::artifact_adapter::{
    ArtifactAdapter, ArtifactEvent, ArtifactEventKind, ArtifactJoinKey, ArtifactParseOutcome, ArtifactSourceConfig, ArtifactSourceHandle,
};

pub struct ReviewFlywheelAdapter;

impl ReviewFlywheelAdapter {
    /// One `Review` record normalizes to exactly one `ArtifactEvent`
    /// (unlike `handoff.rs`'s `events_for`, which can emit several
    /// transitions per row — a `Review` has no transition sequence, it
    /// simply exists once).
    fn event(record: &Review) -> ArtifactEvent {
        ArtifactEvent {
            adapter_id: "review",
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
                "native_kind": "review",
                "scenario_id": record.scenario_id.as_str(),
                "project_id": record.project_id.as_str(),
                "pin": record.pin,
                "reviewer": record.reviewer,
            }),
        }
    }
}

impl ArtifactAdapter for ReviewFlywheelAdapter {
    fn adapter_id(&self) -> &'static str {
        "review"
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
    /// to deserialize as a `Review`, or whose own `kind` field
    /// disagrees with `RecordKind::Review`, is one such skip — this
    /// adapter's source is always `ArtifactSourceHandle::Records` in
    /// production (see `resolve_source`'s doc comment); a `Path`
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
            match serde_json::from_value::<Review>(raw.0.clone()) {
                Ok(record) if record.envelope.kind == RecordKind::Review => events.push(Self::event(&record)),
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
    use canon_model::ids::{ProjectId, RoleId, ScenarioId};
    use canon_model::records::ProvenanceRef;

    use super::*;

    fn review(scenario: &str, role: Option<&str>) -> Review {
        let at = "2026-07-01T09:00:00Z".parse().unwrap();
        let actor = match role {
            Some(r) => Actor::new("s15-fixture", RoleId::parse(r).unwrap()),
            None => Actor::new_unattributed("s15-fixture"),
        };
        Review {
            envelope: Envelope::new(1, RecordKind::Review, at, actor),
            project_id: ProjectId::parse("canon").unwrap(),
            scenario_id: ScenarioId::parse(scenario).unwrap(),
            reviewer: "reviewer-1".to_string(),
            pin: "abcdef123456".to_string(),
            provenance_ref: ProvenanceRef::UpstreamRef("upstream://scenario/1".to_string()),
        }
    }

    fn records(reviews: &[Review]) -> ArtifactSourceHandle {
        ArtifactSourceHandle::Records(reviews.iter().map(|r| RawRecord(serde_json::to_value(r).unwrap())).collect())
    }

    #[test]
    fn resolve_source_is_always_none() {
        assert_eq!(ReviewFlywheelAdapter.resolve_source(&ArtifactSourceConfig::default()), None);
    }

    #[test]
    fn path_handle_yields_empty_outcome_never_a_crash() {
        let outcome = ReviewFlywheelAdapter.parse(&ArtifactSourceHandle::Path("nope".into()));
        assert_eq!(outcome, ArtifactParseOutcome::empty());
    }

    #[test]
    fn a_review_record_emits_one_nonverdict_event_carrying_native_kind_and_role() {
        let r = review("world.firstbuy-hotdeal.26", Some("dev"));
        let outcome = ReviewFlywheelAdapter.parse(&records(&[r]));
        assert_eq!(outcome.skipped, 0);
        assert_eq!(outcome.events.len(), 1);
        let event = &outcome.events[0];
        assert_eq!(event.adapter_id, "review");
        assert_eq!(event.kind, ArtifactEventKind::NonVerdict);
        assert_eq!(event.join_key, ArtifactJoinKey::Scenario(ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap()));
        assert_eq!(event.area.as_deref(), Some("world"));
        assert_eq!(event.authoring_role.as_ref().map(|r| r.as_str()), Some("dev"));
        assert_eq!(event.detail["native_kind"], "review");
    }

    #[test]
    fn a_review_with_no_actor_role_carries_no_authoring_role() {
        // Never fabricated — the caller (canon-cli's driver) is the one
        // that decides to skip a role-less native event; this adapter
        // simply carries the truth through.
        let r = review("world.firstbuy-hotdeal.26", None);
        let outcome = ReviewFlywheelAdapter.parse(&records(&[r]));
        assert_eq!(outcome.events[0].authoring_role, None);
    }

    #[test]
    fn a_malformed_record_is_skipped_and_counted() {
        let raw = RawRecord(serde_json::json!({"not": "a review"}));
        let outcome = ReviewFlywheelAdapter.parse(&ArtifactSourceHandle::Records(vec![raw]));
        assert_eq!(outcome.skipped, 1);
        assert!(outcome.events.is_empty());
    }
}
