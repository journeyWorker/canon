//! `canon policy` — the handlers of the group.

use crate::cli_error::CliErrorExt as _;
use crate::cli::policy::{PolicyApproveArgs, PolicyCommand, PolicyShowArgs};
use std::process::ExitCode;

pub fn run(cmd: PolicyCommand) -> ExitCode {
    match cmd {
        PolicyCommand::Show(PolicyShowArgs { repo, json }) => ExitCode::from(crate::policy_approval::run_show(&repo, json)),
        PolicyCommand::Approve(PolicyApproveArgs { approver, note, relayed_via, actor, repo, json, .. }) => {
            let root = crate::context::resolve_repo_root(&repo);
            let call = crate::relay::Call { via: relayed_via.as_deref(), actor: actor.as_deref(), approver: Some(&approver), note: note.as_deref() };
            let relay = match crate::relay::authorize(
                "canon policy approve",
                "canon policy approve --approver <their name> --note \"<what they reviewed>\"",
                "canon policy approve --approver <their name> --note \"<their own words>\" --relayed-via <the question tool> --actor <your actor id>",
                &root,
                call,
            ) {
                Ok(relay) => relay,
                Err(e) => return ExitCode::from(e.emit("canon policy approve", json)),
            };
            ExitCode::from(crate::policy_approval::run_approve(&repo, &approver, note.as_deref(), relay.as_ref(), json))
        }
    }
}
