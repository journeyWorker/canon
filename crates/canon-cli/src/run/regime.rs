//! `canon regime` — the handlers of the group.

use crate::cli::regime::*;
use std::process::ExitCode;

pub fn run(cmd: RegimeCommand) -> ExitCode {
    match cmd {
        RegimeCommand::Show(RegimeShowArgs { role_segment, repo_name, area, hash, json }) => super::run_regime_show(&role_segment, &repo_name, &area, &hash, json),
    }
}
