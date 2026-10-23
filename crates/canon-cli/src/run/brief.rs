//! `canon brief` — the handlers of the group.

use std::process::ExitCode;

use crate::cli::brief::*;

pub fn run(cmd: BriefCommand) -> ExitCode {
    ExitCode::from(match cmd {
        BriefCommand::Show(BriefShowArgs { change_id, repo, json }) => crate::brief_check::run_next(&repo, change_id.as_ref(), json),
        BriefCommand::Check(BriefCheckArgs { change_id, repo, json }) => crate::brief_check::run_check(&repo, change_id.as_ref(), json),
        BriefCommand::Approve(BriefApproveArgs { by, note, change_id, repo, json }) => crate::brief_approve::run_approve(&repo, change_id.as_ref(), &by, &note, json),
    } as u8)
}
