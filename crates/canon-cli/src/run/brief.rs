//! `canon brief` — the handlers of the group.

use std::process::ExitCode;

use crate::cli::brief::*;

pub fn run(cmd: BriefCommand) -> ExitCode {
    ExitCode::from(match cmd {
        BriefCommand::Show(BriefShowArgs { change_id, repo, json }) => crate::brief_check::run_next(&repo, change_id.as_ref(), json),
        BriefCommand::Check(BriefCheckArgs { change_id, repo, json }) => crate::brief_check::run_check(&repo, change_id.as_ref(), json),
        BriefCommand::Approve(BriefApproveArgs { approver, note, change_id, repo, json }) => {
            if let Err(code) = super::human_only("canon brief approve", "canon brief approve [<CHANGE_ID>] --approver <NAME> --note \"<your own words>\"", json) {
                return code;
            }
            crate::brief_approve::run_approve(&repo, change_id.as_ref(), &approver, &note, json)
        }
    } as u8)
}
