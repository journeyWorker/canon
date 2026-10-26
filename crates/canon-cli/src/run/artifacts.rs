//! `canon artifacts` — the handlers of the group.

use crate::cli::artifacts::{ArtifactsCommand, ArtifactsVerifyArgs, ArtifactsStoreArgs, ArtifactsMoveArgs};
use std::process::ExitCode;

pub fn run(cmd: ArtifactsCommand) -> ExitCode {
    match cmd {
        ArtifactsCommand::Verify(ArtifactsVerifyArgs { full, repo, json }) => ExitCode::from(crate::artifacts::run_verify(&repo, full, json)),
        ArtifactsCommand::Store(ArtifactsStoreArgs { max_artifact_mib, dry_run, repo, json, .. }) => {
            // A preview reads only: it must not refresh the index first.
            if !dry_run.dry_run {
                crate::inventory::refresh_stale_index(&repo, "canon artifacts store");
            }
            ExitCode::from(crate::evidence::run_store(&repo, max_artifact_mib, dry_run.dry_run, json))
        }
        ArtifactsCommand::Move(ArtifactsMoveArgs { tier, safety, repo, json, .. }) => ExitCode::from(crate::artifacts::run_move(&repo, &tier, &safety.mode(), json)),
    }
}
