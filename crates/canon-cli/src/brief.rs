//! Whether a subject's brief is approved (0.16 D1, 0.17 D3): the one
//! decision `canon status` reports, the `verifying → shipped` guard of
//! `canon subject status` enforces, and `canon evidence add` refuses on
//! (0.16 M7). All call [`brief_of`], so status never says `ship` for a
//! subject the guard would refuse on its brief, never suggests an
//! attestation `evidence add` would refuse, and the guards never refuse
//! a subject status reads as needing no brief.
//!
//! One human answer approves a subject's scope: its brief and the
//! policy. The agent records it, from that answer only, as an
//! `Approved:` line in canon's own file for the change,
//! `.canon/changes/<change>/brief.md` (no record kind), read outside HTML
//! comments and code fences ([`canon_ingest::markdown::field`]). Fixes
//! stay in the approved change, so the subject's newest adopted
//! change is its current scope.
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
//! is a brief a human can answer for the newest adopted change: its
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

use canon_gate::spec_coverage::{latest_scenarios, latest_subjects, subject_change_ids};
use canon_gate::GateContext;
use canon_ingest::markdown::{field, without_field};
use canon_model::{paths, ChangeId, ProjectId, ScenarioId, Subject, SubjectStatus, TaskId};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// The error code readers print when a brief changed after its approval.
pub const CHANGED_CODE: &str = "BRIEF_CHANGED_SINCE_APPROVAL";

/// `sha256:<hex>` over `text` with every `Approved:` line removed: what
/// an approval binds to.
pub fn brief_digest(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(without_field(text, "Approved:").as_bytes()))
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

/// Whether a subject's newest adopted change records the human's
/// approval of its brief: an `Approved:` line in that change's canon
/// `brief.md` (module doc). A fix stays inside the approved
/// change, so a newer adopted change is a new scope, and an older
/// change's approval does not cover it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefStatus {
    pub approved: bool,
    /// Whether the missing approval blocks (module doc): not approved,
    /// and the newest adopted change has a brief to answer. Then status
    /// asks the human and the ship guard refuses; otherwise both only
    /// report it.
    pub enforced: bool,
    /// The newest adopted change (the last of the subject's adopted
    /// changes, adoption order), whose canon `brief.md` must carry the
    /// approval. Omitted when the subject adopts no change.
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
    #[serde(skip_serializing_if = "std::ops::Not::not")]
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

    /// The re-approval advice for a digest-less approval line.
    pub fn record_digest_hint(&self) -> String {
        format!("re-approve with `canon brief approve --change {} --by <the human> --note \"<their words>\"`", self.change.as_deref().unwrap_or("<change>"))
    }

    /// How the agent records the human's answer, for an enforced brief.
    pub fn record(&self) -> String {
        let brief = self.brief.as_deref().unwrap_or_default();
        if self.changed {
            return self.record_digest_hint();
        }
        match (self.brief_exists, &self.source) {
            (false, Some(source)) => format!(
                "create {brief} holding a `Source: {source}` line, then record their words with `canon brief approve --change {} --by <the human> --note \"<their words>\"`",
                self.change.as_deref().unwrap_or("<change>")
            ),
            _ => format!("record their words with `canon brief approve --change {} --by <the human> --note \"<their words>\"`", self.change.as_deref().unwrap_or("<change>")),
        }
    }
}

/// The `openspec` plan source roots an external change's `proposal.md`
/// is found under, resolved the way `canon gate task` resolves plan
/// sources (an absent `plans:` section falls back to the repo root).
/// `Err` names why `canon.yaml`'s `plans:` section cannot be read; then
/// no external proposal is found.
pub fn plan_roots(repo: &Path) -> Result<Vec<PathBuf>, String> {
    crate::plans::load_plan_sources_for_gate(repo)
        .map(|sources| sources.iter().filter(|source| source.dialect() == "openspec").map(|source| source.root().to_path_buf()).collect())
        .map_err(|e| e.to_string())
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
        });
    };
    let brief = canon_brief(repo, change);
    let external = || plan_roots.iter().find_map(|root| canon_ingest::plan_adapters::openspec::proposal_path(root, change));
    let status = match brief {
        Some(path) => {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let verdict = approval_of(&text);
            let approval = field(&text, "Approved:").map(str::to_string);
            BriefStatus {
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
            }
        }
        None => match external() {
            Some(proposal) => BriefStatus {
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
            },
            None if proposed => return None,
            None => BriefStatus {
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
            },
        },
    };
    Some(status)
}

/// Every subject an attestation for `task` and/or `scenario` builds
/// under, with its brief (module doc, 0.16 M7), sorted by subject id:
/// the subject the scenario's latest index row names through its
/// `@subject` tag, and every subject that adopted the task's change
/// (joined from both sides, [`subject_change_ids`]: a change shared by
/// several subjects builds under each). Read through the gate's own
/// folds over `ctx`. A scenario the index does not hold, or one with no
/// `@subject` tag, names no subject; a subject that needs no brief yet
/// is left out.
pub fn attested_briefs<'a>(
    repo: &Path,
    plan_roots: &[PathBuf],
    ctx: &'a GateContext,
    task: Option<&TaskId>,
    scenario: Option<(&ProjectId, &ScenarioId)>,
) -> Vec<(&'a Subject, BriefStatus)> {
    let owner = scenario.and_then(|(project, id)| {
        latest_scenarios(ctx).into_iter().find(|s| &s.project_id == project && &s.scenario_id == id).and_then(|s| s.subject_id.as_ref())
    });
    let change = task.map(TaskId::change_id);
    let mut subjects: Vec<(&Subject, Vec<ChangeId>)> = latest_subjects(ctx)
        .into_iter()
        .map(|s| (s, subject_change_ids(ctx, s)))
        .filter(|(s, changes)| owner == Some(&s.subject_id) || change.as_ref().is_some_and(|change| changes.contains(change)))
        .collect();
    subjects.sort_by(|a, b| a.0.subject_id.as_str().cmp(b.0.subject_id.as_str()));
    subjects.into_iter().filter_map(|(s, changes)| brief_of(repo, plan_roots, s, &changes).map(|brief| (s, brief))).collect()
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
    fn the_approval_time_reads_as_a_time_or_a_date_and_never_from_the_note() {
        let at = |line: &str| approved_at(line).map(|t| t.to_rfc3339());
        assert_eq!(at("Journey 2026-10-17T05:12:33Z — \"ok\" (brief sha256:ab)").as_deref(), Some("2026-10-17T05:12:33+00:00"));
        assert_eq!(at("Ana Kim 2026-10-12 — \"ship 2026-12-31\"").as_deref(), Some("2026-10-12T00:00:00+00:00"));
        assert_eq!(at("Journey — \"no time here\""), None);
    }
}
