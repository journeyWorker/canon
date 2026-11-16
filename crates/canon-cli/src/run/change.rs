//! `canon change` — the handlers of the group.

use crate::change_state::{Call, Leaf};
use crate::cli::change::{ChangeCloseArgs, ChangeCommand, ChangeNewArgs, ChangeReopenArgs};
use std::process::ExitCode;

pub fn run(cmd: ChangeCommand) -> ExitCode {
    match cmd {
        ChangeCommand::New(ChangeNewArgs { slug, subject_id, change_kind, title, repo, json, .. }) => ExitCode::from(crate::change::run_new(&repo, &slug, &subject_id, change_kind, &title, json)),
        ChangeCommand::Defer(args) => close(Leaf::Defer, &args),
        ChangeCommand::Drop(args) => close(Leaf::Drop, &args),
        ChangeCommand::Reopen(ChangeReopenArgs { change_id, reason, refs, approver, relayed_via, actor, repo, dry_run, json, .. }) => ExitCode::from(crate::change_state::run(
            &repo,
            Leaf::Reopen,
            &Call { change: &change_id, reason: &reason, refs: &refs, approver: &approver, relayed_via: relayed_via.as_deref(), actor: actor.as_deref(), dry_run: dry_run.dry_run, json },
        )),
    }
}

/// `canon change defer` and `canon change drop`: the same call with another state.
fn close(leaf: Leaf, args: &ChangeCloseArgs) -> ExitCode {
    let ChangeCloseArgs { change_id, reason, refs, approver, relayed_via, actor, repo, dry_run, json, .. } = args;
    ExitCode::from(crate::change_state::run(
        repo,
        leaf,
        &Call { change: change_id, reason, refs, approver, relayed_via: relayed_via.as_deref(), actor: actor.as_deref(), dry_run: dry_run.dry_run, json: *json },
    ))
}
