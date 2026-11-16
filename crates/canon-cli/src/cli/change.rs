//! `canon change` — the clap tree of the group.

use canon_model::{ChangeId, SubjectId};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum ChangeCommand {
    /// Write .canon/changes/<slug>/brief.md, import it, and adopt it into a subject
    New(ChangeNewArgs),
    /// Set an approved change aside for now (a human's decision: needs a reason, a reference and the approver); `canon change reopen` brings it back
    #[command(after_help = "\
Examples:
  canon change defer canon-0-41-ci-nextest --reason 'the offline job is over budget' \\
      --ref https://example.com/pull/147 --approver Ana
  canon change defer add-login --reason 'waiting on the vendor' --ref 'chat 2026-11-16' \\
      --approver Ana --relayed-via ask --actor impl-1 --as orchestrator

Records the decision as a `change_state` record (read it with `canon query --kind
change_state`, find it with `canon search`). A deferred change reads as if it were not on
its subjects' change lists: `gate check`, `status` and the ship and reopen guards stop
asking for it, and the scenarios only it made (flow-tagged `@flow:<change>.*`, created after
its brief was first committed) are set aside. The change's brief, evidence, reviews and
findings stay; a warning (CHANGE_RECORDS_KEPT) counts the records kept. Deferral does not
expire.

Human only, like `brief approve`: an agent role is refused (ACTOR_DENIED) unless it relays
the human's explicit answer with --relayed-via (and --actor, or CANON_ACTOR), which the record
keeps; `approval: { relay: deny }` in canon.yaml refuses that too. --reason is the human's
words, one line; give --ref once per reference a reader can open (a link, a commit, a file,
the date of a conversation).

Refused (CHANGE_CONFLICT, nothing written): `SHIPPED` for a change that already shipped (write
a new change that removes the behavior), `CLOSED` for a change already deferred or dropped with
other values (the same call again is `unchanged`). CHANGE_NOT_FOUND for a change with no
record. A repo with no `change_state:` line under `routing:` is refused (CONFIG_INVALID, TIERS):
run `canon migrate`. Not destructive: nothing is deleted, so there is no --commit.")]
    Defer(ChangeCloseArgs),
    /// Give up an approved change for good (a human's decision: needs a reason, a reference and the approver); it is never brought back
    #[command(after_help = "\
Examples:
  canon change drop add-cart --reason 'the owner chose another design' \\
      --ref https://example.com/issues/42 --approver Ana
  canon change drop add-cart --reason 'the owner chose another design' --ref 'chat 2026-11-16' \\
      --approver Ana --relayed-via ask --actor impl-1 --as orchestrator

The same as `canon change defer`, with state `dropped`: options, human-only rule, refusals,
warning and report. A deferred change can be dropped. A dropped change takes no other state
and the same call again is `unchanged`; any other call is CHANGE_CONFLICT (CLOSED). To bring
the work back, write a new change (`canon change new`) and name the dropped one in its
`Source:`.")]
    Drop(ChangeCloseArgs),
    /// Bring a deferred change back (a human's decision); the old approval is void and the brief needs a new one
    #[command(after_help = "\
Examples:
  canon change reopen add-login --reason 'the vendor answered' --approver Ana
  canon change reopen add-login --reason 'the vendor answered' --ref 'chat 2026-11-20' \\
      --approver Ana --relayed-via ask --actor impl-1 --as orchestrator

Only a deferred change reopens (CHANGE_CONFLICT with reason DROPPED for a dropped one, whose
hint says to write a new change; NOT_DEFERRED for a change that is not deferred). The change
returns to its subjects' change lists and the scenarios it set aside are required again. An
`Approved:` line older than the reopening is not an approval: `status` says the approval is
stale, and `canon brief approve` writes a new line even for the same name and answer.
Human only, like `canon change defer`; --ref is optional here. This reopens a change; to
reopen a shipped subject use `canon subject move --to verifying`.")]
    Reopen(ChangeReopenArgs),
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

/// The flags of `change defer` and `change drop`.
#[derive(Debug, Args)]
pub struct ChangeCloseArgs {
    /// The change to close (it must have a record)
    #[arg(value_name = "CHANGE_ID", value_parser = crate::subject::parse_change_id)]
    pub change_id: ChangeId,
    /// Why, in the human's words (one line)
    #[arg(long, value_name = "TEXT", value_parser = crate::cli::trimmed)]
    pub reason: String,
    /// What a reader can open to see why: a link, a commit, a file, the date of a conversation (one line; once per reference, at least one)
    #[arg(long = "ref", value_name = "TEXT", required = true, value_parser = crate::cli::trimmed)]
    pub refs: Vec<String>,
    /// Who decided (the human, not the agent)
    #[arg(long, value_name = "NAME", value_parser = crate::cli::trimmed)]
    pub approver: String,
    /// Record the human's answer on their behalf: the tool they were asked with (`ask`, `AskUserQuestion`, `chat`); an agent role may run the call only with it
    #[arg(long, value_name = "NAME", value_parser = crate::cli::trimmed)]
    pub relayed_via: Option<String>,
    /// With --relayed-via, the agent's actor id (default: the CANON_ACTOR environment variable)
    #[arg(long, value_name = "ACTOR_ID", value_parser = crate::cli::trimmed)]
    pub actor: Option<String>,
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

/// The flags of `change reopen`: those of [`ChangeCloseArgs`], with `--ref` optional.
#[derive(Debug, Args)]
pub struct ChangeReopenArgs {
    /// The deferred change to bring back
    #[arg(value_name = "CHANGE_ID", value_parser = crate::subject::parse_change_id)]
    pub change_id: ChangeId,
    /// Why, in the human's words (one line)
    #[arg(long, value_name = "TEXT", value_parser = crate::cli::trimmed)]
    pub reason: String,
    /// What a reader can open to see why: a link, a commit, a file, the date of a conversation (one line; once per reference)
    #[arg(long = "ref", value_name = "TEXT", value_parser = crate::cli::trimmed)]
    pub refs: Vec<String>,
    /// Who decided (the human, not the agent)
    #[arg(long, value_name = "NAME", value_parser = crate::cli::trimmed)]
    pub approver: String,
    /// Record the human's answer on their behalf: the tool they were asked with (`ask`, `AskUserQuestion`, `chat`); an agent role may run the call only with it
    #[arg(long, value_name = "NAME", value_parser = crate::cli::trimmed)]
    pub relayed_via: Option<String>,
    /// With --relayed-via, the agent's actor id (default: the CANON_ACTOR environment variable)
    #[arg(long, value_name = "ACTOR_ID", value_parser = crate::cli::trimmed)]
    pub actor: Option<String>,
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
