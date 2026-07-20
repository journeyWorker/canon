//! Vocabulary-defined handoff-body compile + render (design.md D5, task
//! 5.2): "a handoff's `domain` field selects the directive tag; validate the
//! handoff body against that directive through the §3 checker, unchanged."
//!
//! Compiles to [`canon_model::HandoffBody`] — `{domain, template_version,
//! fields}` — NOT to [`canon_model::Handoff`] itself: `HandoffBody` is the
//! typed-vocabulary staging area; `Handoff`'s 13 state-machine fields (`id`,
//! `state`, `chainId`, …) are untouched by this module and by design (task
//! 5.3: "body-validation failure never mutates or blocks the S1 `Handoff`
//! state-machine fields" — this module never constructs or reads a `Handoff`
//! at all, only its `body`).
//!
//! Unlike [`crate::compile::compile_task`]/[`crate::compile::decompile_task`],
//! there is no `decompile_handoff_body`: `HandoffBody` carries no `id` of its
//! own to seed an [`crate::atom::AtomRecord`] with (a `Handoff`'s `id` lives
//! on the OUTER `Handoff` record, out of this module's territory) — this
//! change's own acceptance bar (assignment "Compile + round-trip") states
//! the round-trip property for task atoms; handoff bodies "validate + render
//! through the same pipeline", which this module's two functions cover.
//!
//! S10's canon.core handoff directive tags (`handoff-dev`/`handoff-design`/
//! `handoff-content`/`handoff-test`) use ENGLISH domain names, distinct from
//! S1's already-shipped `GihoekTemplate`'s Korean domain id (`기획`,
//! `crates/canon-model/src/handoff.rs:126-179`). `DomainId` is an opaque
//! string, so both coexist without conflict; reconciling S1's Korean
//! `TemplateRegistry` domain vocabulary with S10's English directive names
//! (e.g. registering an English-domain `HandoffTemplate` impl, or migrating
//! `GihoekTemplate` to an English id) is a follow-up concern for whichever
//! future change wires this compiler's output into S1's `TemplateRegistry`
//! — not required by this change's acceptance bar, which is the typed
//! vocabulary's OWN validate + compile + render pipeline.

const HANDOFF_TAG_PREFIX: &str = "handoff-";
const DEFAULT_TEMPLATE_VERSION: u32 = 1;

use canon_model::{DomainId, HandoffBody};

use crate::atom::AtomRecord;
use crate::checker::{check_directive, Diagnostic};
use crate::manifest::snapshot::CapabilitySnapshot;
use crate::span::Severity;

fn diag(code: &str, message: String, subject: &str) -> Diagnostic {
    Diagnostic { code: code.to_string(), severity: Severity::Error, message, subject: subject.to_string() }
}

/// Compile a validated `{id, tag: "handoff-<domain>", attrs}` atom to a
/// [`HandoffBody`]. Validates against `snapshot` FIRST — a vocabulary
/// violation (missing required field -> `E-MISSING-ATTR`, undeclared domain
/// -> `E-UNKNOWN-DIRECTIVE`) produces no body, only diagnostics.
pub fn compile_handoff_body(atom: &AtomRecord, snapshot: &CapabilitySnapshot) -> Result<HandoffBody, Vec<Diagnostic>> {
    let Some(domain_str) = atom.tag.strip_prefix(HANDOFF_TAG_PREFIX) else {
        return Err(vec![diag("E-NOT-A-HANDOFF-ATOM", format!("atom `{}` has tag `::{}`, expected `::handoff-<domain>`", atom.id, atom.tag), &atom.id)]);
    };

    let diags = check_directive(&atom.tag, &atom.attrs, snapshot, &atom.id);
    if !diags.is_empty() {
        return Err(diags);
    }

    let domain = DomainId::parse(domain_str).map_err(|e| vec![diag("E-INVALID-HANDOFF-DOMAIN", e.to_string(), &atom.id)])?;

    let fields = serde_json::to_value(&atom.attrs).map_err(|e| vec![diag("E-ATOM-ENCODE", format!("could not encode atom `{}` attrs: {e}", atom.id), &atom.id)])?;

    Ok(HandoffBody { domain, template_version: DEFAULT_TEMPLATE_VERSION, fields })
}

/// Render a compiled [`HandoffBody`] to human-readable text — a generic,
/// field-name-titled renderer (unlike `GihoekTemplate::render`'s hand-
/// written per-domain layout, this covers every canon.core handoff directive
/// uniformly: one `## <field>` section per attr, in sorted-key order,
/// string values verbatim, array values as a bullet list). Never panics on
/// an unexpected `fields` shape (an atom that reaches here already passed
/// [`compile_handoff_body`]'s checker validation, but this function does not
/// assume that — a `null`/other JSON shape renders its `Debug` form rather
/// than panicking).
pub fn render_handoff_body(body: &HandoffBody) -> String {
    let mut out = format!("# Handoff: {}\n\n", body.domain.as_str());
    let Some(map) = body.fields.as_object() else {
        return out;
    };
    for (key, value) in map {
        out.push_str(&format!("## {key}\n\n"));
        match value {
            serde_json::Value::String(s) => out.push_str(&format!("{s}\n\n")),
            serde_json::Value::Array(items) => {
                for item in items {
                    let line = item.as_str().map(str::to_string).unwrap_or_else(|| item.to_string());
                    out.push_str(&format!("- {line}\n"));
                }
                out.push('\n');
            }
            other => out.push_str(&format!("{other}\n\n")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::schema::DirectiveDecl;
    use crate::manifest::types::{AttrDecl, Type};
    use std::collections::BTreeMap;

    fn snapshot() -> CapabilitySnapshot {
        let mut snap = CapabilitySnapshot::default();
        snap.directives.insert(
            "handoff-dev".to_string(),
            DirectiveDecl {
                name: "handoff-dev".into(),
                attrs: vec![
                    AttrDecl { name: "title".into(), required: true, ty: Type::Str, default: None },
                    AttrDecl { name: "summary".into(), required: true, ty: Type::Str, default: None },
                    AttrDecl { name: "verification-steps".into(), required: true, ty: Type::List(Box::new(Type::Str)), default: None },
                ],
            },
        );
        snap
    }

    fn valid_atom() -> AtomRecord {
        let mut attrs = BTreeMap::new();
        attrs.insert("title".to_string(), serde_yaml::Value::String("wire the checker".into()));
        attrs.insert("summary".to_string(), serde_yaml::Value::String("ports check_directive".into()));
        attrs.insert("verification-steps".to_string(), serde_yaml::Value::Sequence(vec![serde_yaml::Value::String("cargo test -p canon-vocab".into())]));
        AtomRecord { id: "handoff-1".to_string(), tag: "handoff-dev".to_string(), attrs }
    }

    #[test]
    fn compiling_a_valid_handoff_atom_produces_a_body() {
        let snap = snapshot();
        let body = compile_handoff_body(&valid_atom(), &snap).expect("compiles");
        assert_eq!(body.domain.as_str(), "dev");
        assert_eq!(body.template_version, DEFAULT_TEMPLATE_VERSION);
        assert_eq!(body.fields.get("title").and_then(|v| v.as_str()), Some("wire the checker"));
    }

    #[test]
    fn missing_required_field_yields_e_missing_attr() {
        let snap = snapshot();
        let mut atom = valid_atom();
        atom.attrs.remove("summary");
        let diags = compile_handoff_body(&atom, &snap).unwrap_err();
        assert!(diags.iter().any(|d| d.code == "E-MISSING-ATTR" && d.message.contains("summary")));
    }

    #[test]
    fn undeclared_domain_yields_e_unknown_directive() {
        let snap = snapshot();
        let mut atom = valid_atom();
        atom.tag = "handoff-nonexistent".to_string();
        let diags = compile_handoff_body(&atom, &snap).unwrap_err();
        assert!(diags.iter().any(|d| d.code == "E-UNKNOWN-DIRECTIVE"));
    }

    #[test]
    fn render_produces_titled_sections_for_every_field() {
        let snap = snapshot();
        let body = compile_handoff_body(&valid_atom(), &snap).expect("compiles");
        let rendered = render_handoff_body(&body);
        assert!(rendered.contains("# Handoff: dev"));
        assert!(rendered.contains("## title"));
        assert!(rendered.contains("wire the checker"));
        assert!(rendered.contains("- cargo test -p canon-vocab"));
    }

    /// Task 5.3: a body-validation failure produces NO `HandoffBody` at all
    /// (`compile_handoff_body` returns `Err`, module doc) -- there is
    /// therefore no value to pass to `Handoff::new`, which REQUIRES an
    /// already-constructed `HandoffBody`. This test proves the converse
    /// half of the isolation claim directly: the `Handoff` state machine
    /// (`id`/`state`/`chainId`/`parentHandoffId`/`seq`/`claimedBy`/
    /// `openspecChangeSlug`) is fully exercisable through its own full
    /// pending -> in-progress -> done transition sequence using a body that
    /// would FAIL this crate's own checker (an undeclared domain) -- S1's
    /// state machine has zero coupling to this crate's vocabulary checker,
    /// by construction (`canon_model::Handoff`/`HandoffState` are never
    /// referenced anywhere else in this module).
    #[test]
    fn handoff_state_machine_is_unaffected_by_a_body_that_fails_the_vocabulary_checker() {
        use canon_model::{Actor, DomainId, Envelope, Handoff, HandoffId, HandoffState, RecordKind, RoleId};
        use chrono::Utc;

        let snap = snapshot();
        let mut atom = valid_atom();
        atom.tag = "handoff-nonexistent".to_string();
        assert!(compile_handoff_body(&atom, &snap).is_err(), "the checker must reject this body");

        // A body the vocabulary checker never blessed -- constructed
        // directly, exactly like a caller that skipped this crate entirely.
        let unchecked_body = HandoffBody { domain: DomainId::parse("nonexistent").unwrap(), template_version: 1, fields: serde_json::json!({}) };
        let envelope = Envelope::new(1, RecordKind::Handoff, Utc::now(), Actor::new("test-agent", RoleId::parse("implementer").unwrap()));
        let id = HandoffId::parse("20260711-0000-selftest-abcd").unwrap();
        let mut handoff = Handoff::new(envelope, id.clone(), uuid::Uuid::nil(), None, 1, "state machine isolation proof", None, unchecked_body);

        assert_eq!(handoff.id, id);
        assert_eq!(handoff.state, HandoffState::Pending);

        let now = Utc::now();
        handoff.transition_to(HandoffState::InProgress, now, Some("test-agent")).expect("pending -> in-progress");
        assert_eq!(handoff.claimed_by.as_deref(), Some("test-agent"));
        handoff.transition_to(HandoffState::Done, now, None).expect("in-progress -> done");
        assert_eq!(handoff.state, HandoffState::Done);
        assert!(handoff.completed_at.is_some());
    }
}
