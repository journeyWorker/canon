//! `canon gate task`'s PURE, DIALECT-FREE evidence decision (design
//! decision 6, D6; s35 `gate-plan-dialect-seam` sheds the markdown/
//! dialect knowledge this module used to carry).
//!
//! # What moved out (s35)
//! Before s35 this module WAS the checkbox grammar for one plan
//! dialect's `tasks.md` rows: it parsed and wrote those rows directly
//! (the
//! `TaskRow`/`parse_line`/`format_line` reader+writer). s35 moved that
//! grammar — and every trace of a plan dialect's on-disk shape — into
//! `canon-ingest`'s dialect-neutral `task_rows` module + the per-dialect
//! `plan_writeback::PlanWriteBack` seam. `canon-gate` is dialect-free:
//! it neither reads nor writes a `tasks.md` document, and has no
//! dependency on `canon-ingest`. The document mutation is
//! `canon-cli`'s job — it locates the plan document via the configured
//! dialect's `PlanWriteBack`, asks THIS crate for the pure evidence
//! decision, and delegates the flip back to that dialect.
//!
//! # The decision: evidence + notes -> approved note text | violations
//! [`gate_task`] is the pure fail-closed verdict: given a `task_id`, the
//! repo's [`EvidenceRecord`]s, and their paired [`EvidenceNote`]s, it
//! returns [`TaskFlipDecision::Approved`] (carrying the evidence-note
//! TEXT a `- [x] ` row's ` — ✅ ` suffix is built from) ONLY when a
//! matching, non-`Divergent` record exists AND the paired note (if any)
//! passes [`scan_fake_markers`] cleanly. Every other outcome is
//! [`TaskFlipDecision::Blocked`] carrying the [`Violation`]s that
//! blocked it (`unevidenced-flip` / `fabricated-evidence`) — missing,
//! non-`Faithful`, or fabricated evidence all fail CLOSED (§7 "malformed
//! evidence is no evidence"; spec.md "Flip is blocked with no evidence
//! record" / "Flip is blocked on malformed evidence").
//!
//! Only [`EvidenceVerdict::Divergent`] blocks the flip;
//! [`EvidenceVerdict::NotApplicable`] counts as passing alongside
//! [`EvidenceVerdict::Faithful`] — `Divergent` is the verdict type's own
//! explicit "the evidence says this did NOT hold" state, the one
//! outcome a task-completion claim cannot stand on.
//!
//! # Not a `GateCheck`
//! This is a pure function over `(task_id, evidence, notes)`, not over a
//! [`crate::GateContext`] — it is never a registered
//! [`crate::GateCheck`] (the checkbox flip is a targeted one-`task_id`
//! operation, not a whole-corpus scan). `canon-gate`'s own selftest
//! (`crate::selftest`) exercises it directly, building an
//! `(task_id, evidence, notes)` triple, since there is no document to
//! parse here at all.

use canon_model::{EvidenceRecord, EvidenceVerdict, TaskId};

use crate::markers::{scan_fake_markers, EvidenceNote};
use crate::{FailureClass, Violation};

/// The result of one [`gate_task`] evidence decision (s35: the pure
/// verdict, no document). `canon-cli` turns [`Approved`](Self::Approved)
/// into a `PlanWriteBack::flip_task` call carrying the note text, and
/// [`Blocked`](Self::Blocked) into a gate-red exit printing the
/// violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskFlipDecision {
    /// Evidence clears the flip. `evidence_note` is the one-line text a
    /// flipped row's ` — ✅ <evidence>` suffix is built from — the paired
    /// [`EvidenceNote::summary`] when one exists, else a default derived
    /// from the matching record's verdict/actor/timestamp.
    Approved { evidence_note: String },
    /// Evidence does NOT clear the flip — the row must stay unflipped.
    /// Carries every [`Violation`] that blocked it (`unevidenced-flip`
    /// and/or `fabricated-evidence`); never empty.
    Blocked { violations: Vec<Violation> },
}

fn default_evidence_text(record: &EvidenceRecord) -> String {
    format!("{:?} evidence recorded {} by {}", record.verdict, record.envelope.at.to_rfc3339(), record.envelope.actor.agent_id)
}

/// `canon gate task <task_id>`'s pure evidence decision (design decision
/// 6; spec.md "Evidence-gated task flip"/"Fabrication-marker scanning").
/// Approves the flip — returning the evidence-note TEXT a caller appends
/// as the row's ` — ✅ ` suffix — ONLY when `evidence` carries a
/// matching, non-`Divergent` [`EvidenceRecord`] for `task_id` AND the
/// paired [`EvidenceNote`] (by `task_id`, if any) passes
/// [`scan_fake_markers`] cleanly. Every other path is
/// [`TaskFlipDecision::Blocked`] with the violation(s) that blocked it
/// (module doc: fail closed).
///
/// This function knows NOTHING about the plan document: locating the
/// row, detecting an already-flipped/absent row, and applying the flip
/// are the caller's job (via `canon-ingest`'s `PlanWriteBack`, s35).
pub fn gate_task(task_id: &TaskId, evidence: &[EvidenceRecord], notes: &[EvidenceNote]) -> TaskFlipDecision {
    let matching = evidence.iter().find(|record| record.task_id.as_ref() == Some(task_id) && record.verdict != EvidenceVerdict::Divergent);

    let Some(record) = matching else {
        let violation = Violation::new(
            FailureClass::UnevidencedFlip,
            task_id.to_string(),
            "no matching, non-divergent EvidenceRecord found — missing or malformed evidence is no evidence".to_string(),
        );
        return TaskFlipDecision::Blocked { violations: vec![violation] };
    };

    let note = notes.iter().find(|note| &note.task_id == task_id);
    if let Some(note) = note {
        let scan_violations = scan_fake_markers(note);
        if !scan_violations.is_empty() {
            return TaskFlipDecision::Blocked { violations: scan_violations };
        }
    }

    let evidence_note = note.map(|note| note.summary.clone()).unwrap_or_else(|| default_evidence_text(record));
    TaskFlipDecision::Approved { evidence_note }
}

#[cfg(test)]
mod tests {
    use canon_model::{Actor, Envelope, RecordKind, RoleId};

    use super::*;

    fn evidence_record(task_id: &TaskId, verdict: EvidenceVerdict) -> EvidenceRecord {
        EvidenceRecord::new(
            Envelope::new(1, RecordKind::EvidenceRecord, chrono::Utc::now(), Actor::new("implementer", RoleId::parse("implementer").unwrap())),
            Some(task_id.clone()),
            None,
            None,
            verdict,
        )
    }

    fn blocked_classes(decision: &TaskFlipDecision) -> Vec<FailureClass> {
        match decision {
            TaskFlipDecision::Blocked { violations } => violations.iter().map(|v| v.class).collect(),
            TaskFlipDecision::Approved { .. } => Vec::new(),
        }
    }

    #[test]
    fn fails_closed_with_no_evidence_record() {
        let task_id = TaskId::parse("s5-trust-spine-gate#3.2").unwrap();
        let decision = gate_task(&task_id, &[], &[]);
        assert_eq!(blocked_classes(&decision), vec![FailureClass::UnevidencedFlip]);
    }

    #[test]
    fn fails_closed_on_a_divergent_verdict_malformed_evidence_is_no_evidence() {
        let task_id = TaskId::parse("s5-trust-spine-gate#3.2").unwrap();
        let record = evidence_record(&task_id, EvidenceVerdict::Divergent);
        let decision = gate_task(&task_id, &[record], &[]);
        assert_eq!(blocked_classes(&decision), vec![FailureClass::UnevidencedFlip]);
    }

    #[test]
    fn approves_with_clean_faithful_evidence_and_carries_the_note_text() {
        let task_id = TaskId::parse("s5-trust-spine-gate#3.2").unwrap();
        let record = evidence_record(&task_id, EvidenceVerdict::Faithful);
        let note = EvidenceNote::new(task_id.clone(), "cargo test -p canon-gate: 40 passed", Some("40 passed; 0 failed".to_string()));

        let decision = gate_task(&task_id, &[record], &[note]);

        assert_eq!(decision, TaskFlipDecision::Approved { evidence_note: "cargo test -p canon-gate: 40 passed".to_string() });
    }

    #[test]
    fn approves_with_a_default_note_when_no_evidence_note_companion_exists() {
        let task_id = TaskId::parse("s5-trust-spine-gate#3.2").unwrap();
        let record = evidence_record(&task_id, EvidenceVerdict::Faithful);

        let decision = gate_task(&task_id, &[record], &[]);

        match decision {
            TaskFlipDecision::Approved { evidence_note } => {
                assert!(evidence_note.starts_with("Faithful evidence recorded"), "{evidence_note}");
            }
            TaskFlipDecision::Blocked { .. } => panic!("clean faithful evidence must approve"),
        }
    }

    #[test]
    fn a_not_applicable_verdict_counts_as_passing_alongside_faithful() {
        let task_id = TaskId::parse("s5-trust-spine-gate#3.2").unwrap();
        let record = evidence_record(&task_id, EvidenceVerdict::NotApplicable);
        let decision = gate_task(&task_id, &[record], &[]);
        assert!(matches!(decision, TaskFlipDecision::Approved { .. }), "NotApplicable is not Divergent — it passes");
    }

    #[test]
    fn blocks_on_a_fabricated_evidence_note() {
        let task_id = TaskId::parse("s5-trust-spine-gate#3.2").unwrap();
        let record = evidence_record(&task_id, EvidenceVerdict::Faithful);
        let note = EvidenceNote::new(task_id.clone(), "TBD — will run later", None);

        let decision = gate_task(&task_id, &[record], &[note]);

        let classes = blocked_classes(&decision);
        assert!(!classes.is_empty());
        assert!(classes.iter().all(|c| *c == FailureClass::FabricatedEvidence), "{classes:?}");
    }
}
