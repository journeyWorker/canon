//! `canon review` — the clap tree of the group.

use canon_model::{ChangeId, ProjectId, RoleId, ScenarioId};
use clap::{Args, Subcommand};
use std::path::PathBuf;

/// The flags of `context-pack new`.
#[derive(clap::Args)]
pub struct ContextPackNewArgs {
    /// Repository-relative input manifest
    #[arg(long)]
    pub manifest: PathBuf,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output the complete pack manifest as JSON
    #[arg(long)]
    pub json: bool,
}

/// The flags of `prompt add`.
#[derive(clap::Args)]
pub struct PromptAddArgs {
    /// Bundle name
    #[arg(long)]
    pub name: String,
    /// Bundle version
    #[arg(long)]
    pub version: String,
    /// Repository-relative input manifest
    #[arg(long)]
    pub manifest: PathBuf,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output the complete bundle manifest as JSON
    #[arg(long)]
    pub json: bool,
}

/// The flags of `demo add`.
#[derive(clap::Args)]
pub struct DemoAddArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum ReviewCommand {
    /// Write one attributed Review record per scenario (or, with --change-id, one for a chore change) directly to the ledger, nothing to promote (exactly one provenance ref required). A scenario review pins the scenario's digest and what its latest evidence says (verdict and checks); it counts only while both are unchanged. A change review pins the chore's brief
    #[command(group(clap::ArgGroup::new("provenance").args(["upstream_ref", "original_spec_ref"]).required(true).multiple(false)))]
    Add(ReviewAddArgs),
}

#[derive(Args)]
pub struct ReviewAddArgs {
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: ProjectId,
    /// The reviewed scenario; repeatable, one review per scenario, all or nothing. Each must have a record (`canon inventory sync`), or nothing is written (exit 2)
    #[arg(long = "scenario-id", value_parser = crate::review::parse_scenario_id)]
    pub scenario_ids: Vec<ScenarioId>,
    /// Read more scenario ids from a file (`-` for stdin): one per line, `#` starts a comment
    #[arg(long, value_name = "FILE|-")]
    pub scenario_ids_from: Option<String>,
    /// Review a chore change instead of scenarios: its invariants and checks against the diff. Pins the change's brief; an edit to the brief stales it. A chore needs exactly this one review before `--to shipped`
    #[arg(long, value_name = "SLUG", value_parser = crate::query::parse_change_id, conflicts_with_all = ["scenario_ids", "scenario_ids_from"])]
    pub change_id: Option<ChangeId>,
    #[arg(long)]
    pub reviewer: String,
    #[arg(long)]
    pub pin: String,
    /// Provenance ref (mutually exclusive with --original-spec-ref; exactly one required)
    #[arg(long)]
    pub upstream_ref: Option<String>,
    /// Provenance ref (mutually exclusive with --upstream-ref; exactly one required)
    #[arg(long)]
    pub original_spec_ref: Option<String>,
    /// The invoking actor's id
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    /// The reviewer's role. Required unless the declared role (--as / CANON_ROLE) is implementer or reviewer, which it then defaults to
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: Option<RoleId>,
    /// The reviewing session (recorded as actor.session_id). Under require_review.distinct_actor a review whose session also attested the scenario's evidence does not count
    #[arg(long, value_parser = crate::review::parse_session_id)]
    pub session_id: Option<canon_model::SessionId>,
    /// How the reviewer's run ended: signed (an approval), indeterminate (ran, reached no verdict) or unavailable (could not run). Only signed counts; the others are recorded as review gaps
    #[arg(long, default_value = "signed", value_parser = crate::review::parse_outcome)]
    pub outcome: canon_model::ReviewOutcome,
    /// The reviewer runtime's provider (e.g. anthropic, openai)
    #[arg(long, value_parser = crate::review::parse_runtime_label)]
    pub reviewer_provider: Option<String>,
    /// The reviewer runtime's model id
    #[arg(long, value_parser = crate::review::parse_runtime_label)]
    pub reviewer_model: Option<String>,
    /// The harness that ran the reviewer (e.g. omp, codex, claude)
    #[arg(long, value_parser = crate::review::parse_runtime_label)]
    pub reviewer_harness: Option<String>,
    /// A file the reviewer looked at (screenshot, recording); repeatable; must be inside the repo. Stored in .canon/artifacts/sha256/ and re-hashed by `canon gate check`
    #[arg(long = "artifact", value_name = "PATH")]
    pub artifacts: Vec<PathBuf>,
    /// Per-file size limit for the artifact store (.canon/artifacts/sha256/), in MiB; a larger file is refused
    #[arg(long, value_name = "MIB", default_value_t = canon_gate::DEFAULT_MAX_ARTIFACT_MIB, value_parser = clap::value_parser!(u64).range(1..))]
    pub max_artifact_mib: u64,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
