//! `canon change` — the clap tree of the group.

use canon_model::{ChangeId, SubjectId};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum ChangeCommand {
    /// Write .canon/changes/<slug>/brief.md, import it, and adopt it into a subject
    New(ChangeNewArgs),
}

#[derive(Debug, Args)]
pub struct ChangeNewArgs {
    /// The change's slug (its change_id)
    #[arg(value_name = "CHANGE_ID", value_parser = crate::subject::parse_change_id)]
    pub slug: ChangeId,
    /// The existing subject to adopt the change under
    #[arg(long = "subject", value_parser = crate::subject::parse_subject_id, value_name = "SUBJECT_ID")]
    pub subject_id: SubjectId,
    /// `feature` (default): the PRD brief. `chore`: a short brief for work that
    /// changes how the subject is built or kept, never what it does
    #[arg(long, default_value = "feature", value_parser = crate::brief_template::kind_parser(), value_name = "CHANGE_KIND")]
    pub change_kind: crate::brief_template::ChangeKind,
    /// The change's title (also its brief's summary)
    #[arg(long, value_name = "TEXT")]
    pub title: String,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}
