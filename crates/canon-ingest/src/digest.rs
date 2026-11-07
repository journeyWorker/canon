//! The 12-hex-character content digest that names plan cursor files
//! (`.canon/ingest/cursors/plan-<dialect>-<digest>.json`) and keys the
//! regime hash and trajectory identity of `canon ingest records`.
//!
//! Stored `regime_key`s and cursor files embed these values, so the
//! output of [`content_digest`] MUST NOT change; the fixed vectors in
//! this module's tests pin it.

use sha2::{Digest, Sha256};

/// First 6 bytes of the SHA-256 of `value`'s canonical JSON, as 12
/// lowercase hex characters.
///
/// `serde_json::Value` serializes `Map` as a `BTreeMap` (no
/// `preserve_order` feature anywhere in this workspace; the same
/// invariant `canon-store::partition::content_digest` relies on), so key
/// order never perturbs the digest. This is a different function from
/// `canon-store`'s 8-character `content_digest`.
pub fn content_digest(value: &serde_json::Value) -> String {
    let canonical = serde_json::to_vec(value).expect("serde_json::Value always serializes");
    let digest = Sha256::digest(&canonical);
    digest.iter().take(6).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Vectors computed independently from the 0.38 function's definition
    /// (sha256 of compact, key-sorted JSON, first 12 hex chars).
    #[test]
    fn content_digest_is_pinned_to_fixed_vectors() {
        let cases = [
            (json!("abc"), "6cc43f858fbb"),
            (json!("/Users/x/ws"), "3e582dffb2d6"),
            (json!({"b": [1, 2], "a": null}), "ee743f2fa257"),
            (json!({"k": "한글", "n": 1.5, "t": true}), "e6ea1cbcc995"),
            (json!(42), "73475cb40a56"),
        ];
        for (value, expected) in cases {
            assert_eq!(content_digest(&value), expected, "digest of {value}");
        }
    }
}
