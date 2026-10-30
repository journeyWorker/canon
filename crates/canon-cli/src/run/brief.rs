//! `canon brief` — the handlers of the group.

use std::collections::BTreeMap;
use std::process::ExitCode;

use crate::cli::brief::{BriefApproveArgs, BriefCheckArgs, BriefCommand, BriefShowArgs};
use crate::cli_error::CliError;

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
            let mut shown_digests: BTreeMap<String, String> = BTreeMap::new();
            for entry in &digests {
                let Some((change, digest)) = entry.split_once('=').filter(|(c, d)| !c.is_empty() && d.starts_with("sha256:")) else {
                    return ExitCode::from(CliError::refused("USAGE", format!("--digest takes `<change>=<sha256:digest>`, got `{entry}`")).field("--digest").hint("copy the change id and the `digest:` line `canon brief show <change> --body` printed").emit("canon brief approve", json));
                };
                if let Some(earlier) = shown_digests.insert(change.to_string(), digest.to_string()).filter(|earlier| earlier != digest) {
                    return ExitCode::from(CliError::refused("USAGE", format!("--digest names `{change}` twice with different digests ({earlier} and {digest})")).field("--digest").hint("pass one digest per change").emit("canon brief approve", json));
                }
            }
            crate::brief_approve::run_approve(&repo, &change_ids, &approver, &note, relay.as_ref(), &shown_digests, json)
        }
    })
}
