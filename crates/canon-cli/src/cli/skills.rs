//! `canon skills` — the clap tree of the group.

use clap::{Args, Subcommand};
use std::path::PathBuf;

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
    pub skills_source: Option<PathBuf>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Providers to project (`claude`, `codex`, `omp`, `pi`, a comma-separated list, or `none`);
    /// also rewrites `skills.providers` in the target's canon.yaml. Omitted: canon.yaml
    /// `skills.providers`, else the install lock's providers, else the detected `.claude`,
    /// `.codex`, `.omp`, `.pi` directories; none detected installs nothing. Codex is
    /// projected to `.agents/skills/canon/`.
    #[arg(long)]
    pub providers: Option<String>,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Args)]
pub struct SkillsCheckArgs {
    #[arg(long)]
    pub skills_source: Option<PathBuf>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
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
    pub skills_source: Option<PathBuf>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Providers to inspect (`claude`, `codex`, `omp`, `pi`, or a comma-separated list). Omitted: as for install.
    #[arg(long)]
    pub providers: Option<String>,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
