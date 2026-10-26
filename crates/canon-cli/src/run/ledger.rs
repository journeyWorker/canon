//! `canon ledger` — the handlers of the group.

use crate::cli::ledger::{LedgerCommand, LedgerCompactArgs};
use std::process::ExitCode;

pub fn run(cmd: LedgerCommand) -> ExitCode {
    match cmd {
        LedgerCommand::Compact(LedgerCompactArgs { safety, repo, json, .. }) => ExitCode::from(crate::ledger::run_compact(&repo, &safety.mode(), json) as u8),
    }
}
