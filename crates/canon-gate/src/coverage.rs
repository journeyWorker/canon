//! The static coverage check (design decision 1/D3a): does a
//! policy-required evidence CELL exist at all, independent of whether
//! it passed. Generalizes `tools/parity.py`'s `_required_platforms`/
//! `_allowed_lanes`/`_required_lanes` → uncovered-cell pipeline
//! (the donor parity-harness audit's policy-derivation notes §3.1,
//! static-gate notes §3.2/spec.md "Required cell with no evidence fails
//! coverage") over this crate's own corpus shape.
//!
//! # Generalizing the "fact" side to an `EvidenceRecord`-only corpus
//! parity.py derives required cells from a `Scenario`'s own TAGS
//! (`design_review_risk`/`risk_platforms` intersected against a tag
//! `frozenset` — policy-derivation.md §3.1/3.4: "`(artifact, policy) ->
//! derived-set`, no I/O, no global state"). `GateContext` (this crate's
//! FROZEN corpus type, `context.rs`) carries no separate
//! tagged-artifact type — only `evidence: Vec<EvidenceRecord>` — and
//! [`PolicyResolution`]'s own CEL contract is built for exactly that
//! shape ("Every predicate's `record` variable is one
//! `canon_model::EvidenceRecord`'s own `serde_json::to_value(...)`",
//! `policy.rs` module doc). This check therefore reads
//! [`PolicyResolution::risk_routing`]'s keys as the required-cell
//! vocabulary (parity.py's `design-review`/`code-review`/`unit-test`/
//! `e2e-patrol` lane set, generalized to an open, repo-declared string
//! set — `policy.rs`'s own doc: "each key resolves to a boolean (does
//! this routing rule apply)") and evaluates each rule against the
//! artifact's OWN already-submitted evidence (never a synthetic/absent
//! record — a rule can only be proven to apply using a record that
//! actually exists) to decide whether a cell is REQUIRED; a required
//! cell is COVERED when at least one of the artifact's records was
//! authored by the matching role. A policy diff alone (adding a
//! `risk_routing` key) therefore tightens coverage for every existing
//! artifact with zero corpus edits — the literal acceptance criterion
//! (spec.md "Policy change alone tightens coverage",
//! policy-derivation.md's P4 example: "reviewing and merging one line
//! ... mechanically adds ... required cells to every scenario ... — no
//! ... file is touched, no scenario is re-tagged").
//!
//! # What this check does NOT do
//! Coverage is "a test exists", never "a test passed" (design decision
//! 1: "'A test exists' and 'a test passed' are different facts with
//! different staleness windows") — this check never inspects
//! [`canon_model::EvidenceRecord::verdict`]; a `Divergent` record still
//! satisfies coverage for its role. [`crate::ledger`] (D3b, dynamic)
//! is the SEPARATE pass that reads verdicts.
//!
//! # Interface gap: a genuinely zero-evidence artifact is undiscoverable
//! This check can only enumerate join-key artifacts that appear
//! SOMEWHERE in `ctx.evidence` (there is no external Task/Scenario
//! registry in the frozen `GateContext` shape to discover an artifact
//! with LITERALLY zero submitted evidence). A gated task that has
//! never received any evidence at all is `gated-task-completion`'s
//! territory instead (`unevidenced-flip`, task 3.2's `canon gate task`
//! check, which resolves the task row via the join spine directly
//! rather than the ledger) — the two checks are deliberately
//! complementary, not overlapping: this module answers "given
//! something IS being evidenced, is every required role present";
//! `gated-task-completion` answers "does this specific gated task have
//! ANY evidence before its checkbox may flip".

use std::collections::BTreeMap;

use canon_model::EvidenceRecord;

use crate::context::{GateCheck, GateContext};
use crate::failure_class::{FailureClass, Violation};

/// One coverage subject's join-spine identity — `task_id` preferred
/// (join-spine table: "task ↔ evidence ↔ trajectory"), falling back to
/// `scenario_id` (join-spine table: "spec ↔ test ↔ ledger ↔
/// divergence") when a record carries no `task_id`. A record with
/// neither carries no coverage subject at all and is excluded from
/// this check (module doc's interface-gap note) — it is not, itself, a
/// violation; a bare `run`/`event`-shaped evidence record with no join
/// key is out of `uncovered-cell`'s scope entirely. `pub(crate)`: the
/// SAME subject identity [`crate::ledger`]'s verdict fold and
/// [`crate::staleness`]'s per-cell staleness check reuse — one
/// definition, never three independently hand-rolled `task_id`-or-
/// `scenario_id` fallbacks drifting relative to each other.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CellSubject {
    Task(String),
    Scenario(String),
}

impl CellSubject {
    pub(crate) fn of(record: &EvidenceRecord) -> Option<Self> {
        if let Some(task_id) = &record.task_id {
            Some(CellSubject::Task(task_id.to_string()))
        } else {
            record.scenario_id.as_ref().map(|scenario_id| CellSubject::Scenario(scenario_id.to_string()))
        }
    }

    /// The `Violation::subject` this cell's violations cite.
    pub(crate) fn as_str(&self) -> &str {
        match self {
            CellSubject::Task(s) | CellSubject::Scenario(s) => s,
        }
    }
}

/// The static coverage check (D3a) — one [`GateCheck`] emitting
/// [`FailureClass::UncoveredCell`] for every (artifact, required-role)
/// pair `ctx.policy.risk_routing` derives as required but with no
/// matching-role evidence record (module doc).
pub struct CoverageCheck;

impl GateCheck for CoverageCheck {
    fn name(&self) -> &'static str {
        "coverage"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        // Group every well-formed record by its coverage subject
        // (module doc's `CellSubject`) — records with neither
        // `task_id` nor `scenario_id` are simply not a coverage
        // subject and never enter a group.
        let mut groups: BTreeMap<CellSubject, Vec<&EvidenceRecord>> = BTreeMap::new();
        for record in &ctx.evidence {
            if let Some(subject) = CellSubject::of(record) {
                groups.entry(subject).or_default().push(record);
            }
        }

        let mut violations = Vec::new();
        for (subject, records) in &groups {
            for role in ctx.policy.risk_routing.keys() {
                if !rule_applies_to_any(role, records, ctx) {
                    // Policy declares this cell irrelevant to this
                    // artifact (or the predicate itself failed to
                    // evaluate against every one of its records) — not
                    // required, not a violation.
                    continue;
                }
                if !has_matching_role_record(role, records) {
                    violations.push(Violation::new(
                        FailureClass::UncoveredCell,
                        subject.as_str(),
                        format!("policy-required cell '{role}' has no matching-role evidence record"),
                    ));
                }
            }
        }
        violations
    }
}

/// Does `ctx.policy`'s `role` routing rule apply to this artifact —
/// proven by evaluating it against ANY one of the artifact's OWN
/// already-submitted records (module doc: a rule can only be
/// evaluated against a record that exists; there is no synthetic
/// "would-be" record to test an absent cell against). A record that
/// fails to deserialize into JSON, or a rule whose CEL evaluation
/// itself errors, is treated as "does not prove the rule applies" —
/// never a check-crashing `Err` (design §7, the same fail-soft-per-record
/// discipline [`crate::context::GateCheck::run`]'s own doc mandates).
fn rule_applies_to_any(role: &str, records: &[&EvidenceRecord], ctx: &GateContext) -> bool {
    records.iter().any(|record| {
        let Ok(json) = serde_json::to_value(record) else {
            return false;
        };
        matches!(ctx.policy.risk_routing_for(role, &json, record.envelope.at), Ok(Some(true)))
    })
}

/// Whether any of this artifact's records was authored by an actor
/// whose `role` string matches `role` exactly (`policy.yaml`'s
/// `risk_routing` key IS the required role name — module doc).
fn has_matching_role_record(role: &str, records: &[&EvidenceRecord]) -> bool {
    records.iter().any(|record| record.envelope.actor.role.as_ref().is_some_and(|actor_role| actor_role.as_str() == role))
}

#[cfg(test)]
mod tests {
    use canon_model::{Actor, Envelope, EvidenceVerdict, RecordKind, RoleId, TaskId};
    use chrono::Utc;

    use super::*;
    use crate::context::GateCtx;
    use crate::policy::{PolicyField, PolicyResolution, StalenessPolicy};

    fn record(task: &str, role: &str, verdict: EvidenceVerdict) -> EvidenceRecord {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("agent", RoleId::parse(role).unwrap()));
        EvidenceRecord::new(envelope, Some(TaskId::parse(task).unwrap()), None, None, verdict)
    }

    fn policy_with_required_roles(roles: &[&str]) -> PolicyResolution {
        let risk_routing = roles.iter().map(|r| (r.to_string(), PolicyField::Flat(true))).collect();
        PolicyResolution {
            trust_required: BTreeMap::new(),
            trust_sample: BTreeMap::new(),
            staleness: StalenessPolicy { max_commits_behind: PolicyField::Flat(50), surface_scoped: PolicyField::Flat(true) },
            risk_routing,
            diagnostics: Vec::new(),
        }
    }

    fn ctx_with(policy: PolicyResolution, evidence: Vec<EvidenceRecord>) -> GateContext {
        GateContext { ctx: GateCtx { repo: "/tmp/repo".into(), ledger_root: "/tmp/repo/.canon/ledger".into() }, policy, evidence, violations: Vec::new(), now: Utc::now() }
    }

    #[test]
    fn uncovered_cell_fires_when_a_required_role_has_no_matching_record() {
        let policy = policy_with_required_roles(&["reviewer"]);
        // implementer evidence exists, but policy also requires a
        // "reviewer"-role record for the same task — none exists.
        let evidence = vec![record("s5-trust-spine-gate#1.2", "implementer", EvidenceVerdict::Faithful)];
        let ctx = ctx_with(policy, evidence);

        let violations = CoverageCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::UncoveredCell);
        assert_eq!(violations[0].subject, "s5-trust-spine-gate#1.2");
        assert!(violations[0].detail.contains("reviewer"));
    }

    #[test]
    fn a_matching_role_record_satisfies_coverage_regardless_of_verdict() {
        let policy = policy_with_required_roles(&["implementer"]);
        // Divergent (a FAILING verdict) still satisfies COVERAGE —
        // coverage is "a test exists", never "a test passed" (module
        // doc; design decision 1).
        let evidence = vec![record("s5-trust-spine-gate#1.2", "implementer", EvidenceVerdict::Divergent)];
        let ctx = ctx_with(policy, evidence);

        assert!(CoverageCheck.run(&ctx).is_empty());
    }

    #[test]
    fn no_risk_routing_rules_means_zero_required_cells_fail_soft_default() {
        let policy = policy_with_required_roles(&[]);
        let evidence = vec![record("s5-trust-spine-gate#1.2", "implementer", EvidenceVerdict::Faithful)];
        let ctx = ctx_with(policy, evidence);

        assert!(CoverageCheck.run(&ctx).is_empty());
    }

    #[test]
    fn a_policy_diff_alone_tightens_coverage_with_no_artifact_edits() {
        // spec.md "Policy change alone tightens coverage": the SAME
        // evidence corpus, only the policy's risk_routing set grows.
        let evidence = vec![record("s5-trust-spine-gate#1.2", "implementer", EvidenceVerdict::Faithful)];

        let before = ctx_with(policy_with_required_roles(&["implementer"]), evidence.clone());
        assert!(CoverageCheck.run(&before).is_empty(), "implementer-only requirement is already satisfied");

        let after = ctx_with(policy_with_required_roles(&["implementer", "reviewer"]), evidence);
        let violations = CoverageCheck.run(&after);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].subject, "s5-trust-spine-gate#1.2");
    }

    #[test]
    fn distinct_task_ids_are_checked_independently() {
        let policy = policy_with_required_roles(&["reviewer"]);
        let evidence = vec![
            record("s5-trust-spine-gate#1.2", "implementer", EvidenceVerdict::Faithful),
            record("s5-trust-spine-gate#1.3", "reviewer", EvidenceVerdict::Faithful),
        ];
        let ctx = ctx_with(policy, evidence);

        let violations = CoverageCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].subject, "s5-trust-spine-gate#1.2");
    }

    #[test]
    fn a_record_with_no_join_key_is_excluded_from_coverage_entirely() {
        let policy = policy_with_required_roles(&["reviewer"]);
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("agent", RoleId::parse("implementer").unwrap()));
        let evidence = vec![EvidenceRecord::new(envelope, None, None, None, EvidenceVerdict::Faithful)];
        let ctx = ctx_with(policy, evidence);

        assert!(CoverageCheck.run(&ctx).is_empty(), "a record with neither task_id nor scenario_id has no coverage subject to check");
    }
}
