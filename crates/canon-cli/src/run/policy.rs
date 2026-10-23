//! `canon policy` — the handlers of the group.

use crate::cli::policy::*;
use std::process::ExitCode;

pub fn run(cmd: PolicyCommand) -> ExitCode {
    match cmd {
        PolicyCommand::Show(PolicyShowArgs { repo, json }) => ExitCode::from(crate::policy_approval::run_show(&repo, json) as u8),
        PolicyCommand::Approve(PolicyApproveArgs { by, note, repo, json }) => ExitCode::from(crate::policy_approval::run_approve(&repo, &by, note.as_deref(), json) as u8),
    }
}
