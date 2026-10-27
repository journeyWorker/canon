//! `canon review` — the clap tree of the group.

use canon_model::{ChangeId, ProjectId, RoleId, ScenarioId};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Write one attributed Review record per scenario directly to the ledger, nothing to promote (exactly one provenance ref required). A review pins the scenario's digest and what its latest evidence says (verdict and checks); it counts only while both are unchanged. Each scenario must be indexed, or nothing is written (exit 2)
    #[command(group(clap::ArgGroup::new("provenance").args(["upstream_ref", "original_spec_ref"]).required(true).multiple(false)))]
    Scenario(ReviewScenarioArgs),
    /// Write one attributed Review record for a chore change directly to the ledger (exactly one provenance ref required). The review signs the chore's invariants and checks against the diff and pins the change's brief; an edit to the brief stales it. A chore needs exactly this one review before `subject move --to shipped`
    #[command(group(clap::ArgGroup::new("provenance").args(["upstream_ref", "original_spec_ref"]).required(true).multiple(false)))]
    Change(ReviewChangeArgs),
}

/// The flags of `review scenario`.
#[derive(Debug, Args)]
pub struct ReviewScenarioArgs {
    /// The reviewed scenarios (`<area>.<surface>.<nn>`), one review each, all or nothing. Each must have a record (`canon inventory sync`), or nothing is written (exit 2)
    #[arg(value_name = "SCENARIO_ID", required = true, value_parser = crate::review::parse_scenario_id)]
    pub scenario_ids: Vec<ScenarioId>,
    #[command(flatten)]
    pub review: ReviewOptions,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

/// The flags of `review change`.
#[derive(Debug, Args)]
pub struct ReviewChangeArgs {
    /// The chore change to review
    #[arg(value_name = "CHANGE_ID", value_parser = crate::query::parse_change_id)]
    pub change_id: ChangeId,
    #[command(flatten)]
    pub review: ReviewOptions,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

/// The options both review leaves share.
#[derive(Debug, Args)]
pub struct ReviewOptions {
    /// Spec corpus id of the scenarios. Required only when canon.yaml configures more than one spec root
    #[arg(long = "project", value_parser = crate::review::parse_project_id, value_name = "PROJECT_ID")]
    pub project_id: Option<ProjectId>,
    /// The reviewer's id (recorded as the review's author and as its reviewer; line breaks refused)
    #[arg(long = "actor", value_name = "ACTOR_ID")]
    pub actor_id: String,
    #[command(flatten)]
    pub rev: crate::cli::RevArgs,
    /// Provenance ref (mutually exclusive with --original-spec-ref; exactly one required)
    #[arg(long, value_name = "TEXT")]
    pub upstream_ref: Option<String>,
    /// Provenance ref (mutually exclusive with --upstream-ref; exactly one required)
    #[arg(long, value_name = "TEXT")]
    pub original_spec_ref: Option<String>,
    /// The reviewer's role. Required unless the declared role (--as / CANON_ROLE) is implementer or reviewer, which it then defaults to
    #[arg(long, value_parser = crate::retrieve::parse_role, value_name = "ROLE")]
    pub role: Option<RoleId>,
    /// The reviewing session (recorded as actor.session_id). Under require_review.distinct_actor a review whose session also attested the scenario's evidence does not count
    #[arg(long = "session", value_parser = crate::review::parse_session_id, value_name = "SESSION_ID")]
    pub session_id: Option<canon_model::SessionId>,
    /// How the reviewer's run ended: signed (an approval), indeterminate (ran, reached no verdict) or unavailable (could not run). Only signed counts; the others are recorded as review gaps
    #[arg(long, default_value = "signed", value_parser = crate::review::outcome_parser(), value_name = "REVIEW_OUTCOME")]
    pub review_outcome: canon_model::ReviewOutcome,
    /// The reviewer runtime's provider (e.g. anthropic, openai)
    #[arg(long, value_parser = crate::review::parse_runtime_label, value_name = "NAME")]
    pub reviewer_provider: Option<String>,
    /// The reviewer runtime's model id
    #[arg(long, value_parser = crate::review::parse_runtime_label, value_name = "NAME")]
    pub reviewer_model: Option<String>,
    /// The harness that ran the reviewer (e.g. omp, codex, claude)
    #[arg(long, value_parser = crate::review::parse_runtime_label, value_name = "NAME")]
    pub reviewer_harness: Option<String>,
    /// A file the reviewer looked at (screenshot, recording); repeatable; must be inside the repo. Stored in .canon/artifacts/sha256/ and re-hashed by `canon gate check`
    #[arg(long = "artifact", value_name = "PATH")]
    pub artifacts: Vec<PathBuf>,
    /// Per-file size limit for the artifact store (.canon/artifacts/sha256/), in MiB; a larger file is refused
    #[arg(long, value_name = "N", default_value_t = canon_gate::DEFAULT_MAX_ARTIFACT_MIB, value_parser = clap::value_parser!(u64).range(1..))]
    pub max_artifact_mib: u64,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
