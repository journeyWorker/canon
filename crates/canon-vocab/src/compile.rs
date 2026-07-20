//! Typed task-atom compile/decompile (design.md D2/D4, tasks 4.2/4.3): "a
//! validated atom → S1 `Task` record ... an atom that fails vocabulary
//! validation produces no `Task` record, only diagnostics" and "`Task`
//! record → atom, and prove round-trip equivalence (same `id`/`tag`/`attrs`,
//! and the decompiled atom itself passes validation)".
//!
//! # The `Task.evidence_note` interface gap (documented, not silently worked
//! # around)
//!
//! `canon_model::Task` has exactly ONE extension field beyond
//! `task_id`/`title`/`status`: `evidence_note: Option<String>`, a free
//! string (`crates/canon-model/src/records.rs:81-89`). It has no structured
//! `owner`/`evidence: {kind, ref}` field — mirroring `canon-gate`'s own
//! documented pattern for exactly this situation (`crates/canon-gate/src/
//! lib.rs`'s "INTERFACE REQUESTS to canon-model" doc comment: a gap this
//! crate's territory excludes closing, flagged for a future S1 change), this
//! module records the SAME kind of gap rather than silently working around
//! it: until `Task` grows a structured attrs/evidence field, `compile_task`
//! canonically JSON-encodes the atom's FULL `attrs` map into `evidence_note`
//! (not merely a human evidence note) so [`decompile_task`] can losslessly
//! reconstruct `attrs` — `title`/`status` are ALSO derived redundantly from
//! the same encoded map, so there is exactly one source of truth for a
//! compiled atom's data, never two fields that could disagree.

use std::collections::BTreeMap;

use canon_model::{Envelope, RecordKind, Task, TaskStatus};

use crate::atom::AtomRecord;
use crate::checker::{check_directive, Diagnostic};
use crate::manifest::snapshot::CapabilitySnapshot;
use crate::span::Severity;

const TASK_DIRECTIVE_TAG: &str = "task";

fn compile_diag(code: &str, message: String, subject: &str) -> Diagnostic {
    Diagnostic { code: code.to_string(), severity: Severity::Error, message, subject: subject.to_string() }
}

/// Compile a validated `{id, tag: "task", attrs}` atom to an S1 [`Task`].
/// Validates against `snapshot` FIRST (`check_directive`) — a vocabulary
/// violation produces no `Task`, only its diagnostics (task 4.2's
/// contract). `envelope` is caller-supplied: an atom carries no
/// actor/timestamp of its own.
pub fn compile_task(atom: &AtomRecord, snapshot: &CapabilitySnapshot, envelope: Envelope) -> Result<Task, Vec<Diagnostic>> {
    if atom.tag != TASK_DIRECTIVE_TAG {
        return Err(vec![compile_diag("E-NOT-A-TASK-ATOM", format!("atom `{}` has tag `::{}`, expected `::task`", atom.id, atom.tag), &atom.id)]);
    }

    let diags = check_directive(&atom.tag, &atom.attrs, snapshot, &atom.id);
    if !diags.is_empty() {
        return Err(diags);
    }

    let task_id = canon_model::TaskId::parse(&atom.id).map_err(|e| vec![compile_diag("E-INVALID-TASK-ID", e.to_string(), &atom.id)])?;

    let title = atom.attrs.get("desc").and_then(|v| v.as_str()).unwrap_or_default().to_string();

    let status = match atom.attrs.get("status").and_then(|v| v.as_str()) {
        Some("done") => TaskStatus::Done,
        _ => TaskStatus::Open, // checker already proved `status` is `open`|`done` when present at all.
    };

    let evidence_note = match serde_json::to_string(&atom.attrs) {
        Ok(json) => Some(json),
        Err(e) => return Err(vec![compile_diag("E-ATOM-ENCODE", format!("could not encode atom `{}` attrs: {e}", atom.id), &atom.id)]),
    };

    debug_assert_eq!(envelope.kind, RecordKind::Task);
    Ok(Task::new(envelope, task_id, title, status, evidence_note))
}

#[derive(Debug)]
pub struct DecompileError {
    pub message: String,
}

impl std::fmt::Display for DecompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DecompileError {}

/// Decompile a `Task` BACK to its atom (task 4.3). Only succeeds for a
/// `Task` [`compile_task`] itself produced (its `evidence_note` carries the
/// canonical attrs JSON, module doc) — a `Task` from any other source (no
/// `evidence_note`, or a human free-text one) has nothing to reconstruct
/// `attrs` from and yields a [`DecompileError`], never a panic or a
/// guessed/lossy atom.
pub fn decompile_task(task: &Task) -> Result<AtomRecord, DecompileError> {
    let Some(evidence_note) = &task.evidence_note else {
        return Err(DecompileError { message: format!("task `{}` has no evidence_note to decompile", task.task_id) });
    };
    let attrs: BTreeMap<String, serde_yaml::Value> =
        serde_json::from_str(evidence_note).map_err(|e| DecompileError { message: format!("task `{}` evidence_note is not a canon-vocab-compiled atom: {e}", task.task_id) })?;

    Ok(AtomRecord { id: task.task_id.to_string(), tag: TASK_DIRECTIVE_TAG.to_string(), attrs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::schema::DirectiveDecl;
    use crate::manifest::types::{AttrDecl, Type};
    use canon_model::{Actor, RoleId};
    use chrono::Utc;

    fn snapshot() -> CapabilitySnapshot {
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
        snap.evidence_kinds = vec!["test-run".into()];
        snap
    }

    fn envelope() -> Envelope {
        Envelope::new(1, RecordKind::Task, Utc::now(), Actor::new("test-agent", RoleId::parse("implementer").unwrap()))
    }

    fn valid_atom() -> AtomRecord {
        let mut attrs = BTreeMap::new();
        attrs.insert("desc".to_string(), serde_yaml::Value::String("wire the checker".into()));
        attrs.insert("owner".to_string(), serde_yaml::Value::String("alice".into()));
        attrs.insert("status".to_string(), serde_yaml::Value::String("open".into()));
        attrs.insert("evidence".to_string(), serde_yaml::to_value(BTreeMap::from([("kind", "test-run"), ("ref", "scenario://x")])).unwrap());
        AtomRecord { id: "s10-typed-authoring-vocabulary#4.2".to_string(), tag: "task".to_string(), attrs }
    }

    #[test]
    fn compiling_a_valid_atom_produces_a_task() {
        let snap = snapshot();
        let task = compile_task(&valid_atom(), &snap, envelope()).expect("compiles");
        assert_eq!(task.task_id.to_string(), "s10-typed-authoring-vocabulary#4.2");
        assert_eq!(task.title, "wire the checker");
        assert_eq!(task.status, TaskStatus::Open);
        assert!(task.evidence_note.is_some());
    }

    #[test]
    fn an_invalid_atom_produces_no_task_only_diagnostics() {
        let snap = snapshot();
        let mut atom = valid_atom();
        atom.attrs.insert("status".to_string(), serde_yaml::Value::String("closed".into()));
        let diags = compile_task(&atom, &snap, envelope()).unwrap_err();
        assert!(diags.iter().any(|d| d.code == "E-BAD-ENUM"));
    }

    #[test]
    fn an_evidence_value_with_an_unchecked_extra_key_produces_no_task() {
        let snap = snapshot();
        let mut atom = valid_atom();
        atom.attrs.insert(
            "evidence".to_string(),
            serde_yaml::to_value(BTreeMap::from([("kind", "test-run"), ("ref", "scenario://x"), ("unchecked", "y")])).unwrap(),
        );
        // The checker blocks the extra key upstream: compile_task never
        // reaches its attrs-map JSON encode, so the unchecked nested field
        // can never leak into `Task.evidence_note`.
        let diags = compile_task(&atom, &snap, envelope()).unwrap_err();
        assert!(diags.iter().any(|d| d.code == "E-UNKNOWN-ATTR"), "diags: {diags:?}");
    }

    #[test]
    fn compile_decompile_compile_round_trip_is_idempotent() {
        let snap = snapshot();
        let atom = valid_atom();
        let task1 = compile_task(&atom, &snap, envelope()).expect("compiles");
        let decompiled = decompile_task(&task1).expect("decompiles");
        assert_eq!(decompiled.id, atom.id);
        assert_eq!(decompiled.tag, atom.tag);
        assert_eq!(decompiled.attrs, atom.attrs);
        // The decompiled atom itself passes validation (task 4.3's contract).
        assert!(crate::checker::check_directive(&decompiled.tag, &decompiled.attrs, &snap, &decompiled.id).is_empty());

        let task2 = compile_task(&decompiled, &snap, envelope()).expect("recompiles");
        assert_eq!(task1.task_id, task2.task_id);
        assert_eq!(task1.title, task2.title);
        assert_eq!(task1.status, task2.status);
        assert_eq!(task1.evidence_note, task2.evidence_note);
    }

    #[test]
    fn a_task_not_compiled_by_canon_vocab_fails_to_decompile_instead_of_guessing() {
        let task = Task::new(envelope(), canon_model::TaskId::parse("s10-typed-authoring-vocabulary#9.9").unwrap(), "hand-authored", TaskStatus::Open, Some("just a note".to_string()));
        assert!(decompile_task(&task).is_err());
    }
}
