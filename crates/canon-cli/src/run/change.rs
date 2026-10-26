//! `canon change` — the handlers of the group.

use crate::cli::change::{ChangeCommand, ChangeNewArgs};
use std::process::ExitCode;

pub fn run(cmd: ChangeCommand) -> ExitCode {
    match cmd {
        ChangeCommand::New(ChangeNewArgs { slug, subject_id, change_kind, title, repo, json, .. }) => ExitCode::from(crate::change::run_new(&repo, &slug, &subject_id, change_kind, &title, json)),
    }
}
