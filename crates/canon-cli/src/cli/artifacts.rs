//! `canon artifacts` — the clap tree of the group.

use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum ArtifactsCommand {
    /// Check that the artifact store holds every file the latest evidence and current reviews bind (--full re-hashes the stored bytes)
    #[command(after_help = "Asks the store canon.yaml's `artifacts:` block selects (default: local, .canon/artifacts/sha256) about each bound file, the way `canon gate check` does. For an object store that is one HEAD per distinct digest plus the size and sha256 metadata written at upload; --full downloads each blob and re-hashes it.\n\nExit 0 when every file is stored, 1 when one is missing or altered (data.problems names each), 2 when the store cannot be used or reached: an unreachable store is a refusal, never a pass.")]
    Verify(ArtifactsVerifyArgs),
    /// Store the bound files of existing evidence records and current reviews in the artifact store (.canon/artifacts/sha256/)
    #[command(after_help = "For records written before canon stored bound bytes (0.11-0.13), and for any evidence file, or file of a current review, whose stored blob was lost. Each attachment whose working-tree file still hashes to the recorded digest is copied into .canon/artifacts/sha256/<hex> (or uploaded to the object store); everything else is listed as not stored, with why. A superseded review's files are not stored, as the gate does not check them. Commit the store with the ledger.\n\nExit 0 when the pass ran (the gate decides whether an unstored file matters), 2 when the ledger cannot be read or a write fails.")]
    Store(ArtifactsStoreArgs),
    /// Move the local artifact blobs to an object-store tier, verify them, and remove the local copies
    #[command(after_help = "Examples:\n  canon artifacts move --tier cold\n  canon artifacts move --tier cold --commit --plan-digest <DIGEST>\n\nThe bare run previews: it lists every upload and removal and prints a planDigest, and writes nothing. --commit --plan-digest acts on exactly that plan (PLAN_CHANGED, exit 2, when it changed since).\n\n--tier names an `s3` tier of canon.yaml's `tiers:` (for example `cold`; its credentials are the tier's own). Every blob in .canon/artifacts/sha256 is hashed, uploaded under <prefix>sha256/<digest> (prefix artifacts/), and read back; only when all of them verified does canon.yaml gain `artifact_store: { backend: <tier>, prefix: artifacts/ }`, after which the local copies are removed and `.canon/artifacts/` is added to .gitignore. Commit the removal with the ledger.\n\nIdempotent and resumable: a run stopped anywhere leaves a repo the gate still passes, and the next run finishes; a second run finds nothing to do. A blob whose bytes do not match its name is never moved. Moving back to local is refused; `--tier local` on a local repo is a no-op.")]
    Move(ArtifactsMoveArgs),
}

#[derive(Debug, Args)]
pub struct ArtifactsVerifyArgs {
    /// Download every blob and re-hash it, instead of trusting the store's size and digest metadata
    #[arg(long)]
    pub full: bool,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ArtifactsStoreArgs {
    /// Per-file size limit for the artifact store, in MiB
    #[arg(long, value_name = "N", default_value_t = canon_gate::DEFAULT_MAX_ARTIFACT_MIB, value_parser = clap::value_parser!(u64).range(1..))]
    pub max_artifact_mib: u64,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct ArtifactsMoveArgs {
    /// The tier to move to: the name of an `s3` tier in canon.yaml (for example `cold`)
    #[arg(long, value_name = "TIER")]
    pub tier: String,
    #[command(flatten)]
    pub safety: crate::cli::SafetyArgs,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}
