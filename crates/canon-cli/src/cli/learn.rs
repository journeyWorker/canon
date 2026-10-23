//! `canon learn` — the clap tree of the group.

use canon_learn::StrategyId;
use clap::{Args, Subcommand};
use std::path::PathBuf;

/// The flags of `context-pack new`.
#[derive(clap::Args)]
pub struct ContextPackNewArgs {
    /// Repository-relative input manifest
    #[arg(long)]
    pub manifest: PathBuf,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output the complete pack manifest as JSON
    #[arg(long)]
    pub json: bool,
}

/// The flags of `prompt add`.
#[derive(clap::Args)]
pub struct PromptAddArgs {
    /// Bundle name
    #[arg(long)]
    pub name: String,
    /// Bundle version
    #[arg(long)]
    pub version: String,
    /// Repository-relative input manifest
    #[arg(long)]
    pub manifest: PathBuf,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output the complete bundle manifest as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum LearnCommand {
    /// Promote a strategy after paired evaluation and signed approval
    Promote(LearnPromoteArgs),
    /// Produce an unsigned, externally signable approval payload
    Approve(LearnApproveArgs),
    /// Disable an active strategy with durable rollback provenance
    Rollback(LearnRollbackArgs),
}

#[derive(Args)]
pub struct LearnPromoteArgs {
    /// The StrategyId (ULID) to promote
    #[arg(value_name = "STRATEGY_ID", value_parser = crate::learn::parse_strategy_id)]
    pub strategy_id: StrategyId,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Evaluation bundle produced by the honest promotion adapter
    #[arg(long)]
    pub evaluation: Option<PathBuf>,
    /// Approval JSON containing the detached SSH signature
    #[arg(long)]
    pub approval: Option<PathBuf>,
    /// Optional detached signature file to attach to approval JSON
    #[arg(long)]
    pub signature: Option<PathBuf>,
    /// Preview without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct LearnApproveArgs {
    #[arg(value_name = "STRATEGY_ID", value_parser = crate::learn::parse_strategy_id)]
    pub strategy_id: StrategyId,
    #[arg(long)]
    pub evaluation: PathBuf,
    #[arg(long)]
    pub principal: String,
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct LearnRollbackArgs {
    #[arg(value_name = "STRATEGY_ID", value_parser = crate::learn::parse_strategy_id)]
    pub strategy_id: StrategyId,
    #[arg(long)]
    pub reason: String,
    #[arg(long)]
    pub actor_id: String,
    #[arg(long)]
    pub signature_file: PathBuf,
    #[arg(long)]
    pub approved_at: String,
    #[arg(long)]
    pub contradicting_trajectory_id: Option<String>,
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
