//! `canon gate` — the clap tree of the group.

use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum GateCommand {
    /// Run the coverage/ledger/staleness/trust checks (0 clean, 1 red, 2 usage)
    Check(GateCheckArgs),
    /// Promote staged records (evidence, findings, divergences) to the committed ledger
    Promote(GatePromoteArgs),
    /// Install the gate hook into .claude/settings.json / .codex/hooks.json
    InstallHooks(GateInstallHooksArgs),
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
pub struct GatePromoteArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Promote only this kind of staged record (default: every kind)
    #[arg(long, value_enum, value_name = "KIND")]
    pub kind: Option<PromoteKind>,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
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
    #[arg(long, default_value = "canon task done")]
    pub command: String,
    /// Hook timeout in seconds
    #[arg(long, default_value_t = 30)]
    pub timeout: u32,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

/// The staged record kinds `gate promote --kind` drains: the closed subset of
/// record kinds that `evidence add`, `finding add`/`finding close` and
/// `divergence add` stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum PromoteKind {
    Evidence,
    Finding,
    Divergence,
}

impl PromoteKind {
    pub fn record_kind(self) -> canon_model::envelope::RecordKind {
        use canon_model::envelope::RecordKind;
        match self {
            PromoteKind::Evidence => RecordKind::EvidenceRecord,
            PromoteKind::Finding => RecordKind::Finding,
            PromoteKind::Divergence => RecordKind::Divergence,
        }
    }
}
