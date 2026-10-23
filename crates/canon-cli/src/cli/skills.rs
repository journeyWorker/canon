//! `canon skills` — the clap tree of the group.

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
pub enum SkillsCommand {
    /// Install the canonical `canon` bundle or legacy developer skills.
    Install(SkillsInstallArgs),
    /// Check canonical projections without writing; exit 1 on drift, a legacy remnant, or a lock another canon version wrote.
    Check(SkillsCheckArgs),
    /// Print diagnostics for canonical projections, the install lock's canon version, legacy remnants, canon projections for a provider that is not selected, and other skills whose description claims build work; reports only, never edits.
    Doctor(SkillsDoctorArgs),
}

#[derive(Args)]
pub struct SkillsInstallArgs {
    /// Source directory; defaults to CANON_SKILLS_SOURCE or canon/skills.
    #[arg(long)]
    pub source: Option<PathBuf>,
    /// Consumer repo root to materialize into.
    #[arg(long, default_value = ".")]
    pub target: PathBuf,
    /// Providers to project (`claude`, `codex`, `omp`, `pi`, a comma-separated list, or `none`);
    /// also rewrites `skills.providers` in the target's canon.yaml. Omitted: canon.yaml
    /// `skills.providers`, else the install lock's providers, else the detected `.claude`,
    /// `.codex`, `.omp`, `.pi` directories; none detected installs nothing. Codex is
    /// projected to `.agents/skills/canon/`.
    #[arg(long)]
    pub providers: Option<String>,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct SkillsCheckArgs {
    #[arg(long)]
    pub source: Option<PathBuf>,
    /// Consumer repo root to inspect.
    #[arg(long, default_value = ".")]
    pub target: PathBuf,
    /// Providers to inspect (`claude`, `codex`, `omp`, `pi`, or a comma-separated list). Omitted: as for install.
    #[arg(long)]
    pub providers: Option<String>,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct SkillsDoctorArgs {
    #[arg(long)]
    pub source: Option<PathBuf>,
    /// Consumer repo root to inspect.
    #[arg(long, default_value = ".")]
    pub target: PathBuf,
    /// Providers to inspect (`claude`, `codex`, `omp`, `pi`, or a comma-separated list). Omitted: as for install.
    #[arg(long)]
    pub providers: Option<String>,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
