//! `canon scenario` — the clap tree of the group.

use canon_model::{ChangeId, ProjectId, ScenarioId, SubjectId};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ScenarioCommand {
    /// Append a tagged scenario stub to its .feature file (created if missing); many at once with `--input`
    New(ScenarioNewArgs),
}

#[derive(Args)]
pub struct ScenarioNewArgs {
    /// <area>.<surface>.<nn> scenario tag
    #[arg(value_name = "SCENARIO_ID", value_parser = crate::scaffold::parse_scenario_tag)]
    pub tag: ScenarioId,
    /// The Scenario: header label
    #[arg(long)]
    pub title: String,
    /// Optional subject id tag to pin above the scenario
    #[arg(long, value_parser = crate::subject::parse_subject_id)]
    pub subject_id: Option<SubjectId>,
    /// Optional lane tag to classify the scenario
    #[arg(long, value_parser = crate::scaffold::parse_lane_slug)]
    pub lane: Option<String>,
    /// Optional case tag: which path of the behavior this scenario
    /// specifies (`happy`, `failure`, `edge` in the base vocabulary)
    #[arg(long, value_parser = crate::scaffold::parse_lane_slug)]
    pub case: Option<String>,
    /// The brief flow step this scenario covers (`F1`): writes `@flow:<change>.<F-id>`.
    /// The change is --change-id, else the newest change of --subject-id, else the only live change
    #[arg(long, value_name = "F-ID")]
    pub flow: Option<String>,
    /// The change whose brief defines --flow (default: --subject-id's newest adopted change)
    #[arg(long, requires = "flow", value_parser = crate::subject::parse_change_id)]
    pub change_id: Option<ChangeId>,
    /// Agent id written into provenance (env: CANON_ACTOR)
    #[arg(long, env = "CANON_ACTOR", default_value = "canon-scaffold")]
    pub actor_id: String,
    /// Target .feature file (default: derived from <tag>; must live under a specs.roots[] entry)
    #[arg(long)]
    pub feature: Option<PathBuf>,
    /// Which configured `specs.roots[]` entry to write under, by its id. Required only when the repo configures more than one
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: Option<ProjectId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}
