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
//! - still match what it pinned (0.15 D1), when it carries `pins`: the
//!   scenario's current digest of the kind it pinned — its own
//!   `scenario_digest` when the record had one at review time, else the
//!   file-wide `source_digest` — and the current latest evidence record
//!   per role for the scenario ([`current_pins`]). Each pin is compared
//!   like for like, so a file-pinned review never goes stale just because
//!   the scenario record gained a `scenario_digest`. Otherwise it is a
//!   stale review, and the detail names whether the spec, the evidence,
//!   or both changed.
//! - still describe its scenario (0.16 D6), when it carries no `pins`
//!   (written before 0.15): the scenario's digest at the commit the review
//!   pinned (`pin`, read by [`crate::legacy_pins`]) must equal its current
//!   `scenario_digest`, and no declared evidence record of the scenario
//!   may be newer than the review ([`legacy_currency`]). Otherwise it is
//!   stale, with the same kind of detail. When the pinned commit or the
//!   scenario at it is not in this clone, the review is undecidable and
//!   keeps counting as in 0.15. [`legacy_review_advisory`] feeds the one
//!   advisory line `canon gate check`, `canon subject status` and `canon
//!   status` print with the current, stale and undecidable counts.
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

use canon_model::{
    ChangeId, EvidencePin, EvidenceRecord, Finding, FindingDisposition, FindingSeverity, PinnedSpec, RecordKind, Review, ReviewPins, Scenario, StatusOverride, Subject, SubjectId,
    SubjectStatus,
};

use crate::context::GateContext;
use crate::legacy_pins::PinnedScenario;
use crate::failure_class::{FailureClass, Violation};
use crate::policy::{subject_status_name, RequireReview, SpecCoverage};
use crate::spec_coverage::{latest_by_key, latest_scenarios, latest_subjects, scope_decision, subject_scenarios, ScopeDecision};

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
        .filter_map(|s| review_gap(ctx, s, require_review.distinct_actor).map(|gap| gap.violation))
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

/// What changed under a review since it was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Staleness {
    /// A pinned review: the spec pin no longer matches (naming which kind
    /// of pin it was), or the latest evidence records are not the pinned
    /// ones.
    Pinned { spec: Option<PinnedSpec>, evidence: bool },
    /// A review without pins, checked against its commit (0.16 D6).
    Legacy { spec: bool, evidence: bool },
}

/// Every evidence record for the scenario's id whose `project_id`
/// matches or is absent. Counting a project-less record is the stricter
/// reading: it can only make a review not count, never let one count.
fn scenario_evidence<'a>(ctx: &'a GateContext, scenario: &Scenario) -> Vec<&'a EvidenceRecord> {
    ctx.evidence
        .iter()
        .filter(|e| e.scenario_id.as_ref() == Some(&scenario.scenario_id) && e.project_id.as_ref().is_none_or(|p| *p == scenario.project_id))
        .collect()
}

/// What a review of `scenario` pins right now (0.15 D1): its
/// `scenario_digest` (scope `scenario`) when the record carries one, else
/// its file-wide `source_digest` (scope `file`), and the latest evidence
/// record per role for it (the fold `latest_verdicts` applies), each
/// named by its ledger file: the natural key and digest of
/// `kind=evidence_record/<key>__<digest>.json`, in `(key, digest)` order.
/// A record not read from a file (built in memory) is named by its own
/// content digest. `canon review add` writes this; [`review_gap`]
/// compares a pinned review against the scenario and these evidence pins.
pub fn current_pins(ctx: &GateContext, scenario: &Scenario) -> ReviewPins {
    let evidence = scenario_evidence(ctx, scenario);
    let mut pins: Vec<EvidencePin> = latest_by_key(&evidence, |e| e.envelope.actor.role.as_ref().map(|r| r.as_str().to_string()))
        .into_iter()
        .map(|record| {
            let body = serde_json::to_value(record).unwrap_or_default();
            let key = canon_store::partition::resolve_partition(RecordKind::EvidenceRecord, &body)
                .expect("an evidence record's natural key always resolves (`unscoped` at worst)")
                .natural_key;
            let typed = canon_store::partition::content_digest12(&body);
            let digest = ctx.evidence_file_digests.get(&typed).cloned().unwrap_or(typed);
            EvidencePin { key, digest }
        })
        .collect();
    pins.sort();
    let (scenario_digest, scenario_digest_scope) = match &scenario.scenario_digest {
        Some(digest) => (digest.clone(), PinnedSpec::Scenario),
        None => (scenario.source_digest.clone(), PinnedSpec::File),
    };
    ReviewPins { scenario_digest, scenario_digest_scope, evidence: pins }
}

/// `None` when `review` still describes `scenario`: a pinned review that
/// matches it and the `current` evidence pins, or a legacy review that is
/// current or undecidable ([`legacy_currency`]); otherwise what changed.
/// A spec pin is compared like for like: a file pin against the
/// scenario's `source_digest`, a scenario pin against its
/// `scenario_digest` (a record without one no longer matches it).
fn staleness(ctx: &GateContext, review: &Review, scenario: &Scenario, current: &ReviewPins) -> Option<Staleness> {
    let Some(pins) = review.pins.as_ref() else {
        return match legacy_currency(ctx, review, scenario) {
            LegacyCurrency::Stale { spec, evidence } => Some(Staleness::Legacy { spec, evidence }),
            LegacyCurrency::Current | LegacyCurrency::Undecidable => None,
        };
    };
    let spec_current = match pins.scenario_digest_scope {
        PinnedSpec::File => pins.scenario_digest == scenario.source_digest,
        PinnedSpec::Scenario => scenario.scenario_digest.as_ref() == Some(&pins.scenario_digest),
    };
    let pinned: BTreeSet<&EvidencePin> = pins.evidence.iter().collect();
    let now: BTreeSet<&EvidencePin> = current.evidence.iter().collect();
    let spec = (!spec_current).then_some(pins.scenario_digest_scope);
    let evidence = pinned != now;
    (spec.is_some() || evidence).then_some(Staleness::Pinned { spec, evidence })
}

/// Whether a review without `pins` still describes `scenario` (0.16 D6,
/// module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyCurrency {
    /// The scenario's digest at the review's pinned commit is its current
    /// one, and no declared evidence of it is newer than the review.
    Current,
    /// The scenario changed since the pinned commit (`spec`), or declared
    /// evidence of it is newer than the review (`evidence`), or both.
    Stale { spec: bool, evidence: bool },
    /// The pinned commit, or the scenario at it, is not in this clone
    /// ([`crate::legacy_pins`]), or the scenario record has no digest to
    /// compare with. The review keeps counting, as in 0.15.
    Undecidable,
}

/// [`LegacyCurrency`] of `review`, a review without `pins`, against
/// `scenario`, from what [`GateContext::legacy_pins`] read at its pin.
pub fn legacy_currency(ctx: &GateContext, review: &Review, scenario: &Scenario) -> LegacyCurrency {
    let (Some(PinnedScenario::Digest(at_pin)), Some(now)) = (ctx.legacy_pins.get(review), scenario.scenario_digest.as_ref()) else {
        return LegacyCurrency::Undecidable;
    };
    let spec = at_pin != now;
    let evidence = scenario_evidence(ctx, scenario).iter().any(|e| e.envelope.at > review.envelope.at);
    if spec || evidence {
        LegacyCurrency::Stale { spec, evidence }
    } else {
        LegacyCurrency::Current
    }
}

/// The latest review per `reviewer` of `(project_id, scenario_id)`
/// (module doc), in reviewer order.
fn latest_reviews<'a>(ctx: &'a GateContext, scenario: &Scenario) -> Vec<&'a Review> {
    let reviews: Vec<&Review> = ctx.reviews.iter().filter(|r| r.project_id == scenario.project_id && r.scenario_id == scenario.scenario_id).collect();
    latest_by_key(&reviews, |r| r.reviewer.clone()).into_iter().copied().collect()
}

/// The in-effect reviews: the latest per `(project_id, scenario_id,
/// reviewer)`, the fold every review rule reads (module doc).
pub fn in_effect_reviews(ctx: &GateContext) -> Vec<&Review> {
    latest_by_key(&ctx.reviews, |r| (r.project_id.clone(), r.scenario_id.clone(), r.reviewer.clone()))
}

/// The in-effect reviews that are still current (0.18 D5): every
/// [`in_effect_reviews`] entry except one that no longer describes its
/// live scenario — a pinned review whose spec or evidence pin moved, or a
/// legacy review whose currency check says stale. A stale review is
/// superseded: the gate reads the scenario's newer review in its place,
/// or reports the gap itself (`unreviewed-promotion`), so nothing the
/// stale one bound is still claimed. A legacy review that is undecidable
/// keeps counting, and so is kept, as is a review whose scenario has no
/// live record to compare it with: dropping either would only ever skip
/// a check. Review artifacts are re-checked over exactly these
/// ([`crate::artifact_store`]).
pub fn current_reviews(ctx: &GateContext) -> Vec<&Review> {
    let live: BTreeMap<(&canon_model::ProjectId, &canon_model::ScenarioId), &Scenario> =
        latest_scenarios(ctx).into_iter().map(|s| ((&s.project_id, &s.scenario_id), s)).collect();
    let mut pins: BTreeMap<(&canon_model::ProjectId, &canon_model::ScenarioId), ReviewPins> = BTreeMap::new();
    let mut current = Vec::new();
    for review in in_effect_reviews(ctx) {
        let key = (&review.project_id, &review.scenario_id);
        let Some(scenario) = live.get(&key).copied() else {
            current.push(review);
            continue;
        };
        let now = pins.entry(key).or_insert_with(|| current_pins(ctx, scenario));
        if staleness(ctx, review, scenario, now).is_none() {
            current.push(review);
        }
    }
    current
}

/// How the in-effect reviews without `pins` (written before 0.15) of
/// live scenarios came out of the legacy check: one count per review,
/// and the scenarios a stale one leaves without a qualifying review.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyReviewCounts {
    pub current: usize,
    /// Every stale legacy review, superseded or not.
    pub stale: usize,
    /// The stale ones whose scenario already has another qualifying
    /// review (current, signed, distinct when the policy asks): nothing
    /// is owed for them.
    pub superseded: usize,
    pub undecidable: usize,
    /// The scenario ids with a stale legacy review and no qualifying
    /// review in its place, sorted: the ones to review again.
    pub needs_review: BTreeSet<String>,
}

impl LegacyReviewCounts {
    /// Every in-effect legacy review of a live scenario `owned` accepts.
    /// A review of a scenario no feature file declares any more is not
    /// counted: it reviews nothing the gate reads. Whether a stale one is
    /// superseded is [`review_gap`]'s answer for its scenario, under the
    /// active `distinct_actor` (on when no rule is active, the stricter
    /// reading).
    pub fn of(ctx: &GateContext, owned: impl Fn(&Scenario) -> bool) -> Self {
        let live: BTreeMap<(&canon_model::ProjectId, &canon_model::ScenarioId), &Scenario> =
            latest_scenarios(ctx).into_iter().map(|s| ((&s.project_id, &s.scenario_id), s)).collect();
        let distinct_actor = active_require_review(ctx).is_none_or(|(rr, _)| rr.distinct_actor);
        let mut counts = Self::default();
        for review in in_effect_reviews(ctx).into_iter().filter(|r| r.pins.is_none()) {
            let Some(scenario) = live.get(&(&review.project_id, &review.scenario_id)).copied().filter(|s| owned(s)) else { continue };
            match legacy_currency(ctx, review, scenario) {
                LegacyCurrency::Current => counts.current += 1,
                LegacyCurrency::Stale { .. } => {
                    counts.stale += 1;
                    if review_gap(ctx, scenario, distinct_actor).is_none() {
                        counts.superseded += 1;
                    } else {
                        counts.needs_review.insert(scenario.scenario_id.as_str().to_string());
                    }
                }
                LegacyCurrency::Undecidable => counts.undecidable += 1,
            }
        }
        counts
    }

    pub fn total(&self) -> usize {
        self.current + self.stale + self.undecidable
    }

    /// The one advisory line about these reviews, without a prefix. Only
    /// the scenarios still owed a review are named for re-review.
    pub fn note(&self) -> String {
        let mut line = format!(
            "{} review(s) predate review pinning (0.15) and were checked against the commit each one pinned: {} current, {} stale, {} undecidable",
            self.total(),
            self.current,
            self.stale,
            self.undecidable
        );
        if self.superseded > 0 {
            line.push_str(&format!("; {} stale one(s) superseded by a current review of the same scenario, so nothing is owed for them", self.superseded));
        }
        if !self.needs_review.is_empty() {
            let ids = self.needs_review.iter().map(String::as_str).collect::<Vec<_>>().join(", ");
            line.push_str(&format!("; a stale one with no current review in its place no longer counts — review its scenario again (`canon review add`): {ids}"));
        }
        if self.undecidable > 0 {
            line.push_str(
                "; an undecidable one still counts unchecked, because its pinned commit or the scenario at it is not in this clone — check out full history (`fetch-depth: 0` in CI) to check it",
            );
        }
        line
    }
}

/// `canon gate check`'s legacy-review advisory: `Some(counts)` when
/// `require_review` is active and readable and at least one in-effect
/// review of a live scenario carries no pins. Advisory only; it never
/// changes the exit code (a stale legacy review is reported by
/// [`review_gap`] like any other stale review).
pub fn legacy_review_advisory(ctx: &GateContext) -> Option<LegacyReviewCounts> {
    let (require_review, _) = active_require_review(ctx)?;
    if !unreadable_review_kinds(ctx, require_review).is_empty() {
        return None;
    }
    let counts = LegacyReviewCounts::of(ctx, |_| true);
    (counts.total() > 0).then_some(counts)
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
            "no review record for this scenario; spec_coverage.require_review wants an independent review (`canon review add`) once its subject is claimed done".to_string(),
        );
    }
    let current = current_pins(ctx, scenario);
    let (signed, unsigned): (Vec<&Review>, Vec<&Review>) = reviews.into_iter().partition(|r| r.outcome.is_signed());
    let mut stale: Vec<(&Review, Staleness)> = Vec::new();
    let mut fresh: Vec<&Review> = Vec::new();
    for review in signed {
        match staleness(ctx, review, scenario, &current) {
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
        let pinned: Vec<(&Review, Option<PinnedSpec>, bool)> =
            stale.iter().filter_map(|(r, c)| match c { Staleness::Pinned { spec, evidence } => Some((*r, *spec, *evidence)), Staleness::Legacy { .. } => None }).collect();
        let mut parts = Vec::new();
        if !pinned.is_empty() {
            let who = pinned.iter().map(|(r, _, _)| format!("`{}`", r.reviewer)).collect::<Vec<_>>().join(", ");
            let scenario_spec = pinned.iter().any(|(_, spec, _)| *spec == Some(PinnedSpec::Scenario));
            let file_spec = pinned.iter().any(|(_, spec, _)| *spec == Some(PinnedSpec::File));
            let evidence = pinned.iter().any(|(_, _, evidence)| *evidence);
            let spec_what = match (scenario_spec, file_spec) {
                (true, false) => Some("the scenario's digest"),
                (false, true) => Some("the .feature file's source digest"),
                (true, true) => Some("the scenario's digest, or the .feature file's source digest a file-pinned review saw,"),
                (false, false) => None,
            };
            let changed = match (spec_what, evidence) {
                (Some(what), true) => format!("its spec and its evidence changed ({what} and the latest evidence records are not the pinned ones)"),
                (Some(what), false) => format!("its spec changed ({what} is not the pinned one)"),
                (None, _) => "its evidence changed (the latest evidence records are not the pinned ones)".to_string(),
            };
            parts.push(format!("stale review by {who}: {changed} since the review"));
        }
        for (review, change) in &stale {
            let Staleness::Legacy { spec, evidence } = change else { continue };
            let pin = &review.pin;
            let spec_changed = format!("its spec changed since {pin} (the scenario's digest at that commit is not its current one)");
            let changed = match (spec, evidence) {
                (true, true) => format!("{spec_changed}, and it has declared evidence newer than the review"),
                (true, false) => spec_changed,
                (false, _) => "it has declared evidence newer than the review".to_string(),
            };
            parts.push(format!("stale review by `{}` (written before review pinning; checked against commit {pin}): {changed}", review.reviewer));
        }
        let counts = if pinned.len() == stale.len() { "a pinned review counts" } else { "a review counts" };
        return gap(ReviewGapKind::Stale, format!("{}; {counts} only while what it saw is current — review it again (`canon review add`)", parts.join("; ")));
    }

    let outcomes = unsigned.iter().map(|r| format!("`{}` {}", r.reviewer, r.outcome.as_str())).collect::<Vec<_>>().join(", ");
    gap(
        ReviewGapKind::Unsigned,
        format!(
            "no signed review: the latest review recorded no sign-off ({outcomes}); an indeterminate or unavailable review is a gap, never an approval — run the review again and record `--outcome signed`"
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
            "open blocker finding by `{}` on change `{}` (subject {subjects}): {}; fix it and `canon finding close --disposition fixed --resolution-sha <sha>`, or close it as rejected or deferred",
            finding.reviewer,
            finding.change_id.as_str(),
            finding.summary
        ),
    )
}
