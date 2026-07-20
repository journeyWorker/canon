//! The handoff artifact adapter (S4 wave-2, `design.md` D3): normalizes
//! canon's OWN `Handoff` records — S1's type
//! (`crates/canon-model/src/handoff.rs`), wire-compatible field-for-
//! field with a prior session/event store's `handoffs` table, but read from canon's OWN
//! storage tier — into the state-transition [`ArtifactEvent`]s design
//! §5 D3 names: row insert = created, `claimed_by` set = claimed,
//! `state` = `done`/`abandoned` = terminal.
//!
//! **Never a live prior-session-store/hosted-Postgres connection** (operator rescope
//! directive 2026-07-11, `design.md` D3/D6): this adapter is
//! handle-based, exactly like [`crate::artifact_adapter`]'s module doc
//! describes — [`resolve_source`](HandoffAdapter::resolve_source) always
//! returns `None` (there is no `ArtifactSourceConfig` field this
//! adapter resolves a path from), so
//! `crate::artifact_registry::resolve_and_parse`'s generic config-driven
//! path never calls [`parse`](HandoffAdapter::parse) for this adapter at
//! all. A wave-2 driver living OUTSIDE `canon-ingest` resolves canon's
//! own Postgres-tier `Handoff` table through `canon_store::Tier::read`,
//! converts each `RawRecord` it returns, and calls `parse` directly with
//! `ArtifactSourceHandle::Records(..)` — `canon-ingest` gains no
//! *production* `canon-store` dependency (task 0.1: "so the handoff
//! adapter never needs a `canon-store` dependency inside `canon-ingest`");
//! this crate's `canon-store` dev-dependency (added alongside this
//! module) exists ONLY to build+read a deterministic git-tier tempdir
//! fixture in `tests/handoff_fixture.rs` — `cargo build -p canon-ingest`
//! never touches it.
//!
//! `handoff_id` on every emitted event is the record's own `id` column
//! verbatim (design D3, task 3.2) — never re-derived; `openspec_change_slug`
//! is carried into `detail.change_id` when present (task 3.2's second
//! half), since [`ArtifactJoinKey`] has no separate `Change` variant
//! (`crate::artifact_adapter` module doc: "`change_id` is not listed
//! separately because `TaskId::change_id()` already decomposes it").
//!
//! **A handoff is management plumbing, not a review/CI/merge signal**
//! (design D3): every emitted event is [`ArtifactEventKind::NonVerdict`],
//! so [`crate::verdict::derive_verdict`] always returns `None` for it —
//! the events exist purely so another adapter's verdict can later join
//! to the handoff that carried the work (S1 join spine: `handoff_id |
//! handoff ↔ session ↔ change`).
//!
//! **One snapshot, several transitions.** This adapter's `parse` always
//! receives each handoff's CURRENT row, not an append-only transition
//! log — the caller (`crate::artifact_ingest::read_records_for` in
//! `canon-cli`, this module's own doc names it) is what guarantees
//! that, folding `PgTier::read`'s post-s21 raw multi-version history
//! down to one current row per `handoff_id` BEFORE calling `parse`
//! (s21 P4: `Tier::read` itself no longer pre-folds for ANY adapter) —
//! so one `Handoff` value normalizes to every transition its own state
//! implies has already happened (`events_for`), not a single "current
//! state" event. This is a pure, deterministic function of the row's
//! own fields (no wall-clock reads, no randomness): re-`parse`ing an
//! unchanged batch of records always yields the byte-identical event
//! sequence (task 6's idempotence bar), and a still-in-flight poll that
//! observes the identical row twice re-derives the identical events
//! rather than emitting a growing log.

use canon_model::envelope::RecordKind;
use canon_model::handoff::{Handoff, HandoffState};

use crate::artifact_adapter::{
    ArtifactAdapter, ArtifactEvent, ArtifactEventKind, ArtifactJoinKey, ArtifactParseOutcome, ArtifactSourceConfig, ArtifactSourceHandle,
};

pub struct HandoffAdapter;

impl HandoffAdapter {
    /// Every state-transition event one `Handoff` snapshot implies,
    /// in transition order (created, then claimed if observed, then
    /// terminal if observed) — design D3's three named transitions.
    /// `state` alone cannot stand in for `claimed_by`'s presence: the donor's
    /// CAS claim (`HandoffState::can_transition_to`) always sets
    /// `claimed_at` on `pending -> in-progress`, but a claim call with no
    /// claimant string leaves `claimed_by` itself `None` — design D3
    /// says "`claimedBy` set = claimed", so this checks the field
    /// directly rather than inferring "claimed" from `state !=
    /// pending`.
    fn events_for(record: &Handoff) -> Vec<ArtifactEvent> {
        let join_key = ArtifactJoinKey::Handoff(record.id.clone());
        let mut events = Vec::with_capacity(3);

        events.push(Self::event(join_key.clone(), record.envelope.at, Self::detail(record, "created")));

        if record.claimed_by.is_some() {
            let at = record.claimed_at.unwrap_or(record.envelope.at);
            events.push(Self::event(join_key.clone(), at, Self::detail(record, "claimed")));
        }

        match record.state {
            HandoffState::Done => {
                let at = record.completed_at.unwrap_or(record.envelope.at);
                events.push(Self::event(join_key, at, Self::detail(record, "done")));
            }
            HandoffState::Abandoned => {
                let at = record.abandoned_at.unwrap_or(record.envelope.at);
                events.push(Self::event(join_key, at, Self::detail(record, "abandoned")));
            }
            HandoffState::Pending | HandoffState::InProgress => {}
        }

        events
    }

    fn event(join_key: ArtifactJoinKey, at: chrono::DateTime<chrono::Utc>, detail: serde_json::Value) -> ArtifactEvent {
        ArtifactEvent {
            adapter_id: "handoff",
            join_key,
            // A handoff transition alone is never a verdict (design D3)
            // — `derive_verdict` always returns `None` for this kind, so
            // `authoring_role`/`area`/`trust_level` (verdict-emission
            // fields this adapter never populates) all stay `None`.
            kind: ArtifactEventKind::NonVerdict,
            authoring_role: None,
            area: None,
            trust_level: None,
            at,
            detail,
        }
    }

    /// The normalized `detail` payload every emitted event carries
    /// (mirrors `ArtifactEvent.detail`'s doc comment: copied verbatim
    /// into the eventual `canon_model::records::Event` conversion).
    /// `change_id` (task 3.2) is present only when the source row
    /// carries an `openspec_change_slug`.
    fn detail(record: &Handoff, transition: &'static str) -> serde_json::Value {
        let mut detail = serde_json::json!({
            "transition": transition,
            "handoff_id": record.id.as_str(),
            "state": record.state,
            "chain_id": record.chain_id,
            "seq": record.seq,
        });
        let obj = detail.as_object_mut().expect("object literal always serializes to a JSON object");
        if let Some(parent) = &record.parent_handoff_id {
            obj.insert("parent_handoff_id".to_string(), serde_json::Value::String(parent.as_str().to_string()));
        }
        if let Some(change) = &record.openspec_change_slug {
            obj.insert("change_id".to_string(), serde_json::Value::String(change.as_str().to_string()));
        }
        if let Some(claimed_by) = &record.claimed_by {
            obj.insert("claimed_by".to_string(), serde_json::Value::String(claimed_by.clone()));
        }
        detail
    }
}

impl ArtifactAdapter for HandoffAdapter {
    fn adapter_id(&self) -> &'static str {
        "handoff"
    }

    /// Handle-based adapter (module doc above, `crate::artifact_adapter`
    /// module doc, design D6): there is no `ArtifactSourceConfig` field
    /// this adapter resolves a source from, so this always returns
    /// `None` — its wave-2 driver constructs an
    /// `ArtifactSourceHandle::Records(..)` directly and calls `parse`
    /// with it, skipping this method entirely.
    fn resolve_source(&self, _config: &ArtifactSourceConfig) -> Option<ArtifactSourceHandle> {
        None
    }

    /// Malformed/unparseable content is skipped AND counted, never a
    /// crash (design §7). A candidate that fails to deserialize as a
    /// `Handoff`, or whose own `kind` field disagrees with
    /// `RecordKind::Handoff`, is one such skip — this adapter's source
    /// is always `ArtifactSourceHandle::Records` in production (see
    /// `resolve_source`'s doc comment); a `Path` handle reaching here at
    /// all is a caller-contract violation, handled the same
    /// never-a-crash way rather than treated as one malformed record.
    fn parse(&self, source: &ArtifactSourceHandle) -> ArtifactParseOutcome {
        let ArtifactSourceHandle::Records(raws) = source else {
            return ArtifactParseOutcome::empty();
        };

        let mut events = Vec::new();
        let mut skipped = 0usize;
        for raw in raws {
            match serde_json::from_value::<Handoff>(raw.0.clone()) {
                Ok(record) if record.envelope.kind == RecordKind::Handoff => events.extend(Self::events_for(&record)),
                _ => skipped += 1,
            }
        }
        ArtifactParseOutcome { events, skipped }
    }
}

#[cfg(test)]
mod tests {
    use canon_model::envelope::{Actor, Envelope, RecordKind};
    use canon_model::evidence::RawRecord;
    use canon_model::handoff::{DomainId, HandoffBody};
    use canon_model::ids::HandoffId;

    use super::*;
    use crate::verdict::derive_verdict;

    fn handoff(id: &str, state: HandoffState, claimed_by: Option<&str>) -> Handoff {
        let at = "2026-07-01T09:00:00Z".parse().unwrap();
        Handoff {
            envelope: Envelope::new(1, RecordKind::Handoff, at, Actor::new_unattributed("s4-fixture")),
            id: HandoffId::parse(id).unwrap(),
            state,
            chain_id: "11111111-1111-4111-8111-111111111111".parse().unwrap(),
            parent_handoff_id: None,
            seq: 1,
            claimed_by: claimed_by.map(str::to_string),
            claimed_at: claimed_by.map(|_| "2026-07-01T09:05:00Z".parse().unwrap()),
            completed_at: matches!(state, HandoffState::Done).then(|| "2026-07-01T09:10:00Z".parse().unwrap()),
            abandoned_at: matches!(state, HandoffState::Abandoned).then(|| "2026-07-01T09:10:00Z".parse().unwrap()),
            openspec_change_slug: None,
            research_vendor_slug: None,
            tags: Vec::new(),
            title: "fixture handoff".to_string(),
            body: HandoffBody { domain: DomainId::parse("planning").unwrap(), template_version: 1, fields: serde_json::json!({}) },
        }
    }

    fn records(handoffs: &[Handoff]) -> ArtifactSourceHandle {
        ArtifactSourceHandle::Records(handoffs.iter().map(|h| RawRecord(serde_json::to_value(h).unwrap())).collect())
    }

    #[test]
    fn resolve_source_is_always_none() {
        assert_eq!(HandoffAdapter.resolve_source(&ArtifactSourceConfig::default()), None);
    }

    #[test]
    fn path_handle_yields_empty_outcome_never_a_crash() {
        let outcome = HandoffAdapter.parse(&ArtifactSourceHandle::Path("nope".into()));
        assert_eq!(outcome, ArtifactParseOutcome::empty());
    }

    #[test]
    fn pending_never_claimed_emits_only_created() {
        let h = handoff("20260701-0900-s4-pending-t1a1", HandoffState::Pending, None);
        let outcome = HandoffAdapter.parse(&records(&[h]));
        assert_eq!(outcome.skipped, 0);
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.events[0].detail["transition"], "created");
    }

    #[test]
    fn in_progress_claimed_emits_created_then_claimed() {
        let h = handoff("20260701-0905-s4-claim-c1a2", HandoffState::InProgress, Some("s4handoff-agent"));
        let outcome = HandoffAdapter.parse(&records(&[h]));
        assert_eq!(outcome.skipped, 0);
        let transitions: Vec<_> = outcome.events.iter().map(|e| e.detail["transition"].as_str().unwrap()).collect();
        assert_eq!(transitions, vec!["created", "claimed"]);
    }

    #[test]
    fn done_emits_created_claimed_done_in_order() {
        let h = handoff("20260701-0910-s4-done-d1a3", HandoffState::Done, Some("s4handoff-agent"));
        let outcome = HandoffAdapter.parse(&records(&[h]));
        let transitions: Vec<_> = outcome.events.iter().map(|e| e.detail["transition"].as_str().unwrap()).collect();
        assert_eq!(transitions, vec!["created", "claimed", "done"]);
    }

    #[test]
    fn abandoned_emits_created_claimed_abandoned_in_order() {
        let h = handoff("20260701-0915-s4-drop-a1a4", HandoffState::Abandoned, Some("s4handoff-agent"));
        let outcome = HandoffAdapter.parse(&records(&[h]));
        let transitions: Vec<_> = outcome.events.iter().map(|e| e.detail["transition"].as_str().unwrap()).collect();
        assert_eq!(transitions, vec!["created", "claimed", "abandoned"]);
    }

    #[test]
    fn handoff_id_is_carried_verbatim_on_every_event() {
        let h = handoff("20260701-0910-s4-done-d1a3", HandoffState::Done, Some("s4handoff-agent"));
        let outcome = HandoffAdapter.parse(&records(&[h]));
        for event in &outcome.events {
            assert_eq!(event.join_key, ArtifactJoinKey::Handoff(HandoffId::parse("20260701-0910-s4-done-d1a3").unwrap()));
            assert_eq!(event.detail["handoff_id"], "20260701-0910-s4-done-d1a3");
        }
    }

    #[test]
    fn openspec_change_slug_is_carried_into_detail_change_id_when_present() {
        let mut h = handoff("20260701-0900-s4-pending-t1a1", HandoffState::Pending, None);
        h.openspec_change_slug = Some(canon_model::ids::ChangeId::parse("s4-artifact-ingest").unwrap());
        let outcome = HandoffAdapter.parse(&records(&[h]));
        assert_eq!(outcome.events[0].detail["change_id"], "s4-artifact-ingest");
    }

    #[test]
    fn no_transition_ever_produces_a_verdict() {
        let handoffs = [
            handoff("20260701-0900-s4-pending-t1a1", HandoffState::Pending, None),
            handoff("20260701-0905-s4-claim-c1a2", HandoffState::InProgress, Some("agent")),
            handoff("20260701-0910-s4-done-d1a3", HandoffState::Done, Some("agent")),
            handoff("20260701-0915-s4-drop-a1a4", HandoffState::Abandoned, Some("agent")),
        ];
        let outcome = HandoffAdapter.parse(&records(&handoffs));
        assert!(!outcome.events.is_empty());
        for event in &outcome.events {
            assert_eq!(event.kind, ArtifactEventKind::NonVerdict);
            assert_eq!(derive_verdict(event.kind, event.authoring_role.as_ref()), None);
        }
    }

    #[test]
    fn a_record_whose_envelope_kind_disagrees_is_skipped_not_crashed() {
        let mut json = serde_json::to_value(handoff("20260701-0900-s4-pending-t1a1", HandoffState::Pending, None)).unwrap();
        json["kind"] = serde_json::Value::String("session".to_string());
        let outcome = HandoffAdapter.parse(&ArtifactSourceHandle::Records(vec![RawRecord(json)]));
        assert_eq!(outcome.skipped, 1);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn malformed_json_is_skipped_not_crashed() {
        let outcome = HandoffAdapter.parse(&ArtifactSourceHandle::Records(vec![RawRecord(serde_json::json!({"not": "a handoff"}))]));
        assert_eq!(outcome.skipped, 1);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn reparsing_an_unchanged_batch_is_idempotent() {
        let handoffs = [
            handoff("20260701-0900-s4-pending-t1a1", HandoffState::Pending, None),
            handoff("20260701-0910-s4-done-d1a3", HandoffState::Done, Some("agent")),
        ];
        let source = records(&handoffs);
        let first = HandoffAdapter.parse(&source);
        let second = HandoffAdapter.parse(&source);
        assert_eq!(first, second);
    }
}
