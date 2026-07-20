//! Git-tier Hive layout: partition-key extraction from a record's own
//! JSON content, canonical content digests, and the write/read-shared
//! layout check (git-tier-layout-enforcement spec; S2 design D2,
//! generalizing `tools/parity.py::_ledger_layout_problem`/`_area_of`).
//!
//! `canon-model`'s [`canon_model::envelope::RecordKind::partition_template`]
//! is the pure, storage-agnostic path-TEMPLATE string; everything here is
//! the INTERPRETATION half design D2's Risk section reserves for
//! `canon-store` alone — `canon-model` never sees this module.
//!
//! Every kind's `{id}` placeholder resolves to `<natural-key>__<digest12>`:
//! a human-legible natural key (the kind's own join-key field(s), so a
//! git-tier directory listing stays legible — mirrors the donor's
//! `_write_ledger_records` "digest-suffixed filename construction") PLUS
//! a 12-hex content-digest suffix so a byte-identical resubmission always
//! resolves to the SAME path (append-only "duplicate write rejected" —
//! git-tier-layout-enforcement spec) while a logically different record
//! sharing the same natural key (e.g. a second `Review` of the same
//! `scenario_id`, or a re-emitted `Change` at a new lifecycle state)
//! always resolves to a DIFFERENT path (a genuine new append, never
//! forced through `canon migrate`). This is a deliberate generalization
//! beyond the donor's exact `{scenario_id}.json` review filename: canon's
//! `GitTier` is unconditionally append-only (spec: "writing a record
//! never overwrites an existing file"), which parity.py's in-place-edited
//! review file does not fully honor.

use canon_model::envelope::RecordKind;
use canon_model::evidence::EvidenceViolation;
use canon_model::ids::ScenarioId;
use canon_model::{FailureClass, RawRecord};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A record's resolved git-tier partition coordinates, extracted from
/// its own content (never a source directory — the load-bearing
/// invariant this whole module exists to enforce).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionKey {
    /// `Some(area)` for an area-scoped kind ([`RecordKind::is_area_scoped`]),
    /// always `None` otherwise.
    pub area: Option<String>,
    /// The human-legible portion of the filename stem, before the
    /// digest suffix — the kind's own natural join-key field(s).
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

fn get_int(json: &Value, field: &str) -> Result<i64, EvidenceViolation> {
    json.get(field).and_then(Value::as_i64).ok_or_else(|| malformed(field, format!("missing or non-integer `{field}` field")))
}

/// Filesystem-unsafe characters a join-key grammar otherwise permits
/// (only [`canon_model::ids::RegimeKey`]'s `/`-separated grammar today)
/// collapsed to `_` — never silently dropped, so the sanitized key is
/// still visibly derived from the original.
fn sanitize_component(s: &str) -> String {
    s.chars().map(|c| if c == '/' { '_' } else { c }).collect()
}

/// `ScenarioId::area()`, re-validated from a raw JSON string field
/// rather than trusted as already-parsed — `GitTier::read` calls this
/// against arbitrary on-disk content, which may not have passed
/// `ScenarioId::parse` yet.
fn scenario_area(scenario_id: &str) -> Result<String, EvidenceViolation> {
    ScenarioId::parse(scenario_id)
        .map(|sid| sid.area().to_string())
        .map_err(|e| EvidenceViolation::new(FailureClass::InvalidJoinKey, "scenario_id", e.to_string()))
}

/// Extract this record's git-tier [`PartitionKey`] from its own body —
/// the ONLY input; never the directory it happened to be found in
/// (parity-harness audit: six real feature-dir-vs-scenario-area mismatch
/// cases). `kind` is the caller's already-confirmed kind (the
/// containing `kind=<kind>/` directory for a read, or `T::KIND` for a
/// write) — [`validate_kind_matches_content`] is the separate check that
/// the JSON body's own `kind` field agrees with it.
pub fn resolve_partition(kind: RecordKind, json: &Value) -> Result<PartitionKey, EvidenceViolation> {
    match kind {
        RecordKind::Change => Ok(PartitionKey { area: None, natural_key: get_str(json, "change_id")?.to_string() }),
        RecordKind::Task => Ok(PartitionKey { area: None, natural_key: get_str(json, "task_id")?.to_string() }),
        RecordKind::Scenario => {
            let project_id = get_str(json, "project_id")?;
            let scenario_id = get_str(json, "scenario_id")?;
            Ok(PartitionKey { area: Some(scenario_area(scenario_id)?), natural_key: format!("{project_id}__{scenario_id}") })
        }
        RecordKind::Session => Ok(PartitionKey { area: None, natural_key: sanitize_component(get_str(json, "session_id")?) }),
        RecordKind::Run => Ok(PartitionKey { area: None, natural_key: get_str(json, "run_id")?.to_string() }),
        RecordKind::Event => {
            let run_id = get_str(json, "run_id")?;
            let seq = get_int(json, "seq")?;
            Ok(PartitionKey { area: None, natural_key: format!("{run_id}-{seq:010}") })
        }
        RecordKind::Handoff => Ok(PartitionKey { area: None, natural_key: get_str(json, "id")?.to_string() }),
        RecordKind::Review => {
            let project_id = get_str(json, "project_id")?;
            let scenario_id = get_str(json, "scenario_id")?;
            let pin = get_str(json, "pin")?;
            Ok(PartitionKey { area: Some(scenario_area(scenario_id)?), natural_key: format!("{project_id}__{scenario_id}__{pin}") })
        }
        RecordKind::Divergence => {
            let project_id = get_str(json, "project_id")?;
            let scenario_id = get_str(json, "scenario_id")?;
            let run_seq = get_int(json, "run_seq")?;
            let round = get_int(json, "round")?;
            Ok(PartitionKey {
                area: Some(scenario_area(scenario_id)?),
                natural_key: format!("{project_id}__{scenario_id}__{run_seq:010}__{round:06}"),
            })
        }
        RecordKind::Trajectory => Ok(PartitionKey { area: None, natural_key: get_str(json, "run_id")?.to_string() }),
        RecordKind::StrategyItem => {
            Ok(PartitionKey { area: None, natural_key: sanitize_component(get_str(json, "regime_key")?) })
        }
        RecordKind::EvidenceRecord => {
            // Whichever join key this attestation actually carries
            // (all three are `Option` on `EvidenceRecord` — S1 design);
            // `"unscoped"` only when none is present, so an
            // `EvidenceRecord` about a bare run/task never fails to
            // resolve a partition just for lacking a `scenario_id`.
            let key = ["task_id", "scenario_id", "run_id"].into_iter().find_map(|f| json.get(f).and_then(Value::as_str));
            Ok(PartitionKey { area: None, natural_key: key.unwrap_or("unscoped").to_string() })
        }
        // s36: `Subject` is a by-id, flat-partition kind (like
        // `Change`) — no mandatory `scenario_id`, so no `area=` segment
        // (`RecordKind::is_area_scoped` is false for it). Natural key is
        // the `subject_id` slug.
        RecordKind::Subject => Ok(PartitionKey { area: None, natural_key: get_str(json, "subject_id")?.to_string() }),
    }
}

/// The JSON body's own `kind` field must agree with the caller's
/// already-known kind (a containing `kind=<kind>/` directory on read, or
/// `T::KIND` on write) — a directory/content kind mismatch is itself a
/// layout violation, distinct from (and checked before) path-shape/area
/// agreement.
pub fn validate_kind_matches_content(expected: RecordKind, json: &Value) -> Result<(), EvidenceViolation> {
    let found = get_str(json, "kind")?;
    if found != expected.as_str() {
        return Err(malformed("layout", format!("directory `kind={}/` but record body's own `kind` is `{found}`", expected.as_str())));
    }
    Ok(())
}

/// First 12 lowercase-hex characters of SHA-256 over the canonical
/// (alphabetical-key, since `serde_json::Value::Object` is a `BTreeMap`
/// by default — no `preserve_order` feature anywhere in this workspace)
/// JSON serialization of `json` — the same digest-based-idempotence
/// mechanism design D3 specifies for `canon tier age`, reused here at
/// git-tier filename-uniqueness granularity (module doc).
pub fn content_digest12(json: &Value) -> String {
    let bytes = serde_json::to_vec(json).expect("serde_json::Value always serializes");
    let hash = Sha256::digest(&bytes);
    let mut hex = String::with_capacity(12);
    for byte in hash.iter().take(6) {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// The full Hive-partitioned relative object key a record's OWN
/// content resolves to — `kind={kind}/[area={area}/]{natural_key}__{digest12}.{extension}`
/// — the shared coordinate scheme `GitTier` (extension `"json"`, a
/// filesystem path) and `R2Tier` (extension `"parquet"`, an object
/// store key) both compute identically at write time (where each tier
/// writes exactly here) and read time (where a found file/object's
/// actual key is compared against this). Divergence between the two is
/// precisely a layout violation (git-tier-layout-enforcement spec).
pub fn hive_object_key(kind: RecordKind, json: &Value, extension: &str) -> Result<std::path::PathBuf, EvidenceViolation> {
    validate_kind_matches_content(kind, json)?;
    let key = resolve_partition(kind, json)?;
    let digest = content_digest12(json);
    let filename = format!("{}__{digest}.{extension}", key.natural_key);
    let mut path = std::path::PathBuf::from(format!("kind={}", kind.as_str()));
    if let Some(area) = &key.area {
        path.push(format!("area={area}"));
    }
    path.push(filename);
    Ok(path)
}

/// `GitTier`'s `.json`-specific alias of [`hive_object_key`] — kept as
/// a separate name at git-tier call sites since "the path a git-tier
/// record resolves to" is the concept those call sites reason about,
/// not "some Hive object key at some extension".
pub fn expected_relative_path(kind: RecordKind, json: &Value) -> Result<std::path::PathBuf, EvidenceViolation> {
    hive_object_key(kind, json, "json")
}

/// Validate that `actual_path` (relative to the git tier's `root`) is
/// EXACTLY the path `json`'s own content resolves to for `kind` — the
/// git-tier-layout-enforcement spec's core check, shared by
/// `GitTier::write` (self-consistency: the path it is about to write to)
/// and `GitTier::read` (a found file's actual path). A mismatch names
/// both the expected template-resolved path and the rejected path, as
/// the spec's own scenario wording requires.
pub fn validate_layout(kind: RecordKind, actual_path: &std::path::Path, json: &Value) -> Result<(), EvidenceViolation> {
    let expected = expected_relative_path(kind, json)?;
    if expected != actual_path {
        return Err(malformed(
            "layout",
            format!(
                "expected `{}` (template `{}`), found `{}`",
                expected.display(),
                kind.partition_template(),
                actual_path.display()
            ),
        ));
    }
    Ok(())
}

/// `canon_model::validate_evidence`/`validate_evidence_batch` are the
/// generic-envelope-plus-`EvidenceRecord`-body gate; every OTHER kind's
/// full-body shape is checked by attempting the matching concrete
/// `CanonRecord` type's own `Deserialize` — mirrors
/// `canon_model::fixtures::round_trip_well_formed`'s per-kind dispatch
/// (S1), reused here as the git-tier read path's content-malformed gate.
pub fn validate_body(kind: RecordKind, raw: &RawRecord) -> Result<(), EvidenceViolation> {
    use canon_model::records::*;
    use canon_model::{FailureClass, Handoff};

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
        RecordKind::EvidenceRecord => canon_model::validate_evidence(raw).map(|_| ()),
        RecordKind::Subject => try_deserialize!(Subject),
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

/// [`crate::git_tier::GitTier::write_namespaced`]'s `natural_key`
/// path-safety guard (s16 tasks.md 2.3). REJECTS, before any filesystem
/// path is built, a `natural_key` containing `/`, `\`, `..`, a leading
/// `.`, or that is itself an absolute path -- exactly the traversal
/// surface an otherwise-unconstrained caller-supplied path component
/// would open (`plugin-overlay-records` spec).
pub(crate) fn validate_natural_key(natural_key: &str) -> Result<(), EvidenceViolation> {
    let unsafe_key = natural_key.contains('/')
        || natural_key.contains('\\')
        || natural_key.contains("..")
        || natural_key.starts_with('.')
        || std::path::Path::new(natural_key).is_absolute();
    if unsafe_key {
        return Err(malformed(
            "natural_key",
            format!("`{natural_key}` fails the path-safety grammar (no `/`, `\\`, `..`, leading `.`, or absolute path)"),
        ));
    }
    Ok(())
}

/// [`crate::git_tier::GitTier::write_namespaced`]'s natural-key/body
/// consistency guard (s16 tasks.md 2.3, design.md D4: "a filename and
/// its body's join-key fields can never disagree"). `write_namespaced`
/// has no schema for an arbitrary namespaced kind (unlike
/// [`resolve_partition`]'s per-`RecordKind` field extraction), so this
/// check is deliberately schema-agnostic: every `__`-delimited segment
/// of `natural_key` MUST equal the string value of SOME top-level field
/// already present in `body` -- a `natural_key` can never be
/// manufactured from content absent from the record it accompanies. The
/// caller (canon-plugin's plugin-aware writer) derives `natural_key`
/// from `OverlayDecl.join_key`'s SPECIFIC named fields, in order -- a
/// stronger, decl-aware guarantee this generic check does not (and,
/// lacking the join-key field names, structurally cannot) verify
/// itself, but every value that writer produces always satisfies this
/// weaker one too.
pub(crate) fn natural_key_matches_body(natural_key: &str, body: &Value) -> Result<(), EvidenceViolation> {
    let Some(obj) = body.as_object() else {
        return Err(EvidenceViolation::new(FailureClass::InvalidJoinKey, "natural_key", "overlay body is not a JSON object"));
    };
    let disagrees = natural_key.split("__").any(|segment| !obj.values().any(|v| v.as_str() == Some(segment)));
    if disagrees {
        return Err(EvidenceViolation::new(
            FailureClass::InvalidJoinKey,
            "natural_key",
            format!("`{natural_key}` does not match the join-key field values present in the overlay body"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn area_scoped_kind_resolves_area_from_scenario_id_not_a_directory() {
        let json = json!({"kind": "scenario", "project_id": "root", "scenario_id": "world.firstbuy-hotdeal.26"});
        let key = resolve_partition(RecordKind::Scenario, &json).unwrap();
        assert_eq!(key.area.as_deref(), Some("world"));
        assert_eq!(key.natural_key, "root__world.firstbuy-hotdeal.26");
    }

    #[test]
    fn flat_kind_has_no_area() {
        let json = json!({"kind": "change", "change_id": "s2-tiered-storage"});
        let key = resolve_partition(RecordKind::Change, &json).unwrap();
        assert_eq!(key.area, None);
    }

    #[test]
    fn six_mismatch_style_case_area_disagrees_with_a_plausible_directory_guess() {
        // parity-harness audit's own example: a `promise-date`-prefixed
        // scenario_id living under a `spec/features/world/` feature
        // directory. `resolve_partition` must side with the id, never
        // a caller-supplied directory guess.
        let json = json!({"kind": "review", "project_id": "root", "scenario_id": "promise-date.play.03", "pin": "abc123", "reviewer": "r"});
        let key = resolve_partition(RecordKind::Review, &json).unwrap();
        assert_eq!(key.area.as_deref(), Some("promise-date"), "area must come from scenario_id, never a directory named `world`");
        assert_eq!(key.natural_key, "root__promise-date.play.03__abc123");
    }

    #[test]
    fn scenario_review_divergence_natural_keys_are_project_prefixed() {
        // s15 design D2/D6, scenario-project-identity spec: every
        // spine kind's natural_key gains a `<project_id>__` prefix —
        // `ProjectId`'s grammar forbids `_`, so `__` stays an
        // unambiguous split point alongside each kind's own suffix.
        let scenario = json!({"kind": "scenario", "project_id": "app-a", "scenario_id": "world.firstbuy-hotdeal.26"});
        assert_eq!(resolve_partition(RecordKind::Scenario, &scenario).unwrap().natural_key, "app-a__world.firstbuy-hotdeal.26");

        let review = json!({"kind": "review", "project_id": "app-a", "scenario_id": "world.firstbuy-hotdeal.26", "pin": "abc123", "reviewer": "r"});
        assert_eq!(resolve_partition(RecordKind::Review, &review).unwrap().natural_key, "app-a__world.firstbuy-hotdeal.26__abc123");

        let divergence = json!({"kind": "divergence", "project_id": "app-a", "scenario_id": "world.firstbuy-hotdeal.26", "run_seq": 3, "round": 1});
        assert_eq!(
            resolve_partition(RecordKind::Divergence, &divergence).unwrap().natural_key,
            "app-a__world.firstbuy-hotdeal.26__0000000003__000001"
        );
    }

    #[test]
    fn same_scenario_id_under_different_project_ids_resolves_to_distinct_paths() {
        // Cross-project isolation (scenario-project-identity spec):
        // two roots sharing a `scenario_id` must never collapse to
        // one natural key or one storage path.
        let a = json!({"kind": "scenario", "project_id": "app-a", "scenario_id": "world.firstbuy-hotdeal.26", "schema": 1});
        let b = json!({"kind": "scenario", "project_id": "app-b", "scenario_id": "world.firstbuy-hotdeal.26", "schema": 1});
        let key_a = resolve_partition(RecordKind::Scenario, &a).unwrap();
        let key_b = resolve_partition(RecordKind::Scenario, &b).unwrap();
        assert_ne!(key_a.natural_key, key_b.natural_key, "distinct project_id must never collapse to the same natural key");
        assert_ne!(
            expected_relative_path(RecordKind::Scenario, &a).unwrap(),
            expected_relative_path(RecordKind::Scenario, &b).unwrap(),
            "two projects sharing a scenario_id must resolve to distinct storage paths"
        );
    }

    #[test]
    fn spine_record_missing_project_id_is_malformed() {
        // Clean-cutover migration (scenario-project-identity spec):
        // `project_id` is REQUIRED on the three spine kinds, so a
        // stray record lacking it is malformed=no-evidence — never a
        // legitimately-identified record with an inferred project.
        for (kind, json) in [
            (RecordKind::Scenario, json!({"kind": "scenario", "scenario_id": "world.firstbuy-hotdeal.26"})),
            (RecordKind::Review, json!({"kind": "review", "scenario_id": "world.firstbuy-hotdeal.26", "pin": "abc123"})),
            (RecordKind::Divergence, json!({"kind": "divergence", "scenario_id": "world.firstbuy-hotdeal.26", "run_seq": 1, "round": 1})),
        ] {
            let err = resolve_partition(kind, &json).unwrap_err();
            assert_eq!(err.class, FailureClass::Malformed, "{kind:?} missing project_id must resolve as malformed, not silently default");
        }
    }

    #[test]
    fn regime_key_slashes_are_sanitized_not_left_as_path_separators() {
        let json = json!({"kind": "strategy_item", "regime_key": "implementer/canon/join-spine/9c93d024b1a2"});
        let key = resolve_partition(RecordKind::StrategyItem, &json).unwrap();
        assert_eq!(key.natural_key, "implementer_canon_join-spine_9c93d024b1a2");
    }

    #[test]
    fn identical_content_yields_identical_digest_and_path() {
        let a = json!({"kind": "change", "change_id": "x", "schema": 1});
        let b = json!({"schema": 1, "kind": "change", "change_id": "x"});
        assert_eq!(content_digest12(&a), content_digest12(&b), "key order must not affect the canonical digest");
        assert_eq!(expected_relative_path(RecordKind::Change, &a).unwrap(), expected_relative_path(RecordKind::Change, &b).unwrap());
    }

    #[test]
    fn kind_directory_content_mismatch_is_a_layout_violation() {
        let json = json!({"kind": "task", "change_id": "x"});
        let err = validate_kind_matches_content(RecordKind::Change, &json).unwrap_err();
        assert_eq!(err.class, FailureClass::Malformed);
    }

    #[test]
    fn validate_layout_reports_expected_and_actual_paths() {
        let json = json!({"kind": "change", "change_id": "s2-tiered-storage", "schema": 1});
        let wrong_path = std::path::Path::new("kind=change/wrong-name.json");
        let err = validate_layout(RecordKind::Change, wrong_path, &json).unwrap_err();
        assert!(err.detail.contains("expected"), "detail should name the expected path: {}", err.detail);
        assert!(err.detail.contains("wrong-name.json"), "detail should name the rejected path: {}", err.detail);
    }

    #[test]
    fn validate_namespaced_kind_accepts_a_conforming_two_token_identity() {
        assert!(validate_namespaced_kind("porting.coverage").is_ok());
        assert!(validate_namespaced_kind("a1-b2.c3-d4").is_ok());
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
    fn validate_natural_key_accepts_a_safe_component() {
        assert!(validate_natural_key("root__world.hotdeal.01").is_ok());
    }

    #[test]
    fn validate_natural_key_rejects_path_traversal_and_absolute_paths() {
        for bad in ["../../etc/passwd", "a/b", "a\\b", "..", ".hidden", "/abs"] {
            assert!(validate_natural_key(bad).is_err(), "`{bad}` must be rejected");
        }
    }

    #[test]
    fn natural_key_matches_body_accepts_segments_traceable_to_body_fields() {
        let body = json!({"project_id": "root", "scenario_id": "world.hotdeal.01", "covered": true});
        assert!(natural_key_matches_body("root__world.hotdeal.01", &body).is_ok());
    }

    #[test]
    fn natural_key_matches_body_rejects_a_segment_absent_from_body() {
        let body = json!({"project_id": "root", "scenario_id": "world.hotdeal.99"});
        assert!(natural_key_matches_body("root__world.hotdeal.01", &body).is_err());
    }
}
