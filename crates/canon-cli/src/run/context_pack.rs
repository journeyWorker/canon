//! `canon context-pack` — the handlers of the group.

use crate::cli::context_pack::*;
use std::process::ExitCode;

pub fn run(cmd: ContextPackCommand) -> ExitCode {
    match cmd {
        ContextPackCommand::New(args) => run_context_pack_new(args),
        ContextPackCommand::Show(ContextPackShowArgs { id, repo, json }) => run_context_pack_show(&repo, &id, json),
        ContextPackCommand::Verify(ContextPackVerifyArgs { id, repo, json }) => run_context_pack_verify(&repo, &id, json),
    }
}

fn run_context_pack_new(args: ContextPackNewArgs) -> ExitCode {
    let ContextPackNewArgs { manifest, repo, json } = args;
    let repo = crate::context::resolve_repo_root(&repo);
    match crate::context_pack::create_from_manifest(&repo, &manifest) {
        Ok(pack) => {
            if json {
                crate::json_out::data("context-pack new", &crate::json_out::untyped(serde_json::to_value(&pack).expect("context pack is serializable")));
            } else {
                println!("{}", pack.id);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            ExitCode::from(error.to_cli_error().emit("canon context-pack new", json) as u8)
        }
    }
}

fn run_context_pack_show(repo: &std::path::Path, id: &str, json: bool) -> ExitCode {
    let repo = crate::context::resolve_repo_root(repo);
    match crate::context_pack::show(&repo, id) {
        Ok(pack) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&pack).expect("context pack is serializable"));
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
                println!("{}", serde_json::json!({ "id": id, "verified": true }));
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
