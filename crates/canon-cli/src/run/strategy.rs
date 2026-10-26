//! `canon strategy` — the handlers of the group.

use crate::cli::strategy::{StrategyCommand, StrategyPromoteArgs, StrategyApprovalPayloadArgs, StrategyRollbackArgs};
use std::process::ExitCode;

pub fn run(cmd: StrategyCommand) -> ExitCode {
    match cmd {
        StrategyCommand::Promote(StrategyPromoteArgs { strategy_id, repo, evaluation, approval, signature_file, dry_run, json, .. }) =>
            crate::learn::run_promote_with_evidence(&repo, &strategy_id, dry_run.dry_run, evaluation.as_deref(), approval.as_deref(), signature_file.as_deref(), json),
        StrategyCommand::ApprovalPayload(StrategyApprovalPayloadArgs { strategy_id, evaluation, approver, approval_at, repo, json }) =>
            crate::learn::run_approval_payload(&repo, &strategy_id, &evaluation, &approver, approval_at, json),
        StrategyCommand::Rollback(StrategyRollbackArgs { strategy_id, reason, actor_id, signature_file, approval_at, contradicting_trajectory_id, repo, json, .. }) =>
            crate::learn::run_rollback(&repo, &strategy_id, &reason, &actor_id, contradicting_trajectory_id.as_deref(), &signature_file, &approval_at, json),
    }
}
