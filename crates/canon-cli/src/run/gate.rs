//! `canon gate` — the handlers of the group.

use crate::cli::gate::*;
use std::process::ExitCode;

pub fn run(cmd: GateCommand) -> ExitCode {
    match cmd {
        GateCommand::Check(GateCheckArgs { repo, release, json }) => ExitCode::from(crate::gate::run_check(&repo, release, json) as u8),
        GateCommand::Task(GateTaskArgs { task_id, repo, json }) => ExitCode::from(crate::gate::run_task(&repo, &task_id, json) as u8),
        GateCommand::Promote(GatePromoteArgs { repo, dry_run, json }) => ExitCode::from(crate::gate::run_promote(&repo, dry_run, json) as u8),
        GateCommand::InstallHooks(GateInstallHooksArgs { repo, event, matcher, command, timeout, json }) => {
            ExitCode::from(crate::gate::run_install_hooks(&repo, &event, matcher.as_deref(), &command, timeout, json) as u8)
        }
        GateCommand::Selftest(GateSelftestArgs { json }) => ExitCode::from(crate::gate::run_selftest(json) as u8),
    }
}
