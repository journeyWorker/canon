//! `canon policy` — the clap tree of the group.

use clap::{Args, Subcommand};
use std::path::PathBuf;

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
    pub by: String,
    /// Why, or what was reviewed
    #[arg(long)]
    pub note: Option<String>,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct PolicyShowArgs {
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
