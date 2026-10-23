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
    #[arg(long, value_name = "NAME")]
    pub approver: String,
    /// Why, or what was reviewed
    #[arg(long, value_name = "TEXT")]
    pub note: Option<String>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Args)]
pub struct PolicyShowArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
