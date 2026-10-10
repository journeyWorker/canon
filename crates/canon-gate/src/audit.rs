//! Audited outcomes (0.15 D3, TestJack): an external evaluator's
//! re-audit of a scenario's declared success, recorded with `canon
//! evidence add --kind audit`. Canon never generates or runs the audit;
//! it records which evaluator ran and what it concluded, and judges the
//! record.
//!
//! # Two folds, never one
//! An audit record carries `audit` ([`canon_model::EvidenceAudit`]) and
//! is split out of [`GateContext::evidence`] into
//! [`GateContext::audits`] at load, so the DECLARED fold
//! ([`crate::latest_verdicts`] and every check reading `ctx.evidence`)
//! never sees one: an audit sits beside the declared test-run record and
//! never replaces it. This module folds audits on their own, latest per
//! `(project_id, scenario_id)` ([`latest_audits`]).
//!
//! # `false-success`, always on
//! A scenario whose latest audit is `refuted` is [`FailureClass::FalseSuccess`]
//! ([`AuditCheck`]), whatever its declared verdict says and whether or not
//! `spec_coverage.require_audit` sampled it: a refutation is a recorded
//! fact, and hiding it because the policy did not ask would be the silent
//! false success this class exists to surface. Only a newer audit clears it.
//!
//! # `require_audit`: a deterministic sample
//! With `spec_coverage.require_audit: { rate, seed, scope }`, every
//! in-scope scenario [`crate::policy::RequireAudit::samples`] selects needs
//! a latest audit that is not `indeterminate`; a missing or indeterminate
//! one is `uncovered-cell` with an audit detail ([`obligation_violation`]).
//! Scope works like `require_review.scope` (subject statuses, default
//! `[verifying, shipped]`, empty = every scenario), and
//! `spec_coverage.exclude_lanes` removes scenarios first.

use std::collections::BTreeMap;

use canon_model::{AuditResult, EvidenceRecord, ProjectId, Scenario, ScenarioId, SubjectId, SubjectStatus};

use crate::context::{GateCheck, GateContext};
use crate::failure_class::{FailureClass, Violation};
use crate::policy::{RequireAudit, SpecCoverage};
use crate::spec_coverage::{latest_by_scenario, latest_scenarios, latest_subjects, scope_decision, subject_scenarios, ScopeDecision};

/// The latest audit record per `(project_id, scenario_id)`. An audit
/// without both keys joins no scenario and is skipped (the CLI never
/// writes one).
pub fn latest_audits(ctx: &GateContext) -> BTreeMap<(ProjectId, ScenarioId), &EvidenceRecord> {
    latest_by_scenario(&ctx.audits)
}

/// The always-on audit check: `false-success` for every scenario whose
/// latest audit is `refuted` (module doc).
pub struct AuditCheck;

impl GateCheck for AuditCheck {
    fn name(&self) -> &'static str {
        "audit"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        latest_audits(ctx).into_iter().filter_map(|((_, scenario_id), record)| false_success(scenario_id.as_str(), record)).collect()
    }
}

/// `Some(false-success)` when `record` (a scenario's latest audit) is
/// `refuted`.
pub fn false_success(scenario_id: &str, record: &EvidenceRecord) -> Option<Violation> {
    let audit = record.audit.as_ref().filter(|a| a.result == AuditResult::Refuted)?;
    let mut counterexamples: Vec<&str> = record.attachments.iter().map(|a| a.path.as_str()).collect();
    counterexamples.dedup();
    let counterexamples = if counterexamples.is_empty() { String::new() } else { format!(", counterexamples: {}", counterexamples.join(", ")) };
    Some(Violation::new(
        FailureClass::FalseSuccess,
        scenario_id,
        format!(
            "latest audit refuted this scenario's declared success (evaluator `{}` {}, sha256 {}{counterexamples}); a declared faithful verdict does not override it — fix the behavior, re-attest, and record the evaluator's new audit with `canon evidence add --kind audit`",
            audit.evaluator_id, audit.evaluator_version, audit.evaluator_digest
        ),
    ))
}

/// Where a sampled scenario's audit stands.
#[derive(Debug, Clone, Copy)]
pub enum AuditState<'a> {
    Missing,
    Indeterminate(&'a EvidenceRecord),
    Confirmed(&'a EvidenceRecord),
    Refuted(&'a EvidenceRecord),
}

impl AuditState<'_> {
    /// Whether the sample still owes an audit (missing or indeterminate).
    pub fn is_owed(&self) -> bool {
        matches!(self, AuditState::Missing | AuditState::Indeterminate(_))
    }

    /// `missing`, `indeterminate`, `confirmed`, or `refuted`.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditState::Missing => "missing",
            AuditState::Indeterminate(_) => "indeterminate",
            AuditState::Confirmed(_) => "confirmed",
            AuditState::Refuted(_) => "refuted",
        }
    }
}

/// One in-scope scenario the `require_audit` sample selected.
#[derive(Debug, Clone, Copy)]
pub struct AuditObligation<'a> {
    pub scenario: &'a Scenario,
    pub state: AuditState<'a>,
}

/// The active `require_audit` settings and the `exclude_lanes` they are
/// applied with, or `None` when the policy samples nothing.
pub fn active_require_audit(ctx: &GateContext) -> Option<(&RequireAudit, &[String])> {
    match &ctx.policy.spec_coverage {
        Some(SpecCoverage::Active { require_audit: Some(require_audit), exclude_lanes, .. }) => Some((require_audit, exclude_lanes.as_slice())),
        _ => None,
    }
}

/// Every sampled, in-scope scenario (latest generation, in key order)
/// and its audit state. A scenario naming a subject no record carries is
/// skipped: `spec_coverage` reports the dangling link itself.
pub fn sampled<'a>(ctx: &'a GateContext, require_audit: &RequireAudit, exclude_lanes: &[String]) -> Vec<AuditObligation<'a>> {
    let status: BTreeMap<&str, SubjectStatus> = latest_subjects(ctx).into_iter().map(|s| (s.subject_id.as_str(), s.status)).collect();
    let audits = latest_audits(ctx);
    latest_scenarios(ctx)
        .into_iter()
        .filter(|s| matches!(scope_decision(&require_audit.scope, s.subject_id.as_ref(), &status), ScopeDecision::InScope))
        .filter(|s| !is_excluded(s, exclude_lanes))
        .filter(|s| require_audit.samples(s.project_id.as_str(), s.scenario_id.as_str()))
        .map(|scenario| AuditObligation { scenario, state: state_of(&audits, scenario) })
        .collect()
}

fn state_of<'a>(audits: &BTreeMap<(ProjectId, ScenarioId), &'a EvidenceRecord>, scenario: &Scenario) -> AuditState<'a> {
    let key = (scenario.project_id.clone(), scenario.scenario_id.clone());
    match audits.get(&key).copied() {
        None => AuditState::Missing,
        Some(record) => match record.audit.as_ref().map(|a| a.result) {
            Some(AuditResult::Confirmed) => AuditState::Confirmed(record),
            Some(AuditResult::Refuted) => AuditState::Refuted(record),
            Some(AuditResult::Indeterminate) | None => AuditState::Indeterminate(record),
        },
    }
}

fn is_excluded(scenario: &Scenario, exclude_lanes: &[String]) -> bool {
    scenario.lane.as_ref().is_some_and(|lane| exclude_lanes.contains(lane))
}

/// `Some(uncovered-cell)` for a sampled scenario that still owes an
/// audit. A refuted one is already `false-success` ([`AuditCheck`]); a
/// confirmed one owes nothing.
pub fn obligation_violation(require_audit: &RequireAudit, obligation: &AuditObligation) -> Option<Violation> {
    let sample = format!("sampled for audit by spec_coverage.require_audit (rate {}, seed `{}`)", require_audit.rate.get(), require_audit.seed);
    let detail = match obligation.state {
        AuditState::Missing => format!(
            "{sample} and has no audit record — an external evaluator must re-audit it; record its result with `canon evidence add --kind audit`"
        ),
        AuditState::Indeterminate(record) => {
            let (id, version) = record.audit.as_ref().map_or(("?", "?"), |a| (a.evaluator_id.as_str(), a.evaluator_version.as_str()));
            format!("{sample}, but its latest audit is indeterminate (evaluator `{id}` {version}) — an undecided audit is not an audit; have the evaluator re-audit it")
        }
        AuditState::Confirmed(_) | AuditState::Refuted(_) => return None,
    };
    Some(Violation::new(FailureClass::UncoveredCell, obligation.scenario.scenario_id.as_str(), detail))
}

/// The corpus-wide `require_audit` rule `canon gate check` applies.
pub(crate) fn evaluate(ctx: &GateContext, require_audit: &RequireAudit, exclude_lanes: &[String]) -> Vec<Violation> {
    sampled(ctx, require_audit, exclude_lanes).iter().filter_map(|o| obligation_violation(require_audit, o)).collect()
}

/// The audit rule `canon subject status <id> shipped` applies: every
/// owned scenario whose latest audit is refuted (`false-success`, always),
/// and — when `require_audit` covers `target` — every owned, sampled
/// scenario that still owes an audit. The same [`false_success`] and
/// [`obligation_violation`] the gate uses, so the two cannot disagree.
pub fn subject_audit_gaps(ctx: &GateContext, subject_id: &SubjectId, target: SubjectStatus) -> Vec<Violation> {
    let owned = subject_scenarios(ctx, subject_id);
    let audits = latest_audits(ctx);
    let mut violations: Vec<Violation> = owned
        .iter()
        .filter_map(|s| audits.get(&(s.project_id.clone(), s.scenario_id.clone())).and_then(|record| false_success(s.scenario_id.as_str(), record)))
        .collect();
    if let Some((require_audit, exclude_lanes)) = active_require_audit(ctx).filter(|(ra, _)| ra.covers(target)) {
        violations.extend(
            owned
                .iter()
                .filter(|s| !is_excluded(s, exclude_lanes) && require_audit.samples(s.project_id.as_str(), s.scenario_id.as_str()))
                .filter_map(|scenario| obligation_violation(require_audit, &AuditObligation { scenario, state: state_of(&audits, scenario) })),
        );
    }
    violations
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use canon_model::{Actor, Envelope, EvidenceAudit, EvidenceVerdict, RecordKind, RoleId, SpecDigest};
    use chrono::Utc;

    use super::*;
    use crate::context::GateCtx;
    use crate::ledger::latest_verdicts;
    use crate::policy::{AuditRate, PolicyField, PolicyResolution, StalenessPolicy};

    fn require_audit(rate: f64, seed: &str) -> RequireAudit {
        RequireAudit { rate: AuditRate::new(rate).unwrap(), seed: seed.to_string(), scope: Vec::new() }
    }

    fn record(scenario: &str, verdict: EvidenceVerdict, audit: Option<AuditResult>, offset: i64) -> EvidenceRecord {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now() + chrono::Duration::seconds(offset), Actor::new("agent", RoleId::parse("implementer").unwrap()));
        let record = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse(scenario).unwrap()), None, verdict).with_project_id(ProjectId::parse("root").unwrap());
        match audit {
            Some(result) => record.with_audit(EvidenceAudit { evaluator_id: "ev".into(), evaluator_version: "1".into(), evaluator_digest: "b".repeat(64), result }),
            None => record,
        }
    }

    fn ctx(records: Vec<EvidenceRecord>, require: Option<RequireAudit>) -> GateContext {
        let (audits, evidence) = records.into_iter().partition(EvidenceRecord::is_audit);
        let envelope = Envelope::new(1, RecordKind::Scenario, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
        let scenario = Scenario::new(envelope, ProjectId::parse("root").unwrap(), ScenarioId::parse("shop.cart.02").unwrap(), "t", "d", SpecDigest::of(b"x"));
        GateContext {
            ctx: GateCtx { repo: "/tmp/repo".into(), ledger_root: "/tmp/repo/.canon/ledger".into() },
            policy: PolicyResolution {
                trust_required: BTreeMap::new(),
                trust_sample: BTreeMap::new(),
                staleness: StalenessPolicy { max_commits_behind: PolicyField::Flat(50), surface_scoped: PolicyField::Flat(true) },
                risk_routing: BTreeMap::new(),
                risk_tiers: BTreeMap::new(),
                spec_coverage: require.map(|ra| SpecCoverage::Active {
                    require_evidence: false,
                    scope: Vec::new(),
                    exclude_lanes: Vec::new(),
                    require_cases: Vec::new(),
                    require_review: None,
                    require_audit: Some(ra),
                }),
                evidence_binding: None,
                diagnostics: Vec::new(),
            },
            evidence,
            audits,
            scenarios: vec![scenario],
            divergences: Vec::new(),
            subjects: Vec::new(),
            reviews: Vec::new(),
            findings: Vec::new(),
            violations: Vec::new(),
            corpus_violations: Vec::new(),
            unreadable_kinds: Vec::new(),
            now: Utc::now(),
        }
    }

    /// The documented formula, pinned on known vectors: the first 8
    /// bytes of sha256("canon-0.15" 0x00 "root" 0x00 id), big-endian,
    /// below rate * 2^64.
    #[test]
    fn sampling_is_the_documented_hash_threshold() {
        let ra = require_audit(0.5, "canon-0.15");
        let sampled: Vec<&str> = ["shop.cart.01", "shop.cart.02", "shop.cart.03", "shop.cart.04", "shop.cart.05", "shop.cart.06"]
            .into_iter()
            .filter(|id| ra.samples("root", id))
            .collect();
        assert_eq!(sampled, ["shop.cart.02", "shop.cart.05", "shop.cart.06"]);
        assert!(!require_audit(0.5, "s1").samples("root", "shop.cart.02"), "the seed re-draws the sample");
        assert!(!require_audit(0.5, "canon-0.15").samples("other", "shop.cart.02"), "the project is part of the key");
        assert!((0..50).all(|n| !require_audit(0.0, "x").samples("root", &format!("a.b.{n:02}"))), "rate 0 samples nothing");
        assert!((0..50).all(|n| require_audit(1.0, "x").samples("root", &format!("a.b.{n:02}"))), "rate 1 samples everything");
        assert!(AuditRate::new(1.01).is_none() && AuditRate::new(-0.1).is_none() && AuditRate::new(f64::NAN).is_none());
    }

    /// An audit never displaces the declared verdict, and a refuted
    /// latest audit is `false-success` beside a faithful one.
    #[test]
    fn a_refuted_audit_sits_beside_the_declared_verdict_as_false_success() {
        let c = ctx(vec![record("shop.cart.02", EvidenceVerdict::Faithful, None, 0), record("shop.cart.02", EvidenceVerdict::Divergent, Some(AuditResult::Refuted), 5)], None);
        let verdicts = latest_verdicts(&c);
        assert_eq!(verdicts.len(), 1);
        assert!(verdicts.values().all(|e| e.verdict == EvidenceVerdict::Faithful), "{verdicts:?}");
        let violations = AuditCheck.run(&c);
        assert_eq!(violations.len(), 1);
        assert_eq!((violations[0].class, violations[0].subject.as_str()), (FailureClass::FalseSuccess, "shop.cart.02"));
    }

    /// The latest audit decides: a newer confirmed audit clears a
    /// refutation, and a newer indeterminate one leaves a sampled
    /// scenario owing an audit.
    #[test]
    fn the_latest_audit_decides() {
        let refuted_then_confirmed =
            ctx(vec![record("shop.cart.02", EvidenceVerdict::Divergent, Some(AuditResult::Refuted), 0), record("shop.cart.02", EvidenceVerdict::Faithful, Some(AuditResult::Confirmed), 5)], None);
        assert!(AuditCheck.run(&refuted_then_confirmed).is_empty());

        let ra = require_audit(1.0, "");
        let indeterminate = ctx(vec![record("shop.cart.02", EvidenceVerdict::NotApplicable, Some(AuditResult::Indeterminate), 0)], Some(ra.clone()));
        let owed = evaluate(&indeterminate, &ra, &[]);
        assert_eq!(owed.len(), 1);
        assert!(owed[0].class == FailureClass::UncoveredCell && owed[0].detail.contains("latest audit is indeterminate"), "{owed:?}");

        let missing = ctx(Vec::new(), Some(ra.clone()));
        assert!(evaluate(&missing, &ra, &[])[0].detail.contains("has no audit record"));
    }
}
