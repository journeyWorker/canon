//! `canon policy` — the clap tree of the group.

use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum PolicyCommand {
    /// Approve the current .canon/policy.yaml (written directly; nothing to promote)
    Approve(PolicyApproveArgs),
    /// Show the policy's digest and its approval (who, when, which bytes)
    Show(PolicyShowArgs),
}

#[derive(Debug, Args)]
pub struct PolicyApproveArgs {
    /// Who approved it (a human's name or id)
    #[arg(long, value_name = "NAME")]
    pub approver: String,
    /// Why, or what was reviewed
    #[arg(long, value_name = "TEXT")]
    pub note: Option<String>,
    /// Record the human's answer on their behalf: the tool they were asked with (`ask`, `AskUserQuestion`, `chat`); an agent role may run the call only with it, and then --note carries the human's answer
    #[arg(long, value_name = "NAME")]
    pub relayed_via: Option<String>,
    /// With --relayed-via, the agent's actor id (default: the CANON_ACTOR environment variable)
    #[arg(long, value_name = "ACTOR_ID", requires = "relayed_via")]
    pub actor: Option<String>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct PolicyShowArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
