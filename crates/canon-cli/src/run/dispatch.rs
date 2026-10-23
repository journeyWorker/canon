//! `canon dispatch` — the handlers of the group.

use crate::cli::dispatch::*;
use std::process::ExitCode;

pub fn run(cmd: DispatchCommand) -> ExitCode {
    match cmd {
        DispatchCommand::Begin(DispatchBeginArgs { role, regime, agent_id, provider, model, skill_id, skill_digest, context_manifest, prompt_bundle, task, parent_run, repo, json }) => {
            let binding = crate::dispatch::DispatchBinding { task_id: task, parent_run_id: parent_run };
            let metadata = crate::dispatch::DispatchMetadata {
                provider,
                model,
                skill_id,
                skill_digest,
                context_manifest,
                prompt_bundle,
            };
            crate::dispatch::run_begin(&repo, &role, &regime, &agent_id, &binding, &metadata, json)
        }
        DispatchCommand::End(DispatchEndArgs { run_id, status, repo, json }) => crate::dispatch::run_end(&repo, run_id, status, json),
        DispatchCommand::Diff(DispatchDiffArgs { repo, json }) => crate::dispatch::run_diff(&repo, json),
    }
}
