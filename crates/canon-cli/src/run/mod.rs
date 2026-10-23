//! Running a parsed command line: [`run`] matches the top-level [`Command`]
//! and hands each group to its own module; [`entry`] holds what `main` needs
//! around the parse (usage errors, the missing-config refusal, SIGPIPE).

pub mod adapter;
pub mod artifacts;
pub mod brief;
pub mod change;
pub mod context_pack;
pub mod demo;
pub mod dispatch;
pub mod divergence;
pub mod entry;
pub mod evidence;
pub mod feature;
pub mod finding;
pub mod gate;
pub mod ingest;
pub mod inventory;
pub mod learn;
pub mod ledger;
pub mod plugin;
pub mod policy;
pub mod prompt;
pub mod review;
pub mod scenario;
pub mod skills;
pub mod subject;
pub mod tier;
mod top;

use canon_model::RoleId;
use crate::cli_error::CliError;
use std::process::ExitCode;
use crate::cli::{Cli, Command};
use top::*;
use crate::cli::top::*;

pub fn run(cli: Cli) -> ExitCode {
    if matches!(cli.command, Command::Dashboard(_)) {
        entry::set_sigpipe(false);
    }
    match cli.command {
        Command::Policy { action } => policy::run(action),
        Command::Demo { action } => demo::run(action),
        Command::ContextPack { action } => context_pack::run(action),
        Command::Adapter { action } => adapter::run(action),
        Command::Prompt { action } => prompt::run(action),
        Command::Feature { action } => feature::run(action),
        Command::Scenario { action } => scenario::run(action),
        Command::Subject { action } => subject::run(action),
        Command::Change { action } => change::run(action),
        Command::Brief { action } => brief::run(action),
        Command::Inventory { action } => inventory::run(action),
        Command::Gate { action } => gate::run(action),
        Command::Evidence { action } => evidence::run(action),
        Command::Artifacts { action } => artifacts::run(action),
        Command::Finding { action } => finding::run(action),
        Command::Review { action } => review::run(action),
        Command::Divergence { action } => divergence::run(action),
        Command::Ingest { action } => ingest::run(action),
        Command::Learn { action } => learn::run(action),
        Command::Dispatch { action } => dispatch::run(action),
        Command::Plugin { action } => plugin::run(action),
        Command::Skills { action } => skills::run(action),
        Command::Tier { action } => tier::run(action),
        Command::Ledger { action } => ledger::run(action),
        Command::Query(QueryArgs { kind, since, repo, canon_yaml, json, plugin, history, change_id, status, domain, include_sensitive, page, fields }) => {
            run_query(&repo, canon_yaml.as_deref(), kind, since, json, plugin, history, change_id, status, domain, include_sensitive, page.page(), fields)
        }
        Command::Export(ExportArgs { kind, before, after, out, repo, json, include_sensitive, page, fields }) => ExitCode::from(crate::retention::run_export(&repo, kind, before, after, out.as_deref(), json, include_sensitive, &page.page(), fields.as_deref())),
        Command::Purge(PurgeArgs { kind, before, repo, plan, confirm, json }) => {
            let mode = match (plan, confirm) {
                (_, Some(digest)) => Some(crate::retention::PurgeMode::Confirm(digest)),
                (true, None) => Some(crate::retention::PurgeMode::Plan),
                (false, None) => None,
            };
            ExitCode::from(crate::retention::run_purge(&repo, kind, before, mode, json))
        }
        Command::Format(FormatArgs { check: _, root, repo, json }) => run_fmt(&root, repo.as_deref(), json),
        Command::Context(ContextArgs { repo, json, fields }) => run_context(&repo, json, fields),
        Command::Status(StatusArgs { repo, json, fields }) => ExitCode::from(crate::status::run(&repo, json, fields.as_deref()) as u8),
        Command::Init(InitArgs { repo, check_config, no_agents_md, policy, no_policy, providers, language, json }) => {
            let policy = (!no_policy).then(|| policy.unwrap_or(crate::policy_level::PolicyLevel::Strict));
            let skills = match providers {
                Some(list) => crate::init::SkillsChoice::Providers(list),
                None => crate::init::SkillsChoice::Ask,
            };
            run_init(&repo, check_config, crate::init::InitOptions { agents_md: !no_agents_md, policy, skills, language }, json)
        }
        Command::Migrate(MigrateArgs { repo, dry_run, json }) => ExitCode::from(crate::migrate::run(&repo, dry_run, json) as u8),
        Command::Retrieve(RetrieveArgs { role, regime, domain, subject_id, k, repo, json }) => run_retrieve(&repo, &role, regime.as_ref(), domain.as_deref(), subject_id.as_ref(), k, json),
        Command::Report(ReportArgs { repo, check, snapshot, json }) => run_report(&repo, check, snapshot.as_deref(), json),
        Command::Dashboard(DashboardArgs { repo, snapshot, port, json }) => run_dashboard(&repo, snapshot.as_deref(), port, json),
        Command::RegimeKey(RegimeKeyArgs { role, repo, area, hash, json }) => run_regime_key(&role, &repo, &area, &hash, json),
        Command::Selftest(SelftestArgs { json }) => crate::selftest::run_selftest(json),
        Command::Schema(SchemaArgs { path, json, errors, all }) => ExitCode::from(crate::schema::run(&crate::cli::command(), &path, json, errors, all)),
        Command::Completion(CompletionArgs { shell, json }) => run_completion(shell, json),
    }
}

/// The `--role` an `evidence add` / `review add` record carries: the flag,
/// else the declared role when it is `implementer` or `reviewer` (0.21 D3).
/// Any other declared role, or none, leaves the flag required.
fn record_role(flag: Option<RoleId>, command: &str, json: bool) -> Result<RoleId, ExitCode> {
    use crate::role::Role;
    if let Some(role) = flag {
        return Ok(role);
    }
    match Role::current(None).filter(|role| matches!(role, Role::Implementer | Role::Reviewer)) {
        Some(role) => Ok(RoleId::parse(role.as_str()).expect("a role name is a valid role id")),
        None => Err(ExitCode::from(
            CliError::refused("USAGE", "the following required argument was not provided: --role <ROLE>")
                .hint("pass --role, or declare CANON_ROLE=implementer (or reviewer), or --as implementer (or reviewer)")
                .emit(command, json) as u8,
        )),
    }
}
