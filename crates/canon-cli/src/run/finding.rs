//! `canon finding` — the handlers of the group.

use crate::cli::finding::{FindingCommand, FindingAddArgs, FindingCloseArgs, FindingListArgs};
use std::process::ExitCode;

pub fn run(cmd: FindingCommand) -> ExitCode {
    match cmd {
        FindingCommand::Add(FindingAddArgs {
            change_id,
            round,
            seq,
            severity,
            disposition,
            summary,
            rev,
            resolution_rev,
            introduced_rev,
            file_ref,
            scenario_ids,
            actor_id,
            role,
            session_id,
            promote,
            repo,
            json,
            ..
        }) => {
            crate::role::warn_if_unknown(&role, &crate::context::resolve_repo_root(&repo));
            ExitCode::from(crate::finding::run_add(
            &repo,
            &crate::finding::FindingArgs {
                change_id,
                rev: rev.rev,
                round,
                seq,
                severity,
                disposition,
                reviewer: actor_id.clone(),
                summary,
                resolution_rev,
                introduced_rev,
                file_ref,
                scenario_ids,
                actor_id,
                role,
                session_id,
                promote: promote.flag(),
                json,
            },
        ) as u8)
        }
        FindingCommand::Close(FindingCloseArgs { change_id, round, seq, disposition, resolution_rev, reason, actor_id, role, session_id, promote, repo, json, .. }) => {
            crate::role::warn_if_unknown(&role, &crate::context::resolve_repo_root(&repo));
            ExitCode::from(crate::finding::run_close(
                &repo,
                &crate::finding::FindingCloseArgs { change_id, round, seq, disposition, resolution_rev, reason, actor_id, role, session_id, promote: promote.flag(), json },
            ) as u8)
        }
        FindingCommand::List(FindingListArgs { change_id, open, json, repo, page, fields }) => {
            ExitCode::from(crate::finding::run_list(&repo, &crate::finding::FindingListArgs { change_id, open, json, page: page.page(), fields }) as u8)
        }
    }
}
