//! Re-verification before shipping (0.22 D2, #72, #77): one freshness
//! rule, [`canon_gate::review_gate::scenarios_without_evidence_after`],
//! with two triggers. A subject cannot ship while one of its evidenced
//! scenarios has no evidence recorded after a trigger:
//!
//! - a blocker finding closed `fixed` (trigger: when it was raised); that
//!   half lives in the gate's `require_review` rule
//!   ([`canon_gate::review_gate::fixed_blockers`]) and keeps blocking
//!   under `block_on_findings`;
//! - an approved chore change on the subject (trigger: its approval time,
//!   [`crate::brief::approved_at`]); this half needs the change's
//!   `brief.md`, so it is read here, from the repo.
//!
//! New evidence stales a pinned review, so the re-review follows with no
//! rule of its own. Every refusal names the scenarios and the batch
//! `canon evidence add --project-id <root> --all-scenarios --report …`.

use std::path::Path;

use canon_gate::review_gate::{
    active_require_review, fixed_blockers, is_reverify, reverify_remedy, reverify_scenarios, scenarios_without_evidence_after,
};
use canon_gate::spec_coverage::subject_change_ids;
use canon_gate::{FailureClass, GateContext, Violation};
use canon_model::{ChangeId, Subject};
use chrono::{DateTime, SecondsFormat, Utc};

/// What a re-verification gap waits on.
#[derive(Debug, Clone)]
pub struct Gap {
    /// `blocker finding <change>#<round>.<seq> closed fixed` or
    /// `chore change <slug> approved`.
    pub trigger: String,
    pub since: DateTime<Utc>,
    /// The `(project_id, scenario_id)` pairs with no evidence after `since`.
    pub scenarios: Vec<(String, String)>,
}

impl Gap {
    fn pairs(&self) -> Vec<(&str, &str)> {
        self.scenarios.iter().map(|(project, id)| (project.as_str(), id.as_str())).collect()
    }

    /// The `canon evidence add … --all-scenarios` step this gap names.
    pub fn remedy(&self) -> String {
        reverify_remedy(&self.pairs())
    }

    /// `N evidenced scenario(s) (a, b, c, … k more)`.
    pub fn scenario_list(&self) -> String {
        reverify_scenarios(&self.pairs())
    }

    /// The project to pass to `--project-id`.
    pub fn project(&self) -> &str {
        self.scenarios.first().map_or("<root>", |(project, _)| project.as_str())
    }

    /// The step's text for `subject` in `canon status`.
    pub fn why(&self, subject: &str) -> String {
        format!(
            "{subject}: {} (since {}) and {} evidenced scenario(s) ({}) have no evidence recorded after it; the ship guard refuses until each does. Run the whole suite again, then the re-review follows (new evidence stales the pinned reviews)",
            self.trigger,
            self.since.to_rfc3339_opts(SecondsFormat::Secs, true),
            self.scenarios.len(),
            self.scenario_list()
        )
    }
}

/// The approved chore changes among `subject`'s adopted changes, each with
/// its approval time. A chore whose brief is not approved (or whose
/// approval line carries no readable time) triggers nothing.
fn chore_approvals(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<(ChangeId, DateTime<Utc>)> {
    subject_change_ids(ctx, subject)
        .into_iter()
        .filter_map(|change| {
            let text = std::fs::read_to_string(crate::brief::canon_brief(repo, &change)?).ok()?;
            if !crate::brief_check::is_chore(&text) || !crate::brief::approval_of(&text).approved() {
                return None;
            }
            let at = crate::brief::approved_at(canon_ingest::markdown::field(&text, "Approved:")?)?;
            Some((change, at))
        })
        .collect()
}

/// Every unmet chore trigger of `subject`.
pub fn chore_gaps(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<Gap> {
    chore_approvals(repo, ctx, subject)
        .into_iter()
        .filter_map(|(change, since)| {
            let scenarios: Vec<(String, String)> = scenarios_without_evidence_after(ctx, &subject.subject_id, since)
                .into_iter()
                .map(|s| (s.project_id.as_str().to_string(), s.scenario_id.as_str().to_string()))
                .collect();
            (!scenarios.is_empty()).then(|| Gap { trigger: format!("chore change `{}` approved", change.as_str()), since, scenarios })
        })
        .collect()
}

/// Every unmet trigger of `subject`, for `canon status`: the chore
/// triggers and, when `require_review.block_on_findings` is on, the
/// fixed blockers on its changes.
pub fn gaps(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<Gap> {
    let mut gaps = chore_gaps(repo, ctx, subject);
    if active_require_review(ctx).is_some_and(|(rr, _)| rr.block_on_findings) {
        let changes = subject_change_ids(ctx, subject);
        for fixed in fixed_blockers(ctx).into_iter().filter(|f| changes.contains(&f.finding.change_id)) {
            let scenarios: Vec<(String, String)> = scenarios_without_evidence_after(ctx, &subject.subject_id, fixed.raised)
                .into_iter()
                .map(|s| (s.project_id.as_str().to_string(), s.scenario_id.as_str().to_string()))
                .collect();
            if !scenarios.is_empty() {
                let f = fixed.finding;
                gaps.push(Gap { trigger: format!("blocker finding {}#{}.{} was closed fixed", f.change_id.as_str(), f.round, f.seq), since: fixed.raised, scenarios });
            }
        }
    }
    gaps
}

/// The `verifying → shipped` violations of the chore trigger: one
/// `uncovered-cell` per unapproved-by-evidence chore, naming `subject`.
pub fn chore_violations(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<Violation> {
    chore_gaps(repo, ctx, subject)
        .into_iter()
        .map(|gap| {
            Violation::new(
                FailureClass::UncoveredCell,
                subject.subject_id.as_str(),
                format!(
                    "{} {}: {} evidenced scenario(s) ({}) have no evidence recorded after the approval; {}",
                    gap.trigger,
                    gap.since.to_rfc3339_opts(SecondsFormat::Secs, true),
                    gap.scenarios.len(),
                    gap.scenario_list(),
                    gap.remedy()
                ),
            )
        })
        .collect()
}

/// The hint a refusal carries when any of `violations` is a
/// re-verification one: the batch command and what follows it.
pub fn hint(violations: &[&Violation]) -> Option<String> {
    violations.iter().find(|v| is_reverify(v)).map(|_| {
        "record the suite again for the scenarios named: `canon evidence add --project-id <root> --all-scenarios --report <junit|cucumber|swift-testing>:<path> --kind test-run --role implementer --ref \"<test command>\" --actor-id <unit> --session-id <session>`; the re-review follows (new evidence stales the pinned reviews), then ship".to_string()
    })
}
