//! The checker (design.md D2/D6): validates one `{tag, attrs}` record (a
//! task atom's or handoff body's shape, `crate::atom::AtomRecord`) against a
//! resolved [`crate::manifest::snapshot::CapabilitySnapshot`]. Retargeted
//! from the donor checker's `check_directive` — INSPIRE-only per design.md
//! open-Q1 (no donor checker crate dependency; the algorithm and diagnostic
//! codes/messages are ported, never imported).
//!
//! Diagnostic codes lifted verbatim (D6, from the donor checker):
//! `E-UNKNOWN-DIRECTIVE`, `E-UNKNOWN-ATTR`, `E-MISSING-ATTR`,
//! and the enum "expected one of: …" message — lifted byte-for-
//! byte from the donor checker — reused verbatim for
//! BOTH an inline/domain enum violation (`E-BAD-ENUM`) AND a D4 evidence-kind
//! violation (`E-BAD-EVIDENCE-KIND`, same message shape, listing the
//! policy-resolved kinds instead of static enum members) — design.md D4:
//! "an unrecognized kind yields the SAME 'expected one of: …' diagnostic".
//! One addition beyond the donor's shape, with no donor analog: `E-ATTR-TYPE`
//! for a structurally wrong (non-enum) attribute value, and
//! `E-UNKNOWN-DOMAIN` for a `Type::Domain(name)` referencing a name absent
//! from `enums.yaml`.
//!
//! From the moment this lands, these are canon's OWN stable failure-class
//! strings (D6: "never renamed without migrating both fixtures and hooks
//! that grep them"), independent of the donor's own codes evolving later.

use crate::manifest::snapshot::CapabilitySnapshot;
use crate::manifest::types::{type_accepts, Literal, Type};
use crate::span::Severity;

pub const E_UNKNOWN_DIRECTIVE: &str = "E-UNKNOWN-DIRECTIVE";
pub const E_UNKNOWN_ATTR: &str = "E-UNKNOWN-ATTR";
pub const E_MISSING_ATTR: &str = "E-MISSING-ATTR";
pub const E_BAD_ENUM: &str = "E-BAD-ENUM";
pub const E_BAD_EVIDENCE_KIND: &str = "E-BAD-EVIDENCE-KIND";
pub const E_ATTR_TYPE: &str = "E-ATTR-TYPE";
pub const E_UNKNOWN_DOMAIN: &str = "E-UNKNOWN-DOMAIN";

/// The checker's stable failure-class strings (design.md D6/task 3.3): from
/// the moment this change lands, every one of these is canon's OWN stable
/// failure-class string, "never renamed without migrating both fixtures and
/// hooks that grep them" (D6) — mirrors `canon_gate::FAILURE_CLASSES`'s own
/// `&'static str` array + stability-test pattern
/// (`crates/canon-gate/src/failure_class.rs`), scoped to THIS crate's own
/// checker codes (a different, unrelated closed vocabulary from
/// `canon_gate::FAILURE_CLASSES`'s eight gate-check classes or
/// `canon_model::FailureClass`'s five evidence-integrity classes).
pub const DIAGNOSTIC_CODES: [&str; 7] = [E_UNKNOWN_DIRECTIVE, E_UNKNOWN_ATTR, E_MISSING_ATTR, E_BAD_ENUM, E_BAD_EVIDENCE_KIND, E_ATTR_TYPE, E_UNKNOWN_DOMAIN];

/// One validation finding. `subject` identifies WHAT the diagnostic is about
/// (an atom id, optionally `<atom-id>.<attr-key>`) — the checker's anchor in
/// place of the donor's byte `Span` (`crate::span` module doc explains why no
/// span is threaded here).
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub subject: String,
}

fn diag(code: &str, message: String, subject: &str) -> Diagnostic {
    Diagnostic { code: code.to_string(), severity: Severity::Error, message, subject: subject.to_string() }
}

/// Validate one `{tag, attrs}` record against `snapshot` (the donor checker's
/// algorithm). `subject` is this record's id, used to
/// anchor every diagnostic this call produces. An empty vec means `tag` and
/// every supplied attr are well-formed against the snapshot.
pub fn check_directive(tag: &str, attrs: &std::collections::BTreeMap<String, serde_yaml::Value>, snapshot: &CapabilitySnapshot, subject: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    let Some(decl) = snapshot.directive(tag) else {
        // plugin §11.2 fix-it parity: an installed-but-inactive tag still
        // gets the plain error (canon-vocab has no LSP fix-it object yet to
        // attach — `snapshot.inactive` is available for a future one).
        diags.push(diag(E_UNKNOWN_DIRECTIVE, format!("unknown directive `::{tag}`"), subject));
        return diags;
    };

    for (key, value) in attrs {
        let Some(adecl) = decl.attrs.iter().find(|a| &a.name == key) else {
            diags.push(diag(E_UNKNOWN_ATTR, format!("`::{tag}` has no attribute `{key}`"), subject));
            continue;
        };
        check_attr_value(tag, key, &adecl.ty, value, snapshot, subject, &mut diags);
    }

    for adecl in decl.attrs.iter().filter(|a| a.required) {
        if !attrs.contains_key(&adecl.name) {
            diags.push(diag(E_MISSING_ATTR, format!("`::{tag}` requires attribute `{}`", adecl.name), subject));
        }
    }

    diags
}

fn check_attr_value(tag: &str, key: &str, ty: &Type, value: &serde_yaml::Value, snapshot: &CapabilitySnapshot, subject: &str, diags: &mut Vec<Diagnostic>) {
    match ty {
        Type::Enum(members) => check_enum_member(tag, key, members, value, subject, diags),
        Type::Domain(name) => match snapshot.enums.get(name) {
            Some(members) => check_enum_member(tag, key, members, value, subject, diags),
            None => diags.push(diag(E_UNKNOWN_DOMAIN, format!("attribute `{key}` of `::{tag}` references unknown domain `{name}`"), subject)),
        },
        Type::Evidence => check_evidence_value(tag, key, value, snapshot, subject, diags),
        ty => {
            let Some(lit) = Literal::from_yaml(value) else {
                diags.push(diag(E_ATTR_TYPE, format!("attribute `{key}` of `::{tag}` expects {}", describe(ty)), subject));
                return;
            };
            if !type_accepts(ty, &lit) {
                diags.push(diag(E_ATTR_TYPE, format!("attribute `{key}` of `::{tag}` expects {}", describe(ty)), subject));
            }
        }
    }
}

/// Enum-membership check (the donor checker's verbatim "expected one of: …"
/// message) — shared by `Type::Enum` and a
/// resolved `Type::Domain`.
fn check_enum_member(tag: &str, key: &str, members: &[String], value: &serde_yaml::Value, subject: &str, diags: &mut Vec<Diagnostic>) {
    let Some(got) = value.as_str() else {
        diags.push(diag(E_ATTR_TYPE, format!("attribute `{key}` of `::{tag}` expects an enum string"), subject));
        return;
    };
    if !members.iter().any(|m| m == got) {
        diags.push(diag(E_BAD_ENUM, format!("`{got}` is not a valid value for `{key}` of `::{tag}` (expected one of: {})", members.join(", ")), subject));
    }
}

/// D4: `evidence: {kind, ref}`. Structural shape first (a mapping with both
/// keys present as strings, and NO other keys — the typed evidence contract
/// is exactly `{kind, ref}`; any additional key is rejected with
/// `E_UNKNOWN_ATTR` (the same class a top-level unrecognized attribute gets)
/// rather than silently passing through `compile_task`'s attrs-map JSON
/// encode into `Task.evidence_note` unchecked. `kind` membership is then
/// checked against `snapshot.evidence_kinds` with the SAME "expected one
/// of: …" message (`E_BAD_EVIDENCE_KIND`, design.md D4).
fn check_evidence_value(tag: &str, key: &str, value: &serde_yaml::Value, snapshot: &CapabilitySnapshot, subject: &str, diags: &mut Vec<Diagnostic>) {
    let Some(map) = value.as_mapping() else {
        diags.push(diag(E_ATTR_TYPE, format!("attribute `{key}` of `::{tag}` expects a mapping `{{kind, ref}}`"), subject));
        return;
    };

    let kind_key = serde_yaml::Value::String("kind".into());
    let ref_key = serde_yaml::Value::String("ref".into());
    for map_key in map.keys() {
        if *map_key != kind_key && *map_key != ref_key {
            let shown = map_key.as_str().map(str::to_string).unwrap_or_else(|| format!("{map_key:?}"));
            diags.push(diag(
                E_UNKNOWN_ATTR,
                format!("evidence `{key}` of `::{tag}` has no field `{shown}` (expected one of: kind, ref)"),
                subject,
            ));
        }
    }

    let kind = map.get(&kind_key).and_then(|v| v.as_str());
    let evidence_ref = map.get(&ref_key).and_then(|v| v.as_str());

    match kind {
        None => diags.push(diag(E_MISSING_ATTR, format!("attribute `{key}` of `::{tag}` requires `kind`"), subject)),
        Some(k) if !snapshot.evidence_kinds.iter().any(|m| m == k) => diags.push(diag(
            E_BAD_EVIDENCE_KIND,
            format!("`{k}` is not a valid value for `kind` of `::{tag}` (expected one of: {})", snapshot.evidence_kinds.join(", ")),
            subject,
        )),
        Some(_) => {}
    }
    if evidence_ref.is_none() {
        diags.push(diag(E_MISSING_ATTR, format!("attribute `{key}` of `::{tag}` requires `ref`"), subject));
    }
}

fn describe(ty: &Type) -> &'static str {
    match ty {
        Type::Bool => "a boolean",
        Type::Number => "a number",
        Type::Str => "a string",
        Type::List(_) => "a list",
        Type::Enum(_) | Type::Domain(_) => "an enum value",
        Type::Evidence => "an evidence record",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::schema::DirectiveDecl;
    use crate::manifest::types::AttrDecl;
    use std::collections::BTreeMap;

    fn snapshot_with_task_directive() -> CapabilitySnapshot {
        let mut snap = CapabilitySnapshot::default();
        snap.directives.insert(
            "task".to_string(),
            DirectiveDecl {
                name: "task".into(),
                attrs: vec![
                    AttrDecl { name: "desc".into(), required: true, ty: Type::Str, default: None },
                    AttrDecl { name: "owner".into(), required: false, ty: Type::Str, default: None },
                    AttrDecl { name: "status".into(), required: true, ty: Type::Domain("task-status".into()), default: None },
                    AttrDecl { name: "evidence".into(), required: true, ty: Type::Evidence, default: None },
                ],
            },
        );
        snap.enums.insert("task-status".to_string(), vec!["open".into(), "done".into()]);
        snap.evidence_kinds = vec!["test-run".into(), "manual-review".into()];
        snap
    }

    fn attrs(pairs: &[(&str, &str)]) -> BTreeMap<String, serde_yaml::Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), serde_yaml::Value::String(v.to_string()))).collect()
    }

    #[test]
    fn unknown_directive_is_reported() {
        let snap = snapshot_with_task_directive();
        let diags = check_directive("bogus", &BTreeMap::new(), &snap, "atom-1");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, E_UNKNOWN_DIRECTIVE);
    }

    #[test]
    fn unknown_attr_is_reported() {
        let snap = snapshot_with_task_directive();
        let mut a = attrs(&[("desc", "x"), ("status", "open")]);
        a.insert("evidence".into(), serde_yaml::to_value(std::collections::BTreeMap::from([("kind", "test-run"), ("ref", "r")])).unwrap());
        a.insert("bogus-attr".into(), serde_yaml::Value::String("x".into()));
        let diags = check_directive("task", &a, &snap, "atom-1");
        assert!(diags.iter().any(|d| d.code == E_UNKNOWN_ATTR));
    }

    #[test]
    fn missing_required_attr_is_reported() {
        let snap = snapshot_with_task_directive();
        let a = attrs(&[("desc", "x")]);
        let diags = check_directive("task", &a, &snap, "atom-1");
        assert!(diags.iter().any(|d| d.code == E_MISSING_ATTR && d.message.contains("status")));
        assert!(diags.iter().any(|d| d.code == E_MISSING_ATTR && d.message.contains("evidence")));
    }

    #[test]
    fn bad_enum_value_carries_the_verbatim_expected_one_of_message() {
        let snap = snapshot_with_task_directive();
        let mut a = attrs(&[("desc", "x"), ("status", "closed")]);
        a.insert("evidence".into(), serde_yaml::to_value(std::collections::BTreeMap::from([("kind", "test-run"), ("ref", "r")])).unwrap());
        let diags = check_directive("task", &a, &snap, "atom-1");
        let bad = diags.iter().find(|d| d.code == E_BAD_ENUM).expect("bad enum diagnostic");
        assert_eq!(bad.message, "`closed` is not a valid value for `status` of `::task` (expected one of: open, done)");
    }

    #[test]
    fn evidence_kind_outside_policy_domain_is_rejected_with_the_invalid_kinds_list() {
        let snap = snapshot_with_task_directive();
        let mut a = attrs(&[("desc", "x"), ("status", "open")]);
        a.insert("evidence".into(), serde_yaml::to_value(std::collections::BTreeMap::from([("kind", "totally-made-up"), ("ref", "r")])).unwrap());
        let diags = check_directive("task", &a, &snap, "atom-1");
        let bad = diags.iter().find(|d| d.code == E_BAD_EVIDENCE_KIND).expect("bad evidence kind diagnostic");
        assert_eq!(bad.message, "`totally-made-up` is not a valid value for `kind` of `::task` (expected one of: test-run, manual-review)");
    }

    #[test]
    fn evidence_with_an_unchecked_extra_key_is_rejected_not_silently_passed() {
        let snap = snapshot_with_task_directive();
        let mut a = attrs(&[("desc", "x"), ("status", "open")]);
        a.insert(
            "evidence".into(),
            serde_yaml::to_value(std::collections::BTreeMap::from([("kind", "test-run"), ("ref", "x"), ("unchecked", "y")])).unwrap(),
        );
        let diags = check_directive("task", &a, &snap, "atom-1");
        let bad = diags.iter().find(|d| d.code == E_UNKNOWN_ATTR).expect("unknown evidence field diagnostic");
        assert_eq!(bad.message, "evidence `evidence` of `::task` has no field `unchecked` (expected one of: kind, ref)");
        // A clean `{kind, ref}` evidence value still validates with no diagnostics at all.
        let mut clean = attrs(&[("desc", "x"), ("status", "open")]);
        clean.insert("evidence".into(), serde_yaml::to_value(std::collections::BTreeMap::from([("kind", "test-run"), ("ref", "x")])).unwrap());
        assert!(check_directive("task", &clean, &snap, "atom-1").is_empty());
    }

    #[test]
    fn well_formed_atom_validates_clean() {
        let snap = snapshot_with_task_directive();
        let mut a = attrs(&[("desc", "x"), ("status", "open"), ("owner", "alice")]);
        a.insert("evidence".into(), serde_yaml::to_value(std::collections::BTreeMap::from([("kind", "test-run"), ("ref", "r")])).unwrap());
        let diags = check_directive("task", &a, &snap, "atom-1");
        assert!(diags.is_empty(), "diags: {diags:?}");
    }

    #[test]
    fn diagnostic_codes_registry_has_no_duplicates_and_every_code_is_e_prefixed() {
        let mut seen = std::collections::BTreeSet::new();
        for code in DIAGNOSTIC_CODES {
            assert!(seen.insert(code), "duplicate diagnostic code: {code}");
            assert!(code.starts_with("E-"), "not a stable E-* failure class: {code}");
        }
    }
}
