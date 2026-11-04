//! The codes of a gate's refusals (`canon-0-36-codes-exit`, atip/1 spec 6.4, 7.2.2, 8.3): a gate that refuses a record or a
//! transition answers exit 2 with the code of the condition that failed, and no `data`. Several causes are `error.details[]`, one
//! entry per cause with its own code, the first cause also being the error itself.
//!
//! The condition is the violation's class: a missing approval, review or evidence is `<TARGET>_APPROVAL_REQUIRED`, evidence that
//! went out of date is `EVIDENCE_STALE`, a ledger in an old layout is `LAYOUT_MIGRATION_REQUIRED`, and any other rule is `<TARGET>_INVALID`.
//! A target maps the classes its gate can raise: promotion raises only `malformed-evidence`, so it has no `_APPROVAL_REQUIRED` code.

use canon_gate::FailureClass;

use crate::cli_error::{CliError, CliErrorExt as _};

/// The code of a refusal of a status transition (`subject move`) for a gate violation of `class`.
pub(crate) fn subject_code(class: FailureClass) -> &'static str {
    match class {
        FailureClass::UnreviewedPromotion | FailureClass::UncoveredCell | FailureClass::UnevidencedFlip => "SUBJECT_APPROVAL_REQUIRED",
        FailureClass::StaleEvidence => "EVIDENCE_STALE",
        FailureClass::OldLayout => "LAYOUT_MIGRATION_REQUIRED",
        FailureClass::TrustBelowRequired | FailureClass::MalformedEvidence | FailureClass::Flagged | FailureClass::FabricatedEvidence | FailureClass::OpenBlocker | FailureClass::FalseSuccess => "SUBJECT_INVALID",
    }
}

/// The code of a refusal of a promotion (`gate promote`, a record promoted as it is added) for a gate violation of `class`.
pub(crate) fn promotion_code(class: FailureClass) -> &'static str {
    match class {
        FailureClass::StaleEvidence => "EVIDENCE_STALE",
        FailureClass::OldLayout => "LAYOUT_MIGRATION_REQUIRED",
        _ => "PROMOTION_INVALID",
    }
}

/// One cause of a refusal: its code and its message.
pub(crate) struct Cause {
    pub code: &'static str,
    pub message: String,
}

/// The refusal for `causes`, the first being the error and, with two or more, every one an entry of `details`.
///
/// # Panics
/// When `causes` is empty: a refusal has a cause.
pub(crate) fn refusal(causes: Vec<Cause>, hint: &str) -> CliError {
    let mut errors: Vec<CliError> = causes.into_iter().map(|cause| CliError::refused(cause.code, cause.message).hint(hint)).collect();
    assert!(!errors.is_empty(), "a refusal has a cause");
    let first = errors.remove(0);
    let all: Vec<CliError> = std::iter::once(first.clone()).chain(errors).collect();
    first.details(all)
}
