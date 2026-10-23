//! `canon divergence` — the handlers of the group.

use crate::cli::divergence::*;
use crate::cli_error::CliError;
use canon_model::ProjectId;
use std::path::Path;
use std::process::ExitCode;
use super::{human_only, record_role};
use canon_model::RoleId;

/// `--project-id` of a divergence leaf (D1): the one given, else the only
/// spec root `canon.yaml` configures.
fn project_for(repo: &Path, given: Option<ProjectId>, command: &str, json: bool) -> Result<ProjectId, ExitCode> {
    let repo = crate::context::resolve_repo_root(repo);
    crate::inventory::resolve_project_id(&repo, given.as_ref(), "DIVERGENCE_INVALID_ARGUMENT", command, json).map_err(|code| ExitCode::from(code as u8))
}

pub fn run(cmd: DivergenceCliCommand) -> ExitCode {
    match cmd {
        DivergenceCliCommand::Add(args) => run_divergence_add(args),
        DivergenceCliCommand::Resolve(DivergenceResolveArgs { project_id, scenario_id, rev, round, detail, actor_id, role, repo, json, .. }) => {
            let role = match record_role(role, "canon divergence resolve", json) {
                Ok(role) => role,
                Err(code) => return code,
            };
            let project_id = match project_for(&repo, project_id, "canon divergence resolve", json) {
                Ok(id) => id,
                Err(code) => return code,
            };
            let commit = match crate::git_rev::bind(&repo, &rev.rev) {
                Ok(commit) => commit,
                Err(error) => return ExitCode::from(error.emit("canon divergence resolve", json) as u8),
            };
            ExitCode::from(crate::divergence::run_resolve(&repo, &project_id, &scenario_id, &commit, round, &actor_id, &detail, &actor_id, &role, json) as u8)
        }
        DivergenceCliCommand::Defer(DivergenceDeferArgs { project_id, scenario_id, rev, round, reason, expiry, actor_id, role, repo, json, .. }) => {
            let role = match record_role(role, "canon divergence defer", json) {
                Ok(role) => role,
                Err(code) => return code,
            };
            let project_id = match project_for(&repo, project_id, "canon divergence defer", json) {
                Ok(id) => id,
                Err(code) => return code,
            };
            let commit = match crate::git_rev::bind(&repo, &rev.rev) {
                Ok(commit) => commit,
                Err(error) => return ExitCode::from(error.emit("canon divergence defer", json) as u8),
            };
            ExitCode::from(crate::divergence::run_defer(&repo, &project_id, &scenario_id, &commit, round, &actor_id, &reason, expiry, &actor_id, &role, json) as u8)
        }
        DivergenceCliCommand::Accept(DivergenceAcceptArgs { project_id, scenario_ids, reason, tracking, expiry, actor_id, role, session_id, repo, json, .. }) => {
            if let Err(code) = human_only("canon divergence accept", "canon divergence accept <SCENARIO_ID>... --actor-id <NAME> --reason \"<why>\" --tracking <ISSUE_URL>", json) {
                return code;
            }
            let project_id = match project_for(&repo, project_id, "canon divergence accept", json) {
                Ok(id) => id,
                Err(code) => return code,
            };
            // A human accepts as the reviewer: with no declared role the attribution defaults to `reviewer`.
            let role = role.unwrap_or_else(|| RoleId::parse("reviewer").expect("a role name is a valid role id"));
            crate::inventory::refresh_stale_index(&repo, "canon divergence accept");
            ExitCode::from(crate::divergence::run_accept(&repo, &crate::divergence::AcceptArgs { project_id, scenario_ids, reason, tracking, expiry, reviewer: actor_id.clone(), actor_id, role, session_id, json }) as u8)
        }
        DivergenceCliCommand::List(DivergenceListArgs { repo, as_of, json, page, fields }) => ExitCode::from(crate::divergence::run_list(&repo, as_of, json, &page.page(), fields.as_deref()) as u8),
    }
}

/// `canon divergence add`.
fn run_divergence_add(args: DivergenceAddArgs) -> ExitCode {
    let DivergenceAddArgs { project_id, scenario_id, rev, state, reason, expiry, round, detail, actor_id, role, promote, repo, json, .. } = args;
    let role = match record_role(role, "canon divergence add", json) {
        Ok(role) => role,
        Err(code) => return code,
    };
    let project_id = match project_for(&repo, project_id, "canon divergence add", json) {
        Ok(id) => id,
        Err(code) => return code,
    };
    let commit = match crate::git_rev::bind(&repo, &rev.rev) {
        Ok(commit) => commit,
        Err(error) => return ExitCode::from(error.emit("canon divergence add", json) as u8),
    };
    match crate::divergence::parse_status(&state, reason.as_deref(), expiry) {
        Ok(status) => ExitCode::from(crate::divergence::run_stage(&repo, &project_id, &scenario_id, &commit, status, round, &actor_id, &detail, &actor_id, &role, promote.flag(), json) as u8),
        Err(e) => ExitCode::from(CliError::refused("DIVERGENCE_INVALID_ARGUMENT", e.to_string()).hint("`canon schema divergence add` lists the accepted arguments and their forms").emit("canon divergence add", json) as u8),
    }
}
