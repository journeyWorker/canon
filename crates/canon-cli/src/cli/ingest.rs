//! `canon ingest` — the clap tree of the group.

use canon_model::{RunId};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum IngestCommand {
    /// Import agent CLI session transcripts (omp/pi, Claude Code, Codex, Hermes)
    Sessions(IngestSessionsArgs),
    /// Import review/divergence/task/handoff records and derive verdicts
    Records(IngestRecordsArgs),
    /// Import plans as Change/Task records: canon's own .canon/changes, then the configured external sources (openspec, superpowers)
    Plans(IngestPlansArgs),
}

#[derive(Debug, Args)]
pub struct IngestSessionsArgs {
    /// Keep polling instead of exiting after one pass
    #[arg(long)]
    pub watch: bool,
    /// Seconds between polls
    #[arg(long, default_value_t = 30, value_name = "N")]
    pub interval_secs: u64,
    /// The scan root's home directory (defaults to $HOME)
    #[arg(long, value_name = "DIR")]
    pub home: Option<PathBuf>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Ignore watermark cursors and re-parse every in-scope file
    #[arg(long)]
    pub full: bool,
    /// Scan every workspace on this machine, not just this project
    #[arg(long)]
    pub all_workspaces: bool,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct IngestRecordsArgs {
    /// Keep polling instead of exiting after one pass
    #[arg(long)]
    pub watch: bool,
    /// Seconds between polls
    #[arg(long, default_value_t = 30, value_name = "N")]
    pub interval_secs: u64,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// Attribute this pass's trajectories to a dispatched run (a RunId minted by `canon dispatch begin`)
    #[arg(long = "run", value_name = "RUN_ID", value_parser = crate::dispatch::parse_run_id)]
    pub run_id: Option<RunId>,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct IngestPlansArgs {
    /// One-shot override: import this dialect's --plan-source root (requires --plan-source)
    #[arg(long, requires = "plan_source", value_name = "NAME")]
    pub dialect: Option<String>,
    /// One-shot override's source root (paired with --dialect)
    #[arg(long, value_name = "DIR", requires = "dialect")]
    pub plan_source: Option<PathBuf>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}
