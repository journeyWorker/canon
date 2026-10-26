//! `canon prompt` — the handlers of the group.

use crate::cli::prompt::{PromptCommand, PromptShowArgs, PromptAddArgs};
use std::process::ExitCode;

pub fn run(cmd: PromptCommand) -> ExitCode {
    match cmd {
        PromptCommand::Add(args) => run_prompt_add(args),
        PromptCommand::Show(PromptShowArgs { bundle, repo, json }) => run_prompt_show(&repo, &bundle.name, &bundle.version, json),
    }
}

fn run_prompt_add(args: PromptAddArgs) -> ExitCode {
    let PromptAddArgs { bundle, manifest, repo, json, .. } = args;
    let repo = crate::context::resolve_repo_root(&repo);
    match crate::context_pack::register_prompt_bundle_from_manifest(&repo, &bundle.name, &bundle.version, &manifest) {
        Ok(bundle) => {
            if json {
                crate::json_out::data("prompt add", &bundle);
            } else {
                println!("prompt bundle {}@{} registered ({})", bundle.name, bundle.version, bundle.digest);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            ExitCode::from(error.to_cli_error().emit("canon prompt add", json) as u8)
        }
    }
}

fn run_prompt_show(repo: &std::path::Path, name: &str, version: &str, json: bool) -> ExitCode {
    let repo = crate::context::resolve_repo_root(repo);
    match crate::context_pack::select_prompt_bundle(&repo, name, version) {
        Ok(bundle) => {
            if json {
                crate::json_out::data("prompt show", &bundle);
            } else {
                println!("prompt bundle {}@{} ({})", bundle.name, bundle.version, bundle.digest);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            ExitCode::from(error.to_cli_error().emit("canon prompt show", json) as u8)
        }
    }
}
