//! `canon inventory` — the clap tree of the group.

use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum InventoryCommand {
    /// Validate each spec root, then materialize scenario index records
    Sync(InventorySyncArgs),
}

#[derive(Args)]
pub struct InventorySyncArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Sync exactly one ad hoc root, overriding canon.yaml's specs.roots[]
    #[arg(long)]
    pub spec_root: Option<PathBuf>,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
