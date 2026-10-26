//! The metadata row of every command, by exhaustive `match`: adding a command
//! (or a group's leaf) without saying which row it is fails to compile.

use super::{Command, PolicyCommand, DemoCommand, ContextPackCommand, AdapterCommand, PromptCommand, FeatureCommand, ScenarioCommand, SubjectCommand, ChangeCommand, BriefCommand, InventoryCommand, GateCommand, TaskCommand, EvidenceCommand, ArtifactsCommand, FindingCommand, ReviewCommand, DivergenceCliCommand, IngestCommand, StrategyCommand, DispatchCommand, RegimeCommand, ReportCommand, PluginCommand, SkillsCommand, TierCommand, LedgerCommand};
use crate::schema::{meta_for, CommandMeta};

/// The canonical command path of `cmd`, e.g. `evidence add`. No wildcard arm:
/// a new variant is a compile error until it is listed here.
pub fn path(cmd: &Command) -> &'static str {
    match cmd {
        Command::Init(_) => "init",
        Command::Migrate(_) => "migrate",
        Command::Policy { action } => match action {
            PolicyCommand::Approve(_) => "policy approve",
            PolicyCommand::Show(_) => "policy show",
        },
        Command::Demo { action } => match action {
            DemoCommand::Init(_) => "demo init",
            DemoCommand::Review(_) => "demo review",
        },
        Command::Status(_) => "status",
        Command::Context(_) => "context",
        Command::ContextPack { action } => match action {
            ContextPackCommand::Add(_) => "context-pack add",
            ContextPackCommand::Show(_) => "context-pack show",
            ContextPackCommand::Verify(_) => "context-pack verify",
        },
        Command::Adapter { action } => match action {
            AdapterCommand::Validate(_) => "adapter validate",
            AdapterCommand::Authorize(_) => "adapter authorize",
        },
        Command::Prompt { action } => match action {
            PromptCommand::Add(_) => "prompt add",
            PromptCommand::Show(_) => "prompt show",
        },
        Command::Format(_) => "format",
        Command::Feature { action } => match action {
            FeatureCommand::New(_) => "feature new",
        },
        Command::Scenario { action } => match action {
            ScenarioCommand::New(_) => "scenario new",
        },
        Command::Subject { action } => match action {
            SubjectCommand::New(_) => "subject new",
            SubjectCommand::Adopt(_) => "subject adopt",
            SubjectCommand::Unadopt(_) => "subject unadopt",
            SubjectCommand::Show(_) => "subject show",
            SubjectCommand::Move(_) => "subject move",
        },
        Command::Change { action } => match action {
            ChangeCommand::New(_) => "change new",
        },
        Command::Brief { action } => match action {
            BriefCommand::Show(_) => "brief show",
            BriefCommand::Check(_) => "brief check",
            BriefCommand::Approve(_) => "brief approve",
        },
        Command::Inventory { action } => match action {
            InventoryCommand::Sync(_) => "inventory sync",
        },
        Command::Gate { action } => match action {
            GateCommand::Check(_) => "gate check",
            GateCommand::Promote(_) => "gate promote",
            GateCommand::InstallHooks(_) => "gate install-hooks",
        },
        Command::Task { action } => match action {
            TaskCommand::Done(_) => "task done",
        },
        Command::Evidence { action } => match action {
            EvidenceCommand::ApprovalPayload(_) => "evidence approval-payload",
            EvidenceCommand::Add(_) => "evidence add",
            EvidenceCommand::Audit(_) => "evidence audit",
        },
        Command::Artifacts { action } => match action {
            ArtifactsCommand::Verify(_) => "artifacts verify",
            ArtifactsCommand::Store(_) => "artifacts store",
            ArtifactsCommand::Move(_) => "artifacts move",
        },
        Command::Finding { action } => match action {
            FindingCommand::Add(_) => "finding add",
            FindingCommand::Close(_) => "finding close",
            FindingCommand::List(_) => "finding list",
        },
        Command::Review { action } => match action {
            ReviewCommand::Scenario(_) => "review scenario",
            ReviewCommand::Change(_) => "review change",
        },
        Command::Divergence { action } => match action {
            DivergenceCliCommand::Add(_) => "divergence add",
            DivergenceCliCommand::Resolve(_) => "divergence resolve",
            DivergenceCliCommand::Defer(_) => "divergence defer",
            DivergenceCliCommand::Accept(_) => "divergence accept",
            DivergenceCliCommand::List(_) => "divergence list",
        },
        Command::Ingest { action } => match action {
            IngestCommand::Sessions(_) => "ingest sessions",
            IngestCommand::Records(_) => "ingest records",
            IngestCommand::Plans(_) => "ingest plans",
        },
        Command::Retrieve(_) => "retrieve",
        Command::Strategy { action } => match action {
            StrategyCommand::Promote(_) => "strategy promote",
            StrategyCommand::ApprovalPayload(_) => "strategy approval-payload",
            StrategyCommand::Rollback(_) => "strategy rollback",
        },
        Command::Dispatch { action } => match action {
            DispatchCommand::Begin(_) => "dispatch begin",
            DispatchCommand::End(_) => "dispatch end",
            DispatchCommand::Diff(_) => "dispatch diff",
        },
        Command::Regime { action } => match action {
            RegimeCommand::Show(_) => "regime show",
        },
        Command::Query(_) => "query",
        Command::Export(_) => "export",
        Command::Purge(_) => "purge",
        Command::Report { action } => match action {
            ReportCommand::Build(_) => "report build",
            ReportCommand::Check(_) => "report check",
            ReportCommand::Snapshot(_) => "report snapshot",
        },
        Command::Dashboard(_) => "dashboard",
        Command::Plugin { action } => match action {
            PluginCommand::Sync(_) => "plugin sync",
        },
        Command::Skills { action } => match action {
            SkillsCommand::Install(_) => "skills install",
            SkillsCommand::Check(_) => "skills check",
            SkillsCommand::Doctor(_) => "skills doctor",
            SkillsCommand::Show(_) => "skills show",
        },
        Command::Tier { action } => match action {
            TierCommand::Age(_) => "tier age",
        },
        Command::Ledger { action } => match action {
            LedgerCommand::Compact(_) => "ledger compact",
        },
        Command::Selftest(_) => "selftest",
        Command::Schema(_) => "schema",
        Command::Completion(_) => "completion",
    }
}

/// The [`CommandMeta`] row of `cmd`. Every leaf has exactly one row in
/// [`crate::schema::META`]; `tests/cli_tree.rs` proves each leaf reaches its own.
pub fn meta(cmd: &Command) -> &'static CommandMeta {
    let path = path(cmd);
    meta_for(path).unwrap_or_else(|| panic!("`canon {path}` has no metadata row"))
}
