//! `canon policy` — the handlers of the group.

use crate::cli::policy::{PolicyCommand, PolicyShowArgs, PolicyApproveArgs};
use std::process::ExitCode;

pub fn run(cmd: PolicyCommand) -> ExitCode {
    match cmd {
        PolicyCommand::Show(PolicyShowArgs { repo, json }) => ExitCode::from(crate::policy_approval::run_show(&repo, json) as u8),
        PolicyCommand::Approve(PolicyApproveArgs { approver, note, repo, json, .. }) => {
            if let Err(code) = super::human_only("canon policy approve", "canon policy approve --approver <their name> --note \"<what they reviewed>\"", json) {
                return code;
            }
            ExitCode::from(crate::policy_approval::run_approve(&repo, &approver, note.as_deref(), json) as u8)
        }
    }
}
