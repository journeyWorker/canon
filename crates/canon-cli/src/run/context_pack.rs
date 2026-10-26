//! `canon context-pack` — the handlers of the group.

use crate::cli::context_pack::{ContextPackCommand, ContextPackShowArgs, ContextPackVerifyArgs, ContextPackAddArgs};
use std::process::ExitCode;

pub fn run(cmd: ContextPackCommand) -> ExitCode {
    match cmd {
        ContextPackCommand::Add(args) => run_context_pack_add(args),
        ContextPackCommand::Show(ContextPackShowArgs { id, repo, json }) => run_context_pack_show(&repo, &id, json),
        ContextPackCommand::Verify(ContextPackVerifyArgs { id, repo, json }) => run_context_pack_verify(&repo, &id, json),
    }
}

fn run_context_pack_add(args: ContextPackAddArgs) -> ExitCode {
    let ContextPackAddArgs { manifest, repo, json, .. } = args;
    let repo = crate::context::resolve_repo_root(&repo);
    match crate::context_pack::create_from_manifest(&repo, &manifest) {
        Ok(pack) => {
            if json {
                crate::json_out::data("context-pack add", &pack);
            } else {
                println!("{}", pack.id);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            ExitCode::from(error.to_cli_error().emit("canon context-pack add", json) as u8)
        }
    }
}

fn run_context_pack_show(repo: &std::path::Path, id: &str, json: bool) -> ExitCode {
    let repo = crate::context::resolve_repo_root(repo);
    match crate::context_pack::show(&repo, id) {
        Ok(pack) => {
            if json {
                crate::json_out::data("context-pack show", &pack);
            } else {
                println!("{}", pack.id);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            ExitCode::from(error.to_cli_error().emit("canon context-pack show", json) as u8)
        }
    }
}

fn run_context_pack_verify(repo: &std::path::Path, id: &str, json: bool) -> ExitCode {
    let repo = crate::context::resolve_repo_root(repo);
    match crate::context_pack::verify(&repo, id) {
        Ok(()) => {
            if json {
                crate::json_out::data("context-pack verify", &crate::context_pack::VerifyReport { id: id.to_string(), verified: true });
            } else {
                println!("context pack {id} verified");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            ExitCode::from(error.to_cli_error().emit("canon context-pack verify", json) as u8)
        }
    }
}
