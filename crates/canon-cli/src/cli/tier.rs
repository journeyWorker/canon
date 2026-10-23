//! `canon tier` — the clap tree of the group.

use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum TierCommand {
    /// Apply canon.yaml aging rules, moving old records to their destination tier
    Age(TierAgeArgs),
}

#[derive(Args)]
pub struct TierAgeArgs {
    /// Preview without writing anything
    #[arg(long)]
    pub dry_run: bool,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
