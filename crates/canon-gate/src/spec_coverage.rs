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
//! Two facts about every `Scenario` in the corpus, both of which a
//! spec-driven repo would call "work that is not done":
//!
//! - **Unimplemented** — no `EvidenceRecord` attests to it at all.
//! - **Mismatched** — its folded divergence state is `Open`,
//!   `StillDivergent`, or `ResolvedInvalid`, or its latest ledger
//!   verdict is `Divergent`.
//!
//! Both surface as [`FailureClass::UncoveredCell`] with a
//! distinguishing detail. The closed eight-member class set is NOT
//! extended: `canon-cli`'s `verifying → shipped` subject gate already
//! set that precedent for exactly this situation, reporting both "no
//! verdict" and "divergent verdict" under one class with different
//! details, because the class vocabulary describes gate OUTCOMES, not
//! causes.
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
use canon_model::{EvidenceVerdict, ProjectId, ScenarioId, SubjectId, SubjectStatus};

use crate::context::{GateCheck, GateContext};
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
            Some(SpecCoverage::Active { require_evidence, .. }) if !require_evidence => return Vec::new(),
            Some(active @ SpecCoverage::Active { .. }) => active,
        };

        // A kind routed off the rung this context reads yields an empty
        // vector for a reason that is NOT "the corpus is empty". Passing
        // on that input would be a gate that reports clean because it
        // saw nothing — the failure mode that hid `Task` from
        // `canon report`. Refuse before deriving anything.
        if !ctx.unreadable_kinds.is_empty() {
            let kinds = ctx.unreadable_kinds.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ");
            return vec![Violation::new(
                FailureClass::UncoveredCell,
                "spec_coverage",
                format!(
                    "`spec_coverage` is enabled but these kinds route away from the rung the gate reads ({kinds}), so their corpus would read as empty and this check would pass by seeing nothing; route them to `local` or disable the section"
                ),
            )];
        }

        let SpecCoverage::Active { scope, exclude_lanes, .. } = policy else {
            unreachable!("the non-Active arms return above");
        };

        let evidenced = evidenced_scenarios(ctx);
        let divergence_states = fold_to_current_state(&ctx.divergences, &live_bindings(ctx), ctx.now);
        let verdicts = latest_verdicts(ctx);
        let subject_status: BTreeMap<&str, SubjectStatus> =
            latest_by_key(&ctx.subjects, |s| s.subject_id.as_str().to_string()).into_iter().map(|s| (s.subject_id.as_str(), s.status)).collect();

        let mut violations = Vec::new();
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
        violations
    }
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
fn latest_by_key<'a, T: serde::Serialize, K: Ord>(records: &'a [T], key: impl Fn(&T) -> K) -> Vec<&'a T> {
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

enum ScopeDecision<'a> {
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
fn scope_decision<'a>(
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
            spec_coverage,
            diagnostics: Vec::new(),
        }
    }

    struct Corpus {
        scenarios: Vec<Scenario>,
        evidence: Vec<EvidenceRecord>,
        divergences: Vec<Divergence>,
        subjects: Vec<Subject>,
        unreadable: Vec<RecordKind>,
    }

    impl Corpus {
        fn new() -> Self {
            Self { scenarios: Vec::new(), evidence: Vec::new(), divergences: Vec::new(), subjects: Vec::new(), unreadable: Vec::new() }
        }
    }

    fn run(corpus: Corpus, spec_coverage: Option<SpecCoverage>) -> Vec<Violation> {
        let ctx = GateContext {
            ctx: crate::context::GateCtx { repo: "/tmp/repo".into(), ledger_root: "/tmp/repo/.canon/ledger".into() },
            policy: policy(spec_coverage),
            evidence: corpus.evidence,
            scenarios: corpus.scenarios,
            divergences: corpus.divergences,
            subjects: corpus.subjects,
            violations: Vec::new(),
            corpus_violations: Vec::new(),
            unreadable_kinds: corpus.unreadable,
            now: Utc::now(),
        };
        SpecCoverageCheck.run(&ctx)
    }

    fn active(scope: Vec<SubjectStatus>) -> Option<SpecCoverage> {
        Some(SpecCoverage::Active { require_evidence: true, scope, exclude_lanes: Vec::new() })
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
        assert!(run(corpus, Some(SpecCoverage::Active { require_evidence: false, scope: Vec::new(), exclude_lanes: Vec::new() })).is_empty());
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
}
