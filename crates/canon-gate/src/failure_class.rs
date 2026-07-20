//! Stable failure-class strings (design decision 9, D9): the SAME
//! eight strings every wave-2 check (coverage/D3a, verdict-ledger/D3b,
//! staleness, trust-ladder promotion, the flag ratchet, and
//! `gated-task-completion`'s checkbox-flip guard) emits, grep-stable
//! like `tools/parity.py::FAILURE_CLASSES`
//! (the donor parity-harness audit's static-gate notes §3.1) —
//! never renamed without migrating every fixture + hook in the same
//! change (design decision 9's own text).
//!
//! [`FAILURE_CLASSES`] is the primary, wire-format artifact (a plain
//! `&'static str` list, matching parity.py's own choice of a string
//! tuple over an `Enum` "so shell/other-language consumers ... can
//! match by substring without a shared type" — static-gate.md §3.1);
//! [`FailureClass`] is the typed Rust enum this crate's own violation
//! sites construct from, GENERATING [`FAILURE_CLASSES`] so the two can
//! never silently drift (mirrors `canon_model::FailureClass`'s own
//! `as_str`/`from_str_exact` shape, `canon-model/src/evidence.rs` — a
//! DIFFERENT closed vocabulary for a different gate layer: S1's five
//! classes cover evidence-INTEGRITY malformed-input; this crate's
//! eight cover the trust-SPINE gate's own violations. `malformed` (S1)
//! and `malformed-evidence` (here) are deliberately distinct strings
//! for exactly that reason.).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The trust-spine gate's eight closed failure classes (design
/// decision 9). Renaming a variant's [`FailureClass::as_str`] value is
/// a coordinated migration (fixtures + hooks in the same change) —
/// never a silent rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FailureClass {
    /// A policy-derived required cell has no evidence record at all
    /// (D3a, `specs/trust-spine-gate/spec.md` "Required cell with no
    /// evidence fails coverage"). S5 wave-2's static coverage check
    /// (task 1.2) emits this.
    UncoveredCell,
    /// An artifact's `reviewed` lifecycle tag has no matching ledger
    /// review-record (D21, spec.md "reviewed without a review-record
    /// is a violation"). [`crate::trust_ladder::TrustRung::classify`]
    /// already produces the `UnreviewedPromotion` rung this class
    /// names; S5 wave-2's trust-ladder check (task 1.4/1.9) emits the
    /// violation.
    UnreviewedPromotion,
    /// An artifact's achieved trust level is below `policy.yaml`'s
    /// `trust_required` for its class, scoped to a release check only
    /// (D7, spec.md "Severity below the required trust level at
    /// release"). Never fires outside that release profile.
    TrustBelowRequired,
    /// A passing evidence record has degraded to stale — its surface
    /// changed since the record was produced, or HEAD is beyond the
    /// `max_commits_behind` ceiling (D3b/A3, spec.md "Staleness
    /// detection"). [`crate::policy::PolicyResolution::staleness`]'s
    /// schema is ready; S5 wave-2's staleness check (task 1.7) reads
    /// it.
    StaleEvidence,
    /// A candidate evidence record does not parse / is missing a
    /// required field (§7 "malformed evidence is no evidence") — this
    /// crate's trust-spine-scoped analog of
    /// `canon_model::FailureClass::Malformed`; deliberately a
    /// DIFFERENT wire string (module doc).
    MalformedEvidence,
    /// An artifact carries the human-only `flagged` overlay — never
    /// green regardless of any passing evidence (D21, spec.md "flagged
    /// overrides passing evidence").
    /// [`crate::trust_ladder::TrustRung::Flagged`] is the rung this
    /// class names.
    Flagged,
    /// `canon gate task <task_id>` flipped (or was asked to flip) a
    /// checkbox with no matching `EvidenceRecord`
    /// (`gated-task-completion` capability, design decision 6). S5
    /// wave-2's checkbox-grammar surface (task 3.2/3.4) emits this.
    UnevidencedFlip,
    /// Fabrication-marker scanning found a blocklisted substring or a
    /// bare `verified` claim with no attached structured result
    /// (`gated-task-completion` capability, design decision 6,
    /// `scanFakeMarkers` shape). S5 wave-2's fabrication scanner (task
    /// 3.3/3.4) emits this.
    FabricatedEvidence,
}

impl FailureClass {
    /// All eight classes, in [`FAILURE_CLASSES`]'s own order — the one
    /// iteration point [`FAILURE_CLASSES`] and
    /// [`FailureClass::from_str_exact`] both walk, so "eight classes"
    /// is asserted structurally, not by a comment that can drift.
    pub const ALL: [FailureClass; 8] = [
        FailureClass::UncoveredCell,
        FailureClass::UnreviewedPromotion,
        FailureClass::TrustBelowRequired,
        FailureClass::StaleEvidence,
        FailureClass::MalformedEvidence,
        FailureClass::Flagged,
        FailureClass::UnevidencedFlip,
        FailureClass::FabricatedEvidence,
    ];

    /// The stable, grep-able wire string. Matches
    /// `#[serde(rename_all = "kebab-case")]` exactly — asserted by a
    /// test below, so the two representations can never silently
    /// diverge.
    pub fn as_str(self) -> &'static str {
        match self {
            FailureClass::UncoveredCell => "uncovered-cell",
            FailureClass::UnreviewedPromotion => "unreviewed-promotion",
            FailureClass::TrustBelowRequired => "trust-below-required",
            FailureClass::StaleEvidence => "stale-evidence",
            FailureClass::MalformedEvidence => "malformed-evidence",
            FailureClass::Flagged => "flagged",
            FailureClass::UnevidencedFlip => "unevidenced-flip",
            FailureClass::FabricatedEvidence => "fabricated-evidence",
        }
    }

    /// Parse a failure-class wire string back to its variant — used by
    /// the fixture corpus's `expected_failures.txt` reader (S5
    /// wave-2, task 5.2), never by production violation-raising code.
    pub fn from_str_exact(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

/// The grep-stable wire-format string list (module doc) — generated
/// FROM [`FailureClass::ALL`] so the two representations can never
/// silently diverge (asserted by a test below). This, not the typed
/// enum, is what a shell hook or a fixture's `expected_failures.txt`
/// greps against (static-gate.md §3.1's rationale for a plain string
/// list over an `Enum`).
pub const FAILURE_CLASSES: [&str; 8] = [
    "uncovered-cell",
    "unreviewed-promotion",
    "trust-below-required",
    "stale-evidence",
    "malformed-evidence",
    "flagged",
    "unevidenced-flip",
    "fabricated-evidence",
];

/// One gate violation — mirrors `tools/parity.py::Violation(cls,
/// subject, detail)` + its `.line()` wire-format renderer exactly
/// (static-gate.md §3.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    pub class: FailureClass,
    pub subject: String,
    pub detail: String,
}

impl Violation {
    pub fn new(class: FailureClass, subject: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { class, subject: subject.into(), detail: detail.into() }
    }

    /// The grep-stable wire line: `"{class} {subject} — {detail}"`
    /// (parity.py `Violation.line()`, static-gate.md §3.1) — the same
    /// format a fixture's `expected_failures.txt` (S5 wave-2, task
    /// 5.1) is meant to diff against.
    pub fn line(&self) -> String {
        format!("{} {} — {}", self.class.as_str(), self.subject, self.detail)
    }

    /// The `(class, subject)` pair `cmd_selftest`'s exact-set-match
    /// oracle diffs on (fixtures-selftest audit pattern 3.2) — S5
    /// wave-2's `canon gate selftest` (task 5.2) reduces every actual
    /// violation to this shape before comparing against `EXPECTED`.
    pub fn pair(&self) -> (FailureClass, String) {
        (self.class, self.subject.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `FAILURE_CLASSES` stability test (acceptance criterion): the
    /// eight wire strings are exactly [`FailureClass::ALL`]'s
    /// `as_str()` output, in the same order, and every string
    /// round-trips through [`FailureClass::from_str_exact`] — the two
    /// representations (grep-stable const, typed enum) can never
    /// silently diverge.
    #[test]
    fn failure_classes_const_matches_enum_exactly() {
        let from_enum: Vec<&str> = FailureClass::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(from_enum, FAILURE_CLASSES.to_vec());
        for &s in FAILURE_CLASSES.iter() {
            assert_eq!(FailureClass::from_str_exact(s).map(FailureClass::as_str), Some(s));
        }
        assert_eq!(FailureClass::from_str_exact("not-a-real-class"), None);
    }

    /// Every variant's serde wire form (kebab-case) matches
    /// `as_str()` byte-for-byte — the invariant `FailureClass`'s doc
    /// comment claims.
    #[test]
    fn serde_wire_form_matches_as_str() {
        for class in FailureClass::ALL {
            let json = serde_json::to_string(&class).expect("serialize");
            assert_eq!(json, format!("\"{}\"", class.as_str()));
        }
    }

    #[test]
    fn violation_line_matches_parity_wire_format() {
        let v = Violation::new(FailureClass::UncoveredCell, "world.place-lock.02", "no evidence record for this cell");
        assert_eq!(v.line(), "uncovered-cell world.place-lock.02 — no evidence record for this cell");
        assert_eq!(v.pair(), (FailureClass::UncoveredCell, "world.place-lock.02".to_string()));
    }

    #[test]
    fn all_has_exactly_eight_classes_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for class in FailureClass::ALL {
            assert!(seen.insert(class.as_str()), "duplicate failure class: {}", class.as_str());
        }
        assert_eq!(seen.len(), 8);
    }
}
