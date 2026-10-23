//! `canon policy` — the clap tree of the group.

use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum PolicyCommand {
    /// Approve the current .canon/policy.yaml (written directly; nothing to promote)
    Approve(PolicyApproveArgs),
    /// Show the policy's digest and its approval (who, when, which bytes)
    Show(PolicyShowArgs),
}

#[derive(Args)]
pub struct PolicyApproveArgs {
    /// Who approved it (a human's name or id)
    #[arg(long)]
    pub approver: String,
    /// Why, or what was reviewed
    #[arg(long)]
    pub note: Option<String>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct PolicyShowArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
