//! `canon ingest` — the clap tree of the group.

use canon_model::{RunId};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum IngestCommand {
    /// Import agent CLI session transcripts (omp/pi, Claude Code, Codex, Hermes)
    Sessions(IngestSessionsArgs),
    /// Import review/divergence/task/handoff artifacts and derive verdicts
    Artifacts(IngestArtifactsArgs),
    /// Import plans as Change/Task records: canon's own .canon/changes, then the configured external sources (openspec, superpowers)
    Plans(IngestPlansArgs),
}

#[derive(Args)]
pub struct IngestSessionsArgs {
    /// Keep polling instead of exiting after one pass
    #[arg(long)]
    pub watch: bool,
    /// Seconds between polls
    #[arg(long, default_value_t = 30)]
    pub interval_secs: u64,
    /// The scan root's home directory (defaults to $HOME)
    #[arg(long)]
    pub home: Option<PathBuf>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Ignore watermark cursors and re-parse every in-scope file
    #[arg(long)]
    pub full: bool,
    /// Scan every workspace on this machine, not just this project
    #[arg(long)]
    pub all_workspaces: bool,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct IngestArtifactsArgs {
    /// Keep polling instead of exiting after one pass
    #[arg(long)]
    pub watch: bool,
    /// Seconds between polls
    #[arg(long, default_value_t = 30)]
    pub interval_secs: u64,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// Attribute this pass's trajectories to a dispatched run (a RunId minted by `canon dispatch begin`)
    #[arg(long, value_name = "RUN_ID", value_parser = crate::dispatch::parse_run_id)]
    pub run_id: Option<RunId>,
}

#[derive(Args)]
pub struct IngestPlansArgs {
    /// One-shot override: import this dialect's --source root (requires --source)
    #[arg(long)]
    pub dialect: Option<String>,
    /// One-shot override's source root (paired with --dialect)
    #[arg(long)]
    pub source: Option<PathBuf>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
