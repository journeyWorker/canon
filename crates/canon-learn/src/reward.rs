//! The per-role [`RewardFn`] registry (design D1, task 1.1/1.2) —
//! generalizes the donor's `computeDevReward` weighted
//! composite (the donor's dev-reward backfill) into a role-keyed table,
//! a clean-room reimplementation of the formula SHAPE only
//! (design Non-Goals: the donor's
//! TS is never forked or wrapped).
//!
//! **The granularity gap this module documents and resolves**: the donor's
//! `DevSignalEvent` carries six distinct KINDS (`pr-merged`/`ci-pass`/
//! `no-rollback`/`rollback`/`ci-fail`/`human-approval`) accumulated
//! per-trajectory over time. S4's stabilized [`VerdictRow`] (this
//! crate's ONLY verdict input, design D1's frozen boundary) carries
//! just `{role, polarity, becomes}` — S4's own review→verdict table
//! (`openspec/changes/s4-artifact-ingest/specs/review-verdict-mapping/
//! spec.md`) already COLLAPSES `PrMergeNoRevert`/`RemediationResolved`/
//! `ReviewPromotion` into one identical `(Success, StrategyCandidate)`
//! shape for `dev`, and `CodeReviewFinding`/`CiFailOrPrRevert` into one
//! identical `(Failure, GuardrailCandidate)` shape — there is no field
//! left to separately recover "PR merged" vs "CI passed" vs "no
//! rollback yet" from a bare `VerdictRow`. [`dev_signals_from_verdicts`]
//! is the documented, best-faith adapter closing that gap: S4 only ever
//! emits the `(Success, StrategyCandidate)` shape once a `dev` outcome
//! has FULLY resolved favorably, so canon reads it as the complete
//! positive triad rather than a single partial signal. The pure
//! [`compute_dev_reward`] formula itself carries FULL fidelity to
//! `computeDevReward`'s weights/floors/shortcut and is independently
//! testable on synthetic [`DevRewardSignals`], exactly mirroring
//! MaTTS's "pure statistics core, testable on synthetic arrays"
//! split (per the donor's MaTTS statistical-promotion audit)
//! applied to reward math instead of significance
//! math.

use std::collections::BTreeMap;

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_model::ids::RoleId;

use crate::trajectory::Trajectory;
use crate::verdict_outcome::VerdictOutcome;

/// `pr-merged`'s weight in `computeDevReward`'s additive positive triad
/// (the donor's dev-reward backfill).
pub const PR_MERGED_WEIGHT: f64 = 0.4;
/// `ci-pass`'s weight.
pub const CI_PASS_WEIGHT: f64 = 0.3;
/// `no-rollback`'s weight — the triad sums to exactly `1.0`.
pub const NO_ROLLBACK_WEIGHT: f64 = 0.3;
/// The hard floor a `rollback` or `ci-fail` signal overrides every
/// positive signal down to, regardless of what else fired
/// (the donor's dev-reward backfill override-rule precedence).
pub const FAILURE_FLOOR: f64 = 0.1;
/// `human-approval`'s shortcut reward — "canonical yes path bypassing
/// the triad" (the donor's dev-reward backfill).
pub const HUMAN_APPROVAL_REWARD: f64 = 1.0;

/// The `dev` role's raw signal set — the literal clean-room port of
/// `DevSignalEvent`'s six kinds (the donor's dev-reward backfill), deliberately
/// KEPT as its own richly-typed struct (not collapsed into
/// `VerdictRow`) so [`compute_dev_reward`] stays a pure, synthetic-
/// fixture-testable formula independent of S4's coarser wire shape —
/// exactly the input granularity a FUTURE webhook receiver (task group
/// 5, which sees raw GitHub `pull_request.merged`/`workflow_run.
/// conclusion` events BEFORE they collapse into a `VerdictRow`) can
/// populate directly, without going through [`dev_signals_from_verdicts`]
/// at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DevRewardSignals {
    pub pr_merged: bool,
    pub ci_pass: bool,
    pub no_rollback: bool,
    pub rollback: bool,
    pub ci_fail: bool,
    pub human_approval: bool,
}

/// The pure weighted-composite formula (design D1/task 1.1), ported
/// verbatim from `computeDevReward`'s rule ORDER: `rollback` overrides
/// everything, then `ci-fail`, then the `human-approval` shortcut, then
/// the additive positive triad (`pr-merged` 0.4 + `ci-pass` 0.3 +
/// `no-rollback` 0.3). The triad path never reaches `Success` below a
/// full `1.0` — a partial triad (e.g. `pr-merged` + `ci-pass` only,
/// `0.7`) stays `Pending` at its accumulated reward, matching
/// the donor's own `reward >= 1.0 ? "success" :
/// "pending"` check exactly (and the documented real-world consequence:
/// the donor's own production gap means this path can structurally never
/// reach `1.0` without a `no-rollback` timer — a fixture CAN and does
/// exercise it here, since this formula is pure math, not the donor's
/// half-wired production path).
pub fn compute_dev_reward(signals: DevRewardSignals) -> (VerdictOutcome, f64) {
    if signals.rollback {
        return (VerdictOutcome::RolledBack, FAILURE_FLOOR);
    }
    if signals.ci_fail {
        return (VerdictOutcome::Failure, FAILURE_FLOOR);
    }
    if signals.human_approval {
        return (VerdictOutcome::Success, HUMAN_APPROVAL_REWARD);
    }
    let mut reward = 0.0;
    if signals.pr_merged {
        reward += PR_MERGED_WEIGHT;
    }
    if signals.ci_pass {
        reward += CI_PASS_WEIGHT;
    }
    if signals.no_rollback {
        reward += NO_ROLLBACK_WEIGHT;
    }
    let reward = crate::verdict_outcome::clamp_reward(reward);
    let outcome = if reward >= 1.0 { VerdictOutcome::Success } else { VerdictOutcome::Pending };
    (outcome, reward)
}

/// The documented VerdictRow -> DevRewardSignals adapter (module doc's
/// "granularity gap" section). Reads only the LAST row in a
/// trajectory's accumulated `verdicts` list — `Trajectory.verdicts`'
/// own doc names the canonical worked example ("a code-review finding
/// followed by its later remediation, both folded onto the same
/// regime"): an EARLIER `Failure` that was later remediated must not
/// floor the reward, so the most-recently-appended row is read as the
/// trajectory's current resolved state (S4's own `Verdict.trust_level`
/// doc: "S6/S7's statistical promotion is where trust-weighting
/// actually happens" — this adapter IS that step).
pub fn dev_signals_from_verdicts(verdicts: &[VerdictRow]) -> DevRewardSignals {
    let mut signals = DevRewardSignals::default();
    match verdicts.last() {
        Some(VerdictRow { polarity: Polarity::Success, becomes: Becomes::StrategyCandidate, .. }) => {
            // S4 collapses pr-merged/ci-pass/no-rollback (and the
            // human-approval-shortcut-shaped `ReviewPromotion`) into
            // this ONE shape once fully resolved favorably; a bare
            // `VerdictRow` carries no finer signal, so canon awards the
            // full positive triad here (documented divergence from
            // the donor's richer per-event granularity, see module doc).
            signals.pr_merged = true;
            signals.ci_pass = true;
            signals.no_rollback = true;
        }
        Some(VerdictRow { polarity: Polarity::Failure, becomes: Becomes::GuardrailCandidate, .. }) => {
            // Both `CodeReviewFinding` and `CiFailOrPrRevert` collapse
            // here; canon cannot distinguish "PR never merged" from
            // "merged then reverted" — the ci-fail floor is the
            // conservative choice for either.
            signals.ci_fail = true;
        }
        _ => {} // `Corrective`/other combos never occur for `dev` per
        // S4's own table; stays the zero signal set (Pending).
    }
    signals
}

/// The `dev` role's registered [`RewardFn`] (task 1.1).
pub fn dev_reward_fn(verdicts: &[VerdictRow]) -> (VerdictOutcome, f64) {
    compute_dev_reward(dev_signals_from_verdicts(verdicts))
}

/// The provisional, table-driven reward function for roles with no
/// donor weight set (task 1.2: `content`/`design`/`review`/
/// `planning`/`test` — "drafted at implementation time from S4's
/// verdict table, not fixed in this design", design Open Questions).
/// Unlike [`dev_reward_fn`], these roles have no per-event weighted
/// triad to port — S4's review→verdict table gives each of them AT
/// MOST one dedicated row, so this function applies the DEFAULT reward
/// convention (design D2: 0.9/0.3/0.5) uniformly per `Polarity`, with
/// `Corrective` read as a SUCCESS: the one role the table gives a
/// `Corrective` row to (`review`'s own "clear-record after @flagged",
/// design D1's worked example) is being rewarded for the review
/// PROCESS catching, then clearing, a flagged sample — the review
/// role's own definition of success. This is a genuinely provisional
/// stand-in, not a fixed formula; a future change may replace any of
/// these with a role-specific weighted composite once real cross-role
/// data exists (design Open Questions).
pub fn default_reward_fn(verdicts: &[VerdictRow]) -> (VerdictOutcome, f64) {
    match verdicts.last().map(|v| v.polarity) {
        Some(Polarity::Success) => (VerdictOutcome::Success, VerdictOutcome::Success.default_reward()),
        Some(Polarity::Failure) => (VerdictOutcome::Failure, VerdictOutcome::Failure.default_reward()),
        Some(Polarity::Corrective) => (VerdictOutcome::Success, VerdictOutcome::Success.default_reward()),
        None => (VerdictOutcome::Pending, VerdictOutcome::Pending.default_reward()),
    }
}

/// `RewardFn: Role -> &[VerdictRow] -> (VerdictOutcome, f64)` (design
/// D1's `RewardFn: Role -> VerdictEvent -> f64`, widened to also return
/// the resulting [`VerdictOutcome`] — `mark_trajectory_verdict` needs
/// BOTH, and `computeDevReward` itself already returns `{reward,
/// verdict}` as one pair, per the donor's dev-reward backfill). A plain
/// function pointer, not a boxed closure — every registered entry is a
/// static, stateless formula (mirrors MaTTS's pure-function
/// core), so no allocation is needed to hold one.
pub type RewardFn = fn(&[VerdictRow]) -> (VerdictOutcome, f64);

fn role(slug: &'static str) -> RoleId {
    RoleId::parse(slug).unwrap_or_else(|e| panic!("built-in role slug {slug:?} must be a valid RoleId: {e}"))
}

/// The per-role reward function registry (task 1.1/1.2) — one entry
/// per built-in role this task covers (`dev` + the five task-1.2
/// provisional roles). A role with no registered entry (e.g. `sim`,
/// whose own donor is the DIFFERENT multiplicative reward
/// `reward-computation.md` surface, out of this task's scope, or a
/// consumer repo's own `canon.yaml`-registered custom role) falls back
/// to [`default_reward_fn`] — every role always has SOME reward
/// function, never a missing-entry error, matching design D2's "one
/// comparable scale across roles" requirement.
#[derive(Debug, Clone)]
pub struct RewardRegistry {
    fns: BTreeMap<RoleId, RewardFn>,
}

impl RewardRegistry {
    /// `dev` (task 1.1) plus the five task-1.2 provisional roles.
    pub fn builtin() -> Self {
        let mut fns: BTreeMap<RoleId, RewardFn> = BTreeMap::new();
        fns.insert(role("dev"), dev_reward_fn);
        // Provisional (task 1.2), each drafted from S4's review->verdict
        // table, not a donor formula:
        // - `content`: only the generic "review-record promotion" row
        //   applies (no dedicated content-review-finding row exists yet).
        // - `design`: has BOTH a dedicated negative row ("design-review
        //   finding") and the generic positive promotion row — the
        //   closest any provisional role gets to `dev`'s own granularity.
        // - `review`: its OWN reward source is the corrective
        //   "clear-record after @flagged" row (design D1's own worked
        //   example) — `Corrective` polarity IS `review`'s positive
        //   signal; `review` never appears as `role` on a `Failure` row
        //   in the current table.
        // - `planning`/`test`: same shape as `content` — provisional;
        //   `test`'s eventual real reward source is likely the CI/
        //   test-ledger surface the design doc names ("gate results,
        //   test ledger"), not yet wired to a dedicated S4 table row.
        fns.insert(role("content"), default_reward_fn);
        fns.insert(role("design"), default_reward_fn);
        fns.insert(role("review"), default_reward_fn);
        fns.insert(role("planning"), default_reward_fn);
        fns.insert(role("test"), default_reward_fn);
        Self { fns }
    }

    /// This role's registered reward function, or [`default_reward_fn`]
    /// when none is registered — never a missing-entry error.
    pub fn get(&self, role: &RoleId) -> RewardFn {
        self.fns.get(role).copied().unwrap_or(default_reward_fn)
    }

    pub fn compute(&self, role: &RoleId, verdicts: &[VerdictRow]) -> (VerdictOutcome, f64) {
        (self.get(role))(verdicts)
    }

    /// Convenience: resolves `trajectory`'s own role and computes its
    /// reward from its accumulated `verdicts` in one call — the shape
    /// [`crate::mark_verdict::mark_trajectory_verdict`]'s caller
    /// typically wants.
    pub fn compute_for_trajectory(&self, trajectory: &Trajectory) -> Result<(VerdictOutcome, f64), crate::error::LearnError> {
        let role = trajectory.role()?;
        Ok(self.compute(&role, &trajectory.verdicts))
    }
}

impl Default for RewardRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

#[cfg(test)]
mod tests {
    use canon_model::ids::RegimeKey;
    use chrono::Utc;

    use super::*;
    use crate::ids::TrajectoryId;

    fn regime(role: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(role, "repo", "auth", "abc123")).unwrap()
    }

    // ---- pure formula tests (synthetic DevRewardSignals, no VerdictRow) ----

    #[test]
    fn full_positive_triad_is_success_at_reward_one() {
        let signals = DevRewardSignals { pr_merged: true, ci_pass: true, no_rollback: true, ..Default::default() };
        assert_eq!(compute_dev_reward(signals), (VerdictOutcome::Success, 1.0));
    }

    #[test]
    fn partial_triad_stays_pending_at_its_accumulated_reward() {
        let signals = DevRewardSignals { pr_merged: true, ci_pass: true, ..Default::default() };
        let (outcome, reward) = compute_dev_reward(signals);
        assert_eq!(outcome, VerdictOutcome::Pending);
        assert!((reward - 0.7).abs() < f64::EPSILON, "expected 0.4+0.3=0.7, got {reward}");
    }

    #[test]
    fn no_signals_at_all_is_pending_at_zero() {
        assert_eq!(compute_dev_reward(DevRewardSignals::default()), (VerdictOutcome::Pending, 0.0));
    }

    #[test]
    fn rollback_floors_to_zero_point_one_regardless_of_other_positives() {
        let signals = DevRewardSignals { pr_merged: true, ci_pass: true, no_rollback: true, rollback: true, ..Default::default() };
        assert_eq!(compute_dev_reward(signals), (VerdictOutcome::RolledBack, FAILURE_FLOOR));
    }

    #[test]
    fn ci_fail_floors_to_zero_point_one() {
        let signals = DevRewardSignals { ci_fail: true, ..Default::default() };
        assert_eq!(compute_dev_reward(signals), (VerdictOutcome::Failure, FAILURE_FLOOR));
    }

    #[test]
    fn rollback_takes_precedence_over_ci_fail() {
        let signals = DevRewardSignals { rollback: true, ci_fail: true, ..Default::default() };
        assert_eq!(compute_dev_reward(signals).0, VerdictOutcome::RolledBack);
    }

    #[test]
    fn human_approval_shortcuts_to_success_at_one() {
        let signals = DevRewardSignals { human_approval: true, ..Default::default() };
        assert_eq!(compute_dev_reward(signals), (VerdictOutcome::Success, HUMAN_APPROVAL_REWARD));
    }

    #[test]
    fn ci_fail_takes_precedence_over_human_approval() {
        let signals = DevRewardSignals { ci_fail: true, human_approval: true, ..Default::default() };
        assert_eq!(compute_dev_reward(signals).0, VerdictOutcome::Failure);
    }

    #[test]
    fn weights_sum_to_exactly_one() {
        assert_eq!(PR_MERGED_WEIGHT + CI_PASS_WEIGHT + NO_ROLLBACK_WEIGHT, 1.0);
    }

    // ---- VerdictRow -> DevRewardSignals adapter + registry dispatch ----

    fn verdict_row(role: &str, polarity: Polarity, becomes: Becomes) -> VerdictRow {
        VerdictRow { role: RoleId::parse(role).unwrap(), polarity, becomes }
    }

    #[test]
    fn a_dev_success_strategy_candidate_row_maps_to_the_full_positive_triad() {
        let row = verdict_row("dev", Polarity::Success, Becomes::StrategyCandidate);
        assert_eq!(dev_reward_fn(&[row]), (VerdictOutcome::Success, 1.0));
    }

    #[test]
    fn a_dev_failure_guardrail_candidate_row_maps_to_the_floor() {
        let row = verdict_row("dev", Polarity::Failure, Becomes::GuardrailCandidate);
        assert_eq!(dev_reward_fn(&[row]), (VerdictOutcome::Failure, FAILURE_FLOOR));
    }

    #[test]
    fn an_empty_verdict_list_stays_pending() {
        assert_eq!(dev_reward_fn(&[]), (VerdictOutcome::Pending, 0.0));
    }

    #[test]
    fn the_last_verdict_wins_a_remediation_after_a_finding_is_rewarded_not_floored() {
        // Trajectory.verdicts' own worked example: a code-review finding
        // followed by its later remediation.
        let finding = verdict_row("dev", Polarity::Failure, Becomes::GuardrailCandidate);
        let remediation = verdict_row("dev", Polarity::Success, Becomes::StrategyCandidate);
        assert_eq!(dev_reward_fn(&[finding, remediation]), (VerdictOutcome::Success, 1.0));
    }

    #[test]
    fn a_non_dev_role_never_uses_the_dev_weight_formula() {
        let registry = RewardRegistry::builtin();
        let row = verdict_row("content", Polarity::Success, Becomes::StrategyCandidate);
        // The dev formula would also read Success+StrategyCandidate as
        // 1.0 — assert content's registered fn is a DIFFERENT function
        // pointer AND produces the provisional default-convention value
        // (0.9), not coincidentally the same value for the wrong reason.
        assert_ne!(registry.get(&RoleId::parse("content").unwrap()) as *const (), dev_reward_fn as *const ());
        assert_eq!(registry.compute(&RoleId::parse("content").unwrap(), &[row]), (VerdictOutcome::Success, 0.9));
    }

    #[test]
    fn reviews_own_reward_source_is_the_corrective_clear_after_flagged_row() {
        let registry = RewardRegistry::builtin();
        let row = verdict_row("review", Polarity::Corrective, Becomes::GuardrailWhatTheSampleCaught);
        assert_eq!(registry.compute(&RoleId::parse("review").unwrap(), &[row]), (VerdictOutcome::Success, 0.9));
    }

    #[test]
    fn every_task_one_two_provisional_role_is_registered() {
        let registry = RewardRegistry::builtin();
        for slug in ["dev", "content", "design", "review", "planning", "test"] {
            assert!(registry.fns.contains_key(&RoleId::parse(slug).unwrap()), "{slug} must have a registered RewardFn");
        }
    }

    #[test]
    fn an_unregistered_role_falls_back_to_the_default_convention_never_errors() {
        let registry = RewardRegistry::builtin();
        let row = verdict_row("sim", Polarity::Failure, Becomes::GuardrailCandidate);
        assert_eq!(registry.compute(&RoleId::parse("sim").unwrap(), &[row]), (VerdictOutcome::Failure, 0.3));
    }

    #[test]
    fn compute_for_trajectory_resolves_role_from_the_regime_key() {
        let registry = RewardRegistry::builtin();
        let row = verdict_row("dev", Polarity::Success, Becomes::StrategyCandidate);
        let trajectory =
            Trajectory::new(TrajectoryId::new(), regime("dev"), "task", "ctx", vec![row], Utc::now(), vec![]).unwrap();
        assert_eq!(registry.compute_for_trajectory(&trajectory).unwrap(), (VerdictOutcome::Success, 1.0));
    }
}
