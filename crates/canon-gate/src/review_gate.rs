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
//! The reviews of the scenario's `(project_id, scenario_id)` are folded to
//! the LATEST per `reviewer` first, with the store's supersession order
//! (`at`, then `schema`, then content digest). A reviewer's later review
//! replaces their earlier one, so a reviewer who signed and then recorded
//! `indeterminate` no longer counts as signing. One of those latest
//! reviews must:
//! - be `signed` (0.15 D4). `indeterminate` and `unavailable` are gaps
//!   with their own detail, never an approval and never a rejection.
//! - still match what it pinned (0.15 D1, reshaped by 0.23 D5): the
//!   scenario's current digest of the kind it pinned — its own
//!   `scenario_digest` when the record had one at review time, else the
//!   file-wide `source_digest` — and what the latest evidence per role says
//!   now, its verdict and the identity of the checks it bound
//!   ([`crate::freshness`]). Evidence recorded again with the same verdict
//!   from the same checks leaves the review current. Otherwise it is a
//!   stale review, and the detail names whether the spec, the evidence, or
//!   both changed. A review pinned before 0.23 (to evidence record ids, or
//!   to nothing) is not comparable: it is stale until `canon migrate`
//!   rewrites it.
//! - with `distinct_actor`, have a `reviewer` and an envelope actor that
//!   both differ from every actor on that scenario's evidence records, and
//!   when its envelope carries a `session_id`, that session must differ
//!   from every `session_id` on those evidence records. A record without a
//!   session never matches one, so records written before sessions were
//!   recorded keep counting by actor alone.
//!
//! Review carries no verdict about the behavior: the `verifying → shipped`
//! ship gate already refuses missing and `divergent` verdicts, and that
//! stays the verdict rule.
//!
//! # Open blockers
//! A [`Finding`] is folded to its latest version per natural key
//! `(change_id, round, seq)` first, with the same supersession rule
//! `canon report`'s `finding_latest` SQL applies (`version_rank`, which is
//! [`canon_store::fold::fold_latest_by_key`]'s ordering). A finding that
//! was raised `open` and later closed as `fixed`/`rejected`/`deferred`
//! therefore no longer counts. One whose latest version is still `open`
//! with severity `blocker`, on a change an in-scope subject adopted
//! ([`subject_change_ids`]: its `change_ids` and every change whose
//! `subject_ids` names it), is an `open-blocker` violation. A change
//! shared by several subjects gates each of them.
//!
//! # Fixed blockers need re-verification (0.22 D2, #72)
//! A blocker whose latest disposition is `fixed` keeps blocking, as an
//! `open-blocker` violation, until every evidenced scenario of each
//! subject holding its change has an evidence record newer than the
//! finding's `fixed` record (when it was closed, [`fixed_blockers`]):
//! evidence recorded before the fix cannot verify it. A finding that
//! names its scenarios ([`Finding::scenario_ids`], 0.24.1, #87) asks only
//! those ([`FixedBlocker::unmet`]); one that names none asks every
//! evidenced scenario.
//! [`crate::freshness::scenarios_without_evidence_after`] is that rule. A
//! scenario review stays current when the new evidence says the same
//! thing (0.23 D5); the finding's close is the reviewer's step. The chore
//! trigger (a chore change approved on the subject) reads the same rule
//! from `canon subject status`, and asks for one change-level review
//! instead of a re-review per scenario ([`change_review_gap`]).
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

use chrono::{DateTime, Utc};

use canon_model::{
    ChangeId, CheckPin, Finding, FindingDisposition, FindingSeverity, PinnedSpec, RecordKind, Review, Scenario, SpecDigest, StatusOverride, Subject, SubjectId, SubjectStatus,
};

use crate::context::GateContext;
use crate::failure_class::{FailureClass, Violation};
use crate::freshness::{current_checks, drift, scenario_evidence, scenarios_without_evidence_after, Drift};
use crate::policy::{subject_status_name, RequireReview, SpecCoverage};
use crate::spec_coverage::{latest_by_key, latest_scenarios, latest_subjects, scope_decision, subject_change_ids, subject_scenarios, ScopeDecision};

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
pub(crate) struct Evaluation {
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
pub(crate) fn evaluate(ctx: &GateContext, require_review: &RequireReview, exclude_lanes: &[String]) -> Evaluation {
    let subjects: BTreeMap<&str, &Subject> = latest_subjects(ctx).into_iter().map(|s| (s.subject_id.as_str(), s)).collect();
    let status: BTreeMap<&str, SubjectStatus> = subjects.iter().map(|(id, s)| (*id, s.status)).collect();
    // The waiver of `id`'s subject, if it recorded exactly `violation`.
    let waiver_for =
        |id: &str, violation: &Violation| subjects.get(id).and_then(|s| current_waiver(s)).filter(|w| w.covers(violation.class.as_str(), &violation.subject));

    let mut outcome = Evaluation::default();
    for scenario in latest_scenarios(ctx) {
        // A scenario naming a subject no record carries has no status
        // that could put it in scope; `spec_coverage` reports the
        // dangling link itself.
        if !matches!(scope_decision(&require_review.scope, scenario.subject_id.as_ref(), &status), ScopeDecision::InScope) {
            continue;
        }
        if is_excluded(scenario, exclude_lanes) {
            continue;
        }
        let Some(ReviewGap { violation, .. }) = review_gap(ctx, scenario, require_review.distinct_actor) else { continue };
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
        let mut holders: BTreeMap<ChangeId, Vec<&Subject>> = BTreeMap::new();
        for subject in subjects.values().filter(|s| require_review.covers(s.status)) {
            for change_id in subject_change_ids(ctx, subject) {
                holders.entry(change_id).or_default().push(subject);
            }
        }
        for finding in open_blockers(ctx) {
            let Some(holding) = holders.get(&finding.change_id) else { continue };
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
        for fixed in fixed_blockers(ctx) {
            let Some(holding) = holders.get(&fixed.finding.change_id) else { continue };
            let mut unmet: BTreeSet<(&str, &str)> = BTreeSet::new();
            for subject in holding {
                unmet.extend(fixed.unmet(ctx, &subject.subject_id).into_iter().map(|s| (s.project_id.as_str(), s.scenario_id.as_str())));
            }
            if unmet.is_empty() {
                continue;
            }
            let names: Vec<&str> = holding.iter().map(|s| s.subject_id.as_str()).collect();
            let violation = fixed_blocker_violation(ctx, &fixed, &names, &unmet.into_iter().collect::<Vec<_>>());
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
/// `subject` into a status `require_review.scope` covers: every
/// scenario the subject owns needs a qualifying review, and (with
/// `block_on_findings`) none of its adopted changes
/// ([`subject_change_ids`]) may carry an open blocker. The same
/// [`review_gap`] and [`open_blockers`] the corpus-wide rule uses, so
/// the guard and `canon gate check` cannot disagree.
pub fn subject_guard(ctx: &GateContext, require_review: &RequireReview, exclude_lanes: &[String], subject: &Subject) -> Vec<Violation> {
    let subject_id = &subject.subject_id;
    let mut violations: Vec<Violation> = subject_scenarios(ctx, subject_id)
        .into_iter()
        .filter(|s| !is_excluded(s, exclude_lanes))
        .filter_map(|s| review_gap(ctx, s, require_review.distinct_actor).map(|gap| gap.violation))
        .collect();
    if require_review.block_on_findings {
        let changes: BTreeSet<ChangeId> = subject_change_ids(ctx, subject).into_iter().collect();
        violations.extend(
            open_blockers(ctx).into_iter().filter(|f| changes.contains(&f.change_id)).map(|f| open_blocker_violation(f, &[subject_id.as_str()])),
        );
        for fixed in fixed_blockers(ctx).into_iter().filter(|f| changes.contains(&f.finding.change_id)) {
            let unmet: Vec<(&str, &str)> = fixed.unmet(ctx, subject_id).into_iter().map(|s| (s.project_id.as_str(), s.scenario_id.as_str())).collect();
            if !unmet.is_empty() {
                violations.push(fixed_blocker_violation(ctx, &fixed, &[subject_id.as_str()], &unmet));
            }
        }
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

/// Why a scenario has no qualifying review (module doc), most actionable
/// first: `canon status` counts the kinds, the gate reports the detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewGapKind {
    /// No review record at all.
    Missing,
    /// Every latest review is `indeterminate` or `unavailable` (D4).
    Unsigned,
    /// Every signed latest review is pinned to a spec or evidence that
    /// has since changed (D1).
    Stale,
    /// A current signed review exists, but none by an actor and session
    /// other than the evidence's (`distinct_actor`).
    NotDistinct,
}

/// One scenario's review gap: its kind and the `unreviewed-promotion`
/// violation the gate reports for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewGap {
    pub kind: ReviewGapKind,
    pub violation: Violation,
}


/// The latest scenario review per `reviewer` of `(project_id, scenario_id)`
/// (module doc), in reviewer order.
fn latest_reviews<'a>(ctx: &'a GateContext, scenario: &Scenario) -> Vec<&'a Review> {
    let reviews: Vec<&Review> = ctx.reviews.iter().filter(|r| r.project_id == scenario.project_id && r.scenario_id.as_ref() == Some(&scenario.scenario_id)).collect();
    latest_by_key(&reviews, |r| r.reviewer.clone()).into_iter().copied().collect()
}

/// The in-effect reviews: the latest per `(project_id, scenario_id or
/// change_id, reviewer)`, the fold every review rule reads (module doc).
pub fn in_effect_reviews(ctx: &GateContext) -> Vec<&Review> {
    latest_by_key(&ctx.reviews, |r| (r.project_id.clone(), r.scenario_id.clone(), r.change_id.clone(), r.reviewer.clone()))
}

/// The in-effect reviews that are still current (0.18 D5): every
/// [`in_effect_reviews`] entry except a scenario review that no longer
/// describes its live scenario ([`drift`]). A stale review is superseded:
/// the gate reads the scenario's newer review in its place, or reports the
/// gap itself (`unreviewed-promotion`), so nothing the stale one bound is
/// still claimed. A review whose scenario has no live record to compare it
/// with is kept, as is a change-level review (its currency is the chore
/// rule's): dropping either would only ever skip a check. Review artifacts
/// are re-checked over exactly these ([`crate::artifact_store`]).
pub fn current_reviews(ctx: &GateContext) -> Vec<&Review> {
    let live: BTreeMap<(&canon_model::ProjectId, &canon_model::ScenarioId), &Scenario> =
        latest_scenarios(ctx).into_iter().map(|s| ((&s.project_id, &s.scenario_id), s)).collect();
    let mut checks: BTreeMap<(&canon_model::ProjectId, &canon_model::ScenarioId), Vec<CheckPin>> = BTreeMap::new();
    let mut current = Vec::new();
    for review in in_effect_reviews(ctx) {
        let Some(scenario_id) = &review.scenario_id else {
            current.push(review);
            continue;
        };
        let key = (&review.project_id, scenario_id);
        let Some(scenario) = live.get(&key).copied() else {
            current.push(review);
            continue;
        };
        let now = checks.entry(key).or_insert_with(|| current_checks(ctx, scenario));
        if drift(review.pins.as_ref(), scenario, now).is_none() {
            current.push(review);
        }
    }
    current
}

/// The scenarios whose in-effect review predates 0.23 review pinning (it
/// pins evidence record ids, or nothing), sorted. The gate does not count
/// such a review; `canon migrate` rewrites it.
pub fn unmigrated_review_scenarios(ctx: &GateContext) -> Vec<String> {
    let live: BTreeSet<(&canon_model::ProjectId, &canon_model::ScenarioId)> = latest_scenarios(ctx).into_iter().map(|s| (&s.project_id, &s.scenario_id)).collect();
    // A scenario another review already covers is owed nothing: the old
    // review `canon migrate` left stale beside it is only history.
    let covered: BTreeSet<(&canon_model::ProjectId, &canon_model::ScenarioId)> =
        current_reviews(ctx).into_iter().filter_map(|r| r.scenario_id.as_ref().map(|id| (&r.project_id, id))).collect();
    let ids: BTreeSet<String> = in_effect_reviews(ctx)
        .into_iter()
        .filter(|r| r.scenario_id.as_ref().is_none_or(|id| !covered.contains(&(&r.project_id, id))))
        .filter(|r| r.pins.as_ref().is_none_or(|p| p.checks.is_none()))
        .filter_map(|r| r.scenario_id.as_ref().filter(|id| live.contains(&(&r.project_id, id))).map(|id| id.as_str().to_string()))
        .collect();
    ids.into_iter().collect()
}

/// The one advisory line `canon gate check`, `canon subject status` and
/// `canon status` print about reviews [`unmigrated_review_scenarios`]
/// names, or `None` when there are none or `require_review` is not
/// active and readable.
pub fn unmigrated_review_note(ctx: &GateContext) -> Option<String> {
    let (require_review, _) = active_require_review(ctx)?;
    if !unreadable_review_kinds(ctx, require_review).is_empty() {
        return None;
    }
    let ids = unmigrated_review_scenarios(ctx);
    if ids.is_empty() {
        return None;
    }
    let shown = ids.iter().take(5).map(String::as_str).collect::<Vec<_>>().join(", ");
    let more = if ids.len() > 5 { format!(", … {} more", ids.len() - 5) } else { String::new() };
    Some(format!(
        "{} scenario(s) have a review that predates 0.23 review pinning ({shown}{more}); such a review does not count until `canon migrate` rewrites it to what the evidence says",
        ids.len()
    ))
}

/// `Some(gap)` when `scenario` has no qualifying review (module doc),
/// naming which rule failed. When several latest reviews fail for
/// different reasons, the one closest to qualifying is reported: a
/// current signed review failing only `distinct_actor`, then a stale
/// signed review, then non-signed outcomes.
pub fn review_gap(ctx: &GateContext, scenario: &Scenario, distinct_actor: bool) -> Option<ReviewGap> {
    let subject = scenario.scenario_id.as_str();
    let gap = |kind, detail: String| Some(ReviewGap { kind, violation: Violation::new(FailureClass::UnreviewedPromotion, subject, detail) });
    let reviews = latest_reviews(ctx, scenario);
    if reviews.is_empty() {
        return gap(
            ReviewGapKind::Missing,
            "no review record for this scenario; spec_coverage.require_review wants an independent review (`canon review scenario`, or `canon review change` for a chore) once its subject is claimed done".to_string(),
        );
    }
    let now = current_checks(ctx, scenario);
    let (signed, unsigned): (Vec<&Review>, Vec<&Review>) = reviews.into_iter().partition(|r| r.outcome.is_signed());
    let mut stale: Vec<(&Review, Drift)> = Vec::new();
    let mut fresh: Vec<&Review> = Vec::new();
    for review in signed {
        match drift(review.pins.as_ref(), scenario, &now) {
            Some(changed) => stale.push((review, changed)),
            None => fresh.push(review),
        }
    }

    if !fresh.is_empty() {
        if !distinct_actor {
            return None;
        }
        let evidence = scenario_evidence(ctx, scenario);
        let evidence_actors: BTreeSet<&str> = evidence.iter().map(|e| e.envelope.actor.agent_id.as_str()).collect();
        let evidence_sessions: BTreeSet<&str> = evidence.iter().filter_map(|e| e.envelope.actor.session_id.as_ref()).map(|s| s.as_str()).collect();
        let distinct_actors = |r: &Review| !evidence_actors.contains(r.reviewer.as_str()) && !evidence_actors.contains(r.envelope.actor.agent_id.as_str());
        let distinct_session = |r: &Review| r.envelope.actor.session_id.as_ref().is_none_or(|s| !evidence_sessions.contains(s.as_str()));
        if fresh.iter().any(|&r| distinct_actors(r) && distinct_session(r)) {
            return None;
        }
        let actors = evidence_actors.iter().map(|a| format!("`{a}`")).collect::<Vec<_>>().join(", ");
        // A review by another actor that still failed did so on the session
        // rule alone; name it, or the operator is told the actors collide
        // when they do not.
        let shared_sessions: BTreeSet<&str> =
            fresh.iter().filter(|&&r| distinct_actors(r)).filter_map(|r| r.envelope.actor.session_id.as_ref()).map(|s| s.as_str()).collect();
        let detail = if shared_sessions.is_empty() {
            format!(
                "every review of this scenario is by an actor that also attested its evidence ({actors}); spec_coverage.require_review.distinct_actor wants a reviewer other than the evidence actor"
            )
        } else {
            let sessions = shared_sessions.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", ");
            format!(
                "every review of this scenario is by an actor that also attested its evidence ({actors}) or from a session that did ({sessions}); spec_coverage.require_review.distinct_actor wants a reviewer other than the evidence actor, in a session other than any evidence session"
            )
        };
        return gap(ReviewGapKind::NotDistinct, detail);
    }

    if !stale.is_empty() {
        let who = |keep: &dyn Fn(&Drift) -> bool| stale.iter().filter(|(_, d)| keep(d)).map(|(r, _)| format!("`{}`", r.reviewer)).collect::<Vec<_>>().join(", ");
        let mut parts = Vec::new();
        let unmigrated = who(&|d| d.unmigrated);
        if !unmigrated.is_empty() {
            parts.push(format!(
                "review by {unmigrated} predates 0.23 review pinning (it pins evidence records, or nothing): `canon migrate` rewrites it to what the evidence says, and a review that is not migrated does not count"
            ));
        }
        let moved = stale.iter().filter(|(_, d)| !d.unmigrated).map(|(_, d)| d).collect::<Vec<_>>();
        if !moved.is_empty() {
            let scenario_spec = moved.iter().any(|d| d.spec == Some(PinnedSpec::Scenario));
            let file_spec = moved.iter().any(|d| d.spec == Some(PinnedSpec::File));
            let evidence = moved.iter().any(|d| d.evidence);
            let spec_what = match (scenario_spec, file_spec) {
                (true, false) => Some("the scenario's digest"),
                (false, true) => Some("the .feature file's source digest"),
                (true, true) => Some("the scenario's digest, or the .feature file's source digest a file-pinned review saw,"),
                (false, false) => None,
            };
            let changed = match (spec_what, evidence) {
                (Some(what), true) => format!("its spec and its evidence changed ({what} is not the pinned one, and the latest evidence says a different verdict or names different checks)"),
                (Some(what), false) => format!("its spec changed ({what} is not the pinned one)"),
                (None, _) => "its evidence changed (the latest evidence says a different verdict or names different checks than the review saw)".to_string(),
            };
            parts.push(format!("stale review by {}: {changed} since the review", who(&|d| !d.unmigrated)));
        }
        return gap(ReviewGapKind::Stale, format!("{}; a review counts only while what it saw is current — review it again (`canon review scenario`, or `canon review change` for a chore)", parts.join("; ")));
    }

    let outcomes = unsigned.iter().map(|r| format!("`{}` {}", r.reviewer, r.outcome.as_str())).collect::<Vec<_>>().join(", ");
    gap(
        ReviewGapKind::Unsigned,
        format!(
            "no signed review: the latest review recorded no sign-off ({outcomes}); an indeterminate or unavailable review is a gap, never an approval — run the review again and record `--review-outcome signed`"
        ),
    )
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
            "open blocker finding by `{}` on change `{}` (subject {subjects}): {}; fix it and `canon finding close <change> --round <n> --seq <n> --disposition fixed`, or close it as rejected or deferred",
            finding.reviewer,
            finding.change_id.as_str(),
            finding.summary
        ),
    )
}

/// A blocker whose latest disposition is `fixed` (module doc).
#[derive(Debug, Clone, Copy)]
pub struct FixedBlocker<'a> {
    /// Its latest version: the `fixed` one.
    pub finding: &'a Finding,
    /// When it was closed `fixed`: the `at` of its latest record. A fix
    /// is re-verified by evidence recorded after this.
    pub closed: DateTime<Utc>,
}

impl FixedBlocker<'_> {
    /// Whether the finding names the scenarios it is about
    /// ([`Finding::scenario_ids`]), so only those need fresh evidence.
    pub fn is_scoped(&self) -> bool {
        !self.finding.scenario_ids.is_empty()
    }

    /// The scenarios of `subject` that still wait on evidence recorded
    /// after the close: the evidenced ones the finding names, or, when it
    /// names none, every evidenced one ([`scenarios_without_evidence_after`]).
    /// The one answer the corpus rule, the subject guard and `canon status`
    /// all read, so they cannot disagree.
    pub fn unmet<'c>(&self, ctx: &'c GateContext, subject: &SubjectId) -> Vec<&'c Scenario> {
        let mut unmet = scenarios_without_evidence_after(ctx, subject, self.closed);
        if self.is_scoped() {
            unmet.retain(|s| self.finding.scenario_ids.contains(&s.scenario_id));
        }
        unmet
    }
}

/// Every blocker finding whose latest disposition is `fixed`, each with
/// the time it was closed (the `at` of that latest record).
pub fn fixed_blockers(ctx: &GateContext) -> Vec<FixedBlocker<'_>> {
    latest_by_key(&ctx.findings, |f| (f.change_id.as_str().to_string(), f.round, f.seq))
        .into_iter()
        .filter(|f| f.severity == FindingSeverity::Blocker && f.disposition() == FindingDisposition::Fixed)
        .map(|finding| FixedBlocker { finding, closed: finding.envelope.at })
        .collect()
}

/// The report files `scenario` of spec root `project` bound to its latest
/// evidence, as `<format>:<path>` words (`junit:reports/a.xml`), sorted, each
/// once. Empty when that evidence bound no report.
fn latest_report_words(ctx: &GateContext, project: &str, scenario: &str) -> BTreeSet<String> {
    let evidence: Vec<&canon_model::EvidenceRecord> = ctx
        .evidence
        .iter()
        .filter(|e| e.scenario_id.as_ref().is_some_and(|id| id.as_str() == scenario) && e.project_id.as_ref().is_none_or(|p| p.as_str() == project))
        .collect();
    let Some(latest) = crate::freshness::latest_evidence(&evidence) else { return BTreeSet::new() };
    latest
        .attachments
        .iter()
        .filter_map(|a| {
            let format = serde_json::to_value(a.format?).ok()?;
            Some(format!("{}:{}", format.as_str()?, a.path))
        })
        .collect()
}

/// The ONE batch command that records the suite again (0.28 D11): `canon
/// evidence add --all-scenarios` with a `--report` for every report file bound
/// to the latest evidence of `scenarios` (`(project_id, scenario_id)` pairs),
/// as the roles a re-verification runs under. Never a per-scenario loop.
/// `--project-id` is added only when the repo has several spec roots
/// (`multi_root`), where it is required, and then once per root that has
/// evidenced scenarios: one command each, in root order. A root whose
/// evidence bound no report has no command (the caller says to pass its
/// `--report`s).
pub fn batch_commands(ctx: &GateContext, scenarios: &[(&str, &str)], multi_root: bool) -> Vec<String> {
    let mut roots: Vec<(&str, BTreeSet<String>)> = Vec::new();
    for (project, scenario) in scenarios {
        let words = latest_report_words(ctx, project, scenario);
        match roots.iter_mut().find(|(p, _)| p == project) {
            Some((_, all)) => all.extend(words),
            None => roots.push((project, words)),
        }
    }
    roots
        .into_iter()
        .filter(|(_, reports)| !reports.is_empty())
        .map(|(project, reports)| {
            let mut command = String::from("canon evidence add --all-scenarios");
            if multi_root {
                command.push_str(&format!(" --project-id {project}"));
            }
            for word in reports {
                command.push_str(&format!(" --report {}", crate::audit::shell_quote(&word)));
            }
            command.push_str(" --role implementer");
            command
        })
        .collect()
}

/// What a re-verification refusal says, ending in the command that
/// clears it ([`batch_commands`]). `scenarios` are `(project_id, scenario_id)` pairs.
pub fn reverify_remedy(ctx: &GateContext, scenarios: &[(&str, &str)], multi_root: bool) -> String {
    let commands = batch_commands(ctx, scenarios, multi_root);
    let how = match commands.as_slice() {
        [] => "record the evidence again with `canon evidence add --all-scenarios` and one --report for each test report of the suite (junit:, cucumber: or swift-testing: followed by its path; add --project-id when the repo has several spec roots)".to_string(),
        [one] => format!("record the evidence again with `{one}`"),
        many => format!("record the evidence again, once per spec root: {}", many.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join("; ")),
    };
    format!("{how}; a scenario review stays current when the verdict and the checks are the same, and a changed verdict or check stales it")
}

/// The re-verification remedy of a finding that names its scenarios: the
/// per-scenario `canon evidence add` (it takes one `--scenario-id`, and
/// no filter on a batch), once for each scenario, grouped by spec root so
/// each command carries its own `--project-id`. Not the whole-suite batch.
pub fn scoped_reverify_remedy(scenarios: &[(&str, &str)]) -> String {
    let mut roots: Vec<(&str, Vec<&str>)> = Vec::new();
    for (project, id) in scenarios {
        match roots.iter_mut().find(|(p, _)| p == project) {
            Some((_, ids)) => ids.push(id),
            None => roots.push((project, vec![id])),
        }
    }
    if roots.is_empty() {
        roots.push(("<root>", vec!["<scenario-id>"]));
    }
    let commands = roots
        .iter()
        .map(|(project, ids)| {
            let ids = ids.iter().map(|id| format!("`{id}`")).collect::<Vec<_>>().join(", ");
            format!("`canon evidence add --project-id {project} --scenario-id <id> --report <junit|cucumber|swift-testing>:<path> --role implementer` for each of {ids}")
        })
        .collect::<Vec<_>>()
        .join("; and ");
    format!(
        "record the evidence again for {SCOPED_MARK} with {commands}; the rest of the suite needs nothing; a scenario review stays current when the verdict and the checks are the same, and a changed verdict or check stales it"
    )
}

/// `<first three>, … N more`: the scenarios a re-verification names.
pub fn reverify_scenarios(scenarios: &[(&str, &str)]) -> String {
    let shown = scenarios.iter().take(3).map(|(_, id)| format!("`{id}`")).collect::<Vec<_>>().join(", ");
    match scenarios.len().saturating_sub(3) {
        0 => shown,
        more => format!("{shown}, … {more} more"),
    }
}

/// How many spec roots the indexed scenarios come from: the gate does not read
/// `canon.yaml`, so this stands in for "the repo has several spec roots".
fn distinct_projects(ctx: &GateContext) -> usize {
    ctx.scenarios.iter().map(|s| s.project_id.as_str()).collect::<BTreeSet<_>>().len()
}

/// Whether `violation` is a re-verification refusal ([`reverify_remedy`]),
/// so a caller can add the batch command as the refusal's hint.
pub fn is_reverify(violation: &Violation) -> bool {
    violation.detail.contains(REVERIFY_MARK)
}

/// Whether `violation` is a re-verification refusal that names only the
/// scenarios its finding is about ([`scoped_reverify_remedy`]).
pub fn is_scoped_reverify(violation: &Violation) -> bool {
    violation.detail.contains(SCOPED_MARK)
}

const SCOPED_MARK: &str = "the scenarios the finding names";

const REVERIFY_MARK: &str = "no evidence recorded after";

fn fixed_blocker_violation(ctx: &GateContext, fixed: &FixedBlocker<'_>, subjects: &[&str], unmet: &[(&str, &str)]) -> Violation {
    let finding = fixed.finding;
    let subjects = subjects.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", ");
    Violation::new(
        FailureClass::OpenBlocker,
        format!("{}#{}.{}", finding.change_id.as_str(), finding.round, finding.seq),
        format!(
            "blocker finding by `{}` on change `{}` (subject {subjects}) is closed `fixed` but not re-verified: {} evidenced scenario(s) ({}) have {REVERIFY_MARK} the finding was closed `fixed` ({}); {}",
            finding.reviewer,
            finding.change_id.as_str(),
            unmet.len(),
            reverify_scenarios(unmet),
            fixed.closed.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            if fixed.is_scoped() { scoped_reverify_remedy(unmet) } else { reverify_remedy(ctx, unmet, distinct_projects(ctx) > 1) }
        ),
    )
}

/// Why a chore change has no qualifying change-level review (0.23 D5), or
/// `None` when it has one. The reviews of `change` are folded to the latest
/// per reviewer; one must be `signed`, pinned to the brief as it reads now
/// (`brief`, [`PinnedSpec::Brief`]: an edit to the brief after the review
/// stales it), and, with `distinct_actor`, by a reviewer and an actor that
/// attested none of the evidence of `subject`'s scenarios, nor from one of
/// its sessions. [`crate::freshness`] has no scenario to compare here, so
/// the pin check is this one comparison.
pub fn change_review_gap(ctx: &GateContext, subject: &Subject, change: &ChangeId, brief: &SpecDigest, distinct_actor: bool) -> Option<String> {
    let reviews: Vec<&Review> = ctx.reviews.iter().filter(|r| r.change_id.as_ref() == Some(change)).collect();
    let reviews: Vec<&Review> = latest_by_key(&reviews, |r| r.reviewer.clone()).into_iter().copied().collect();
    if reviews.is_empty() {
        return Some(format!("no review of change `{}` (a chore needs one change-level review)", change.as_str()));
    }
    let (signed, unsigned): (Vec<&Review>, Vec<&Review>) = reviews.into_iter().partition(|r| r.outcome.is_signed());
    let (current, stale): (Vec<&Review>, Vec<&Review>) = signed
        .into_iter()
        .partition(|r| r.pins.as_ref().is_some_and(|p| p.scenario_digest_scope == PinnedSpec::Brief && p.scenario_digest == *brief));
    if current.is_empty() {
        if !stale.is_empty() {
            let who = stale.iter().map(|r| format!("`{}`", r.reviewer)).collect::<Vec<_>>().join(", ");
            return Some(format!("stale review of change `{}` by {who}: its brief was edited since the review; review the chore again", change.as_str()));
        }
        let outcomes = unsigned.iter().map(|r| format!("`{}` {}", r.reviewer, r.outcome.as_str())).collect::<Vec<_>>().join(", ");
        return Some(format!("no signed review of change `{}`: the latest review recorded no sign-off ({outcomes})", change.as_str()));
    }
    if !distinct_actor {
        return None;
    }
    let evidence: Vec<&canon_model::EvidenceRecord> = crate::spec_coverage::subject_scenarios(ctx, &subject.subject_id).into_iter().flat_map(|s| scenario_evidence(ctx, s)).collect();
    let actors: BTreeSet<&str> = evidence.iter().map(|e| e.envelope.actor.agent_id.as_str()).collect();
    let sessions: BTreeSet<&str> = evidence.iter().filter_map(|e| e.envelope.actor.session_id.as_ref()).map(|s| s.as_str()).collect();
    let independent = |r: &Review| {
        !actors.contains(r.reviewer.as_str()) && !actors.contains(r.envelope.actor.agent_id.as_str()) && r.envelope.actor.session_id.as_ref().is_none_or(|s| !sessions.contains(s.as_str()))
    };
    if current.iter().any(|r| independent(r)) {
        return None;
    }
    Some(format!(
        "every review of change `{}` is by an actor, or from a session, that also attested the subject's evidence; spec_coverage.require_review.distinct_actor wants an independent reviewer",
        change.as_str()
    ))
}
