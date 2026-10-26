//! `canon demo` — the handlers of the group.

use crate::cli::demo::{DemoCommand, DemoInitArgs};
use std::process::ExitCode;

pub fn run(cmd: DemoCommand) -> ExitCode {
    match cmd {
        DemoCommand::Init(DemoInitArgs { repo, json, .. }) => ExitCode::from(crate::demo::run_demo_init(&repo, json)),
        DemoCommand::Review(args) => ExitCode::from(crate::demo::run_demo_review(&args.repo, args.json)),
    }
}
