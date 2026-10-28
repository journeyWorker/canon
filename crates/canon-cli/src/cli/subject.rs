//! `canon subject` — the clap tree of the group.

use canon_model::{ChangeId, RoleId, SubjectId};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum SubjectCommand {
    /// Author a new subject at status `proposed`
    New(SubjectNewArgs),
    /// Link an imported plan change to a subject (adopting it under another subject adds that subject; a change may belong to several)
    Adopt(SubjectAdoptArgs),
    /// Take a change back out of a subject (the mirror of adopt; refused for a change whose brief is approved)
    #[command(
        after_help = "Examples:\n  canon subject unadopt demo --change add-login\n\nRemoves the link on both sides (the subject's change list and the change's\nsubject list). The change and its brief stay and can be adopted elsewhere, for\nexample by a proposed holding subject. Refused (SUBJECT_UNADOPT_REFUSED) for a\nchange whose brief carries an `Approved:` line: it is the subject's approved\nscope, and for a change a scenario the subject owns is tagged\n`@flow:<change>.*` for (remove or retag it first). A pair that is not linked writes nothing and exits 0."
    )]
    Unadopt(SubjectUnadoptArgs),
    /// Show a subject (read-only)
    Show(SubjectShowArgs),
    /// Move a subject to another lifecycle state (shipping is evidence-gated; with spec_coverage.require_review, entering its scope is review-gated; shipped → verifying reopens with --reason)
    Move(SubjectMoveArgs),
}

#[derive(Debug, Args)]
pub struct SubjectNewArgs {
    /// The subject's kebab-slug id
    #[arg(value_name = "SUBJECT_ID", value_parser = crate::subject::parse_subject_id)]
    pub id: SubjectId,
    /// The subject's domain (kebab slug)
    #[arg(long, value_name = "NAME")]
    pub domain: String,
    /// The subject's title
    #[arg(long, value_name = "TEXT")]
    pub title: String,
    /// The subject's summary (optional)
    #[arg(long, value_name = "TEXT")]
    pub summary: Option<String>,
    /// The accountable owning role (default: implementer)
    #[arg(long, value_parser = crate::retrieve::parse_role, default_value = "implementer", value_name = "ROLE")]
    pub owner_role: RoleId,
    /// The invoking actor's id
    #[arg(long = "actor", default_value = "canon", value_name = "ACTOR_ID")]
    pub actor_id: String,
    #[command(flatten)]
    pub session: crate::cli::SessionArgs,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct SubjectAdoptArgs {
    /// The subject to adopt the change under
    #[arg(value_name = "SUBJECT_ID", value_parser = crate::subject::parse_subject_id)]
    pub subject_id: SubjectId,
    /// The imported Change's id
    #[arg(long = "change", value_parser = crate::subject::parse_change_id, value_name = "CHANGE_ID")]
    pub change_id: ChangeId,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct SubjectUnadoptArgs {
    /// The subject to take the change out of
    #[arg(value_name = "SUBJECT_ID", value_parser = crate::subject::parse_subject_id)]
    pub subject_id: SubjectId,
    /// The change to take out of the subject
    #[arg(long = "change", value_parser = crate::subject::parse_change_id, value_name = "CHANGE_ID")]
    pub change_id: ChangeId,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct SubjectShowArgs {
    /// The subject to show
    #[arg(value_name = "SUBJECT_ID", value_parser = crate::subject::parse_subject_id)]
    pub id: SubjectId,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct SubjectMoveArgs {
    /// The subject to move
    #[arg(value_name = "SUBJECT_ID", value_parser = crate::subject::parse_subject_id)]
    pub id: SubjectId,
    /// The state to move the subject to (proposed/specced/building/verifying/shipped/retired)
    #[arg(long, value_name = "STATE", value_parser = crate::subject::status_parser())]
    pub to: canon_model::SubjectStatus,
    /// Move anyway when only the require_review checks (unreviewed-promotion, open-blocker) refuse; the reason is recorded on the subject and `canon gate check` lists the gaps as advisories
    #[arg(long, value_name = "TEXT")]
    pub override_reason: Option<String>,
    /// Why the subject moves (required for shipped → verifying, optional on every other transition); recorded on the subject with the actor
    #[arg(long, value_name = "TEXT")]
    pub reason: Option<String>,
    /// The invoking actor's id, recorded with an --override-reason waiver or a --reason
    #[arg(long = "actor", default_value = "canon", value_name = "ACTOR_ID")]
    pub actor_id: String,
    #[command(flatten)]
    pub session: crate::cli::SessionArgs,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}
