//! Every one of canon-model's fourteen record kinds round-trips through
//! `GitTier` using the SAME well-formed fixture corpus S1 already ships
//! (`crates/canon-model/fixtures/well-formed/*.json`) — reusing S1's
//! fixtures rather than a second hand-authored set keeps the two crates'
//! notion of "a well-formed record" from silently drifting apart
//! (task 2.4's "one well-formed record per area-scoped and non-area-
//! scoped kind", generalized to all kinds since the fixtures already
//! exist for free).

use canon_model::envelope::RecordKind;
use canon_model::RawRecord;
use canon_store::git_tier::GitTier;
use canon_store::tier::{RawWrite, Tier, TierQuery};

fn fixtures_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../canon-model/fixtures/well-formed")
}

#[test]
fn every_well_formed_fixture_round_trips_through_git_tier() {
    let dir = tempfile::tempdir().unwrap();
    let tier = GitTier::new(dir.path());

    let mut wrote = 0;
    for kind in RecordKind::ALL {
        let path = fixtures_dir().join(format!("{}.json", kind.as_str()));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("reading fixture {}: {e}", path.display()));
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let raw = RawRecord(json.clone());

        let receipt = tier.write(&RawWrite(raw)).unwrap_or_else(|e| panic!("writing {} fixture: {e}", kind.as_str()));
        assert_eq!(receipt.kind, kind);
        assert!(!receipt.deduped);
        wrote += 1;

        let result = tier.read(&TierQuery::kind(kind)).unwrap_or_else(|e| panic!("reading {} back: {e}", kind.as_str()));
        assert!(result.violations.is_empty(), "{}: unexpected violations {:?}", kind.as_str(), result.violations);
        assert_eq!(result.records.len(), 1, "{}: expected exactly one record back", kind.as_str());
        assert_eq!(result.records[0].0, json, "{}: round-tripped content must equal what was written", kind.as_str());
    }

    assert_eq!(wrote, RecordKind::ALL.len(), "every RecordKind::ALL kind must have been exercised");
}

#[test]
fn area_scoped_kinds_land_under_area_and_flat_kinds_do_not() {
    let dir = tempfile::tempdir().unwrap();
    let tier = GitTier::new(dir.path());

    for kind in RecordKind::ALL {
        let path = fixtures_dir().join(format!("{}.json", kind.as_str()));
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let receipt = tier.write(&RawWrite(RawRecord(json))).unwrap();
        let has_area_segment = receipt.location.contains("/area=");
        assert_eq!(
            has_area_segment,
            kind.is_area_scoped(),
            "{}: location {:?} area-segment presence must match RecordKind::is_area_scoped()",
            kind.as_str(),
            receipt.location
        );
    }
}

/// s43: the well-formed `finding.json` fixture exercises every field,
/// which means it names a `reviewed_sha`. The COMMON case does not — a
/// review round usually reads an uncommitted working tree — so the
/// shape the fixture cannot cover gets its own end-to-end write/read,
/// proving `resolve_partition` keys off `change_id` and never needs a
/// commit to store a finding.
#[test]
fn a_finding_whose_round_reviewed_a_worktree_round_trips_through_git_tier() {
    let dir = tempfile::tempdir().unwrap();
    let tier = GitTier::new(dir.path());

    let json = serde_json::json!({
        "schema": 1,
        "kind": "finding",
        "at": "2026-07-30T09:15:00Z",
        "actor": {"agent_id": "codex-cli", "role": "reviewer"},
        "change_id": "s42-close-the-open-loops",
        "round": 8,
        "seq": 10,
        "severity": "should_fix",
        "disposition": "open",
        "reviewer": "review-voice",
        "summary": "round 8 reviewed a working tree that was never committed"
    });

    let receipt = tier.write(&RawWrite(RawRecord(json.clone()))).expect("a finding with no reviewed_sha must be storable");
    assert_eq!(receipt.kind, RecordKind::Finding);
    assert!(
        receipt.location.contains("s42-close-the-open-loops__0008__0010__"),
        "the natural key is change-keyed, never sha-keyed: {}",
        receipt.location
    );
    assert!(!receipt.location.contains("/area="), "a finding is never area-scoped");

    let result = tier.read(&TierQuery::kind(RecordKind::Finding)).expect("reading the finding back");
    assert!(result.violations.is_empty(), "unexpected violations {:?}", result.violations);
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].0, json, "round-tripped content must equal what was written");
}
