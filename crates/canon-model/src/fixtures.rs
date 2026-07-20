//! Fixture corpus round-trip + malformed-evidence test (task 6.2,
//! `canon_model::fixtures::round_trip_all` equivalent): every
//! well-formed fixture under `fixtures/well-formed/` round-trips
//! through its record kind's own `Deserialize`/`Serialize`; every
//! malformed fixture under `fixtures/malformed/` produces exactly its
//! `fixtures/EXPECTED-violations.json`-declared [`FailureClass`] — this
//! is the design doc's S1 "schema crate round-trips all fixture
//! corpora" acceptance bar.

use std::path::PathBuf;

use crate::envelope::RecordKind;
use crate::evidence::{FailureClass, RawRecord, validate_envelope_shape, validate_evidence};
use crate::handoff::{DomainId, GihoekTemplate, Handoff, HandoffState, TemplateRegistry};
use crate::records::*;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// Deserialize a well-formed fixture into its declared kind's concrete
/// type and assert a lossless round trip — the per-kind dispatch every
/// caller needs instead of re-deriving a `kind` ↔ type mapping.
fn round_trip_well_formed(kind: RecordKind, json: &serde_json::Value) {
    macro_rules! check {
        ($ty:ty) => {{
            let value: $ty = serde_json::from_value(json.clone()).unwrap_or_else(|e| panic!("{kind:?} fixture failed to deserialize: {e}"));
            let re_serialized = serde_json::to_value(&value).unwrap();
            assert_eq!(&re_serialized, json, "{kind:?} fixture did not round-trip losslessly");
        }};
    }
    match kind {
        RecordKind::Change => check!(Change),
        RecordKind::Task => check!(Task),
        RecordKind::Scenario => check!(Scenario),
        RecordKind::Session => check!(Session),
        RecordKind::Run => check!(Run),
        RecordKind::Event => check!(Event),
        RecordKind::Handoff => check!(Handoff),
        RecordKind::Review => check!(Review),
        RecordKind::Divergence => check!(Divergence),
        RecordKind::Trajectory => check!(Trajectory),
        RecordKind::StrategyItem => check!(StrategyItem),
        RecordKind::EvidenceRecord => check!(EvidenceRecord),
        RecordKind::Subject => check!(Subject),
    }
}

/// Validate one malformed fixture, dispatching to whichever
/// `canon-model` validation entry point that fixture actually exercises
/// (there is deliberately no single "validate any of the twelve kinds"
/// function — `validate_evidence` is `EvidenceRecord`-scoped per its own
/// spec, and a state-transition/template-registry violation isn't an
/// evidence-shape problem at all). Returns the produced [`FailureClass`].
fn validate_malformed_fixture(filename: &str, json: &serde_json::Value) -> FailureClass {
    match filename {
        "missing-actor.json" => validate_envelope_shape(&RawRecord(json.clone())).expect_err("expected a violation").class,
        "invalid-scenario-id.json" => validate_evidence(&RawRecord(json.clone())).expect_err("expected a violation").class,
        "invalid-state-transition.json" => {
            let from: HandoffState = serde_json::from_value(json["from"].clone()).unwrap();
            let to: HandoffState = serde_json::from_value(json["to"].clone()).unwrap();
            assert!(!from.can_transition_to(to), "fixture claims an invalid transition that HandoffState actually allows");
            FailureClass::InvalidStateTransition
        }
        "unregistered-handoff-domain.json" => {
            let handoff: Handoff = serde_json::from_value(json.clone()).expect("handoff fixture itself must be well-formed");
            let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let canon_yaml = std::fs::read_to_string(repo_root.join("canon.yaml")).expect("repo root canon.yaml");
            let registry = TemplateRegistry::from_manifest(&canon_yaml, vec![Box::new(GihoekTemplate)]).unwrap();
            assert!(!registry.is_registered(&handoff.body.domain), "fixture's domain is unexpectedly registered");
            registry.validate_body(&handoff.body).expect_err("expected a violation").class
        }
        other => panic!("no validator wired up for malformed fixture `{other}` — add a case to validate_malformed_fixture"),
    }
}

/// `canon_model::fixtures::round_trip_all` (task 6.2): round-trips
/// every well-formed fixture (one per record kind) and asserts every
/// malformed fixture produces exactly its `EXPECTED-violations.json`
/// `FailureClass`.
#[test]
fn round_trip_all() {
    let well_formed_dir = fixtures_dir().join("well-formed");
    let mut seen_kinds = std::collections::HashSet::new();
    let entries: Vec<_> = std::fs::read_dir(&well_formed_dir).unwrap().map(|e| e.unwrap()).collect();
    assert_eq!(entries.len(), 13, "expected exactly one well-formed fixture per record kind");

    for entry in entries {
        let text = std::fs::read_to_string(entry.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let kind_str = json.get("kind").and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{:?} fixture has no `kind`", entry.path()));
        let kind = RecordKind::ALL
            .into_iter()
            .find(|k| k.as_str() == kind_str)
            .unwrap_or_else(|| panic!("{:?} fixture's kind `{kind_str}` is not one of the twelve closed kinds", entry.path()));
        assert!(seen_kinds.insert(kind), "{kind:?} has more than one well-formed fixture");
        round_trip_well_formed(kind, &json);
    }
    for kind in RecordKind::ALL {
        assert!(seen_kinds.contains(&kind), "{kind:?} has no well-formed fixture");
    }

    let expected: std::collections::HashMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(fixtures_dir().join("EXPECTED-violations.json")).unwrap()).unwrap();
    assert_eq!(expected.len(), 4, "expected exactly the four documented malformed variants");

    let malformed_dir = fixtures_dir().join("malformed");
    let malformed_entries: Vec<_> = std::fs::read_dir(&malformed_dir).unwrap().map(|e| e.unwrap()).collect();
    assert_eq!(malformed_entries.len(), expected.len(), "malformed/ directory and EXPECTED-violations.json disagree on fixture count");

    for entry in malformed_entries {
        let filename = entry.file_name().to_string_lossy().into_owned();
        let expected_class_str = expected.get(&filename).unwrap_or_else(|| panic!("{filename} has no EXPECTED-violations.json entry"));
        let expected_class = FailureClass::from_str_exact(expected_class_str).unwrap_or_else(|| panic!("{expected_class_str} is not a known FailureClass"));

        let text = std::fs::read_to_string(entry.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let actual_class = validate_malformed_fixture(&filename, &json);
        assert_eq!(actual_class, expected_class, "{filename} produced {actual_class:?}, expected {expected_class:?}");
    }
}

// A stray `DomainId` import guard: keeps this module honest that
// `unregistered-handoff-domain.json`'s domain really is a `DomainId`,
// not an arbitrary string comparison.
#[test]
fn unregistered_domain_fixture_is_a_valid_domain_id_shape() {
    let text = std::fs::read_to_string(fixtures_dir().join("malformed/unregistered-handoff-domain.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let domain = json["body"]["domain"].as_str().unwrap();
    assert!(DomainId::parse(domain).is_ok());
}
