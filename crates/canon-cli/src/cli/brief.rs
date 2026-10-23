//! `canon brief` — read, check and approve a change's brief.

use clap::{Args, Subcommand};

use canon_model::ChangeId;

#[derive(Subcommand)]
pub enum BriefCommand {
    /// Show where the change stands and the next one to three steps; never edits
    #[command(after_help = "Examples:\n  canon brief show\n  canon brief show add-login --json\n\nThe operand names the change; without it the repo's only live change is read.")]
    Show(BriefShowArgs),
    /// Read the brief against the rules: blocker and advisory findings (exit 0 clean, 2 blockers, 1 error)
    Check(BriefCheckArgs),
    /// Record the human's approval of the brief, bound to its text; refused while `brief check` has blockers
    #[command(after_help = "Examples:\n  canon brief approve --change-id add-login --by Ana --note 'Go. Ship the failure flow too.'\n\nWrites `Approved: <who> <when> — \"<words>\" (brief sha256:<digest>)` into the\nbrief (the digest covers the brief without its `Approved:` lines) and records\n`canon policy approve` in the same call. Edit the brief afterwards and it reads as\nnot approved (BRIEF_CHANGED_SINCE_APPROVAL) until a human answers again and this\nruns again. Run it only from the human's answer, with their words as --note.")]
    Approve(BriefApproveArgs),
}

/// The flags of `brief show`.
#[derive(Args)]
pub struct BriefShowArgs {
    /// The change to read (default: the repo's only live change)
    #[arg(value_parser = crate::subject::parse_change_id)]
    pub change_id: Option<ChangeId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

/// The flags of `brief check`.
#[derive(Args)]
pub struct BriefCheckArgs {
    /// The change to read (default: the repo's only live change)
    #[arg(long, value_parser = crate::subject::parse_change_id)]
    pub change_id: Option<ChangeId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

/// The flags of `brief approve`.
#[derive(Args)]
pub struct BriefApproveArgs {
    /// Who approved (the human, not the agent)
    #[arg(long)]
    pub by: String,
    /// The human's own words
    #[arg(long)]
    pub note: String,
    /// The change to read (default: the repo's only live change)
    #[arg(long, value_parser = crate::subject::parse_change_id)]
    pub change_id: Option<ChangeId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
