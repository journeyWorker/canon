//! `canon policy approve --by <who> [--note <text>] [--repo <dir>]`
//! (0.15 D5): who approved the policy, and which bytes they approved.
//!
//! `.canon/policy.yaml` is what the gate grades the work against, and the
//! skill makes approving it a human stop point. Until 0.15 that approval
//! was recorded nowhere, so nobody could say who approved the policy in
//! force, or whether it changed after they did (dogfood eval 2, G12).
//!
//! # The record
//! `.canon/policy.approval.json`, pretty JSON, git-tracked:
//! `policy_digest` (`sha256:<hex>` over the policy file's exact bytes),
//! `approved_by`, `approved_at` (RFC3339 UTC) and `note` (`null` when
//! none). It is a file in the working tree rather than a ledger record:
//! one approval is in force at a time, git keeps its history, and the
//! command writes it directly, with nothing to promote.
//!
//! # What reads it
//! `canon gate check` and `canon status` print one advisory line when a
//! policy exists but has no approval, or when its bytes no longer match
//! the approved digest, or when the approval file cannot be read. The
//! advisory never changes an exit code: canon cannot tell a human from an
//! agent running this command, so the record is attribution, not proof,
//! and a gate that failed on it would only teach agents to run it.

use std::fs;
use std::path::Path;

use canon_model::paths;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cli_error::CliError;
use crate::context::resolve_repo_root;

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

/// The one advisory line `canon gate check` and `canon status` print, or
/// `None` when there is nothing to say (no policy, or an approval that
/// still matches). Each line names the command that resolves it.
pub fn advisory(repo: &Path) -> Option<String> {
    const FIX: &str = "a human reviews it and runs `canon policy approve --by <who>`";
    match approval_state(repo) {
        ApprovalState::NoPolicy | ApprovalState::Approved(_) => None,
        ApprovalState::Unapproved => Some(format!("policy not approved: {} has no approval record ({}); {FIX}", paths::POLICY_FILE, paths::POLICY_APPROVAL_FILE)),
        ApprovalState::Changed { approval, current_digest } => Some(format!(
            "policy changed since approval: {} is {current_digest}, but `{}` approved {} at {}; {FIX}",
            paths::POLICY_FILE,
            approval.approved_by,
            approval.policy_digest,
            approval.approved_at
        )),
        ApprovalState::Unreadable { detail } => Some(format!("policy approval unreadable: {} ({detail}); {FIX}", paths::POLICY_APPROVAL_FILE)),
    }
}

/// Why an approval fails closed (module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    /// The policy's bytes differ from the approved digest.
    Changed,
    /// The approval file cannot be read.
    Unreadable,
}

impl Closed {
    /// The words the gate's violation line starts with.
    pub fn label(self) -> &'static str {
        match self {
            Closed::Changed => "policy changed since approval",
            Closed::Unreadable => "policy approval unreadable",
        }
    }
}

/// The fail-closed reading of the approval (module doc): why, and a
/// one-line message, for a policy that changed since its approval or whose
/// approval cannot be read; `None` for no policy, an approved policy, and a
/// never-approved one (advisory only).
pub fn closed_failure(repo: &Path) -> Option<(Closed, String)> {
    match approval_state(repo) {
        ApprovalState::Changed { approval, current_digest } => Some((
            Closed::Changed,
            format!(
                "{} is {current_digest}, but `{}` approved {} at {}; the gate grades the work against the policy in force, so a policy that is not the approved one fails closed (--override-reason does not waive it)",
                paths::POLICY_FILE,
                approval.approved_by,
                approval.policy_digest,
                approval.approved_at
            ),
        )),
        ApprovalState::Unreadable { detail } => Some((
            Closed::Unreadable,
            format!("{} cannot be read ({detail}), so the policy in force cannot be checked against its approval; the gate fails closed (--override-reason does not waive it)", paths::POLICY_APPROVAL_FILE),
        )),
        ApprovalState::NoPolicy | ApprovalState::Approved(_) | ApprovalState::Unapproved => None,
    }
}

/// What resolves [`closed_failure`]: the human re-approves, or the approved
/// bytes come back.
pub const REAPPROVE_HINT: &str = "the human re-approves the policy in force with `canon policy approve --by <human> --note \"<words>\"`, or restore the approved .canon/policy.yaml (and .canon/policy.approval.json) from git";

/// [`closed_failure`] as the violation `canon gate check` reports.
pub fn violation(repo: &Path) -> Option<canon_gate::Violation> {
    let (closed, detail) = closed_failure(repo)?;
    Some(canon_gate::Violation::new(canon_gate::FailureClass::UnreviewedPromotion, "policy", format!("{}: {detail}; {REAPPROVE_HINT}", closed.label())))
}

/// `canon policy show [--json]`: the read-back for `canon policy approve`.
/// Exits `0` whatever the state is: it reports, the gate decides.
pub fn run_show(repo: &Path, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let state = approval_state(&repo);
    let current = fs::read(repo.join(paths::POLICY_FILE)).ok().map(|bytes| policy_digest(&bytes));
    let (name, approval, detail) = match &state {
        ApprovalState::NoPolicy => ("none", None, None),
        ApprovalState::Approved(a) => ("approved", Some(a), None),
        ApprovalState::Unapproved => ("unapproved", None, None),
        ApprovalState::Changed { approval, .. } => ("changed", Some(approval), None),
        ApprovalState::Unreadable { detail } => ("unreadable", None, Some(detail.as_str())),
    };
    let data = serde_json::json!({
        "policy": paths::POLICY_FILE,
        "policyDigest": current,
        "approval": paths::POLICY_APPROVAL_FILE,
        "state": name,
        "approvedBy": approval.map(|a| &a.approved_by),
        "approvedAt": approval.map(|a| &a.approved_at),
        "approvedDigest": approval.map(|a| &a.policy_digest),
        "note": approval.and_then(|a| a.note.as_ref()),
        "detail": detail,
    });
    if json {
        crate::json_out::data("policy show", data);
        return 0;
    }
    println!("policy: {}", paths::POLICY_FILE);
    println!("digest: {}", current.as_deref().unwrap_or("-"));
    println!("approval: {name}{}", detail.map(|d| format!(" ({d})")).unwrap_or_default());
    if let Some(a) = approval {
        println!("approved by: {}", a.approved_by);
        println!("approved at: {}", a.approved_at);
        println!("approved digest: {}", a.policy_digest);
        if let Some(note) = &a.note {
            println!("note: {note}");
        }
    }
    if let Some((closed, _)) = closed_failure(&repo) {
        println!("gate: fails closed ({}); {REAPPROVE_HINT}", closed.label());
    }
    0
}

/// Writes the policy approval record for `by` under an already-resolved
/// repo root. `Err` carries the refusal text (no policy to approve, an
/// empty `by`, or a read/write failure). Shared by `canon policy approve`
/// and `canon brief approve`, which records both in one call.
pub fn record(repo: &Path, by: &str, note: Option<&str>) -> Result<PolicyApproval, String> {
    if by.trim().is_empty() {
        return Err("--by must name who approved the policy".to_string());
    }
    let policy_path = repo.join(paths::POLICY_FILE);
    let policy = match fs::read(&policy_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("{} does not exist, so there is no policy to approve; `canon init` writes a starter policy", policy_path.display()));
        }
        Err(e) => return Err(format!("cannot read {}: {e}", policy_path.display())),
    };
    let approval = PolicyApproval {
        policy_digest: policy_digest(&policy),
        approved_by: by.to_string(),
        approved_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        note: note.map(str::to_string),
    };
    let approval_path = repo.join(paths::POLICY_APPROVAL_FILE);
    let mut body = serde_json::to_string_pretty(&approval).expect("a PolicyApproval always serializes");
    body.push('\n');
    fs::write(&approval_path, body).map_err(|e| format!("cannot write {}: {e}", approval_path.display()))?;
    Ok(approval)
}

/// `canon policy approve`. Returns the process exit code: `0` written,
/// `2` refused (no policy to approve, an empty `--by`, or a write
/// failure).
pub fn run_approve(repo: &Path, by: &str, note: Option<&str>, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    match record(&repo, by, note) {
        Ok(approval) => {
            crate::say!(
                json,
                "canon policy approve: {} ({}) approved by `{}` — wrote {} — {}",
                paths::POLICY_FILE,
                approval.policy_digest,
                approval.approved_by,
                paths::POLICY_APPROVAL_FILE,
                crate::write_mode::DIRECT
            );
            let data = serde_json::json!({ "policy": paths::POLICY_FILE, "policyDigest": approval.policy_digest, "approvedBy": approval.approved_by, "approvedAt": approval.approved_at, "approval": paths::POLICY_APPROVAL_FILE });
            crate::next::finish(&repo, "policy approve", &crate::next::Scope::Repo, false, json, data);
            0
        }
        Err(e) => {
            CliError::refused("POLICY_APPROVAL_FAILED", e.to_string()).hint("with no policy, `canon init` writes a starter one; otherwise check that .canon/policy.yaml is readable and writable, then run `canon policy approve --by <name>` again").emit("canon policy approve", json)
        }
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

    #[test]
    fn approval_tracks_the_policy_bytes() {
        let dir = repo_with_policy("spec_coverage: {}\n");
        assert_eq!(approval_state(dir.path()), ApprovalState::Unapproved);
        assert_eq!(run_approve(dir.path(), "alice", Some("first look"), false), 0);
        assert!(matches!(&approval_state(dir.path()), ApprovalState::Approved(a) if a.approved_by == "alice" && a.note.as_deref() == Some("first look")));
        assert_eq!(advisory(dir.path()), None);

        fs::write(dir.path().join(paths::POLICY_FILE), "spec_coverage: { require_evidence: true }\n").unwrap();
        assert!(matches!(approval_state(dir.path()), ApprovalState::Changed { .. }));
        assert!(advisory(dir.path()).is_some_and(|line| line.starts_with("policy changed since approval")));
    }

    #[test]
    fn no_policy_is_refused_and_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(run_approve(dir.path(), "alice", None, false), 2);
        assert!(!dir.path().join(paths::POLICY_APPROVAL_FILE).exists());
        assert_eq!(advisory(dir.path()), None);
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
}
