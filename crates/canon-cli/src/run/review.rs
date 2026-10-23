//! `canon review` — the handlers of the group.

use crate::cli::review::*;
use std::process::ExitCode;
use super::record_role;

pub fn run(cmd: ReviewCommand) -> ExitCode {
    match cmd {
        ReviewCommand::Add(ReviewAddArgs {
            project_id,
            scenario_ids,
            scenario_ids_from,
            change_id,
            reviewer,
            pin,
            upstream_ref,
            original_spec_ref,
            actor_id,
            role,
            session_id,
            outcome,
            reviewer_provider,
            reviewer_model,
            reviewer_harness,
            artifacts,
            max_artifact_mib,
            repo,
            json,
        }) => {
            let role = match record_role(role, "canon review add", json) {
                Ok(role) => role,
                Err(code) => return code,
            };
            crate::inventory::refresh_stale_index(&repo, "canon review add");
            ExitCode::from(crate::review::run_add(
                &repo,
                &crate::review::ReviewAddArgs {
                    project_id,
                    scenario_ids,
                    scenario_ids_from,
                    change: change_id,
                    reviewer,
                    pin,
                    upstream_ref,
                    original_spec_ref,
                    actor_id,
                    role,
                    session_id,
                    outcome,
                    reviewer_provider,
                    reviewer_model,
                    reviewer_harness,
                    artifacts,
                    max_artifact_mib,
                    json,
                },
            ) as u8)
        }
    }
}
