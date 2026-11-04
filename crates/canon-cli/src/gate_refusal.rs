//! The codes of a gate's refusals (`canon-0-36-codes-exit`, atip/1 spec 6.4, 7.2.2, 8.3) and their reasons (`canon-0-36-code-reasons`, spec 7.6):
//! a gate that refuses a record or a transition answers exit 2 with the code of the condition that failed, and no `data`. Several causes are
//! `error.details[]`, one entry per cause with its own code, the first cause also being the error itself.
//!
//! The condition is the violation's class: a missing approval, review or evidence is `<TARGET>_APPROVAL_REQUIRED`, evidence that
//! went out of date is `EVIDENCE_STALE`, a ledger in an old layout is `LAYOUT_MIGRATION_REQUIRED`, and any other rule is `<TARGET>_INVALID`.
//! Only `subject move` raises the classes; promotion raises only `malformed-evidence`.
//!
//! A class is a reason only where the next action differs inside one code (spec 7.6, the contract author's rule) and a transition raises it:
//! `SUBJECT_INVALID` has the reasons `MALFORMED_EVIDENCE` (a record the gate fails closed on: fix or remove it) and `OPEN_BLOCKER` (close the
//! blocker finding), the two classes `subject move` raises under it. The other four classes that map to it (`trust-below-required`, `flagged`,
//! `fabricated-evidence`, `false-success`) are reported by `gate check` and `gate task` and raised by no transition, so they have no reason
//! until one does. The five classes that name their own code (`SUBJECT_APPROVAL_REQUIRED`, `EVIDENCE_STALE`, `LAYOUT_MIGRATION_REQUIRED`)
//! carry none, and promotion has one situation and no reason.

use canon_gate::FailureClass;

use crate::cli_error::{CliError, CliErrorExt as _};

/// The code of a refusal of a status transition (`subject move`) for a gate violation of `class`.
pub fn subject_code(class: FailureClass) -> &'static str {
    match class {
        FailureClass::UnreviewedPromotion | FailureClass::UncoveredCell | FailureClass::UnevidencedFlip => "SUBJECT_APPROVAL_REQUIRED",
        FailureClass::StaleEvidence => "EVIDENCE_STALE",
        FailureClass::OldLayout => "LAYOUT_MIGRATION_REQUIRED",
        FailureClass::TrustBelowRequired | FailureClass::MalformedEvidence | FailureClass::Flagged | FailureClass::FabricatedEvidence | FailureClass::OpenBlocker | FailureClass::FalseSuccess => "SUBJECT_INVALID",
    }
}

/// The reason of a refusal of a status transition for a violation of `class`: the two classes a transition raises under `SUBJECT_INVALID` are
/// told apart by what to fix; a class that has its own code, or that no transition raises, has none (module doc).
pub fn subject_reason(class: FailureClass) -> Option<&'static str> {
    match class {
        FailureClass::MalformedEvidence => Some("MALFORMED_EVIDENCE"),
        FailureClass::OpenBlocker => Some("OPEN_BLOCKER"),
        FailureClass::TrustBelowRequired | FailureClass::Flagged | FailureClass::FabricatedEvidence | FailureClass::FalseSuccess => None,
        FailureClass::UnreviewedPromotion | FailureClass::UncoveredCell | FailureClass::UnevidencedFlip | FailureClass::StaleEvidence | FailureClass::OldLayout => None,
    }
}

/// The code of a refusal of a promotion (`gate promote`, a record promoted as it is added): `gate promote` meets only records the staging read
/// refuses (`malformed-evidence`), so it has the one code and no `*_APPROVAL_REQUIRED` or `EVIDENCE_STALE` of its own.
pub(crate) const PROMOTION_CODE: &str = "PROMOTION_INVALID";

/// One cause of a refusal: its code, its reason when the code has several situations, and its message.
pub(crate) struct Cause {
    pub code: &'static str,
    pub reason: Option<&'static str>,
    pub message: String,
}

impl Cause {
    /// A cause whose code has one situation and so no reason.
    pub(crate) fn new(code: &'static str, message: String) -> Self {
        Self { code, reason: None, message }
    }

    /// The cause of a gate violation of `class` that blocks a status transition: [`subject_code`] and [`subject_reason`].
    pub(crate) fn of_class(class: FailureClass, message: String) -> Self {
        Self { code: subject_code(class), reason: subject_reason(class), message }
    }
}

/// The refusal for `causes`, the first being the error and, with two or more, every one an entry of `details`.
///
/// # Panics
/// When `causes` is empty: a refusal has a cause.
pub(crate) fn refusal(causes: Vec<Cause>, hint: &str) -> CliError {
    let mut errors: Vec<CliError> = causes
        .into_iter()
        .map(|cause| {
            let error = CliError::refused(cause.code, cause.message).hint(hint);
            match cause.reason {
                Some(reason) => error.reason(reason),
                None => error,
            }
        })
        .collect();
    assert!(!errors.is_empty(), "a refusal has a cause");
    let first = errors.remove(0);
    let all: Vec<CliError> = std::iter::once(first.clone()).chain(errors).collect();
    first.details(all)
}
