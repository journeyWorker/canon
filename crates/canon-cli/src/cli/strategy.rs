//! `canon strategy` — the clap tree of the group.

use canon_learn::StrategyId;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum StrategyCommand {
    /// Promote a strategy after paired evaluation and signed approval
    Promote(StrategyPromoteArgs),
    /// The unsigned approval payload bytes an external signer signs (a read: identical arguments give identical bytes)
    ApprovalPayload(StrategyApprovalPayloadArgs),
    /// Disable an active strategy with durable rollback provenance
    Rollback(StrategyRollbackArgs),
}

#[derive(Args)]
pub struct StrategyPromoteArgs {
    /// The StrategyId (ULID) to promote
    #[arg(value_name = "STRATEGY_ID", value_parser = crate::learn::parse_strategy_id)]
    pub strategy_id: StrategyId,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Evaluation bundle produced by the honest promotion adapter
    #[arg(long, requires = "approval")]
    pub evaluation: Option<PathBuf>,
    /// Approval JSON containing the detached SSH signature
    #[arg(long, requires = "evaluation")]
    pub approval: Option<PathBuf>,
    /// Detached signature file to attach to the approval JSON
    #[arg(long, requires = "approval")]
    pub signature_file: Option<PathBuf>,
    /// Preview without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct StrategyApprovalPayloadArgs {
    /// The StrategyId (ULID) whose approval payload to print
    #[arg(value_name = "STRATEGY_ID", value_parser = crate::learn::parse_strategy_id)]
    pub strategy_id: StrategyId,
    /// The paired evaluation bundle the approval is for
    #[arg(long)]
    pub evaluation: PathBuf,
    /// Who approves (the human whose detached signature the payload is for)
    #[arg(long)]
    pub approver: String,
    /// The approval time that is signed into the payload: RFC3339, or YYYY-MM-DD (UTC midnight)
    #[arg(long, value_name = "TIMESTAMP", value_parser = crate::timestamp::parse)]
    pub approval_at: chrono::DateTime<chrono::Utc>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Envelope the payload as JSON instead of printing its bytes
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct StrategyRollbackArgs {
    /// The StrategyId (ULID) to roll back
    #[arg(value_name = "STRATEGY_ID", value_parser = crate::learn::parse_strategy_id)]
    pub strategy_id: StrategyId,
    /// Why the strategy is rolled back (signed: `strategy:<id>:rollback:<sha256(reason)>`)
    #[arg(long)]
    pub reason: String,
    /// The approver; must be a policy-pinned approver
    #[arg(long)]
    pub actor_id: String,
    /// Detached signature file over the rollback payload
    #[arg(long)]
    pub signature_file: PathBuf,
    /// When the approver signed: RFC3339, or YYYY-MM-DD (UTC midnight)
    #[arg(long, value_name = "TIMESTAMP", value_parser = crate::timestamp::normalize)]
    pub approval_at: String,
    /// A trajectory that contradicts the strategy (must exist in the trajectory store)
    #[arg(long)]
    pub contradicting_trajectory_id: Option<String>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
