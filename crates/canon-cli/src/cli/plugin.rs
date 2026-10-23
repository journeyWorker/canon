//! `canon plugin` — the clap tree of the group.

use canon_model::{ProjectId, RoleId, ScenarioId, Sha};
use chrono::{DateTime, Utc};
use clap::{Args, Subcommand};
use std::path::PathBuf;
use super::PromoteArgs;

/// The flags of `context-pack new`.
#[derive(clap::Args)]
pub struct ContextPackNewArgs {
    /// Repository-relative input manifest
    #[arg(long)]
    pub manifest: PathBuf,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output the complete pack manifest as JSON
    #[arg(long)]
    pub json: bool,
}

/// The flags of `prompt add`.
#[derive(clap::Args)]
pub struct PromptAddArgs {
    /// Bundle name
    #[arg(long)]
    pub name: String,
    /// Bundle version
    #[arg(long)]
    pub version: String,
    /// Repository-relative input manifest
    #[arg(long)]
    pub manifest: PathBuf,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output the complete bundle manifest as JSON
    #[arg(long)]
    pub json: bool,
}

/// The flags of `demo add`.
#[derive(clap::Args)]
pub struct DemoAddArgs {
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

/// The flags of `divergence add`.
#[derive(clap::Args)]
pub struct DivergenceAddArgs {
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: ProjectId,
    #[arg(long, value_parser = crate::review::parse_scenario_id)]
    pub scenario_id: ScenarioId,
    #[arg(long, value_parser = crate::divergence::parse_sha)]
    pub sha: Sha,
    /// open / still-divergent / resolved / deferred (deferred needs --reason/--expiry)
    #[arg(long, default_value = "open")]
    pub status: String,
    #[arg(long)]
    pub reason: Option<String>,
    #[arg(long, value_parser = crate::divergence::parse_timestamp)]
    pub expiry: Option<DateTime<Utc>>,
    #[arg(long, default_value_t = 1)]
    pub round: u32,
    #[arg(long)]
    pub reviewer: String,
    #[arg(long, default_value = "")]
    pub detail: String,
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: RoleId,
    #[command(flatten)]
    pub promote: PromoteArgs,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum PluginCommand {
    /// Validate and write a plugin's overlay records
    Sync(PluginSyncArgs),
}

#[derive(Args)]
pub struct PluginSyncArgs {
    /// A .canon/plugins/<id>/plugin.yaml manifest id (e.g. porting)
    pub plugin_id: String,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Sync exactly one ad hoc root, overriding canon.yaml's specs.roots[]
    #[arg(long)]
    pub spec_root: Option<PathBuf>,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
