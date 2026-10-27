//! `canon evidence` — the handlers of the group.

use crate::cli::evidence::{EvidenceCommand, EvidenceApprovalPayloadArgs, EvidenceAddArgs, EvidenceAuditArgs};
use crate::cli_error::CliError;
use canon_model::ProjectId;
use std::path::Path;
use std::process::ExitCode;
use super::record_role;

/// `--project` of a leaf that needs a spec root (D1): the one given, else
/// the only root `canon.yaml` configures.
fn project_for(repo: &Path, given: Option<ProjectId>, command: &str, json: bool) -> Result<ProjectId, ExitCode> {
    let repo = crate::context::resolve_repo_root(repo);
    crate::inventory::resolve_project_id(&repo, given.as_ref(), "EVIDENCE_INVALID_ARGUMENT", command, json).map_err(ExitCode::from)
}

pub fn run(cmd: EvidenceCommand) -> ExitCode {
    match cmd {
        EvidenceCommand::ApprovalPayload(EvidenceApprovalPayloadArgs {
            task_id, project_id, scenario_id, run_id, rev, surface_ref,
            approver, approval_at, repo, json,
        }) => {
            let project_id = match (&scenario_id, project_id) {
                (Some(_), given) => match project_for(&repo, given, "canon evidence approval-payload", json) {
                    Ok(id) => Some(id),
                    Err(code) => return code,
                },
                (None, given) => given,
            };
            let commit = match crate::git_rev::bind(&repo, &rev.rev) {
                Ok(commit) => commit,
                Err(error) => return ExitCode::from(error.emit("canon evidence approval-payload", json)),
            };
            ExitCode::from(crate::evidence::run_approval_payload(
                &repo,
                &crate::evidence::ApprovalPayloadArgs {
                    task_id, scenario_id, project_id, run_id, artifact_sha: commit.sha,
                    surface_ref, approval_by: approver, approval_at, json,
                },
            ))
        }
        EvidenceCommand::Add(EvidenceAddArgs {
            task_id,
            project_id,
            class,
            evidence_ref,
            verdict,
            summary,
            command_result,
            scenario_id,
            run_id,
            surface_ref,
            actor_id,
            role,
            session_id,
            approver,
            approval_role,
            signature_file,
            approval_at,
            rev,
            artifacts,
            reports,
            report_cases,
            all_scenarios,
            max_artifact_mib,
            override_reason,
            promote,
            repo,
            json,
            ..
        }) => {
            let kind = class.unwrap_or_else(|| "test-run".to_string());
            let evidence_ref = evidence_ref.unwrap_or_else(|| reports.iter().map(|(_, path)| path.display().to_string()).collect::<Vec<_>>().join(","));
            if kind == crate::evidence::AUDIT_KIND {
                return ExitCode::from(
                    CliError::refused("EVIDENCE_INVALID_ARGUMENT", "`--class audit` is not an `evidence add` class: an external evaluator's re-audit is its own command")
                        .field("--class")
                        .hint("use `canon evidence audit <SCENARIO_ID> --ref <REF> --evaluator <ID> --evaluator-version <TEXT> --evaluator-digest <SHA256> --audit-result <confirmed|refuted|indeterminate>`")
                        .emit("canon evidence add", json),
                );
            }
            let project_id = if scenario_id.is_some() || all_scenarios {
                match project_for(&repo, project_id, "canon evidence add", json) {
                    Ok(id) => Some(id),
                    Err(code) => return code,
                }
            } else {
                project_id
            };
            let role = match record_role(role, &repo, "canon evidence add", json) {
                Ok(role) => role,
                Err(code) => return code,
            };
            let commit = match crate::git_rev::bind(&repo, &rev.rev) {
                Ok(commit) => commit,
                Err(error) => return ExitCode::from(error.emit("canon evidence add", json)),
            };
            crate::inventory::refresh_stale_index(&repo, "canon evidence add");
            ExitCode::from(
                crate::evidence::run_add(
                    &repo,
                    &crate::evidence::EvidenceArgs {
                        task_id,
                        project_id,
                        kind,
                        evidence_ref,
                        verdict,
                        summary,
                        command_result,
                        scenario_id,
                        run_id,
                        surface_ref,
                        actor_id,
                        role,
                        session_id,
                        approval_by: approver,
                        approval_role,
                        approval_signature_file: signature_file,
                        approval_at,
                        commit,
                        artifacts,
                        reports,
                        report_cases,
                        all_scenarios,
                        max_artifact_mib,
                        evaluator_id: None,
                        evaluator_version: None,
                        evaluator_digest: None,
                        audit_result: None,
                        override_reason,
                        promote: promote.flag(),
                        json,
                    },
                ),
            )
        }
        EvidenceCommand::Audit(EvidenceAuditArgs {
            scenario_id,
            project_id,
            evidence_ref,
            summary,
            command_result,
            run_id,
            actor_id,
            role,
            session_id,
            rev,
            artifacts,
            reports,
            report_cases,
            max_artifact_mib,
            evaluator_id,
            evaluator_version,
            evaluator_digest,
            audit_result,
            override_reason,
            promote,
            repo,
            json,
            ..
        }) => {
            let project_id = match project_for(&repo, project_id, "canon evidence audit", json) {
                Ok(id) => Some(id),
                Err(code) => return code,
            };
            let role = match record_role(role, &repo, "canon evidence audit", json) {
                Ok(role) => role,
                Err(code) => return code,
            };
            let commit = match crate::git_rev::bind(&repo, &rev.rev) {
                Ok(commit) => commit,
                Err(error) => return ExitCode::from(error.emit("canon evidence audit", json)),
            };
            crate::inventory::refresh_stale_index(&repo, "canon evidence audit");
            ExitCode::from(
                crate::evidence::run_add(
                    &repo,
                    &crate::evidence::EvidenceArgs {
                        task_id: None,
                        project_id,
                        kind: crate::evidence::AUDIT_KIND.to_string(),
                        evidence_ref,
                        verdict: None,
                        summary,
                        command_result,
                        scenario_id: Some(scenario_id),
                        run_id,
                        surface_ref: Vec::new(),
                        actor_id,
                        role,
                        session_id,
                        approval_by: None,
                        approval_role: None,
                        approval_signature_file: None,
                        approval_at: None,
                        commit,
                        artifacts,
                        reports,
                        report_cases,
                        all_scenarios: false,
                        max_artifact_mib,
                        evaluator_id: Some(evaluator_id),
                        evaluator_version: Some(evaluator_version),
                        evaluator_digest: Some(evaluator_digest),
                        audit_result: Some(audit_result),
                        override_reason,
                        promote: promote.flag(),
                        json,
                    },
                ),
            )
        }
    }
}
