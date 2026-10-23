//! `canon feature` — the clap tree of the group.

use canon_model::ProjectId;
use clap::{Args, Subcommand};
use crate::scaffold::AreaSurface;

#[derive(Subcommand)]
pub enum FeatureCommand {
    /// Create a fresh .feature file for a new area.surface
    New(FeatureNewArgs),
}

#[derive(Args)]
pub struct FeatureNewArgs {
    /// <area>.<surface> the fresh .feature file scaffolds
    #[arg(value_name = "SURFACE", value_parser = crate::scaffold::parse_area_surface)]
    pub surface: AreaSurface,
    /// The Feature: header label
    #[arg(long, value_name = "TEXT")]
    pub title: String,
    /// Agent id written into provenance (env: CANON_ACTOR)
    #[arg(long, env = "CANON_ACTOR", default_value = "canon-scaffold", value_name = "ACTOR_ID")]
    pub actor_id: String,
    /// Which configured `specs.roots[]` entry to write under, by its id. Required only when the repo configures more than one
    #[arg(long, value_parser = crate::review::parse_project_id, value_name = "PROJECT_ID")]
    pub project_id: Option<ProjectId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}
