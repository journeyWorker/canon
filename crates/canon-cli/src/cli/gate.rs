//! `canon gate` — the clap tree of the group.

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
pub enum GateCommand {
    /// Run the coverage/ledger/staleness/trust checks (0 clean, 1 red, 2 usage)
    Check(GateCheckArgs),
    /// Flip one task checkbox, gated on real evidence (fails closed)
    Task(GateTaskArgs),
    /// Promote staged evidence records to the committed ledger
    Promote(GatePromoteArgs),
    /// Install the gate hook into .claude/settings.json / .codex/hooks.json
    InstallHooks(GateInstallHooksArgs),
    /// Run the gate's self-contained fixture self-test
    Selftest(GateSelftestArgs),
}

#[derive(Args)]
pub struct GateCheckArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Additionally engage the release-scoped trust check
    #[arg(long)]
    pub release: bool,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct GateTaskArgs {
    /// The task id (<change_id>#<n>)
    pub task_id: String,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct GatePromoteArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Preview without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct GateInstallHooksArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// The hook event name (e.g. PreToolUse, Stop)
    #[arg(long, default_value = "PreToolUse")]
    pub event: String,
    /// Omitted for a matcher-less event
    #[arg(long)]
    pub matcher: Option<String>,
    /// The command the hook runs
    #[arg(long, default_value = "canon gate task")]
    pub command: String,
    /// Hook timeout in seconds
    #[arg(long, default_value_t = 30)]
    pub timeout: u32,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct GateSelftestArgs {
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
