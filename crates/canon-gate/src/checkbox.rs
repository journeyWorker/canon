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
//! matching, non-`Divergent` record exists AND every note paired with
//! the task passes [`scan_fake_markers`] cleanly. The approved text
//! aggregates every record bound to the task (0.14 D5). Every other outcome is
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
    /// flipped row's ` — ✅ <evidence>` suffix is built from: the count of
    /// every record bound to the task by verdict, then the latest
    /// [`EvidenceNote::summary`] (or, with none, a default derived from
    /// the latest passing record's verdict/actor/timestamp).
    Approved { evidence_note: String },
    /// Evidence does NOT clear the flip — the row must stay unflipped.
    /// Carries every [`Violation`] that blocked it (`unevidenced-flip`
    /// and/or `fabricated-evidence`); never empty.
    Blocked { violations: Vec<Violation> },
}

fn default_evidence_text(record: &EvidenceRecord) -> String {
    format!("{:?} evidence recorded {} by {}", record.verdict, record.envelope.at.to_rfc3339(), record.envelope.actor.agent_id)
}

/// The one-line row suffix (0.14 D5): every record bound to the task,
/// counted by verdict, then the latest summary —
/// `3 evidence records (2 faithful, 1 not-applicable); latest: <summary>`.
/// A row used to carry ONE record's summary, so a task backed by nine
/// records read as if one test had run.
fn aggregate_note(records: &[&EvidenceRecord], latest: &str) -> String {
    let count = |verdict| records.iter().filter(|r| r.verdict == verdict).count();
    let counts: Vec<String> = [
        (EvidenceVerdict::Faithful, "faithful"),
        (EvidenceVerdict::NotApplicable, "not-applicable"),
        (EvidenceVerdict::Divergent, "divergent"),
    ]
    .into_iter()
    .filter_map(|(verdict, name)| match count(verdict) {
        0 => None,
        n => Some(format!("{n} {name}")),
    })
    .collect();
    let noun = if records.len() == 1 { "record" } else { "records" };
    format!("{} evidence {noun} ({}); latest: {latest}", records.len(), counts.join(", "))
}

/// `canon gate task <task_id>`'s pure evidence decision (design decision
/// 6; spec.md "Evidence-gated task flip"/"Fabrication-marker scanning").
/// Approves the flip — returning the evidence-note TEXT a caller appends
/// as the row's ` — ✅ ` suffix — ONLY when `evidence` carries a
/// matching, non-`Divergent` [`EvidenceRecord`] for `task_id` AND every
/// [`EvidenceNote`] paired with the task passes [`scan_fake_markers`]
/// cleanly (the row aggregates them all, so any fabricated note taints
/// it). Every other path is [`TaskFlipDecision::Blocked`] with the
/// violation(s) that blocked it (module doc: fail closed).
///
/// `notes` are in ledger time order, oldest first: the LAST note for the
/// task is its latest summary. The approved text is [`aggregate_note`]
/// over every record bound to the task.
///
/// This function knows NOTHING about the plan document: locating the
/// row, detecting an already-flipped/absent row, and applying the flip
/// are the caller's job (via `canon-ingest`'s `PlanWriteBack`, s35).
pub fn gate_task(task_id: &TaskId, evidence: &[EvidenceRecord], notes: &[EvidenceNote]) -> TaskFlipDecision {
    let records: Vec<&EvidenceRecord> = evidence.iter().filter(|record| record.task_id.as_ref() == Some(task_id)).collect();
    let latest_passing = records.iter().copied().filter(|record| record.verdict != EvidenceVerdict::Divergent).max_by_key(|record| record.envelope.at);

    let Some(record) = latest_passing else {
        let violation = Violation::new(
            FailureClass::UnevidencedFlip,
            task_id.to_string(),
            "no matching, non-divergent EvidenceRecord found — missing or malformed evidence is no evidence".to_string(),
        );
        return TaskFlipDecision::Blocked { violations: vec![violation] };
    };

    let task_notes: Vec<&EvidenceNote> = notes.iter().filter(|note| &note.task_id == task_id).collect();
    let scan_violations: Vec<Violation> = task_notes.iter().flat_map(|note| scan_fake_markers(note)).collect();
    if !scan_violations.is_empty() {
        return TaskFlipDecision::Blocked { violations: scan_violations };
    }

    let latest = task_notes.last().map(|note| note.summary.clone()).unwrap_or_else(|| default_evidence_text(record));
    TaskFlipDecision::Approved { evidence_note: aggregate_note(&records, &latest) }
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

        assert_eq!(decision, TaskFlipDecision::Approved { evidence_note: "1 evidence record (1 faithful); latest: cargo test -p canon-gate: 40 passed".to_string() });
    }

    #[test]
    fn approves_with_a_default_note_when_no_evidence_note_companion_exists() {
        let task_id = TaskId::parse("s5-trust-spine-gate#3.2").unwrap();
        let record = evidence_record(&task_id, EvidenceVerdict::Faithful);

        let decision = gate_task(&task_id, &[record], &[]);

        match decision {
            TaskFlipDecision::Approved { evidence_note } => {
                assert!(evidence_note.starts_with("1 evidence record (1 faithful); latest: Faithful evidence recorded"), "{evidence_note}");
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

    /// 0.14 D5 (dogfood F8): the row names every record bound to the task
    /// and the latest summary, not one record's text.
    #[test]
    fn the_approved_note_aggregates_every_record_and_carries_the_latest_summary() {
        let task_id = TaskId::parse("s5-trust-spine-gate#3.2").unwrap();
        let records = vec![
            evidence_record(&task_id, EvidenceVerdict::Faithful),
            evidence_record(&task_id, EvidenceVerdict::NotApplicable),
            evidence_record(&task_id, EvidenceVerdict::Faithful),
        ];
        let notes = vec![EvidenceNote::new(task_id.clone(), "1 vitest case", None), EvidenceNote::new(task_id.clone(), "9 vitest cases, 2 smoke checks", None)];
        assert_eq!(
            gate_task(&task_id, &records, &notes),
            TaskFlipDecision::Approved { evidence_note: "3 evidence records (2 faithful, 1 not-applicable); latest: 9 vitest cases, 2 smoke checks".to_string() }
        );
    }

    #[test]
    fn a_fabricated_note_on_any_record_blocks_the_aggregated_flip() {
        let task_id = TaskId::parse("s5-trust-spine-gate#3.2").unwrap();
        let record = evidence_record(&task_id, EvidenceVerdict::Faithful);
        let notes = vec![EvidenceNote::new(task_id.clone(), "TBD", None), EvidenceNote::new(task_id.clone(), "cargo test: ok", None)];
        assert_eq!(blocked_classes(&gate_task(&task_id, &[record], &notes)), vec![FailureClass::FabricatedEvidence]);
    }
}
