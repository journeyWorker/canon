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
/// pass"`, `"TBD"`, `"n/a"`), matched ASCII-case-insensitively as a WHOLE
/// TOKEN of a structured evidence field's text ([`contains_marker`]). A
/// hit is a violation regardless of the surrounding sentence — these are
/// markers of an evidence claim that was never actually checked, not
/// words that happen to co-occur with a legitimate one. A marker inside a
/// longer word, a path segment, a file name or a URL is not a hit (0.18
/// D5, #32: `pack.json/atlas` holds the bytes `n/a` and is no dismissal).
pub const FABRICATION_BLOCKLIST: [&str; 3] = [
    "would pass", // an unexecuted, hypothetical claim standing in for a real result
    "tbd",        // a placeholder left where a real evidence value belongs
    "n/a",        // a dismissal masquerading as a completed check
];

/// Whether `marker` (lowercase ASCII) occurs in `text` as a whole token:
/// some occurrence that neither side continues into a longer token
/// ([`continues_token`]). Case-insensitive over ASCII only, so byte
/// offsets in the lowered text are the original's.
pub fn contains_marker(text: &str, marker: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    lower.match_indices(marker).any(|(start, hit)| {
        let end = start + hit.len();
        !continues_token(bytes[..start].iter().rev().copied()) && !continues_token(bytes[end..].iter().copied())
    })
}

/// Whether the bytes on one side of a match, read outward from it, join
/// it into a longer token: a word byte right next to it (an ASCII letter
/// or digit, `_`, or a path separator `/` `\`), or a `.` or `-` with a
/// word byte past it (`n/a.json`, `x-tbd`). A `.` or `-` with nothing
/// word-like past it is punctuation (`n/a.`, `TBD - later`). Non-ASCII
/// bytes are boundaries, so `TBD입니다` and `TBD — later` still hit.
fn continues_token(mut outward: impl Iterator<Item = u8>) -> bool {
    let is_word = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'/' | b'\\');
    match outward.next() {
        Some(b) if is_word(b) => true,
        Some(b'.' | b'-') => outward.next().is_some_and(is_word),
        _ => false,
    }
}

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
    scan_note_text(note.task_id.as_str(), &note.summary, note.command_result.as_deref())
}

/// [`scan_fake_markers`]'s rules over a note's two fields, reported
/// against `subject`. Split out (0.14 D5) so a SCENARIO-only record's
/// `--summary` is scanned by the identical rules: it never reaches a plan
/// row, but it lands in the same `evidence_note` companion a reader
/// trusts.
pub fn scan_note_text(subject: &str, summary: &str, command_result: Option<&str>) -> Vec<Violation> {
    let mut violations = Vec::new();

    let mut fields: Vec<(&'static str, &str)> = vec![("summary", summary)];
    if let Some(command_result) = command_result {
        fields.push(("command_result", command_result));
    }

    for (field_name, text) in &fields {
        for marker in FABRICATION_BLOCKLIST {
            if contains_marker(text, marker) {
                violations.push(Violation::new(
                    FailureClass::FabricatedEvidence,
                    subject,
                    format!("{field_name} field contains fabrication marker {marker:?}: {text:?}"),
                ));
            }
        }
    }

    if summary.trim().eq_ignore_ascii_case("verified") && command_result.is_none() {
        violations.push(Violation::new(
            FailureClass::FabricatedEvidence,
            subject,
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

    /// gate.markers.01 (0.18 D5, #32): markers match as whole tokens only.
    /// Every row is `(summary, hits)`: the markers the scan must find, in
    /// blocklist order.
    #[test]
    fn markers_match_whole_tokens_never_inside_a_path_a_file_name_a_url_or_a_longer_word() {
        let table: &[(&str, &[&str])] = &[
            // true positives
            ("this would pass once the CLI is wired", &["would pass"]),
            ("Tbd — will fill in after review", &["tbd"]),
            ("n/a, this task has no test surface", &["n/a"]),
            ("TBD: forgot to capture the real output", &["tbd"]),
            ("TBD", &["tbd"]),
            ("N/A", &["n/a"]),
            ("result: n/a.", &["n/a"]),
            ("(tbd)", &["tbd"]),
            ("\"n/a\"", &["n/a"]),
            ("status tbd - later", &["tbd"]),
            ("it Would Pass", &["would pass"]),
            ("TBD입니다", &["tbd"]),
            ("smoke: TBD, n/a for the rest; it would pass", &["would pass", "tbd", "n/a"]),
            ("ran it; n/a\nfor the GPU path", &["n/a"]),
            // negatives: paths, file names, URLs, longer words
            ("loader reads pack.json/atlas JSON and names the bad file", &[]),
            ("cat assets/n/a/sprite.png", &[]),
            ("wrote n/a.json", &[]),
            ("see https://example.com/tbd for the plan", &[]),
            ("tbd_notes.md checked", &[]),
            ("x-tbd flag parsed", &[]),
            ("tbd-notes reviewed", &[]),
            ("C:\\tbd\\log.txt", &[]),
            ("the tbdx parser", &[]),
            ("1tbd", &[]),
            ("it would passthrough the header", &[]),
            ("cargo test -p canon-gate: 40 passed, 0 failed", &[]),
        ];
        for (summary, hits) in table {
            let found: Vec<&str> = FABRICATION_BLOCKLIST.into_iter().filter(|marker| contains_marker(summary, marker)).collect();
            assert_eq!(&found, hits, "summary {summary:?}");
            assert_eq!(scan_note_text("a.b.01", summary, Some("ok")).len(), hits.len(), "summary {summary:?}");
        }
    }

    /// gate.markers.02 (0.18 D5, #32): a path holding a marker's letters
    /// does not hide a standalone marker elsewhere in the same field. The
    /// note is still fabricated, and the flip it was meant to justify is
    /// blocked.
    #[test]
    fn a_standalone_marker_beside_a_path_holding_its_letters_is_still_a_finding_and_blocks_the_flip() {
        let note = EvidenceNote::new(task_id(), "loaded pack.json/atlas; GPU path: n/a", None);
        let violations = scan_fake_markers(&note);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].class, FailureClass::FabricatedEvidence);
        assert!(violations[0].detail.contains("fabrication marker \"n/a\""), "{}", violations[0].detail);

        let record = canon_model::EvidenceRecord::new(
            canon_model::Envelope::new(1, canon_model::RecordKind::EvidenceRecord, chrono::Utc::now(), canon_model::Actor::new("impl", canon_model::RoleId::parse("implementer").unwrap())),
            Some(task_id()),
            None,
            None,
            canon_model::EvidenceVerdict::Faithful,
        );
        let decision = crate::gate_task(&task_id(), &[record], &[note]);
        assert!(matches!(decision, crate::TaskFlipDecision::Blocked { ref violations } if violations.iter().all(|v| v.class == FailureClass::FabricatedEvidence)), "{decision:?}");
    }
}
