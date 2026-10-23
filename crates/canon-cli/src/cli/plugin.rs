//! `canon plugin` — the clap tree of the group.

use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum PluginCommand {
    /// Validate and write a plugin's overlay records
    Sync(PluginSyncArgs),
}

#[derive(Args)]
pub struct PluginSyncArgs {
    /// A .canon/plugins/<id>/plugin.yaml manifest id (e.g. porting)
    #[arg(value_name = "PLUGIN_ID")]
    pub plugin_id: String,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Sync exactly one ad hoc root, overriding canon.yaml's specs.roots[]
    #[arg(long, value_name = "DIR")]
    pub spec_root: Option<PathBuf>,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}
