//! `canon divergence` — the handlers of the group.

use crate::cli::divergence::*;
use crate::cli_error::CliError;
use canon_model::ProjectId;
use std::path::Path;
use std::process::ExitCode;
use super::record_role;

/// `--project-id` of a divergence leaf (D1): the one given, else the only
/// spec root `canon.yaml` configures.
fn project_for(repo: &Path, given: Option<ProjectId>, command: &str, json: bool) -> Result<ProjectId, ExitCode> {
    let repo = crate::context::resolve_repo_root(repo);
    crate::inventory::resolve_project_id(&repo, given.as_ref(), "DIVERGENCE_INVALID_ARGUMENT", command, json).map_err(|code| ExitCode::from(code as u8))
}

pub fn run(cmd: DivergenceCliCommand) -> ExitCode {
    match cmd {
        DivergenceCliCommand::Add(args) => run_divergence_add(args),
        DivergenceCliCommand::Promote(DivergencePromoteArgs { repo, dry_run, json }) => ExitCode::from(crate::divergence::run_promote(&repo, dry_run.dry_run, json) as u8),
        DivergenceCliCommand::Resolve(DivergenceResolveArgs { project_id, scenario_id, sha, round, reviewer, detail, actor_id, role, repo, json }) => {
            let project_id = match project_for(&repo, project_id, "canon divergence resolve", json) {
                Ok(id) => id,
                Err(code) => return code,
            };
            ExitCode::from(crate::divergence::run_resolve(&repo, &project_id, &scenario_id, &sha, round, &reviewer, &detail, &actor_id, &role, json) as u8)
        }
        DivergenceCliCommand::Defer(DivergenceDeferArgs { project_id, scenario_id, sha, round, reviewer, reason, expiry, actor_id, role, repo, json }) => {
            let project_id = match project_for(&repo, project_id, "canon divergence defer", json) {
                Ok(id) => id,
                Err(code) => return code,
            };
            ExitCode::from(crate::divergence::run_defer(&repo, &project_id, &scenario_id, &sha, round, &reviewer, &reason, expiry, &actor_id, &role, json) as u8)
        }
        DivergenceCliCommand::Accept(DivergenceAcceptArgs { project_id, scenario_ids, reason, tracking, expiry, reviewer, actor_id, role, session_id, repo, json }) => {
            let project_id = match project_for(&repo, project_id, "canon divergence accept", json) {
                Ok(id) => id,
                Err(code) => return code,
            };
            let role = match record_role(role, "canon divergence accept", json) {
                Ok(role) => role,
                Err(code) => return code,
            };
            crate::inventory::refresh_stale_index(&repo, "canon divergence accept");
            ExitCode::from(crate::divergence::run_accept(&repo, &crate::divergence::AcceptArgs { project_id, scenario_ids, reason, tracking, expiry, reviewer, actor_id, role, session_id, json }) as u8)
        }
        DivergenceCliCommand::Status(DivergenceStatusArgs { repo, as_of, json, page, fields }) => ExitCode::from(crate::divergence::run_status(&repo, as_of, json, &page.page(), fields.as_deref()) as u8),
    }
}

/// `canon divergence add`.
fn run_divergence_add(args: DivergenceAddArgs) -> ExitCode {
    let DivergenceAddArgs { project_id, scenario_id, sha, status, reason, expiry, round, reviewer, detail, actor_id, role, promote, repo, json } = args;
    let project_id = match project_for(&repo, project_id, "canon divergence add", json) {
        Ok(id) => id,
        Err(code) => return code,
    };
    match crate::divergence::parse_status(&status, reason.as_deref(), expiry) {
        Ok(status) => ExitCode::from(crate::divergence::run_stage(&repo, &project_id, &scenario_id, &sha, status, round, &reviewer, &detail, &actor_id, &role, promote.flag(), json) as u8),
        Err(e) => ExitCode::from(CliError::refused("DIVERGENCE_INVALID_ARGUMENT", e.to_string()).hint("`canon schema divergence add` lists the accepted arguments and their forms").emit("canon divergence add", json) as u8),
    }
}
