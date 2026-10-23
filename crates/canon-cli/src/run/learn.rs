//! `canon learn` — the handlers of the group.

use crate::cli::learn::*;
use std::process::ExitCode;

pub fn run(cmd: LearnCommand) -> ExitCode {
    match cmd {
        LearnCommand::Promote(LearnPromoteArgs { strategy_id, repo, evaluation, approval, signature, dry_run, json }) =>
            crate::learn::run_promote_with_evidence(&repo, &strategy_id, dry_run.dry_run, evaluation.as_deref(), approval.as_deref(), signature.as_deref(), json),
        LearnCommand::Approve(LearnApproveArgs { strategy_id, evaluation, principal, repo, json }) =>
            crate::learn::run_approve(&repo, &strategy_id, &evaluation, &principal, json),
        LearnCommand::Rollback(LearnRollbackArgs { strategy_id, reason, actor_id, signature_file, approved_at, contradicting_trajectory_id, repo, json }) =>
            crate::learn::run_rollback(&repo, &strategy_id, &reason, &actor_id, contradicting_trajectory_id.as_deref(), &signature_file, &approved_at, json),
    }
}
