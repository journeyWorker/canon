//! `canon ledger` — the clap tree of the group.

use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum LedgerCommand {
    /// Move superseded versions of every record out of the ledger into the archive
    Compact(LedgerCompactArgs),
}

#[derive(Args)]
pub struct LedgerCompactArgs {
    /// Report what would move and write nothing
    #[arg(long)]
    pub dry_run: bool,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
