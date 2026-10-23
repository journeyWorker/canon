//! `canon demo` — the clap tree of the group.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// The flags of `demo review`.
#[derive(clap::Args)]
pub struct DemoReviewArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Subcommand)]
pub enum DemoCommand {
    /// Scaffold the demo repo (gate starts RED)
    Init(DemoInitArgs),
    /// Record the missing reviewer evidence (gate turns GREEN)
    Review(DemoReviewArgs),
}

#[derive(Args)]
pub struct DemoInitArgs {
    /// Directory to set up (used as-is)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}
