//! Whether a subject's brief is approved (0.16 D1, 0.17 D3): the one
//! decision `canon status` reports, the `verifying → shipped` guard of
//! `canon subject move` enforces, and `canon evidence add` refuses on
//! (0.16 M7). All call [`brief_of`], so status never says `ship` for a
//! subject the guard would refuse on its brief, never suggests an
//! attestation `evidence add` would refuse, and the guards never refuse
//! a subject move reads as needing no brief.
//!
//! One human answer approves a change's brief and the policy. The human
//! records it, from that answer only, as an `Approved:` line in canon's own
//! file for the change, `.canon/changes/<change>/brief.md` (no record kind),
//! read outside HTML comments and code fences
//! ([`canon_ingest::markdown::field`]); `canon brief approve` takes several
//! ids and records one answer for all (0.31). Fixes stay in the approved
//! change.
//!
//! # The changes a subject answers for (0.31)
//! A subject answers for every adopted change that is **tagged** (a
//! scenario it owns is tagged `@flow:<change>.*`), has an **approved** line
//! (edited since or not), or is a **chore** ([`governing`], [`Reason`]).
//! Reopen, ship and `canon status` judge each of them, whatever the order of
//! adoption and of approval. A never-approved, untagged feature change is a
//! pending draft and blocks nothing. With no such change the subject keeps
//! the older rule: its newest adopted change awaits its first approval.
//! Evidence follows the scenario ([`attested_briefs`]): a tagged scenario
//! answers to the changes it tags, an untagged one to every change with an
//! approved line; an unapproved chore never blocks evidence.
//!
//! # The approval is bound to the brief's bytes (0.19 D3, #46)
//! `canon brief approve` records the line as
//! `Approved: <who> <when> — "<words>" (brief sha256:<digest>)`, the
//! digest over the brief with every `Approved:` line removed
//! ([`brief_digest`]). Every reader goes through [`approval_of`]: a digest
//! that no longer matches means the brief changed after the human
//! answered, so it is not approved ([`BriefStatus::changed`], code
//! [`CHANGED_CODE`]) until `canon brief approve` runs again on a new
//! answer. A line with no digest (written before 0.19) stays approved
//! ([`BriefStatus::legacy_approval`]); status advises re-approving it.
//!
//! # Canon never writes into an external plan (0.17 D3)
//! The approval is read only from that canon `brief.md`. A change
//! `canon change new` wrote has its brief there already. A change
//! imported from an external plan (an openspec change) gets its brief as
//! a canon file too: a `brief.md` whose `Source: <path>` line points at
//! the external document and which, once the human answers, holds the
//! `Approved:` line. Canon never edits the external document; an
//! `Approved:` line in it is not read.
//!
//! # Enforced only where a brief exists
//! The approval is enforced ([`BriefStatus::enforced`]) only when there
//! is a brief a human can answer for the change: its
//! canon `brief.md`, or, with no canon `brief.md`, an external openspec
//! `proposal.md` under the plan sources. Without an `Approved:` line in
//! the canon `brief.md` (which, in the second case, does not exist yet),
//! status lists the human step as the subject's only step, `canon
//! evidence add` refuses the subject's scenarios and tasks, and the ship
//! guard refuses; each names the canon file to write, with its `Source:`
//! pointer when the plan is external, and the approval to record. With
//! no adopted change or neither file, there is nothing to answer: status
//! still shows the brief as not approved, the ship guard prints it as an
//! advisory, and nothing blocks.
//!
//! # The approval is never assumed (0.16 M7)
//! An agent that writes the brief stops there and reports it; nothing is
//! built or attested for the subject until the human answers. The ship
//! guard and the evidence refusal are backstops for that stop:
//! [`attested_briefs`] names the briefs an attestation would build
//! under, through the scenario's `@subject` owner and through every
//! subject adopting a task's change.
//!
//! # A `proposed` subject
//! A `proposed` subject's brief is read too, but only once there is one
//! to read (dogfood eval 3 H3): its newest adopted change has a canon
//! `brief.md` or an external `proposal.md`. That is the moment the human
//! is needed, so status shows the brief line and, when it is not
//! approved, the human step instead of the steps that start the work. A
//! `proposed` subject with no adopted change or no brief has no brief
//! yet, as before. A `specced` subject's brief is not read. The ship
//! guard reads the brief on `verifying → shipped` only.

use std::path::{Path, PathBuf};

use canon_gate::spec_coverage::{latest_scenario, latest_subjects, subject_change_ids};
use canon_gate::GateContext;
use canon_ingest::markdown::{field, without_field};
use canon_model::{paths, ChangeId, ProjectId, ScenarioId, Subject, SubjectStatus, TaskId};
use chrono::{DateTime, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// The error code readers print when a brief changed after its approval.
pub const CHANGED_CODE: &str = "BRIEF_CHANGED_SINCE_APPROVAL";

/// `sha256:<hex>` over `text` with every `Approved:` line removed: what
/// an approval binds to.
pub fn brief_digest(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(without_field(text, "Approved:").as_bytes()))
}

/// [`brief_digest`] as a [`SpecDigest`]: what a change-level review of a
/// chore pins (0.23 D5).
pub fn brief_spec_digest(text: &str) -> canon_model::SpecDigest {
    canon_model::SpecDigest::of(without_field(text, "Approved:").as_bytes())
}

/// What a brief's `Approved:` line says about its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Approval {
    /// No `Approved:` line.
    Missing,
    /// The line's digest matches the brief's text.
    Bound { line: String },
    /// A line with no digest (pre-0.19): approved, advisory to re-approve.
    Unbound { line: String },
    /// The line's digest does not match: the brief changed after it.
    Changed { line: String, recorded: String, current: String },
}

impl Approval {
    /// Whether the brief reads as approved.
    pub fn approved(&self) -> bool {
        matches!(self, Approval::Bound { .. } | Approval::Unbound { .. })
    }
}

/// The digest an `Approved:` line carries: its trailing
/// `(brief sha256:<64 hex>)`.
fn recorded_digest(line: &str) -> Option<&str> {
    let inner = line.trim_end().strip_suffix(')')?;
    let (_, digest) = inner.rsplit_once("(brief ")?;
    let hex = digest.strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(digest)
}

/// Reads the approval of the brief `text` (module doc). The first
/// readable `Approved:` line decides, as [`field`] reads it.
pub fn approval_of(text: &str) -> Approval {
    let Some(line) = field(text, "Approved:") else { return Approval::Missing };
    let line = line.to_string();
    match recorded_digest(&line) {
        None => Approval::Unbound { line },
        Some(recorded) => {
            let current = brief_digest(text);
            if recorded == current {
                Approval::Bound { line }
            } else {
                Approval::Changed { recorded: recorded.to_string(), line, current }
            }
        }
    }
}

/// When the human approved: the `<when>` of an `Approved:` line,
/// `<who> <when> — "<words>" (brief sha256:<digest>)`. Written as a UTC
/// RFC 3339 time (0.22); a line from before that carries only a date,
/// read as the start of that day, UTC. `None` when no time is readable.
pub fn approved_at(line: &str) -> Option<DateTime<Utc>> {
    let head = line.split_once(" — ").map_or(line, |(head, _)| head);
    let when = head.split_whitespace().next_back()?;
    DateTime::parse_from_rfc3339(when)
        .map(|at| at.with_timezone(&Utc))
        .ok()
        .or_else(|| NaiveDate::parse_from_str(when, "%Y-%m-%d").ok().and_then(|day| day.and_hms_opt(0, 0, 0)).map(|at| at.and_utc()))
}

/// Whether one adopted change records the human's approval of its brief: an
/// `Approved:` line in that change's canon `brief.md` (module doc). A fix
/// stays inside the approved change, so a newer adopted change is a new
/// scope, and an older change's approval does not cover it. One entry per
/// change the subject answers for ([`governing`]) is `canon status`'s
/// `briefs`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BriefStatus {
    /// `approved`, `unapproved` or `changed` (edited after its approval).
    pub status: BriefState,
    /// Why the subject answers for this change ([`Reason`]); `None` for the
    /// newest adopted change of a subject with no governing change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    #[serde(skip)]
    pub approved: bool,
    /// Whether the missing approval blocks (module doc): not approved,
    /// and the change has a brief to answer. Then status
    /// asks the human and the ship guard refuses; otherwise both only
    /// report it.
    pub enforced: bool,
    /// The adopted change this entry judges, whose canon `brief.md` must carry
    /// the approval. Omitted when the subject adopts no change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    /// That change's canon `brief.md`, relative to the repo root: the one
    /// file the approval is read from. Omitted when the change has no
    /// brief to answer (neither a canon `brief.md` nor an external
    /// `proposal.md`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Whether [`Self::brief`] exists. `false` for an external change
    /// whose canon brief is still to be created, with a `Source:` line
    /// naming [`Self::source`].
    pub brief_exists: bool,
    /// The external plan document the change comes from: the `Source:`
    /// line of the canon `brief.md`, or, before that file exists, the
    /// external openspec `proposal.md` found under the plan sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The approval line's text after `Approved:`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<String>,
    /// The brief changed after its approval (0.19 D3): the line's digest
    /// no longer matches, so it is not approved and `canon brief approve`
    /// must run again.
    #[serde(skip)]
    pub changed: bool,
    /// The approval line has no digest (pre-0.19): approved, but status
    /// advises re-approving it so it binds to the brief's text.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub legacy_approval: bool,
    /// What `canon brief check` found in the brief's text (0.19 D2):
    /// filled by `canon status` only, for a brief in the PRD format.
    /// Omitted for a pre-0.19 brief, which is read as before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub findings: Option<crate::brief_check::Counts>,
    /// The advisory rule ids of that check, most findings first (the
    /// planner step's polish hint); never serialized.
    #[serde(skip)]
    pub advisory_rules: Vec<String>,
    /// The brief is in the format canon 0.30 and earlier wrote (0.31): a
    /// command that writes and judges it is refused ([`crate::old_briefs`]).
    #[serde(skip)]
    pub old_format: Option<crate::old_briefs::OldBrief>,
}

impl BriefStatus {
    /// Where the human reads the brief: the canon `brief.md`, or the
    /// external plan document before the canon file exists.
    pub fn report_in(&self) -> &str {
        match (self.brief_exists, &self.source, &self.brief) {
            (false, Some(source), _) => source,
            (_, _, Some(brief)) => brief,
            _ => "",
        }
    }

    /// Why the brief reads as unanswered, for an enforced brief.
    pub fn unanswered(&self) -> String {
        let brief = self.brief.as_deref().unwrap_or_default();
        match (self.brief_exists, &self.source) {
            (false, Some(source)) => format!(
                "{brief} does not exist yet: change `{}` comes from the external plan {source}, which canon reads and never edits, so its approval lives in that canon file",
                self.change.as_deref().unwrap_or_default()
            ),
            _ if self.changed => format!("{brief} changed after its approval ({CHANGED_CODE})"),
            _ => format!("{brief} has no `Approved:` line"),
        }
    }

    /// The re-approval advice for a digest-less or changed approval: the human's call, never the agent's.
    pub fn record_digest_hint(&self) -> String {
        format!(
            "the human re-approves with `canon brief approve {} --approver <their name> --note \"<their words>\"` in their own shell (or on `canon dashboard`); never run it yourself or unset your role to run it",
            self.change.as_deref().unwrap_or("<change>")
        )
    }

    /// How the human's answer is recorded, for an enforced brief: the human runs the call.
    pub fn record(&self) -> String {
        let brief = self.brief.as_deref().unwrap_or_default();
        if self.changed {
            return self.record_digest_hint();
        }
        let call = format!("`canon brief approve {} --approver <their name> --note \"<their words>\"` in their own shell (or on `canon dashboard`); never run it yourself or unset your role to run it", self.change.as_deref().unwrap_or("<change>"));
        match (self.brief_exists, &self.source) {
            (false, Some(source)) => format!("create {brief} holding a `Source: {source}` line; when they answer, the human records it with {call}"),
            _ => format!("when they answer, the human records it with {call}"),
        }
    }
}

/// The `openspec` plan source roots an external change's `proposal.md`
/// is found under, resolved the way `canon task done` resolves plan
/// sources (an absent `plans:` section falls back to the repo root).
/// `Err` names why `canon.yaml`'s `plans:` section cannot be read; then
/// no external proposal is found.
pub fn plan_roots(repo: &Path) -> Result<Vec<PathBuf>, crate::plans::PlansError> {
    crate::plans::load_plan_sources_for_gate(repo)
        .map(|sources| sources.iter().filter(|source| source.dialect() == "openspec").map(|source| source.root().to_path_buf()).collect())
}

/// `change`'s canon `brief.md` under `repo`'s `.canon/changes/`, a live
/// dir before an archived one; `None` when neither holds the file.
pub(crate) fn canon_brief(repo: &Path, change: &ChangeId) -> Option<PathBuf> {
    let changes = repo.join(paths::CHANGES_DIR);
    [changes.join(change.as_str()), changes.join("archive").join(change.as_str())].into_iter().map(|dir| dir.join("brief.md")).find(|path| path.is_file())
}

/// `path` relative to `repo`, for output.
fn rel(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo).unwrap_or(path).display().to_string()
}

/// The brief decision for `subject`: `None` for a subject that needs no
/// brief yet (`specced`, `retired`, and a `proposed` subject whose newest
/// adopted change has no brief to answer), otherwise whether its newest
/// adopted change (the last of `changes`, the subject's adopted changes
/// joined from both sides by
/// [`canon_gate::spec_coverage::subject_change_ids`], adoption order)
/// carries an `Approved:` line in its canon `brief.md` (module doc). An
/// external openspec `proposal.md` under `plan_roots` (the first root
/// holding the change wins, as in `canon ingest plans`) is a brief to
/// answer only while no canon `brief.md` exists; its own lines are never
/// read.
pub fn brief_of(repo: &Path, plan_roots: &[PathBuf], subject: &Subject, changes: &[ChangeId]) -> Option<BriefStatus> {
    let proposed = subject.status == SubjectStatus::Proposed;
    if !proposed && !matches!(subject.status, SubjectStatus::Building | SubjectStatus::Verifying | SubjectStatus::Shipped) {
        return None;
    }
    let Some(change) = changes.last() else {
        return (!proposed).then_some(BriefStatus {
            status: BriefState::Unapproved,
            reason: None,
            approved: false,
            enforced: false,
            change: None,
            brief: None,
            brief_exists: false,
            source: None,
            approval: None,
            changed: false,
            legacy_approval: false,
            findings: None,
            advisory_rules: Vec::new(),
            old_format: None,
        });
    };
    let brief = canon_brief(repo, change);
    let external = || plan_roots.iter().find_map(|root| canon_ingest::plan_adapters::openspec::proposal_path(root, change));
    let status = match brief {
        Some(path) => {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let verdict = approval_of(&text);
            let approval = field(&text, "Approved:").map(str::to_string);
            let old_format = canon_ingest::brief::parse(&text).old_format.map(|old| crate::old_briefs::OldBrief { change: change.as_str().to_string(), path: rel(repo, &path), what: old.describe() });
            BriefStatus {
                status: BriefState::of(&verdict),
                reason: None,
                approved: verdict.approved(),
                enforced: !verdict.approved(),
                change: Some(change.as_str().to_string()),
                brief: Some(rel(repo, &path)),
                brief_exists: true,
                source: field(&text, "Source:").map(str::to_string),
                approval,
                changed: matches!(verdict, Approval::Changed { .. }),
                legacy_approval: matches!(verdict, Approval::Unbound { .. }),
                findings: None,
                advisory_rules: Vec::new(),
                old_format,
            }
        }
        None => match external() {
            Some(proposal) => BriefStatus {
                status: BriefState::Unapproved,
                reason: None,
                approved: false,
                enforced: true,
                change: Some(change.as_str().to_string()),
                brief: Some(format!("{}/{}/brief.md", paths::CHANGES_DIR, change.as_str())),
                brief_exists: false,
                source: Some(rel(repo, &proposal)),
                approval: None,
                changed: false,
                legacy_approval: false,
                findings: None,
                advisory_rules: Vec::new(),
                old_format: None,
            },
            None if proposed => return None,
            None => BriefStatus {
                status: BriefState::Unapproved,
                reason: None,
                approved: false,
                enforced: false,
                change: Some(change.as_str().to_string()),
                brief: None,
                brief_exists: false,
                source: None,
                approval: None,
                changed: false,
                legacy_approval: false,
                findings: None,
                advisory_rules: Vec::new(),
                old_format: None,
            },
        },
    };
    Some(status)
}

/// Whether `change`'s canon `brief.md` carries an `Approved:` line (approved
/// now, or edited since).
fn has_approved_line(repo: &Path, change: &ChangeId) -> bool {
    canon_brief(repo, change)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|text| field(&text, "Approved:").is_some())
}

/// What a brief's approval reads as ([`BriefStatus::status`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BriefState {
    /// An `Approved:` line binds the brief's text (or predates digests).
    Approved,
    /// No `Approved:` line, or no canon brief yet.
    Unapproved,
    /// The brief changed after its approval.
    Changed,
}

impl BriefState {
    fn of(approval: &Approval) -> Self {
        match approval {
            Approval::Bound { .. } | Approval::Unbound { .. } => Self::Approved,
            Approval::Missing => Self::Unapproved,
            Approval::Changed { .. } => Self::Changed,
        }
    }
}

/// Why a subject answers for an adopted change ([`governing`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    /// A scenario the subject owns is tagged `@flow:<change>.*`.
    Tagged,
    /// The change's brief carries an `Approved:` line (edited since or not),
    /// whatever tags say.
    Approved,
    /// The change is a chore: it has no scenario to tag, so it governs from
    /// the moment it is adopted.
    Chore,
}

/// A subject's adopted changes sorted by how the subject-level brief rules
/// read them (0.31).
#[derive(Debug, Clone, Default)]
pub struct Governing {
    /// The changes the subject answers for, in adoption order: reopen, ship
    /// and status judge each one's brief.
    pub governed: Vec<(ChangeId, Reason)>,
    /// The adopted changes no rule reads: never approved, not a chore, no
    /// scenario tagged for them. They block nothing.
    pub drafts: Vec<ChangeId>,
    /// With no governing change, the newest adopted change, whose first
    /// approval the subject waits for (the pre-0.31 rule, kept for a subject
    /// that has no approved scope yet).
    pub awaiting: Option<ChangeId>,
}

impl Governing {
    /// The changes whose brief reopen, ship and the structure check read, in
    /// adoption order.
    pub fn judged(&self) -> Vec<ChangeId> {
        self.governed.iter().map(|(change, _)| change.clone()).chain(self.awaiting.clone()).collect()
    }

    /// The last judged change: where a reopen keeps its follow-up.
    pub fn last(&self) -> Option<ChangeId> {
        self.judged().pop()
    }

    /// The reason `change` governs, when it does.
    pub fn reason_of(&self, change: &ChangeId) -> Option<Reason> {
        self.governed.iter().find(|(c, _)| c == change).map(|(_, reason)| *reason)
    }
}

/// Sorts the subject's adopted `changes` (module doc, 0.31): a change
/// governs when a scenario the subject owns is tagged `@flow:<change>.*`
/// ([`Reason::Tagged`]), when its brief has an `Approved:` line
/// ([`Reason::Approved`]) or when it is a chore ([`Reason::Chore`]); the
/// rest are pending drafts. Independent of adoption order and approval order.
pub fn governing(repo: &Path, ctx: &GateContext, flow_changes: &crate::brief_check::FlowChangesIndex, subject: &Subject, changes: &[ChangeId]) -> Governing {
    let tagged: Vec<ChangeId> = canon_gate::spec_coverage::subject_scenarios(ctx, &subject.subject_id)
        .into_iter()
        .flat_map(|s| flow_changes.changes_of(repo, &s.project_id, &s.scenario_id).iter().cloned())
        .collect();
    let mut out = Governing::default();
    for change in changes {
        let text = canon_brief(repo, change).and_then(|path| std::fs::read_to_string(path).ok());
        let reason = if tagged.contains(change) {
            Some(Reason::Tagged)
        } else if text.as_deref().is_some_and(|text| field(text, "Approved:").is_some()) {
            Some(Reason::Approved)
        } else if text.as_deref().is_some_and(crate::brief_check::is_chore) {
            Some(Reason::Chore)
        } else {
            None
        };
        match reason {
            Some(reason) => out.governed.push((change.clone(), reason)),
            None => out.drafts.push(change.clone()),
        }
    }
    if out.governed.is_empty() {
        out.awaiting = changes.last().cloned();
        out.drafts.retain(|draft| Some(draft) != out.awaiting.as_ref());
    }
    out
}

/// The one line a transition prints when `drafts` are pending drafts,
/// naming every one.
pub fn pending_draft_line(drafts: &[ChangeId]) -> String {
    let names = drafts.iter().map(|d| format!("`{}`", d.as_str())).collect::<Vec<_>>().join(", ");
    let (verb, tags) = if drafts.len() == 1 { ("is a pending draft", format!("`@flow:{}.*`", drafts[0].as_str())) } else { ("are pending drafts", "`@flow:<change>.*`".to_string()) };
    format!("{names} {verb} (never approved, not a chore, no scenario tagged {tags}): not part of this transition, which judges the subject's tagged, approved and chore changes")
}

/// How the human's answer to `blockers` is recorded, for the briefs that wait
/// for it: [`BriefStatus::record`] for one, otherwise one call naming every
/// id, preceded by the canon `brief.md` an external change still needs.
pub fn record_all(blockers: &[&BriefStatus]) -> String {
    match blockers {
        [only] => only.record(),
        many => {
            let creates = many
                .iter()
                .filter(|brief| !brief.brief_exists)
                .map(|brief| format!("create {} holding a `Source: {}` line", brief.brief.as_deref().unwrap_or_default(), brief.source.as_deref().unwrap_or_default()))
                .collect::<Vec<_>>();
            let call = approve_all_call(many);
            if creates.is_empty() {
                format!("when they answer, the human records them with {call}")
            } else {
                format!("{}; when they answer, the human records them with {call}", creates.join("; "))
            }
        }
    }
}

/// The human's call that answers every brief in `blockers` at once, as the
/// hint of a refusal names it: one `canon brief approve <id>…` line.
pub fn approve_all_call(blockers: &[&BriefStatus]) -> String {
    let ids = blockers.iter().filter_map(|b| b.change.as_deref()).collect::<Vec<_>>().join(" ");
    format!("`canon brief approve {ids} --approver <their name> --note \"<their words>\"` in their own shell (or on `canon dashboard`); never run it yourself or unset your role to run it")
}

/// When `subject` first adopted a change: the earliest of its own record
/// versions (ledger and archive) with a non-empty change list and of the
/// change versions (ledger and archive) naming it. `None` when the history
/// cannot tell.
fn first_adopted_at(ctx: &GateContext, subject: &Subject) -> Option<DateTime<Utc>> {
    let mut subjects: Vec<Subject> = ctx.subjects.clone();
    let mut changes: Vec<canon_model::Change> = ctx.changes.clone();
    if let Some(archive) = canon_store::git_tier::GitTier::new(&ctx.ctx.ledger_root).archive_tier() {
        use canon_store::tier::{Tier, TierQuery};
        if let Ok(read) = archive.read(&TierQuery::kind(canon_model::RecordKind::Subject)) {
            subjects.extend(read.records.iter().filter_map(|r| serde_json::from_value::<Subject>(r.0.clone()).ok()));
        }
        if let Ok(read) = archive.read(&TierQuery::kind(canon_model::RecordKind::Change)) {
            changes.extend(read.records.iter().filter_map(|r| serde_json::from_value::<canon_model::Change>(r.0.clone()).ok()));
        }
    }
    let by_subject = subjects.iter().filter(|s| s.subject_id == subject.subject_id && !s.change_ids.is_empty()).map(|s| s.envelope.at);
    let by_change = changes.iter().filter(|c| c.subject_ids.contains(&subject.subject_id)).map(|c| c.envelope.at);
    by_subject.chain(by_change).min()
}

/// The briefs in the 0.30 format among every governing change of the subject the scenario (or task) belongs to, not
/// only the ones the evidence answers for: an old-format brief the subject is governed by makes `evidence add` refuse
/// (0.31 `brief-format`, [`crate::old_briefs`]).
pub fn old_format_governing(
    repo: &Path,
    ctx: &GateContext,
    flow_changes: &crate::brief_check::FlowChangesIndex,
    task: Option<&TaskId>,
    scenario: Option<(&ProjectId, &ScenarioId)>,
) -> Vec<crate::old_briefs::OldBrief> {
    let owner = scenario.and_then(|(project, id)| latest_scenario(ctx, project, id).and_then(|s| s.subject_id.as_ref()));
    let task_change = task.map(TaskId::change_id);
    let mut out: Vec<crate::old_briefs::OldBrief> = Vec::new();
    for subject in latest_subjects(ctx) {
        let changes = subject_change_ids(ctx, subject);
        let involved = owner == Some(&subject.subject_id) || task_change.as_ref().is_some_and(|change| changes.contains(change));
        if !involved {
            continue;
        }
        let governed: Vec<ChangeId> = governing(repo, ctx, flow_changes, subject, &changes).governed.into_iter().map(|(change, _)| change).collect();
        for old in crate::old_briefs::among(repo, &governed) {
            if !out.contains(&old) {
                out.push(old);
            }
        }
    }
    out
}

/// The changes whose briefs a scenario without a `@flow:` tag answers to
/// (0.25 D1, 0.31), among the subject's adopted `changes`:
/// 1. every one whose canon `brief.md` carries an `Approved:` line (approved
///    now, or edited since: [`BriefState::Changed`] still refuses; a chore
///    with one included), so a change adopted later does not freeze
///    scenarios an approved scope already covers, and an unapproved chore
///    never blocks evidence;
/// 2. otherwise, when the scenario already has an evidence record, recorded
///    before the subject first adopted a change and carrying no brief
///    override (`evidenced`), none: it is existing
///    behavior, attested before any approved scope, so a subject built
///    before briefs existed keeps re-verifying (a waiver never becomes a
///    standing exemption);
/// 3. otherwise the change the subject waits on for a first approval: its
///    newest tagged change, or, with no governing change at all, its newest
///    adopted change ([`Governing::awaiting`]). A chore or a pending draft
///    is never it, so neither blocks evidence.
fn untagged_scope(repo: &Path, changes: &[ChangeId], waiting_on: Option<ChangeId>, evidenced: impl FnOnce() -> bool) -> Vec<ChangeId> {
    let approved: Vec<ChangeId> = changes.iter().filter(|change| has_approved_line(repo, change)).cloned().collect();
    if !approved.is_empty() {
        return approved;
    }
    if evidenced() {
        return Vec::new();
    }
    waiting_on.into_iter().collect()
}

/// Every subject an attestation for `task` and/or `scenario` builds
/// under, with the brief of the change it answers to (module doc, 0.16
/// M7; scoped by 0.25 D1), sorted by subject id:
///
/// - a scenario, through its `@subject` owner, answers to the change of
///   each `@flow:<change>.<F-id>` tag it carries; with no such tag, to the
///   owner's [`untagged_scope`] (which may be none);
/// - a task answers to its own change only, under every subject that
///   adopted it (joined from both sides, [`subject_change_ids`]).
///
/// Read through the gate's own folds over `ctx`. A scenario the index does
/// not hold, or one with no `@subject` tag, names no subject; a change
/// with no brief to answer, or a subject that needs no brief yet, is left
/// out.
pub fn attested_briefs<'a>(
    repo: &Path,
    plan_roots: &[PathBuf],
    ctx: &'a GateContext,
    flow_changes: &crate::brief_check::FlowChangesIndex,
    task: Option<&TaskId>,
    scenario: Option<(&ProjectId, &ScenarioId)>,
) -> Vec<(&'a Subject, BriefStatus)> {
    let owner = scenario.and_then(|(project, id)| latest_scenario(ctx, project, id).and_then(|s| s.subject_id.as_ref()));
    let task_change = task.map(TaskId::change_id);
    let tagged = scenario.map(|(project, id)| flow_changes.changes_of(repo, project, id).to_vec()).unwrap_or_default();
    let mut subjects: Vec<(&Subject, Vec<ChangeId>)> = latest_subjects(ctx)
        .into_iter()
        .map(|s| (s, subject_change_ids(ctx, s)))
        .collect();
    subjects.sort_by(|a, b| a.0.subject_id.as_str().cmp(b.0.subject_id.as_str()));
    let mut out: Vec<(&Subject, BriefStatus)> = Vec::new();
    for (subject, changes) in subjects {
        let mut answers: Vec<ChangeId> = Vec::new();
        if owner == Some(&subject.subject_id) {
            // A tag counts only for a change this subject adopted.
            let counted: Vec<ChangeId> = tagged.iter().filter(|c| changes.contains(c)).cloned().collect();
            if counted.is_empty() {
                let evidenced = || {
                    scenario.is_some_and(|(project, id)| {
                        canon_gate::history::evidence_history(ctx)
                            .iter()
                            .chain(&ctx.audits)
                            .any(|r| {
                                r.brief_override.is_none()
                                    && r.project_id.as_ref() == Some(project)
                                    && r.scenario_id.as_ref() == Some(id)
                                    && first_adopted_at(ctx, subject).is_some_and(|first| r.envelope.at < first)
                            })
                    })
                };
                let governing = governing(repo, ctx, flow_changes, subject, &changes);
                let waiting_on = governing.governed.iter().rev().find(|(_, reason)| *reason == Reason::Tagged).map(|(change, _)| change.clone()).or(governing.awaiting.clone());
                answers.extend(untagged_scope(repo, &changes, waiting_on, evidenced));
            } else {
                answers.extend(counted);
            }
        }
        if let Some(change) = task_change
            .as_ref()
            .filter(|change| changes.contains(change))
        {
            answers.push(change.clone());
        }
        answers.dedup();
        let mut seen: Vec<ChangeId> = Vec::new();
        for change in answers {
            if seen.contains(&change) {
                continue;
            }
            if let Some(brief) = brief_of(repo, plan_roots, subject, std::slice::from_ref(&change))
            {
                out.push((subject, brief));
            }
            seen.push(change);
        }
    }
    out
}

/// The warning an adoption owes (0.25 D1): `subject` is building,
/// verifying or shipped and the adopted `change` has a brief that is not
/// approved. The subject's existing scenarios keep accepting evidence, so
/// the line says what does wait, and how to take the adoption back. `None`
/// for a proposed or specced subject, an approved brief, or a change with
/// no brief to answer.
pub fn adoption_warning(repo: &Path, subject: &Subject, change: &ChangeId) -> Option<String> {
    if !matches!(
        subject.status,
        SubjectStatus::Building | SubjectStatus::Verifying | SubjectStatus::Shipped
    ) {
        return None;
    }
    let roots = plan_roots(repo).unwrap_or_default();
    let brief = brief_of(repo, &roots, subject, std::slice::from_ref(change))
        .filter(|brief| brief.enforced)?;
    let (change, subject_id) = (change.as_str(), subject.subject_id.as_str());
    Some(format!(
        "subject `{subject_id}` is {} and change `{change}` is not approved ({}): the subject's existing scenarios keep accepting evidence, and while no scenario is tagged `@flow:{change}.*` it can still be reopened and shipped for its approved scope; once a scenario is tagged `@flow:{change}.*`, those scenarios wait for the approval and `{subject_id}` cannot ship until the brief is approved; to take the adoption back run `canon subject unadopt {subject_id} --change {change}`",
        serde_json::to_value(subject.status).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
        brief.unanswered()
    ))
}

/// One line per evidence record attested under a brief override (0.16
/// M7), oldest first: what it attests, who waived the brief and when, the
/// subjects waived, and the reason. `canon status` lists them as warnings
/// and `canon gate check` as advisories; neither fails on them.
pub fn override_lines(ctx: &GateContext) -> Vec<String> {
    let mut records: Vec<_> = ctx.evidence.iter().chain(&ctx.audits).filter(|r| r.brief_override.is_some()).collect();
    records.sort_by_key(|r| r.envelope.at);
    records
        .into_iter()
        .filter_map(|r| {
            let waiver = r.brief_override.as_ref()?;
            let what = [r.scenario_id.as_ref().map(|s| s.as_str().to_string()), r.task_id.as_ref().map(ToString::to_string)]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" and ");
            let subjects = waiver.waived.iter().map(|w| w.subject.as_str()).collect::<Vec<_>>().join(", ");
            Some(format!(
                "evidence for {what} was attested by `{}` at {} while the brief of {subjects} was not approved (override: {})",
                waiver.actor.agent_id,
                r.envelope.at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                waiver.reason
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::approved_at;

    #[test]
    fn an_unreadable_plans_section_is_refused_in_its_old_words() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "plans: 3\n").unwrap();
        let message = super::plan_roots(dir.path()).unwrap_err().to_string();
        assert!(message.starts_with("canon.yaml `plans:` section is malformed (fail-loud -- a silent fallback would scan zero sources while appearing configured): "), "{message}");
    }

    #[test]
    fn the_approval_time_reads_as_a_time_or_a_date_and_never_from_the_note() {
        let at = |line: &str| approved_at(line).map(|t| t.to_rfc3339());
        assert_eq!(at("Journey 2026-10-17T05:12:33Z — \"ok\" (brief sha256:ab)").as_deref(), Some("2026-10-17T05:12:33+00:00"));
        assert_eq!(at("Ana Kim 2026-10-12 — \"ship 2026-12-31\"").as_deref(), Some("2026-10-12T00:00:00+00:00"));
        assert_eq!(at("Journey — \"no time here\""), None);
    }
}
