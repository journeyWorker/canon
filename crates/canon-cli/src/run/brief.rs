//! `canon brief` — the handlers of the group.

use std::collections::BTreeMap;
use std::process::ExitCode;

use crate::cli::brief::{BriefApproveArgs, BriefCheckArgs, BriefCommand, BriefShowArgs};
use crate::cli_error::CliErrorExt as _;

pub fn run(cmd: BriefCommand) -> ExitCode {
    ExitCode::from(match cmd {
        BriefCommand::Show(BriefShowArgs { change_id, body, repo, json }) if body => crate::brief_check::run_body(&repo, change_id.as_ref(), json),
        BriefCommand::Show(BriefShowArgs { change_id, repo, json, .. }) => crate::brief_check::run_next(&repo, change_id.as_ref(), json),
        BriefCommand::Check(BriefCheckArgs { change_id, repo, json }) => crate::brief_check::run_check(&repo, change_id.as_ref(), json),
        BriefCommand::Approve(BriefApproveArgs { approver, note, relayed_via, actor, digests, change_ids, repo, json, .. }) => {
            let ids = change_ids.iter().map(|c| format!(" {c}")).collect::<String>();
            let call = format!("canon brief approve{ids} --approver <their name> --note \"<their own words>\"");
            let shown = change_ids.iter().map(|c| format!(" --digest {c}=<its shown digest>")).collect::<String>();
            let relay_call = format!("canon brief approve{ids} --approver <their name> --note \"<their own words>\" --relayed-via <the question tool> --actor <your actor id>{shown}");
            let root = crate::context::resolve_repo_root(&repo);
            let call_args = crate::relay::Call { via: relayed_via.as_deref(), actor: actor.as_deref(), approver: Some(&approver), note: Some(&note) };
            let relay = match crate::relay::authorize("canon brief approve", &call, &relay_call, &root, call_args) {
                Ok(relay) => relay,
                Err(e) => return ExitCode::from(e.emit("canon brief approve", json)),
            };
            // The entries were checked by `validators::check` before this ran.
            let shown_digests: BTreeMap<String, String> = crate::brief_approve::shown_digests(&digests).unwrap_or_default();
            crate::brief_approve::run_approve(&repo, &change_ids, &approver, &note, relay.as_ref(), &shown_digests, json)
        }
    })
}
