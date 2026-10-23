//! `canon artifacts` — the handlers of the group.

use crate::cli::artifacts::*;
use std::process::ExitCode;

pub fn run(cmd: ArtifactsCommand) -> ExitCode {
    match cmd {
        ArtifactsCommand::Verify(ArtifactsVerifyArgs { full, repo, json }) => ExitCode::from(crate::artifacts::run_verify(&repo, full, json) as u8),
        ArtifactsCommand::Migrate(ArtifactsMigrateArgs { to, dry_run, repo, json }) => ExitCode::from(crate::artifacts::run_migrate(&repo, &to, dry_run, json) as u8),
    }
}
