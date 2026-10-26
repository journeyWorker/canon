//! `canon gate` — the handlers of the group.

use crate::cli::gate::{GateCommand, GateCheckArgs, GatePromoteArgs, PromoteKind, GateInstallHooksArgs};
use std::process::ExitCode;

pub fn run(cmd: GateCommand) -> ExitCode {
    match cmd {
        GateCommand::Check(GateCheckArgs { repo, release, json }) => ExitCode::from(crate::gate::run_check(&repo, release, json)),
        GateCommand::Promote(GatePromoteArgs { repo, kind, dry_run, json, .. }) => ExitCode::from(crate::gate::run_promote(&repo, kind.map(PromoteKind::record_kind), dry_run.dry_run, json)),
        GateCommand::InstallHooks(GateInstallHooksArgs { repo, event, matcher, command, timeout, dry_run, json, .. }) => {
            ExitCode::from(crate::gate::run_install_hooks(&repo, &event, matcher.as_deref(), &command, timeout, dry_run.dry_run, json))
        }
    }
}
