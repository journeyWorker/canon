//! `canon ledger` — the handlers of the group.

use crate::cli::ledger::*;
use std::process::ExitCode;

pub fn run(cmd: LedgerCommand) -> ExitCode {
    match cmd {
        LedgerCommand::Compact(LedgerCompactArgs { dry_run, repo, json }) => ExitCode::from(crate::ledger::run_compact(&repo, dry_run, json) as u8),
    }
}
