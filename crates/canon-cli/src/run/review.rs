//! `canon review` — the handlers of the group.

use crate::cli::review::{ReviewCommand, ReviewScenarioArgs, ReviewChangeArgs, ReviewOptions};
use canon_model::{ChangeId, ScenarioId};
use std::process::ExitCode;
use super::record_role;

pub fn run(cmd: ReviewCommand) -> ExitCode {
    match cmd {
        ReviewCommand::Scenario(ReviewScenarioArgs { scenario_ids, review, .. }) => run_review("canon review scenario", scenario_ids, None, review),
        ReviewCommand::Change(ReviewChangeArgs { change_id, review, .. }) => run_review("canon review change", Vec::new(), Some(change_id), review),
    }
}

/// Both leaves: `--actor-id` is the reviewer, so the record's `reviewer` and
/// its actor are the same id.
fn run_review(command: &str, scenario_ids: Vec<ScenarioId>, change: Option<ChangeId>, options: ReviewOptions) -> ExitCode {
    let ReviewOptions {
        project_id,
        actor_id,
        rev,
        upstream_ref,
        original_spec_ref,
        role,
        session_id,
        review_outcome,
        reviewer_provider,
        reviewer_model,
        reviewer_harness,
        artifacts,
        max_artifact_mib,
        repo,
        json,
    } = options;
    let role = match record_role(role, &repo, command, json) {
        Ok(role) => role,
        Err(code) => return code,
    };
    let root = crate::context::resolve_repo_root(&repo);
    let project_id = match crate::inventory::resolve_project_id(&root, project_id.as_ref(), "REVIEW_INVALID_ARGUMENT", command, json) {
        Ok(id) => id,
        Err(code) => return ExitCode::from(code),
    };
    crate::inventory::refresh_stale_index(&repo, command);
    ExitCode::from(crate::review::run_add(
        &repo,
        &crate::review::ReviewAddArgs {
            project_id,
            scenario_ids,
            change,
            reviewer: actor_id.clone(),
            rev: rev.rev,
            upstream_ref,
            original_spec_ref,
            actor_id,
            role,
            session_id,
            outcome: review_outcome,
            reviewer_provider,
            reviewer_model,
            reviewer_harness,
            artifacts,
            max_artifact_mib,
            json,
        },
    ))
}
