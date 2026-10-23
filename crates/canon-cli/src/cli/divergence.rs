//! `canon divergence` — the clap tree of the group.

use canon_model::{ProjectId, RoleId, ScenarioId, Sha};
use chrono::{DateTime, Utc};
use clap::{Args, Subcommand};
use std::path::PathBuf;
use super::PromoteArgs;

/// The flags of `context-pack new`.
#[derive(clap::Args)]
pub struct ContextPackNewArgs {
    /// Repository-relative input manifest
    #[arg(long)]
    pub manifest: PathBuf,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
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
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output the complete bundle manifest as JSON
    #[arg(long)]
    pub json: bool,
}

/// The flags of `demo add`.
#[derive(clap::Args)]
pub struct DemoAddArgs {
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

/// The flags of `divergence add`.
#[derive(clap::Args)]
pub struct DivergenceAddArgs {
    /// The scenario (`<area>.<surface>.<nn>`)
    #[arg(value_name = "SCENARIO_ID", value_parser = crate::review::parse_scenario_id)]
    pub scenario_id: ScenarioId,
    /// Spec corpus id of the scenario. Required only when canon.yaml configures more than one spec root
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: Option<ProjectId>,
    #[arg(long, value_parser = crate::divergence::parse_sha)]
    pub sha: Sha,
    /// open / still-divergent / resolved / deferred (deferred needs --reason/--expiry)
    #[arg(long, default_value = "open")]
    pub status: String,
    #[arg(long)]
    pub reason: Option<String>,
    #[arg(long, value_parser = crate::divergence::parse_timestamp)]
    pub expiry: Option<DateTime<Utc>>,
    #[arg(long, default_value_t = 1)]
    pub round: u32,
    #[arg(long)]
    pub reviewer: String,
    #[arg(long, default_value = "")]
    pub detail: String,
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: RoleId,
    #[command(flatten)]
    pub promote: PromoteArgs,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum DivergenceCliCommand {
    /// Add a divergence candidate, staged with no run_seq yet (commit it with `canon divergence promote`, or in the same run with --promote / `write: { promote: auto }`)
    Add(DivergenceAddArgs),
    /// Promote all staged candidates, assigning run_seq
    Promote(DivergencePromoteArgs),
    /// Directly record a resolved divergence
    Resolve(DivergenceResolveArgs),
    /// Directly record a deferred divergence (requires --reason/--expiry)
    Defer(DivergenceDeferArgs),
    /// Accept a divergent scenario as a tracked known gap (a reviewer step): its latest evidence must be divergent and not the accepting actor's or session's. The acceptance pins that evidence, so new evidence ends it, and lapses at --expiry; until then the ship guard counts the cell as covered while `status` and `gate check` list it as `accepted-divergence`
    Accept(DivergenceAcceptArgs),
    /// Show the current divergence burn-down state
    Status(DivergenceStatusArgs),
}

#[derive(Args)]
pub struct DivergencePromoteArgs {
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Preview without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DivergenceResolveArgs {
    /// The scenario (`<area>.<surface>.<nn>`)
    #[arg(value_name = "SCENARIO_ID", value_parser = crate::review::parse_scenario_id)]
    pub scenario_id: ScenarioId,
    /// Spec corpus id of the scenario. Required only when canon.yaml configures more than one spec root
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: Option<ProjectId>,
    #[arg(long, value_parser = crate::divergence::parse_sha)]
    pub sha: Sha,
    #[arg(long, default_value_t = 1)]
    pub round: u32,
    #[arg(long)]
    pub reviewer: String,
    #[arg(long, default_value = "")]
    pub detail: String,
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: RoleId,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DivergenceDeferArgs {
    /// The scenario (`<area>.<surface>.<nn>`)
    #[arg(value_name = "SCENARIO_ID", value_parser = crate::review::parse_scenario_id)]
    pub scenario_id: ScenarioId,
    /// Spec corpus id of the scenario. Required only when canon.yaml configures more than one spec root
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: Option<ProjectId>,
    #[arg(long, value_parser = crate::divergence::parse_sha)]
    pub sha: Sha,
    #[arg(long, default_value_t = 1)]
    pub round: u32,
    #[arg(long)]
    pub reviewer: String,
    #[arg(long)]
    pub reason: String,
    #[arg(long, value_parser = crate::divergence::parse_timestamp)]
    pub expiry: DateTime<Utc>,
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: RoleId,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DivergenceAcceptArgs {
    /// The accepted scenarios (`<area>.<surface>.<nn>`), one acceptance each, all or nothing
    #[arg(value_name = "SCENARIO_ID", required = true, value_parser = crate::review::parse_scenario_id)]
    pub scenario_ids: Vec<ScenarioId>,
    /// Spec corpus id of the scenarios. Required only when canon.yaml configures more than one spec root
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: Option<ProjectId>,
    /// Why the gap is accepted
    #[arg(long)]
    pub reason: String,
    /// Where the gap is tracked: an issue URL or reference
    #[arg(long)]
    pub tracking: String,
    /// The date (YYYY-MM-DD, UTC) the acceptance stops holding; none means it holds until new evidence
    #[arg(long, value_parser = crate::divergence::parse_expiry_date)]
    pub expiry: Option<DateTime<Utc>>,
    #[arg(long)]
    pub reviewer: String,
    /// The invoking actor's id
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    /// Must be reviewer. Required unless the declared role (--as / CANON_ROLE) is reviewer, which it then defaults to
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: Option<RoleId>,
    /// The accepting session (recorded as actor.session_id); it must not be one that attested the scenario's evidence
    #[arg(long, value_parser = crate::review::parse_session_id)]
    pub session_id: Option<canon_model::SessionId>,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DivergenceStatusArgs {
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Governs Deferred expiry; defaults to now
    #[arg(long, value_parser = crate::divergence::parse_timestamp)]
    pub as_of: Option<DateTime<Utc>>,
    /// Print `{data, meta}` as JSON
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub page: crate::paging::PageFlags,
    /// With --json, keep only these scenario fields (comma-separated): project_id, scenario_id, state, reason, expiry, tracking; an unknown field is refused
    #[arg(long, value_delimiter = ',')]
    pub fields: Option<Vec<String>>,
}
