//! The spec-corpus coverage check (s44 `spec-derived-worklist`): the
//! one gate pass that starts from what a repo SPECIFIED rather than
//! from what it already attested to.
//!
//! # Why this is a separate check, not an extension of `crate::coverage`
//! Three reasons, in the order that decides it:
//!
//! 1. **Direction.** [`crate::coverage::CellSubject::of`] maps a
//!    RECORD to a subject. A left join needs the inverse — CORPUS to
//!    subject — and no amount of extending a record-keyed enum
//!    produces a subject for an artifact that has no record. The
//!    enumeration has to come from a different collection, and once it
//!    does, the two passes share no loop.
//! 2. **Contract.** `crate::coverage` is unconditional: it runs for
//!    every repo, always, and a `risk_routing` diff alone tightens it.
//!    This check is opt-in and silent by default. Folding an
//!    opt-in-silent behavior into an always-on check makes that check's
//!    contract conditional on a policy section it never had to consult,
//!    which is a worse trade than one more entry in
//!    [`crate::dispatch::check_set`].
//! 3. **Requiredness is unanswerable here.** `risk_routing` is a CEL
//!    predicate bound to `RecordKind::EvidenceRecord`
//!    (`crate::coverage`'s own note on `PolicyResolution`'s binding
//!    contract) and, by that module's stated rule, is "evaluate[d] ...
//!    against the artifact's OWN already-submitted evidence (never a
//!    synthetic/absent record — a rule can only be proven to apply
//!    using a record that actually exists)". A zero-evidence scenario
//!    offers nothing to evaluate against, so `risk_routing` cannot say
//!    whether a cell is required for it. This check therefore asks a
//!    PRESENCE question, which needs no predicate at all.
//!
//! # What it answers
//! Three facts about every `Scenario` in the corpus, each of which a
//! spec-driven repo would call "work that is not done":
//!
//! - **Unimplemented** — no `EvidenceRecord` attests to it at all.
//! - **Mismatched** — its folded divergence state is `Open`,
//!   `StillDivergent`, or `ResolvedInvalid`, or its latest ledger
//!   verdict is `Divergent`.
//! - **Golden-path only** — with `require_cases`, a feature surface
//!   (`<area>.<surface>`) none of whose in-scope scenarios carries a
//!   required `@case:` (typically `failure`). Evidence presence cannot
//!   see this gap: three attested happy-path scenarios read as fully
//!   covered while nothing specifies what happens when the input is
//!   wrong, missing, or refused.
//!
//! All three surface as [`FailureClass::UncoveredCell`] with a
//! distinguishing detail. The closed class set is NOT extended for
//! them: `canon-cli`'s `verifying → shipped` subject gate already
//! set that precedent for exactly this situation, reporting both "no
//! verdict" and "divergent verdict" under one class with different
//! details, because the class vocabulary describes gate OUTCOMES, not
//! causes.
//!
//! With `require_review` (issue #2), this check also runs
//! [`crate::review_gate`]'s rule: an in-scope scenario without a
//! qualifying review is `unreviewed-promotion`, and an open blocker
//! finding on an in-scope subject's change is `open-blocker`. See that
//! module for the rule and for how a recorded waiver turns a gap into an
//! advisory.
//!
//! # Joins on the composite key, never the bare scenario id
//! `Scenario`'s identity is `(project_id, scenario_id)` — `project_id`
//! is a required field, and a repo may configure several
//! `specs.roots[]`, so two roots can carry the same scenario id. An
//! `EvidenceRecord` whose `project_id` is absent therefore cannot
//! satisfy this join; `canon evidence add` requires `--project-id`
//! alongside `--scenario-id` for that reason.

use std::collections::{BTreeMap, BTreeSet};

use canon_model::fold::{fold_to_current_state, FoldedState};
use canon_model::{EvidenceVerdict, ProjectId, Scenario, ScenarioId, SubjectId, SubjectStatus};

use crate::context::{GateCheck, GateContext, SPEC_CORPUS_KINDS};
use crate::review_gate;
use crate::failure_class::{FailureClass, Violation};
use crate::ledger::latest_verdicts;
use crate::policy::SpecCoverage;

/// The spec-corpus coverage check. Silent unless `policy.yaml` declares
/// a `spec_coverage:` section (module doc).
pub struct SpecCoverageCheck;

impl GateCheck for SpecCoverageCheck {
    fn name(&self) -> &'static str {
        "spec-coverage"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        let policy = match &ctx.policy.spec_coverage {
            // Absent section: the opt-in default. Nothing to derive.
            None => return Vec::new(),
            // Present but broken. `PolicyResolution::resolve` is frozen
            // infallible so it could not refuse at load; refusing HERE
            // is what stops a typo from being silently identical to
            // never opting in.
            Some(SpecCoverage::Invalid { detail }) => {
                return vec![Violation::new(
                    FailureClass::UncoveredCell,
                    "spec_coverage",
                    format!("policy.yaml's `spec_coverage` section is unusable ({detail}); refusing rather than treating it as absent"),
                )];
            }
            Some(SpecCoverage::Active { require_evidence: false, require_cases, require_review: None, .. }) if require_cases.is_empty() => return Vec::new(),
            Some(active @ SpecCoverage::Active { .. }) => active,
        };

        // A kind routed off the rung this context reads yields an empty
        // vector for a reason that is NOT "the corpus is empty". Passing
        // on that input would be a gate that reports clean because it
        // saw nothing — the failure mode that hid `Task` from
        // `canon report`. Refuse before deriving anything.
        let unreadable: Vec<&str> = ctx.unreadable_kinds.iter().filter(|k| SPEC_CORPUS_KINDS.contains(k)).map(|k| k.as_str()).collect();
        if !unreadable.is_empty() {
            let kinds = unreadable.join(", ");
            return vec![Violation::new(
                FailureClass::UncoveredCell,
                "spec_coverage",
                format!(
                    "`spec_coverage` is enabled but these kinds route away from the rung the gate reads ({kinds}), so their corpus would read as empty and this check would pass by seeing nothing; route them to `local` or disable the section"
                ),
            )];
        }

        let SpecCoverage::Active { require_evidence, scope, exclude_lanes, require_cases, require_review } = policy else {
            unreachable!("the non-Active arms return above");
        };

        let evidenced = evidenced_scenarios(ctx);
        let divergence_states = fold_to_current_state(&ctx.divergences, &live_bindings(ctx), ctx.now);
        let verdicts = latest_verdicts(ctx);
        let subject_status: BTreeMap<&str, SubjectStatus> =
            latest_by_key(&ctx.subjects, |s| s.subject_id.as_str().to_string()).into_iter().map(|s| (s.subject_id.as_str(), s.status)).collect();

        let mut violations = Vec::new();
        let mut in_scope = Vec::new();
        for scenario in latest_by_key(&ctx.scenarios, |s| (s.project_id.clone(), s.scenario_id.clone())) {
            match scope_decision(scope, scenario.subject_id.as_ref(), &subject_status) {
                ScopeDecision::OutOfScope => continue,
                ScopeDecision::DanglingSubject(subject_id) => {
                    violations.push(Violation::new(
                        FailureClass::UncoveredCell,
                        scenario.scenario_id.as_str(),
                        format!("scenario names subject `{subject_id}`, but no Subject record carries that id — the scope filter cannot place it"),
                    ));
                    continue;
                }
                ScopeDecision::InScope => {}
            }
            if scenario.lane.as_ref().is_some_and(|lane| exclude_lanes.contains(lane)) {
                continue;
            }
            in_scope.push(scenario);
            if !require_evidence {
                continue;
            }

            let key = (scenario.project_id.clone(), scenario.scenario_id.clone());

            if !evidenced.contains(&key) {
                violations.push(Violation::new(
                    FailureClass::UncoveredCell,
                    scenario.scenario_id.as_str(),
                    "spec scenario has no evidence record — specified, never attested to",
                ));
                continue;
            }

            // Mismatch is reported only for a scenario that IS
            // evidenced: an unimplemented spec is already reported
            // above, and emitting both would double-count one scenario.
            if let Some(state) = divergence_states.get(&key) {
                if let Some(detail) = mismatch_detail(state) {
                    violations.push(Violation::new(FailureClass::UncoveredCell, scenario.scenario_id.as_str(), detail));
                    continue;
                }
            }

            if let Some(entry) = verdicts.iter().find(|((subject, _), _)| subject == scenario.scenario_id.as_str()).map(|(_, entry)| entry) {
                if entry.verdict == EvidenceVerdict::Divergent {
                    violations.push(Violation::new(
                        FailureClass::UncoveredCell,
                        scenario.scenario_id.as_str(),
                        format!("latest ledger verdict is divergent (by {})", entry.agent_id),
                    ));
                }
            }
        }
        violations.extend(case_gaps(in_scope, require_cases).iter().map(CaseGap::violation));
        if let Some(require_review) = require_review {
            let unreadable = review_gate::unreadable_review_kinds(ctx, require_review);
            if unreadable.is_empty() {
                violations.extend(review_gate::evaluate(ctx, require_review, exclude_lanes).violations);
            } else {
                violations.push(review_gate::unreadable_violation(&unreadable));
            }
        }
        violations
    }
}

/// One feature surface missing a required `@case:` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseGap {
    pub project_id: ProjectId,
    /// `<area>.<surface>` — the scenario id without its number, i.e. one
    /// `.feature` file's behavior surface.
    pub surface: String,
    pub case: String,
    /// How many scenarios the surface does carry, none of them `case`.
    pub scenarios: usize,
}

impl CaseGap {
    pub fn violation(&self) -> Violation {
        Violation::new(
            FailureClass::UncoveredCell,
            self.surface.clone(),
            format!(
                "feature surface `{}` (root `{}`) has no `@case:{}` scenario among its {} in-scope scenario(s) — its spec covers only other paths; specify what happens on that path",
                self.surface,
                self.project_id.as_str(),
                self.case,
                self.scenarios
            ),
        )
    }
}

/// The feature surfaces among `scenarios` that carry no scenario of a
/// `required` case, grouped by `(project_id, <area>.<surface>)`.
///
/// The ONE case-coverage rule: `canon gate check` applies it to the
/// scenarios `spec_coverage` puts in scope, and the `verifying →
/// shipped` subject gate applies it to the scenarios a subject owns. An
/// untagged scenario counts toward a surface but satisfies no case — an
/// unclassified corpus therefore reports every surface, rather than
/// passing because it saw no tags. Empty `required` yields nothing.
pub fn case_gaps<'a>(scenarios: impl IntoIterator<Item = &'a Scenario>, required: &[String]) -> Vec<CaseGap> {
    if required.is_empty() {
        return Vec::new();
    }
    let mut surfaces: BTreeMap<(ProjectId, String), (usize, BTreeSet<&str>)> = BTreeMap::new();
    for scenario in scenarios {
        let id = scenario.scenario_id.as_str();
        let surface = id.rsplit_once('.').map_or(id, |(surface, _)| surface).to_string();
        let entry = surfaces.entry((scenario.project_id.clone(), surface)).or_default();
        entry.0 += 1;
        if let Some(case) = scenario.case.as_deref() {
            entry.1.insert(case);
        }
    }
    let mut gaps = Vec::new();
    for ((project_id, surface), (count, cases)) in surfaces {
        for case in required {
            if !cases.contains(case.as_str()) {
                gaps.push(CaseGap { project_id: project_id.clone(), surface: surface.clone(), case: case.clone(), scenarios: count });
            }
        }
    }
    gaps
}

/// Collapse an append-only tier read to the LATEST record per natural
/// key, through the one shared supersession rule
/// ([`canon_store::fold::fold_latest_by_key`]).
///
/// `GateContext`'s corpus vectors are raw tier reads: canon's ledger is
/// append-only, so re-running `canon inventory sync` after a `.feature`
/// edit leaves BOTH generations of a `Scenario` on disk. Iterating them
/// unfolded would judge one scenario twice — and worse, judge the STALE
/// generation, so a scenario that just gained an `@subject:` tag would
/// still be measured against its untagged row and fall out of `scope`.
///
/// `Divergence` is deliberately NOT folded this way: its supersession is
/// `(run_seq, round)`, not `(at, schema, digest)`, and
/// [`fold_to_current_state`] owns that rule and needs every row.
pub(crate) fn latest_by_key<'a, T: serde::Serialize, K: Ord>(records: &'a [T], key: impl Fn(&T) -> K) -> Vec<&'a T> {
    struct Row<'a, T> {
        record: &'a T,
        at: chrono::DateTime<chrono::Utc>,
        schema: u32,
        digest: String,
    }
    let rows = records.iter().filter_map(|record| {
        let body = serde_json::to_value(record).ok()?;
        let at = body.get("at")?.as_str()?.parse::<chrono::DateTime<chrono::Utc>>().ok()?;
        let schema = body.get("schema").and_then(serde_json::Value::as_u64).unwrap_or(1) as u32;
        let digest = canon_store::partition::content_digest12(&body);
        Some(Row { record, at, schema, digest })
    });
    canon_store::fold::fold_latest_by_key(rows, |r| key(r.record), |r| r.at, |r| r.schema, |r| r.digest.as_str())
        .into_values()
        .map(|r| r.record)
        .collect()
}

/// The scenarios a Subject owns: the LATEST `Scenario` generation of
/// every `(project_id, scenario_id)` whose latest generation carries
/// `subject_id == subject`, ordered by that key.
///
/// This is the ONE subject ↔ scenario join. It is written by `canon
/// inventory sync` from each scenario's `@subject:<id>` Gherkin tag and
/// read by this check's `scope`, the `verifying → shipped` subject gate,
/// and `canon report`'s Subjects panel (`mart_subjects` applies the same
/// latest-generation fold in SQL). Folding first matters: a scenario
/// whose tag was removed or moved to another subject still has its old
/// tagged generation on disk, and reading that row would keep it linked.
pub fn subject_scenarios<'a>(ctx: &'a GateContext, subject: &SubjectId) -> Vec<&'a Scenario> {
    let mut owned: Vec<&Scenario> = latest_by_key(&ctx.scenarios, |s| (s.project_id.clone(), s.scenario_id.clone()))
        .into_iter()
        .filter(|s| s.subject_id.as_ref() == Some(subject))
        .collect();
    owned.sort_by(|a, b| (&a.project_id, &a.scenario_id).cmp(&(&b.project_id, &b.scenario_id)));
    owned
}

/// Every `(project_id, scenario_id)` an `EvidenceRecord` attests to.
///
/// A record carrying a `scenario_id` but no `project_id` is EXCLUDED,
/// not matched loosely: the composite key is the scenario's identity
/// (module doc), and treating a half-key as a match would let evidence
/// authored against one spec root silently cover a same-named scenario
/// in another.
fn evidenced_scenarios(ctx: &GateContext) -> BTreeSet<(ProjectId, ScenarioId)> {
    ctx.evidence
        .iter()
        .filter_map(|record| Some((record.project_id.clone()?, record.scenario_id.clone()?)))
        .collect()
}

/// The divergence fold's live-binding input, derived through the ONE
/// shared derivation `canon divergence status` uses
/// ([`canon_store::fold::live_bindings_of`]) so the two surfaces cannot
/// disagree about which scenarios are open.
fn live_bindings(ctx: &GateContext) -> BTreeMap<(ProjectId, ScenarioId), canon_model::fold::BindingSnapshot> {
    canon_store::fold::live_bindings_of(ctx.evidence.clone())
}

/// Whether a folded divergence state counts as mismatched work, and how
/// to say so. `Resolved` and un-expired `Deferred` are the only states
/// that do not — an expired `Deferred` already folds to
/// `StillDivergent` upstream, so it needs no arm here.
fn mismatch_detail(state: &FoldedState) -> Option<String> {
    match state {
        FoldedState::Open { .. } => Some("spec has an open divergence — implementation does not match the spec".to_string()),
        FoldedState::StillDivergent { .. } => Some("spec divergence was re-checked and is still divergent".to_string()),
        // The staleest resolution state: the record claims Resolved, but
        // the app sha it resolved against has moved. Included
        // deliberately — a resolution that no longer binds is a mismatch
        // nobody has looked at since.
        FoldedState::ResolvedInvalid { .. } => {
            Some("spec divergence is marked resolved against an app sha that has since moved — the resolution no longer binds".to_string())
        }
        FoldedState::Resolved { .. } | FoldedState::Deferred { .. } => None,
    }
}

pub(crate) enum ScopeDecision<'a> {
    InScope,
    OutOfScope,
    /// The scenario names a subject no `Subject` record carries. Not
    /// silently skipped: a dangling link is a corpus defect that would
    /// otherwise make the scope filter quietly exclude real work.
    DanglingSubject(&'a str),
}

/// Apply `spec_coverage.scope`. An EMPTY scope means corpus-wide, so
/// every scenario is in scope regardless of its subject link — the
/// opposite reading would make an omitted `scope:` silently disable the
/// whole check.
pub(crate) fn scope_decision<'a>(
    scope: &[SubjectStatus],
    subject_id: Option<&'a SubjectId>,
    subject_status: &BTreeMap<&str, SubjectStatus>,
) -> ScopeDecision<'a> {
    if scope.is_empty() {
        return ScopeDecision::InScope;
    }
    // A scope IS configured, so an unlinked scenario has no status to
    // match and falls outside it. This is the documented behavior, not
    // an oversight: `scope` exists to narrow blocking to work a subject
    // says is underway.
    let Some(subject_id) = subject_id else {
        return ScopeDecision::OutOfScope;
    };
    match subject_status.get(subject_id.as_str()) {
        Some(status) if scope.contains(status) => ScopeDecision::InScope,
        Some(_) => ScopeDecision::OutOfScope,
        None => ScopeDecision::DanglingSubject(subject_id.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{PolicyField, StalenessPolicy};
    use canon_model::{
        Actor, DivergenceStatus, Divergence, Envelope, EvidenceRecord, RecordKind, RoleId, Scenario, SpecDigest, Subject, TotalOrder,
    };
    use chrono::Utc;

    const PROJECT: &str = "platformer";

    fn project() -> ProjectId {
        ProjectId::parse(PROJECT).unwrap()
    }

    fn scenario(id: &str, subject: Option<&str>) -> Scenario {
        let envelope = Envelope::new(1, RecordKind::Scenario, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
        let mut s = Scenario::new(envelope, project(), ScenarioId::parse(id).unwrap(), "t", "d", SpecDigest::of(b"bytes"));
        s.subject_id = subject.map(|x| SubjectId::parse(x).unwrap());
        s
    }

    /// A SCENARIO-keyed record carrying the composite key — the shape
    /// `canon evidence add --scenario-id --project-id` now produces.
    fn evidence(scenario_id: &str, verdict: EvidenceVerdict) -> EvidenceRecord {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("agent-a", RoleId::parse("implementer").unwrap()));
        EvidenceRecord::new(envelope, None, Some(ScenarioId::parse(scenario_id).unwrap()), None, verdict).with_project_id(project())
    }

    fn divergence(scenario_id: &str, status: DivergenceStatus) -> Divergence {
        let envelope = Envelope::new(1, RecordKind::Divergence, Utc::now(), Actor::new("reviewer-1", RoleId::parse("reviewer").unwrap()));
        Divergence::new(
            envelope,
            project(),
            ScenarioId::parse(scenario_id).unwrap(),
            canon_model::Sha::parse("a".repeat(40)).unwrap(),
            status,
            TotalOrder::from(1u64),
            1,
            "reviewer-1",
            "d",
        )
    }

    fn subject(id: &str, status: SubjectStatus) -> Subject {
        let envelope = Envelope::new(1, RecordKind::Subject, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
        Subject::new(envelope, SubjectId::parse(id).unwrap(), "t", "s", "dev", status, RoleId::parse("implementer").unwrap())
    }

    fn policy(spec_coverage: Option<SpecCoverage>) -> crate::policy::PolicyResolution {
        crate::policy::PolicyResolution {
            trust_required: BTreeMap::new(),
            trust_sample: BTreeMap::new(),
            staleness: StalenessPolicy { max_commits_behind: PolicyField::Flat(50), surface_scoped: PolicyField::Flat(true) },
            risk_routing: BTreeMap::new(),
            risk_tiers: BTreeMap::new(),
            spec_coverage,
            evidence_binding: None,
            diagnostics: Vec::new(),
        }
    }

    struct Corpus {
        scenarios: Vec<Scenario>,
        evidence: Vec<EvidenceRecord>,
        divergences: Vec<Divergence>,
        subjects: Vec<Subject>,
        reviews: Vec<canon_model::Review>,
        findings: Vec<canon_model::Finding>,
        unreadable: Vec<RecordKind>,
    }

    impl Corpus {
        fn new() -> Self {
            Self {
                scenarios: Vec::new(),
                evidence: Vec::new(),
                divergences: Vec::new(),
                subjects: Vec::new(),
                reviews: Vec::new(),
                findings: Vec::new(),
                unreadable: Vec::new(),
            }
        }
    }

    fn context(corpus: Corpus, spec_coverage: Option<SpecCoverage>) -> GateContext {
        GateContext {
            ctx: crate::context::GateCtx { repo: "/tmp/repo".into(), ledger_root: "/tmp/repo/.canon/ledger".into() },
            policy: policy(spec_coverage),
            evidence: corpus.evidence,
            scenarios: corpus.scenarios,
            divergences: corpus.divergences,
            subjects: corpus.subjects,
            reviews: corpus.reviews,
            findings: corpus.findings,
            violations: Vec::new(),
            corpus_violations: Vec::new(),
            unreadable_kinds: corpus.unreadable,
            now: Utc::now(),
        }
    }

    fn run(corpus: Corpus, spec_coverage: Option<SpecCoverage>) -> Vec<Violation> {
        SpecCoverageCheck.run(&context(corpus, spec_coverage))
    }

    fn active(scope: Vec<SubjectStatus>) -> Option<SpecCoverage> {
        Some(SpecCoverage::Active { require_evidence: true, scope, exclude_lanes: Vec::new(), require_cases: Vec::new(), require_review: None })
    }

    fn requiring(cases: &[&str], exclude_lanes: &[&str]) -> Option<SpecCoverage> {
        Some(SpecCoverage::Active {
            require_evidence: false,
            scope: Vec::new(),
            exclude_lanes: exclude_lanes.iter().map(|s| s.to_string()).collect(),
            require_cases: cases.iter().map(|s| s.to_string()).collect(),
            require_review: None,
        })
    }

    fn with_case(mut s: Scenario, case: &str) -> Scenario {
        s.case = Some(case.to_string());
        s
    }

    /// The gap this axis exists for: a surface whose every scenario is
    /// attested still names no failure path, and evidence presence alone
    /// reads it as fully covered. Only the golden-path surface is
    /// reported; a surface with a failure case passes.
    #[test]
    fn a_surface_with_only_happy_scenarios_is_reported_for_the_missing_case() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![
            with_case(scenario("cart.add.01", None), "happy"),
            with_case(scenario("cart.add.02", None), "happy"),
            with_case(scenario("cart.pay.01", None), "happy"),
            with_case(scenario("cart.pay.02", None), "failure"),
        ];
        let violations = run(corpus, requiring(&["failure"], &[]));
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].subject, "cart.add");
        assert!(violations[0].detail.contains("@case:failure") && violations[0].detail.contains("2 in-scope"), "{}", violations[0].detail);
    }

    /// Untagged scenarios satisfy no case: an unclassified corpus must
    /// report every surface, never pass because it saw no tags.
    #[test]
    fn untagged_scenarios_satisfy_no_required_case() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("cart.add.01", None)];
        assert_eq!(run(corpus, requiring(&["failure"], &[])).len(), 1);
    }

    /// A failure scenario that is out of scope (an excluded lane) does
    /// not count for its surface; the requirement reads the same
    /// in-scope set the evidence check does.
    #[test]
    fn an_excluded_failure_scenario_does_not_satisfy_its_surface() {
        let mut corpus = Corpus::new();
        let mut process = with_case(scenario("cart.add.02", None), "failure");
        process.lane = Some("process".to_string());
        corpus.scenarios = vec![with_case(scenario("cart.add.01", None), "happy"), process];
        let violations = run(corpus, requiring(&["failure"], &["process"]));
        assert_eq!(violations.len(), 1);
        assert!(violations[0].detail.contains("1 in-scope"), "{}", violations[0].detail);
    }

    /// Re-tagging a scenario supersedes its old generation: the case
    /// read is the LATEST one, so adding `@case:failure` and re-syncing
    /// clears the gap.
    #[test]
    fn the_latest_scenario_generation_decides_its_case() {
        let mut corpus = Corpus::new();
        let stale = with_case(scenario("cart.add.01", None), "happy");
        let mut fresh = with_case(scenario("cart.add.01", None), "failure");
        fresh.envelope.at = stale.envelope.at + chrono::Duration::seconds(1);
        corpus.scenarios = vec![stale, fresh];
        assert!(run(corpus, requiring(&["failure"], &[])).is_empty());
    }

    /// The upgrade path every existing consumer takes: a corpus full of
    /// unevidenced scenarios, no policy section, zero violations.
    #[test]
    fn an_absent_policy_section_derives_nothing_even_with_an_unevidenced_corpus() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", None), scenario("p.b.01", None)];
        assert!(run(corpus, None).is_empty());
    }

    #[test]
    fn require_evidence_false_is_also_silent() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", None)];
        assert!(run(
            corpus,
            Some(SpecCoverage::Active { require_evidence: false, scope: Vec::new(), exclude_lanes: Vec::new(), require_cases: Vec::new(), require_review: None })
        )
        .is_empty());
    }

    /// The defect the whole change exists for: a spec nobody attested
    /// to used to be invisible to every check.
    #[test]
    fn a_scenario_with_no_evidence_is_reported() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", None)];

        let out = run(corpus, active(Vec::new()));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].class, FailureClass::UncoveredCell);
        assert_eq!(out[0].subject, "p.a.01");
        assert!(out[0].detail.contains("no evidence record"), "{}", out[0].detail);
    }

    #[test]
    fn an_excluded_lane_is_out_of_scope_after_subject_scope() {
        let mut corpus = Corpus::new();
        let mut excluded = scenario("p.a.01", None);
        excluded.lane = Some("process".to_string());
        corpus.scenarios = vec![excluded];
        let policy = Some(SpecCoverage::Active {
            require_evidence: true,
            scope: Vec::new(),
            exclude_lanes: vec!["process".to_string()],
            require_cases: Vec::new(),
            require_review: None,
        });
        assert!(run(corpus, policy).is_empty());
    }

    #[test]
    fn an_unlisted_lane_remains_in_scope() {
        let mut corpus = Corpus::new();
        let mut scenario = scenario("p.a.01", None);
        scenario.lane = Some("behavior".to_string());
        corpus.scenarios = vec![scenario];
        let policy = Some(SpecCoverage::Active {
            require_evidence: true,
            scope: Vec::new(),
            exclude_lanes: vec!["process".to_string()],
            require_cases: Vec::new(),
            require_review: None,
        });
        assert_eq!(run(corpus, policy).len(), 1);
    }

    #[test]
    fn absent_exclude_lanes_preserves_existing_coverage_behavior() {
        let mut corpus = Corpus::new();
        let mut scenario = scenario("p.a.01", None);
        scenario.lane = Some("process".to_string());
        corpus.scenarios = vec![scenario];
        assert_eq!(run(corpus, active(Vec::new())).len(), 1);
    }

    #[test]
    fn an_evidenced_scenario_is_clean() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", None)];
        corpus.evidence = vec![evidence("p.a.01", EvidenceVerdict::Faithful)];
        assert!(run(corpus, active(Vec::new())).is_empty());
    }

    /// The composite key is the scenario's identity: evidence missing a
    /// `project_id` must NOT satisfy the join, or evidence authored
    /// against one spec root would silently cover a same-named scenario
    /// in another.
    #[test]
    fn evidence_without_a_project_id_does_not_cover_a_scenario() {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("a", RoleId::parse("implementer").unwrap()));
        let half_keyed = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse("p.a.01").unwrap()), None, EvidenceVerdict::Faithful);

        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", None)];
        corpus.evidence = vec![half_keyed];

        let out = run(corpus, active(Vec::new()));
        assert_eq!(out.len(), 1, "a half key is not a match");
    }

    #[test]
    fn an_open_divergence_on_an_evidenced_scenario_is_mismatched_work() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", None)];
        corpus.evidence = vec![evidence("p.a.01", EvidenceVerdict::Faithful)];
        corpus.divergences = vec![divergence("p.a.01", DivergenceStatus::Open)];

        let out = run(corpus, active(Vec::new()));
        assert_eq!(out.len(), 1);
        assert!(out[0].detail.contains("open divergence"), "{}", out[0].detail);
    }

    #[test]
    fn a_resolved_divergence_is_not_mismatched_work() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", None)];
        corpus.evidence = vec![evidence("p.a.01", EvidenceVerdict::Faithful)];
        corpus.divergences = vec![divergence("p.a.01", DivergenceStatus::Resolved)];
        assert!(run(corpus, active(Vec::new())).is_empty());
    }

    #[test]
    fn a_divergent_latest_verdict_is_mismatched_work() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", None)];
        corpus.evidence = vec![evidence("p.a.01", EvidenceVerdict::Divergent)];

        let out = run(corpus, active(Vec::new()));
        assert_eq!(out.len(), 1);
        assert!(out[0].detail.contains("divergent"), "{}", out[0].detail);
    }

    /// One scenario yields at most one violation — an unevidenced
    /// scenario must not also be reported for whatever divergence rows
    /// happen to name it.
    #[test]
    fn one_scenario_is_never_double_counted() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", None)];
        corpus.divergences = vec![divergence("p.a.01", DivergenceStatus::Open)];

        let out = run(corpus, active(Vec::new()));
        assert_eq!(out.len(), 1, "unimplemented wins; the divergence arm must not fire too: {out:?}");
    }

    /// canon's ledger is append-only, so re-running `canon inventory
    /// sync` after a `.feature` edit leaves BOTH generations of a
    /// `Scenario` on disk. Measured on canon's own corpus at authoring
    /// time: 22 rows for 16 distinct keys. Unfolded, one scenario would
    /// be judged twice — and the STALE generation would decide `scope`,
    /// so a scenario that just gained an `@subject:` tag would still be
    /// measured against its untagged row.
    #[test]
    fn a_resynced_scenario_is_judged_once_on_its_latest_generation() {
        let stale = scenario("p.a.01", None);
        let mut fresh = scenario("p.a.01", Some("live"));
        fresh.envelope.at = stale.envelope.at + chrono::Duration::seconds(1);

        let mut corpus = Corpus::new();
        corpus.scenarios = vec![stale, fresh];
        corpus.subjects = vec![subject("live", SubjectStatus::Building)];

        let out = run(corpus, active(vec![SubjectStatus::Building]));
        assert_eq!(out.len(), 1, "one scenario, one violation — the stale row must not be judged too: {out:?}");
        assert!(out[0].detail.contains("no evidence record"), "and it must be judged on the FRESH row's subject link: {}", out[0].detail);
    }

    /// A scenario re-tagged from one subject to another belongs only to
    /// the new one: its stale generation still names the old subject on
    /// disk, and reading that row would keep shipping `old` gated on
    /// work it no longer owns.
    #[test]
    fn subject_scenarios_follow_the_latest_tag_generation() {
        let stale = scenario("p.a.01", Some("old"));
        let mut fresh = scenario("p.a.01", Some("new"));
        fresh.envelope.at = stale.envelope.at + chrono::Duration::seconds(1);
        let other = scenario("p.a.02", Some("old"));

        let mut corpus = Corpus::new();
        corpus.scenarios = vec![stale, fresh, other];
        let ctx = context(corpus, None);
        let ids = |s: &str| subject_scenarios(&ctx, &SubjectId::parse(s).unwrap()).into_iter().map(|sc| sc.scenario_id.as_str().to_string()).collect::<Vec<_>>();
        assert_eq!(ids("old"), vec!["p.a.02".to_string()]);
        assert_eq!(ids("new"), vec!["p.a.01".to_string()]);
    }

    // ── scope ──

    #[test]
    fn scope_narrows_to_subjects_in_the_named_statuses() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", Some("live")), scenario("p.b.01", Some("done"))];
        corpus.subjects = vec![subject("live", SubjectStatus::Building), subject("done", SubjectStatus::Shipped)];

        let out = run(corpus, active(vec![SubjectStatus::Building]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].subject, "p.a.01", "only the building subject's scenario blocks");
    }

    /// A configured scope excludes unlinked scenarios; an EMPTY scope
    /// means corpus-wide. Reading an omitted scope the other way would
    /// silently disable the check.
    #[test]
    fn an_unlinked_scenario_is_out_of_scope_only_when_a_scope_is_configured() {
        let mut with_scope = Corpus::new();
        with_scope.scenarios = vec![scenario("p.a.01", None)];
        assert!(run(with_scope, active(vec![SubjectStatus::Building])).is_empty());

        let mut no_scope = Corpus::new();
        no_scope.scenarios = vec![scenario("p.a.01", None)];
        assert_eq!(run(no_scope, active(Vec::new())).len(), 1);
    }

    #[test]
    fn a_dangling_subject_link_is_reported_rather_than_skipped() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", Some("ghost"))];

        let out = run(corpus, active(vec![SubjectStatus::Building]));
        assert_eq!(out.len(), 1);
        assert!(out[0].detail.contains("no Subject record"), "{}", out[0].detail);
    }

    // ── refusals ──

    /// A poisoned section must not behave like an absent one, or a typo
    /// silently equals never opting in.
    #[test]
    fn a_poisoned_policy_section_refuses_instead_of_passing() {
        let out = run(Corpus::new(), Some(SpecCoverage::Invalid { detail: "bad scope".to_string() }));
        assert_eq!(out.len(), 1);
        assert!(out[0].detail.contains("unusable"), "{}", out[0].detail);
    }

    /// A corpus kind routed off the gate's rung reads as empty. Passing
    /// on that is a gate that reports clean because it saw nothing —
    /// the failure that hid `Task` from `canon report`.
    #[test]
    fn a_corpus_kind_routed_off_the_read_rung_refuses_rather_than_passing_empty() {
        let mut corpus = Corpus::new();
        corpus.unreadable = vec![RecordKind::Scenario];

        let out = run(corpus, active(Vec::new()));
        assert_eq!(out.len(), 1);
        assert!(out[0].detail.contains("route away"), "{}", out[0].detail);
    }

    #[test]
    fn unreadable_kinds_are_ignored_when_the_section_is_absent() {
        let mut corpus = Corpus::new();
        corpus.unreadable = vec![RecordKind::Scenario];
        assert!(run(corpus, None).is_empty(), "an opted-out repo must not be told about routing it never engaged");
    }

    // ── require_review (issue #2) ──

    use crate::policy::RequireReview;
    use canon_model::{ChangeId, Finding, FindingDisposition, FindingSeverity, ProvenanceRef, Review, StatusOverride, WaivedViolation};

    fn reviewing(scope: Vec<SubjectStatus>, distinct_actor: bool) -> Option<SpecCoverage> {
        Some(SpecCoverage::Active {
            require_evidence: false,
            scope: Vec::new(),
            exclude_lanes: vec!["process".to_string()],
            require_cases: Vec::new(),
            require_review: Some(RequireReview { scope, distinct_actor, block_on_findings: true }),
        })
    }

    fn review(scenario_id: &str, reviewer: &str, actor: &str) -> Review {
        let envelope = Envelope::new(1, RecordKind::Review, Utc::now(), Actor::new(actor, RoleId::parse("reviewer").unwrap()));
        Review::new(envelope, project(), ScenarioId::parse(scenario_id).unwrap(), reviewer, "pin-1", ProvenanceRef::OriginalSpecRef("spec".into()))
    }

    fn finding(change: &str, seq: u32, severity: FindingSeverity, disposition: FindingDisposition, at_offset: i64) -> Finding {
        let mut envelope = Envelope::new(1, RecordKind::Finding, Utc::now(), Actor::new("reviewer-1", RoleId::parse("reviewer").unwrap()));
        envelope.at += chrono::Duration::seconds(at_offset);
        let f = Finding::new(envelope, ChangeId::parse(change).unwrap(), 1, seq, severity, "reviewer-1", "unbounded wait");
        match disposition {
            FindingDisposition::Open => f,
            FindingDisposition::Fixed => f.fixed_by(canon_model::Sha::parse("b".repeat(40)).unwrap()),
            FindingDisposition::Rejected => f.rejected(),
            FindingDisposition::Deferred => f.deferred(),
        }
    }

    fn verifying_subject(id: &str, changes: &[&str]) -> Subject {
        subject(id, SubjectStatus::Verifying).with_change_ids(changes.iter().map(|c| ChangeId::parse(*c).unwrap()).collect())
    }

    /// The issue's own scenario: implementer evidence, no review, subject
    /// at `verifying`. Clean before 0.12; `unreviewed-promotion` now.
    #[test]
    fn an_in_scope_scenario_without_a_review_is_unreviewed_promotion() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", Some("live")), scenario("p.b.01", Some("wip"))];
        corpus.subjects = vec![subject("live", SubjectStatus::Verifying), subject("wip", SubjectStatus::Building)];
        corpus.evidence = vec![evidence("p.a.01", EvidenceVerdict::Faithful)];

        let out = run(corpus, reviewing(RequireReview::DEFAULT_SCOPE.to_vec(), true));
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].class, FailureClass::UnreviewedPromotion);
        assert_eq!(out[0].subject, "p.a.01", "the building subject is out of review scope");
        assert!(out[0].detail.contains("no review record"), "{}", out[0].detail);
    }

    #[test]
    fn a_review_by_the_evidence_actor_counts_only_without_distinct_actor() {
        let corpus = || {
            let mut c = Corpus::new();
            c.scenarios = vec![scenario("p.a.01", Some("live"))];
            c.subjects = vec![subject("live", SubjectStatus::Verifying)];
            c.evidence = vec![evidence("p.a.01", EvidenceVerdict::Faithful)];
            // `agent-a` attested the evidence; a review naming another
            // reviewer but authored BY agent-a is still self-review.
            c.reviews = vec![review("p.a.01", "agent-a", "agent-a"), review("p.a.01", "someone-else", "agent-a")];
            c
        };
        let out = run(corpus(), reviewing(RequireReview::DEFAULT_SCOPE.to_vec(), true));
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].detail.contains("`agent-a`") && out[0].detail.contains("distinct_actor"), "{}", out[0].detail);

        assert!(run(corpus(), reviewing(RequireReview::DEFAULT_SCOPE.to_vec(), false)).is_empty());
    }

    #[test]
    fn a_distinct_review_passes_and_an_excluded_lane_needs_none() {
        let mut corpus = Corpus::new();
        let mut process = scenario("p.b.01", Some("live"));
        process.lane = Some("process".to_string());
        corpus.scenarios = vec![scenario("p.a.01", Some("live")), process];
        corpus.subjects = vec![subject("live", SubjectStatus::Shipped)];
        corpus.evidence = vec![evidence("p.a.01", EvidenceVerdict::Faithful)];
        corpus.reviews = vec![review("p.a.01", "reviewer-2", "reviewer-2")];
        assert!(run(corpus, reviewing(RequireReview::DEFAULT_SCOPE.to_vec(), true)).is_empty());
    }

    /// Findings fold latest-by-natural-key first: a blocker later closed
    /// as fixed no longer counts; a still-open one does; a should-fix or
    /// a finding on an unlisted change never does.
    #[test]
    fn only_a_latest_open_blocker_on_an_in_scope_subjects_change_is_open_blocker() {
        let mut corpus = Corpus::new();
        corpus.subjects = vec![verifying_subject("live", &["c-live"]), subject("wip", SubjectStatus::Building).with_change_ids(vec![ChangeId::parse("c-wip").unwrap()])];
        corpus.findings = vec![
            finding("c-live", 1, FindingSeverity::Blocker, FindingDisposition::Open, 0),
            finding("c-live", 2, FindingSeverity::Blocker, FindingDisposition::Open, 0),
            finding("c-live", 2, FindingSeverity::Blocker, FindingDisposition::Fixed, 5),
            finding("c-live", 3, FindingSeverity::ShouldFix, FindingDisposition::Open, 0),
            finding("c-wip", 1, FindingSeverity::Blocker, FindingDisposition::Open, 0),
            finding("c-other", 1, FindingSeverity::Blocker, FindingDisposition::Open, 0),
        ];
        let out = run(corpus, reviewing(RequireReview::DEFAULT_SCOPE.to_vec(), true));
        assert_eq!(out.iter().map(|v| (v.class, v.subject.as_str())).collect::<Vec<_>>(), vec![(FailureClass::OpenBlocker, "c-live#1.1")]);
        assert!(out[0].detail.contains("`live`"), "{}", out[0].detail);
    }

    fn waiver(waived: &[(&str, &str)]) -> StatusOverride {
        StatusOverride {
            to: SubjectStatus::Verifying,
            reason: "reviewer out".into(),
            waived: waived.iter().map(|(class, subject)| WaivedViolation { class: class.to_string(), subject: subject.to_string() }).collect(),
            actor: Actor::new_unattributed("lead"),
        }
    }

    /// A subject moved under `--override-reason` reports the gaps its
    /// waiver recorded as advisories naming the waiver, not violations,
    /// while it stays at the waived status.
    #[test]
    fn a_waived_subjects_recorded_gaps_are_advisories_not_violations() {
        let mut waived = verifying_subject("live", &["c-live"]);
        waived.status_override = Some(waiver(&[("open-blocker", "c-live#1.1"), ("unreviewed-promotion", "p.a.01")]));
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", Some("live"))];
        corpus.subjects = vec![waived];
        corpus.findings = vec![finding("c-live", 1, FindingSeverity::Blocker, FindingDisposition::Open, 0)];
        let ctx = context(corpus, reviewing(RequireReview::DEFAULT_SCOPE.to_vec(), true));

        assert!(SpecCoverageCheck.run(&ctx).is_empty());
        let advisories = crate::review_gate::review_advisories(&ctx).unwrap();
        assert_eq!(advisories.len(), 2);
        assert!(advisories.iter().all(|a| a.line().contains("[waiver: subject `live` moved to verifying by `lead`: reviewer out]")), "{advisories:?}");
    }

    /// The bug a class-level waiver had: waiving only the unreviewed
    /// scenario must not also waive a blocker raised afterwards.
    #[test]
    fn a_new_blocker_after_an_unreviewed_only_waiver_is_a_violation() {
        let mut waived = verifying_subject("live", &["c-live"]);
        waived.status_override = Some(waiver(&[("unreviewed-promotion", "p.a.01")]));
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", Some("live"))];
        corpus.subjects = vec![waived];
        corpus.findings = vec![finding("c-live", 1, FindingSeverity::Blocker, FindingDisposition::Open, 0)];
        let ctx = context(corpus, reviewing(RequireReview::DEFAULT_SCOPE.to_vec(), true));

        let out = SpecCoverageCheck.run(&ctx);
        assert_eq!(out.iter().map(|v| (v.class, v.subject.as_str())).collect::<Vec<_>>(), vec![(FailureClass::OpenBlocker, "c-live#1.1")]);
        let advisories = crate::review_gate::review_advisories(&ctx).unwrap();
        assert_eq!(advisories.iter().map(|a| a.violation.subject.as_str()).collect::<Vec<_>>(), vec!["p.a.01"]);
    }

    /// A scenario tagged to the subject after the waiver was granted is
    /// not covered by it, though the same class was waived for another.
    #[test]
    fn a_new_unreviewed_scenario_after_a_waiver_is_a_violation() {
        let mut waived = verifying_subject("live", &[]);
        waived.status_override = Some(waiver(&[("unreviewed-promotion", "p.a.01")]));
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", Some("live")), scenario("p.a.02", Some("live"))];
        corpus.subjects = vec![waived];
        let ctx = context(corpus, reviewing(RequireReview::DEFAULT_SCOPE.to_vec(), true));

        let out = SpecCoverageCheck.run(&ctx);
        assert_eq!(out.iter().map(|v| (v.class, v.subject.as_str())).collect::<Vec<_>>(), vec![(FailureClass::UnreviewedPromotion, "p.a.02")]);
        assert_eq!(crate::review_gate::review_advisories(&ctx).unwrap().len(), 1);
    }

    /// A Finding row the ledger read refused blocks as `malformed-evidence`
    /// named by its file, even with no subject in scope: it might be an
    /// open blocker. Other kinds' read problems are not this rule's, and
    /// `block_on_findings: false` turns it off.
    #[test]
    fn an_unreadable_finding_blocks_as_malformed_evidence_naming_its_file() {
        use canon_model::EvidenceViolation;
        let path = "kind=finding/c-live__0001__0001__0123456789ab.json";
        let corpus_violations = vec![
            (RecordKind::Finding, EvidenceViolation::new(canon_model::FailureClass::Malformed, path, "resolution_sha: fixed with no `resolution_sha`")),
            (RecordKind::Scenario, EvidenceViolation::new(canon_model::FailureClass::Malformed, "kind=scenario/x.json", "bad")),
        ];
        let mut ctx = context(Corpus::new(), reviewing(RequireReview::DEFAULT_SCOPE.to_vec(), true));
        ctx.corpus_violations = corpus_violations.clone();

        let out = SpecCoverageCheck.run(&ctx);
        assert_eq!(out.iter().map(|v| (v.class, v.subject.as_str())).collect::<Vec<_>>(), vec![(FailureClass::MalformedEvidence, path)]);
        assert!(out[0].detail.contains("resolution_sha") && out[0].detail.contains("fails closed"), "{}", out[0].detail);

        let mut off = RequireReview { scope: RequireReview::DEFAULT_SCOPE.to_vec(), distinct_actor: true, block_on_findings: false };
        let mut ctx = context(Corpus::new(), None);
        ctx.corpus_violations = corpus_violations;
        assert!(crate::review_gate::malformed_findings(&ctx, &off).is_empty());
        off.block_on_findings = true;
        assert_eq!(crate::review_gate::malformed_findings(&ctx, &off).len(), 1);
    }

    #[test]
    fn absent_require_review_reads_neither_reviews_nor_findings() {
        let mut corpus = Corpus::new();
        corpus.scenarios = vec![scenario("p.a.01", Some("live"))];
        corpus.subjects = vec![verifying_subject("live", &["c-live"])];
        corpus.findings = vec![finding("c-live", 1, FindingSeverity::Blocker, FindingDisposition::Open, 0)];
        corpus.unreadable = vec![RecordKind::Review, RecordKind::Finding];
        let ctx = context(corpus, requiring(&[], &[]));
        assert!(SpecCoverageCheck.run(&ctx).is_empty());
        assert!(crate::review_gate::review_advisories(&ctx).is_none());
    }

    #[test]
    fn a_review_kind_routed_off_the_read_rung_refuses() {
        let mut corpus = Corpus::new();
        corpus.unreadable = vec![RecordKind::Review];
        let out = run(corpus, reviewing(RequireReview::DEFAULT_SCOPE.to_vec(), true));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].subject, "spec_coverage.require_review");
    }
}
