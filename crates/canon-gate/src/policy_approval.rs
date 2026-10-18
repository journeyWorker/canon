//! `unapproved-policy`: the policy the gate grades against is the one a
//! human approved (0.22 D1, #71).
//!
//! `canon policy approve` (0.15 D5) records who approved `policy.yaml`
//! and the digest of the bytes they approved, in
//! `.canon/policy.approval.json`. Until 0.22 a mismatch was an advisory,
//! so an agent could delete `require_review` and ship with no review
//! while the gate stayed green. Now a policy whose bytes changed after
//! the approval ([`ApprovalState::Changed`]), or an approval file that
//! does not read as one ([`ApprovalState::Unreadable`]), is a violation:
//! `canon gate check` fails on it and `canon subject status` refuses
//! every transition that runs the gate's guards. A policy never approved
//! ([`ApprovalState::Unapproved`]) stays an advisory: there is no
//! approved digest to be stale against, and every repo initialised
//! before 0.15 is in that state.
//!
//! The approval state lives here, not in `canon-cli`, so the gate's own
//! selftest corpus can prove the class fires (one fixture per class).

use std::fs;
use std::path::Path;

use canon_model::paths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::context::{GateCheck, GateContext};
use crate::failure_class::{FailureClass, Violation};

/// The approval record's wire shape. Unknown keys are refused on read,
/// like every other authored canon file, so a hand edit that adds a key
/// reads as unreadable rather than as silently approved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyApproval {
    pub policy_digest: String,
    pub approved_by: String,
    pub approved_at: String,
    pub note: Option<String>,
}

/// What the approval file says about the policy in force.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalState {
    /// No `.canon/policy.yaml` (or it cannot be read): nothing to approve.
    NoPolicy,
    /// The approved digest matches the policy's current bytes.
    Approved(PolicyApproval),
    /// A policy exists and no approval file does.
    Unapproved,
    /// The policy's bytes changed after the recorded approval.
    Changed { approval: PolicyApproval, current_digest: String },
    /// The approval file exists but is not a readable approval record.
    Unreadable { detail: String },
}

impl ApprovalState {
    /// The wire name `canon policy show` reports (0.22 D2).
    pub fn as_str(&self) -> &'static str {
        match self {
            ApprovalState::NoPolicy => "none",
            ApprovalState::Approved(_) => "approved",
            ApprovalState::Unapproved => "unapproved",
            ApprovalState::Changed { .. } => "changed",
            ApprovalState::Unreadable { .. } => "unreadable",
        }
    }
}

/// `sha256:<hex>` over `bytes`, the digest spelling canon uses elsewhere
/// (`context_pack`, `dispatch`).
pub fn policy_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Read the policy and its approval under an already-resolved repo root.
pub fn approval_state(repo: &Path) -> ApprovalState {
    let Ok(policy) = fs::read(repo.join(paths::POLICY_FILE)) else {
        return ApprovalState::NoPolicy;
    };
    let text = match fs::read_to_string(repo.join(paths::POLICY_APPROVAL_FILE)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return ApprovalState::Unapproved,
        Err(e) => return ApprovalState::Unreadable { detail: e.to_string() },
    };
    let approval: PolicyApproval = match serde_json::from_str(&text) {
        Ok(approval) => approval,
        Err(e) => return ApprovalState::Unreadable { detail: e.to_string() },
    };
    let current_digest = policy_digest(&policy);
    if approval.policy_digest == current_digest {
        ApprovalState::Approved(approval)
    } else {
        ApprovalState::Changed { approval, current_digest }
    }
}

/// How a human lifts the violation; every refusal of it ends with this.
pub const FIX: &str = "a human reviews the policy and approves it with `canon policy approve --by <who>` (or `canon brief approve`, which records both); editing the policy never makes the gate pass";

/// The `unapproved-policy` violation for `state`: `Some` for
/// [`ApprovalState::Changed`] and [`ApprovalState::Unreadable`] only.
pub fn violation(state: &ApprovalState) -> Option<Violation> {
    match state {
        ApprovalState::NoPolicy | ApprovalState::Approved(_) | ApprovalState::Unapproved => None,
        ApprovalState::Changed { approval, current_digest } => Some(Violation::new(
            FailureClass::UnapprovedPolicy,
            paths::POLICY_FILE,
            format!(
                "changed since its approval: the file is {current_digest}, but `{}` approved {} at {}; {FIX}",
                approval.approved_by, approval.policy_digest, approval.approved_at
            ),
        )),
        ApprovalState::Unreadable { detail } => Some(Violation::new(
            FailureClass::UnapprovedPolicy,
            paths::POLICY_APPROVAL_FILE,
            format!("not a readable policy approval ({detail}), so nothing says {} is the policy a human approved; {FIX}", paths::POLICY_FILE),
        )),
    }
}

/// The gate's check over the repo [`GateContext`] was loaded from.
pub struct PolicyApprovalCheck;

impl GateCheck for PolicyApprovalCheck {
    fn name(&self) -> &'static str {
        "policy-approval"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        violation(&approval_state(&ctx.ctx.repo)).into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_with_policy(policy: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(paths::CANON_DIR)).unwrap();
        fs::write(dir.path().join(paths::POLICY_FILE), policy).unwrap();
        dir
    }

    fn approve(dir: &Path, digest: &str) {
        let body = serde_json::json!({ "policy_digest": digest, "approved_by": "alice", "approved_at": "2026-10-16T00:00:00Z", "note": null });
        fs::write(dir.join(paths::POLICY_APPROVAL_FILE), body.to_string()).unwrap();
    }

    #[test]
    fn only_changed_and_unreadable_are_violations() {
        let dir = repo_with_policy("spec_coverage: {}\n");
        assert_eq!(approval_state(dir.path()), ApprovalState::Unapproved);
        assert_eq!(violation(&approval_state(dir.path())), None);

        approve(dir.path(), &policy_digest(b"spec_coverage: {}\n"));
        assert!(matches!(approval_state(dir.path()), ApprovalState::Approved(_)));
        assert_eq!(violation(&approval_state(dir.path())), None);

        fs::write(dir.path().join(paths::POLICY_FILE), "spec_coverage: { require_evidence: true }\n").unwrap();
        let changed = violation(&approval_state(dir.path())).expect("a changed policy is a violation");
        assert_eq!((changed.class, changed.subject.as_str()), (FailureClass::UnapprovedPolicy, paths::POLICY_FILE));
        assert!(changed.detail.contains("`alice` approved sha256:") && changed.detail.contains("canon policy approve --by <who>"), "{}", changed.detail);

        fs::write(dir.path().join(paths::POLICY_APPROVAL_FILE), "{").unwrap();
        let unreadable = violation(&approval_state(dir.path())).expect("an unreadable approval is a violation");
        assert_eq!((unreadable.class, unreadable.subject.as_str()), (FailureClass::UnapprovedPolicy, paths::POLICY_APPROVAL_FILE));
    }

    #[test]
    fn an_unknown_key_reads_as_unreadable() {
        let dir = repo_with_policy("x: 1\n");
        fs::write(
            dir.path().join(paths::POLICY_APPROVAL_FILE),
            r#"{"policy_digest":"sha256:00","approved_by":"a","approved_at":"t","note":null,"extra":1}"#,
        )
        .unwrap();
        assert!(matches!(approval_state(dir.path()), ApprovalState::Unreadable { .. }));
    }

    #[test]
    fn no_policy_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(approval_state(dir.path()), ApprovalState::NoPolicy);
        assert_eq!(violation(&ApprovalState::NoPolicy), None);
    }
}
