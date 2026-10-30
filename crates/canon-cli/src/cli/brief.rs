//! `canon brief` — read, check and approve a change's brief.

use clap::{Args, Subcommand};

use canon_model::ChangeId;

#[derive(Debug, Subcommand)]
pub enum BriefCommand {
    /// Show where the change stands and the next one to three steps; never edits
    #[command(after_help = "Examples:\n  canon brief show\n  canon brief show add-login --json\n\nThe operand names the change; without it the repo's only live change is read.")]
    Show(BriefShowArgs),
    /// Read the brief against the rules: blocker and advisory findings (exit 0 clean, 1 blockers, 2 refused)
    Check(BriefCheckArgs),
    /// Record the human's approval of the brief, bound to its text; refused while `brief check` has blockers
    #[command(after_help = "Examples:\n  canon brief approve add-login --approver Ana --note 'Go. Ship the failure flow too.'\n  canon brief approve add-login add-search fix-sync --approver Ana --note 'All three, go.'\n\nSeveral ids take one name, one answer and one timestamp. Every id is checked\nfirst; one refused id refuses the call (BRIEF_APPROVE_REFUSED) and nothing is\nwritten. Writes `Approved: <who> <when> — \"<words>\" (brief sha256:<digest>)` into the\nbrief (the digest covers the brief without its `Approved:` lines) and records\n`canon policy approve` in the same call. Edit the brief afterwards and it reads as\nnot approved (BRIEF_CHANGED_SINCE_APPROVAL) until a human answers again and this\nruns again. Run it only from the human's answer, with their words as --note.")]
    Approve(BriefApproveArgs),
}

/// The flags of `brief show`.
#[derive(Debug, Args)]
pub struct BriefShowArgs {
    /// The change to read (default: the repo's only live change)
    #[arg(value_parser = crate::subject::parse_change_id)]
    pub change_id: Option<ChangeId>,
    /// Print the brief file's bytes as they are, then a last line with the approval digest `brief approve` would record (with --json: `body` and `digest` fields); what a human is shown before they answer
    #[arg(long)]
    pub body: bool,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

/// The flags of `brief check`.
#[derive(Debug, Args)]
pub struct BriefCheckArgs {
    /// The change to read (default: the repo's only live change)
    #[arg(value_name = "CHANGE_ID", value_parser = crate::subject::parse_change_id)]
    pub change_id: Option<ChangeId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

/// The flags of `brief approve`.
#[derive(Debug, Args)]
pub struct BriefApproveArgs {
    /// Who approved (the human, not the agent)
    #[arg(long, value_name = "NAME")]
    pub approver: String,
    /// The human's own words
    #[arg(long, value_name = "TEXT")]
    pub note: String,
    /// Record the human's answer on their behalf: the tool they were asked with (`ask`, `AskUserQuestion`, `chat`); an agent role may run the call only with it
    #[arg(long, value_name = "NAME")]
    pub relayed_via: Option<String>,
    /// With --relayed-via, the agent's actor id (default: the CANON_ACTOR environment variable)
    #[arg(long, value_name = "ACTOR_ID", requires = "relayed_via")]
    pub actor: Option<String>,
    /// With --relayed-via, the digest `canon brief show --body` showed the human, as `<change>=<sha256:digest>`, once per change; a brief whose digest differs is refused
    #[arg(long = "digest", value_name = "DIGEST", requires = "relayed_via")]
    pub digests: Vec<String>,
    /// The changes to approve, one answer for all, none written unless every one is approvable (default: the repo's only live change)
    #[arg(value_name = "CHANGE_ID", value_parser = crate::subject::parse_change_id)]
    pub change_ids: Vec<ChangeId>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}
