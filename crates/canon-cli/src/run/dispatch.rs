//! `canon dispatch` — the handlers of the group.

use crate::cli::dispatch::*;
use std::process::ExitCode;

pub fn run(cmd: DispatchCommand) -> ExitCode {
    match cmd {
        DispatchCommand::Begin(DispatchBeginArgs { role, regime, actor_id, provider, model, skill_id, skill_digest, manifest, prompt_bundle, task_id, parent_run_id, repo, json, .. }) => {
            let role = match super::record_role(role, &repo, "canon dispatch begin", json) {
                Ok(role) => role,
                Err(code) => return code,
            };
            let binding = crate::dispatch::DispatchBinding { task_id, parent_run_id };
            let metadata = crate::dispatch::DispatchMetadata {
                provider,
                model,
                skill_id,
                skill_digest,
                context_manifest: manifest,
                prompt_bundle,
            };
            crate::dispatch::run_begin(&repo, &role, &regime, &actor_id, &binding, &metadata, json)
        }
        DispatchCommand::End(DispatchEndArgs { run_id, run_outcome, repo, json, .. }) => crate::dispatch::run_end(&repo, run_id, run_outcome, json),
        DispatchCommand::Diff(DispatchDiffArgs { repo, json }) => crate::dispatch::run_diff(&repo, json),
    }
}
