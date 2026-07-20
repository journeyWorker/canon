//! Read-time overlay projection (s16 P3, `openspec/changes/
//! s16-plugin-extensibility/`, tasks.md 3.1-3.2, design.md D3,
//! `plugin-overlay-projection` spec): [`project_overlay`] -- the PURE,
//! in-memory JOIN of a core `Scenario` slice against a scanned overlay
//! record slice, keyed by `(ProjectId, ScenarioId)`.
//!
//! # Core is read-only, never rewritten (design.md D3)
//!
//! `core: &[Scenario]` is this function's LEFT side, identifying which
//! `(project_id, scenario_id)` keys exist to project onto -- it is
//! never mutated, never re-serialized, never passed anywhere near a
//! write path. This module takes NO `canon-store` core-write
//! dependency at all (the architectural half of design.md D3's "two
//! independent places" enforcement; the write-time half is P2's
//! `write_namespaced` rejecting a core-`RecordKind`-colliding
//! namespaced kind). The returned `BTreeMap` is a brand-new in-memory
//! structure; no core `Scenario` file is ever opened for writing by
//! any path this function reaches.
//!
//! s16 concretely projects onto `Scenario` only (`core_kind: scenario`,
//! enforced at 1.4's manifest resolution) -- a generic `project_overlay`
//! over other core kinds is explicit FUTURE work, out of this change's
//! scope (tasks.md 3.1).
//!
//! # Fail-soft (tasks.md 3.2)
//!
//! Every overlay record is independently validated
//! ([`crate::overlay::validate_overlay_body`]) against `decl` before it
//! can contribute to the projection -- a record that fails validation,
//! or whose `project_id`/`scenario_id` join-key values don't themselves
//! parse as well-formed [`ProjectId`]/[`ScenarioId`] (a check
//! `validate_overlay_body` itself does not make -- it only confirms the
//! join-key fields are PRESENT and JSON-string-typed, generically, for
//! any `OverlayDecl`), is SKIPPED with a diagnostic, never aborting
//! projection for any sibling record. An absent overlay record for a
//! given core key, or an empty `overlay_raw` slice entirely, degrades
//! to an empty projected map -- every unmatched core key is simply
//! absent from the output, never a panic, default, or guessed value.

use std::collections::{BTreeMap, BTreeSet};

use canon_model::{ProjectId, RawRecord, ScenarioId};
use canon_model::records::Scenario;
use canon_store::fold_latest_by_key;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::diagnostic::{Diagnostic, E_PLUGIN_BODY_TYPE};
use crate::manifest::snapshot::OverlayDecl;
use crate::overlay::validate_overlay_body;

/// A core `Scenario` record's composite identity -- this projection's
/// join key, matching `canon_model::records::Scenario`'s own
/// `project_id`+`scenario_id` composite identity.
pub type ScenarioKey = (ProjectId, ScenarioId);

/// [`project_overlay`]'s own return shape -- one overlay kind's
/// declared fields, folded to the single latest-`at` winner per
/// [`ScenarioKey`] (factored out of the function signature per
/// `clippy::type_complexity`).
pub type ProjectedOverlay = BTreeMap<ScenarioKey, serde_json::Map<String, Value>>;

fn diag(code: &str, message: impl Into<String>, subject: impl Into<String>) -> Diagnostic {
    Diagnostic::error(code, message, subject)
}

/// One overlay record that survived [`validate_overlay_body`] and
/// carried well-formed `project_id`/`scenario_id` join-key values --
/// the intermediate shape [`fold_latest_by_key`] folds over.
struct Survivor {
    key: ScenarioKey,
    at: DateTime<Utc>,
    digest: String,
    fields: serde_json::Map<String, Value>,
}

/// `project_id`/`scenario_id` are the ONLY join-key field names this
/// function looks up -- not a generic walk over `decl.join_key` -- by
/// design: this function's own return type is concretely keyed
/// `(ProjectId, ScenarioId)`, matching `Scenario`'s own composite
/// identity (`canon_model::records::Scenario`), because s16 projects
/// onto `Scenario` alone (module doc). A manifest whose `core_kind:
/// scenario` overlay declares a DIFFERENT `join_key` (e.g. `[foo]`)
/// still resolves and validates (P1/P2 are `join_key`-generic), but
/// every one of its records fails the lookup below and is skipped with
/// a diagnostic -- fail-soft, never a panic, never a misattributed
/// join.
fn extract_key(obj: &serde_json::Map<String, Value>, decl: &OverlayDecl, diags: &mut Vec<Diagnostic>) -> Option<ScenarioKey> {
    let project_id = match obj.get("project_id").and_then(Value::as_str).map(ProjectId::parse) {
        Some(Ok(id)) => id,
        Some(Err(e)) => {
            diags.push(diag(E_PLUGIN_BODY_TYPE, format!("join-key field `project_id` is not a valid ProjectId: {e}"), decl.identity.clone()));
            return None;
        }
        None => {
            diags.push(diag(E_PLUGIN_BODY_TYPE, "join-key field `project_id` is absent or not a string", decl.identity.clone()));
            return None;
        }
    };
    let scenario_id = match obj.get("scenario_id").and_then(Value::as_str).map(ScenarioId::parse) {
        Some(Ok(id)) => id,
        Some(Err(e)) => {
            diags.push(diag(E_PLUGIN_BODY_TYPE, format!("join-key field `scenario_id` is not a valid ScenarioId: {e}"), decl.identity.clone()));
            return None;
        }
        None => {
            diags.push(diag(E_PLUGIN_BODY_TYPE, "join-key field `scenario_id` is absent or not a string", decl.identity.clone()));
            return None;
        }
    };
    Some((project_id, scenario_id))
}

/// Project `decl`'s declared overlay fields onto `core`'s
/// `(project_id, scenario_id)` keys from `overlay_raw`'s scanned
/// records -- pure, no IO, never panics (tasks.md 3.1). Every
/// surviving record (passed [`validate_overlay_body`] AND carried
/// well-formed join-key values) is folded latest-by-`(join_key, at)`,
/// reusing [`fold_latest_by_key`]'s exact last-wins-by-`at`,
/// ties-broken-by-content-digest semantics (s21 P1); the winning record's own
/// `decl.fields`-named values are extracted into the returned map,
/// filtered to keys `core` actually carries -- `core` is this join's
/// LEFT side, so an overlay record for a key `core` doesn't contain
/// (e.g. a scenario since deleted from the corpus) projects onto
/// nothing and is silently excluded, never fabricating a phantom
/// output row. Every diagnostic accumulated along the way (a
/// validation failure, a malformed join-key value) is returned
/// alongside for the caller to surface -- this function itself never
/// aborts on any single record's defect (tasks.md 3.2).
pub fn project_overlay(
    core: &[Scenario],
    overlay_raw: &[RawRecord],
    decl: &OverlayDecl,
) -> (ProjectedOverlay, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let core_keys: BTreeSet<ScenarioKey> = core.iter().map(|s| (s.project_id.clone(), s.scenario_id.clone())).collect();

    let mut survivors = Vec::new();
    for raw in overlay_raw {
        if let Err(mut body_diags) = validate_overlay_body(decl, raw) {
            diags.append(&mut body_diags);
            continue;
        }
        // `validate_overlay_body` already confirmed `raw.0` is a JSON
        // object (a non-object body fails validation before this
        // point, via `check_envelope_fields`).
        let obj = raw.0.as_object().expect("validate_overlay_body confirmed the body is a JSON object");

        let Some(key) = extract_key(obj, decl, &mut diags) else { continue };

        // `validate_overlay_body` also already confirmed `at` is a
        // present, RFC3339-valid string (`check_envelope_fields`
        // reuses `canon_model::evidence::validate_envelope_shape`) --
        // safe to call unconditionally at this point.
        let at = canon_store::tier::raw_record_at(raw);
        let digest = canon_store::partition::content_digest12(&raw.0);

        let fields: serde_json::Map<String, Value> =
            decl.fields.iter().filter_map(|field| obj.get(&field.name).map(|v| (field.name.clone(), v.clone()))).collect();

        survivors.push(Survivor { key, at, digest, fields });
    }

    let folded = fold_latest_by_key(survivors, |s| s.key.clone(), |s| s.at, |s| s.digest.as_str());

    let projected =
        folded.into_iter().filter(|(key, _)| core_keys.contains(key)).map(|(key, survivor)| (key, survivor.fields)).collect();

    (projected, diags)
}

#[cfg(test)]
mod tests {
    use canon_model::ids::RoleId;
    use canon_model::{Actor, Envelope, RecordKind, SpecDigest};
    use serde_json::json;

    use super::*;
    use crate::manifest::schema::FieldDecl;
    use crate::manifest::types::Type;
    use crate::overlay::{OverlayEnvelope, compose_overlay_body};

    fn actor() -> Actor {
        Actor::new("test-agent", RoleId::parse("implementer").unwrap())
    }

    fn coverage_decl() -> OverlayDecl {
        OverlayDecl {
            namespace: "porting".to_string(),
            kind: "coverage".to_string(),
            identity: "porting.coverage".to_string(),
            core_kind: "scenario".to_string(),
            join_key: vec!["project_id".to_string(), "scenario_id".to_string()],
            fields: vec![
                FieldDecl { name: "covered".to_string(), ty: Type::Bool },
                FieldDecl { name: "surface_ref".to_string(), ty: Type::List(Box::new(Type::Str)) },
            ],
        }
    }

    fn scenario(project_id: &str, scenario_id: &str, at: DateTime<Utc>) -> Scenario {
        Scenario::new(
            Envelope::new(1, RecordKind::Scenario, at, actor()),
            ProjectId::parse(project_id).unwrap(),
            canon_model::ScenarioId::parse(scenario_id).unwrap(),
            "a title",
            "",
            SpecDigest::parse("a".repeat(64)).unwrap(),
        )
    }

    fn overlay_body(project_id: &str, scenario_id: &str, at: DateTime<Utc>, covered: bool, surface_ref: &[&str]) -> RawRecord {
        let envelope = OverlayEnvelope::new(1, "porting.coverage", at, actor());
        let mut fields = serde_json::Map::new();
        fields.insert("project_id".to_string(), json!(project_id));
        fields.insert("scenario_id".to_string(), json!(scenario_id));
        fields.insert("covered".to_string(), json!(covered));
        fields.insert("surface_ref".to_string(), json!(surface_ref));
        compose_overlay_body(&envelope, fields)
    }

    fn at(offset_secs: i64) -> DateTime<Utc> {
        DateTime::UNIX_EPOCH + chrono::Duration::seconds(offset_secs)
    }

    #[test]
    fn a_core_record_with_a_matching_overlay_record_projects_the_declared_fields() {
        let core = vec![scenario("root", "world.hotdeal.01", at(0))];
        let overlay = vec![overlay_body("root", "world.hotdeal.01", at(1), true, &["world.hotdeal.01"])];
        let (projected, diags) = project_overlay(&core, &overlay, &coverage_decl());
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
        let key = (ProjectId::parse("root").unwrap(), canon_model::ScenarioId::parse("world.hotdeal.01").unwrap());
        let fields = projected.get(&key).expect("projected fields for the matching key");
        assert_eq!(fields.get("covered"), Some(&json!(true)));
        assert_eq!(fields.get("surface_ref"), Some(&json!(["world.hotdeal.01"])));
        // Exactly the declared fields -- no envelope/join-key leakage.
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn a_core_record_with_no_overlay_record_projects_unmodified() {
        let core = vec![scenario("root", "world.hotdeal.01", at(0))];
        let (projected, diags) = project_overlay(&core, &[], &coverage_decl());
        assert!(diags.is_empty());
        assert!(projected.is_empty(), "no overlay record exists for this key -- the projected map must carry no entry for it");
    }

    #[test]
    fn a_malformed_overlay_record_is_skipped_while_siblings_still_project() {
        let core = vec![scenario("root", "world.hotdeal.01", at(0)), scenario("root", "world.hotdeal.02", at(0))];
        let malformed = {
            let envelope = OverlayEnvelope::new(1, "porting.coverage", at(1), actor());
            let mut fields = serde_json::Map::new();
            fields.insert("project_id".to_string(), json!("root"));
            fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
            fields.insert("covered".to_string(), json!("not-a-bool")); // wrong type -- fails validate_overlay_body
            fields.insert("surface_ref".to_string(), json!([]));
            compose_overlay_body(&envelope, fields)
        };
        let well_formed = overlay_body("root", "world.hotdeal.02", at(1), true, &[]);

        let (projected, diags) = project_overlay(&core, &[malformed, well_formed], &coverage_decl());
        assert!(!diags.is_empty(), "the malformed record must be diagnosed");
        assert!(diags.iter().any(|d| d.code == "E-PLUGIN-BODY-TYPE"));

        let malformed_key = (ProjectId::parse("root").unwrap(), canon_model::ScenarioId::parse("world.hotdeal.01").unwrap());
        assert!(!projected.contains_key(&malformed_key), "the malformed record must never contribute a projected entry");

        let sibling_key = (ProjectId::parse("root").unwrap(), canon_model::ScenarioId::parse("world.hotdeal.02").unwrap());
        assert_eq!(projected.get(&sibling_key).and_then(|f| f.get("covered")), Some(&json!(true)));
    }

    #[test]
    fn latest_at_wins_per_key_reusing_fold_latest_by_key() {
        let core = vec![scenario("root", "world.hotdeal.01", at(0))];
        let older = overlay_body("root", "world.hotdeal.01", at(1), false, &[]);
        let newer = overlay_body("root", "world.hotdeal.01", at(2), true, &["fresh"]);
        // Iteration order deliberately newer-then-older: last-wins-by-`at`
        // must still pick `newer`, not "whichever came last in the slice".
        let (projected, diags) = project_overlay(&core, &[newer, older], &coverage_decl());
        assert!(diags.is_empty());
        let key = (ProjectId::parse("root").unwrap(), canon_model::ScenarioId::parse("world.hotdeal.01").unwrap());
        assert_eq!(projected.get(&key).and_then(|f| f.get("covered")), Some(&json!(true)));
    }

    #[test]
    fn an_overlay_record_for_a_key_absent_from_core_is_excluded_never_a_phantom_row() {
        let core = vec![scenario("root", "world.hotdeal.01", at(0))];
        // Overlay record targets a scenario NOT present in `core` (e.g.
        // deleted from the corpus since the overlay was written).
        let orphan = overlay_body("root", "world.hotdeal.99", at(1), true, &[]);
        let (projected, diags) = project_overlay(&core, &[orphan], &coverage_decl());
        assert!(diags.is_empty());
        assert!(projected.is_empty(), "an overlay record with no matching core key must never appear in the projected map");
    }

    #[test]
    fn empty_overlay_raw_and_empty_core_both_degrade_to_an_empty_map_never_a_panic() {
        let (projected, diags) = project_overlay(&[], &[], &coverage_decl());
        assert!(projected.is_empty());
        assert!(diags.is_empty());
    }
}
