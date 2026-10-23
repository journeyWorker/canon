//! `canon divergence` — the clap tree of the group.

use canon_model::{ProjectId, RoleId, ScenarioId};
use chrono::{DateTime, Utc};
use clap::{Args, Subcommand};
use super::PromoteArgs;

/// The flags of `divergence add`.
#[derive(clap::Args)]
pub struct DivergenceAddArgs {
    /// The scenario (`<area>.<surface>.<nn>`)
    #[arg(value_name = "SCENARIO_ID", value_parser = crate::review::parse_scenario_id)]
    pub scenario_id: ScenarioId,
    /// Spec corpus id of the scenario. Required only when canon.yaml configures more than one spec root
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: Option<ProjectId>,
    #[command(flatten)]
    pub rev: crate::cli::RevArgs,
    /// The divergence state to record (deferred needs --reason and --expiry)
    #[arg(long, value_name = "STATE", default_value = "open", value_parser = clap::builder::PossibleValuesParser::new(["open", "still-divergent", "resolved", "deferred"]))]
    pub state: String,
    #[arg(long)]
    pub reason: Option<String>,
    #[arg(long, value_parser = crate::divergence::parse_timestamp)]
    pub expiry: Option<DateTime<Utc>>,
    #[arg(long, default_value_t = 1)]
    pub round: u32,
    /// The reviewer recording this (recorded as the divergence's reviewer and actor)
    #[arg(long)]
    pub actor_id: String,
    #[arg(long, default_value = "")]
    pub detail: String,
    /// The role this record is attributed to (default: the role declared with --as / CANON_ROLE)
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: Option<RoleId>,
    #[command(flatten)]
    pub promote: PromoteArgs,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum DivergenceCliCommand {
    /// Add a divergence candidate, staged with no run_seq yet (commit it with `canon gate promote --kind divergence`, or in the same run with --promote / `write: { promote: auto }`)
    Add(DivergenceAddArgs),
    /// Directly record a resolved divergence
    Resolve(DivergenceResolveArgs),
    /// Directly record a deferred divergence (requires --reason/--expiry)
    Defer(DivergenceDeferArgs),
    /// Accept a divergent scenario as a tracked known gap (a reviewer step): its latest evidence must be divergent and not the accepting actor's or session's. The acceptance pins that evidence, so new evidence ends it, and lapses at --expiry; until then the ship guard counts the cell as covered while `status` and `gate check` list it as `accepted-divergence`
    Accept(DivergenceAcceptArgs),
    /// List the current divergence burn-down state
    List(DivergenceListArgs),
}

#[derive(Args)]
pub struct DivergenceResolveArgs {
    /// The scenario (`<area>.<surface>.<nn>`)
    #[arg(value_name = "SCENARIO_ID", value_parser = crate::review::parse_scenario_id)]
    pub scenario_id: ScenarioId,
    /// Spec corpus id of the scenario. Required only when canon.yaml configures more than one spec root
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: Option<ProjectId>,
    #[command(flatten)]
    pub rev: crate::cli::RevArgs,
    #[arg(long, default_value_t = 1)]
    pub round: u32,
    /// The reviewer recording this (recorded as the divergence's reviewer and actor)
    #[arg(long)]
    pub actor_id: String,
    #[arg(long, default_value = "")]
    pub detail: String,
    /// The role this record is attributed to (default: the role declared with --as / CANON_ROLE)
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: Option<RoleId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
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
    #[command(flatten)]
    pub rev: crate::cli::RevArgs,
    #[arg(long, default_value_t = 1)]
    pub round: u32,
    /// The reviewer recording this (recorded as the divergence's reviewer and actor)
    #[arg(long)]
    pub actor_id: String,
    #[arg(long)]
    pub reason: String,
    #[arg(long, value_parser = crate::divergence::parse_timestamp)]
    pub expiry: DateTime<Utc>,
    /// The role this record is attributed to (default: the role declared with --as / CANON_ROLE)
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: Option<RoleId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
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
    #[arg(long, value_parser = crate::divergence::parse_timestamp)]
    pub expiry: Option<DateTime<Utc>>,
    /// The reviewer recording this (recorded as the divergence's reviewer and actor)
    #[arg(long)]
    pub actor_id: String,
    /// Must be reviewer (default: the declared role, --as / CANON_ROLE; required when none is declared)
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: Option<RoleId>,
    /// The accepting session (recorded as actor.session_id); it must not be one that attested the scenario's evidence
    #[arg(long, value_parser = crate::review::parse_session_id)]
    pub session_id: Option<canon_model::SessionId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DivergenceListArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Governs Deferred expiry; defaults to now
    #[arg(long, value_parser = crate::divergence::parse_timestamp)]
    pub as_of: Option<DateTime<Utc>>,
    /// Print `{data, meta}` as JSON
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub page: crate::paging::PageFlags,
    /// With --json, keep only these scenario fields (comma-separated): project_id, scenario_id, state, reason, expiry, tracking; an unknown field is refused
    #[arg(long, value_name = "FIELDS", value_delimiter = ',', action = clap::ArgAction::Set)]
    pub fields: Option<Vec<String>>,
}
