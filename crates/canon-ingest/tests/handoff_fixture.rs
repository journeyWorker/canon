//! S4 wave-2 handoff-adapter fixture test (design.md D3, task 3.1/3.2,
//! assignment acceptance: "Fixture: FROZEN Handoff records"). The four
//! `tests/fixtures/handoffs/*.json` files are a checked-in, point-in-time
//! export of canon's OWN `Handoff` records — never a live query (design
//! §8 risk mitigation: "the S4 fixture corpus captures a point-in-time
//! export... never a live connection during tests").
//!
//! This test writes that frozen corpus through a real `canon_store::Tier`
//! (`GitTier`, rooted at a fresh `tempfile::tempdir()`) and reads it back
//! via `Tier::read` — the exact `canon-store`-tier round trip design D3
//! names ("reads canon's own `Handoff` table... via `canon-store`'s
//! Postgres tier `Tier::read`"), substituting the git tier for the
//! Postgres tier so the test is fully offline and deterministic (no
//! docker, no network — `Tier` is one trait, three interchangeable
//! adapters, S2 design D1; which concrete tier canon's own handoffs
//! table is actually routed through in production `canon.yaml` is a
//! wave-2 CLI-wiring concern, out of `canon-ingest`'s scope). The
//! `Vec<RawRecord>` `Tier::read` returns is handed to
//! `HandoffAdapter::parse` unchanged — the identical shape a real
//! `PgTier::read` call would hand a wave-2 driver.

use std::path::PathBuf;

use canon_ingest::artifact_adapters::handoff::HandoffAdapter;
use canon_ingest::{ArtifactAdapter, ArtifactJoinKey, ArtifactSourceHandle};
use canon_model::envelope::RecordKind;
use canon_model::handoff::Handoff;
use canon_model::ids::HandoffId;
use canon_store::git_tier::GitTier;
use canon_store::tier::{Tier, TierQuery};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/handoffs")
}

/// Load every frozen fixture, write each through a fresh tempdir
/// `GitTier`, and read the whole `kind=handoff/` batch back — the
/// point-in-time-export-through-canon-store's-own-tier round trip this
/// module doc describes.
fn write_and_read_fixture_corpus() -> Vec<canon_model::evidence::RawRecord> {
    let dir = tempfile::tempdir().unwrap();
    let tier = GitTier::new(dir.path());

    for entry in std::fs::read_dir(fixtures_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let handoff: Handoff = serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        tier.write(&handoff).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }

    let result = tier.read(&TierQuery::kind(RecordKind::Handoff)).unwrap();
    assert!(result.violations.is_empty(), "fixture corpus must round-trip cleanly: {:?}", result.violations);
    result.records
}

#[test]
fn frozen_fixture_corpus_has_exactly_the_four_checked_in_handoffs() {
    let records = write_and_read_fixture_corpus();
    assert_eq!(records.len(), 4, "expected one record per tests/fixtures/handoffs/*.json file");
}

#[test]
fn every_state_transition_the_fixture_corpus_implies_is_emitted_and_no_verdict_is_ever_produced() {
    let records = write_and_read_fixture_corpus();
    let outcome = HandoffAdapter.parse(&ArtifactSourceHandle::Records(records));

    assert_eq!(outcome.skipped, 0, "every frozen fixture is well-formed");
    // pending-never-claimed: created (1)
    // in-progress-claimed:   created, claimed (2)
    // done:                  created, claimed, done (3)
    // abandoned:              created, claimed, abandoned (3)
    assert_eq!(outcome.events.len(), 1 + 2 + 3 + 3);

    for event in &outcome.events {
        // A handoff transition alone is never a verdict (design D3).
        assert_eq!(canon_ingest::derive_verdict(event.kind, event.authoring_role.as_ref()), None);
    }

    // handoff_id carried verbatim (design D3, task 3.2): every event's
    // join key round-trips through the exact id its source fixture
    // named, never a re-derived identity.
    let done_id = HandoffId::parse("20260701-0910-s4-done-d1a3").unwrap();
    let done_events: Vec<_> = outcome.events.iter().filter(|e| e.join_key == ArtifactJoinKey::Handoff(done_id.clone())).collect();
    assert_eq!(done_events.len(), 3);
    for event in &done_events {
        assert_eq!(event.detail["handoff_id"], "20260701-0910-s4-done-d1a3");
    }
    let transitions: Vec<_> = done_events.iter().map(|e| e.detail["transition"].as_str().unwrap()).collect();
    assert_eq!(transitions, vec!["created", "claimed", "done"]);

    // openspec_change_slug carried into detail.change_id when present
    // (task 3.2's second half) — three of the four fixtures set it,
    // `abandoned.json` deliberately does not.
    let claimed_id = HandoffId::parse("20260701-0905-s4-claim-c1a2").unwrap();
    let claimed_created = outcome
        .events
        .iter()
        .find(|e| e.join_key == ArtifactJoinKey::Handoff(claimed_id.clone()) && e.detail["transition"] == "created")
        .unwrap();
    assert_eq!(claimed_created.detail["change_id"], "s4-artifact-ingest");

    let abandoned_id = HandoffId::parse("20260701-0915-s4-drop-a1a4").unwrap();
    let abandoned_created = outcome
        .events
        .iter()
        .find(|e| e.join_key == ArtifactJoinKey::Handoff(abandoned_id.clone()) && e.detail["transition"] == "created")
        .unwrap();
    assert!(abandoned_created.detail.get("change_id").is_none(), "abandoned.json carries no openspec_change_slug");
}

#[test]
fn reingesting_the_unchanged_fixture_corpus_twice_is_idempotent() {
    let first_records = write_and_read_fixture_corpus();
    let first = HandoffAdapter.parse(&ArtifactSourceHandle::Records(first_records));

    // A second, independent write+read of the identical frozen corpus
    // into a SECOND tempdir tier — not just re-parsing the same
    // in-memory `Vec` — proves the adapter's output is a pure function
    // of the fixture's own content, not incidentally stable because
    // nothing re-ran the tier round trip.
    let second_records = write_and_read_fixture_corpus();
    let second = HandoffAdapter.parse(&ArtifactSourceHandle::Records(second_records));

    assert_eq!(first, second, "re-ingesting an unchanged fixture corpus must never emit a new/duplicate/reordered event");
}
