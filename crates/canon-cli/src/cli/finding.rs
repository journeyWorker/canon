//! `canon finding` — the clap tree of the group.

use canon_model::{ChangeId, RoleId, ScenarioId};
use clap::{Args, Subcommand};
use crate::finding::CommitArg;
use std::path::PathBuf;
use super::PromoteArgs;

/// The flags of `context-pack new`.
#[derive(clap::Args)]
pub struct ContextPackNewArgs {
    /// Repository-relative input manifest
    #[arg(long)]
    pub manifest: PathBuf,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
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
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output the complete bundle manifest as JSON
    #[arg(long)]
    pub json: bool,
}

/// The flags of `demo add`.
#[derive(clap::Args)]
pub struct DemoAddArgs {
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum FindingCommand {
    /// Record one code-review finding (promoted in the same run under `write: { promote: auto }` or with --promote; under `write.promote: manual`, commit it with `canon gate promote`)
    #[command(after_help = "RECORDED OBSERVATION, NOT PROOF. canon does not verify that the defect\nexisted, that it was real, that it was fixed, or that --introduced-by names\nthe commit that actually caused it. This records that a named reviewer, at a\nstamped time, RAISED this finding against this change.\n\nAn agent that can run this command can record its own review round, with its\nown severities: the author of a finding and its beneficiary are the same\nparty, with no signature and no second party — the same gap `canon evidence\nadd` states about attestations.\n\nWhat canon does NOT check:\n  - --summary, against the diff, the code, or the review transcript. The\n    record is identical whether the defect was real, misdiagnosed, or invented\n  - --resolution-sha: never resolved, never read. `--disposition fixed`\n    records that someone SAID this commit closed it, not that it did\n  - --introduced-by: never verified, and never guessed (below)\n  - --reviewed-sha: optional, because most rounds review an uncommitted\n    working tree. When absent, canon does not infer one\n  - nothing here is gated. No `canon gate check` check consumes findings, and\n    no checkbox flips on one\n\nWhat the record IS good for:\n  - attribution: an append-only row naming who raised what, in which round,\n    against which change, at which severity and disposition. Before this\n    command, a review round left nothing at all once its transcript was gone\n  - a DERIVED count that stops being typed from memory. v0.4.0's release note\n    hand-typed its issue counts into a published tag and got them wrong; that\n    is why this command exists. No count is quoted here and none is asserted\n    in its place -- read them off the 'Review rounds' panel in\n    .canon/REPORT.md or the dashboard, which derive them from these records\n\n--introduced-by must be SOURCED:\n  Set it only when the introducing commit was actually established — someone\n  read the diff, or the fixing commit names it. Never from timing, commit\n  adjacency, or `git blame`. Leaving it UNSET is the correct record when it is\n  not known, not an incomplete one. Unset means UNSOURCED, never 'no cause'; a\n  guessed value would inflate the exact number this command exists to make\n  trustworthy.\n\nWhat a derived fix-of-fix count MEANS, stated here word for word as every\nother surface states it (canon_report::render::FIX_OF_FIX_MEANING) rather\nthan paraphrased, because paraphrase is how the previous wording went wrong\non seven surfaces at once:\n\n  `fix_of_fix` bounds NOTHING — not from below, not from above: it\n  UNDER-counts, because an unsourced finding is never counted and a fix in\n  one change that breaks something first found while reviewing a DIFFERENT\n  change is not counted at all; it OVER-counts, because a `resolution_sha`\n  commit may carry work BEYOND the fix and every finding recording that\n  commit is counted regardless; and for any individual match the data cannot\n  say whether the fix or the other work in that commit introduced the\n  defect.\n\nFix-of-fix is DERIVED, never recorded: a finding is one when its\nintroduced_by equals an EARLIER finding's resolution_sha (lower round/seq in\nthe same change). There is no flag for it and there will not be one — a flag\nwould relocate the hand-typing from the release notes into the record.\n\n--seq is REQUIRED, never auto-assigned: two concurrent adds would read the\nsame highest seq and both pick the next one, landing two records under one\nfinding's identity and inflating the count. An already-occupied\n(--change-id, --round, --seq) is refused, naming it.\n\nRefused before anything is staged:\n  - --disposition fixed with no --resolution-sha, and --resolution-sha with\n    any other disposition\n  - a line separator in --summary, --reviewer, --file-ref or --actor-id: a\n    finding is rendered as ONE row, so a separator appends a second row that\n    no record backs\n\nThe loop:\n  canon finding add ...     Record a Finding (promoted in the same run under\n                            `write: { promote: auto }`, which `canon init` writes)\nUnder `write.promote: manual` (no `write:` key), run `canon gate promote` to\ncommit it.")]
    Add(FindingAddArgs),
    /// Move an already-committed finding's disposition (promoted in the same run under `write: { promote: auto }` or with --promote; under `write.promote: manual`, commit it with `canon gate promote`)
    #[command(after_help = "A finding is raised `open` and later closed. This is the ONLY way to record\nthat second state: `canon finding add` refuses an occupied natural key,\nbecause two DIFFERENT findings under one (change, round, seq) are two\nreviewers' work collapsed into one identity.\n\nWhat it changes: the disposition, `resolution_sha` with it, and the closing\n--reason (required for rejected and deferred). NOTHING else.\nSeverity, reviewer, summary, --reviewed-sha, --introduced-by and --file-ref\nare READ from the committed record and copied byte-identically — this command\ncannot edit them, and no command can. A finding recorded wrongly stays\nwrongly recorded; that is the append-only ledger working, not a gap.\n\nThe committed record STAYS. This appends a second version at the same\nnatural key, and the pair IS the history: `open` at one instant, `fixed` at\nanother, each with its own author and timestamp. Every reader folds by\nnatural key to the latest version, so the finding is still counted once.\n\n`canon gate promote` re-derives the transition rule itself before committing\nthe second version, so a hand-written body must clear the same bar.\n\nExamples:\n  canon finding close --change-id s44-spec-derived-worklist --round 1 --seq 1 \\\n      --disposition fixed --resolution-sha $(git rev-parse HEAD)\n  canon finding close --change-id s44-spec-derived-worklist --round 1 --seq 9 \\\n      --disposition rejected")]
    Close(FindingCloseArgs),
    /// List a change's committed findings at their latest version: round, seq, severity, disposition, reviewer and summary
    List(FindingListArgs),
}

#[derive(Args)]
pub struct FindingAddArgs {
    /// The change under review (e.g. s43-findings-are-records) — first component of the natural key
    #[arg(long, value_parser = crate::finding::parse_change_id)]
    pub change_id: ChangeId,
    /// Which review round on this change raised it (1-based)
    #[arg(long)]
    pub round: u32,
    /// This finding's index within the round (1-based). Required, never auto-assigned; a collision is refused
    #[arg(long)]
    pub seq: u32,
    /// blocker / should-fix / note — the reviewer's own judgement, never a score canon derives
    #[arg(long, value_parser = crate::finding::severity_parser())]
    pub severity: canon_model::FindingSeverity,
    /// open / fixed / rejected / deferred (fixed requires --resolution-sha, and only fixed may carry one)
    #[arg(long, default_value = "open", value_parser = crate::finding::disposition_parser())]
    pub disposition: canon_model::FindingDisposition,
    /// Who raised it (line breaks refused)
    #[arg(long)]
    pub reviewer: String,
    /// One line of what the finding IS, in the reviewer's own words. NEVER checked against the code (line breaks refused)
    #[arg(long)]
    pub summary: String,
    /// The commit whose state this round reviewed. Omit when the round reviewed an uncommitted working tree — canon never infers one. A full or short sha; a short one is resolved through git to the full sha. Must name a COMMIT object this repo holds (existence only; never its message or diff)
    #[arg(long, value_parser = crate::finding::parse_commit)]
    pub reviewed_sha: Option<CommitArg>,
    /// The commit that closed it. Required by, and permitted only with, --disposition fixed. A full or short sha (resolved through git). Must name a COMMIT object this repo holds (existence only; never its message or diff)
    #[arg(long, value_parser = crate::finding::parse_commit)]
    pub resolution_sha: Option<CommitArg>,
    /// The SOURCED introducing commit. Leave unset when it is not known — never guess (see --help). A full or short sha (resolved through git). Must name a COMMIT object this repo holds (existence only; never its message or diff)
    #[arg(long, value_parser = crate::finding::parse_introduced_by)]
    pub introduced_by: Option<CommitArg>,
    /// Where in the tree, as path/to/file.rs:120-134 (line breaks refused)
    #[arg(long)]
    pub file_ref: Option<String>,
    /// A scenario this finding is about (repeatable). After a `fixed` close, fresh evidence for these scenarios alone re-verifies the fix; with none named, every evidenced scenario of the subject does. Each must be a scenario the index knows; a close inherits the list
    #[arg(long = "scenario-id", value_parser = crate::review::parse_scenario_id)]
    pub scenario_ids: Vec<ScenarioId>,
    /// The authoring actor's id (recorded as the record's author; line breaks refused)
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    /// Attribution only — unlike `canon evidence add`, no partition key derives from it
    #[arg(long, default_value = "reviewer", value_parser = crate::retrieve::parse_role)]
    pub role: RoleId,
    /// The authoring session (recorded as actor.session_id)
    #[arg(long, value_parser = crate::review::parse_session_id)]
    pub session_id: Option<canon_model::SessionId>,
    #[command(flatten)]
    pub promote: PromoteArgs,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct FindingCloseArgs {
    /// The change under review — first component of the natural key
    #[arg(value_name = "CHANGE_ID", value_parser = crate::finding::parse_change_id)]
    pub change_id: ChangeId,
    /// Which review round raised it (1-based)
    #[arg(long)]
    pub round: u32,
    /// The finding's index within the round (1-based)
    #[arg(long)]
    pub seq: u32,
    /// Where it lands: fixed / rejected / deferred / open (reopening is a real transition). fixed requires --resolution-sha, and only fixed may carry one
    #[arg(long, value_parser = crate::finding::disposition_parser())]
    pub disposition: canon_model::FindingDisposition,
    /// The commit that closed it. Required by, and permitted only with, --disposition fixed. A full or short sha (resolved through git). Must name a COMMIT object this repo holds
    #[arg(long, value_parser = crate::finding::parse_commit)]
    pub resolution_sha: Option<CommitArg>,
    /// Why it was closed, one line. Required for rejected and deferred, optional for fixed and open; stored on the closing version and shown by `finding list`
    #[arg(long)]
    pub reason: Option<String>,
    /// The actor recorded as authoring the TRANSITION — not the original finding's author, which is preserved on the record this supersedes
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    /// Attribution only — no partition key derives from it
    #[arg(long, default_value = "reviewer", value_parser = crate::retrieve::parse_role)]
    pub role: RoleId,
    /// The session that authored the TRANSITION (recorded as actor.session_id)
    #[arg(long, value_parser = crate::review::parse_session_id)]
    pub session_id: Option<canon_model::SessionId>,
    #[command(flatten)]
    pub promote: PromoteArgs,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct FindingListArgs {
    /// The change whose findings to list
    #[arg(long, value_parser = crate::finding::parse_change_id)]
    pub change_id: ChangeId,
    /// Only findings whose current disposition is open
    #[arg(long)]
    pub open: bool,
    /// Print `{change_id, findings: [...], meta}` as JSON
    #[arg(long)]
    pub json: bool,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    #[command(flatten)]
    pub page: crate::paging::PageFlags,
    /// With --json, keep only these finding fields (comma-separated): round, seq, severity, disposition, reviewer, summary, resolution_sha, file_ref
    #[arg(long, value_delimiter = ',')]
    pub fields: Option<Vec<String>>,
}
