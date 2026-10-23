//! `canon report` — the handlers of the group.

use crate::cli::report::*;
use std::process::ExitCode;

pub fn run(cmd: ReportCommand) -> ExitCode {
    match cmd {
        ReportCommand::Build(ReportBuildArgs { repo, json, .. }) => super::run_report(&repo, false, None, json),
        ReportCommand::Check(ReportCheckArgs { repo, json }) => super::run_report(&repo, true, None, json),
        ReportCommand::Snapshot(ReportSnapshotArgs { dir, repo, json, .. }) => super::run_report(&repo, false, Some(&dir), json),
    }
}
