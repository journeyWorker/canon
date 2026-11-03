//! The plan digest test vectors of the contract repository (`examples/plan-digest/`, atip v1.2.1, spec 12.6), run against
//! `atip::jcs` and `atip::plan`: the RFC 8785 text of each plan and its `sha256:` digest.
//!
//! The vectors are vendored byte for byte under `tests/fixtures/plan-digest/` and pinned by the SHA-256 of each file, so a
//! changed or missing vector fails here instead of silently weakening the test. They bind no tool that computes its digest
//! another way (12.6 is a SHOULD); they bind this crate, which says it follows the recommendation.

use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

use atip::plan::{digest_of, Plan};

/// The contract tag the vectors come from and the commit it names.
const TAG: &str = "v1.2.1";
const TAG_COMMIT: &str = "f61f2e02d461766d2b686ba23f6be2e1f2fb7d0d";

/// Every vector with the SHA-256 of its bytes at `TAG`.
const VECTORS: [(&str, &str); 9] = [
    ("empty-effects.json", "ca2fa8253158f42c72933c2b984d43f7cb25cfa9d65273634ea51d8c4fcb5a1b"),
    ("key-order-utf16.json", "11accb49bf6f89ebfc5db9c8e316a2b63e06ab19d3b18cfa06630a8c5b50f5df"),
    ("key-order.json", "48d5176f287ff9fe314d619add8cc1931194a55135fee687305b5c5143208587"),
    ("large-integers-as-strings.json", "5a9b20d0dfde1a3d1a235cd3d69106bf1157850a288b52c4a5293eacdd8006d4"),
    ("nested.json", "f30a6674007d7f2dc2e2adbbf562c762afb0fc21d844daed09246ba5960f9207"),
    ("numbers-integral.json", "471703755bcb23ab76e0397524258d8e242cc5cff8129fcb425580d020e73235"),
    ("numbers-other.json", "6845971d9a35cd4b16c5c595e809f82a658c4dd73ba9c5635694fc142fed0880"),
    ("preview.json", "5b8621aaa6fb58f0bdd96c2918129e1f26da16099cda8be1c0749962f5055a05"),
    ("strings.json", "1ba1d532c20c3eae774d4c04970d2774f4a6fc5ddd80dc09a46bbffba5b12700"),
];

fn directory() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plan-digest")
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn vector(name: &str) -> Value {
    let bytes = std::fs::read(directory().join(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"))
}

#[test]
fn atip_crateplan_12_the_vendored_vectors_are_the_ones_of_the_pinned_contract_tag_byte_for_byte() {
    assert_eq!((TAG, TAG_COMMIT.len()), ("v1.2.1", 40));
    for (name, digest) in VECTORS {
        let bytes = std::fs::read(directory().join(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(hex(&bytes), digest, "{name} differs from {TAG}");
    }
    // No vector is added or left out: the directory holds exactly the pinned files.
    let mut found: Vec<String> = std::fs::read_dir(directory()).expect("the fixtures").map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned()).collect();
    found.sort();
    let mut pinned: Vec<String> = VECTORS.iter().map(|(name, _)| (*name).to_string()).collect();
    pinned.sort();
    assert_eq!(found, pinned);
}

#[test]
fn atip_crateplan_12_every_vector_has_the_canonical_text_and_the_digest_the_contract_gives() {
    for (name, _) in VECTORS {
        let document = vector(name);
        let plan = &document["plan"];
        let canonical = atip::jcs::to_string(plan).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(canonical, document["canonical"].as_str().expect("a canonical text"), "{name}: the RFC 8785 text");
        assert_eq!(digest_of(plan).unwrap_or_else(|error| panic!("{name}: {error}")).as_str(), document["digest"].as_str().expect("a digest"), "{name}: the digest");
    }
}

#[test]
fn atip_crateplan_12_a_plan_of_the_crates_own_type_has_the_same_digest_for_every_vector() {
    for (name, _) in VECTORS {
        let document = vector(name);
        let plan: Plan = serde_json::from_value(document["plan"].clone()).unwrap_or_else(|error| panic!("{name}: the vector is a Plan: {error}"));
        assert_eq!(plan.digest().unwrap_or_else(|error| panic!("{name}: {error}")).as_str(), document["digest"].as_str().expect("a digest"), "{name}");
        // The report of its preview carries that digest.
        assert_eq!(plan.preview().expect("a preview").plan_digest, document["digest"].as_str().expect("a digest"), "{name}");
    }
}

#[test]
fn atip_crateplan_12_an_integer_beyond_the_safe_range_is_refused_and_its_string_is_the_vectors_form() {
    let document = vector("large-integers-as-strings.json");
    assert_eq!(document["plan"]["inputs"]["since"], "12345678901234567890", "the vector holds such an integer as a string");
    // The same plan with the integers as numbers is refused, never digested to something another implementation would not.
    let as_numbers: Value = serde_json::from_str(r#"{"effects":[{"action":"update","record":"18446744073709551615"}],"inputs":{"since":12345678901234567890,"sequence":9007199254740993,"previous":-9223372036854775808}}"#).expect("JSON");
    assert!(digest_of(&as_numbers).is_err(), "a 64-bit integer beyond ±(2^53-1) is not a plan input");
    let sequence: Value = serde_json::from_str(r#"{"effects":[],"inputs":{"sequence":9007199254740993}}"#).expect("JSON");
    assert!(matches!(digest_of(&sequence), Err(atip::jcs::Error::NumberNotSafe(_))));
}
