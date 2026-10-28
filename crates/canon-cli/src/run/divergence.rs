//! `canon divergence` — the handlers of the group.

use crate::cli::divergence::{DivergenceCliCommand, DivergenceResolveArgs, DivergenceDeferArgs, DivergenceAcceptArgs, DivergenceListArgs, DivergenceAddArgs};
use crate::cli_error::CliError;
use canon_model::ProjectId;
use std::path::Path;
use std::process::ExitCode;
use super::{human_only, record_role};
use canon_model::RoleId;

/// `--project` of a divergence leaf (D1): the one given, else the only
/// spec root `canon.yaml` configures.
fn project_for(repo: &Path, given: Option<ProjectId>, command: &str, json: bool) -> Result<ProjectId, ExitCode> {
    let repo = crate::context::resolve_repo_root(repo);
    crate::inventory::resolve_project_id(&repo, given.as_ref(), "DIVERGENCE_INVALID_ARGUMENT", command, json).map_err(ExitCode::from)
}

pub fn run(cmd: DivergenceCliCommand) -> ExitCode {
    match cmd {
        DivergenceCliCommand::Add(args) => run_divergence_add(args),
        DivergenceCliCommand::Resolve(DivergenceResolveArgs { project_id, scenario_id, rev, round, detail, actor_id, session, role, repo, json, .. }) => {
            let role = match record_role(role, &repo, "canon divergence resolve", json) {
                Ok(role) => role,
                Err(code) => return code,
            };
            let project_id = match project_for(&repo, project_id, "canon divergence resolve", json) {
                Ok(id) => id,
                Err(code) => return code,
            };
            let commit = match crate::git_rev::bind(&repo, &rev.rev) {
                Ok(commit) => commit,
                Err(error) => return ExitCode::from(error.emit("canon divergence resolve", json)),
            };
            ExitCode::from(crate::divergence::run_resolve(crate::divergence::CommitArgs { repo: &repo, project_id: &project_id, scenario_id: &scenario_id, commit: &commit, round, reviewer: &actor_id, detail: detail.as_deref().unwrap_or_default(), actor_id: &actor_id, role: &role, session: session.session_id.as_ref(), json }))
        }
        DivergenceCliCommand::Defer(DivergenceDeferArgs { project_id, scenario_id, rev, round, reason, expiry, actor_id, session, role, repo, json, .. }) => {
            let role = match record_role(role, &repo, "canon divergence defer", json) {
                Ok(role) => role,
                Err(code) => return code,
            };
            let project_id = match project_for(&repo, project_id, "canon divergence defer", json) {
                Ok(id) => id,
                Err(code) => return code,
            };
            let commit = match crate::git_rev::bind(&repo, &rev.rev) {
                Ok(commit) => commit,
                Err(error) => return ExitCode::from(error.emit("canon divergence defer", json)),
            };
            ExitCode::from(crate::divergence::run_defer(crate::divergence::CommitArgs { repo: &repo, project_id: &project_id, scenario_id: &scenario_id, commit: &commit, round, reviewer: &actor_id, detail: "", actor_id: &actor_id, role: &role, session: session.session_id.as_ref(), json }, &reason, expiry))
        }
        DivergenceCliCommand::Accept(DivergenceAcceptArgs { project_id, scenario_ids, reason, tracking, expiry, actor_id, role, session_id, repo, json, .. }) => {
            let ids: Vec<String> = scenario_ids.iter().map(ToString::to_string).collect();
            let call = format!("canon divergence accept {} --actor <their name> --session <their session> --reason \"<why>\" --tracking <ISSUE_URL>", ids.join(" "));
            if let Err(code) = human_only("canon divergence accept", &call, json) {
                return code;
            }
            let project_id = match project_for(&repo, project_id, "canon divergence accept", json) {
                Ok(id) => id,
                Err(code) => return code,
            };
            // A human accepts as the reviewer: with no declared role the attribution defaults to `reviewer`.
            let role = role.unwrap_or_else(|| RoleId::parse("reviewer").expect("a role name is a valid role id"));
            crate::inventory::refresh_stale_index(&repo, "canon divergence accept");
            ExitCode::from(crate::divergence::run_accept(&repo, &crate::divergence::AcceptArgs { project_id, scenario_ids, reason, tracking, expiry, reviewer: actor_id.clone(), actor_id, role, session_id, json }))
        }
        DivergenceCliCommand::List(DivergenceListArgs { repo, as_of, json, page, fields }) => ExitCode::from(crate::divergence::run_list(&repo, as_of, json, &page.page(), fields.as_deref())),
    }
}

/// `canon divergence add`.
fn run_divergence_add(args: DivergenceAddArgs) -> ExitCode {
    let DivergenceAddArgs { project_id, scenario_id, rev, state, reason, expiry, round, detail, actor_id, session, role, promote, repo, json, .. } = args;
    let role = match record_role(role, &repo, "canon divergence add", json) {
        Ok(role) => role,
        Err(code) => return code,
    };
    let project_id = match project_for(&repo, project_id, "canon divergence add", json) {
        Ok(id) => id,
        Err(code) => return code,
    };
    let commit = match crate::git_rev::bind(&repo, &rev.rev) {
        Ok(commit) => commit,
        Err(error) => return ExitCode::from(error.emit("canon divergence add", json)),
    };
    match crate::divergence::parse_status(&state, reason.as_deref(), expiry) {
        Ok(status) => ExitCode::from(crate::divergence::run_stage(crate::divergence::CommitArgs { repo: &repo, project_id: &project_id, scenario_id: &scenario_id, commit: &commit, round, reviewer: &actor_id, detail: detail.as_deref().unwrap_or_default(), actor_id: &actor_id, role: &role, session: session.session_id.as_ref(), json }, status, promote.flag())),
        Err(e) => ExitCode::from(CliError::refused("DIVERGENCE_INVALID_ARGUMENT", e.to_string()).hint("`canon schema divergence add` lists the accepted arguments and their forms").emit("canon divergence add", json)),
    }
}
