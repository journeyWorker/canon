//! `canon evidence` — the handlers of the group.

use crate::cli::evidence::*;
use std::process::ExitCode;
use super::record_role;

pub fn run(cmd: EvidenceCommand) -> ExitCode {
    match cmd {
        EvidenceCommand::ApprovalPayload(EvidenceApprovalPayloadArgs {
            task, project_id, scenario_id, run_id, artifact_sha, surface_ref,
            approval_by, approval_at, repo, json,
        }) => ExitCode::from(crate::evidence::run_approval_payload(
            &repo,
            &crate::evidence::ApprovalPayloadArgs {
                task_id: task, scenario_id, project_id, run_id, artifact_sha,
                surface_ref, approval_by, approval_at, json,
            },
        ) as u8),
        EvidenceCommand::Add(EvidenceAddArgs {
            task,
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
            approval_by,
            approval_role,
            approval_signature_file,
            approval_at,
            artifact_sha,
            artifacts,
            reports,
            report_cases,
            all_scenarios,
            max_artifact_mib,
            evaluator_id,
            evaluator_version,
            evaluator_digest,
            audit_result,
            override_reason,
            promote,
            repo,
            json,
        }) => {
            let role = match record_role(role, "canon evidence add", json) {
                Ok(role) => role,
                Err(code) => return code,
            };
            crate::inventory::refresh_stale_index(&repo, "canon evidence add");
            ExitCode::from(
                crate::evidence::run_add(
                    &repo,
                    &crate::evidence::EvidenceArgs {
                        task_id: task,
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
                        approval_by,
                        approval_role,
                        approval_signature_file,
                        approval_at,
                        artifact_sha,
                        artifacts,
                        reports,
                        report_cases,
                        all_scenarios,
                        max_artifact_mib,
                        evaluator_id,
                        evaluator_version,
                        evaluator_digest,
                        audit_result,
                        override_reason,
                        promote: promote.flag(),
                        json,
                    },
                ) as u8,
            )
        }
        EvidenceCommand::Vault(EvidenceVaultArgs { max_artifact_mib, repo, json }) => {
            crate::inventory::refresh_stale_index(&repo, "canon evidence vault");
            ExitCode::from(crate::evidence::run_vault(&repo, max_artifact_mib, json) as u8)
        }
    }
}
