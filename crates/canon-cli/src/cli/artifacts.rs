//! `canon artifacts` — the clap tree of the group.

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
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
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
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output the complete bundle manifest as JSON
    #[arg(long)]
    pub json: bool,
}

/// The flags of `demo add`.
#[derive(clap::Args)]
pub struct DemoAddArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
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
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum ArtifactsCommand {
    /// Check that the artifact store holds every file the latest evidence and current reviews bind (--full re-hashes the stored bytes)
    #[command(after_help = "Asks the store canon.yaml's `artifacts:` block selects (default: local, .canon/artifacts/sha256) about each bound file, the way `canon gate check` does. For an object store that is one HEAD per distinct digest plus the size and sha256 metadata written at upload; --full downloads each blob and re-hashes it.\n\nExit 0 when every file is stored, 1 when one is missing or altered (data.problems names each), 2 when the store cannot be used or reached: an unreachable store is a refusal, never a pass.")]
    Verify(ArtifactsVerifyArgs),
    /// Move the local artifact blobs to an object-store tier, verify them, and remove the local copies
    #[command(after_help = "Examples:\n  canon artifacts migrate --to cold --dry-run\n  canon artifacts migrate --to cold\n\n--to names an `s3` tier of canon.yaml's `tiers:` (for example `cold`; its credentials are the tier's own). Every blob in .canon/artifacts/sha256 is hashed, uploaded under <prefix>sha256/<digest> (prefix artifacts/), and read back; only when all of them verified does canon.yaml gain `artifacts: { backend: <tier>, prefix: artifacts/ }`, after which the local copies are removed and `.canon/artifacts/` is added to .gitignore. Commit the removal with the ledger.\n\nIdempotent and resumable: a run stopped anywhere leaves a repo the gate still passes, and the next run finishes; a second run finds nothing to do. A blob whose bytes do not match its name is never migrated, and stops the run with nothing removed. `--to local` is a no-op on a local repo; moving bytes back is not supported.")]
    Migrate(ArtifactsMigrateArgs),
}

#[derive(Args)]
pub struct ArtifactsVerifyArgs {
    /// Download every blob and re-hash it, instead of trusting the store's size and digest metadata
    #[arg(long)]
    pub full: bool,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ArtifactsMigrateArgs {
    /// The backend to move to: the name of an `s3` tier in canon.yaml (for example `cold`)
    #[arg(long, value_name = "BACKEND")]
    pub to: String,
    /// Preview what would be uploaded and removed, without writing anything
    #[arg(long)]
    pub dry_run: bool,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
