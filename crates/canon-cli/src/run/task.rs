//! `canon task` — the handlers of the group.

use crate::cli::task::*;
use std::process::ExitCode;

pub fn run(cmd: TaskCommand) -> ExitCode {
    match cmd {
        TaskCommand::Done(TaskDoneArgs { task_id, repo, json, .. }) => ExitCode::from(crate::gate::run_task(&repo, &task_id, json) as u8),
    }
}
