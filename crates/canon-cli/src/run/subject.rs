//! `canon subject` — the handlers of the group.

use crate::cli::subject::*;
use std::process::ExitCode;

pub fn run(cmd: SubjectCommand) -> ExitCode {
    match cmd {
        SubjectCommand::New(SubjectNewArgs { id, domain, title, summary, owner_role, actor_id, repo, json }) => {
            ExitCode::from(crate::subject::run_new(&repo, &id, &domain, &title, &summary, &owner_role, &actor_id, json) as u8)
        }
        SubjectCommand::Adopt(SubjectAdoptArgs { change_id, subject_id, repo, json }) => ExitCode::from(crate::subject::run_adopt(&repo, &change_id, &subject_id, json) as u8),
        SubjectCommand::Unadopt(SubjectUnadoptArgs { change_id, subject_id, repo, json }) => ExitCode::from(crate::subject::run_unadopt(&repo, &change_id, &subject_id, json) as u8),
        SubjectCommand::Status(SubjectStatusArgs { id, to, override_reason, reason, actor_id, repo, json }) => {
            crate::inventory::refresh_stale_index(&repo, "canon subject status");
            ExitCode::from(match to {
                Some(target) => crate::subject::run_status(&repo, &id, target, override_reason.as_deref(), reason.as_deref(), &actor_id, json),
                None => crate::subject::run_show(&repo, &id, override_reason.as_deref(), reason.as_deref(), json),
            } as u8)
        }
    }
}
