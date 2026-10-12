//! Whether a subject's brief is approved (0.16 D1): the one decision
//! `canon status` reports, the `verifying → shipped` guard of
//! `canon subject status` enforces, and `canon evidence add` refuses on
//! (0.16 M7). All call [`brief_of`], so status never says `ship` for a
//! subject the guard would refuse on its brief, never suggests an
//! attestation `evidence add` would refuse, and the guards never refuse
//! a subject status reads as needing no brief.
//!
//! One human answer approves a subject's scope: its brief and the
//! policy. The agent records it, from that answer only, as an
//! `Approved:` line in the change's `proposal.md`, read under the
//! openspec plan sources (no record kind), outside HTML comments and
//! code fences. Fixes live as tasks in the approved change, so the
//! subject's newest adopted change is its current scope.
//!
//! # Enforced only where a brief exists
//! The approval is enforced ([`BriefStatus::enforced`]) only when there
//! is a brief a human can answer: the newest adopted change has a
//! `proposal.md` under the plan sources, and it carries no `Approved:`
//! line. Then status lists the human step as the subject's only step,
//! `canon evidence add` refuses the subject's scenarios and tasks, and
//! the ship guard refuses. With no plan source, no adopted change or no
//! `proposal.md`, there is nothing to answer: status still shows the
//! brief as not approved, the ship guard prints it as an advisory, and
//! nothing blocks.
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
//! to read (dogfood eval 3 H3): its newest adopted change has a
//! `proposal.md`. That is the moment the human is needed, so status
//! shows the brief line and, when it is not approved, the human step
//! instead of the steps that start the work. A `proposed` subject with
//! no adopted change or no `proposal.md` has no brief yet, as before. A
//! `specced` subject's brief is not read. The ship guard reads the brief
//! on `verifying → shipped` only.

use std::path::{Path, PathBuf};

use canon_gate::spec_coverage::{latest_scenarios, latest_subjects};
use canon_gate::GateContext;
use canon_model::{ProjectId, ScenarioId, Subject, SubjectStatus, TaskId};
use serde::Serialize;

/// Whether a subject's newest adopted change records the human's
/// approval of its brief: an `Approved:` line in that change's
/// `proposal.md`, read under the repo's `openspec` plan sources. A fix
/// lives as tasks inside the approved change, so a newer adopted change
/// is a new scope, and an older change's approval does not cover it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefStatus {
    pub approved: bool,
    /// Whether the missing approval blocks (module doc): not approved,
    /// and the newest adopted change has a `proposal.md` to carry it.
    /// Then status asks the human and the ship guard refuses; otherwise
    /// both only report it.
    pub enforced: bool,
    /// The newest adopted change (the last in `change_ids`, adoption
    /// order), whose `proposal.md` must carry the approval. Omitted when
    /// the subject adopts no change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    /// That change's `proposal.md`, relative to the repo root. Omitted
    /// when no `openspec` plan source holds one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<String>,
    /// The approval line's text after `Approved:`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<String>,
}

/// The `openspec` plan source roots briefs are read under, resolved the
/// way `canon gate task` resolves plan sources (an absent `plans:`
/// section falls back to the repo root). `Err` names why `canon.yaml`'s
/// `plans:` section cannot be read; then no brief reads as approved.
pub fn plan_roots(repo: &Path) -> Result<Vec<PathBuf>, String> {
    crate::plans::load_plan_sources_for_gate(repo)
        .map(|sources| sources.iter().filter(|source| source.dialect() == "openspec").map(|source| source.root().to_path_buf()).collect())
        .map_err(|e| e.to_string())
}

/// The brief decision for `subject`: `None` for a subject that needs no
/// brief yet (`specced`, `retired`, and a `proposed` subject whose newest
/// adopted change has no `proposal.md` under `plan_roots`), otherwise
/// whether its newest adopted change (the last in `change_ids`, adoption
/// order) carries an `Approved:` line in its `proposal.md` under
/// `plan_roots` (the first root holding the change wins, as in `canon
/// ingest plans`). A fix lives as tasks inside the approved change, so a
/// newer adopted change is a new scope that an older change's approval
/// does not cover.
pub fn brief_of(repo: &Path, plan_roots: &[PathBuf], subject: &Subject) -> Option<BriefStatus> {
    let proposed = subject.status == SubjectStatus::Proposed;
    if !proposed && !matches!(subject.status, SubjectStatus::Building | SubjectStatus::Verifying | SubjectStatus::Shipped) {
        return None;
    }
    let Some(change) = subject.change_ids.last() else {
        return (!proposed).then_some(BriefStatus { approved: false, enforced: false, change: None, proposal: None, approval: None });
    };
    let path = plan_roots.iter().find_map(|root| canon_ingest::plan_adapters::openspec::proposal_path(root, change));
    if proposed && path.is_none() {
        return None;
    }
    let text = path.as_ref().and_then(|path| std::fs::read_to_string(path).ok()).unwrap_or_default();
    let approval = approval_line(&text).map(str::to_string);
    Some(BriefStatus {
        approved: approval.is_some(),
        enforced: approval.is_none() && path.is_some(),
        change: Some(change.as_str().to_string()),
        proposal: path.map(|path| path.strip_prefix(repo).unwrap_or(&path).display().to_string()),
        approval,
    })
}

/// Every subject an attestation for `task` and/or `scenario` builds
/// under, with its brief (module doc, 0.16 M7), sorted by subject id:
/// the subject the scenario's latest index row names through its
/// `@subject` tag, and every subject whose `change_ids` holds the task's
/// change. Read through the gate's own folds over `ctx`. A scenario the
/// index does not hold, or one with no `@subject` tag, names no subject;
/// a subject that needs no brief yet is left out.
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
    let mut subjects: Vec<&Subject> = latest_subjects(ctx)
        .into_iter()
        .filter(|s| owner == Some(&s.subject_id) || change.as_ref().is_some_and(|change| s.change_ids.contains(change)))
        .collect();
    subjects.sort_by(|a, b| a.subject_id.as_str().cmp(b.subject_id.as_str()));
    subjects.into_iter().filter_map(|s| brief_of(repo, plan_roots, s).map(|brief| (s, brief))).collect()
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

/// The text after `Approved:` on the first line of `proposal` that
/// starts with it (leading whitespace allowed), says something, and sits
/// outside fenced code blocks (the same fence rule `canon init` reads
/// AGENTS.md with) and HTML comments, which may span lines: a quoted or
/// commented-out approval is not one.
fn approval_line(proposal: &str) -> Option<&str> {
    let mut fence: Option<(char, usize)> = None;
    let mut in_comment = false;
    for line in proposal.lines() {
        if !in_comment {
            match (fence, crate::init::fence_of(line)) {
                (None, Some((ch, run, _))) => {
                    fence = Some((ch, run));
                    continue;
                }
                (Some((open_ch, open_run)), Some((ch, run, true))) if ch == open_ch && run >= open_run => {
                    fence = None;
                    continue;
                }
                (Some(_), _) => continue,
                (None, None) => {}
            }
            if let Some(approval) = line.trim_start().strip_prefix("Approved:").map(str::trim).filter(|rest| !rest.is_empty()) {
                return Some(approval);
            }
        }
        in_comment = comment_open_after(line, in_comment);
    }
    None
}

/// Whether an HTML comment is open at the end of `line`, given whether
/// one was open at its start.
fn comment_open_after(line: &str, mut open: bool) -> bool {
    let mut rest = line;
    loop {
        let marker = if open { "-->" } else { "<!--" };
        let Some(at) = rest.find(marker) else {
            return open;
        };
        rest = &rest[at + marker.len()..];
        open = !open;
    }
}
