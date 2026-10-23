//! `canon finding add` (s43 `findings-are-records`, task group 2): the
//! AUTHORING surface for `RecordKind::Finding`.
//!
//! # Why this command exists
//! v0.4.0 shipped after a long run of code-review rounds. Canon
//! recorded none of their findings — `canon_ingest::artifact_adapter`'s
//! `ArtifactEventKind::CodeReviewFinding` recognised them on the way to
//! a verdict and then had nowhere to put them. So the release summary's
//! issue counts were typed from memory into a git tag, and they were
//! wrong. Nothing caught it, because canon gates `EvidenceRecord`s and
//! checkboxes, and a narrative claim in a tag is ungated prose.
//!
//! No number from that tag is repeated here, and none is asserted in
//! its place. The counts live in `mart_review_rounds`, read off
//! `.canon/REPORT.md`'s "Review rounds" panel or the dashboard's — a
//! command built to stop hand-typed release metrics has no business
//! shipping one in its own help, and s43 round 2 (finding 4) found this
//! doc and the `--help` text below both doing exactly that, with a
//! number that was itself wrong.
//!
//! This command is one half of the fix. It does not lint prose. It
//! makes the number DERIVABLE, so that the next release note is written
//! by reading a generated table instead of by remembering.
//!
//! # What a record from here IS: a RECORDED OBSERVATION
//! Not proof of anything. Canon does not verify that the defect
//! existed, that it was real, that it was fixed, or that
//! `--introduced-rev` names the commit that actually caused it. State it
//! the blunt way, in the register `canon evidence add` set for
//! attestations (`crate::evidence`'s module doc), because this command
//! exists precisely because an unsupported claim shipped:
//!
//! - **`--summary` is whatever the author typed.** Canon does not read
//!   the diff, the review transcript, or the code. A finding saying
//!   "teardown races HMR" is recorded identically whether that race is
//!   real, misdiagnosed, or invented.
//! - **`--disposition fixed --resolution-rev <sha>` is not evidence the
//!   defect was fixed.** Canon never resolves the sha, never reads that
//!   commit, and never checks that it touches anything the finding
//!   mentions. It records that someone SAID this commit closed it.
//! - **`--introduced-rev` is not causation.** It is the author's sourced
//!   claim about which commit introduced the defect. Canon cannot check
//!   it, and deliberately never guesses it (below).
//! - **The author and the beneficiary are the same party**, exactly as
//!   in `canon evidence add`: an agent that can run this command can
//!   record its own review round, with its own severities.
//!
//! What the record IS good for, and it is worth having:
//! **attribution** — a permanent, append-only row naming who raised
//! what, in which round, against which change, with which disposition —
//! and a **derived count** that stops being typed from memory. Both are
//! a strict improvement on a review round's findings evaporating with
//! its transcript. Neither is verification.
//!
//! # A defect the author found in its own work
//! Record it like any other: `--actor-id` and `--actor-id` name the
//! author itself. Nothing refuses a finding whose reviewer is also the
//! change's implementer or evidence actor, and none should: hiding a
//! defect because nobody else saw it is exactly how a review round looks
//! cleaner than it was. An open `blocker` the author raised blocks under
//! `require_review.block_on_findings` like one a reviewer raised.
//! `--introduced-rev` is not where "the author found it" goes: it names
//! the commit that introduced the defect, so `--introduced-rev self` is
//! refused with a pointer to `--actor-id`.
//!
//! # `--introduced-rev` must be SOURCED, and unset is the CORRECT answer
//! This is the decision the CLI is where a user makes, so it is where
//! the guidance belongs. Set `--introduced-rev` only when the
//! introducing commit was actually established — someone read the diff,
//! or the fixing commit's own message names it. Do NOT set it from
//! timing, commit adjacency, `git blame` output, or "it was probably
//! the last thing that touched this file". Leaving it unset is not an
//! incomplete record; it is the accurate one, and canon-model
//! deliberately offers no "guess it" constructor.
//!
//! The consequence, which every consumer must repeat: `None` means
//! UNSOURCED, never "no cause". A guessed value would inflate exactly
//! the number this change exists to make trustworthy — which would be
//! the same failure as the one in the tag, relocated from prose into a
//! record where it would look authoritative.
//!
//! What a derived fix-of-fix count MEANS is one sentence, stated here
//! exactly as `canon_report::render::FIX_OF_FIX_MEANING` states it and
//! as every other surface repeats it — verbatim, never paraphrased,
//! because paraphrase is how the previous wording went wrong on seven
//! surfaces at once: this doc asserted a one-directional bound, and it
//! was wrong in the direction it did not name. The retired word is not
//! repeated here; `FIX_OF_FIX_MEANING`'s own doc records the history.
//!
//! `fix_of_fix` bounds NOTHING — not from below, not from above: it
//! UNDER-counts, because an unsourced finding is never counted and a
//! fix in one change that breaks something first found while reviewing
//! a DIFFERENT change is not counted at all; it OVER-counts, because a
//! `resolution_sha` commit may carry work BEYOND the fix and every
//! finding recording that commit is counted regardless; and for any
//! individual match the data cannot say whether the fix or the other
//! work in that commit introduced the defect.
//!
//! # Fix-of-fix is DERIVED here too — this command cannot record it
//! There is no `--fix-of-fix` flag and there will not be one. A finding
//! is a fix-of-fix when its `introduced_by` equals some EARLIER
//! finding's `resolution_sha` (earlier = lower `(round, seq)` within the
//! same `change_id`); the join computes it, nobody labels it. A flag
//! here would merely relocate the hand-typing from the release notes
//! into the record.
//!
//! # Staged, then promoted — but NOT for the reason evidence is
//! [`run_add`] writes to `crate::gate::evidence_staging_dir`, which
//! `canon gate promote` drains (`canon_gate::STAGED_KINDS` registers
//! `Finding` under `canon_gate::StagedAssignment::Nothing`). The loop:
//!
//! ```text
//! canon finding add --change-id <c> --round <n> --seq <n> ...
//! canon gate promote
//! ```
//!
//! `canon evidence add` stages because `EvidenceRecord.run_seq` does not
//! exist until promotion assigns it. That reason does NOT apply here: a
//! `Finding`'s natural key is `{change_id}__{round}__{seq}`, complete
//! the moment it is authored, and promotion commits the staged body
//! unchanged. What staging buys a finding instead is RE-VALIDATION
//! (`staging.read()` runs `canon_store::partition::validate_body`, whose
//! `Finding` arm is `Finding::from_body`) and batch atomicity for
//! an author staging a whole round before committing any of it. Stated
//! because the next reader will otherwise assume it mirrors evidence's
//! reason, and it does not.
//!
//! # `--seq` is REQUIRED, and a collision is refused HERE and enforced at PROMOTE
//! Auto-assigning `seq` from the highest existing one in
//! `(change_id, round)` would be friendlier and is the wrong trade.
//! Two concurrent adds (the normal case in this repo, which reviews in
//! parallel) both read the same maximum and both pick `N+1`. Their
//! bodies differ, so they resolve to two DIFFERENT files under the SAME
//! natural key, and the corpus then carries two distinct findings each
//! claiming to be round `r` finding `N+1` — one of which every reader
//! that folds by natural key silently drops. A race that quietly loses
//! a reviewer's finding is not a friendliness win.
//!
//! Required is also the more USEFUL option: a review artifact already
//! numbers its findings, so `--seq` is transcription rather than
//! invention, and an explicit seq means a ledger row can be matched
//! back against the artifact it came from. Under auto-assignment `seq`
//! would only record insertion order and carry no meaning at all.
//!
//! [`run_add`] also refuses a `(change_id, round, seq)` that any
//! readable staged or committed record already occupies, naming the
//! occupant — and that refusal is a DIAGNOSTIC, not the defence. s43
//! round 1 caught the earlier claim that it was: the occupancy scan
//! ([`occupied_by`]) and the `staging.write` below are two separate
//! steps with no lock between them, so two concurrent adds both observe
//! a free key, write two different bodies to two different paths (the
//! content digest is in the filename, so `GitTier::write`'s
//! duplicate-PATH check does not fire), and both succeed. No amount of
//! rescanning here closes that; a check that runs before a write it
//! does not hold a lock over can only ever narrow the window.
//!
//! Uniqueness is therefore ENFORCED where records become durable:
//! `canon_gate::promote`'s `NaturalKeyRule::Unique` reads both tiers
//! and refuses a second, DIFFERENT body at an occupied key, leaving it
//! staged and named for a human rather than committing it. What THIS
//! check buys is the fast, well-worded refusal for the ordinary
//! sequential case — the one an author hits by mis-transcribing a seq,
//! which is far more common than the race — at the moment the author
//! can still fix it, instead of two commands later.
//!
//! # A finding is raised open and CLOSED later: `canon finding close`
//! The uniqueness rule above says what a second body at an occupied key
//! must not be. It leaves open what a second body legitimately IS — and
//! there is exactly one such body: the same finding, transitioned.
//!
//! A finding's life is `open` → `fixed`/`rejected`/`deferred`, and
//! every reader folds by natural key to the LATEST version precisely so
//! the disposition columns report the state NOW. Until [`run_close`]
//! there was no way to author that second version at all, so a repo
//! that recorded its findings at review time — the honest moment, while
//! the reviewer still has them — could never record that it fixed them.
//! The disposition columns could only ever report the state each
//! finding was BORN in, and `.canon/REPORT.md`'s own prose about
//! folding an `open` finding to `fixed` described a state the CLI
//! structurally could not produce.
//!
//! [`run_close`] is a separate verb rather than a flag on [`run_add`]
//! for one structural reason: it READS the committed record and copies
//! every content field, so a close CANNOT alter the severity, the
//! reviewer, the summary, or either sourced sha. Re-typing them on
//! `add` would turn a typo into promote's "two records under one
//! identity" refusal — the correct refusal for the wrong reason. A
//! finding recorded wrongly stays wrongly recorded; that is the
//! append-only ledger working, not a gap.
//!
//! The exemption is enforced at the AUTHORITY, not here:
//! `canon_gate::promote` re-derives
//! `canon_model::Finding::is_disposition_transition_of` against the
//! CURRENT committed version before admitting the append, so a
//! hand-written body must clear the same bar as this command's output.
//!
//! # Reading them back: `canon finding list` (0.18, #34)
//! [`run_list`] prints a change's committed findings at their latest
//! version — round, seq, severity, disposition, reviewer, summary —
//! optionally only the open ones, or as JSON. Before it, a reviewer
//! looking for the next free seq or the open blockers grepped ledger
//! JSON, and one such loop read the wrong file.
//!
//! # Line breaks are refused, not escaped
//! `--summary`, `--actor-id`, `--file-ref` and `--actor-id` are the
//! free-text fields of this record — the ones with no grammar
//! constraining them — and they are what a rendered finding row is
//! built from. A row is ONE line, so a line separator in any of them
//! does not produce a longer row; it produces a SECOND row, which no
//! record backs. s42's review found this exact vector against
//! `canon evidence add --summary`, where `$'ok\n- [x] 9.9 Forged task'`
//! appended a fully checked task row.
//!
//! Refused at the authoring boundary rather than escaped at each
//! renderer: there is no escaping that makes a two-line finding summary
//! meaningful in a one-line row, silently rewriting a reviewer's words
//! is worse than refusing them, and a refusal here holds for every
//! consumer instead of being re-derived correctly by each one.
//!
//! The character set is IMPORTED —
//! `canon_ingest::task_rows::first_row_line_break` over
//! `ROW_LINE_BREAKS`, the identical predicate `canon evidence add` and
//! `canon_ingest::reject_multi_line_note` use. Never restated here: two
//! copies of a "which characters break a row" set are two copies that
//! can disagree, and the one that disagrees is the one that lets a
//! forged row through.
//!
//! # A named commit is resolved by git (0.28 D7b)
//! `--rev`, `--resolution-rev` and `--introduced-rev` take any git
//! revision (`HEAD~1`, a branch, a tag, a short sha) and are resolved
//! with [`crate::git_rev`]: `git rev-parse --verify <rev>^{commit}`, the
//! FULL sha is recorded, and a revision git cannot resolve (also: outside
//! a git work tree, or in a repo with no commit) is `NO_COMMIT`, exit 2.
//! `--rev` and `--resolution-rev` also record the code tree they saw
//! (`code_tree`, `resolution_code_tree`) and `worktree_dirty`: the
//! default `HEAD` is the working tree as it stands, so a round that
//! reviewed uncommitted code is recorded, never refused. Only
//! `--introduced-rev` has no default: it must be sourced, never guessed.
//! Canon never reads a commit's message, diff or files.
//!
//! # Coherence is enforced by the TYPE; this command adds the diagnostic
//! `FindingDisposition::Fixed` is unreachable without a closing sha on
//! every construction path canon-model has — `Finding::fixed_by` is the
//! only constructor that sets it, `rejected`/`deferred` clear
//! `resolution_sha`, the pair is PRIVATE so no clone-and-mutate reaches
//! it, and `Finding`'s hand-written `Deserialize` re-checks it. So an
//! incoherent record is unconstructible through the API [`run_add`]
//! builds with, and there is deliberately NO second coherence check
//! here that could drift from `Finding::check_coherence`. What this
//! command adds is the LOUD, early diagnostic: a `--resolution-rev`
//! alongside any disposition but `fixed` is refused by name before
//! anything is staged, rather than surfacing two commands later as a
//! `malformed` record. `fixed` takes `--resolution-rev` or its default
//! `HEAD`, so it can no longer lack a commit.
//!
//! # Exit-code contract
//! `0` staged; `2` usage-or-infra (a malformed flag combination, a line
//! separator in a free-text field, an empty required field, a sha this
//! repository does not hold, an occupied natural key, a tier write
//! failure). A malformed SHA never reaches [`run_add`] at all —
//! [`parse_commit`] is a clap `value_parser`, so clap refuses it by flag
//! name and grammar and exits `2` before this module runs.
//!
//! There is deliberately no `1`. `crate::evidence` grades `1` for "the
//! authored record would be gate-red", and no gate consumes findings:
//! grading a code nothing can produce would be a claim about a check
//! that does not exist.

use std::path::Path;
use canon_gate::GateCtx;
use canon_ingest::task_rows::first_row_line_break;
use canon_model::{Actor, ChangeId, Envelope, Finding, FindingDisposition, FindingSeverity, RawRecord, RecordKind, RoleId, ScenarioId, SessionId, Sha};
use canon_store::fold::fold_latest_by_key;
use canon_store::git_tier::GitTier;
use canon_store::partition::{content_digest12, resolve_partition};
use canon_store::tier::{RawWrite, Tier, TierQuery};
use chrono::Utc;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli_error::CliError;
use crate::context::resolve_repo_root;
use crate::gate::evidence_staging_dir;

/// `--severity`'s accepted values, in blocking order: `blocker` must be
/// resolved before the reviewed work is claimed done, `should-fix` may be
/// scheduled, `note` carries no obligation. `canon context` lists these.
pub const SEVERITY_VALUES: [&str; 3] = ["blocker", "should-fix", "note"];

/// `--disposition`'s accepted values. `canon context` lists these.
pub const DISPOSITION_VALUES: [&str; 4] = ["open", "fixed", "rejected", "deferred"];

/// `--severity` for clap, with the accepted values known to it.
pub fn severity_parser() -> impl clap::builder::TypedValueParser<Value = FindingSeverity> {
    crate::cli_error::closed_set(SEVERITY_VALUES, parse_severity)
}

/// `--disposition` for clap, with the accepted values known to it.
pub fn disposition_parser() -> impl clap::builder::TypedValueParser<Value = FindingDisposition> {
    crate::cli_error::closed_set(DISPOSITION_VALUES, parse_disposition)
}

/// `--severity`'s clap `value_parser`. Kebab-cased on the CLI, matching
/// `crate::divergence::parse_status`'s established spelling for a
/// snake_case-serialized model enum, and exhaustive over
/// [`FindingSeverity`].
pub fn parse_severity(s: &str) -> Result<FindingSeverity, String> {
    match s {
        "blocker" => Ok(FindingSeverity::Blocker),
        "should-fix" => Ok(FindingSeverity::ShouldFix),
        "note" => Ok(FindingSeverity::Note),
        other => Err(format!("`{other}` is not a finding severity — expected one of: {}", SEVERITY_VALUES.join(", "))),
    }
}

/// `--disposition`'s clap `value_parser`, exhaustive over
/// [`FindingDisposition`].
pub fn parse_disposition(s: &str) -> Result<FindingDisposition, String> {
    match s {
        "open" => Ok(FindingDisposition::Open),
        "fixed" => Ok(FindingDisposition::Fixed),
        "rejected" => Ok(FindingDisposition::Rejected),
        "deferred" => Ok(FindingDisposition::Deferred),
        other => Err(format!("`{other}` is not a finding disposition — expected one of: {}", DISPOSITION_VALUES.join(", "))),
    }
}

/// `--change-id`'s clap `value_parser` — grammar-level
/// [`ChangeId::parse`], the same shape `crate::query::parse_change_id`
/// and `crate::subject::parse_change_id` already expose.
pub fn parse_change_id(s: &str) -> Result<ChangeId, String> {
    ChangeId::parse(s).map_err(|e| e.to_string())
}

/// `--introduced-rev`'s value parser: any revision, except that `self` (the
/// spelling an author reaches for when recording its own defect) is refused
/// with a pointer to the option that records who found it.
pub fn parse_introduced_rev(s: &str) -> Result<String, String> {
    if s == "self" {
        return Err("--introduced-rev names the COMMIT that introduced the defect, not who found it; to record a defect you found in your own work, set --actor-id to your own id, and leave --introduced-rev unset unless the introducing commit was sourced".to_string());
    }
    if s.trim().is_empty() {
        return Err("--introduced-rev must name a git revision".to_string());
    }
    Ok(s.to_string())
}

/// One `canon finding add` invocation's already-parsed flags.
///
/// A named struct rather than a positional parameter list, for
/// `crate::evidence::EvidenceArgs`'s reason and more acutely: thirteen
/// arguments, of which three are revisions and four are plain
/// strings — `rev`/`resolution_rev`/`introduced_rev` mean three
/// completely different things and would all compile if transposed,
/// and transposing them would silently produce a WRONG fix-of-fix
/// derivation rather than an error.
pub struct FindingArgs {
    /// The change under review — the finding's scope and the first
    /// component of its natural key.
    pub change_id: ChangeId,
    /// `--rev`: the commit whose state this round reviewed, any git
    /// revision. The default `HEAD` is the working tree as it stands, so a
    /// round that reviewed uncommitted code is recorded with its code tree
    /// and `worktree_dirty`, never refused. Recorded as `reviewed_sha`.
    pub rev: String,
    /// Which review round on this change raised it (1-based).
    pub round: u32,
    /// This finding's index within `round` (1-based). REQUIRED and
    /// collision-checked — see the module doc for why it is not
    /// auto-assigned.
    pub seq: u32,
    pub severity: FindingSeverity,
    /// The disposition to record. Only `fixed` may carry
    /// `resolution_sha`, and it MUST (module doc's coherence section).
    pub disposition: FindingDisposition,
    /// Who raised it — free text, since a reviewer may be a human name
    /// no canon [`RoleId`] models. Refused when it carries a line
    /// separator (module doc).
    pub reviewer: String,
    /// One line of what the finding IS, in the reviewer's own words.
    /// Never checked against anything (module doc's observation
    /// section). Refused when it carries a line separator.
    pub summary: String,
    /// `--resolution-rev`: the commit that closed it. Permitted only with
    /// `disposition == Fixed`, where `None` means `HEAD`.
    pub resolution_rev: Option<String>,
    /// The SOURCED introducing commit, or `None`. `None` means
    /// UNSOURCED — never "no cause" — and is one of the two reasons a
    /// derived fix-of-fix count UNDER-counts. See the module doc's
    /// canonical sentence before setting this.
    pub introduced_rev: Option<String>,
    /// Where in the tree, as `path/to/file.rs:120-134`. Refused when it
    /// carries a line separator.
    pub file_ref: Option<String>,
    /// The scenarios this finding is about (`--scenario-id`, repeatable;
    /// [`Finding::scenario_ids`]). Each must exist in the scenario index;
    /// empty names none, and the whole subject re-verifies a `fixed` fix.
    pub scenario_ids: Vec<ScenarioId>,
    /// The authoring actor's id — the ATTRIBUTION half of what this
    /// command buys. Refused when it carries a line separator.
    pub actor_id: String,
    /// The role the record is attributed to.
    ///
    /// Defaulted, unlike `canon evidence add`'s REQUIRED `--role`, and
    /// the difference is not stylistic: promotion derives an
    /// `EvidenceRecord`'s `run_seq` partition from `actor.role`, so a
    /// defaulted role there would silently mis-partition a sequence. A
    /// `Finding` has no `run_seq` and no partition derived from role,
    /// so this is attribution only and a wrong default costs nothing
    /// but an inaccurate author label.
    pub role: RoleId,
    /// The authoring session, recorded as `actor.session_id`.
    pub session_id: Option<SessionId>,
    /// `--promote`/`--no-promote` (0.16 D2; [`crate::write_mode`]).
    pub promote: crate::write_mode::PromoteFlag,
    /// `--json`: one JSON document on stdout, every human line on stderr.
    pub json: bool,
}

/// The envelope actor for a record this module stages: the id, the role,
/// and the session when one was given.
fn authoring_actor(actor_id: &str, role: &RoleId, session_id: Option<&SessionId>) -> Actor {
    let actor = Actor::new(actor_id, role.clone());
    match session_id {
        Some(session_id) => actor.with_session(session_id.clone()),
        None => actor,
    }
}

/// `canon finding add`: the finding staged or committed.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindingAddReport<'a> {
    #[serde(flatten)]
    pub landing: crate::retry::Landing,
    pub change_id: &'a str,
    pub round: u32,
    pub seq: u32,
    pub severity: &'static str,
    pub disposition: &'static str,
    /// No `--introduced-rev` was given: the finding names no commit that introduced it.
    pub unsourced: bool,
}

impl crate::reports::Report for FindingAddReport<'_> {}

/// `canon finding close`: the transition staged or committed, or a no-op.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindingCloseReport<'a> {
    /// Where the transition landed; absent on a no-op, which writes nothing.
    #[serde(flatten)]
    pub landing: Option<crate::retry::Landing>,
    pub change_id: &'a str,
    pub round: u32,
    pub seq: u32,
    /// The disposition it moved from; absent when unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<&'static str>,
    pub disposition: &'static str,
}

impl crate::reports::Report for FindingCloseReport<'_> {}

/// One finding of `canon finding list`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindingRow<'a> {
    pub round: u32,
    pub seq: u32,
    pub severity: &'static str,
    pub disposition: &'static str,
    pub reviewer: &'a str,
    pub summary: &'a str,
    pub resolution_sha: Option<&'a str>,
    pub file_ref: Option<&'a str>,
    pub reason: Option<&'a str>,
}

/// `canon finding list`: a change's findings; paging is in `meta`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindingListReport<'a> {
    pub change_id: &'a str,
    pub findings: Vec<FindingRow<'a>>,
}

impl crate::reports::Report for FindingListReport<'_> {}

/// `canon finding list --fields`: each finding keeps only the requested members.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindingListFieldsReport<'a> {
    pub change_id: &'a str,
    pub findings: Vec<crate::json_out::Projected<FindingRow<'static>>>,
}

impl crate::reports::Report for FindingListFieldsReport<'_> {}

/// `canon finding add` (module doc). Returns the process exit code.
///
/// Order matters and is not incidental: every refusal below happens
/// BEFORE the single `staging.write` at the end, so a refused
/// invocation leaves the staging directory byte-identical — the same
/// "a refused add writes nothing" property `crate::evidence::run_add`
/// and `crate::review::run_add` hold.
///
/// Idempotent in the way that matters, and unlike `canon evidence add`:
/// a second add at an already-occupied `(change_id, round, seq)` is
/// REFUSED rather than staged as a second record. Two attestations at
/// two times are two pieces of evidence, but two records claiming to be
/// the same numbered finding are a corrupted count.
pub fn run_add(repo: &Path, args: &FindingArgs) -> i32 {
    let repo = resolve_repo_root(repo);
    let promote = match crate::write_mode::should_promote(&repo, args.promote) {
        Ok(promote) => promote,
        Err(error) => {
            return CliError::refused("CONFIG_UNREADABLE", error.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit("canon finding add", args.json);
        }
    };

    if args.round == 0 || args.seq == 0 {
        return CliError::refused("FINDING_INVALID_ARGUMENT", format!("--round and --seq are 1-based (got --round {} --seq {}); a 0 would sort ahead of the first real finding in a key whose whole job is ordering rounds",
            args.round, args.seq
        )).hint("--round and --seq start at 1; pass --round 1 --seq 1 for the first finding").emit("canon finding add", args.json);
    }

    for (flag, value) in [("--actor-id", args.reviewer.as_str()), ("--summary", args.summary.as_str())] {
        if value.trim().is_empty() {
            return CliError::refused("FINDING_INVALID_ARGUMENT", format!("{flag} must be non-empty; a finding with no {} is a row that records nothing", &flag[2..])).hint("`canon schema finding add` lists the accepted arguments and their forms").emit("canon finding add", args.json);
        }
    }

    // The free-text fields, refused before anything is staged (module
    // doc's line-break section). Checked through the row grammar's OWN
    // separator set rather than a local `contains('\n')`, so this
    // refusal cannot disagree with `canon evidence add`'s or with
    // `canon_ingest::reject_multi_line_note`'s about which inputs are
    // safe to put in a row.
    for (flag, value) in [
        ("--summary", Some(args.summary.as_str())),
        ("--actor-id", Some(args.reviewer.as_str())),
        ("--file-ref", args.file_ref.as_deref()),
        ("--actor-id", Some(args.actor_id.as_str())),
    ] {
        let Some((offset, separator)) = value.and_then(first_row_line_break) else { continue };
        return CliError::refused("FINDING_INVALID_ARGUMENT", format!("{flag} carries the line separator {separator:?} at byte offset {offset}; a finding is rendered as ONE row, so a separator there appends a second row that no finding record backs")).hint("`canon schema finding add` lists the accepted arguments and their forms").emit("canon finding add", args.json);
    }

    // The coherence pair, as a NAMED diagnostic rather than a second
    // enforcement layer (module doc): `fixed` takes `--resolution-rev` or
    // its default `HEAD`, so only the other direction can be wrong.
    if let (disposition, Some(rev)) = (args.disposition, &args.resolution_rev) {
        if disposition != FindingDisposition::Fixed {
            return CliError::refused("FINDING_RESOLUTION_REV_UNEXPECTED", format!("--resolution-rev {rev} was given with --disposition {}; only a `fixed` finding was closed by a commit, so drop the revision or set --disposition fixed",
                disposition.as_str()
            )).hint("drop --resolution-rev, or set --disposition fixed").field("--resolution-rev").emit("canon finding add", args.json);
        }
    }

    // Every commit option is resolved by git to the full sha the record
    // stores (module doc); the code tree each saw is recorded with it.
    // Placed after the pure-string refusals and before anything is
    // constructed, so a refused add still writes nothing.
    let commits = match resolve_commits(&repo, args) {
        Ok(commits) => commits,
        Err(refusal) => {
            return refusal.emit("canon finding add", args.json);
        }
    };

    // Constructed through the builder that makes the pair above
    // unconstructible: `fixed_by` is the ONLY path to `Fixed` and takes
    // the sha by value, `rejected`/`deferred` clear it.
    let mut finding = Finding::new(
        Envelope::current(RecordKind::Finding, Utc::now(), authoring_actor(&args.actor_id, &args.role, args.session_id.as_ref())),
        args.change_id.clone(),
        args.round,
        args.seq,
        args.severity,
        args.reviewer.as_str(),
        args.summary.as_str(),
    );
    finding = match (args.disposition, commits.resolution) {
        (FindingDisposition::Fixed, Some(resolution)) => finding.fixed_by(resolution.sha).with_resolution_code(&resolution.code_tree, resolution.worktree_dirty),
        (FindingDisposition::Rejected, _) => finding.rejected(),
        (FindingDisposition::Deferred, _) => finding.deferred(),
        // `Open` is what `Finding::new` already produced.
        (FindingDisposition::Open, _) => finding,
        // Unreachable: `resolve_commits` binds `HEAD` for a `fixed` finding
        // with no `--resolution-rev`. Handled rather than assumed away.
        (FindingDisposition::Fixed, None) => unreachable!("a fixed finding always resolves a commit"),
    };
    finding = finding.reviewing_sha(commits.reviewed.sha).with_code(&commits.reviewed.code_tree, commits.reviewed.worktree_dirty);
    if let Some(sha) = commits.introduced {
        finding = finding.with_introduced_by(sha);
    }
    if let Some(file_ref) = &args.file_ref {
        finding = finding.with_file_ref(file_ref.as_str());
    }
    if !args.scenario_ids.is_empty() {
        if let Err(refusal) = check_scenarios_exist(&repo, &args.scenario_ids) {
            return refusal.emit("canon finding add", args.json);
        }
        finding = finding.with_scenario_ids(args.scenario_ids.iter().cloned());
    }

    let body = serde_json::to_value(&finding).expect("a Finding always serializes");
    let natural_key = match resolve_partition(RecordKind::Finding, &body) {
        Ok(key) => key.natural_key,
        Err(violation) => {
            return CliError::refused("FINDING_INVALID_ARGUMENT", violation.detail.to_string()).hint("`canon schema finding add` lists the accepted arguments and their forms").emit("canon finding add", args.json);
        }
    };

    let ledger_root = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx.ledger_root,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit("canon finding add", args.json);
        }
    };
    let staging = GitTier::new(evidence_staging_dir(&ledger_root));
    let committed = GitTier::new(&ledger_root);
    // An identical add repeated (same key, same content, whoever the clock
    // says) is a repeat: nothing is staged and the existing record is reported.
    let same_finding = |stored: &serde_json::Value| resolve_partition(RecordKind::Finding, stored).is_ok_and(|key| key.natural_key == natural_key);
    if let Some(existing) = crate::retry::repeats_latest(&ledger_root, Some(&evidence_staging_dir(&ledger_root)), RecordKind::Finding.as_str(), &body, same_finding, None) {
        crate::say!(args.json, "canon finding add: unchanged — an identical finding (`{}`, round {}, seq {}) is already {}; nothing written", args.change_id, args.round, args.seq, existing.describe());
        crate::json_out::mark_unchanged();
        let record = FindingAddReport {
            landing: existing.landing(),
            change_id: args.change_id.as_str(),
            round: args.round,
            seq: args.seq,
            severity: severity_slug(args.severity),
            disposition: args.disposition.as_str(),
            unsourced: args.introduced_rev.is_none(),
        };
        crate::next::finish(&repo, "finding add", &crate::next::Scope::Change(args.change_id.as_str().to_string()), false, args.json, &record);
        return 0;
    }
    match occupied_by(&natural_key, &staging, &committed) {
        Ok(Some(occupant)) => {
            return CliError::refused("FINDING_SEQ_OCCUPIED", format!("{} is already occupied by a {occupant} finding (`{}`, round {}, seq {}); pick the next free --seq rather than authoring a second record under one finding's identity. To move THAT finding's disposition instead, use `canon finding close`.",
                natural_key, args.change_id, args.round, args.seq
            )).hint("pick the next free --seq, or move the existing finding with `canon finding close`").emit("canon finding add", args.json);
        }
        Ok(None) => {}
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon finding add", args.json);
        }
    }

    match staging.write(&RawWrite(RawRecord(body))) {
        Ok(receipt) => {
            let settled = crate::gate::promote_after_stage("canon finding add", &ledger_root, &receipt.location, promote, args.json);
            crate::say!(
                args.json,
                "canon finding add: {} — {} round {} seq {} ({}, {}) — {}",
                settled.landed(&receipt.location),
                args.change_id,
                args.round,
                args.seq,
                severity_slug(args.severity),
                args.disposition.as_str(),
                settled.suffix()
            );
            if args.introduced_rev.is_none() {
                // Said on the success path, not only in `--help`: this
                // is the moment the author still remembers whether the
                // introducing commit was sourced, and an unsourced
                // record is one of the two reasons the derived count
                // UNDER-counts (module doc's canonical sentence).
                crate::say!(
                    args.json,
                    "canon finding add: note — no --introduced-rev, so this finding is UNSOURCED for fix-of-fix derivation. That is the correct record when the introducing commit was not established; never guess one."
                );
            }
            if settled.exit_code() == 0 {
                let record = FindingAddReport {
                    landing: settled.landing(&receipt.location),
                    change_id: args.change_id.as_str(),
                    round: args.round,
                    seq: args.seq,
                    severity: severity_slug(args.severity),
                    disposition: args.disposition.as_str(),
                    unsourced: args.introduced_rev.is_none(),
                };
                crate::next::finish(&repo, "finding add", &crate::next::Scope::Change(args.change_id.as_str().to_string()), matches!(settled, crate::gate::StagedOutcome::Staged), args.json, &record);
            }
            settled.exit_code()
        }
        Err(e) => {
            CliError::refused("WRITE_FAILED", e.to_string()).hint("check that the path is writable and the disk has room, then run the command again").emit("canon finding add", args.json)
        }
    }
}

/// `canon finding close`'s inputs — the natural key of an
/// already-committed finding, plus the disposition to move it to.
///
/// Deliberately NOT [`FindingArgs`] with an extra flag. Every content
/// field is READ from the committed record rather than re-typed, so a
/// close structurally cannot alter the severity, the reviewer, the
/// summary, or either sourced sha: the only thing this command can
/// change is the one thing it is for. Re-typing them would turn a typo
/// into `canon_gate::promote`'s "two records under one identity"
/// refusal — the correct refusal for the wrong reason.
#[derive(Debug, Clone)]
pub struct FindingCloseArgs {
    pub change_id: ChangeId,
    pub round: u32,
    pub seq: u32,
    /// Where the finding lands. `open` is legal — a finding reopened
    /// after a fix that did not hold is a real transition, and refusing
    /// it would leave the ledger able to record only one direction.
    pub disposition: FindingDisposition,
    /// `--resolution-rev`; `None` means `HEAD` when the disposition is `fixed`.
    pub resolution_rev: Option<String>,
    /// Why it was closed (0.25, #98): required for `rejected` and
    /// `deferred`, optional otherwise, always one line.
    pub reason: Option<String>,
    pub actor_id: String,
    pub role: RoleId,
    /// The session that authored the transition, recorded as
    /// `actor.session_id`.
    pub session_id: Option<SessionId>,
    /// `--promote`/`--no-promote` (0.16 D2; [`crate::write_mode`]).
    pub promote: crate::write_mode::PromoteFlag,
    /// `--json`: one JSON document on stdout, every human line on stderr.
    pub json: bool,
}

/// The command shape `FINDING_REASON_REQUIRED`'s hint shows: this close,
/// with the one flag it lacks.
fn reason_shape(args: &FindingCloseArgs) -> String {
    format!(
        "canon finding close {} --round {} --seq {} --disposition {} --reason \"<why, one line>\"",
        args.change_id,
        args.round,
        args.seq,
        args.disposition.as_str()
    )
}

/// Stage the DISPOSITION TRANSITION of one already-committed finding
/// (`canon finding close`).
///
/// # Why this is a second verb and not a flag on `add`
/// [`run_add`] refuses an occupied natural key, and must: two different
/// findings under one `{change_id}__{round}__{seq}` are two reviewers'
/// work collapsed into one identity. But a finding's whole life is
/// `open` → `fixed`/`rejected`/`deferred`, and every reader folds by
/// natural key to the LATEST version precisely so the disposition
/// columns report the state NOW. Before this verb there was no way to
/// author that second version, so a repo that recorded its findings at
/// review time — the honest moment, while the reviewer still has them —
/// could never record that it fixed them. The disposition columns could
/// only ever report the state a finding was BORN in.
///
/// # The append, and what stays
/// This writes a NEW record; the committed one stays exactly where it
/// is. The ledger is append-only and the pair IS the history: `open` at
/// one instant, `fixed` at another, both attributable. `canon gate
/// promote` admits the second only after re-deriving
/// [`Finding::is_disposition_transition_of`] itself, so the exemption is
/// enforced at the authority and a hand-written body must clear the
/// same bar as this command's output.
pub fn run_close(repo: &Path, args: &FindingCloseArgs) -> i32 {
    let repo = resolve_repo_root(repo);
    let promote = match crate::write_mode::should_promote(&repo, args.promote) {
        Ok(promote) => promote,
        Err(error) => {
            return CliError::refused("CONFIG_UNREADABLE", error.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit("canon finding close", args.json);
        }
    };

    if args.round == 0 || args.seq == 0 {
        return CliError::refused("FINDING_INVALID_ARGUMENT", format!("--round and --seq are 1-based (got --round {} --seq {}); no finding was ever authored under a 0",
            args.round, args.seq
        )).hint("--round and --seq start at 1; pass --round 1 --seq 1 for the first finding").emit("canon finding close", args.json);
    }

    // The same coherence pair `add` enforces, in this command's own
    // voice. It runs BEFORE the lookup so a caller who named the wrong
    // flags is told that, rather than being told a finding they spelled
    // correctly does not exist.
    if let (disposition, Some(rev)) = (args.disposition, &args.resolution_rev) {
        if disposition != FindingDisposition::Fixed {
            return CliError::refused("FINDING_RESOLUTION_REV_UNEXPECTED", format!("--resolution-rev {rev} was given with --disposition {}; only a `fixed` finding was closed by a commit, so drop the revision or set --disposition fixed",
                disposition.as_str()
            )).hint("drop --resolution-rev, or set --disposition fixed").field("--resolution-rev").emit("canon finding close", args.json);
        }
    }

    // `--reason`, refused before any lookup or write: owed by `rejected`
    // and `deferred`, never blank, always one line.
    let owed = matches!(args.disposition, FindingDisposition::Rejected | FindingDisposition::Deferred);
    match args.reason.as_deref() {
        None if owed => {
            return CliError::refused("FINDING_REASON_REQUIRED", format!("--disposition {0} requires --reason; a {0} finding with no recorded why leaves the next reader nothing to go on", args.disposition.as_str())).hint(format!("run `{}`", reason_shape(args))).field("--reason").emit("canon finding close", args.json);
        }
        Some(reason) if reason.trim().is_empty() => {
            return CliError::refused("FINDING_REASON_REQUIRED", "--reason must be non-empty text; a blank reason records no why").hint(format!("run `{}`", reason_shape(args))).field("--reason").emit("canon finding close", args.json);
        }
        Some(reason) => {
            if let Some((offset, separator)) = first_row_line_break(reason) {
                return CliError::refused("FINDING_REASON_REQUIRED", format!("--reason carries the line separator {separator:?} at byte offset {offset}; a reason is ONE line")).hint(format!("run `{}` with the reason on one line", reason_shape(args))).field("--reason").emit("canon finding close", args.json);
            }
        }
        None => {}
    }

    let resolution = match (args.disposition, args.resolution_rev.as_deref()) {
        (FindingDisposition::Fixed, rev) => match crate::git_rev::bind(&repo, rev.unwrap_or(crate::git_rev::DEFAULT_REV)) {
            Ok(bound) => Some(bound),
            Err(refusal) => return refusal.emit("canon finding close", args.json),
        },
        _ => None,
    };

    let ledger_root = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx.ledger_root,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit("canon finding close", args.json);
        }
    };
    let staging = GitTier::new(evidence_staging_dir(&ledger_root));
    let committed = GitTier::new(&ledger_root);

    // A STAGED occupant is refused rather than transitioned. The
    // original was never committed, so there is nothing to supersede —
    // and `canon gate promote` deliberately refuses two transitions of
    // one key in a single drain, since they have no defined order
    // between them. Editing the staged copy is the honest repair.
    match find_finding(args, &staging) {
        Ok(Some(_)) => {
            return CliError::refused("FINDING_STAGED", format!("`{}` round {} seq {} is still STAGED, never committed, so there is no recorded disposition to move. Run `canon gate promote` first, or delete the staged copy and re-author it with `canon finding add --disposition {}`.",
                args.change_id,
                args.round,
                args.seq,
                args.disposition.as_str()
            )).hint("run `canon gate promote` first, then close the finding").emit("canon finding close", args.json);
        }
        Ok(None) => {}
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon finding close", args.json);
        }
    }

    let existing = match find_finding(args, &committed) {
        Ok(Some(finding)) => finding,
        Ok(None) => {
            return CliError::refused("FINDING_NOT_FOUND", format!("no committed finding at (`{}`, round {}, seq {}). `close` moves an EXISTING finding's disposition; authoring a new one is `canon finding add`.",
                args.change_id, args.round, args.seq
            )).hint("`canon finding list` shows the committed findings; author a new one with `canon finding add`").emit("canon finding close", args.json);
        }
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon finding close", args.json);
        }
    };

    if existing.disposition() == args.disposition {
        // Not an error: re-running a close is how a caller recovers from
        // a half-finished batch. But it stages NOTHING — an identical
        // body at one key is not a transition, `promote` would refuse
        // it, and saying so here is cheaper than a refusal two commands
        // later.
        crate::say!(
            args.json,
            "canon finding close: no-op — (`{}`, round {}, seq {}) is already `{}`; nothing staged.",
            args.change_id,
            args.round,
            args.seq,
            args.disposition.as_str()
        );
        crate::json_out::mark_unchanged();
        let noop = FindingCloseReport { landing: None, change_id: args.change_id.as_str(), round: args.round, seq: args.seq, from: None, disposition: args.disposition.as_str() };
        crate::next::finish(&repo, "finding close", &crate::next::Scope::Change(args.change_id.as_str().to_string()), false, args.json, &noop);
        return 0;
    }

    // Rebuilt FROM the committed record, so every content field survives
    // byte-identically and only the disposition pair moves. The envelope
    // is fresh: the transition happened NOW, authored by whoever ran
    // this, and stamping the original author's `at` would lose exactly
    // the fact that makes the pair a history.
    let mut transitioned = Finding::new(
        Envelope::current(RecordKind::Finding, Utc::now(), authoring_actor(&args.actor_id, &args.role, args.session_id.as_ref())),
        existing.change_id.clone(),
        existing.round,
        existing.seq,
        existing.severity,
        existing.reviewer.as_str(),
        existing.summary.as_str(),
    );
    transitioned = match (args.disposition, resolution) {
        (FindingDisposition::Fixed, Some(resolution)) => transitioned.fixed_by(resolution.sha).with_resolution_code(&resolution.code_tree, resolution.worktree_dirty),
        (FindingDisposition::Rejected, _) => transitioned.rejected(),
        (FindingDisposition::Deferred, _) => transitioned.deferred(),
        (FindingDisposition::Open, _) => transitioned,
        // Unreachable: a `fixed` close always binds a commit above.
        (FindingDisposition::Fixed, None) => unreachable!("a fixed close always resolves a commit"),
    };
    if let Some(sha) = existing.reviewed_sha.clone() {
        transitioned = transitioned.reviewing_sha(sha);
    }
    if let (Some(tree), Some(dirty)) = (&existing.code_tree, existing.worktree_dirty) {
        transitioned = transitioned.with_code(tree, dirty);
    }
    if let Some(sha) = existing.introduced_by.clone() {
        transitioned = transitioned.with_introduced_by(sha);
    }
    if let Some(file_ref) = &existing.file_ref {
        transitioned = transitioned.with_file_ref(file_ref.as_str());
    }
    if let Some(reason) = &args.reason {
        transitioned = transitioned.with_reason(reason.trim());
    }
    // The scenarios it is about carry over: a close takes no list of its
    // own, and dropping them would widen the re-verification to the suite.
    transitioned = transitioned.with_scenario_ids(existing.scenario_ids.iter().cloned());

    // The predicate `canon gate promote` will re-derive, asserted here
    // so a divergence between this construction and the exemption fails
    // LOUDLY at authoring time rather than as a refusal the operator
    // cannot act on.
    debug_assert!(
        transitioned.is_disposition_transition_of(&existing),
        "a close must construct a body that `promote`'s exemption accepts"
    );

    let body = serde_json::to_value(&transitioned).expect("a Finding always serializes");
    match staging.write(&RawWrite(RawRecord(body))) {
        Ok(receipt) => {
            let settled = crate::gate::promote_after_stage("canon finding close", &ledger_root, &receipt.location, promote, args.json);
            crate::say!(
                args.json,
                "canon finding close: {} — {} round {} seq {} ({} → {}) — {}",
                settled.landed(&receipt.location),
                args.change_id,
                args.round,
                args.seq,
                existing.disposition().as_str(),
                args.disposition.as_str(),
                settled.suffix()
            );
            if settled.exit_code() == 0 {
                let record = FindingCloseReport {
                    landing: Some(settled.landing(&receipt.location)),
                    change_id: args.change_id.as_str(),
                    round: args.round,
                    seq: args.seq,
                    from: Some(existing.disposition().as_str()),
                    disposition: args.disposition.as_str(),
                };
                crate::next::finish(&repo, "finding close", &crate::next::Scope::Change(args.change_id.as_str().to_string()), matches!(settled, crate::gate::StagedOutcome::Staged), args.json, &record);
            }
            settled.exit_code()
        }
        Err(e) => {
            CliError::refused("WRITE_FAILED", e.to_string()).hint("check that the path is writable and the disk has room, then run the command again").emit("canon finding close", args.json)
        }
    }
}

/// `canon finding list` (0.18, #34).
#[derive(Debug, Clone)]
pub struct FindingListArgs {
    pub change_id: ChangeId,
    /// Only findings whose CURRENT disposition is `open`.
    pub open: bool,
    pub json: bool,
    pub page: crate::paging::Page,
    /// `--fields`: the row fields `--json` keeps.
    pub fields: Option<Vec<String>>,
}

/// The current version of every committed finding on `change_id`, one
/// per `(round, seq)`, ordered by round then seq. The fold is
/// [`fold_latest_by_key`], the rule [`find_finding`] and the report use,
/// so a closed finding lists with its closing disposition, never the
/// one it was raised with.
fn current_findings(tier: &GitTier, change_id: &ChangeId) -> Result<Vec<Finding>, canon_store::tier::StoreError> {
    let versions: Vec<(Finding, String)> = tier
        .read(&TierQuery::kind(RecordKind::Finding))?
        .records
        .into_iter()
        .filter_map(|raw| {
            let finding = Finding::from_body(&raw.0).ok()?;
            (&finding.change_id == change_id).then(|| (finding, content_digest12(&raw.0)))
        })
        .collect();
    let mut current: Vec<Finding> =
        fold_latest_by_key(versions, |(f, _)| (f.round, f.seq), |(f, _)| f.envelope.at, |(f, _)| f.envelope.schema, |(_, digest)| digest.as_str())
            .into_values()
            .map(|(finding, _)| finding)
            .collect();
    current.sort_by_key(|f| (f.round, f.seq));
    Ok(current)
}

/// Print the findings recorded against one change (`canon finding
/// list`, 0.18 #34): round, seq, severity, disposition, reviewer and
/// summary per finding, each at its latest version
/// ([`current_findings`]). `--open` keeps the findings whose current
/// disposition is `open`; `--json` prints `{change_id, findings: [...]}`.
///
/// Committed findings only: a staged one is not yet a record, and its
/// count is named on stderr with the command that commits it. Exit `0`
/// whether or not anything matched; `2` when the ledger cannot be read.
pub fn run_list(repo: &Path, args: &FindingListArgs) -> i32 {
    let repo = resolve_repo_root(repo);
    let ledger_root = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx.ledger_root,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit("canon finding list", args.json);
        }
    };
    let read = current_findings(&GitTier::new(&ledger_root), &args.change_id)
        .and_then(|committed| Ok((committed, current_findings(&GitTier::new(evidence_staging_dir(&ledger_root)), &args.change_id)?.len())));
    let (mut findings, staged) = match read {
        Ok(read) => read,
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon finding list", args.json);
        }
    };
    if args.open {
        findings.retain(|f| f.disposition() == FindingDisposition::Open);
    }
    if let Some(requested) = &args.fields {
        let known: std::collections::BTreeSet<String> = LIST_FIELDS.iter().map(|f| f.to_string()).collect();
        if let Err(e) = crate::paging::check_fields("canon finding list", requested, &known) {
            return e.emit("canon finding list", args.json);
        }
    }
    let scope = format!("finding list:{}", args.change_id);
    let (findings, meta) = match crate::paging::paginate(&scope, findings, |f| format!("{}-{}", f.round, f.seq), &args.page) {
        Ok(paged) => paged,
        Err(e) => return e.emit("canon finding list", args.json),
    };

    if args.json {
        let rows = findings.iter().map(|f| FindingRow {
            round: f.round,
            seq: f.seq,
            severity: severity_slug(f.severity),
            disposition: f.disposition().as_str(),
            reviewer: &f.reviewer,
            summary: &f.summary,
            resolution_sha: f.resolution_sha().map(|sha| sha.as_str()),
            file_ref: f.file_ref.as_deref(),
            reason: f.reason.as_deref(),
        });
        match &args.fields {
            Some(fields) => {
                let findings = rows
                    .map(|row| crate::json_out::Projected::new(crate::paging::project(&serde_json::to_value(row).expect("a finding row serializes"), fields)))
                    .collect();
                crate::json_out::data_with_meta("finding list", &FindingListFieldsReport { change_id: args.change_id.as_str(), findings }, meta.to_json());
            }
            None => crate::json_out::data_with_meta("finding list", &FindingListReport { change_id: args.change_id.as_str(), findings: rows.collect() }, meta.to_json()),
        }
    } else if findings.is_empty() {
        let which = if args.open { "open findings" } else { "findings" };
        println!("canon finding list: no committed {which} on change `{}`", args.change_id);
    } else {
        print!("{}", render_finding_table(&findings));
        if let Some(line) = meta.human_line() {
            println!("{line}");
        }
    }
    if staged > 0 {
        crate::say_err!(args.json, "canon finding list: {staged} staged finding(s) on change `{}` are not listed until committed — run `canon gate promote`", args.change_id);
    }
    0
}

/// The JSON fields of one `finding list` row, which `--fields` may name.
const LIST_FIELDS: [&str; 9] = ["round", "seq", "severity", "disposition", "reviewer", "summary", "resolutionSha", "fileRef", "reason"];

/// One header row and one row per finding, columns padded to their
/// widest cell; the summary, last, is never padded and carries the
/// closing reason, when there is one, as a trailing `[reason: …]`.
fn render_finding_table(findings: &[Finding]) -> String {
    const LIST_COLUMNS: usize = 6;
    let header: [&str; LIST_COLUMNS] = ["round", "seq", "severity", "disposition", "reviewer", "summary"];
    let rows: Vec<[String; LIST_COLUMNS]> = findings
        .iter()
        .map(|f| {
            [
                f.round.to_string(),
                f.seq.to_string(),
                severity_slug(f.severity).to_string(),
                f.disposition().as_str().to_string(),
                f.reviewer.clone(),
                match &f.reason {
                    Some(reason) => format!("{} [reason: {reason}]", f.summary),
                    None => f.summary.clone(),
                },
            ]
        })
        .collect();
    let mut widths = header.map(str::len);
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let line = |cells: [&str; LIST_COLUMNS]| {
        let mut out = String::new();
        for (i, cell) in cells.iter().enumerate() {
            if i + 1 == cells.len() {
                out.push_str(cell);
            } else {
                out.push_str(cell);
                out.push_str(&" ".repeat(widths[i] - cell.chars().count() + 2));
            }
        }
        out.push('\n');
        out
    };
    let mut out = line(header);
    for row in &rows {
        out.push_str(&line([&row[0], &row[1], &row[2], &row[3], &row[4], &row[5]]));
    }
    out
}

/// The CURRENT [`Finding`] `tier` holds under `args`' natural key, if
/// any — the latest version, never merely the first one read.
///
/// # Why a fold and not a first match
/// A finding's key can legitimately carry more than one committed
/// record now: that is exactly what a transition appends. Reading the
/// first match would answer with whichever version the walk reached
/// first, so a second close would compare against the ORIGINAL `open`
/// body, miss the no-op, and stage a third record that says nothing new.
/// The fold is `canon_store::fold::fold_latest_by_key` — the ONE
/// supersession rule every reader of this corpus already uses, so this
/// command's idea of "the current disposition" is the same one
/// `.canon/REPORT.md` prints.
///
/// # What it matches on
/// The PARSED record's own key fields, not a re-`format!`ed composite
/// string — [`occupied_by`]'s "never a second `format!` of the
/// composite" rule turned one step further. That function must compare
/// key strings because it works from raw bodies; this one already
/// deserializes to return a `Finding` at all, so comparing the three
/// typed fields is both cheaper and immune to the key grammar changing
/// underneath it.
///
/// A record too malformed to parse as a `Finding` is invisible here for
/// [`occupied_by`]'s stated reason: it carries no readable body, every
/// consumer of the corpus already skips it, and `canon gate promote`
/// refuses it on its own.
fn find_finding(args: &FindingCloseArgs, tier: &GitTier) -> Result<Option<Finding>, canon_store::tier::StoreError> {
    let versions: Vec<(Finding, String)> = tier
        .read(&TierQuery::kind(RecordKind::Finding))?
        .records
        .into_iter()
        .filter_map(|raw| {
            let finding = Finding::from_body(&raw.0).ok()?;
            (finding.change_id == args.change_id && finding.round == args.round && finding.seq == args.seq)
                .then(|| (finding, content_digest12(&raw.0)))
        })
        .collect();
    Ok(fold_latest_by_key(versions, |_| (), |(f, _)| f.envelope.at, |(f, _)| f.envelope.schema, |(_, digest)| digest.as_str())
        .into_values()
        .map(|(finding, _)| finding)
        .next())
}

/// Which tier, if either, already carries a readable finding at
/// `natural_key` — the DIAGNOSTIC half of the `--seq` collision story
/// (module doc), never the enforcement half.
///
/// It cannot be the enforcement half and the module doc says why: this
/// scan and [`run_add`]'s later `staging.write` are separate steps, so a
/// concurrent add can slip between them. `canon_gate::promote`'s
/// `NaturalKeyRule::Unique` is where "one key, one record" is actually
/// made to hold.
///
/// Scoped honestly: it scans the records each tier can READ. A record
/// so malformed that the tier reports it as a violation instead is not
/// matched here — it carries no body to derive a key from, it is
/// invisible to every consumer of the corpus, and `canon gate promote`
/// refuses it on its own. The key is derived with
/// `canon_store::partition::resolve_partition`, the same function the
/// layout itself uses, never a second `format!` of the composite.
fn occupied_by(natural_key: &str, staging: &GitTier, committed: &GitTier) -> Result<Option<&'static str>, canon_store::tier::StoreError> {
    for (label, tier) in [("staged", staging), ("committed", committed)] {
        for raw in tier.read(&TierQuery::kind(RecordKind::Finding))?.records {
            if resolve_partition(RecordKind::Finding, &raw.0).is_ok_and(|key| key.natural_key == natural_key) {
                return Ok(Some(label));
            }
        }
    }
    Ok(None)
}

/// Refuse a `--scenario-id` the scenario index does not know, naming it,
/// so a finding never scopes a re-verification to a scenario that cannot
/// be evidenced. Matched by id across spec roots, as the finding keys it.
fn check_scenarios_exist(repo: &Path, ids: &[ScenarioId]) -> Result<(), CliError> {
    let unreadable = |e: String| {
        CliError::refused("CONFIG_UNREADABLE", e).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again")
    };
    let ctx = GateCtx::from_repo(repo).map_err(|e| unreadable(e.to_string()))?;
    let context = canon_gate::GateContext::load(ctx, &canon_policy::SchemaRegistry::load(), Utc::now()).map_err(|e| unreadable(e.to_string()))?;
    let indexed = canon_gate::spec_coverage::latest_scenarios(&context);
    let missing: Vec<String> = ids.iter().filter(|id| !indexed.iter().any(|s| &s.scenario_id == *id)).map(|id| format!("`{}`", id.as_str())).collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(CliError::refused(
        "FINDING_UNKNOWN_SCENARIO",
        format!("no scenario record for {}; a finding names scenarios the index knows, so fresh evidence for them can re-verify its fix", missing.join(", ")),
    )
    .field("--scenario-id")
    .hint("run `canon inventory sync`, or name an id from `canon query --kind scenario`, then run `canon finding add` again"))
}

/// The commit options of one `canon finding add`, each resolved to the full
/// sha its record stores.
struct ResolvedCommits {
    /// `--rev`, with the code tree it saw.
    reviewed: crate::git_rev::Bound,
    /// `--resolution-rev`, or `HEAD` for a `fixed` finding; `None` for any other disposition.
    resolution: Option<crate::git_rev::Bound>,
    /// `--introduced-rev`; `None` when it was not sourced.
    introduced: Option<Sha>,
}

/// Resolve every commit option through [`crate::git_rev`], or the refusal
/// (`NO_COMMIT`) of the first that does not resolve (module doc).
fn resolve_commits(repo: &Path, args: &FindingArgs) -> Result<ResolvedCommits, CliError> {
    let reviewed = crate::git_rev::bind(repo, &args.rev)?;
    let resolution = match (args.disposition, args.resolution_rev.as_deref()) {
        (FindingDisposition::Fixed, rev) => Some(crate::git_rev::bind(repo, rev.unwrap_or(crate::git_rev::DEFAULT_REV))?),
        _ => None,
    };
    let introduced = match args.introduced_rev.as_deref() {
        Some(rev) => {
            let sha = crate::git_rev::resolve_rev(repo, rev)?;
            Some(Sha::parse(&sha).map_err(|e| crate::git_rev::no_commit(format!("`{rev}` resolves to `{sha}`, which is not a 40-digit sha: {e}")))?)
        }
        None => None,
    };
    Ok(ResolvedCommits { reviewed, resolution, introduced })
}

/// A [`FindingSeverity`]'s operator-facing spelling, matching
/// [`parse_severity`]'s own accepted domain. An exhaustive match rather
/// than a `Debug` render, so a variant added later has to choose its
/// wording here instead of silently leaking a Rust identifier —
/// `crate::evidence::verdict_slug`'s established shape. (There is no
/// counterpart for [`FindingDisposition`]: canon-model already exposes
/// `FindingDisposition::as_str`, and a second spelling of it here is
/// exactly the drift this pattern exists to avoid.)
fn severity_slug(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Blocker => "blocker",
        FindingSeverity::ShouldFix => "should-fix",
        FindingSeverity::Note => "note",
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use super::*;

    fn args(change_id: &str, round: u32, seq: u32) -> FindingArgs {
        FindingArgs {
            change_id: ChangeId::parse(change_id).unwrap(),
            rev: "HEAD".to_string(),
            round,
            seq,
            severity: FindingSeverity::Blocker,
            disposition: FindingDisposition::Open,
            reviewer: "reviewer-1".to_string(),
            summary: "the release count was typed from memory".to_string(),
            resolution_rev: None,
            introduced_rev: None,
            file_ref: None,
            scenario_ids: Vec::new(),
            actor_id: "canon".to_string(),
            role: RoleId::parse("reviewer").unwrap(),
            session_id: None,
            promote: crate::write_mode::PromoteFlag::Config,
            json: false,
        }
    }

    fn repo() -> tempfile::TempDir {
        git_repo().0
    }

    fn files() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::write(dir.path().join("canon.yaml"), "routing:\n  finding: local\n").expect("writing canon.yaml");
        dir
    }

    fn staged_bodies(repo: &Path) -> Vec<serde_json::Value> {
        let staging = GitTier::new(evidence_staging_dir(&GateCtx::from_repo(repo).unwrap().ledger_root));
        staging.read(&TierQuery::kind(RecordKind::Finding)).expect("reading staging").records.into_iter().map(|raw| raw.0).collect()
    }

    /// The fixture files committed as one git commit, which every
    /// revision-binding command needs.
    fn git_repo() -> (tempfile::TempDir, Sha) {
        let dir = files();
        let git = |args: &[&str]| {
            let status = Command::new("git").arg("-C").arg(dir.path()).args(args).status().expect("git must be on PATH for this test");
            assert!(status.success(), "git {args:?} failed");
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "test@canon.invalid"]);
        git(&["config", "user.name", "canon test"]);
        git(&["add", "canon.yaml"]);
        git(&["commit", "-q", "-m", "initial"]);
        let out = Command::new("git").arg("-C").arg(dir.path()).args(["rev-parse", "HEAD"]).output().expect("git rev-parse");
        let head = Sha::parse(String::from_utf8(out.stdout).unwrap().trim()).expect("HEAD is a 40-hex sha");
        (dir, head)
    }

    /// The three revision flags with the mutation that sets each one.
    /// `--resolution-rev` drags `--disposition fixed` along, or the
    /// coherence refusal fires first and the revision check never runs.
    fn rev_flags() -> [(&'static str, fn(&mut FindingArgs, &str)); 3] {
        [
            ("--rev", |a: &mut FindingArgs, r: &str| a.rev = r.to_string()),
            ("--introduced-rev", |a: &mut FindingArgs, r: &str| a.introduced_rev = Some(r.to_string())),
            ("--resolution-rev", |a: &mut FindingArgs, r: &str| {
                a.disposition = FindingDisposition::Fixed;
                a.resolution_rev = Some(r.to_string());
            }),
        ]
    }

    /// A revision that names no commit is refused on every flag, and a
    /// refused add stages nothing.
    #[test]
    fn a_revision_that_names_no_commit_is_refused_on_every_flag() {
        for revision in ["f".repeat(40), "no-such-branch".to_string()] {
            for (label, apply) in rev_flags() {
                let (dir, _head) = git_repo();
                let mut fabricating = args("s43-findings-are-records", 2, 10);
                apply(&mut fabricating, &revision);
                assert_eq!(run_add(dir.path(), &fabricating), 2, "{label} accepted `{revision}`");
                assert!(staged_bodies(dir.path()).is_empty(), "{label}: a refused add must stage nothing");
            }
        }
    }

    /// Outside a git work tree there is nothing to bind to: refused.
    #[test]
    fn outside_a_git_checkout_the_add_is_refused() {
        let dir = files();
        assert!(!dir.path().join(".git").exists(), "the premise: this fixture is not a checkout");
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 2, 12)), 2);
        assert!(staged_bodies(dir.path()).is_empty());
    }

    /// A revision this repository holds goes through on every flag, in
    /// any spelling git resolves, and is stored as the full sha.
    #[test]
    fn a_revision_this_repository_holds_is_accepted_and_stored_in_full() {
        let (dir, head) = git_repo();
        let mut resolvable = args("s43-findings-are-records", 2, 11);
        resolvable.rev = head.as_str()[..8].to_string();
        resolvable.introduced_rev = Some("HEAD".to_string());
        resolvable.disposition = FindingDisposition::Fixed;
        resolvable.resolution_rev = Some("main".to_string());
        assert_eq!(run_add(dir.path(), &resolvable), 0);
        let staged = staged_bodies(dir.path());
        assert_eq!(staged.len(), 1);
        for field in ["reviewed_sha", "introduced_by", "resolution_sha"] {
            assert_eq!(staged[0][field].as_str(), Some(head.as_str()), "{field}");
        }
    }

    /// The round-trip contract at the authoring surface: a record this
    /// command STAGED is a record the tier's own read returns, with no
    /// violations. `staging.read()` runs the identical
    /// `canon_store::partition::validate_body` the committed read path
    /// does, so a body canon wrote and canon cannot read would surface
    /// here as a violation rather than a record.
    #[test]
    fn every_staged_record_reads_back_without_a_violation() {
        let dir = repo();

        for (seq, apply) in [
            (1, (|_: &mut FindingArgs| {}) as fn(&mut FindingArgs)),
            (2, |a: &mut FindingArgs| {
                a.disposition = FindingDisposition::Fixed;
                a.resolution_rev = Some("HEAD".to_string());
            }),
            (3, |a: &mut FindingArgs| a.disposition = FindingDisposition::Rejected),
            (4, |a: &mut FindingArgs| a.disposition = FindingDisposition::Deferred),
        ] {
            let mut authored = args("s43-findings-are-records", 9, seq);
            apply(&mut authored);
            assert_eq!(run_add(dir.path(), &authored), 0, "seq {seq}");
        }

        let staging = GitTier::new(evidence_staging_dir(&GateCtx::from_repo(dir.path()).unwrap().ledger_root));
        let read_back = staging.read(&TierQuery::kind(RecordKind::Finding)).expect("reading staging");
        assert!(read_back.violations.is_empty(), "canon wrote records canon cannot read: {:?}", read_back.violations);
        assert_eq!(read_back.records.len(), 4);
    }

    /// The whole point of `--seq` being required: the second add at an
    /// occupied key is REFUSED, not staged. Without this, two records
    /// would sit under one finding's identity and inflate the count s43
    /// exists to make trustworthy.
    #[test]
    fn a_second_add_at_an_occupied_natural_key_is_refused_naming_the_key() {
        let dir = repo();
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 1, 1)), 0);

        let mut clashing = args("s43-findings-are-records", 1, 1);
        // A DIFFERENT body at the SAME key — the case `GitTier::write`'s
        // own duplicate-path check cannot catch, because the content
        // digest in the filename differs.
        clashing.summary = "a completely different finding".to_string();
        assert_eq!(run_add(dir.path(), &clashing), 2);

        assert_eq!(staged_bodies(dir.path()).len(), 1, "the clashing add must stage nothing");
    }

    /// A different `seq` in the same round is a different finding and
    /// must go through — the collision check must not be a blanket
    /// per-round refusal.
    #[test]
    fn a_different_seq_in_the_same_round_stages_normally() {
        let dir = repo();
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 1, 1)), 0);
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 1, 2)), 0);
        assert_eq!(staged_bodies(dir.path()).len(), 2);
    }

    /// An add with no `--rev` binds HEAD and records the code tree.
    #[test]
    fn an_add_without_rev_records_head_and_its_code_tree() {
        let (dir, head) = git_repo();
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 8, 1)), 0);

        let body = &staged_bodies(dir.path())[0];
        assert_eq!(body["reviewed_sha"].as_str(), Some(head.as_str()), "{body}");
        assert!(body["code_tree"].is_string(), "{body}");
        assert!(body["worktree_dirty"].is_boolean(), "{body}");
        assert!(body.get("introduced_by").is_none(), "{body}");
        assert!(body.get("resolution_sha").is_none(), "{body}");
        assert_eq!(body["change_id"], "s43-findings-are-records");
        assert_eq!(body["round"], 8);
        assert_eq!(body["disposition"], "open");
    }

    /// The `Fixed` path builds through `fixed_by`, so the staged body
    /// carries both halves of the biconditional.
    #[test]
    fn a_fixed_finding_stages_with_its_resolution_sha() {
        let (dir, head) = git_repo();
        let mut fixed = args("s43-findings-are-records", 2, 1);
        fixed.disposition = FindingDisposition::Fixed;
        fixed.resolution_rev = Some("HEAD".to_string());
        fixed.introduced_rev = Some("HEAD".to_string());
        assert_eq!(run_add(dir.path(), &fixed), 0);

        let body = &staged_bodies(dir.path())[0];
        assert_eq!(body["disposition"], "fixed");
        assert_eq!(body["resolution_sha"].as_str(), Some(head.as_str()));
        assert_eq!(body["introduced_by"].as_str(), Some(head.as_str()));
        assert!(body["resolution_code_tree"].is_string(), "{body}");
    }

    /// `fixed` with no `--resolution-rev` resolves HEAD.
    #[test]
    fn fixed_without_a_resolution_rev_defaults_to_head() {
        let (dir, head) = git_repo();
        let mut fixed = args("s43-findings-are-records", 3, 1);
        fixed.disposition = FindingDisposition::Fixed;
        assert_eq!(run_add(dir.path(), &fixed), 0);
        assert_eq!(staged_bodies(dir.path())[0]["resolution_sha"].as_str(), Some(head.as_str()));
    }

    /// Every non-`Fixed` disposition, not just `Open` — `rejected` and
    /// `deferred` clear `resolution_sha` in the model, so a CLI that
    /// only refused the `Open` case would silently DROP a sha the
    /// caller supplied.
    #[test]
    fn a_resolution_sha_with_any_non_fixed_disposition_is_refused() {
        for disposition in [FindingDisposition::Open, FindingDisposition::Rejected, FindingDisposition::Deferred] {
            let dir = repo();
            let mut incoherent = args("s43-findings-are-records", 4, 1);
            incoherent.disposition = disposition;
            incoherent.resolution_rev = Some("HEAD".to_string());
            assert_eq!(run_add(dir.path(), &incoherent), 2, "{disposition:?} must not accept a resolution sha");
            assert!(staged_bodies(dir.path()).is_empty(), "{disposition:?} staged something");
        }
    }

    /// Every member of the IMPORTED set, over every free-text field —
    /// so a later edit that narrows the check to `'\n'`, or that misses
    /// a field, fails here.
    #[test]
    fn every_row_line_break_in_every_free_text_field_is_refused() {
        for separator in canon_ingest::task_rows::ROW_LINE_BREAKS {
            for field in ["summary", "reviewer", "file_ref", "actor_id"] {
                let dir = repo();
                let mut forged = args("s43-findings-are-records", 5, 1);
                let injected = format!("ok{separator}| forged | row |");
                match field {
                    "summary" => forged.summary = injected,
                    "reviewer" => forged.reviewer = injected,
                    "file_ref" => forged.file_ref = Some(injected),
                    _ => forged.actor_id = injected,
                }
                assert_eq!(run_add(dir.path(), &forged), 2, "{field} accepted separator {separator:?}");
                assert!(staged_bodies(dir.path()).is_empty(), "{field} staged a forged row for {separator:?}");
            }
        }
    }

    #[test]
    fn an_empty_reviewer_or_summary_is_refused() {
        for blank in ["", "   "] {
            let dir = repo();
            let mut empty_summary = args("s43-findings-are-records", 6, 1);
            empty_summary.summary = blank.to_string();
            assert_eq!(run_add(dir.path(), &empty_summary), 2);

            let mut empty_reviewer = args("s43-findings-are-records", 6, 1);
            empty_reviewer.reviewer = blank.to_string();
            assert_eq!(run_add(dir.path(), &empty_reviewer), 2);
            assert!(staged_bodies(dir.path()).is_empty());
        }
    }

    /// `round`/`seq` are 1-based in the model and zero-padded into the
    /// natural key; a `0` would sort ahead of every real finding.
    #[test]
    fn a_zero_round_or_seq_is_refused() {
        let dir = repo();
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 0, 1)), 2);
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 1, 0)), 2);
        assert!(staged_bodies(dir.path()).is_empty());
    }

    #[test]
    fn severity_slug_round_trips_through_parse_severity() {
        for severity in [FindingSeverity::Blocker, FindingSeverity::ShouldFix, FindingSeverity::Note] {
            assert_eq!(parse_severity(severity_slug(severity)), Ok(severity), "{severity:?}");
        }
    }

    #[test]
    fn parse_disposition_accepts_exactly_the_models_own_wire_spellings() {
        for disposition in [FindingDisposition::Open, FindingDisposition::Fixed, FindingDisposition::Rejected, FindingDisposition::Deferred] {
            assert_eq!(parse_disposition(disposition.as_str()), Ok(disposition), "{disposition:?}");
        }
        assert!(parse_disposition("fix-of-fix").is_err(), "fix-of-fix is DERIVED and must never be an authorable disposition");
    }

    fn close_args(change_id: &str, round: u32, seq: u32, disposition: FindingDisposition) -> FindingCloseArgs {
        FindingCloseArgs {
            change_id: ChangeId::parse(change_id).unwrap(),
            round,
            seq,
            disposition,
            resolution_rev: None,
            reason: matches!(disposition, FindingDisposition::Rejected | FindingDisposition::Deferred).then(|| "by design".to_string()),
            actor_id: "canon".to_string(),
            role: RoleId::parse("reviewer").unwrap(),
            session_id: None,
            promote: crate::write_mode::PromoteFlag::Config,
            json: false,
        }
    }

    fn committed_bodies(repo: &Path) -> Vec<serde_json::Value> {
        let committed = GitTier::new(GateCtx::from_repo(repo).unwrap().ledger_root);
        committed.read(&TierQuery::kind(RecordKind::Finding)).expect("reading ledger").records.into_iter().map(|raw| raw.0).collect()
    }

    /// Commit whatever is staged, the way `canon gate promote` would,
    /// so the close path under test sees a COMMITTED occupant. Written
    /// through the tier rather than by moving files, so the committed
    /// copy lands at its own content-derived path.
    fn promote_staged(repo: &Path) {
        let ledger_root = GateCtx::from_repo(repo).unwrap().ledger_root;
        let staging = GitTier::new(evidence_staging_dir(&ledger_root));
        let committed = GitTier::new(&ledger_root);
        for raw in staging.read(&TierQuery::kind(RecordKind::Finding)).expect("reading staging").records {
            committed.write(&RawWrite(raw)).expect("committing the staged finding");
        }
        std::fs::remove_dir_all(evidence_staging_dir(&ledger_root)).expect("draining staging");
    }

    /// The gap s46 closes. A finding raised `open` at review time — the
    /// honest moment — must be closeable, and before `close` existed the
    /// disposition columns could only ever report the state a finding
    /// was BORN in.
    #[test]
    fn close_stages_a_transition_that_preserves_every_content_field() {
        let (dir, head) = git_repo();
        let mut raised = args("s44-spec-derived-worklist", 1, 1);
        raised.file_ref = Some("crates/canon-cli/src/finding.rs:1-2".to_string());
        raised.introduced_rev = Some(head.clone().to_string());
        raised.rev = head.clone().to_string();
        assert_eq!(run_add(dir.path(), &raised), 0);
        promote_staged(dir.path());

        let mut close = close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Fixed);
        close.resolution_rev = Some(head.clone().to_string());
        close.actor_id = "someone-who-fixed-it".to_string();
        assert_eq!(run_close(dir.path(), &close), 0);

        let staged = staged_bodies(dir.path());
        assert_eq!(staged.len(), 1);
        let body = &staged[0];
        assert_eq!(body["disposition"].as_str(), Some("fixed"));
        assert_eq!(body["resolution_sha"].as_str(), Some(head.as_str()));
        for (field, expected) in [
            ("severity", "blocker"),
            ("reviewer", "reviewer-1"),
            ("summary", "the release count was typed from memory"),
            ("file_ref", "crates/canon-cli/src/finding.rs:1-2"),
        ] {
            assert_eq!(body[field].as_str(), Some(expected), "`{field}` is READ from the committed record, never re-typed");
        }
        assert_eq!(body["introduced_by"].as_str(), Some(head.as_str()), "the sourced introducing commit survives the transition");
        assert_eq!(body["reviewed_sha"].as_str(), Some(head.as_str()));
        assert_eq!(body["actor"]["agent_id"].as_str(), Some("someone-who-fixed-it"), "the TRANSITION's author is recorded, not the original finding's");
    }

    /// The committed record is untouched: the ledger is append-only and
    /// the pair IS the history.
    #[test]
    fn close_never_edits_the_record_it_supersedes() {
        let (dir, head) = git_repo();
        assert_eq!(run_add(dir.path(), &args("s44-spec-derived-worklist", 1, 1)), 0);
        promote_staged(dir.path());
        let before = committed_bodies(dir.path());

        let mut close = close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Fixed);
        close.resolution_rev = Some(head.to_string());
        assert_eq!(run_close(dir.path(), &close), 0);
        assert_eq!(committed_bodies(dir.path()), before, "close appends; it must never rewrite the committed corpus");
    }

    /// Re-running a close is how a caller recovers from a half-finished
    /// batch, so it exits 0 — but it must stage NOTHING, because an
    /// identical disposition carries no new fact and `canon gate
    /// promote` would refuse the append.
    #[test]
    fn closing_to_the_current_disposition_is_a_no_op_that_stages_nothing() {
        let (dir, head) = git_repo();
        assert_eq!(run_add(dir.path(), &args("s44-spec-derived-worklist", 1, 1)), 0);
        promote_staged(dir.path());

        let mut close = close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Fixed);
        close.resolution_rev = Some(head.to_string());
        assert_eq!(run_close(dir.path(), &close), 0);
        promote_staged(dir.path());

        assert_eq!(run_close(dir.path(), &close), 0, "a repeated close is recovery, not an error");
        assert!(staged_bodies(dir.path()).is_empty(), "the current disposition is already `fixed`, so there is nothing to append");
    }

    /// The no-op is judged against the CURRENT version. Reading the
    /// first record at the key instead would compare a second close
    /// against the original `open` body and stage a third record saying
    /// nothing new.
    #[test]
    fn the_current_disposition_is_the_latest_version_not_the_first_read() {
        let (dir, head) = git_repo();
        assert_eq!(run_add(dir.path(), &args("s44-spec-derived-worklist", 1, 1)), 0);
        promote_staged(dir.path());
        let mut close = close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Fixed);
        close.resolution_rev = Some(head.to_string());
        assert_eq!(run_close(dir.path(), &close), 0);
        promote_staged(dir.path());
        assert_eq!(committed_bodies(dir.path()).len(), 1, "the ledger holds the key's current version: the fixed one");
        let ledger_root = GateCtx::from_repo(dir.path()).unwrap().ledger_root;
        assert_eq!(GitTier::new(ledger_root).read_history(RecordKind::Finding).unwrap().records.len(), 2, "the open version it replaced is in the archive");

        assert_eq!(run_close(dir.path(), &close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Rejected)), 0);
        let staged = staged_bodies(dir.path());
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0]["disposition"].as_str(), Some("rejected"));
        assert!(staged[0].get("resolution_sha").is_none(), "moving off `fixed` clears the sha rather than leaving it to contradict the disposition");
    }

    /// `close` moves an EXISTING finding. Authoring a new one is `add`,
    /// and conflating them would let a typo in `--seq` mint a finding
    /// nobody raised.
    #[test]
    fn close_refuses_a_key_no_committed_finding_holds() {
        let (dir, _) = git_repo();
        assert_eq!(run_close(dir.path(), &close_args("s44-spec-derived-worklist", 9, 9, FindingDisposition::Rejected)), 2);
        assert!(staged_bodies(dir.path()).is_empty());
    }

    /// A staged-but-unpromoted finding has no recorded disposition to
    /// move, and `canon gate promote` refuses two transitions of one key
    /// in a single drain. Editing the staged copy is the honest repair,
    /// and the refusal says so.
    #[test]
    fn close_refuses_a_finding_that_was_never_committed() {
        let (dir, head) = git_repo();
        assert_eq!(run_add(dir.path(), &args("s44-spec-derived-worklist", 1, 1)), 0);

        let mut close = close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Fixed);
        close.resolution_rev = Some(head.to_string());
        assert_eq!(run_close(dir.path(), &close), 2);
        assert_eq!(staged_bodies(dir.path()).len(), 1, "the staged original is left exactly as it was");
    }

    /// `close --disposition fixed` with no `--resolution-rev` resolves
    /// HEAD; any other disposition refuses one.
    #[test]
    fn close_defaults_fixed_to_head_and_refuses_a_rev_on_other_dispositions() {
        let (dir, head) = git_repo();
        assert_eq!(run_add(dir.path(), &args("s44-spec-derived-worklist", 1, 1)), 0);
        promote_staged(dir.path());

        let mut with_rev = close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Rejected);
        with_rev.resolution_rev = Some(head.to_string());
        assert_eq!(run_close(dir.path(), &with_rev), 2, "only a `fixed` finding was closed by a commit");
        assert!(staged_bodies(dir.path()).is_empty(), "the refusal stages nothing");

        assert_eq!(run_close(dir.path(), &close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Fixed)), 0);
        assert_eq!(staged_bodies(dir.path())[0]["resolution_sha"].as_str(), Some(head.as_str()));
    }

    /// A resolution sha nobody can check out closes nothing.
    #[test]
    fn close_refuses_a_resolution_sha_this_repo_does_not_hold() {
        let (dir, _) = git_repo();
        assert_eq!(run_add(dir.path(), &args("s44-spec-derived-worklist", 1, 1)), 0);
        promote_staged(dir.path());

        let mut close = close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Fixed);
        close.resolution_rev = Some("b".repeat(40));
        assert_eq!(run_close(dir.path(), &close), 2);
        assert!(staged_bodies(dir.path()).is_empty());
    }

    /// Reopening is a real transition: a fix that did not hold is a fact
    /// the ledger must be able to carry, and refusing it would leave the
    /// corpus able to record only one direction.
    #[test]
    fn a_finding_can_be_reopened() {
        let (dir, head) = git_repo();
        assert_eq!(run_add(dir.path(), &args("s44-spec-derived-worklist", 1, 1)), 0);
        promote_staged(dir.path());
        let mut close = close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Fixed);
        close.resolution_rev = Some(head.to_string());
        assert_eq!(run_close(dir.path(), &close), 0);
        promote_staged(dir.path());

        assert_eq!(run_close(dir.path(), &close_args("s44-spec-derived-worklist", 1, 1, FindingDisposition::Open)), 0);
        assert_eq!(staged_bodies(dir.path())[0]["disposition"].as_str(), Some("open"));
    }

    /// 1-based, like `add`'s: no finding was ever authored under a 0.
    #[test]
    fn close_refuses_a_zero_round_or_seq() {
        let (dir, _) = git_repo();
        assert_eq!(run_close(dir.path(), &close_args("s44-spec-derived-worklist", 0, 1, FindingDisposition::Rejected)), 2);
        assert_eq!(run_close(dir.path(), &close_args("s44-spec-derived-worklist", 1, 0, FindingDisposition::Rejected)), 2);
    }
}
