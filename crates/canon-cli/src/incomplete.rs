//! What every `<SUBJECT>_INCOMPLETE` answer leads with (spec 8.3.2, `canon-0-36-codes-incomplete`): a command that stopped after an
//! earlier step made its effects puts a command that READS what was done first in `meta.next`, so the caller reads before it repeats.

use crate::status::{NextStep, StepActor};

/// Puts `command` (a command of effect `read`) first in `meta.next`, with `why` saying what it shows.
pub(crate) fn read_first(command: &str, why: &str) {
    crate::json_out::prepend_next(NextStep {
        command: command.to_string(),
        why: why.to_string(),
        actor: StepActor::Agent,
        role: Some(crate::role::Role::Orchestrator),
    });
}

/// [`read_first`] with `canon status --json`, the read that shows where the repo stands after the stopped command.
pub(crate) fn read_status(why: &str) {
    read_first("canon status --json", why);
}

/// Debug builds only, the tests' seam (like `CANON_TEST_BLOB_RACE_AFTER`): `CANON_TEST_STOP_AT=<step>` makes the named step of a command that works
/// in steps fail with an I/O error after the steps before it made their real effects, as a disk or a store that gives out part-way would. The
/// steps are `hooks-second-file`, `hooks-script`, `artifacts-store-second`, `artifacts-prune-after-delete`, `artifacts-prune-object-after-delete`, `promote-staged-corrupt`, `purge-after-delete`, `ledger-compact-after-move`, `tier-age-after-move`,
/// `plugin-second-candidate`, `plugin-every-candidate`, `purge-before-delete`, `inventory-second-root` and `migrate-pins`. A release build has no seam: the result is always `None`.
pub(crate) fn injected_stop(step: &str) -> Option<std::io::Error> {
    #[cfg(debug_assertions)]
    {
        if std::env::var("CANON_TEST_STOP_AT").is_ok_and(|wanted| wanted == step) {
            return Some(std::io::Error::other(format!(
                "the test asked for a stop at `{step}`"
            )));
        }
    }
    let _ = step;
    None
}
