//! `canon dispatch` — the clap tree of the group.

use canon_model::{RegimeKey, RoleId, RunId, RunStatus, TaskId};
use clap::{Args, Subcommand};
use std::path::PathBuf;

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
    #[arg(long, value_parser = crate::retrieve::parse_role, value_name = "ROLE")]
    pub role: Option<RoleId>,
    /// Full regime key (<role>/<repo>/<area>/<hash>) to retrieve guidance for
    #[arg(long, value_parser = crate::retrieve::parse_regime, value_name = "KEY")]
    pub regime: RegimeKey,
    /// The dispatching agent's id (recorded as the run's actor)
    #[arg(long, default_value = "canon", value_name = "ACTOR_ID")]
    pub actor_id: String,
    #[command(flatten)]
    pub session: crate::cli::SessionArgs,
    /// Declared provider metadata (copied verbatim; never inferred from --actor-id)
    #[arg(long, value_name = "NAME")]
    pub provider: Option<String>,
    /// Declared model metadata (copied verbatim)
    #[arg(long, value_name = "NAME")]
    pub model: Option<String>,
    /// Skill that contributed to this run
    #[arg(long, value_name = "ID")]
    pub skill_id: Option<String>,
    /// Declared skill digest; requires --skill-id
    #[arg(long, value_name = "DIGEST")]
    pub skill_digest: Option<String>,
    /// Explicit repository-relative ContextPack input manifest
    #[arg(long, value_name = "FILE")]
    pub manifest: Option<PathBuf>,
    /// Registered prompt bundle to select (`name@version`)
    #[arg(long, value_parser = crate::dispatch::parse_prompt_bundle, value_name = "NAME@VERSION")]
    pub prompt_bundle: Option<crate::context_pack::PromptBundleSelection>,
    /// Plan task this run serves (<change_id>#<n>); validated against the plan corpus
    #[arg(long, value_parser = crate::dispatch::parse_task_id, value_name = "TASK_ID")]
    pub task_id: Option<TaskId>,
    /// RunId (ULID) of the run that dispatched this one
    #[arg(long, value_parser = crate::dispatch::parse_run_id, value_name = "RUN_ID")]
    pub parent_run_id: Option<RunId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Args)]
pub struct DispatchEndArgs {
    /// The RunId (ULID) minted by `canon dispatch begin`
    #[arg(value_name = "RUN_ID", value_parser = crate::dispatch::parse_run_id)]
    pub run_id: RunId,
    /// Terminal outcome: succeeded or failed
    #[arg(long, value_parser = crate::dispatch::run_status_parser(), value_name = "RUN_OUTCOME")]
    pub run_outcome: RunStatus,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Args)]
pub struct DispatchDiffArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
