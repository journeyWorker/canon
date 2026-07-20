//! The dynamic verdict-ledger check (design decision 1/D3b): given a
//! cell that exists (`crate::coverage`'s concern), did the LATEST
//! matching evidence pass, and by whom. Generalizes `tools/parity.py`'s
//! `_review_index`/`_conformance_index`/`_clear_index` fold-to-latest
//! index builders (the donor parity-harness audit's ledger-reader notes
//! §"Index builders": "each takes the flat
//! `list[dict]` `_load_ledger` returns and folds it into a
//! `dict[scenario_id, record]` ..., with explicit last-wins-by-`at`
//! semantics") over this crate's single homogeneous `EvidenceRecord`
//! corpus — an evidence record's own [`canon_model::envelope::Actor::role`]
//! stands in for parity.py's per-KIND ledger directory (`review` vs
//! `code-review` vs `clear`), since canon-gate reads one record family,
//! not several kind-partitioned ones.
//!
//! # Two outputs, on purpose (design decision 1)
//! "'A test exists' and 'a test passed' are different facts with
//! different staleness windows" (design decision 1) — this module
//! produces BOTH, through two DIFFERENT surfaces:
//! - [`latest_verdicts`]: the fold-to-latest-per-cell READ MODEL (pass/
//!   fail/by-whom, spec.md "Covered cell with a failing verdict is not
//!   green" — "the failure is visible as its own fact"). This is
//!   informational, not a [`crate::GateCheck`] — [`FAILURE_CLASSES`]
//!   (design decision 9) is a CLOSED eight-string set with no member
//!   named for "the latest verdict was `Divergent`"; a failing verdict
//!   is a REPORTED fact for `canon gate check`/`report` (task 1.9,
//!   `canon gate promote`'s re-validation, task 2.2) to render, never a
//!   gate-blocking violation this closed vocabulary was never given a
//!   string for. Inventing a ninth string here would violate design
//!   decision 9's own frozen-contract discipline mid-wave-2, outside
//!   any foundation-review migration.
//! - [`LedgerCheck`]: the ACTUAL [`crate::GateCheck`] this module
//!   contributes — `malformed-evidence` for every already-collected
//!   [`canon_model::EvidenceViolation`] on [`crate::GateContext`]. §7's
//!   "malformed evidence is no evidence" content/layout validation
//!   already happened one layer down (`canon_model::validate_evidence_batch`
//!   inside `canon-store`'s `GitTier::read`, per `context.rs`'s own
//!   `GateContext::load` — ledger-reader.md §3.2's soft-skip-reader /
//!   fail-loud-twin split: `GateContext.evidence` IS the soft-skip
//!   half, `GateContext.violations` IS the fail-loud twin's collected
//!   output); this check's entire job is surfacing that ALREADY-fail-
//!   loud collection as this crate's own `Violation` type, never
//!   re-validating content itself.
//!
//! [`FAILURE_CLASSES`]: crate::FAILURE_CLASSES

use std::collections::BTreeMap;

use canon_model::EvidenceVerdict;
use canon_store::fold_latest_by_key;
use chrono::{DateTime, Utc};

use crate::context::{GateCheck, GateContext};
use crate::coverage::CellSubject;
use crate::failure_class::{FailureClass, Violation};

/// One (subject, role) cell's key in the fold — `subject` is
/// [`CellSubject::as_str`]'s output (a `task_id`/`scenario_id`
/// string); `role` is the authoring actor's role, `None` for a
/// legacy/unattributed record (S11 design D5's own "absence stays
/// absent" discipline, `envelope.rs`'s `Actor::role` doc).
pub type CellKey = (String, Option<String>);

/// The LATEST evidence record for one (subject, role) cell —
/// "pass/fail + by-whom" (this crate's own wave-2 acceptance
/// language), parity.py's index-builder shape generalized (module
/// doc).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    pub subject: String,
    pub role: Option<String>,
    pub verdict: EvidenceVerdict,
    /// "by whom" — the authoring actor's `agent_id`.
    pub agent_id: String,
    pub at: DateTime<Utc>,
}

impl LedgerEntry {
    /// Whether this cell's latest verdict is passing — [`crate::staleness`]
    /// reads this before deciding whether a green cell has since
    /// degraded stale (staleness only ever demotes an ALREADY-green
    /// verdict, spec.md "Staleness detection": "degrade a PASSING
    /// evidence record to stale").
    pub fn is_green(&self) -> bool {
        matches!(self.verdict, EvidenceVerdict::Faithful)
    }
}

/// Fold `ctx.evidence` to the latest matching record per (subject,
/// role) cell (module doc) — a thin caller of the hoisted
/// [`canon_store::fold_latest_by_key`] (design D11/s21 D3): winner
/// per cell is the greatest `(envelope.at, content_digest)` pair — a
/// total, machine-independent order, never corpus/iteration order.
/// Records with neither `task_id` nor `scenario_id` carry no cell
/// identity ([`CellSubject::of`]) and are excluded, mirroring
/// [`crate::coverage::CoverageCheck`]'s identical interface-gap
/// treatment (one shared `CellSubject`, module doc).
pub fn latest_verdicts(ctx: &GateContext) -> BTreeMap<CellKey, LedgerEntry> {
    struct Candidate {
        entry: LedgerEntry,
        digest: String,
    }
    let candidates = ctx.evidence.iter().filter_map(|record| {
        let subject = CellSubject::of(record)?;
        let role = record.envelope.actor.role.as_ref().map(|r| r.as_str().to_string());
        let entry = LedgerEntry { subject: subject.as_str().to_string(), role, verdict: record.verdict, agent_id: record.envelope.actor.agent_id.clone(), at: record.envelope.at };
        let digest = canon_store::partition::content_digest12(&serde_json::to_value(record).unwrap_or_default());
        Some(Candidate { entry, digest })
    });
    fold_latest_by_key(candidates, |c| (c.entry.subject.clone(), c.entry.role.clone()), |c| c.entry.at, |c| c.digest.as_str())
        .into_iter()
        .map(|(key, c)| (key, c.entry))
        .collect()
}

/// The dynamic verdict-ledger [`crate::GateCheck`] (D3b) — surfaces
/// every already-collected [`canon_model::EvidenceViolation`] as
/// `malformed-evidence` (module doc). Never re-derives pass/fail —
/// [`latest_verdicts`] is the separate, non-`GateCheck` surface for
/// that (module doc).
pub struct LedgerCheck;

impl GateCheck for LedgerCheck {
    fn name(&self) -> &'static str {
        "ledger"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        ctx.violations
            .iter()
            .map(|violation| Violation::new(FailureClass::MalformedEvidence, violation.subject.clone(), format!("{} ({})", violation.detail, violation.class.as_str())))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use canon_model::{Actor, Envelope, EvidenceRecord, EvidenceViolation, RecordKind, RoleId, TaskId};
    use canon_model::evidence::FailureClass as EvidenceFailureClass;
    use chrono::{Duration, Utc};

    use super::*;
    use crate::context::GateCtx;
    use crate::policy::{PolicyField, PolicyResolution, StalenessPolicy};

    fn record_at(task: &str, role: &str, agent: &str, verdict: EvidenceVerdict, at: DateTime<Utc>) -> EvidenceRecord {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, at, Actor::new(agent, RoleId::parse(role).unwrap()));
        EvidenceRecord::new(envelope, Some(TaskId::parse(task).unwrap()), None, None, verdict)
    }

    fn empty_policy() -> PolicyResolution {
        PolicyResolution {
            trust_required: BTreeMap::new(),
            trust_sample: BTreeMap::new(),
            staleness: StalenessPolicy { max_commits_behind: PolicyField::Flat(50), surface_scoped: PolicyField::Flat(true) },
            risk_routing: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }

    fn ctx_with(evidence: Vec<EvidenceRecord>, violations: Vec<EvidenceViolation>) -> GateContext {
        GateContext { ctx: GateCtx { repo: "/tmp/repo".into(), ledger_root: "/tmp/repo/canon/ledger".into() }, policy: empty_policy(), evidence, violations, now: Utc::now() }
    }

    #[test]
    fn ledger_check_surfaces_every_evidence_violation_as_malformed_evidence() {
        let violations = vec![EvidenceViolation::new(EvidenceFailureClass::Malformed, "kind=evidence-record/bad.json", "missing field `at`")];
        let ctx = ctx_with(Vec::new(), violations);

        let out = LedgerCheck.run(&ctx);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].class, FailureClass::MalformedEvidence);
        assert_eq!(out[0].subject, "kind=evidence-record/bad.json");
        assert!(out[0].detail.contains("missing field `at`"));
    }

    #[test]
    fn ledger_check_is_silent_on_a_failing_verdict_alone() {
        // A Divergent verdict is a REPORTED fact via `latest_verdicts`,
        // never a `GateCheck` violation (module doc — no FAILURE_CLASSES
        // member exists for it).
        let now = Utc::now();
        let ctx = ctx_with(vec![record_at("s5-trust-spine-gate#1.3", "implementer", "agent-a", EvidenceVerdict::Divergent, now)], Vec::new());

        assert!(LedgerCheck.run(&ctx).is_empty());
    }

    #[test]
    fn latest_verdicts_last_wins_by_at_over_a_stale_earlier_record() {
        let earlier = Utc::now() - Duration::days(2);
        let later = Utc::now();
        let evidence = vec![
            record_at("s5-trust-spine-gate#1.3", "implementer", "agent-a", EvidenceVerdict::Divergent, earlier),
            record_at("s5-trust-spine-gate#1.3", "implementer", "agent-b", EvidenceVerdict::Faithful, later),
        ];
        let ctx = ctx_with(evidence, Vec::new());

        let verdicts = latest_verdicts(&ctx);
        assert_eq!(verdicts.len(), 1);
        let entry = verdicts.get(&("s5-trust-spine-gate#1.3".to_string(), Some("implementer".to_string()))).unwrap();
        assert!(entry.is_green());
        assert_eq!(entry.agent_id, "agent-b");
        assert_eq!(entry.at, later);
    }

    #[test]
    fn latest_verdicts_tracks_distinct_roles_for_the_same_subject_independently() {
        let now = Utc::now();
        let evidence = vec![
            record_at("s5-trust-spine-gate#1.3", "implementer", "agent-a", EvidenceVerdict::Faithful, now),
            record_at("s5-trust-spine-gate#1.3", "reviewer", "agent-b", EvidenceVerdict::Divergent, now),
        ];
        let ctx = ctx_with(evidence, Vec::new());

        let verdicts = latest_verdicts(&ctx);
        assert_eq!(verdicts.len(), 2);
        assert!(verdicts.get(&("s5-trust-spine-gate#1.3".to_string(), Some("implementer".to_string()))).unwrap().is_green());
        assert!(!verdicts.get(&("s5-trust-spine-gate#1.3".to_string(), Some("reviewer".to_string()))).unwrap().is_green());
    }

    #[test]
    fn a_record_with_no_join_key_is_excluded_from_the_verdict_fold() {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("agent-a", RoleId::parse("implementer").unwrap()));
        let evidence = vec![EvidenceRecord::new(envelope, None, None, None, EvidenceVerdict::Faithful)];
        let ctx = ctx_with(evidence, Vec::new());

        assert!(latest_verdicts(&ctx).is_empty());
    }
}
