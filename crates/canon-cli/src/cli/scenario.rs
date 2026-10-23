//! `canon scenario` — the clap tree of the group.

use canon_model::{ChangeId, ProjectId, ScenarioId, SubjectId};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ScenarioCommand {
    /// Append a tagged scenario stub to its .feature file (created if missing), or many with --from
    #[command(group(clap::ArgGroup::new("scenario_source").args(["from", "tag"]).required(true).multiple(false)))]
    New(ScenarioNewArgs),
}

#[derive(Args)]
pub struct ScenarioNewArgs {
    /// <area>.<surface>.<nn> scenario tag
    #[arg(value_name = "SCENARIO_ID", value_parser = crate::scaffold::parse_scenario_tag)]
    pub tag: Option<ScenarioId>,
    /// Write many scenarios from NDJSON (a file, or `-` for stdin): one JSON object per line,
    /// blank lines and `#` comments skipped. A line carries this command's fields by long flag
    /// name: id (the tag), title, subject, lane, case, flow, change, feature, project, e.g.
    /// {"id":"auth.login.02","title":"Sign out","subject":"demo","case":"failure","flow":"F1"}.
    /// Every line is checked first; one bad line writes nothing and `data.items` lists each
    /// `{line, id, error}`. Otherwise `data.items` lists each `{line, id, path}`
    #[arg(long, value_name = "FILE|-", conflicts_with_all = ["title", "subject_id", "lane", "case", "flow", "change_id", "feature", "project_id"])]
    pub from: Option<String>,
    /// The Scenario: header label
    #[arg(long, required_unless_present = "from")]
    pub title: Option<String>,
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
}
