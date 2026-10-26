//! The git-tier name of a record: its components from its own content,
//! the canonical content digest, and the write/read-shared layout check
//! (git-tier-layout-enforcement spec; S2 design D2; 0.30
//! `ledger-naming`, `docs/design/ledger-layout.md`).
//!
//! `canon-model`'s [`canon_model::layout`] is the component TABLE and the
//! path builder; everything here is the INTERPRETATION half: which digest a
//! body gets, which relative path or row key results, and whether a found
//! file sits where its own content says it must.
//!
//! Every store names a record the same way:
//! `kind=<kind>/<component>_<component>…_<digest8>.json` (git ledger,
//! archive, `_staging`, plugin overlay), `(kind, <key>, <digest8>)` (sqlite
//! and Postgres rows), `<prefix>kind=<kind>/<…>_<digest8>.parquet` (R2). The
//! 8-hex digest makes a byte-identical resubmission resolve to the SAME
//! name, while a logically different record with the same key resolves to a
//! different one (a genuine append). A same key with different content is
//! never silent in any store (R7).

use canon_model::envelope::RecordKind;
use canon_model::evidence::EvidenceViolation;
use canon_model::layout::{self, Component, Name};
use canon_model::{FailureClass, RawRecord};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A record's resolved name coordinates, extracted from its own content
/// (never a source directory — the load-bearing invariant this whole
/// module exists to enforce).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionKey {
    /// The typed components in table order (`canon_model::layout`).
    pub components: Vec<Component>,
    /// The components as the user reads them, joined by `_` and never
    /// escaped: the key folds group by, and the one query, cursors and error
    /// messages print. The escaped form a store names the record by is
    /// [`resolve_name`].
    pub natural_key: String,
}

fn malformed(subject: &str, detail: impl Into<String>) -> EvidenceViolation {
    EvidenceViolation::new(FailureClass::Malformed, subject, detail)
}

fn get_str<'a>(json: &'a Value, field: &str) -> Result<&'a str, EvidenceViolation> {
    json.get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| malformed(field, format!("missing or non-string `{field}` field")))
}

/// Extract this record's [`PartitionKey`] from its own body — the ONLY
/// input; never the directory it happened to be found in. `kind` is the
/// caller's already-confirmed kind (the containing `kind=<kind>/` directory
/// for a read, or `T::KIND` for a write) —
/// [`validate_kind_matches_content`] is the separate check that the JSON
/// body's own `kind` field agrees with it.
pub fn resolve_partition(kind: RecordKind, json: &Value) -> Result<PartitionKey, EvidenceViolation> {
    let components = layout::components(kind, json)?;
    let natural_key = layout::display_key(&components);
    Ok(PartitionKey { components, natural_key })
}

/// The JSON body's own `kind` field must agree with the caller's
/// already-known kind (a containing `kind=<kind>/` directory on read, or
/// `T::KIND` on write) — a directory/content kind mismatch is itself a
/// layout violation, distinct from (and checked before) path-shape
/// agreement.
pub fn validate_kind_matches_content(expected: RecordKind, json: &Value) -> Result<(), EvidenceViolation> {
    let found = get_str(json, "kind")?;
    if found != expected.as_str() {
        return Err(malformed("layout", format!("directory `kind={}/` but record body's own `kind` is `{found}`", expected.as_str())));
    }
    Ok(())
}

/// First 8 lowercase-hex characters of SHA-256 over the canonical
/// (alphabetical-key, since `serde_json::Value::Object` is a `BTreeMap`
/// by default — no `preserve_order` feature anywhere in this workspace)
/// JSON serialization of `json`: the digest every name ends in (R7), and
/// the identity digest-based idempotence (`canon tier age`) compares.
pub fn content_digest(json: &Value) -> String {
    let bytes = serde_json::to_vec(json).expect("serde_json::Value always serializes");
    let hash = Sha256::digest(&bytes);
    let mut hex = String::with_capacity(layout::DIGEST_LEN);
    for byte in hash.iter().take(layout::DIGEST_LEN / 2) {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// The total order every place that orders records uses (R5): the body's
/// `at`, then its typed name components in table order, then the body
/// digest. Never the file name or the order a store happened to scan in.
pub fn record_order(kind: RecordKind, raw: &RawRecord) -> (chrono::DateTime<chrono::Utc>, Vec<Component>, String) {
    (crate::tier::raw_record_at(raw), layout::components(kind, &raw.0).unwrap_or_default(), content_digest(&raw.0))
}

/// The escaped key and digest every store names this record by.
pub fn resolve_name(kind: RecordKind, json: &Value) -> Result<Name, EvidenceViolation> {
    validate_kind_matches_content(kind, json)?;
    let components = layout::components(kind, json)?;
    Ok(layout::name(&components, &content_digest(json)))
}

/// The full relative object key a record's OWN content resolves to —
/// `kind={kind}/{key}_{digest8}.{extension}` — the shared coordinate scheme
/// `GitTier` (extension `"json"`, a filesystem path) and `R2Tier`
/// (extension `"parquet"`, an object store key) both compute identically at
/// write time (where each tier writes exactly here) and read time (where a
/// found file/object's actual key is compared against this). Divergence
/// between the two is precisely a layout violation.
pub fn hive_object_key(kind: RecordKind, json: &Value, extension: &str) -> Result<std::path::PathBuf, EvidenceViolation> {
    Ok(resolve_name(kind, json)?.relative_path(kind.as_str(), extension))
}

/// `GitTier`'s `.json`-specific alias of [`hive_object_key`] — kept as
/// a separate name at git-tier call sites since "the path a git-tier
/// record resolves to" is the concept those call sites reason about,
/// not "some object key at some extension".
pub fn expected_relative_path(kind: RecordKind, json: &Value) -> Result<std::path::PathBuf, EvidenceViolation> {
    hive_object_key(kind, json, "json")
}

/// The 0.29 directory of evidence records (0.30 names the kind `evidence`).
pub const LEGACY_KIND_DIR: &str = "kind=evidence_record";

/// Whether `relative` (a path under a tier root) is shaped like a 0.29
/// name: an `area=` level, a `__<digest12>` stem, or the `evidence_record`
/// directory. Such a file is what `canon migrate` moves; a reader reports it
/// as [`FailureClass::OldLayout`] and never uses it for a verdict.
pub fn is_old_layout_path(relative: &std::path::Path) -> bool {
    let mut components = relative.components().filter_map(|c| c.as_os_str().to_str());
    components.any(|c| c == LEGACY_KIND_DIR || c.starts_with("area="))
        || relative.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".json")).is_some_and(is_old_stem)
}

/// `<key>__<12 lowercase hex>`: the 0.29 stem.
pub fn is_old_stem(stem: &str) -> bool {
    stem.rsplit_once("__").is_some_and(|(_, digest)| digest.len() == 12 && digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

/// Validate that `actual_path` (relative to the git tier's `root`) is
/// EXACTLY the path `json`'s own content resolves to for `kind` — the
/// git-tier-layout-enforcement spec's core check, shared by
/// `GitTier::write` (self-consistency: the path it is about to write to)
/// and `GitTier::read` (a found file's actual path). A mismatch names
/// both the expected path and the rejected path; a 0.29-shaped file is
/// the dedicated [`FailureClass::OldLayout`] with the `canon migrate` hint.
pub fn validate_layout(kind: RecordKind, actual_path: &std::path::Path, json: &Value) -> Result<(), EvidenceViolation> {
    let expected = expected_relative_path(kind, json)?;
    if expected != actual_path {
        if is_old_layout_path(actual_path) {
            return Err(old_layout(actual_path));
        }
        return Err(malformed(
            "layout",
            format!("expected `{}` (template `{}`), found `{}`", expected.display(), kind.partition_template(), actual_path.display()),
        ));
    }
    Ok(())
}

/// The violation of a file named by the 0.29 rule (F2): its hint is
/// `canon migrate`, and the file is not used for any judgement.
pub fn old_layout(actual_path: &std::path::Path) -> EvidenceViolation {
    EvidenceViolation::new(
        FailureClass::OldLayout,
        actual_path.display().to_string(),
        "named by the 0.29 rule (an `area=` level, a 12-hex digest or `kind=evidence_record/`); canon 0.30 does not read it. Hint: run `canon migrate` (a bare run only previews)",
    )
}

/// `canon_model::validate_evidence`/`validate_evidence_batch` are the
/// generic-envelope-plus-`EvidenceRecord`-body gate; every OTHER kind's
/// full-body shape is checked by attempting the matching concrete
/// `CanonRecord` type's own `Deserialize` — mirrors
/// `canon_model::fixtures::round_trip_well_formed`'s per-kind dispatch
/// (S1), reused here as the git-tier read path's content-malformed gate.
pub fn validate_body(kind: RecordKind, raw: &RawRecord) -> Result<(), EvidenceViolation> {
    use canon_model::records::{Change, Task, Scenario, Session, Run, Event, Review, Divergence, Trajectory, StrategyItem, Subject, Finding};
    use canon_model::{Handoff};

    macro_rules! try_deserialize {
        ($ty:ty) => {
            serde_json::from_value::<$ty>(raw.0.clone())
                .map(|_| ())
                .map_err(|e| EvidenceViolation::new(FailureClass::Malformed, "<candidate>", e.to_string()))
        };
    }

    match kind {
        RecordKind::Change => try_deserialize!(Change),
        RecordKind::Task => try_deserialize!(Task),
        RecordKind::Scenario => try_deserialize!(Scenario),
        RecordKind::Session => try_deserialize!(Session),
        RecordKind::Run => try_deserialize!(Run),
        RecordKind::Event => try_deserialize!(Event),
        RecordKind::Handoff => try_deserialize!(Handoff),
        RecordKind::Review => try_deserialize!(Review),
        RecordKind::Divergence => try_deserialize!(Divergence),
        RecordKind::Trajectory => try_deserialize!(Trajectory),
        RecordKind::StrategyItem => try_deserialize!(StrategyItem),
        // `EvidenceRecord` reuses S1's own dedicated validator directly
        // (S2 assignment's S1-interface note: "the ledger-read loop
        // calls it directly") rather than re-deriving the same
        // scenario_id-grammar + verdict-enum checks a second time.
        RecordKind::Evidence => canon_model::validate_evidence(raw).map(|_| ()),
        RecordKind::Subject => try_deserialize!(Subject),
        // s43: the `disposition` ⇔ `resolution_sha` biconditional is
        // enforced by `Finding`'s OWN `Deserialize` (canon-model keeps
        // `resolution_sha` a FLAT field so the fix-of-fix join reads it
        // top-level, rather than folding it into a `Fixed { .. }`
        // variant, so the pair is checked rather than made
        // unrepresentable). `from_body` is that same check with the
        // STRUCTURED violation kept: an incoherent hand-authored record
        // is `malformed` against `resolution_sha` BY NAME, never a
        // generic deserialize-failed string, and never a fix nobody can
        // name a closing commit for.
        RecordKind::Finding => Finding::from_body(&raw.0).map(|_| ()),
    }
}

/// `true` iff `s` matches `[a-z0-9]+(-[a-z0-9]+)*` -- the SAME
/// kebab-token grammar `canon_plugin::manifest::grammar::is_kebab_token`
/// validates a manifest's `namespace`/overlay `kind` against at
/// RESOLUTION time (s16 design.md D2/R4). Reimplemented here
/// independently -- `canon-store` never depends on `canon-plugin` (P2's
/// overlay-write path is the CONSUMER of `canon-store`, never the other
/// way around) -- so [`validate_namespaced_kind`] can reject a malformed
/// namespaced-kind string as write-time defense in depth even if a
/// manifest somehow bypassed the resolution-time check.
fn is_kebab_token(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// [`crate::git_tier::GitTier::write_namespaced`]/
/// [`crate::git_tier::GitTier::scan_namespaced_kind`]'s shared
/// `namespaced_kind` guard (s16 tasks.md 2.3, design.md D4/R5 defense in
/// depth alongside canon-plugin's OWN resolution-time check). REJECTS,
/// before any filesystem path is built: (a) a `namespaced_kind` equal to
/// a core [`RecordKind::as_str()`] value -- so a misconfigured manifest
/// can never alias a core `kind=<x>/` directory even after bypassing
/// resolution-time validation -- and (b) a `namespaced_kind` that is not
/// EXACTLY two dot-joined kebab tokens (`plugin-overlay-records` spec:
/// "`<namespace>.<kind>` is exactly two dot-joined kebab tokens -- no
/// `/`, `..`, or other path separator is ever constructible from a
/// conforming pair").
pub(crate) fn validate_namespaced_kind(namespaced_kind: &str) -> Result<(), EvidenceViolation> {
    if RecordKind::ALL.iter().any(|k| k.as_str() == namespaced_kind) {
        return Err(malformed(
            "namespaced_kind",
            format!("`{namespaced_kind}` collides with a core RecordKind -- an overlay kind may never alias a core kind directory"),
        ));
    }
    let mut parts = namespaced_kind.split('.');
    let (Some(namespace), Some(kind), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(malformed("namespaced_kind", format!("`{namespaced_kind}` is not exactly two dot-joined tokens")));
    };
    if !is_kebab_token(namespace) || !is_kebab_token(kind) {
        return Err(malformed(
            "namespaced_kind",
            format!("`{namespaced_kind}` -- both `<namespace>` and `<kind>` must match [a-z0-9]+(-[a-z0-9]+)*"),
        ));
    }
    Ok(())
}

/// The components of an overlay record's name: its declared join-key values,
/// in declaration order, each a free-text component (an overlay's values
/// have no grammar canon knows). REJECTS an empty value or list.
pub fn overlay_components(key: &[String]) -> Result<Vec<Component>, EvidenceViolation> {
    if key.is_empty() || key.iter().any(String::is_empty) {
        return Err(malformed("natural_key", "an overlay key needs at least one join-key value, none of them empty"));
    }
    Ok(key.iter().cloned().map(Component::Text).collect())
}

/// [`crate::git_tier::GitTier::write_namespaced`]'s key/body consistency
/// guard (design.md D4: "a filename and its body's join-key fields can never
/// disagree"). `write_namespaced` has no schema for an arbitrary namespaced
/// kind, so this check is deliberately schema-agnostic: every key value MUST
/// equal the string value of SOME top-level field already present in `body`
/// -- a key can never be manufactured from content absent from the record it
/// accompanies. The caller (canon-plugin's plugin-aware writer) derives the
/// key from `OverlayDecl.join_key`'s SPECIFIC named fields, in order -- a
/// stronger, decl-aware guarantee this generic check does not verify itself.
pub fn overlay_key_matches_body(key: &[String], body: &Value) -> Result<(), EvidenceViolation> {
    let Some(obj) = body.as_object() else {
        return Err(EvidenceViolation::new(FailureClass::InvalidJoinKey, "natural_key", "overlay body is not a JSON object"));
    };
    if key.iter().any(|value| !obj.values().any(|v| v.as_str() == Some(value.as_str()))) {
        return Err(EvidenceViolation::new(
            FailureClass::InvalidJoinKey,
            "natural_key",
            format!("`{}` does not match the join-key field values present in the overlay body", key.join("_")),
        ));
    }
    Ok(())
}

/// Whether an overlay file's `<key>` (the escaped components of its name)
/// traces back to `body`: each `_`-separated component, unescaped, equals a
/// top-level string field of the body, or is the `h<16 hex>` of one.
pub(crate) fn overlay_name_matches_body(escaped_key: &str, body: &Value) -> bool {
    let Some(obj) = body.as_object() else { return false };
    let strings = || obj.values().filter_map(Value::as_str);
    escaped_key.split('_').all(|part| {
        if let Some(value) = layout::unescape(part) {
            if strings().any(|s| s == value) {
                return true;
            }
        }
        part.len() == 17 && part.starts_with('h') && strings().any(|s| layout::shortened(s) == part)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn stem(kind: RecordKind, body: &Value) -> String {
        resolve_name(kind, body).unwrap().stem()
    }

    #[test]
    fn scenario_review_divergence_keys_start_with_the_project_and_have_no_area() {
        let scenario = json!({"kind": "scenario", "project_id": "app-a", "scenario_id": "world.firstbuy-hotdeal.26"});
        assert_eq!(resolve_partition(RecordKind::Scenario, &scenario).unwrap().natural_key, "app-a_world.firstbuy-hotdeal.26");
        let review = json!({"kind": "review", "project_id": "app-a", "scenario_id": "world.firstbuy-hotdeal.26", "pin": "abc123", "reviewer": "r"});
        assert_eq!(resolve_partition(RecordKind::Review, &review).unwrap().natural_key, "app-a_scenario_world.firstbuy-hotdeal.26");
        let divergence = json!({"kind": "divergence", "project_id": "app-a", "scenario_id": "world.firstbuy-hotdeal.26", "run_seq": 3, "round": 1});
        assert_eq!(resolve_partition(RecordKind::Divergence, &divergence).unwrap().natural_key, "app-a_world.firstbuy-hotdeal.26_3_1");
        let path = expected_relative_path(RecordKind::Review, &review).unwrap();
        assert_eq!(path.components().count(), 2, "one directory level, no area=: {}", path.display());
        assert!(path.to_str().unwrap().starts_with("kind=review/app-a_scenario_world.firstbuy-hotdeal.26_"));
    }

    #[test]
    fn finding_name_is_change_round_seq_unpadded() {
        let json = json!({"kind": "finding", "change_id": "s42-close-the-open-loops", "round": 11, "seq": 4});
        assert_eq!(resolve_partition(RecordKind::Finding, &json).unwrap().natural_key, "s42-close-the-open-loops_11_4");
    }

    #[test]
    fn finding_with_no_reviewed_sha_still_resolves_a_key() {
        let worktree_round = json!({"kind": "finding", "change_id": "s42-close-the-open-loops", "round": 8, "seq": 10});
        let committed_round = json!({"kind": "finding", "change_id": "s42-close-the-open-loops", "reviewed_sha": "8c81f9e13e9bda0a6a5ee29ba1b6b5137e7bf552", "round": 8, "seq": 10});
        assert_eq!(
            resolve_partition(RecordKind::Finding, &committed_round).unwrap().natural_key,
            resolve_partition(RecordKind::Finding, &worktree_round).unwrap().natural_key,
            "`reviewed_sha` is provenance, never part of the identity"
        );
    }

    #[test]
    fn finding_missing_any_key_component_is_malformed() {
        let change = "s42-close-the-open-loops";
        for json in [
            json!({"kind": "finding", "round": 1, "seq": 1}),
            json!({"kind": "finding", "change_id": change, "seq": 1}),
            json!({"kind": "finding", "change_id": change, "round": 1}),
        ] {
            let err = resolve_partition(RecordKind::Finding, &json).unwrap_err();
            assert_eq!(err.class, FailureClass::Malformed);
        }
    }

    #[test]
    fn finding_body_rejects_an_incoherent_disposition_resolution_pair() {
        let body = |disposition: &str, resolution: Option<&str>| {
            let mut json = json!({
                "schema": 1, "kind": "finding", "at": "2026-07-30T09:15:00Z", "actor": {"agent_id": "codex-cli"},
                "change_id": "s42-close-the-open-loops", "round": 1, "seq": 1, "severity": "blocker",
                "disposition": disposition, "reviewer": "review-voice", "summary": "s"
            });
            if let Some(sha) = resolution {
                json["resolution_sha"] = Value::String(sha.to_string());
            }
            RawRecord(json)
        };
        let closing = "b41d7e0925ca38f6ee1cb70d4a982f65173d0ce8";
        validate_body(RecordKind::Finding, &body("fixed", Some(closing))).unwrap();
        validate_body(RecordKind::Finding, &body("open", None)).unwrap();
        for incoherent in [body("fixed", None), body("open", Some(closing)), body("rejected", Some(closing))] {
            let err = validate_body(RecordKind::Finding, &incoherent).unwrap_err();
            assert_eq!(err.subject, "resolution_sha");
        }
    }

    #[test]
    fn same_scenario_id_under_different_projects_resolves_to_distinct_paths() {
        let a = json!({"kind": "scenario", "project_id": "app-a", "scenario_id": "world.firstbuy-hotdeal.26", "schema": 1});
        let b = json!({"kind": "scenario", "project_id": "app-b", "scenario_id": "world.firstbuy-hotdeal.26", "schema": 1});
        assert_ne!(expected_relative_path(RecordKind::Scenario, &a).unwrap(), expected_relative_path(RecordKind::Scenario, &b).unwrap());
    }

    #[test]
    fn spine_record_missing_project_id_is_malformed() {
        for (kind, json) in [
            (RecordKind::Scenario, json!({"kind": "scenario", "scenario_id": "world.firstbuy-hotdeal.26"})),
            (RecordKind::Review, json!({"kind": "review", "scenario_id": "world.firstbuy-hotdeal.26", "pin": "abc123"})),
            (RecordKind::Divergence, json!({"kind": "divergence", "scenario_id": "world.firstbuy-hotdeal.26", "run_seq": 1, "round": 1})),
        ] {
            assert_eq!(resolve_partition(kind, &json).unwrap_err().class, FailureClass::Malformed, "{kind:?}");
        }
    }

    #[test]
    fn regime_key_and_session_text_are_escaped_only_in_the_name() {
        let json = json!({"kind": "strategy_item", "regime_key": "implementer/canon/join-spine/9c93d024b1a2"});
        let key = resolve_partition(RecordKind::StrategyItem, &json).unwrap();
        assert_eq!(key.natural_key, "implementer/canon/join-spine/9c93d024b1a2", "typed value, never escaped");
        assert!(stem(RecordKind::StrategyItem, &json).starts_with("implementer+2fcanon+2fjoin-spine+2f9c93d024b1a2_"));
        let session = json!({"kind": "session", "session_id": "Sess_1/x"});
        assert!(stem(RecordKind::Session, &session).starts_with("+53ess+5f1+2fx_"));
    }

    #[test]
    fn identical_content_yields_identical_digest_and_path() {
        let a = json!({"kind": "change", "change_id": "x", "schema": 1});
        let b = json!({"schema": 1, "kind": "change", "change_id": "x"});
        assert_eq!(content_digest(&a), content_digest(&b));
        assert_eq!(content_digest(&a).len(), 8);
        assert_eq!(expected_relative_path(RecordKind::Change, &a).unwrap(), expected_relative_path(RecordKind::Change, &b).unwrap());
    }

    #[test]
    fn kind_directory_content_mismatch_is_a_layout_violation() {
        let json = json!({"kind": "task", "change_id": "x"});
        assert_eq!(validate_kind_matches_content(RecordKind::Change, &json).unwrap_err().class, FailureClass::Malformed);
    }

    #[test]
    fn validate_layout_reports_expected_and_actual_paths() {
        let json = json!({"kind": "change", "change_id": "s2-tiered-storage", "schema": 1});
        let err = validate_layout(RecordKind::Change, std::path::Path::new("kind=change/wrong-name.json"), &json).unwrap_err();
        assert!(err.detail.contains("expected") && err.detail.contains("wrong-name.json"), "{}", err.detail);
        assert_eq!(err.class, FailureClass::Malformed);
    }

    #[test]
    fn a_file_named_by_the_0_29_rule_is_the_old_layout_violation_with_the_migrate_hint() {
        let json = json!({"kind": "change", "change_id": "s2-tiered-storage", "schema": 1});
        for old in ["kind=change/s2-tiered-storage__0123456789ab.json", "kind=review/area=gate/x__0123456789ab.json"] {
            let err = validate_layout(RecordKind::Change, std::path::Path::new(old), &json).unwrap_err();
            assert_eq!(err.class, FailureClass::OldLayout, "{old}");
            assert!(err.detail.contains("canon migrate"));
        }
        assert!(!is_old_layout_path(std::path::Path::new("kind=change/wrong-name.json")));
    }

    #[test]
    fn validate_namespaced_kind_accepts_a_conforming_two_token_identity() {
        validate_namespaced_kind("porting.coverage").unwrap();
        validate_namespaced_kind("a1-b2.c3-d4").unwrap();
    }

    #[test]
    fn validate_namespaced_kind_rejects_every_core_recordkind_string() {
        for kind in RecordKind::ALL {
            assert!(validate_namespaced_kind(kind.as_str()).is_err(), "`{}` must be rejected", kind.as_str());
        }
    }

    #[test]
    fn validate_namespaced_kind_rejects_non_two_token_or_non_kebab_strings() {
        for bad in ["porting", "porting.coverage.extra", "porting/coverage", "Porting.Coverage", "porting.cov erage", "porting.-coverage", ".coverage", "porting."] {
            assert!(validate_namespaced_kind(bad).is_err(), "`{bad}` must be rejected");
        }
    }

    #[test]
    fn overlay_key_must_trace_back_to_the_body() {
        let body = json!({"project_id": "root", "scenario_id": "world.hotdeal.01", "covered": true});
        overlay_key_matches_body(&["root".into(), "world.hotdeal.01".into()], &body).unwrap();
        assert!(overlay_key_matches_body(&["root".into(), "world.hotdeal.99".into()], &body).is_err());
        assert!(overlay_name_matches_body("root_world.hotdeal.01", &body));
        assert!(!overlay_name_matches_body("root_world.hotdeal.99", &body));
        assert!(overlay_components(&[]).is_err());
    }
}
