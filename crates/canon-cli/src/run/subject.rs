//! `canon subject` — the handlers of the group.

use crate::cli::subject::{SubjectCommand, SubjectNewArgs, SubjectAdoptArgs, SubjectUnadoptArgs, SubjectShowArgs, SubjectMoveArgs};
use std::process::ExitCode;

pub fn run(cmd: SubjectCommand) -> ExitCode {
    match cmd {
        SubjectCommand::New(SubjectNewArgs { id, domain, title, summary, owner_role, actor_id, session, repo, json, .. }) => {
            ExitCode::from(crate::subject::run_new(&repo, &crate::subject::NewSubject { subject_id: &id, domain: &domain, title: &title, summary: summary.as_deref().unwrap_or_default(), owner_role: &owner_role }, crate::subject::Actor { id: &actor_id, session: session.session_id.as_ref() }, json))
        }
        SubjectCommand::Adopt(SubjectAdoptArgs { change_id, subject_id, repo, dry_run, json, .. }) => ExitCode::from(crate::subject::run_adopt(&repo, &change_id, &subject_id, dry_run.dry_run, json)),
        SubjectCommand::Unadopt(SubjectUnadoptArgs { change_id, subject_id, repo, dry_run, json, .. }) => ExitCode::from(crate::subject::run_unadopt(&repo, &change_id, &subject_id, dry_run.dry_run, json)),
        SubjectCommand::Show(SubjectShowArgs { id, repo, json }) => {
            crate::inventory::refresh_stale_index(&repo, "canon subject show");
            ExitCode::from(crate::subject::run_show(&repo, &id, json))
        }
        SubjectCommand::Move(SubjectMoveArgs { id, to, override_reason, reason, actor_id, session, repo, json, .. }) => {
            crate::inventory::refresh_stale_index(&repo, "canon subject move");
            ExitCode::from(crate::subject::run_move(&repo, &id, to, override_reason.as_deref(), reason.as_deref(), crate::subject::Actor { id: &actor_id, session: session.session_id.as_ref() }, json))
        }
    }
}
