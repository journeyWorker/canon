//! One rule for "pinned to what, stale when" (0.23 D6).
//!
//! Three things in canon are pinned to what a scenario's evidence said, and
//! they used to compute that three ways:
//!
//! - a **review** ([`crate::review_gate`]) pins the scenario's spec digest
//!   and, per role, the verdict and checks of its latest evidence;
//! - an **accepted divergence** ([`crate::acceptance`]) pins the same
//!   evidence state, so it ends when the gap it accepted changes;
//! - **re-verification** ([`crate::review_gate::fixed_blockers`], and the
//!   chore trigger in `canon-cli`) waits for evidence newer than a moment
//!   ([`scenarios_without_evidence_after`]).
//!
//! # What evidence says
//! Not which record it is. Evidence is pinned by its **verdict** and the
//! **identity of its checks** ([`CheckPin`]): for a record bound to a test
//! report, the report format plus the name of each matched case; for a
//! record bound to artifacts only, the digest of each file. Recording the
//! suite again therefore leaves a review current when the verdict and the
//! cases are the same, and stales it when a verdict flips, a case appears
//! or disappears, or the scenario's own text changes. Evidence is read as
//! the latest record per role, the fold every other rule uses.
//!
//! # Stale when
//! [`drift`] compares what was pinned with what is true now and names what
//! moved: the spec (`spec`, and which kind of digest), the evidence
//! (`evidence`), or the pin itself being of the pre-0.23 kind
//! (`unmigrated`): a review pinned to evidence record ids says nothing the
//! gate can compare, so it does not count until `canon migrate` rewrites it.

use std::collections::BTreeSet;

use canon_model::{CheckPin, EvidenceRecord, PinnedSpec, ReviewPins, Scenario, SpecDigest, SubjectId};
use chrono::{DateTime, Utc};

use crate::context::GateContext;
use crate::spec_coverage::{latest_by_key, subject_scenarios};

/// Every evidence record for the scenario's id whose `project_id`
/// matches or is absent. Counting a project-less record is the stricter
/// reading: it can only make a review not count, never let one count.
pub fn scenario_evidence<'a>(ctx: &'a GateContext, scenario: &Scenario) -> Vec<&'a EvidenceRecord> {
    ctx.evidence
        .iter()
        .filter(|e| e.scenario_id.as_ref() == Some(&scenario.scenario_id) && e.project_id.as_ref().is_none_or(|p| *p == scenario.project_id))
        .collect()
}

/// The identity of the checks `record` bound: `<format>:<case>` for each
/// matched report case, or, when it is bound to no report, `sha256:<hex>`
/// for each artifact. Sorted, each once; empty for a record that bound
/// nothing.
pub fn check_identity(record: &EvidenceRecord) -> Vec<String> {
    let mut cases: BTreeSet<String> = BTreeSet::new();
    let mut artifacts: BTreeSet<String> = BTreeSet::new();
    for attachment in &record.attachments {
        match &attachment.format {
            Some(format) => {
                let format = serde_json::to_value(format).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
                cases.insert(format!("{format}:{}", attachment.case.as_deref().unwrap_or_default()));
            }
            None => {
                artifacts.insert(format!("sha256:{}", attachment.sha256));
            }
        }
    }
    if cases.is_empty() { artifacts.into_iter().collect() } else { cases.into_iter().collect() }
}

/// What `scenario`'s evidence says now: per role, the latest record's
/// verdict and check identity, in `(role, verdict, checks)` order.
pub fn current_checks(ctx: &GateContext, scenario: &Scenario) -> Vec<CheckPin> {
    let evidence = scenario_evidence(ctx, scenario);
    let mut pins: Vec<CheckPin> = latest_by_key(&evidence, |e| e.envelope.actor.role.as_ref().map(|r| r.as_str().to_string()))
        .into_iter()
        .map(|record| CheckPin { role: record.envelope.actor.role.as_ref().map(|r| r.as_str().to_string()), verdict: record.verdict, checks: check_identity(record) })
        .collect();
    pins.sort();
    pins
}

/// The spec pin `scenario` offers now: its `scenario_digest` (scope
/// `scenario`) when the record carries one, else its file-wide
/// `source_digest` (scope `file`).
pub fn spec_pin(scenario: &Scenario) -> (SpecDigest, PinnedSpec) {
    match &scenario.scenario_digest {
        Some(digest) => (digest.clone(), PinnedSpec::Scenario),
        None => (scenario.source_digest.clone(), PinnedSpec::File),
    }
}

/// What a review of `scenario` pins right now: the spec pin and the
/// evidence's verdicts and checks. `canon review add` writes this.
pub fn current_pins(ctx: &GateContext, scenario: &Scenario) -> ReviewPins {
    let (scenario_digest, scenario_digest_scope) = spec_pin(scenario);
    ReviewPins { scenario_digest, scenario_digest_scope, checks: Some(current_checks(ctx, scenario)), evidence: Vec::new() }
}

/// What moved under something pinned since it was pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drift {
    /// The spec pin no longer matches, and which kind of digest it was.
    pub spec: Option<PinnedSpec>,
    /// The latest evidence's verdict or checks are not the pinned ones.
    pub evidence: bool,
    /// The pin is of the pre-0.23 kind (evidence record ids, or none at
    /// all), which the gate cannot compare: `canon migrate` rewrites it.
    pub unmigrated: bool,
}

/// `None` when `pins` still describes `scenario` and `now` (its evidence
/// from [`current_checks`]); otherwise what moved. A spec pin is compared
/// like for like: a file pin against the scenario's `source_digest`, a
/// scenario pin against its `scenario_digest` (a record without one no
/// longer matches it).
pub fn drift(pins: Option<&ReviewPins>, scenario: &Scenario, now: &[CheckPin]) -> Option<Drift> {
    let Some(pinned_checks) = pins.and_then(|p| p.checks.as_ref()) else {
        return Some(Drift { spec: None, evidence: false, unmigrated: true });
    };
    let pins = pins.expect("checks came from the pins");
    let spec_current = match pins.scenario_digest_scope {
        PinnedSpec::File => pins.scenario_digest == scenario.source_digest,
        PinnedSpec::Scenario => scenario.scenario_digest.as_ref() == Some(&pins.scenario_digest),
        PinnedSpec::Brief => false,
    };
    let mut pinned = pinned_checks.clone();
    pinned.sort();
    let spec = (!spec_current).then_some(pins.scenario_digest_scope);
    let evidence = pinned != now;
    (spec.is_some() || evidence).then_some(Drift { spec, evidence, unmigrated: false })
}

/// Whether `pinned`, the evidence state an acceptance saw, is still what
/// `scenario`'s latest evidence says. `None` (an acceptance recorded
/// before 0.23) is never current.
pub fn evidence_current(pinned: Option<&[CheckPin]>, now: &[CheckPin]) -> bool {
    let Some(pinned) = pinned else { return false };
    let mut pinned = pinned.to_vec();
    pinned.sort();
    pinned == now
}

/// Re-verification (0.22 D2): the scenarios `subject` owns that have
/// evidence but none recorded after `after`. A scenario with no evidence at
/// all is not listed; the ship guard reports it as missing evidence on its
/// own.
pub fn scenarios_without_evidence_after<'a>(ctx: &'a GateContext, subject: &SubjectId, after: DateTime<Utc>) -> Vec<&'a Scenario> {
    subject_scenarios(ctx, subject)
        .into_iter()
        .filter(|scenario| {
            let evidence = scenario_evidence(ctx, scenario);
            !evidence.is_empty() && evidence.iter().all(|e| e.envelope.at <= after)
        })
        .collect()
}
