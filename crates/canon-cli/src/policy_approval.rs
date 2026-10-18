//! `canon policy approve --by <who> [--note <text>] [--repo <dir>]`
//! (0.15 D5): who approved the policy, and which bytes they approved;
//! `canon policy show [--repo <dir>]` (0.22 D2) reads it back.
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
//! command writes it directly, with nothing to promote. The state it
//! gives the policy is [`canon_gate::policy_approval::approval_state`].
//!
//! # What reads it
//! A policy whose bytes changed after the approval, or an approval file
//! that does not read, fails closed (0.22 D1, #71): `canon gate check`
//! reports `unapproved-policy` and exits 1, `canon subject status`
//! refuses every transition that runs the gate's guards
//! ([`refusal_hint`], not waivable), and `canon status` makes
//! the human's review its one step ([`crate::status`]). Before 0.22 it was
//! an advisory, and deleting `require_review` shipped work with no
//! review. A policy never approved stays the advisory [`advisory`]
//! prints: there is no approved digest to be stale against, and every
//! repo initialised before 0.15 is in that state. canon cannot tell a
//! human from an agent running `policy approve`, so the record is
//! attribution, not proof; what it stops is a silent edit.

use std::fs;
use std::path::Path;

use canon_gate::policy_approval::{approval_state, policy_digest, violation, ApprovalState, PolicyApproval};
use canon_model::paths;
use chrono::{SecondsFormat, Utc};

use crate::cli_error::CliError;
use crate::context::resolve_repo_root;
use crate::role::Role;
use crate::status::{NextStep, StepActor};

/// The one advisory line `canon gate check` and `canon status` print for
/// a policy that was never approved, or `None` (no policy, approved, or
/// a state [`canon_gate::policy_approval::violation`] fails on).
pub fn advisory(repo: &Path) -> Option<String> {
    matches!(approval_state(repo), ApprovalState::Unapproved).then(|| {
        format!(
            "policy not approved: {} has no approval record ({}); a human reviews it and runs `canon policy approve --by <who>`",
            paths::POLICY_FILE,
            paths::POLICY_APPROVAL_FILE
        )
    })
}

/// The hint on `canon subject status`'s refusal of a transition that runs
/// the gate's guards while the approved policy does not hold (0.22 D1:
/// `POLICY_CHANGED_SINCE_APPROVAL`, `POLICY_APPROVAL_UNREADABLE`).
/// `--override-reason` does not waive it: a waiver graded by a policy
/// nobody approved would be circular.
pub fn refusal_hint(override_given: bool) -> String {
    let waiver = if override_given { "; --override-reason does not waive it" } else { "" };
    format!("stop and report the policy to the human: `canon policy show` reads its state, and the human's approval lifts this{waiver}")
}

/// `canon status`'s one human step while the approved policy does not
/// hold (0.22 D1); no transition is suggested beside it.
pub fn review_step(repo: &Path) -> Option<NextStep> {
    let violation = violation(&approval_state(repo))?;
    Some(NextStep {
        actor: StepActor::Human,
        role: None,
        command: REVIEW_STEP.to_string(),
        why: format!("{}; `canon policy show` reads the state", violation.line()),
    })
}

/// [`review_step`]'s command, fixed so [`crate::next`] keeps it in every
/// scoped list: it blocks every subject.
pub const REVIEW_STEP: &str =
    "stop and report .canon/policy.yaml to the human: it does not match an approval, so move no subject until the human reviews it and approves it";

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
/// failure). Its first next step is the read-back, `canon policy show`.
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
            let read_back = NextStep {
                actor: StepActor::Agent,
                role: Some(Role::Orchestrator),
                command: "canon policy show".into(),
                why: "read back the approval in force: the digest it names, who approved it, and when".into(),
            };
            crate::next::finish_with_first(&repo, "policy approve", &crate::next::Scope::Repo, json, data, read_back);
            0
        }
        Err(e) => {
            CliError::refused("POLICY_APPROVAL_FAILED", e.to_string()).emit("canon policy approve", json)
        }
    }
}

/// `canon policy show` (0.22 D2): the policy path, its current digest,
/// the approval state, and the approval when one reads. A read: always
/// exits `0`, whatever the state.
pub fn run_show(repo: &Path, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let state = approval_state(&repo);
    let current = fs::read(repo.join(paths::POLICY_FILE)).ok().map(|bytes| policy_digest(&bytes));
    let approval = match &state {
        ApprovalState::Approved(approval) | ApprovalState::Changed { approval, .. } => Some(approval),
        _ => None,
    };
    let unreadable = match &state {
        ApprovalState::Unreadable { detail } => Some(detail.as_str()),
        _ => None,
    };
    if json {
        let approval = approval.map(|a| serde_json::json!({ "policyDigest": a.policy_digest, "approvedBy": a.approved_by, "approvedAt": a.approved_at, "note": a.note }));
        crate::json_out::data(
            "policy show",
            serde_json::json!({ "policy": paths::POLICY_FILE, "policyDigest": current, "state": state.as_str(), "approvalFile": paths::POLICY_APPROVAL_FILE, "approval": approval, "approvalError": unreadable }),
        );
        return 0;
    }
    println!("canon policy show: {}", paths::POLICY_FILE);
    println!("  state: {}", state.as_str());
    if let Some(digest) = &current {
        println!("  digest: {digest}");
    }
    if let Some(a) = approval {
        let note = a.note.as_deref().map(|note| format!(" — {note}")).unwrap_or_default();
        println!("  approved: {} by `{}` at {}{note}", a.policy_digest, a.approved_by, a.approved_at);
    }
    match &state {
        ApprovalState::NoPolicy => println!("  no policy file, so there is nothing to approve"),
        ApprovalState::Approved(_) => {}
        ApprovalState::Unapproved => println!("  no approval record ({}); a human reviews the policy and runs `canon policy approve --by <who>`", paths::POLICY_APPROVAL_FILE),
        ApprovalState::Changed { .. } | ApprovalState::Unreadable { .. } => {
            if let Some(detail) = unreadable {
                println!("  {} does not read: {detail}", paths::POLICY_APPROVAL_FILE);
            }
            println!("  `canon gate check` fails and `canon subject status` refuses until {}", canon_gate::policy_approval::FIX);
        }
    }
    0
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
        assert!(advisory(dir.path()).is_some_and(|line| line.starts_with("policy not approved")));
        assert_eq!(run_approve(dir.path(), "alice", Some("first look"), false), 0);
        assert!(matches!(&approval_state(dir.path()), ApprovalState::Approved(a) if a.approved_by == "alice" && a.note.as_deref() == Some("first look")));
        assert_eq!(advisory(dir.path()), None);

        fs::write(dir.path().join(paths::POLICY_FILE), "spec_coverage: { require_evidence: true }\n").unwrap();
        assert!(matches!(approval_state(dir.path()), ApprovalState::Changed { .. }));
        assert_eq!(advisory(dir.path()), None, "a changed policy is a violation, not an advisory");
        assert!(refusal_hint(true).contains("canon policy show") && refusal_hint(true).contains("does not waive"));
        assert!(review_step(dir.path()).is_some_and(|step| step.actor == StepActor::Human));

        fs::write(dir.path().join(paths::POLICY_APPROVAL_FILE), "not json").unwrap();
        assert!(review_step(dir.path()).is_some(), "an unreadable approval holds every subject too");
    }

    #[test]
    fn no_policy_is_refused_and_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(run_approve(dir.path(), "alice", None, false), 2);
        assert!(!dir.path().join(paths::POLICY_APPROVAL_FILE).exists());
        assert_eq!(advisory(dir.path()), None);
        assert!(review_step(dir.path()).is_none());
        assert_eq!(run_show(dir.path(), true), 0);
    }
}
