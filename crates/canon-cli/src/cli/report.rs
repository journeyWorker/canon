//! `canon report` — the clap tree of the group.

use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ReportCommand {
    /// Write the status report (.canon/REPORT.md)
    Build(ReportBuildArgs),
    /// Byte-diff .canon/REPORT.md against a fresh build (0 fresh, 1 missing or drifted); writes nothing
    Check(ReportCheckArgs),
    /// Export the panel marts to <DIR>/*.parquet plus manifest.json
    Snapshot(ReportSnapshotArgs),
}

#[derive(Args)]
pub struct ReportBuildArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ReportCheckArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ReportSnapshotArgs {
    /// The output directory (created if absent)
    #[arg(value_name = "DIR")]
    pub dir: PathBuf,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
