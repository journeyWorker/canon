//! `canon change defer | drop | reopen <CHANGE_ID>` (0.42 `change-drop`): a human sets an approved change
//! aside, gives it up, or brings a deferred one back.
//!
//! A decision is one [`ChangeState`] record (kind `change_state`, natural key the change id): the latest record
//! of a change is its current state, the ones it replaced are history (the archive, `canon query --history`).
//! Nothing is deleted: the change's brief, evidence, reviews and findings stay, and a closed (deferred or
//! dropped) change reads, in every reader that asks what a subject still requires ([`canon_gate::set_aside`]),
//! as if it were not on the subject's change list. The scenarios only it made are set aside the same way.
//!
//! # Human only
//! The three leaves record a human's decision, so they are `brief approve`'s kind of leaf: an agent role is
//! refused (`ACTOR_DENIED`) unless it relays the human's explicit answer with `--relayed-via` ([`crate::relay`]),
//! and `approval: { relay: deny }` in `canon.yaml` refuses that too. The record keeps the relaying role, actor and
//! tool. `--reason` is the human's answer, word for word.
//!
//! # What each leaf does with the change's current state
//! | leaf     | no state or `reopened` | `deferred`                       | `dropped`                  |
//! |----------|------------------------|----------------------------------|----------------------------|
//! | `defer`  | writes `deferred`      | same values: `unchanged`, else `CLOSED` | `CLOSED`            |
//! | `drop`   | writes `dropped`       | writes `dropped`                 | same values: `unchanged`, else `CLOSED` |
//! | `reopen` | `NOT_DEFERRED`         | writes `reopened`                | `DROPPED`                  |
//!
//! `CHANGE_NOT_FOUND` for a change with no record. `defer` and `drop` also refuse a change that already shipped
//! (`CHANGE_CONFLICT`, reason `SHIPPED`, [`shipped`]): a shipped change is not closed, a new change removes its
//! behavior. A subject that moved to `shipped` while the change was deferred or dropped shipped without it, so
//! that move is no ship of the change (a move after a reopening is). Every refusal writes nothing, `--dry-run` included. The leaves are not destructive: they append a
//! record and delete nothing, so there is no `--commit`.
//!
//! # Reopening and the old approval
//! An `Approved:` line written no later than the second of the reopening is not an approval
//! ([`approval_predates`]); the change comes back as an unapproved one and `canon brief approve` writes a new line
//! even for the same name, answer and code tree ([`approval_time`] keeps that new line from reading as older than
//! the reopening it answers).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use canon_gate::set_aside::latest_states;
use canon_gate::{GateContext, GateCtx};
use canon_ingest::markdown::field;
use canon_model::{Actor, ApprovalRelay, Change, ChangeDecision, ChangeId, ChangeState, Envelope, ProjectId, RecordKind, RoleId, ScenarioId, Subject, SubjectId, SubjectStatus};
use canon_policy::SchemaRegistry;
use canon_store::git::GitCall;
use canon_store::git_tier::GitTier;
use canon_store::tier::TierQuery;
use chrono::{DateTime, SecondsFormat, Utc};
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli_error::{CliError, CliErrorExt as _};
use crate::context::{resolve_canon_yaml, resolve_repo_root};

/// The leaf that runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaf {
    Defer,
    Drop,
    Reopen,
}

impl Leaf {
    /// The state the leaf records.
    pub fn decision(self) -> ChangeDecision {
        match self {
            Leaf::Defer => ChangeDecision::Deferred,
            Leaf::Drop => ChangeDecision::Dropped,
            Leaf::Reopen => ChangeDecision::Reopened,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Leaf::Defer => "defer",
            Leaf::Drop => "drop",
            Leaf::Reopen => "reopen",
        }
    }

    /// The command as an error or a human line names it.
    pub fn command(self) -> &'static str {
        match self {
            Leaf::Defer => "canon change defer",
            Leaf::Drop => "canon change drop",
            Leaf::Reopen => "canon change reopen",
        }
    }

    /// The command id of the envelope (`meta.command`).
    fn id(self) -> &'static str {
        match self {
            Leaf::Defer => "change defer",
            Leaf::Drop => "change drop",
            Leaf::Reopen => "change reopen",
        }
    }
}

/// What a leaf is called with.
#[derive(Debug, Clone, Copy)]
pub struct Call<'a> {
    pub change: &'a ChangeId,
    pub reason: &'a str,
    pub refs: &'a [String],
    pub approver: &'a str,
    pub relayed_via: Option<&'a str>,
    pub actor: Option<&'a str>,
    pub dry_run: bool,
    pub json: bool,
}

/// How many records of each kind a closing leaves in place.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordsKept {
    pub evidence: usize,
    pub reviews: usize,
    pub findings: usize,
}

impl RecordsKept {
    fn total(self) -> usize {
        self.evidence + self.reviews + self.findings
    }
}

/// `canon change defer | drop | reopen`: the decision recorded (or, for a repeat, found already recorded).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeStateReport {
    pub change_id: String,
    /// `deferred`, `dropped` or `reopened`.
    pub state: String,
    /// The human's words.
    pub reason: String,
    pub refs: Vec<String>,
    pub approver: String,
    /// Set when an agent recorded the decision for the human: the relaying role, actor and tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay: Option<ApprovalRelay>,
    /// When the decision was recorded: UTC, RFC 3339, to the second.
    pub at: String,
    /// The subjects the change is on: they stop requiring it on a closing, and require it again on a reopening.
    pub subjects: Vec<String>,
    /// The scenarios only this change made, which the closing sets aside.
    pub set_aside_scenarios: Vec<String>,
    /// The scenarios a reopening requires again.
    pub returned_scenarios: Vec<String>,
    /// The evidence, review and finding records of the change and of its set-aside scenarios, none of them removed.
    pub records_kept: RecordsKept,
    /// Where the record went, when that is the hot store.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hot_store: Option<String>,
    /// A `--dry-run`: nothing was written.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
}

impl crate::reports::Report for ChangeStateReport {}

// ------------------------------------------------------------------------------------------------ reading states

/// Every `change_state` record the local ledger holds, unfolded. Empty when the ledger cannot be read: the readers
/// that call this treat "no decision" as the answer, as the gate does for a kind it cannot read.
pub(crate) fn local_states(repo: &Path) -> Vec<ChangeState> {
    let Ok(ctx) = GateCtx::from_repo(repo) else { return Vec::new() };
    let Ok(read) = canon_store::tier::Tier::read(&GitTier::new(&ctx.ledger_root), &TierQuery::kind(RecordKind::ChangeState)) else { return Vec::new() };
    read.records.into_iter().filter_map(|raw| serde_json::from_value::<ChangeState>(raw.0).ok()).collect()
}

/// The current state of `change` in the local ledger: its latest `change_state` record.
pub(crate) fn current_state(repo: &Path, change: &ChangeId) -> Option<ChangeState> {
    let states = local_states(repo);
    latest_states(&states).into_iter().find(|state| &state.change_id == change).cloned()
}

/// When `change` was reopened, when its current state is `reopened`.
pub fn reopened_at(repo: &Path, change: &ChangeId) -> Option<DateTime<Utc>> {
    current_state(repo, change).filter(|state| state.state == ChangeDecision::Reopened).map(|state| state.envelope.at)
}

/// Whether an `Approved:` line written at `line_at` (whole seconds; `None` when it carries no readable time)
/// predates the reopening at `reopened`: it is no later than the reopening's own second. A line that carries no
/// readable time cannot be shown to come after, so it predates.
pub fn approval_predates(line_at: Option<DateTime<Utc>>, reopened: DateTime<Utc>) -> bool {
    line_at.is_none_or(|at| at.timestamp() <= reopened.timestamp())
}

/// The reopening that makes the `Approved:` line `line` of `change`'s brief void, with its time; `None` when the
/// line stands (the change was never reopened, or the line was written after it).
pub fn reopened_since(repo: &Path, change: &ChangeId, line: &str) -> Option<DateTime<Utc>> {
    let reopened = reopened_at(repo, change)?;
    approval_predates(crate::brief::approved_at(line), reopened).then_some(reopened)
}

/// The time an approval of `change` written at `now` carries: `now`, or the first second after the reopening the
/// change came back from when that is later, so the new line never reads as older than the reopening it answers.
pub fn approval_time(repo: &Path, change: &ChangeId, now: DateTime<Utc>) -> DateTime<Utc> {
    match reopened_at(repo, change).and_then(|at| DateTime::from_timestamp(at.timestamp() + 1, 0)) {
        Some(earliest) => now.max(earliest),
        None => now,
    }
}

/// The refusal of a write that would add to a closed change (`brief approve`, `scenario new --flow --change`, the
/// dashboard's approval): `CHANGE_CONFLICT`, reason `CLOSED`. `None` for a change that is open.
pub(crate) fn closed_refusal(repo: &Path, change: &ChangeId) -> Option<CliError> {
    let state = current_state(repo, change).filter(|state| state.state.is_closed())?;
    let id = change.as_str();
    let hint = if state.state == ChangeDecision::Deferred {
        format!("a closed change takes no approval and no new flow-tagged scenario: a human brings a deferred change back with `canon change reopen {id} --reason <their words> --approver <their name>`, then approves its brief again")
    } else {
        format!("a dropped change is final: write a new change (`canon change new <slug> --subject <id> --title <title>`) and name `{id}` in its `Source:` line")
    };
    Some(CliError::refused("CHANGE_CONFLICT", format!("change `{id}` is {} (decided by {}: {}), so nothing is written for it", state.state.as_str(), state.approver, state.reason)).reason("CLOSED").hint(hint))
}

/// Whether `canon.yaml` has no `routing:` section with an entry in it: `canon migrate` adds a line to a section that exists
/// and leaves any other file as it is, so for this file its preview is no way to the line.
fn has_no_routing_entries(canon_yaml: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(canon_yaml) else { return false };
    let Ok(document) = serde_yaml::from_str::<serde_yaml::Value>(&text) else { return false };
    document.get("routing").and_then(serde_yaml::Value::as_mapping).is_none_or(serde_yaml::Mapping::is_empty)
}

/// The refusal for a call on a repo whose `canon.yaml` has no `change_state` line under `routing:`. With a `routing:` section the hint names
/// `canon migrate`, which adds the line; with none `canon migrate` adds nothing, so the hint says to add the section by hand. Any other
/// fault of the tiers is as every read of the repo says it.
fn tiers_refusal(canon_yaml: &Path, message: String) -> CliError {
    let error = crate::read_cause::repo_error(message);
    let names_kind = {
        let lower = error.message.to_ascii_lowercase();
        lower.contains("changestate") || lower.contains("change_state")
    };
    if error.code == "CONFIG_INVALID" && error.reason_name() == Some("TIERS") && names_kind {
        if has_no_routing_entries(canon_yaml) {
            return error.hint("canon.yaml has no `routing:` section (canon 0.42 routes every record kind with a line there), and `canon migrate` adds nothing to a file without one: add a `routing:` section with the entry `change_state: local` by hand (the lines `canon init` writes show the other kinds), then run this command again");
        }
        return error.hint("canon.yaml has no `change_state` line under `routing:` (canon 0.42 added the record kind): run `canon migrate` to preview the line it adds, then `canon migrate --commit --plan-digest <PLAN_DIGEST>`, then run this command again");
    }
    error
}

// ------------------------------------------------------------------------------------------------ the verdict

/// A refusal of the change's state.
#[derive(Debug, PartialEq, Eq)]
struct Conflict {
    reason: &'static str,
    message: String,
    hint: String,
}

/// What the change's current state allows the leaf to do.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Proceed,
    /// The same decision with the same values is already recorded.
    Unchanged,
    Conflict(Conflict),
}

fn same_values(state: &ChangeState, call: &Call<'_>) -> bool {
    fn refs(list: &[String]) -> BTreeSet<&str> {
        list.iter().map(String::as_str).collect()
    }
    state.reason == call.reason && state.approver == call.approver && refs(&state.refs) == refs(call.refs)
}

/// The table of the module doc.
fn judge(leaf: Leaf, current: Option<&ChangeState>, call: &Call<'_>) -> Verdict {
    let id = call.change.as_str();
    let state = current.map(|state| state.state);
    match (leaf, state) {
        (Leaf::Reopen, Some(ChangeDecision::Deferred)) => Verdict::Proceed,
        (Leaf::Reopen, Some(ChangeDecision::Dropped)) => Verdict::Conflict(Conflict {
            reason: "DROPPED",
            message: format!("change `{id}` was dropped, and a dropped change is not brought back"),
            hint: format!("write a new change with `canon change new <slug> --subject <id> --title <title>` and name `{id}` in its `Source:` line"),
        }),
        (Leaf::Reopen, _) => Verdict::Conflict(Conflict {
            reason: "NOT_DEFERRED",
            message: format!("change `{id}` is not deferred, so there is nothing to reopen"),
            hint: "only a deferred change reopens: `canon query --kind change_state --status deferred` lists them".to_string(),
        }),
        (_, Some(closed)) if closed.is_closed() => {
            let current = current.expect("a state came from a record");
            if closed == leaf.decision() && same_values(current, call) {
                Verdict::Unchanged
            } else if leaf == Leaf::Drop && closed == ChangeDecision::Deferred {
                Verdict::Proceed
            } else {
                let hint = if closed == ChangeDecision::Deferred {
                    format!("repeat the decision with the same --reason, --ref and --approver to leave it as it is, `canon change drop {id}` to give the change up, or `canon change reopen {id}` to bring it back")
                } else {
                    format!("a dropped change is final: write a new change (`canon change new <slug> --subject <id> --title <title>`) and name `{id}` in its `Source:` line")
                };
                Verdict::Conflict(Conflict { reason: "CLOSED", message: format!("change `{id}` is already {} (decided by {}: {}) and the call differs from that decision", closed.as_str(), current.approver, current.reason), hint })
            }
        }
        _ => Verdict::Proceed,
    }
}

// ------------------------------------------------------------------------------------------------ shipped

/// One stored version of a subject, reduced to what the ship decision reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub subject: SubjectId,
    pub at: DateTime<Utc>,
    pub status: SubjectStatus,
    pub changes: Vec<ChangeId>,
}

impl Version {
    pub fn of(subject: &Subject) -> Self {
        Self { subject: subject.subject_id.clone(), at: subject.envelope.at, status: subject.status, changes: subject.change_ids.clone() }
    }
}

/// When a change was first approved, from what its brief's git history and the brief now say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirstApproval {
    /// The brief never carried an `Approved:` line: the change was a draft all along.
    Never,
    /// The first `Approved:` line, in complete history, is of this time.
    At(DateTime<Utc>),
    /// The first approval is at this time or earlier: the history is short (a shallow clone), or the line was
    /// never committed and only the brief now shows it.
    AtMost(DateTime<Utc>),
    /// The history cannot be read.
    Unknown(String),
}

/// Whether a change shipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shipped {
    No,
    Yes { subject: SubjectId, at: DateTime<Utc> },
    /// The move or approval history cannot rule it out: refused as if it had shipped (fail closed).
    Unknown(String),
}

/// The times a subject moved to `shipped` with `change` on its change list, oldest first. A move is a version that
/// is `shipped` after a version that is not; a version written onto an already shipped subject (`subject adopt`) is
/// no move. A subject whose first stored version is `shipped` counts as moved then: its earlier versions are gone.
pub fn ship_moves(versions: &[Version], change: &ChangeId) -> Vec<(SubjectId, DateTime<Utc>)> {
    let mut by_subject: BTreeMap<&SubjectId, Vec<&Version>> = BTreeMap::new();
    for version in versions {
        by_subject.entry(&version.subject).or_default().push(version);
    }
    let mut moves = Vec::new();
    for (subject, mut list) in by_subject {
        list.sort_by_key(|version| version.at);
        let mut previous: Option<SubjectStatus> = None;
        for version in list {
            if version.status == SubjectStatus::Shipped && previous != Some(SubjectStatus::Shipped) && version.changes.contains(change) {
                moves.push((subject.clone(), version.at));
            }
            previous = Some(version.status);
        }
    }
    moves.sort_by_key(|(_, at)| *at);
    moves
}

/// The decisions of one change (`deferred`, `dropped`, `reopened`) with their times, oldest first: what its `change_state` history holds.
pub type Decisions = Vec<(DateTime<Utc>, ChangeDecision)>;

/// Whether the change was closed (its latest decision no later than `at` is `deferred` or `dropped`) at `at`.
fn closed_at(decisions: &[(DateTime<Utc>, ChangeDecision)], at: DateTime<Utc>) -> bool {
    decisions.iter().filter(|(decided, _)| *decided <= at).max_by_key(|(decided, _)| *decided).is_some_and(|(_, state)| state.is_closed())
}

/// Whether `change` shipped: a subject moved to `shipped` with the change on its list while the change was not closed,
/// and the change was not an unapproved draft at that time (its first approval is no later than the move). A subject
/// that shipped while the change was deferred or dropped shipped without it (a closed change is no requirement of
/// the subject), so that move is no ship of the change; a move after a reopening is. Re-approving the brief or
/// reopening the subject afterwards changes nothing: the first approval is what counts, which is why
/// `first_approval` reads the brief's history and not the current `Approved:` line. `decisions` (the change's own
/// `change_state` history) and `first_approval` are asked only when a move names the change.
pub fn shipped(versions: &[Version], change: &ChangeId, decisions: impl FnOnce() -> Result<Decisions, String>, first_approval: impl FnOnce() -> FirstApproval) -> Shipped {
    let moves = ship_moves(versions, change);
    if moves.is_empty() {
        return Shipped::No;
    }
    let decisions = match decisions() {
        Ok(decisions) => decisions,
        Err(why) => return Shipped::Unknown(why),
    };
    let moves: Vec<_> = moves.into_iter().filter(|(_, at)| !closed_at(&decisions, *at)).collect();
    if moves.is_empty() {
        return Shipped::No;
    }
    let approval = first_approval();
    let mut unknown: Option<String> = None;
    for (subject, at) in moves {
        match &approval {
            FirstApproval::Never => {}
            FirstApproval::At(approved) if *approved <= at => return Shipped::Yes { subject, at },
            FirstApproval::At(_) => {}
            FirstApproval::AtMost(approved) if *approved <= at => return Shipped::Yes { subject, at },
            FirstApproval::AtMost(approved) => {
                unknown = Some(format!("the history that would say whether the brief was approved before subject `{}` shipped (at {}) is not here: the oldest approval it holds is at {}", subject.as_str(), at.to_rfc3339_opts(SecondsFormat::Secs, true), approved.to_rfc3339_opts(SecondsFormat::Secs, true)));
            }
            FirstApproval::Unknown(why) => unknown = Some(why.clone()),
        }
    }
    unknown.map_or(Shipped::No, Shipped::Unknown)
}

/// The commit time, or the time its own `Approved:` line carries, of the first commit that added an `Approved:` line
/// to the brief at `path`; `Ok(None)` when no commit did. Every git call goes through [`canon_store::git`].
fn committed_first_approval(repo: &Path, path: &str) -> Result<Option<DateTime<Utc>>, String> {
    let out = match GitCall::new(repo).args(["log", "--reverse", "--format=%H", "-G^Approved:", "--", path]).run() {
        Ok(out) => out,
        // A repository with no commit yet has no history to read.
        Err(error) if error.is_exit() && error.stderr_text().contains("does not have any commits") => return Ok(None),
        Err(error) => return Err(format!("the git history of `{path}` cannot be read: {error}")),
    };
    let listed = out.stdout_trimmed();
    let Some(sha) = listed.lines().map(str::trim).find(|line| !line.is_empty()) else { return Ok(None) };
    let blob = format!("{sha}:{path}");
    let line_at = GitCall::new(repo).args(["show", blob.as_str()]).run().ok().and_then(|shown| {
        let text = String::from_utf8_lossy(&shown.stdout).into_owned();
        field(&text, "Approved:").and_then(crate::brief::approved_at)
    });
    if line_at.is_some() {
        return Ok(line_at);
    }
    let committed = GitCall::new(repo).args(["show", "-s", "--format=%ct", sha]).run().map_err(|error| format!("the commit time of `{sha}` cannot be read: {error}"))?;
    committed
        .stdout_trimmed()
        .parse::<i64>()
        .ok()
        .and_then(|secs| DateTime::from_timestamp(secs, 0))
        .map(Some)
        .ok_or_else(|| format!("the commit time of `{sha}` is not a time"))
}

/// When `change`'s brief was first approved, read from git history first (the current line is replaced by every
/// re-approval) and, where that history has no `Approved:` line, from the brief now.
fn first_approval(repo: &Path, change: &ChangeId) -> FirstApproval {
    let slug = change.as_str();
    let live = format!("{}/{slug}/brief.md", canon_model::paths::CHANGES_DIR);
    let archived = format!("{}/archive/{slug}/brief.md", canon_model::paths::CHANGES_DIR);
    let mut committed: Option<DateTime<Utc>> = None;
    for path in [live.as_str(), archived.as_str()] {
        match committed_first_approval(repo, path) {
            Ok(Some(at)) => committed = Some(committed.map_or(at, |old| old.min(at))),
            Ok(None) => {}
            Err(why) => return FirstApproval::Unknown(why),
        }
    }
    let shallow = match GitCall::new(repo).args(["rev-parse", "--is-shallow-repository"]).run() {
        Ok(out) => out.stdout_trimmed() == "true",
        Err(error) if error.is_exit() && error.stderr_text().contains("not a git repository") => return FirstApproval::Unknown("the repository has no git history to read".to_string()),
        Err(error) => return FirstApproval::Unknown(format!("git cannot say whether the history is complete: {error}")),
    };
    let working = crate::brief::canon_brief(repo, change).and_then(|path| std::fs::read_to_string(path).ok()).map(|text| field(&text, "Approved:").map(crate::brief::approved_at));
    match (committed, working.flatten()) {
        (Some(at), _) if shallow => FirstApproval::AtMost(at),
        (Some(at), _) => FirstApproval::At(at),
        (None, Some(Some(at))) => FirstApproval::AtMost(at),
        (None, Some(None)) => FirstApproval::Unknown("the brief's `Approved:` line carries no readable time and no commit holds an earlier one".to_string()),
        (None, None) => FirstApproval::Never,
    }
}

/// `Err` (`CHANGE_CONFLICT`, reason `SHIPPED`) when `change` shipped or canon cannot tell that it did not.
fn refuse_shipped(repo: &Path, ctx: &GateContext, change: &ChangeId) -> Result<(), CliError> {
    let id = change.as_str();
    let unreadable = |why: String| {
        CliError::refused("CHANGE_CONFLICT", format!("cannot tell whether change `{id}` shipped, so it is treated as shipped: {why}"))
            .reason("SHIPPED")
            .hint("a shipped change is not closed; if the history is only short (a shallow clone), run `git fetch --unshallow` and call again, otherwise write a new change that removes the behavior (`canon change new <slug> --subject <id> --title <title>`, naming this change in its `Source:`)")
    };
    if ctx.unreadable_kinds.contains(&RecordKind::Subject) {
        return Err(unreadable("subject records are routed to a store the local ledger does not hold".to_string()));
    }
    let read = GitTier::new(&ctx.ctx.ledger_root).read_history(RecordKind::Subject).map_err(|e| unreadable(format!("the subjects' move history cannot be read: {e}")))?;
    if !read.violations.is_empty() {
        return Err(unreadable("a stored subject version does not read".to_string()));
    }
    let mut versions = Vec::new();
    for raw in read.records {
        match serde_json::from_value::<Subject>(raw.0) {
            Ok(subject) => versions.push(Version::of(&subject)),
            Err(e) => return Err(unreadable(format!("a stored subject version does not read: {e}"))),
        }
    }
    let decisions = || -> Result<Decisions, String> {
        if ctx.unreadable_kinds.contains(&RecordKind::ChangeState) {
            return Err("change_state records are routed to a store the local ledger does not hold".to_string());
        }
        let read = GitTier::new(&ctx.ctx.ledger_root).read_history(RecordKind::ChangeState).map_err(|e| format!("the change's decision history cannot be read: {e}"))?;
        if !read.violations.is_empty() {
            return Err("a stored change_state record does not read".to_string());
        }
        let mut decisions = Decisions::new();
        for raw in read.records {
            let state = serde_json::from_value::<ChangeState>(raw.0).map_err(|e| format!("a stored change_state record does not read: {e}"))?;
            if &state.change_id == change {
                decisions.push((state.envelope.at, state.state));
            }
        }
        decisions.sort_by_key(|(at, _)| *at);
        Ok(decisions)
    };
    match shipped(&versions, change, decisions, || first_approval(repo, change)) {
        Shipped::No => Ok(()),
        Shipped::Yes { subject, at } => Err(CliError::refused(
            "CHANGE_CONFLICT",
            format!("change `{id}` already shipped: subject `{}` moved to shipped at {} with it on its change list, after its brief was approved", subject.as_str(), at.to_rfc3339_opts(SecondsFormat::Secs, true)),
        )
        .reason("SHIPPED")
        .hint(format!("a shipped change is not closed: write a new change that removes the behavior (`canon change new <slug> --subject <id> --title <title>`, naming `{id}` in its `Source:` line)"))),
        Shipped::Unknown(why) => Err(unreadable(why)),
    }
}

// ------------------------------------------------------------------------------------------------ the leaf

/// The human's own words, as the arguments carry them: each one non-blank line. The argument rules refuse these before
/// the leaf runs; this is the same check for a call that reaches it another way.
fn check_call(leaf: Leaf, call: &Call<'_>) -> Result<(), CliError> {
    let one_line = |text: &str| !text.trim().is_empty() && !text.contains(['\n', '\r']);
    let command = leaf.command();
    if !one_line(call.approver) {
        return Err(CliError::refused("APPROVAL_INVALID", "--approver must name the human who decided, one non-empty line").field("--approver").hint(format!("pass the human's own name and run `{command}` again")));
    }
    if !one_line(call.reason) {
        return Err(CliError::refused("CHANGE_ARGUMENT_INVALID", "--reason must be the human's words, one non-empty line").field("--reason").hint(format!("pass the reason the human gave and run `{command}` again")));
    }
    if leaf != Leaf::Reopen && call.refs.is_empty() {
        return Err(CliError::refused("CHANGE_ARGUMENT_INVALID", format!("`{command}` needs at least one --ref: what a reader can open to see why")).field("--ref").hint("give a link, a commit, a file or the date of a conversation with --ref, once per reference"));
    }
    if call.refs.iter().any(|reference| !one_line(reference)) {
        return Err(CliError::refused("CHANGE_ARGUMENT_INVALID", "--ref must be one non-empty line").field("--ref").hint(format!("give a link, a commit, a file or the date of a conversation, and run `{command}` again")));
    }
    Ok(())
}

fn authorize(root: &Path, leaf: Leaf, call: &Call<'_>) -> Result<Option<ApprovalRelay>, CliError> {
    let id = call.change.as_str();
    let refs = if leaf == Leaf::Reopen { " [--ref <link>]" } else { " --ref <link>" };
    let human_call = format!("canon change {} {id} --reason \"<their words>\"{refs} --approver <their name>", leaf.word());
    let relay_call = format!("{human_call} --relayed-via <the question tool> --actor <your actor id>");
    let call_args = crate::relay::Call { via: call.relayed_via, actor: call.actor, approver: Some(call.approver), note: Some(call.reason) };
    crate::relay::authorize(leaf.command(), &human_call, &relay_call, root, call_args)
        .map(|relay| relay.map(|relay| ApprovalRelay { approver: Some(call.approver.to_string()), answer: Some(call.reason.to_string()), ..relay }))
}

fn read_states(registry: &canon_store::registry::TierRegistry, canon_yaml: &Path) -> Result<Vec<ChangeState>, CliError> {
    let raws = crate::subject::latest_records(registry, RecordKind::ChangeState).map_err(|e| tiers_refusal(canon_yaml, e.to_string()))?;
    raws.into_iter()
        .map(|raw| serde_json::from_value::<ChangeState>(raw.0).map_err(|e| crate::read_cause::repo_error(format!("a stored change_state record is malformed: {e}")).hint("fix or remove the record the message names, then run the command again")))
        .collect()
}

fn read_change(registry: &canon_store::registry::TierRegistry, id: &ChangeId) -> Result<Option<Change>, CliError> {
    let raws = crate::subject::latest_records(registry, RecordKind::Change).map_err(|e| crate::read_cause::repo_error(e.to_string()))?;
    for raw in raws {
        if raw.0.get("change_id").and_then(serde_json::Value::as_str) == Some(id.as_str()) {
            return serde_json::from_value::<Change>(raw.0).map(Some).map_err(|e| crate::read_cause::repo_error(format!("stored change `{}` is malformed: {e}", id.as_str())));
        }
    }
    Ok(None)
}

fn load_context(root: &Path) -> Result<GateContext, CliError> {
    GateCtx::from_repo(root)
        .map_err(|e| e.to_string())
        .and_then(|ctx| GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string()))
        .map_err(|e| crate::read_cause::config_error(e).hint("fix canon.yaml (`canon init --check` names the problem), then run the command again"))
}

/// `canon change defer | drop | reopen` (module doc). Exit `0` written (or `unchanged`, or a `--dry-run` that would
/// write), `2` refused with nothing written.
pub fn run(repo: &Path, leaf: Leaf, call: &Call<'_>) -> u8 {
    let root = resolve_repo_root(repo);
    let relay = match authorize(&root, leaf, call) {
        Ok(relay) => relay,
        Err(error) => return error.emit(leaf.command(), call.json),
    };
    if let Err(error) = check_call(leaf, call) {
        return error.emit(leaf.command(), call.json);
    }
    match execute(&root, leaf, call, relay) {
        Ok(()) => 0,
        Err(error) => error.emit(leaf.command(), call.json),
    }
}

fn execute(root: &Path, leaf: Leaf, call: &Call<'_>, relay: Option<ApprovalRelay>) -> Result<(), CliError> {
    let change = call.change;
    let id = change.as_str();
    let canon_yaml = resolve_canon_yaml(root);
    let registry = crate::subject::registry_for(&canon_yaml, &[RecordKind::ChangeState, RecordKind::Change, RecordKind::Subject]).map_err(|e| tiers_refusal(&canon_yaml, e.to_string()))?;
    let states = read_states(&registry, &canon_yaml)?;
    let Some(record) = read_change(&registry, change)? else {
        return Err(CliError::refused("CHANGE_NOT_FOUND", format!("change `{id}` does not exist (it has no change record, so there is nothing to {})", leaf.word()))
            .hint(format!("import the change first: `canon ingest plans` reads `.canon/changes/{id}/brief.md` into a change record and `canon subject adopt <subject> --change {id}` links it to its subject; check the id against the changes `canon status` lists")));
    };
    let current = latest_states(&states).into_iter().find(|state| &state.change_id == change);
    let verdict = judge(leaf, current, call);
    let unchanged = verdict == Verdict::Unchanged;
    match verdict {
        Verdict::Conflict(Conflict { reason, message, hint }) => return Err(CliError::refused("CHANGE_CONFLICT", message).reason(reason).hint(hint)),
        Verdict::Proceed | Verdict::Unchanged => {}
    }
    let mut ctx = load_context(root)?;
    if !unchanged && leaf != Leaf::Reopen {
        refuse_shipped(root, &ctx, change)?;
    }
    if !unchanged {
        crate::write_mode::check_kinds_writable(&canon_yaml, &[RecordKind::ChangeState])?;
    }

    let owners: Vec<String> = crate::subject::change_owners(root, &record).unwrap_or_else(|_| record.subject_ids.clone()).iter().map(|subject| subject.as_str().to_string()).collect();
    let (stored, at) = match (unchanged, current) {
        (true, Some(current)) => (current.clone(), current.envelope.at),
        _ => {
            let now = Utc::now();
            let actor = match &relay {
                Some(relay) => RoleId::parse(relay.role.clone()).map_or_else(|_| Actor::new_unattributed(&relay.actor), |role| Actor::new(&relay.actor, role)),
                None => Actor::new_unattributed(call.approver),
            };
            let mut stored = ChangeState::new(Envelope::current(RecordKind::ChangeState, now, actor), change.clone(), leaf.decision(), call.reason, call.refs.to_vec(), call.approver);
            stored.relay = relay;
            (stored, now)
        }
    };

    let (set_aside, returned, kept) = match leaf {
        Leaf::Reopen => {
            crate::set_aside::attach(&mut ctx);
            let before: BTreeSet<(ProjectId, ScenarioId)> = ctx.set_aside.scenarios.iter().filter(|(_, by)| by.contains(change)).map(|(key, _)| key.clone()).collect();
            let still_closed: BTreeMap<ChangeId, DateTime<Utc>> = ctx.set_aside.closed.iter().filter(|(other, _)| **other != *change).map(|(other, closed)| (other.clone(), closed.at)).collect();
            let (after, _) = crate::set_aside::compute(root, &still_closed);
            let returned: Vec<(ProjectId, ScenarioId)> = before.into_iter().filter(|key| !after.contains_key(key)).collect();
            (Vec::new(), returned, RecordsKept::default())
        }
        _ if unchanged => {
            crate::set_aside::attach(&mut ctx);
            let set_aside: Vec<(ProjectId, ScenarioId)> = ctx.set_aside.scenarios.iter().filter(|(_, by)| by.contains(change)).map(|(key, _)| key.clone()).collect();
            let kept = records_kept(&ctx, change, &set_aside);
            (set_aside, Vec::new(), kept)
        }
        _ => {
            let set_aside = crate::set_aside::scenarios_made_by(root, &ctx, change, at);
            let kept = records_kept(&ctx, change, &set_aside);
            (set_aside, Vec::new(), kept)
        }
    };

    if !unchanged {
        let persisted = if call.dry_run { registry.check_persist::<ChangeState>() } else { registry.persist(&stored).map(drop) };
        persisted.map_err(|e| {
            let message = format!("failed to record the decision: {e}; nothing was written");
            CliError::refused(crate::read_cause::write_code_of_text(&message), message).hint(format!("check that the ledger directory is writable, then run `{}` again", leaf.command()))
        })?;
    }

    let ids = |keys: &[(ProjectId, ScenarioId)]| -> Vec<String> {
        let mut ids: Vec<String> = keys.iter().map(|(_, scenario)| scenario.as_str().to_string()).collect();
        ids.sort();
        ids.dedup();
        ids
    };
    let hot = (!unchanged && !call.dry_run).then(|| crate::hot_store::HotStore::written_by(&canon_yaml, &[RecordKind::ChangeState])).flatten();
    let report = ChangeStateReport {
        change_id: id.to_string(),
        state: stored.state.as_str().to_string(),
        reason: stored.reason.clone(),
        refs: stored.refs.clone(),
        approver: stored.approver.clone(),
        relay: stored.relay.clone(),
        at: at.to_rfc3339_opts(SecondsFormat::Secs, true),
        subjects: owners,
        set_aside_scenarios: ids(&set_aside),
        returned_scenarios: ids(&returned),
        records_kept: kept,
        hot_store: hot.as_ref().map(crate::hot_store::HotStore::location),
        dry_run: call.dry_run,
    };

    if kept.total() > 0 && !unchanged {
        let mut parts = Vec::new();
        for (count, kind) in [(kept.evidence, "evidence"), (kept.reviews, "review"), (kept.findings, "finding")] {
            if count > 0 {
                parts.push(format!("{count} {kind}"));
            }
        }
        crate::json_out::warn(
            "CHANGE_RECORDS_KEPT",
            format!("change `{id}` and the {} scenario(s) only it made have {} record(s); none was removed and `canon query` still lists them", report.set_aside_scenarios.len(), parts.join(", ")),
        );
    }
    say(leaf, call, &report, unchanged, hot.as_ref());
    if unchanged {
        crate::json_out::mark_unchanged();
    }
    finish(leaf, call.json, &report);
    Ok(())
}

/// The evidence, review and finding records that belong to `change` or to the scenarios `made` by it alone.
fn records_kept(ctx: &GateContext, change: &ChangeId, made: &[(ProjectId, ScenarioId)]) -> RecordsKept {
    let made: BTreeSet<&(ProjectId, ScenarioId)> = made.iter().collect();
    let of_scenario = |project: Option<&ProjectId>, scenario: Option<&ScenarioId>| matches!((project, scenario), (Some(project), Some(scenario)) if made.contains(&(project.clone(), scenario.clone())));
    let evidence = ctx
        .evidence
        .iter()
        .chain(ctx.audits.iter())
        .filter(|record| record.task_id.as_ref().is_some_and(|task| &task.change_id() == change) || of_scenario(record.project_id.as_ref(), record.scenario_id.as_ref()))
        .count();
    let reviews = ctx.reviews.iter().filter(|review| review.change_id.as_ref() == Some(change) || of_scenario(Some(&review.project_id), review.scenario_id.as_ref())).count();
    let findings = ctx.findings.iter().filter(|finding| &finding.change_id == change).count();
    RecordsKept { evidence, reviews, findings }
}

/// The human lines of a success.
fn say(leaf: Leaf, call: &Call<'_>, report: &ChangeStateReport, unchanged: bool, hot: Option<&crate::hot_store::HotStore>) {
    let command = leaf.command();
    let json = call.json;
    let id = &report.change_id;
    if unchanged {
        crate::say!(json, "{command}: change `{id}` is already {} with these values; nothing written", report.state);
    } else if call.dry_run {
        crate::say!(json, "{command}: would record change `{id}` as {} (decided by {}: {}) — dry run, nothing written", report.state, report.approver, report.reason);
    } else {
        crate::say!(json, "{command}: recorded change `{id}` as {} (decided by {}: {}) — {}", report.state, report.approver, report.reason, crate::write_mode::DIRECT);
    }
    if let Some(hot) = hot {
        crate::say!(json, "{}", hot.line());
    }
    let list = |ids: &[String]| ids.iter().map(|id| format!("`{id}`")).collect::<Vec<_>>().join(", ");
    if !report.subjects.is_empty() {
        let effect = if leaf == Leaf::Reopen { "requires it again" } else { "no longer requires it" };
        crate::say!(json, "  subjects it is on ({effect}): {}", list(&report.subjects));
    }
    if !report.set_aside_scenarios.is_empty() {
        crate::say!(json, "  scenarios set aside: {}", list(&report.set_aside_scenarios));
    }
    if !report.returned_scenarios.is_empty() {
        crate::say!(json, "  scenarios required again: {}", list(&report.returned_scenarios));
    }
    let kept = report.records_kept;
    if kept.total() > 0 {
        crate::say!(json, "  records kept: {} evidence, {} review, {} finding", kept.evidence, kept.reviews, kept.findings);
    }
    if leaf == Leaf::Reopen && !call.dry_run {
        crate::say!(json, "  the old approval no longer counts: have the human approve the brief again (`canon brief approve {id} ...`)");
    }
    if leaf == Leaf::Drop {
        crate::say!(json, "  a dropped change is not brought back, only a new change is: write one with `canon change new <slug> --subject <id> --title <title>` and name `{id}` in its `Source:` line");
    }
}

/// The last output: the report, and `meta.next` naming `canon status` and `canon gate check`, the two reads that now
/// answer without (or, for a reopening, with) the change.
fn finish(leaf: Leaf, json: bool, report: &ChangeStateReport) {
    use crate::status::{NextStep, StepActor};
    let role = crate::role::Role::current(None).unwrap_or(crate::role::Role::Orchestrator);
    let effect = if leaf == Leaf::Reopen { "it is a requirement again" } else { "it is no longer a requirement" };
    let steps = vec![
        NextStep { command: "canon status".into(), why: format!("see where the repo stands now that change `{}` is {}: {effect}", report.change_id, report.state), actor: StepActor::Agent, role: Some(role) },
        NextStep { command: "canon gate check".into(), why: "run the gate: it asks for what the repo still requires".into(), actor: StepActor::Agent, role: Some(role) },
    ];
    let after = crate::next::After::fixed(steps);
    if json {
        crate::json_out::data_with_next_and_meta(leaf.id(), report, &after, serde_json::Value::Null);
    } else {
        for line in after.lines() {
            println!("{line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(id: &str) -> ChangeId {
        ChangeId::parse(id).expect("a change id")
    }

    fn subject(id: &str) -> SubjectId {
        SubjectId::parse(id).expect("a subject id")
    }

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).expect("a time")
    }

    fn version(subject_id: &str, secs: i64, status: SubjectStatus, changes: &[&str]) -> Version {
        Version { subject: subject(subject_id), at: at(secs), status, changes: changes.iter().map(|c| change(c)).collect() }
    }

    fn never() -> FirstApproval {
        panic!("the approval history is read only when a move names the change")
    }

    /// A subject that shipped `c` (approved at 50, shipped at 100), was reopened (200) and shipped again (300).
    fn shipped_reopened() -> Vec<Version> {
        vec![
            version("auth", 10, SubjectStatus::Proposed, &["c"]),
            version("auth", 60, SubjectStatus::Building, &["c"]),
            version("auth", 90, SubjectStatus::Verifying, &["c"]),
            version("auth", 100, SubjectStatus::Shipped, &["c"]),
            version("auth", 200, SubjectStatus::Verifying, &["c"]),
            version("auth", 300, SubjectStatus::Shipped, &["c"]),
        ]
    }

    /// change.drop.06: a change a subject shipped, then the subject reopened and the brief approved again, is shipped.
    #[test]
    fn a_change_shipped_then_reopened_and_re_approved_is_shipped() {
        // The current `Approved:` line is of 250, after the first ship; the first approval (git history) is of 50.
        let result = shipped(&shipped_reopened(), &change("c"), open, || FirstApproval::At(at(50)));
        assert_eq!(result, Shipped::Yes { subject: subject("auth"), at: at(100) });
    }

    /// change.drop.06: `subject adopt` onto an already shipped subject is no ship, so the change can be closed.
    #[test]
    fn a_change_adopted_into_a_shipped_subject_is_not_shipped() {
        let versions = vec![
            version("auth", 10, SubjectStatus::Proposed, &[]),
            version("auth", 100, SubjectStatus::Shipped, &[]),
            // The adoption re-stamps the shipped subject with the change on its list.
            version("auth", 150, SubjectStatus::Shipped, &["c"]),
        ];
        assert!(ship_moves(&versions, &change("c")).is_empty());
        assert_eq!(shipped(&versions, &change("c"), open, never), Shipped::No);
    }

    /// change.drop.06: a change that was an unapproved draft when the subject shipped did not ship.
    #[test]
    fn a_change_that_was_an_unapproved_draft_when_the_subject_shipped_is_not_shipped() {
        // Approved at 150, shipped at 100: the draft rode along on the list but nothing approved shipped.
        assert_eq!(shipped(&shipped_reopened()[..4], &change("c"), open, || FirstApproval::At(at(150))), Shipped::No);
        // Never approved at all.
        assert_eq!(shipped(&shipped_reopened(), &change("c"), open, || FirstApproval::Never), Shipped::No);
    }

    /// change.drop.06: a change shipped again after a later approval is shipped by the second move.
    #[test]
    fn a_change_approved_after_the_reopening_and_shipped_again_is_shipped_by_the_second_move() {
        let result = shipped(&shipped_reopened()[3..], &change("c"), open, || FirstApproval::At(at(250)));
        // The first version of the slice is already shipped with no earlier one: counted as a move at 100, before the
        // approval; the move at 300 comes after it.
        assert_eq!(result, Shipped::Yes { subject: subject("auth"), at: at(300) });
    }

    /// change.drop.06: a subject that never moved to shipped with the change on its list asks nothing of git.
    #[test]
    fn a_change_no_ship_names_is_not_shipped_and_the_history_is_not_read() {
        let versions = vec![version("auth", 10, SubjectStatus::Proposed, &["c"]), version("auth", 20, SubjectStatus::Shipped, &["other"])];
        assert_eq!(shipped(&versions, &change("c"), open, never), Shipped::No);
        assert_eq!(shipped(&[], &change("c"), open, never), Shipped::No);
    }

    /// change.drop.06: history that cannot be read refuses, and a short history only when it cannot show the answer.
    #[test]
    fn a_history_that_cannot_be_read_is_unknown_and_a_short_one_only_when_it_cannot_say() {
        let versions = shipped_reopened();
        assert!(matches!(shipped(&versions, &change("c"), open, || FirstApproval::Unknown("no git".to_string())), Shipped::Unknown(why) if why == "no git"));
        // The oldest approval the clone holds is already before the ship: the first one is no later.
        assert_eq!(shipped(&versions, &change("c"), open, || FirstApproval::AtMost(at(80))), Shipped::Yes { subject: subject("auth"), at: at(100) });
        // The oldest approval it holds is after every ship: the earlier history that would say is missing.
        assert!(matches!(shipped(&versions[..4], &change("c"), open, || FirstApproval::AtMost(at(500))), Shipped::Unknown(_)));
    }

    /// A change with no decision in its history.
    fn open() -> Result<Decisions, String> {
        Ok(Vec::new())
    }

    /// change.drop.06: a subject that shipped while the change was deferred shipped without it, so the change did not ship.
    #[test]
    fn a_move_to_shipped_made_while_the_change_was_closed_is_no_ship_of_it() {
        let versions = shipped_reopened()[..4].to_vec();
        // Deferred at 95, before the move at 100.
        let deferred = || Ok(vec![(at(95), ChangeDecision::Deferred)]);
        assert_eq!(shipped(&versions, &change("c"), deferred, never), Shipped::No, "the approval history is not even read");
        let dropped = || Ok(vec![(at(40), ChangeDecision::Deferred), (at(95), ChangeDecision::Dropped)]);
        assert_eq!(shipped(&versions, &change("c"), dropped, never), Shipped::No);
        // Deferred after the move: the subject had shipped with it.
        let later = || Ok(vec![(at(150), ChangeDecision::Deferred)]);
        assert_eq!(shipped(&versions, &change("c"), later, || FirstApproval::At(at(50))), Shipped::Yes { subject: subject("auth"), at: at(100) });
    }

    /// change.drop.06: the change reopened before the second move: that move is a ship; one still deferred is not.
    #[test]
    fn a_move_after_the_reopening_is_a_ship_and_the_closed_one_before_it_is_not() {
        let versions = shipped_reopened();
        // Deferred at 95 (the move at 100 is skipped), reopened at 250 (the move at 300 counts).
        let reopened = || Ok(vec![(at(95), ChangeDecision::Deferred), (at(250), ChangeDecision::Reopened)]);
        assert_eq!(shipped(&versions, &change("c"), reopened, || FirstApproval::At(at(50))), Shipped::Yes { subject: subject("auth"), at: at(300) });
        // Deferred again at 260: the move at 300 was made while the change was closed again.
        let again = || Ok(vec![(at(95), ChangeDecision::Deferred), (at(250), ChangeDecision::Reopened), (at(260), ChangeDecision::Deferred)]);
        assert_eq!(shipped(&versions, &change("c"), again, never), Shipped::No);
        // The order of the history does not matter, only the times.
        let shuffled = || Ok(vec![(at(250), ChangeDecision::Reopened), (at(95), ChangeDecision::Deferred)]);
        assert_eq!(shipped(&versions, &change("c"), shuffled, || FirstApproval::At(at(50))), Shipped::Yes { subject: subject("auth"), at: at(300) });
    }

    /// change.drop.06: a decision history that cannot be read refuses (fails closed), and only when a move names the change.
    #[test]
    fn a_decision_history_that_cannot_be_read_is_unknown() {
        let unreadable = || Err("no ledger".to_string());
        assert!(matches!(shipped(&shipped_reopened(), &change("c"), unreadable, never), Shipped::Unknown(why) if why == "no ledger"));
        assert_eq!(shipped(&[], &change("c"), unreadable, never), Shipped::No);
    }

    /// The versions of two subjects do not mix: a move of one is not a move of the other.
    #[test]
    fn the_moves_of_each_subject_are_read_apart() {
        let versions = vec![
            version("auth", 10, SubjectStatus::Verifying, &["c"]),
            version("billing", 20, SubjectStatus::Shipped, &["c"]),
            version("auth", 30, SubjectStatus::Shipped, &["c"]),
        ];
        let moves = ship_moves(&versions, &change("c"));
        assert_eq!(moves, vec![(subject("billing"), at(20)), (subject("auth"), at(30))]);
    }

    fn state(decision: ChangeDecision, reason: &str, refs: &[&str], approver: &str) -> ChangeState {
        let envelope = Envelope::current(RecordKind::ChangeState, at(1000), Actor::new_unattributed(approver));
        ChangeState::new(envelope, change("c"), decision, reason, refs.iter().map(ToString::to_string).collect(), approver)
    }

    fn call<'a>(id: &'a ChangeId, reason: &'a str, refs: &'a [String], approver: &'a str) -> Call<'a> {
        Call { change: id, reason, refs, approver, relayed_via: None, actor: None, dry_run: false, json: false }
    }

    /// change.drop.01, 02, 07, 08, 19: the table of the module doc.
    #[test]
    fn the_state_of_a_change_decides_what_each_leaf_may_do() {
        let id = change("c");
        let refs = vec!["r1".to_string()];
        let same = call(&id, "why", &refs, "Ana");
        let other = call(&id, "another reason", &refs, "Ana");
        let deferred = state(ChangeDecision::Deferred, "why", &["r1"], "Ana");
        let dropped = state(ChangeDecision::Dropped, "why", &["r1"], "Ana");
        let reopened = state(ChangeDecision::Reopened, "why", &["r1"], "Ana");

        for current in [None, Some(&reopened)] {
            assert_eq!(judge(Leaf::Defer, current, &same), Verdict::Proceed);
            assert_eq!(judge(Leaf::Drop, current, &same), Verdict::Proceed);
            assert!(matches!(judge(Leaf::Reopen, current, &same), Verdict::Conflict(Conflict { reason: "NOT_DEFERRED", .. })));
        }
        assert_eq!(judge(Leaf::Defer, Some(&deferred), &same), Verdict::Unchanged);
        assert!(matches!(judge(Leaf::Defer, Some(&deferred), &other), Verdict::Conflict(Conflict { reason: "CLOSED", .. })));
        assert_eq!(judge(Leaf::Drop, Some(&deferred), &other), Verdict::Proceed);
        assert_eq!(judge(Leaf::Reopen, Some(&deferred), &other), Verdict::Proceed);
        assert_eq!(judge(Leaf::Drop, Some(&dropped), &same), Verdict::Unchanged);
        assert!(matches!(judge(Leaf::Drop, Some(&dropped), &other), Verdict::Conflict(Conflict { reason: "CLOSED", .. })));
        assert!(matches!(judge(Leaf::Defer, Some(&dropped), &same), Verdict::Conflict(Conflict { reason: "CLOSED", .. })));
        assert!(matches!(judge(Leaf::Reopen, Some(&dropped), &same), Verdict::Conflict(Conflict { reason: "DROPPED", .. })));
    }

    /// A repeat is the same call whatever order its references come in.
    #[test]
    fn a_repeat_with_the_same_references_in_another_order_is_unchanged() {
        let id = change("c");
        let refs = vec!["r2".to_string(), "r1".to_string()];
        let deferred = state(ChangeDecision::Deferred, "why", &["r1", "r2"], "Ana");
        assert_eq!(judge(Leaf::Defer, Some(&deferred), &call(&id, "why", &refs, "Ana")), Verdict::Unchanged);
        let fewer = vec!["r1".to_string()];
        assert!(matches!(judge(Leaf::Defer, Some(&deferred), &call(&id, "why", &fewer, "Ana")), Verdict::Conflict(_)));
        assert!(matches!(judge(Leaf::Defer, Some(&deferred), &call(&id, "why", &refs, "Bo")), Verdict::Conflict(_)));
    }

    /// change.drop.18: an approval of the reopening's second or earlier is void; a later one stands.
    #[test]
    fn an_approval_no_later_than_the_reopening_second_predates_it() {
        let reopened = DateTime::from_timestamp(1000, 700_000_000).expect("a time");
        assert!(approval_predates(Some(at(999)), reopened));
        assert!(approval_predates(Some(at(1000)), reopened), "the same second cannot be told apart, so it predates");
        assert!(!approval_predates(Some(at(1001)), reopened));
        assert!(approval_predates(None, reopened), "a line with no readable time cannot be shown to come after");
    }
}
