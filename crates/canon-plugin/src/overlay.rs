//! Overlay write + validation (s16 P2, `openspec/changes/
//! s16-plugin-extensibility/`, design.md D4, tasks.md 2.4-2.6): canon-
//! plugin's own record envelope ([`OverlayEnvelope`]), the manifest-schema
//! body validator ([`validate_overlay_body`]), and the plugin-aware writer
//! ([`write_overlay`]) that validates a candidate body, derives its
//! `natural_key` from the body's own join-key field values, and calls
//! `canon_store::git_tier::GitTier::write_namespaced` -- NEVER an
//! independently supplied `natural_key`.
//!
//! # Why `OverlayEnvelope`, not `canon_model::Envelope`
//!
//! `canon_model::Envelope.kind: RecordKind` is closed to the twelve core
//! kinds (canon-model design D1) -- an overlay record's on-disk kind is a
//! plugin-declared `<namespace>.<kind>` STRING (design.md D1: "a plain
//! string, never a `RecordKind` variant"), which that closed type
//! structurally cannot represent. [`OverlayEnvelope`] reuses
//! `canon_model::Actor` (S1's attribution type, which is NOT closed) for
//! everything else.
//!
//! # Scope: P2 only
//!
//! No projection (P3's `project_overlay`), no `canon query --plugin`, no
//! porting plugin (P4) -- this module's only job is turning a validated
//! candidate body into a written overlay record.

use std::collections::HashSet;

use canon_model::{Actor, RawRecord};
use canon_store::git_tier::{GitTier, NamespacedWriteReceipt};
use canon_store::tier::StoreError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::diagnostic::{Diagnostic, E_PLUGIN_BODY_KIND, E_PLUGIN_BODY_MISSING, E_PLUGIN_BODY_TYPE, E_PLUGIN_BODY_UNDECLARED};
use crate::manifest::snapshot::OverlayDecl;
use crate::manifest::types::type_accepts;

/// canon-plugin's own record envelope (design.md D4, tasks.md 2.5):
/// `{schema, kind, at, actor}`, `#[serde(flatten)]`-composed into every
/// overlay record body exactly as `canon_model::Envelope` is composed
/// into every core record (S1 design D2) -- see this module's doc
/// comment for why it cannot literally BE `canon_model::Envelope`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverlayEnvelope {
    pub schema: u32,
    /// The overlay's own on-disk identity (`<namespace>.<kind>`, e.g.
    /// `"porting.coverage"`) -- an ordinary `String`, never
    /// `canon_model::RecordKind`.
    pub kind: String,
    pub at: DateTime<Utc>,
    pub actor: Actor,
}

impl OverlayEnvelope {
    pub fn new(schema: u32, kind: impl Into<String>, at: DateTime<Utc>, actor: Actor) -> Self {
        Self { schema, kind: kind.into(), at, actor }
    }
}

/// Compose a full overlay record body: [`OverlayEnvelope`]'s own fields
/// flattened alongside `fields` (the join-key + declared-field values) --
/// mirrors `#[serde(flatten)]`'s effect for every `canon_model::CanonRecord`
/// (S1 design D2), assembled by hand here since an overlay body has no
/// single owning Rust struct per overlay kind (overlay bodies are
/// plugin-declared, not compile-time Rust types). The one body-assembly
/// point this module's own tests and a future P4 `OverlaySource` both use,
/// so envelope-flattening logic never drifts between them.
pub fn compose_overlay_body(envelope: &OverlayEnvelope, mut fields: serde_json::Map<String, Value>) -> RawRecord {
    let Value::Object(mut obj) = serde_json::to_value(envelope).expect("OverlayEnvelope always serializes") else {
        unreachable!("OverlayEnvelope serializes to a JSON object")
    };
    obj.append(&mut fields);
    RawRecord(Value::Object(obj))
}

fn diag(code: &str, message: impl Into<String>, subject: impl Into<String>) -> Diagnostic {
    Diagnostic::error(code, message, subject)
}

/// `OverlayEnvelope`'s four fields, present + structurally typed -- reuses
/// `canon_model::evidence::validate_envelope_shape` (already tested,
/// `kind`-generic: it only checks `kind` is SOME string, never a specific
/// `RecordKind`) rather than re-deriving the same RFC3339/actor-shape
/// checks a second time. `validate_envelope_shape` itself conflates
/// "absent" and "present but structurally invalid" in one message per
/// field, so [`envelope_field_is_present`] walks the violation's own
/// `subject` dot-path back against `body` to split them: a field that
/// IS present (however malformed) is `E-PLUGIN-BODY-TYPE`, never
/// `E-PLUGIN-BODY-MISSING` -- only a truly absent field keeps the
/// MISSING class, matching both codes' own doc comments
/// (`crate::diagnostic`).
fn check_envelope_fields(body: &RawRecord, identity: &str, diags: &mut Vec<Diagnostic>) {
    if let Err(violation) = canon_model::evidence::validate_envelope_shape(body) {
        let code = match body.0.as_object() {
            Some(obj) if envelope_field_is_present(obj, &violation.subject) => E_PLUGIN_BODY_TYPE,
            _ => E_PLUGIN_BODY_MISSING,
        };
        diags.push(diag(code, violation.detail, format!("{identity}.{}", violation.subject)));
    }
}

/// `true` iff the dot-joined `subject` path (`"schema"`, `"at"`,
/// `"actor.role"`, …) resolves to SOME value inside `obj` -- regardless
/// of whether that value is well-typed. Used only to tell an absent
/// envelope field from a present-but-invalid one (see
/// [`check_envelope_fields`]); a path through a non-object intermediate
/// (e.g. `actor.role` when `actor` itself isn't an object) is treated
/// as absent, since there is no field there to be "present".
fn envelope_field_is_present(obj: &serde_json::Map<String, Value>, subject: &str) -> bool {
    let mut parts = subject.split('.');
    let Some(first) = parts.next() else { return false };
    let Some(mut value) = obj.get(first) else { return false };
    for part in parts {
        let Some(next) = value.as_object().and_then(|m| m.get(part)) else { return false };
        value = next;
    }
    true
}

/// A plugin-aware writer validates a candidate overlay body against the
/// manifest's declared schema BEFORE ever calling `write_namespaced`
/// (design.md D4, tasks.md 2.4, `plugin-overlay-records` spec). The body
/// is treated as exactly three field groups: (a) [`OverlayEnvelope`]
/// (`schema`/`kind`/`at`/`actor`), (b) the REQUIRED join-key field(s)
/// named by `decl.join_key`, and (c) `decl`'s declared `fields`
/// (`type_accepts`-checked). All three groups must be present and
/// structurally typed; any field outside their union is rejected -- a
/// join-key field is NEVER mistaken for undeclared, and an overlay kind's
/// field set is closed exactly one level down from `RecordKind`'s own
/// twelve-kind closure. Every violation is accumulated (never stops at
/// the first), so a caller reports them all at once.
pub fn validate_overlay_body(decl: &OverlayDecl, body: &RawRecord) -> Result<(), Vec<Diagnostic>> {
    let mut diags = Vec::new();
    check_envelope_fields(body, &decl.identity, &mut diags);

    let Some(obj) = body.0.as_object() else {
        // `check_envelope_fields` already reported "not a JSON object";
        // nothing further to check against a non-object body.
        return Err(diags);
    };

    // F1: the same directory/content-kind invariant core's
    // `validate_kind_matches_content` enforces for the twelve closed
    // kinds (canon-store/src/partition.rs), one level down -- an
    // overlay body's own `kind` field must equal `decl.identity`, the
    // `<namespace>.<kind>` this validation call is FOR. Only checked
    // when `kind` is present as a string; a missing/wrong-typed `kind`
    // was already reported by `check_envelope_fields` above, and
    // reporting it again here as a spurious mismatch would be noise.
    if let Some(Value::String(kind)) = obj.get("kind") {
        if kind != &decl.identity {
            diags.push(diag(
                E_PLUGIN_BODY_KIND,
                format!("body kind `{kind}` does not match this overlay's identity `{}`", decl.identity),
                decl.identity.clone(),
            ));
        }
    }

    for key in &decl.join_key {
        match obj.get(key) {
            None => diags.push(diag(E_PLUGIN_BODY_MISSING, format!("missing required join-key field `{key}`"), decl.identity.clone())),
            Some(Value::String(_)) => {}
            Some(_) => diags.push(diag(E_PLUGIN_BODY_TYPE, format!("join-key field `{key}` is not a string"), decl.identity.clone())),
        }
    }

    for field in &decl.fields {
        match obj.get(&field.name) {
            None => diags.push(diag(E_PLUGIN_BODY_MISSING, format!("missing declared field `{}`", field.name), decl.identity.clone())),
            Some(value) if type_accepts(&field.ty, value) => {}
            Some(_) => diags.push(diag(E_PLUGIN_BODY_TYPE, format!("declared field `{}` does not match its manifest type", field.name), decl.identity.clone())),
        }
    }

    let allowed: HashSet<&str> = ["schema", "kind", "at", "actor"]
        .into_iter()
        .chain(decl.join_key.iter().map(String::as_str))
        .chain(decl.fields.iter().map(|f| f.name.as_str()))
        .collect();
    for key in obj.keys() {
        if !allowed.contains(key.as_str()) {
            diags.push(diag(
                E_PLUGIN_BODY_UNDECLARED,
                format!("field `{key}` is outside the overlay's declared schema (envelope \u{222A} join-key \u{222A} declared fields)"),
                decl.identity.clone(),
            ));
        }
    }

    if diags.is_empty() { Ok(()) } else { Err(diags) }
}

/// Derive an overlay's `natural_key` from its OWN validated body -- the
/// `__`-joined values of `decl.join_key`'s named fields, IN DECLARED
/// ORDER (design.md D4: "mirroring canon-store core's `resolve_partition`,
/// which derives a record's on-disk path FROM the record's own fields,
/// never from an out-of-band argument"), generalized to `OverlayDecl`'s
/// data-driven join key instead of a hardcoded per-`RecordKind` match arm.
/// Callable only AFTER [`validate_overlay_body`] has confirmed every
/// join-key field is present and string-typed -- `.expect`s that
/// guarantee rather than re-threading a second `Result`.
fn derive_natural_key(decl: &OverlayDecl, body: &RawRecord) -> String {
    decl.join_key
        .iter()
        .map(|field| {
            body.0
                .get(field)
                .and_then(Value::as_str)
                .expect("validate_overlay_body already confirmed every join-key field is present and string-typed")
        })
        .collect::<Vec<_>>()
        .join("__")
}

/// Everything that can keep an overlay from being written -- `body`
/// failed [`validate_overlay_body`], or the derived write itself failed
/// (`canon_store::tier::StoreError`, including `write_namespaced`'s own
/// defense-in-depth rejections).
#[derive(Debug)]
pub enum OverlayWriteError {
    Validation(Vec<Diagnostic>),
    Store(StoreError),
}

impl std::fmt::Display for OverlayWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverlayWriteError::Validation(diags) => {
                write!(f, "overlay body failed validation: ")?;
                for (i, d) in diags.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "[{}] {}: {}", d.code, d.subject, d.message)?;
                }
                Ok(())
            }
            OverlayWriteError::Store(e) => write!(f, "overlay write failed: {e}"),
        }
    }
}

impl std::error::Error for OverlayWriteError {}

/// The plugin-aware writer (design.md D4): [`validate_overlay_body`] ->
/// DERIVE `natural_key` from the validated body's own join-key field
/// values -> `GitTier::write_namespaced`. `natural_key` is NEVER an
/// independently supplied argument here -- the only value this function
/// ever passes to `write_namespaced` is the one it just derived from
/// `body` itself, so the natural_key/body-agreement invariant
/// `write_namespaced`'s own defense-in-depth check enforces can never
/// actually be exercised through THIS call path (it exists to catch a
/// hypothetical different/future/misbehaving caller of the primitive --
/// design.md R5's "defense in depth" framing).
pub fn write_overlay(tier: &GitTier, decl: &OverlayDecl, body: RawRecord) -> Result<NamespacedWriteReceipt, OverlayWriteError> {
    validate_overlay_body(decl, &body).map_err(OverlayWriteError::Validation)?;
    let natural_key = derive_natural_key(decl, &body);
    tier.write_namespaced(&decl.identity, &natural_key, body).map_err(OverlayWriteError::Store)
}

#[cfg(test)]
mod tests {
    use canon_model::ids::RoleId;
    use serde_json::json;

    use super::*;
    use crate::manifest::schema::FieldDecl;
    use crate::manifest::types::Type;

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

    fn well_formed_body() -> RawRecord {
        let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
        let mut fields = serde_json::Map::new();
        fields.insert("project_id".to_string(), json!("root"));
        fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
        fields.insert("covered".to_string(), json!(true));
        fields.insert("surface_ref".to_string(), json!(["world.hotdeal.01"]));
        compose_overlay_body(&envelope, fields)
    }

    // --- validate_overlay_body ---

    #[test]
    fn well_formed_body_passes_validation() {
        assert!(validate_overlay_body(&coverage_decl(), &well_formed_body()).is_ok());
    }

    #[test]
    fn join_key_fields_are_recognized_not_rejected_as_undeclared() {
        // The join-key fields (project_id/scenario_id) are NOT in
        // decl.fields -- validation must still accept them (group b),
        // never flagging them as outside the closed set.
        let decl = coverage_decl();
        let result = validate_overlay_body(&decl, &well_formed_body());
        assert!(result.is_ok(), "unexpected diagnostics: {result:?}");
    }

    #[test]
    fn missing_declared_field_is_rejected() {
        let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
        let mut fields = serde_json::Map::new();
        fields.insert("project_id".to_string(), json!("root"));
        fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
        fields.insert("covered".to_string(), json!(true));
        // surface_ref omitted entirely.
        let body = compose_overlay_body(&envelope, fields);
        let diags = validate_overlay_body(&coverage_decl(), &body).unwrap_err();
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_BODY_MISSING && d.message.contains("surface_ref")), "{diags:?}");
    }

    #[test]
    fn missing_join_key_field_is_rejected() {
        let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
        let mut fields = serde_json::Map::new();
        fields.insert("project_id".to_string(), json!("root"));
        // scenario_id omitted.
        fields.insert("covered".to_string(), json!(true));
        fields.insert("surface_ref".to_string(), json!([]));
        let body = compose_overlay_body(&envelope, fields);
        let diags = validate_overlay_body(&coverage_decl(), &body).unwrap_err();
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_BODY_MISSING && d.message.contains("scenario_id")), "{diags:?}");
    }

    #[test]
    fn undeclared_field_is_rejected() {
        let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
        let mut fields = serde_json::Map::new();
        fields.insert("project_id".to_string(), json!("root"));
        fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
        fields.insert("covered".to_string(), json!(true));
        fields.insert("surface_ref".to_string(), json!([]));
        fields.insert("notes".to_string(), json!("stray field")); // undeclared
        let body = compose_overlay_body(&envelope, fields);
        let diags = validate_overlay_body(&coverage_decl(), &body).unwrap_err();
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_BODY_UNDECLARED && d.message.contains("notes")), "{diags:?}");
    }

    #[test]
    fn wrong_typed_declared_field_is_rejected() {
        let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
        let mut fields = serde_json::Map::new();
        fields.insert("project_id".to_string(), json!("root"));
        fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
        fields.insert("covered".to_string(), json!("not-a-bool")); // wrong type
        fields.insert("surface_ref".to_string(), json!([]));
        let body = compose_overlay_body(&envelope, fields);
        let diags = validate_overlay_body(&coverage_decl(), &body).unwrap_err();
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_BODY_TYPE && d.message.contains("covered")), "{diags:?}");
    }

    #[test]
    fn wrong_typed_join_key_field_is_rejected() {
        let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
        let mut fields = serde_json::Map::new();
        fields.insert("project_id".to_string(), json!("root"));
        fields.insert("scenario_id".to_string(), json!(42)); // not a string
        fields.insert("covered".to_string(), json!(true));
        fields.insert("surface_ref".to_string(), json!([]));
        let body = compose_overlay_body(&envelope, fields);
        let diags = validate_overlay_body(&coverage_decl(), &body).unwrap_err();
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_BODY_TYPE && d.message.contains("scenario_id")), "{diags:?}");
    }

    #[test]
    fn all_violations_are_accumulated_not_just_the_first() {
        let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
        let mut fields = serde_json::Map::new();
        fields.insert("project_id".to_string(), json!("root"));
        fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
        // covered missing, surface_ref wrong-typed, notes undeclared.
        fields.insert("surface_ref".to_string(), json!("not-a-list"));
        fields.insert("notes".to_string(), json!("stray"));
        let body = compose_overlay_body(&envelope, fields);
        let diags = validate_overlay_body(&coverage_decl(), &body).unwrap_err();
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_BODY_MISSING));
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_BODY_TYPE));
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_BODY_UNDECLARED));
    }

    #[test]
    fn body_kind_disagreeing_with_decl_identity_is_rejected() {
        // F1: the body's own OverlayEnvelope.kind claims a DIFFERENT
        // overlay identity than the one this validation call is FOR --
        // the manifest-schema half of the directory/content-kind
        // invariant (store-layer half: `write_namespaced`/
        // `scan_namespaced_kind` in canon-store/src/git_tier.rs).
        let envelope = OverlayEnvelope::new(1, "other.coverage", Utc::now(), actor());
        let mut fields = serde_json::Map::new();
        fields.insert("project_id".to_string(), json!("root"));
        fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
        fields.insert("covered".to_string(), json!(true));
        fields.insert("surface_ref".to_string(), json!([]));
        let body = compose_overlay_body(&envelope, fields);
        let diags = validate_overlay_body(&coverage_decl(), &body).unwrap_err();
        assert!(
            diags.iter().any(|d| d.code == E_PLUGIN_BODY_KIND && d.message.contains("other.coverage") && d.message.contains("porting.coverage")),
            "{diags:?}"
        );
    }

    #[test]
    fn present_but_malformed_envelope_field_is_type_not_missing() {
        // F4: `at` PRESENT but not RFC3339 must be E-PLUGIN-BODY-TYPE,
        // never E-PLUGIN-BODY-MISSING -- only a truly ABSENT field
        // keeps the MISSING class.
        let mut body = well_formed_body();
        body.0.as_object_mut().unwrap().insert("at".to_string(), json!("not-rfc3339"));
        let diags = validate_overlay_body(&coverage_decl(), &body).unwrap_err();
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_BODY_TYPE && d.subject.ends_with(".at")), "{diags:?}");
        assert!(!diags.iter().any(|d| d.code == E_PLUGIN_BODY_MISSING), "{diags:?}");
    }

    #[test]
    fn absent_envelope_field_is_missing_not_type() {
        // F4's other half: an `at` field that is truly ABSENT keeps
        // E-PLUGIN-BODY-MISSING.
        let mut body = well_formed_body();
        body.0.as_object_mut().unwrap().remove("at");
        let diags = validate_overlay_body(&coverage_decl(), &body).unwrap_err();
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_BODY_MISSING && d.subject.ends_with(".at")), "{diags:?}");
        assert!(!diags.iter().any(|d| d.code == E_PLUGIN_BODY_TYPE && d.subject.ends_with(".at")), "{diags:?}");
    }

    // --- write_overlay ---

    #[test]
    fn write_overlay_derives_natural_key_from_join_key_in_declared_order() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let receipt = write_overlay(&tier, &coverage_decl(), well_formed_body()).unwrap();
        assert!(receipt.location.starts_with("kind=porting.coverage/root__world.hotdeal.01__"), "got {}", receipt.location);
    }

    #[test]
    fn write_overlay_round_trips_through_scan_namespaced_kind() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        write_overlay(&tier, &coverage_decl(), well_formed_body()).unwrap();
        let (records, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert!(violations.is_empty(), "{violations:?}");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].1.0["scenario_id"], "world.hotdeal.01");
    }

    #[test]
    fn write_overlay_never_calls_write_namespaced_when_validation_fails() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
        let mut fields = serde_json::Map::new();
        fields.insert("project_id".to_string(), json!("root"));
        fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
        // covered/surface_ref both missing.
        let body = compose_overlay_body(&envelope, fields);

        let err = write_overlay(&tier, &coverage_decl(), body).unwrap_err();
        assert!(matches!(err, OverlayWriteError::Validation(_)));
        assert!(!dir.path().join("kind=porting.coverage").exists(), "an invalid body must never reach disk");
    }

    #[test]
    fn write_overlay_byte_identical_resubmission_dedupes() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let at = Utc::now();
        let make_body = || {
            let envelope = OverlayEnvelope::new(1, "porting.coverage", at, actor());
            let mut fields = serde_json::Map::new();
            fields.insert("project_id".to_string(), json!("root"));
            fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
            fields.insert("covered".to_string(), json!(true));
            fields.insert("surface_ref".to_string(), json!(["world.hotdeal.01"]));
            compose_overlay_body(&envelope, fields)
        };
        let first = write_overlay(&tier, &coverage_decl(), make_body()).unwrap();
        let second = write_overlay(&tier, &coverage_decl(), make_body()).unwrap();
        assert!(!first.deduped);
        assert!(second.deduped);
        assert_eq!(first.location, second.location);

        let (records, _) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn write_overlay_logically_different_body_appends_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let decl = coverage_decl();

        let uncovered = {
            let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
            let mut fields = serde_json::Map::new();
            fields.insert("project_id".to_string(), json!("root"));
            fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
            fields.insert("covered".to_string(), json!(false));
            fields.insert("surface_ref".to_string(), json!([]));
            compose_overlay_body(&envelope, fields)
        };
        let covered = {
            let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
            let mut fields = serde_json::Map::new();
            fields.insert("project_id".to_string(), json!("root"));
            fields.insert("scenario_id".to_string(), json!("world.hotdeal.01"));
            fields.insert("covered".to_string(), json!(true));
            fields.insert("surface_ref".to_string(), json!(["world.hotdeal.01"]));
            compose_overlay_body(&envelope, fields)
        };

        let first = write_overlay(&tier, &decl, uncovered).unwrap();
        let second = write_overlay(&tier, &decl, covered).unwrap();
        assert_ne!(first.location, second.location);

        let (records, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert!(violations.is_empty());
        assert_eq!(records.len(), 2, "the flipped record must append, never overwrite the first");
    }
}
