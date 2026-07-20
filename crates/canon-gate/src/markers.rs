//! Fabrication-marker scanning (design decision 6, D6; spec
//! `gated-task-completion` "Fabrication-marker scanning") —
//! `scanFakeMarkers`'s SHAPE (structured evidence fields only, a bare
//! `verified` claim with no attached command result still fails)
//! re-implemented against `canon-model`'s evidence schema, never
//! imported from the donor CLI's gate-markers module (design.md decision 6's own
//! text). [`scan_fake_markers`] takes an [`EvidenceNote`], never a bare
//! `&str` of arbitrary prose — that signature is itself how this module
//! enforces "only structured evidence fields, never free conversational
//! prose" (spec.md "Free prose containing a blocklist word is not
//! scanned"): there is no code path that could hand it ambient chat
//! text even by accident.
//!
//! # INTERFACE REQUEST to canon-model (S1) — not implemented here
//! `canon_model::EvidenceRecord` (verified against
//! `crates/canon-model/src/records.rs`, 2026-07-11) carries only
//! `task_id`/`scenario_id`/`run_id`/`verdict` — no free-text field a
//! fabrication scanner could read at all. [`EvidenceNote`] is
//! canon-gate's own companion type, joined by `task_id`, mirroring
//! [`crate::trust_ladder::TrustLadderState`]'s already-established
//! precedent (that module's own doc comment) for "the exact shape a
//! future `EvidenceRecord` structured-evidence field would carry,
//! joined by whatever join-spine key the artifact already has, since
//! this crate's territory excludes editing `canon-model` directly."
//! `summary` is the one-line evidence-note text a `- [x] ` row's ` — ✅
//! <evidence>` suffix is built from (`crate::checkbox::gate_task`);
//! `command_result` is the "attached captured command result" spec.md's
//! bare-`verified` scenario names — its mere PRESENCE (not its content)
//! is what turns a bare `verified` claim from fabricated into
//! substantiated.

use canon_model::TaskId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{FailureClass, Violation};

/// The closed fabrication-marker blocklist — spec.md's own three named
/// examples ("A fabricated evidence field blocks the flip", quoting `"would
/// pass"`, `"TBD"`, `"n/a"`), matched case-insensitively as a substring
/// of a structured evidence field's text. A hit is a violation
/// regardless of surrounding text — these are markers of an evidence
/// claim that was never actually checked, not words that happen to
/// co-occur with a legitimate one.
pub const FABRICATION_BLOCKLIST: [&str; 3] = [
    "would pass", // an unexecuted, hypothetical claim standing in for a real result
    "tbd",        // a placeholder left where a real evidence value belongs
    "n/a",        // a dismissal masquerading as a completed check
];

/// Canon-gate's own companion type for the structured evidence text a
/// `- [x] ` flip's evidence note is built from and scanned against
/// (module doc's INTERFACE REQUEST) — joined to a task by `task_id`,
/// never embedded in [`canon_model::EvidenceRecord`] itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceNote {
    pub task_id: TaskId,
    /// The one-line evidence-note text (what a `- [x] ` row's ` — ✅
    /// <evidence>` suffix is built from). Scanned for fabrication
    /// markers.
    pub summary: String,
    /// A captured command result (stdout/exit-code/test-count — this
    /// crate does not constrain its shape), when one was actually run.
    /// `None` here, paired with `summary` being a bare `"verified"`
    /// claim, is exactly the fabrication pattern spec.md's "A bare
    /// verified token with no command result blocks the flip" scenario
    /// names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_result: Option<String>,
}

impl EvidenceNote {
    pub fn new(task_id: TaskId, summary: impl Into<String>, command_result: Option<String>) -> Self {
        Self { task_id, summary: summary.into(), command_result }
    }
}

/// Read the optional `evidence_note` companion off one record's RAW
/// JSON body, joined by `task_id` (module doc's INTERFACE REQUEST) —
/// the same "independently re-read the raw ledger JSON for a companion
/// key `EvidenceRecord`'s own strict `Deserialize` drops" move
/// `crate::trust::trust_ladder_tag_of`/`crate::staleness::surface_hint_of`
/// already use for `trust_ladder`/`evidence_sha`+`surface_ref` — this is
/// `canon-cli`'s `canon gate task` wiring's ONE way to recover a
/// structured evidence note from a real, on-disk ledger record (S5
/// wave-2-part2's CLI territory; the pure `gate_task`/`scan_fake_markers`
/// functions themselves stay `EvidenceNote`-typed and know nothing about
/// this JSON shape). `None` for legitimately absent (a record with no
/// `evidence_note` key at all — `gate_task`'s own `default_evidence_text`
/// fallback covers that case); `Some(Err(_))` for present-but-unparseable,
/// never silently folded into "absent" (the identical bypass-avoidance
/// discipline `trust_ladder_tag_of`'s own doc names).
pub fn evidence_note_of(raw: &serde_json::Value, task_id: &TaskId) -> Option<Result<EvidenceNote, serde_json::Error>> {
    #[derive(Deserialize)]
    struct RawEvidenceNote {
        summary: String,
        #[serde(default)]
        command_result: Option<String>,
    }

    let value = raw.get("evidence_note")?;
    Some(serde_json::from_value::<RawEvidenceNote>(value.clone()).map(|r| EvidenceNote::new(task_id.clone(), r.summary, r.command_result)))
}

/// `scanFakeMarkers`'s two detection rules (module doc), run ONLY over
/// `note`'s own structured fields (`summary`, and `command_result` when
/// present) — never any other text a caller might hold. Returns every
/// finding as a [`FailureClass::FabricatedEvidence`] [`Violation`];
/// empty means clean.
pub fn scan_fake_markers(note: &EvidenceNote) -> Vec<Violation> {
    let mut violations = Vec::new();

    let mut fields: Vec<(&'static str, &str)> = vec![("summary", note.summary.as_str())];
    if let Some(command_result) = &note.command_result {
        fields.push(("command_result", command_result.as_str()));
    }

    for (field_name, text) in &fields {
        let lower = text.to_lowercase();
        for marker in FABRICATION_BLOCKLIST {
            if lower.contains(marker) {
                violations.push(Violation::new(
                    FailureClass::FabricatedEvidence,
                    note.task_id.to_string(),
                    format!("{field_name} field contains fabrication marker {marker:?}: {text:?}"),
                ));
            }
        }
    }

    if note.summary.trim().eq_ignore_ascii_case("verified") && note.command_result.is_none() {
        violations.push(Violation::new(
            FailureClass::FabricatedEvidence,
            note.task_id.to_string(),
            "summary is a bare 'verified' claim with no attached command result".to_string(),
        ));
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_id() -> TaskId {
        TaskId::parse("s5-trust-spine-gate#3.3").unwrap()
    }

    #[test]
    fn a_clean_note_with_a_real_command_result_produces_no_violations() {
        let note = EvidenceNote::new(task_id(), "cargo test -p canon-gate: 40 passed, 0 failed", Some("test result: ok. 40 passed; 0 failed".to_string()));
        assert!(scan_fake_markers(&note).is_empty());
    }

    #[test]
    fn a_would_pass_claim_is_flagged() {
        let note = EvidenceNote::new(task_id(), "this would pass once the CLI is wired", None);
        let violations = scan_fake_markers(&note);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::FabricatedEvidence);
    }

    #[test]
    fn a_tbd_placeholder_is_flagged_case_insensitively() {
        let note = EvidenceNote::new(task_id(), "Tbd — will fill in after review", None);
        assert_eq!(scan_fake_markers(&note).len(), 1);
    }

    #[test]
    fn an_n_a_dismissal_is_flagged() {
        let note = EvidenceNote::new(task_id(), "n/a, this task has no test surface", None);
        assert_eq!(scan_fake_markers(&note).len(), 1);
    }

    #[test]
    fn a_bare_verified_with_no_command_result_is_flagged() {
        let note = EvidenceNote::new(task_id(), "verified", None);
        let violations = scan_fake_markers(&note);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::FabricatedEvidence);
    }

    #[test]
    fn a_verified_claim_with_an_attached_command_result_is_not_flagged() {
        let note = EvidenceNote::new(task_id(), "verified", Some("exit code 0".to_string()));
        assert!(scan_fake_markers(&note).is_empty());
    }

    #[test]
    fn a_blocklist_hit_inside_the_command_result_field_is_also_scanned() {
        let note = EvidenceNote::new(task_id(), "ran the suite", Some("TBD: forgot to capture the real output".to_string()));
        assert_eq!(scan_fake_markers(&note).len(), 1);
    }

    #[test]
    fn free_prose_outside_the_structured_fields_is_never_scanned() {
        // spec.md "Free prose containing a blocklist word is not scanned":
        // scan_fake_markers's signature only ever sees `note`'s own
        // fields — an out-of-band string an agent's chat reply might
        // contain is structurally unreachable here. A clean note stays
        // clean no matter what free text exists ELSEWHERE in the caller's
        // process; this test documents that guarantee rather than
        // exercising a code path that doesn't exist.
        let ambient_chat_reply = "yeah this would pass, TBD on the edge cases, n/a for now";
        let note = EvidenceNote::new(task_id(), "cargo test -p canon-gate: 40 passed", Some("test result: ok".to_string()));
        assert!(!ambient_chat_reply.is_empty(), "the ambient text exists but is never passed to the scanner");
        assert!(scan_fake_markers(&note).is_empty());
    }
}
