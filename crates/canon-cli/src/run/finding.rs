//! `canon finding` — the handlers of the group.

use crate::cli::finding::*;
use std::process::ExitCode;

pub fn run(cmd: FindingCommand) -> ExitCode {
    match cmd {
        FindingCommand::Add(FindingAddArgs {
            change_id,
            round,
            seq,
            severity,
            disposition,
            reviewer,
            summary,
            reviewed_sha,
            resolution_sha,
            introduced_by,
            file_ref,
            scenario_ids,
            actor_id,
            role,
            session_id,
            promote,
            repo,
            json,
        }) => ExitCode::from(crate::finding::run_add(
            &repo,
            &crate::finding::FindingArgs {
                change_id,
                reviewed_sha,
                round,
                seq,
                severity,
                disposition,
                reviewer,
                summary,
                resolution_sha,
                introduced_by,
                file_ref,
                scenario_ids,
                actor_id,
                role,
                session_id,
                promote: promote.flag(),
                json,
            },
        ) as u8),
        FindingCommand::Close(FindingCloseArgs { change_id, round, seq, disposition, resolution_sha, reason, actor_id, role, session_id, promote, repo, json }) => {
            ExitCode::from(crate::finding::run_close(
                &repo,
                &crate::finding::FindingCloseArgs { change_id, round, seq, disposition, resolution_sha, reason, actor_id, role, session_id, promote: promote.flag(), json },
            ) as u8)
        }
        FindingCommand::List(FindingListArgs { change_id, open, json, repo, page, fields }) => {
            ExitCode::from(crate::finding::run_list(&repo, &crate::finding::FindingListArgs { change_id, open, json, page: page.page(), fields }) as u8)
        }
    }
}
