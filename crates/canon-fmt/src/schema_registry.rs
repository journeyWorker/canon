//! Consults `canon-model`'s REGISTERED artifact-family JSON schemas
//! (`canon_model::schema_export::family_schemas`) as the real validation
//! authority `canon fmt --check` was missing (S11 review finding
//! `artifact-family-schema` #1): the exact same `schema_for!`-derived
//! schemas the committed `schemas/family-<kind>.schema.json` files are
//! generated from (`canon_model::gen::write`/`check` enforce the two
//! never drift) are compiled once into [`jsonschema::Validator`]s here
//! and run against every family record's parsed content — never a
//! second, hand-maintained copy of the schema, and never bypassed by a
//! kind-shaped hand-rolled field check alone.
//!
//! # Enum-constraint diagnostics (S12 task 6.2)
//!
//! An enum-constraint violation is re-formatted into the mandated
//! "expected one of: …" shape (matching `canon-vocab/src/checker.rs::
//! check_enum_member`'s and `canon-policy/src/diagnostics.rs`'s own
//! phrasing — consistency, never a third format), sourcing the member
//! list from the SAME schema `jsonschema` just validated the value
//! against: the [`jsonschema::error::ValidationErrorKind::Enum`]
//! variant carries `options` — the resolved enum array from that
//! schema — so the member list is read straight off the one validation
//! authority, never a second, hand-maintained copy and never a
//! cross-crate call. (`canon-policy` owns the RecordKind-keyed
//! `SchemaRegistry::enum_domain` accessor that `canon context`'s
//! `resolve_surface` uses for canon's twelve internal envelope kinds,
//! S12 task 6.1; `canon-fmt` validates the SEPARATE, larger `FamilyKind`
//! vocabulary — `canon_model::family`'s own module doc — so it reads the
//! member list from its own compiled family schema rather than depending
//! on the CEL policy registry.) Every other violation kind keeps
//! `jsonschema`'s own message.

use std::collections::HashMap;
use std::sync::LazyLock;

use canon_model::schema_export::family_schemas;
use jsonschema::error::ValidationErrorKind;

/// Every [`canon_model::family::FamilyKind`]'s validator, keyed by its
/// wire string (`FamilyKind::as_str()` — `"run"`, `"divergence"`, …),
/// built once from the SAME `family_schemas()` call that produces the
/// committed schema files. A wire `kind` string absent from this map
/// has NO registered family schema at all — [`check`] reports that
/// distinctly from a schema VALIDATION failure, so a caller can tell
/// "unknown kind" apart from "known kind, non-conforming record".
static REGISTRY: LazyLock<HashMap<&'static str, jsonschema::Validator>> = LazyLock::new(|| {
    family_schemas()
        .into_iter()
        .map(|(kind, schema)| {
            let schema_value = serde_json::to_value(&schema).expect("a schemars-generated schema always serializes to JSON");
            let validator = jsonschema::validator_for(&schema_value).unwrap_or_else(|e| {
                panic!("registered `{}` family schema fails to compile as a JSON-schema validator: {e}", kind.as_str())
            });
            (kind.as_str(), validator)
        })
        .collect()
});

/// One family schema-conformance check's outcome for a single record.
#[derive(Debug, Clone)]
pub enum SchemaCheck {
    /// `kind_str` matches no registered [`canon_model::family::FamilyKind`]
    /// at all — the registry lookup itself found nothing to validate
    /// against (never silently treated as "conforms").
    NoRegisteredSchema,
    /// The record failed one or more of the registered schema's
    /// constraints — every violation message, in the validator's own
    /// order (S12 6.2: an enum-constraint violation is re-phrased into
    /// the mandated "expected one of: …" shape; every other violation
    /// keeps `jsonschema`'s own message).
    Violations(Vec<String>),
}

/// Validate `value` (a family record's full parsed content — JSON
/// natively, or a YAML document already converted to
/// [`serde_json::Value`]) against the family schema registered for
/// `kind_str`. `None` means `value` conforms; `Some(SchemaCheck::…)`
/// carries why it doesn't (or that no schema is registered at all).
pub fn check(kind_str: &str, value: &serde_json::Value) -> Option<SchemaCheck> {
    let Some(validator) = REGISTRY.get(kind_str) else {
        return Some(SchemaCheck::NoRegisteredSchema);
    };
    let errors: Vec<String> = validator.iter_errors(value).map(|e| format_violation(kind_str, &e)).collect();
    if errors.is_empty() {
        None
    } else {
        Some(SchemaCheck::Violations(errors))
    }
}

/// Re-derives the mandated "expected one of: …" phrasing (S12 6.2) for
/// an enum-constraint violation, sourcing the member list from the
/// [`ValidationErrorKind::Enum`] `options` — the resolved enum array
/// off the SAME schema `jsonschema` just validated `error` against, the
/// one validation authority (never a second, hand-maintained list).
/// Falls back to the raw `jsonschema` error text for every other
/// violation kind, and for an enum violation with no locatable field or
/// empty option set.
fn format_violation(kind_str: &str, error: &jsonschema::ValidationError<'_>) -> String {
    if let ValidationErrorKind::Enum { options } = error.kind() {
        if let (Some(field), Some(members)) = (last_path_segment(error.instance_path().as_str()), options.as_array()) {
            if !members.is_empty() {
                let render = |m: &serde_json::Value| m.as_str().map(str::to_string).unwrap_or_else(|| m.to_string());
                let rendered = members.iter().map(render).collect::<Vec<_>>().join(", ");
                let got = error.instance().as_str().map(str::to_string).unwrap_or_else(|| error.instance().to_string());
                return format!("`{got}` is not a valid value for `{field}` of `{kind_str}` (expected one of: {rendered})");
            }
        }
    }
    format!("{error} (at instance path `{}`)", error.instance_path())
}

/// The last non-empty `/`-delimited segment of a JSON pointer instance
/// path (e.g. `/kind` -> `kind`) — the violated field's name.
fn last_path_segment(path: &str) -> Option<String> {
    path.rsplit('/').find(|segment| !segment.is_empty()).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use canon_model::family::LedgerKind;

    use super::*;

    #[test]
    fn registry_covers_every_family_kind_wire_string() {
        for kind in canon_model::family::FamilyKind::ALL {
            assert!(REGISTRY.contains_key(kind.as_str()), "no registered schema validator for `{}`", kind.as_str());
        }
    }

    #[test]
    fn unknown_kind_reports_no_registered_schema() {
        let value = serde_json::json!({"schema": 1});
        match check("not-a-real-kind", &value) {
            Some(SchemaCheck::NoRegisteredSchema) => {}
            other => panic!("expected NoRegisteredSchema, got {other:?}"),
        }
    }

    #[test]
    fn a_conforming_run_record_has_no_schema_violations() {
        let value = serde_json::json!({
            "schema": 1,
            "kind": "run",
            "scenario_ids": ["settings.index.03"],
            "lane": "unit",
            "app_sha": "2745ca4c889d49f11aa96c51b2f2cf01a4be0009",
            "actor": {"agent_id": "flutter-test-machine"},
            "at": "2026-07-07T15:52:27.268418Z",
            "result": "pass",
            "evidence": []
        });
        assert!(check("run", &value).is_none());
    }

    #[test]
    fn a_run_record_missing_schema_and_at_is_a_violation() {
        let value = serde_json::json!({
            "kind": "run",
            "scenario_ids": ["settings.index.03"],
            "lane": "unit",
            "app_sha": "2745ca4c889d49f11aa96c51b2f2cf01a4be0009",
            "actor": {"agent_id": "flutter-test-machine"},
            "result": "pass",
            "evidence": []
        });
        match check("run", &value) {
            Some(SchemaCheck::Violations(errors)) => assert!(!errors.is_empty()),
            other => panic!("expected schema Violations for missing schema/at, got {other:?}"),
        }
    }

    #[test]
    fn a_run_record_with_wrong_type_scenario_ids_is_a_violation() {
        let value = serde_json::json!({
            "schema": 1,
            "kind": "run",
            "scenario_ids": "settings.index.03",
            "lane": "unit",
            "app_sha": "2745ca4c889d49f11aa96c51b2f2cf01a4be0009",
            "actor": {"agent_id": "flutter-test-machine"},
            "at": "2026-07-07T15:52:27.268418Z",
            "result": "pass",
            "evidence": []
        });
        match check("run", &value) {
            Some(SchemaCheck::Violations(errors)) => assert!(!errors.is_empty()),
            other => panic!("expected schema Violations for wrong-type scenario_ids, got {other:?}"),
        }
    }

    /// S12 task 6.3: an out-of-domain enum value gets the mandated
    /// "expected one of: …" shape, member list sourced from the SAME
    /// family schema `jsonschema` validated against (its `Enum` error's
    /// resolved `options`) — never the raw `jsonschema` error text.
    #[test]
    fn an_out_of_domain_kind_uses_the_mandated_expected_one_of_shape() {
        let value = serde_json::json!({
            "schema": 1,
            "kind": "not-a-real-kind",
            "scenario_ids": ["settings.index.03"],
            "lane": "unit",
            "app_sha": "2745ca4c889d49f11aa96c51b2f2cf01a4be0009",
            "actor": {"agent_id": "flutter-test-machine"},
            "at": "2026-07-07T15:52:27.268418Z",
            "result": "pass",
            "evidence": []
        });
        let members = LedgerKind::ALL.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ");
        let expected = format!("`not-a-real-kind` is not a valid value for `kind` of `run` (expected one of: {members})");

        match check("run", &value) {
            Some(SchemaCheck::Violations(errors)) => {
                assert!(errors.iter().any(|e| e == &expected), "expected {expected:?} among {errors:?}");
                assert!(expected.contains("expected one of: "), "sanity: the mandated shape itself must carry the phrase");
            }
            other => panic!("expected schema Violations for an out-of-domain `kind`, got {other:?}"),
        }
    }

    /// S12 task 5.2 (reflected-change): a schema enum edit (`severity`
    /// gains a third member) propagates into canon-fmt's own diagnostic
    /// member list with NO second, hand-maintained copy — the member
    /// list is read straight off the schema `jsonschema` validated
    /// against (its `Enum` error's `options`), so one schema edit is the
    /// only edit site. (`canon-policy`'s own `enum_domain` reflected
    /// behaviour is covered by `canon-policy/tests/reflected_change.rs`.)
    #[test]
    fn a_schema_enum_edit_propagates_into_the_canon_fmt_diagnostic() {
        fn severity_validator(members: &[&str]) -> jsonschema::Validator {
            jsonschema::validator_for(&serde_json::json!({
                "type": "object",
                "properties": {"severity": {"type": "string", "enum": members}},
                "required": ["severity"],
            }))
            .unwrap()
        }

        let before = severity_validator(&["low", "high"]);
        let before_value = serde_json::json!({"severity": "medium"});
        let before_error = before.iter_errors(&before_value).next().expect("`medium` is outside the two-member domain");
        assert_eq!(
            format_violation("synthetic", &before_error),
            "`medium` is not a valid value for `severity` of `synthetic` (expected one of: low, high)"
        );

        // `medium` now conforms after the edit — pick a value still outside the (now three-member) domain.
        let after = severity_validator(&["low", "medium", "high"]);
        let after_value = serde_json::json!({"severity": "critical"});
        let after_error = after.iter_errors(&after_value).next().expect("`critical` is outside the three-member domain");
        assert_eq!(
            format_violation("synthetic", &after_error),
            "`critical` is not a valid value for `severity` of `synthetic` (expected one of: low, medium, high)"
        );
    }
}
