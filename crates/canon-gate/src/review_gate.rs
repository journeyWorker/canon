//! `spec_coverage.require_review` (issue #2): "done" needs an independent
//! review, and no open blocker finding.
//!
//! Without this, a subject could reach `verifying`/`shipped` on evidence
//! its own implementer attested to: `require_evidence` counts any
//! evidence record, and `trust_required` keys on an evidence record's
//! class, so neither notices that nobody else ever looked. This module
//! is the one place the review rule lives. [`crate::spec_coverage`]'s
//! check reports it through `canon gate check`, and `canon subject
//! status` runs the same per-subject rule ([`subject_guard`]) before it
//! writes a transition.
//!
//! # What counts as a qualifying review
//! A [`Review`] record for the scenario's `(project_id, scenario_id)`.
//! With `distinct_actor`, its `reviewer` and its envelope actor must both
//! differ from every actor on that scenario's evidence records. Review
//! carries no verdict: the `verifying → shipped` ship gate already
//! refuses missing and `divergent` verdicts, and that stays the verdict
//! rule.
//!
//! # Open blockers
//! A [`Finding`] is folded to its latest version per natural key
//! `(change_id, round, seq)` first, with the same supersession rule
//! `canon report`'s `finding_latest` SQL applies (`version_rank`, which is
//! [`canon_store::fold::fold_latest_by_key`]'s ordering). A finding that
//! was raised `open` and later closed as `fixed`/`rejected`/`deferred`
//! therefore no longer counts. One whose latest version is still `open`
//! with severity `blocker`, on a change listed in an in-scope subject's
//! `change_ids`, is an `open-blocker` violation.
//!
//! A Finding row the ledger read refused (unparseable, schema-invalid,
//! misfiled) is a `malformed-evidence` violation naming its file, for
//! every subject, and no waiver covers it. The rule cannot tell whether
//! the row it could not read is an open blocker, or a newer version that
//! reopened one, so it fails closed. A Review row the read refused needs
//! no such rule: dropping it can only remove a review, which leaves its
//! scenario `unreviewed-promotion`, never cleared.
//!
//! # Waivers
//! `canon subject status --override-reason` records a
//! [`StatusOverride`] on the subject listing exactly the violations it
//! let through, as `(class, subject)` pairs. While the subject stays at
//! the status the waiver was granted for, a violation matching one of
//! those pairs is reported as a [`ReviewAdvisory`] naming the waiver.
//! Anything else stays a violation: a blocker raised after the waiver, or
//! a scenario tagged to the subject later, was never waived. A waiver is
//! visible, never silent, and never wider than what it named.

use std::collections::{BTreeMap, BTreeSet};

use canon_model::{ChangeId, Finding, FindingDisposition, FindingSeverity, RecordKind, Scenario, StatusOverride, Subject, SubjectId, SubjectStatus};

use crate::context::GateContext;
use crate::failure_class::{FailureClass, Violation};
use crate::policy::{subject_status_name, RequireReview, SpecCoverage};
use crate::spec_coverage::{latest_by_key, scope_decision, subject_scenarios, ScopeDecision};

/// One review gap a recorded waiver let through: reported by `canon gate
/// check` as an advisory, never as a violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewAdvisory {
    /// The violation this would have been without the waiver.
    pub violation: Violation,
    /// The subject whose waiver covers it.
    pub subject_id: SubjectId,
    pub waiver: StatusOverride,
}

impl ReviewAdvisory {
    /// `<class> <subject> — <detail> [waiver: …]`, the advisory
    /// counterpart of [`Violation::line`].
    pub fn line(&self) -> String {
        format!(
            "{} [waiver: subject `{}` moved to {} by `{}`: {}]",
            self.violation.line(),
            self.subject_id.as_str(),
            subject_status_name(self.waiver.to),
            self.waiver.actor.agent_id,
            self.waiver.reason
        )
    }
}

/// The two halves of one evaluation.
#[derive(Debug, Default)]
pub(crate) struct ReviewOutcome {
    pub violations: Vec<Violation>,
    pub advisories: Vec<ReviewAdvisory>,
}

/// The active `require_review` settings and the `exclude_lanes` they are
/// applied with, or `None` when the policy does not ask for review.
pub fn active_require_review(ctx: &GateContext) -> Option<(&RequireReview, &[String])> {
    match &ctx.policy.spec_coverage {
        Some(SpecCoverage::Active { require_review: Some(require_review), exclude_lanes, .. }) => Some((require_review, exclude_lanes.as_slice())),
        _ => None,
    }
}

/// The kinds this rule reads that route away from the rung the gate
/// reads. A non-empty answer means the rule cannot run honestly: it would
/// see no reviews and report every scenario, or see no findings and pass.
pub fn unreadable_review_kinds(ctx: &GateContext, require_review: &RequireReview) -> Vec<RecordKind> {
    let mut kinds = vec![RecordKind::Review];
    if require_review.block_on_findings {
        kinds.push(RecordKind::Finding);
    }
    kinds.retain(|kind| ctx.unreadable_kinds.contains(kind));
    kinds
}

/// The violation reported when [`unreadable_review_kinds`] is non-empty.
pub fn unreadable_violation(kinds: &[RecordKind]) -> Violation {
    let kinds = kinds.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ");
    Violation::new(
        FailureClass::UncoveredCell,
        "spec_coverage.require_review",
        format!("`require_review` is enabled but these kinds route away from the rung the gate reads ({kinds}), so the rule would judge an empty corpus; route them to `local` or remove `require_review`"),
    )
}

/// Every in-scope advisory `canon gate check` prints below its
/// violations. `None` when `require_review` is not active, or when it
/// cannot run (its violation is already in the check's output).
pub fn review_advisories(ctx: &GateContext) -> Option<Vec<ReviewAdvisory>> {
    let (require_review, exclude_lanes) = active_require_review(ctx)?;
    if !unreadable_review_kinds(ctx, require_review).is_empty() {
        return None;
    }
    Some(evaluate(ctx, require_review, exclude_lanes).advisories)
}

/// The corpus-wide rule `canon gate check` applies.
pub(crate) fn evaluate(ctx: &GateContext, require_review: &RequireReview, exclude_lanes: &[String]) -> ReviewOutcome {
    let subjects: BTreeMap<&str, &Subject> =
        latest_by_key(&ctx.subjects, |s| s.subject_id.as_str().to_string()).into_iter().map(|s| (s.subject_id.as_str(), s)).collect();
    let status: BTreeMap<&str, SubjectStatus> = subjects.iter().map(|(id, s)| (*id, s.status)).collect();
    // The waiver of `id`'s subject, if it recorded exactly `violation`.
    let waiver_for =
        |id: &str, violation: &Violation| subjects.get(id).and_then(|s| current_waiver(s)).filter(|w| w.covers(violation.class.as_str(), &violation.subject));

    let mut outcome = ReviewOutcome::default();
    for scenario in latest_by_key(&ctx.scenarios, |s| (s.project_id.clone(), s.scenario_id.clone())) {
        // A scenario naming a subject no record carries has no status
        // that could put it in scope; `spec_coverage` reports the
        // dangling link itself.
        if !matches!(scope_decision(&require_review.scope, scenario.subject_id.as_ref(), &status), ScopeDecision::InScope) {
            continue;
        }
        if is_excluded(scenario, exclude_lanes) {
            continue;
        }
        let Some(violation) = review_gap(ctx, scenario, require_review.distinct_actor) else { continue };
        match scenario.subject_id.as_ref().and_then(|id| waiver_for(id.as_str(), &violation).map(|w| (id, w))) {
            Some((subject_id, waiver)) => outcome.advisories.push(ReviewAdvisory { violation, subject_id: subject_id.clone(), waiver: waiver.clone() }),
            None => outcome.violations.push(violation),
        }
    }
    // Never waivable: a waiver names violations by subject, and nobody
    // can know which subject an unreadable finding belongs to.
    outcome.violations.extend(malformed_findings(ctx, require_review));

    if require_review.block_on_findings {
        // A change can belong to several subjects. Its blocker is waived
        // only when EVERY in-scope subject holding it recorded that exact
        // blocker in its waiver.
        let mut holders: BTreeMap<&str, Vec<&Subject>> = BTreeMap::new();
        for subject in subjects.values().filter(|s| require_review.covers(s.status)) {
            for change_id in &subject.change_ids {
                holders.entry(change_id.as_str()).or_default().push(subject);
            }
        }
        for finding in open_blockers(ctx) {
            let Some(holding) = holders.get(finding.change_id.as_str()) else { continue };
            let names: Vec<&str> = holding.iter().map(|s| s.subject_id.as_str()).collect();
            let violation = open_blocker_violation(finding, &names);
            let waived: Option<(&Subject, &StatusOverride)> = holding
                .iter()
                .map(|s| waiver_for(s.subject_id.as_str(), &violation).map(|w| (*s, w)))
                .collect::<Option<Vec<_>>>()
                .and_then(|all| all.into_iter().next());
            match waived {
                Some((subject, waiver)) => {
                    outcome.advisories.push(ReviewAdvisory { violation, subject_id: subject.subject_id.clone(), waiver: waiver.clone() })
                }
                None => outcome.violations.push(violation),
            }
        }
    }
    outcome
}

/// The per-subject rule `canon subject status` applies before moving
/// `subject_id` into a status `require_review.scope` covers: every
/// scenario the subject owns needs a qualifying review, and (with
/// `block_on_findings`) none of `change_ids` may carry an open blocker.
/// The same [`review_gap`] and [`open_blockers`] the corpus-wide rule
/// uses, so the guard and `canon gate check` cannot disagree.
pub fn subject_guard(
    ctx: &GateContext,
    require_review: &RequireReview,
    exclude_lanes: &[String],
    subject_id: &SubjectId,
    change_ids: &[ChangeId],
) -> Vec<Violation> {
    let mut violations: Vec<Violation> = subject_scenarios(ctx, subject_id)
        .into_iter()
        .filter(|s| !is_excluded(s, exclude_lanes))
        .filter_map(|s| review_gap(ctx, s, require_review.distinct_actor))
        .collect();
    if require_review.block_on_findings {
        let changes: BTreeSet<&str> = change_ids.iter().map(ChangeId::as_str).collect();
        violations.extend(
            open_blockers(ctx).into_iter().filter(|f| changes.contains(f.change_id.as_str())).map(|f| open_blocker_violation(f, &[subject_id.as_str()])),
        );
    }
    violations
}

/// One `malformed-evidence` violation per Finding row the ledger read
/// refused, named by its ledger file (module doc), when
/// `block_on_findings` is on. Empty otherwise. Both `canon gate check`
/// and `canon subject status` refuse on these, and neither lets
/// `--override-reason` waive them.
pub fn malformed_findings(ctx: &GateContext, require_review: &RequireReview) -> Vec<Violation> {
    if !require_review.block_on_findings {
        return Vec::new();
    }
    ctx.corpus_violations
        .iter()
        .filter(|(kind, _)| *kind == RecordKind::Finding)
        .map(|(_, violation)| {
            Violation::new(
                FailureClass::MalformedEvidence,
                violation.subject.clone(),
                format!(
                    "unreadable finding record: {} ({}); require_review.block_on_findings cannot tell whether it is an open blocker, so the gate fails closed — fix or remove the record",
                    violation.detail,
                    violation.class.as_str()
                ),
            )
        })
        .collect()
}

/// The waiver a subject's latest record carries, if it was granted for
/// the status the subject is still at.
fn current_waiver(subject: &Subject) -> Option<&StatusOverride> {
    subject.status_override.as_ref().filter(|w| w.to == subject.status)
}

fn is_excluded(scenario: &Scenario, exclude_lanes: &[String]) -> bool {
    scenario.lane.as_ref().is_some_and(|lane| exclude_lanes.contains(lane))
}

/// `Some(violation)` when `scenario` has no qualifying review (module
/// doc), naming which rule failed.
///
/// Evidence actors are collected from every evidence record for the
/// scenario's id whose `project_id` matches or is absent. Counting a
/// project-less record here is the stricter reading: it can only make a
/// review not count, never let one count.
pub fn review_gap(ctx: &GateContext, scenario: &Scenario, distinct_actor: bool) -> Option<Violation> {
    let reviews: Vec<_> = ctx.reviews.iter().filter(|r| r.project_id == scenario.project_id && r.scenario_id == scenario.scenario_id).collect();
    let subject = scenario.scenario_id.as_str();
    if reviews.is_empty() {
        return Some(Violation::new(
            FailureClass::UnreviewedPromotion,
            subject,
            "no review record for this scenario; spec_coverage.require_review wants an independent review (`canon review add`) once its subject is claimed done",
        ));
    }
    if !distinct_actor {
        return None;
    }
    let evidence_actors: BTreeSet<&str> = ctx
        .evidence
        .iter()
        .filter(|e| e.scenario_id.as_ref() == Some(&scenario.scenario_id) && e.project_id.as_ref().is_none_or(|p| *p == scenario.project_id))
        .map(|e| e.envelope.actor.agent_id.as_str())
        .collect();
    let independent = reviews.iter().any(|r| !evidence_actors.contains(r.reviewer.as_str()) && !evidence_actors.contains(r.envelope.actor.agent_id.as_str()));
    if independent {
        return None;
    }
    let actors = evidence_actors.iter().map(|a| format!("`{a}`")).collect::<Vec<_>>().join(", ");
    Some(Violation::new(
        FailureClass::UnreviewedPromotion,
        subject,
        format!(
            "every review of this scenario is by an actor that also attested its evidence ({actors}); spec_coverage.require_review.distinct_actor wants a reviewer other than the evidence actor"
        ),
    ))
}

/// The latest version of every Finding per `(change_id, round, seq)`
/// whose disposition is still `open` and whose severity is `blocker`,
/// in natural-key order.
pub fn open_blockers(ctx: &GateContext) -> Vec<&Finding> {
    latest_by_key(&ctx.findings, |f| (f.change_id.as_str().to_string(), f.round, f.seq))
        .into_iter()
        .filter(|f| f.severity == FindingSeverity::Blocker && f.disposition() == FindingDisposition::Open)
        .collect()
}

fn open_blocker_violation(finding: &Finding, subjects: &[&str]) -> Violation {
    let subjects = subjects.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", ");
    Violation::new(
        FailureClass::OpenBlocker,
        format!("{}#{}.{}", finding.change_id.as_str(), finding.round, finding.seq),
        format!(
            "open blocker finding by `{}` on change `{}` (subject {subjects}): {}; fix it and `canon finding close --disposition fixed --resolution-sha <sha>`, or close it as rejected or deferred",
            finding.reviewer,
            finding.change_id.as_str(),
            finding.summary
        ),
    )
}
