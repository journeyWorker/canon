//! `canon subject` — the handlers of the group.

use crate::cli::subject::*;
use std::process::ExitCode;

pub fn run(cmd: SubjectCommand) -> ExitCode {
    match cmd {
        SubjectCommand::New(SubjectNewArgs { id, domain, title, summary, owner_role, actor_id, repo, json }) => {
            ExitCode::from(crate::subject::run_new(&repo, &id, &domain, &title, &summary, &owner_role, &actor_id, json) as u8)
        }
        SubjectCommand::Adopt(SubjectAdoptArgs { change_id, subject_id, repo, dry_run, json }) => ExitCode::from(crate::subject::run_adopt(&repo, &change_id, &subject_id, dry_run.dry_run, json) as u8),
        SubjectCommand::Unadopt(SubjectUnadoptArgs { change_id, subject_id, repo, dry_run, json }) => ExitCode::from(crate::subject::run_unadopt(&repo, &change_id, &subject_id, dry_run.dry_run, json) as u8),
        SubjectCommand::Show(SubjectShowArgs { id, repo, json }) => {
            crate::inventory::refresh_stale_index(&repo, "canon subject show");
            ExitCode::from(crate::subject::run_show(&repo, &id, json) as u8)
        }
        SubjectCommand::Move(SubjectMoveArgs { id, to, override_reason, reason, actor_id, repo, json }) => {
            crate::inventory::refresh_stale_index(&repo, "canon subject move");
            ExitCode::from(crate::subject::run_move(&repo, &id, to, override_reason.as_deref(), reason.as_deref(), &actor_id, json) as u8)
        }
    }
}
