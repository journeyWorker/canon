//! `canon dispatch` — the clap tree of the group.

use canon_model::{RegimeKey, RoleId, RunId, RunStatus, TaskId};
use clap::{Args, Subcommand};
use std::path::PathBuf;

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

#[derive(Subcommand)]
pub enum DispatchCommand {
    /// Mint a Run manifest with retrieved guidance and context/policy lineage
    Begin(DispatchBeginArgs),
    /// Close a dispatched run with a terminal status and an ended_at
    End(DispatchEndArgs),
    /// Diff the declared plan DAG against the observed execution graph
    Diff(DispatchDiffArgs),
}

#[derive(Args)]
pub struct DispatchBeginArgs {
    /// Role about to run; must equal --regime's leading segment
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: RoleId,
    /// Full regime key (<role>/<repo>/<area>/<hash>) to retrieve guidance for
    #[arg(long, value_parser = crate::retrieve::parse_regime)]
    pub regime: RegimeKey,
    /// The dispatching agent's id (recorded as the run's actor)
    #[arg(long, default_value = "canon")]
    pub agent_id: String,
    /// Declared provider metadata (copied verbatim; never inferred from --agent-id)
    #[arg(long)]
    pub provider: Option<String>,
    /// Declared model metadata (copied verbatim)
    #[arg(long)]
    pub model: Option<String>,
    /// Skill that contributed to this run
    #[arg(long)]
    pub skill_id: Option<String>,
    /// Declared skill digest; requires --skill-id
    #[arg(long)]
    pub skill_digest: Option<String>,
    /// Explicit repository-relative ContextPack input manifest
    #[arg(long)]
    pub context_manifest: Option<PathBuf>,
    /// Registered prompt bundle to select (`name@version`)
    #[arg(long, value_parser = crate::dispatch::parse_prompt_bundle)]
    pub prompt_bundle: Option<crate::context_pack::PromptBundleSelection>,
    /// Plan task this run serves (<change_id>#<n>); validated against the plan corpus
    #[arg(long, value_parser = crate::dispatch::parse_task_id)]
    pub task: Option<TaskId>,
    /// RunId (ULID) of the run that dispatched this one
    #[arg(long, value_parser = crate::dispatch::parse_run_id)]
    pub parent_run: Option<RunId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DispatchEndArgs {
    /// The RunId (ULID) minted by `canon dispatch begin`
    #[arg(value_name = "RUN_ID", value_parser = crate::dispatch::parse_run_id)]
    pub run_id: RunId,
    /// Terminal outcome: succeeded or failed
    #[arg(long, value_parser = crate::dispatch::run_status_parser())]
    pub status: RunStatus,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DispatchDiffArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
