//! `canon ledger` — the clap tree of the group.

use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum LedgerCommand {
    /// Move superseded versions of every record out of the ledger into the archive
    Compact(LedgerCompactArgs),
}

#[derive(Debug, Args)]
pub struct LedgerCompactArgs {
    #[command(flatten)]
    pub safety: crate::cli::SafetyArgs,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}
