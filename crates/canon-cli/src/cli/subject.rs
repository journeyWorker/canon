//! `canon subject` — the clap tree of the group.

use canon_model::{ChangeId, ProjectId, RoleId, ScenarioId, Sha, SubjectId};
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

/// The flags of `divergence add`.
#[derive(clap::Args)]
pub struct DivergenceAddArgs {
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: ProjectId,
    #[arg(long, value_parser = crate::review::parse_scenario_id)]
    pub scenario_id: ScenarioId,
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
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum SubjectCommand {
    /// Author a new subject at status `proposed`
    New(SubjectNewArgs),
    /// Link an imported plan change to a subject (adopting it under another subject adds that subject; a change may belong to several)
    Adopt(SubjectAdoptArgs),
    /// Take a change back out of a subject (the mirror of adopt; refused for a change whose brief is approved)
    #[command(
        after_help = "Examples:\n  canon subject unadopt demo --change-id add-login\n\nRemoves the link on both sides (the subject's change list and the change's\nsubject list). The change and its brief stay and can be adopted elsewhere, for\nexample by a proposed holding subject. Refused (SUBJECT_UNADOPT_REFUSED) for a\nchange whose brief carries an `Approved:` line: it is the subject's approved\nscope, and for a change a scenario the subject owns is tagged\n`@flow:<change>.*` for (remove or retag it first). A pair that is not linked writes nothing and exits 0."
    )]
    Unadopt(SubjectUnadoptArgs),
    /// Show a subject, or with --to transition its lifecycle status (shipping is evidence-gated; with spec_coverage.require_review, entering its scope is review-gated; shipped → verifying reopens with --reason)
    Status(SubjectStatusArgs),
}

#[derive(Args)]
pub struct SubjectNewArgs {
    /// The subject's kebab-slug id
    #[arg(value_name = "SUBJECT_ID", value_parser = crate::subject::parse_subject_id)]
    pub id: SubjectId,
    /// The subject's domain (kebab slug)
    #[arg(long)]
    pub domain: String,
    /// The subject's title
    #[arg(long)]
    pub title: String,
    /// The subject's summary (optional)
    #[arg(long, default_value = "")]
    pub summary: String,
    /// The accountable owning role (default: implementer)
    #[arg(long, value_parser = crate::retrieve::parse_role, default_value = "implementer")]
    pub owner_role: RoleId,
    /// The invoking actor's id
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct SubjectAdoptArgs {
    /// The subject to adopt the change under
    #[arg(value_name = "SUBJECT_ID", value_parser = crate::subject::parse_subject_id)]
    pub subject_id: SubjectId,
    /// The imported Change's id
    #[arg(long, value_parser = crate::subject::parse_change_id)]
    pub change_id: ChangeId,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct SubjectUnadoptArgs {
    /// The subject to take the change out of
    #[arg(value_name = "SUBJECT_ID", value_parser = crate::subject::parse_subject_id)]
    pub subject_id: SubjectId,
    /// The change to take out of the subject
    #[arg(long, value_parser = crate::subject::parse_change_id)]
    pub change_id: ChangeId,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct SubjectStatusArgs {
    /// The subject to show, or transition with --to
    #[arg(value_name = "SUBJECT_ID", value_parser = crate::subject::parse_subject_id)]
    pub id: SubjectId,
    /// Move the subject to this state (proposed/specced/building/verifying/shipped/retired); without it the subject is shown and nothing is written
    #[arg(long, value_name = "STATE", value_parser = crate::subject::status_parser())]
    pub to: Option<canon_model::SubjectStatus>,
    /// Move anyway when only the require_review checks (unreviewed-promotion, open-blocker) refuse; the reason is recorded on the subject and `canon gate check` lists the gaps as advisories
    #[arg(long)]
    pub override_reason: Option<String>,
    /// Why the subject moves (required for shipped → verifying, optional on every other transition); recorded on the subject with the actor
    #[arg(long)]
    pub reason: Option<String>,
    /// The invoking actor's id, recorded with an --override-reason waiver or a --reason
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
