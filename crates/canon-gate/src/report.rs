//! The shared violation/report type every S5 wave-2 [`crate::GateCheck`]
//! (coverage/D3a, verdict-ledger/D3b, staleness/D4, trust-ladder
//! promotion, the flag ratchet, checkbox-grammar's evidence gate)
//! accumulates into — the aggregation layer over
//! [`crate::failure_class::Violation`], mirroring `tools/parity.py`'s
//! own `run_static_gate`/`cmd_coverage`/`cmd_validate` glue
//! (the donor parity-harness audit's static-gate notes §3.1/§3.5):
//! one flat `list[Violation]` per gate run, one exit-code contract.
//!
//! # Reuses the frozen foundation `Violation`, never a parallel type
//! [`crate::context::GateCheck::run`] is ALREADY typed
//! `fn run(&self, ctx: &GateContext) -> Vec<Violation>` (`context.rs`,
//! landed in the FOUNDATION commit) — `Violation{class: FailureClass,
//! subject, detail}` is the direct, donor-exact port of
//! `tools/parity.py::Violation(cls, subject, detail)` +
//! its `.line()` wire-format renderer (static-gate.md §3.1). This
//! module does NOT define a second, string-typed violation struct
//! alongside it — a second convention beside an existing one is
//! exactly the drift `FAILURE_CLASSES` (design decision 9) exists to
//! prevent. [`GateViolation`]/[`GateFailureClass`] are plain type
//! aliases onto the SAME frozen types, spelled with this crate's own
//! `Gate`-prefixed naming ([`crate::GateCtx`]/[`crate::GateContext`]/
//! [`crate::GateCheck`]) purely so call sites that want the
//! crate-flavored name can use it; `GateViolation::new(class, subject,
//! detail)` and `Violation::new(...)` are the identical call.
//!
//! # `GateReport`: the aggregator
//! [`GateReport`] is the flat accumulator every [`crate::GateCheck`]'s
//! output folds into (task 1.9's `canon gate check` dispatcher, not
//! implemented in this wave, will be its only production caller) —
//! `is_clean()`/`exit_code()` implement parity.py's own two-way
//! exit-code half of its three-way contract (module docstring: "0
//! green / 1 gate-red / 2 usage-or-missing-dep", static-gate.md §3.5);
//! the "2" (CLI usage/dependency failure) case is `canon gate`'s own
//! concern, never a `GateReport` state — a report only ever describes
//! violations found during a completed run.

use crate::failure_class::{FailureClass, Violation};

/// The SAME frozen [`Violation`] type ([`crate::context::GateCheck`]'s
/// own return-type element), spelled with this crate's `Gate`-prefixed
/// naming convention (module doc) — never a second struct.
pub type GateViolation = Violation;

/// The SAME frozen [`FailureClass`] enum, `Gate`-prefixed alias (module
/// doc).
pub type GateFailureClass = FailureClass;

/// Every violation one gate run collected, across every registered
/// [`crate::GateCheck`] — the flat accumulator `tools/parity.py`'s own
/// `run_static_gate` builds before `cmd_coverage`/`cmd_validate` render
/// it (static-gate.md §3.1's `_static_violations` dispatcher: "one
/// function running ... independent check-loops ..., accumulating into
/// a flat `list[Violation]`").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateReport {
    pub violations: Vec<Violation>,
}

impl GateReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Wraps an already-collected violation set — the shape a
    /// dispatcher folding several [`crate::GateCheck::run`] calls
    /// together produces.
    pub fn from_violations(violations: Vec<Violation>) -> Self {
        Self { violations }
    }

    pub fn push(&mut self, violation: Violation) {
        self.violations.push(violation);
    }

    pub fn extend(&mut self, violations: impl IntoIterator<Item = Violation>) {
        self.violations.extend(violations);
    }

    /// The gate fails loud (design decision 9): a `GateReport` is
    /// clean IFF it carries zero violations — never "mostly clean" or
    /// a severity-weighted threshold. §7's "fail loud" discipline
    /// means every violation this crate records blocks green,
    /// unconditionally.
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Every violation carrying failure class `class` — the per-class
    /// slice a `canon gate check` summary line (`"N uncovered-cell,
    /// M stale-evidence, ..."`) or a fixture-corpus selftest's
    /// exact-set-match oracle (`Violation::pair()`, `fixtures-
    /// selftest.md` §3.2) both need.
    pub fn by_class(&self, class: FailureClass) -> impl Iterator<Item = &Violation> {
        self.violations.iter().filter(move |v| v.class == class)
    }

    /// The grep-stable wire lines (`Violation::line()`, static-gate.md
    /// §3.1) every violation renders to — what a CLI's stdout or a
    /// fixture's `expected_failures.txt` diff sees.
    pub fn lines(&self) -> Vec<String> {
        self.violations.iter().map(Violation::line).collect()
    }

    /// parity.py's own two-way half of its three-way exit-code
    /// contract (module doc): `0` on a clean report, `1` when any
    /// violation was found (`"gate-red"`) — the gate fails loud, never
    /// a silent pass on a non-empty violation set.
    pub fn exit_code(&self) -> i32 {
        if self.is_clean() {
            0
        } else {
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violation(class: FailureClass) -> Violation {
        Violation::new(class, "subject", "detail")
    }

    #[test]
    fn new_report_is_clean_with_zero_exit_code() {
        let report = GateReport::new();
        assert!(report.is_clean());
        assert_eq!(report.exit_code(), 0);
        assert!(report.lines().is_empty());
    }

    #[test]
    fn pushing_a_violation_makes_the_report_dirty_with_nonzero_exit_code() {
        let mut report = GateReport::new();
        report.push(violation(FailureClass::UncoveredCell));
        assert!(!report.is_clean());
        assert_eq!(report.exit_code(), 1);
        assert_eq!(report.lines(), vec!["uncovered-cell subject — detail".to_string()]);
    }

    #[test]
    fn extend_folds_every_checks_violations_into_one_flat_report() {
        let mut report = GateReport::new();
        report.extend(vec![violation(FailureClass::UncoveredCell), violation(FailureClass::StaleEvidence)]);
        report.extend(vec![violation(FailureClass::MalformedEvidence)]);
        assert_eq!(report.violations.len(), 3);
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn by_class_filters_to_exactly_the_matching_violations() {
        let report = GateReport::from_violations(vec![
            violation(FailureClass::UncoveredCell),
            violation(FailureClass::StaleEvidence),
            violation(FailureClass::UncoveredCell),
        ]);
        assert_eq!(report.by_class(FailureClass::UncoveredCell).count(), 2);
        assert_eq!(report.by_class(FailureClass::Flagged).count(), 0);
    }

    #[test]
    fn gate_violation_alias_constructs_identically_to_violation() {
        let via_alias = GateViolation::new(GateFailureClass::UnreviewedPromotion, "s", "d");
        let via_direct = Violation::new(FailureClass::UnreviewedPromotion, "s", "d");
        assert_eq!(via_alias, via_direct);
    }
}
