//! `OccurrencePromotionGate` (S7 design D3, task group 3) — the
//! n-occurrence + zero-contradiction promotion gate for roles whose
//! domain does NOT support deterministic CRN replay (design D3: "the
//! n-occurrence fallback is the permanent answer for non-replayable
//! domains, not a stopgap" — most of `dev`/`content`/`design`/
//! `review`). No direct donor (design D3's own text: "new — no
//! direct donor for this gate"); this is canon's own net-new gate.
//!
//! **Rule** (spec.md "N-occurrence + zero-contradiction promotion
//! gate"): `n_min` corroborating `Success`-verdict trajectories for the
//! regime, AND zero `Failure`-verdict trajectories for that regime,
//! inside a configurable observation window — a single contradicting
//! failure RESETS the corroboration count rather than being averaged
//! away. [`OccurrencePromotionGate::evaluate`] additionally treats
//! `RolledBack` as a contradiction alongside `Failure` (not literally
//! only "failure-verdict" as the design text's shorthand reads):
//! `verdict_outcome.rs`'s own doc frames `RolledBack` as a STRONGER
//! negative signal than a bare `Failure` (its default reward floors
//! even lower, `0.1` vs `0.3`) — excluding it from contradiction
//! detection would let a strategy get promoted despite an actual
//! recorded rollback in its own regime, defeating the gate's whole
//! purpose.
//!
//! **Window**: `samples` outside the trailing `window` (measured from
//! the caller-supplied `as_of` — [`super::PromotionGate::evaluate`]'s
//! own explicit evaluation-instant argument, never `Utc::now()` read
//! internally) are excluded entirely — neither corroborating nor
//! contradicting. `Pending` samples are skipped (neither corroborate
//! nor contradict; a trajectory with no covering verdict yet says
//! nothing either way).

use canon_model::ids::RegimeKey;
use chrono::{DateTime, Duration, Utc};

use crate::config::PromotionRoleConfig;
use crate::trajectory::Trajectory;
use crate::verdict_outcome::VerdictOutcome;

use super::{PromotionDecision, PromotionGate};

/// The n-occurrence + zero-contradiction gate (design D3). `n_min`/
/// `window` are per-role config
/// ([`crate::config::LearnConfig::promotion_config_for`]) —
/// [`OccurrencePromotionGate::from_config`] is the usual constructor;
/// [`OccurrencePromotionGate::default`] ships the SAME conservative
/// defaults ([`PromotionRoleConfig::default_occurrence`]) for a caller
/// with no `canon.yaml` `promotion.<role>` entry at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OccurrencePromotionGate {
    pub n_min: u32,
    pub window: Duration,
}

impl OccurrencePromotionGate {
    pub fn new(n_min: u32, window: Duration) -> Self {
        Self { n_min, window }
    }

    /// Built from a role's resolved [`PromotionRoleConfig`] (`mode` is
    /// ignored here — a caller routes to THIS gate only after already
    /// checking `config.mode == PromotionMode::Occurrence`, per
    /// `super` module doc's "a role declares which gate applies via
    /// `canon.yaml`" contract).
    pub fn from_config(config: PromotionRoleConfig) -> Self {
        Self { n_min: config.n_min, window: Duration::days(config.window_days) }
    }
}

impl Default for OccurrencePromotionGate {
    fn default() -> Self {
        Self::from_config(PromotionRoleConfig::default_occurrence())
    }
}

impl PromotionGate for OccurrencePromotionGate {
    fn evaluate(&self, regime_key: &RegimeKey, samples: &[Trajectory], as_of: DateTime<Utc>) -> PromotionDecision {
        let cutoff = as_of - self.window;

        // Defense-in-depth (`super` module doc: samples are TYPICALLY
        // already regime-scoped by the caller's own `query_by_
        // regime_key`, but this gate does not trust that blindly) +
        // the observation-window scoping design D3 requires: both the
        // n_min successes AND the zero-contradiction check are scoped
        // to samples inside the trailing window, nothing older counts
        // either way.
        let mut in_window: Vec<&Trajectory> =
            samples.iter().filter(|t| &t.regime_key == regime_key && t.recorded_at >= cutoff).collect();
        in_window.sort_by_key(|t| t.recorded_at);

        // A contradicting failure resets the counter rather than being
        // averaged away (design D3) — walking chronologically and
        // resetting on any Failure/RolledBack means the FINAL streak
        // already reflects "zero contradictions since the last reset",
        // no second pass needed.
        let mut streak = 0u32;
        for t in &in_window {
            match t.verdict_record.outcome {
                VerdictOutcome::Success => streak += 1,
                VerdictOutcome::Failure | VerdictOutcome::RolledBack => streak = 0,
                VerdictOutcome::Pending => {}
            }
        }

        if streak >= self.n_min {
            PromotionDecision::Promote {
                reason: format!(
                    "{streak} corroborating successes (>= n_min {}) with zero contradictions in the trailing {}-day window",
                    self.n_min,
                    self.window.num_days()
                ),
            }
        } else {
            PromotionDecision::Reject {
                reason: format!(
                    "{streak} corroborating successes since the last contradiction (need n_min {}) in the trailing {}-day window",
                    self.n_min,
                    self.window.num_days()
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
    use canon_model::ids::{RoleId, regime_key};
    use chrono::{DateTime, Duration};

    use super::*;
    use crate::ids::TrajectoryId;
    use crate::verdict_outcome::TrajectoryVerdict;

    fn regime() -> RegimeKey {
        RegimeKey::parse(regime_key("dev", "repo", "auth-flow", "deadbeef")).unwrap()
    }

    fn trajectory_at(outcome: VerdictOutcome, recorded_at: DateTime<Utc>) -> Trajectory {
        let verdict = VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
        Trajectory::new(TrajectoryId::new(), regime(), "task", "context", vec![verdict], recorded_at, vec![])
            .unwrap()
            .with_verdict_record(TrajectoryVerdict::new(outcome, outcome.default_reward()))
    }

    /// A fixed evaluation instant — every test below passes this
    /// explicitly as `as_of` rather than reading `Utc::now()`, so a
    /// decision here reflects ONLY what `evaluate`'s documented pure
    /// contract computes from its arguments, never when the test
    /// itself happens to run.
    fn fixed_as_of() -> DateTime<Utc> {
        "2025-06-15T12:00:00Z".parse().unwrap()
    }

    #[test]
    fn n_min_successes_with_zero_contradictions_promotes() {
        let gate = OccurrencePromotionGate::new(3, Duration::days(30));
        let now = fixed_as_of();
        let samples: Vec<Trajectory> = (0..3).map(|i| trajectory_at(VerdictOutcome::Success, now - Duration::days(3 - i))).collect();

        let decision = gate.evaluate(&regime(), &samples, now);
        assert!(decision.is_promote(), "{decision:?}");
    }

    #[test]
    fn below_n_min_rejects() {
        let gate = OccurrencePromotionGate::new(5, Duration::days(30));
        let now = fixed_as_of();
        let samples: Vec<Trajectory> = (0..3).map(|i| trajectory_at(VerdictOutcome::Success, now - Duration::days(3 - i))).collect();

        let decision = gate.evaluate(&regime(), &samples, now);
        assert!(!decision.is_promote(), "{decision:?}");
    }

    #[test]
    fn a_contradicting_failure_resets_the_count_even_at_n_min_minus_one() {
        let gate = OccurrencePromotionGate::new(3, Duration::days(30));
        let now = fixed_as_of();
        let mut samples: Vec<Trajectory> = (0..2).map(|i| trajectory_at(VerdictOutcome::Success, now - Duration::days(5 - i))).collect();
        samples.push(trajectory_at(VerdictOutcome::Failure, now - Duration::days(1)));

        let decision = gate.evaluate(&regime(), &samples, now);
        assert!(!decision.is_promote(), "{decision:?}");
        assert_eq!(decision.reason(), "0 corroborating successes since the last contradiction (need n_min 3) in the trailing 30-day window");
    }

    #[test]
    fn successes_after_a_reset_can_still_reach_n_min() {
        let gate = OccurrencePromotionGate::new(2, Duration::days(30));
        let now = fixed_as_of();
        let samples = vec![
            trajectory_at(VerdictOutcome::Success, now - Duration::days(10)),
            trajectory_at(VerdictOutcome::Failure, now - Duration::days(8)),
            trajectory_at(VerdictOutcome::Success, now - Duration::days(6)),
            trajectory_at(VerdictOutcome::Success, now - Duration::days(4)),
        ];

        let decision = gate.evaluate(&regime(), &samples, now);
        assert!(decision.is_promote(), "{decision:?}");
    }

    #[test]
    fn samples_outside_the_window_are_excluded_entirely() {
        let gate = OccurrencePromotionGate::new(2, Duration::days(7));
        let now = fixed_as_of();
        let samples = vec![
            // Old failure, OUTSIDE the 7-day window — must not
            // contradict a promotion built from recent successes.
            trajectory_at(VerdictOutcome::Failure, now - Duration::days(30)),
            trajectory_at(VerdictOutcome::Success, now - Duration::days(2)),
            trajectory_at(VerdictOutcome::Success, now - Duration::days(1)),
        ];

        let decision = gate.evaluate(&regime(), &samples, now);
        assert!(decision.is_promote(), "{decision:?}");
    }

    #[test]
    fn a_rolled_back_outcome_also_resets_the_count_not_just_failure() {
        let gate = OccurrencePromotionGate::new(2, Duration::days(30));
        let now = fixed_as_of();
        let samples = vec![
            trajectory_at(VerdictOutcome::Success, now - Duration::days(5)),
            trajectory_at(VerdictOutcome::RolledBack, now - Duration::days(1)),
        ];

        let decision = gate.evaluate(&regime(), &samples, now);
        assert!(!decision.is_promote(), "{decision:?}");
    }

    #[test]
    fn a_pending_sample_neither_corroborates_nor_contradicts() {
        let gate = OccurrencePromotionGate::new(2, Duration::days(30));
        let now = fixed_as_of();
        let samples = vec![
            trajectory_at(VerdictOutcome::Success, now - Duration::days(3)),
            trajectory_at(VerdictOutcome::Pending, now - Duration::days(2)),
            trajectory_at(VerdictOutcome::Success, now - Duration::days(1)),
        ];

        let decision = gate.evaluate(&regime(), &samples, now);
        assert!(decision.is_promote(), "{decision:?}");
    }

    #[test]
    fn a_trajectory_from_a_different_regime_is_ignored() {
        let gate = OccurrencePromotionGate::new(1, Duration::days(30));
        let other_regime = RegimeKey::parse(regime_key("dev", "repo", "other-area", "cafebabe")).unwrap();
        let now = fixed_as_of();
        let verdict = VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
        let foreign = Trajectory::new(TrajectoryId::new(), other_regime, "task", "context", vec![verdict], now, vec![])
            .unwrap()
            .with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, VerdictOutcome::Success.default_reward()));

        let decision = gate.evaluate(&regime(), &[foreign], now);
        assert!(!decision.is_promote(), "{decision:?}");
    }

    #[test]
    fn default_gate_matches_the_conservative_n_min_5_thirty_day_defaults() {
        let gate = OccurrencePromotionGate::default();
        assert_eq!(gate.n_min, 5);
        assert_eq!(gate.window, Duration::days(30));
    }

    /// Determinism lock: the SAME `(regime_key, samples, as_of)` triple
    /// yields an IDENTICAL [`PromotionDecision`] no matter how many
    /// times — or how much real wall-clock time passes between calls —
    /// `evaluate` runs. `as_of` is a fixed, caller-supplied instant
    /// (`fixed_as_of`) rather than `Utc::now()`; if `evaluate` ever
    /// regressed to reading the clock internally, the second call
    /// below (after a real sleep) could observe a different `cutoff`
    /// and flip the decision.
    #[test]
    fn evaluate_is_deterministic_for_the_same_regime_key_samples_and_as_of() {
        let gate = OccurrencePromotionGate::new(3, Duration::days(30));
        let as_of = fixed_as_of();
        let samples: Vec<Trajectory> =
            (0..3).map(|i| trajectory_at(VerdictOutcome::Success, as_of - Duration::days(3 - i))).collect();

        let first = gate.evaluate(&regime(), &samples, as_of);
        std::thread::sleep(std::time::Duration::from_millis(5));
        let second = gate.evaluate(&regime(), &samples, as_of);

        assert_eq!(first, second, "the same (regime_key, samples, as_of) must yield an identical decision every time");
        assert!(first.is_promote(), "{first:?}");
    }

    /// A success just BEFORE the window start (`as_of - window`) is
    /// excluded entirely — not merely "doesn't corroborate", genuinely
    /// invisible to the gate, per this module's own "measured from the
    /// caller-supplied `as_of`" window doc. Then, a second sample that
    /// WAS inside the window for an earlier `as_of` is dropped OUT once
    /// `as_of` advances far enough that the (still `window`-wide)
    /// trailing window's start moves past its `recorded_at` — proving
    /// the window tracks `as_of`, not a clock frozen at gate-construction
    /// time.
    #[test]
    fn a_sample_outside_as_of_minus_window_is_excluded_and_advancing_as_of_can_drop_a_sample_out_of_view() {
        let gate = OccurrencePromotionGate::new(1, Duration::days(7));
        let as_of = fixed_as_of();
        let window_start = as_of - Duration::days(7);

        // One millisecond before the window's opening instant: outside
        // [as_of - window, as_of], must not count.
        let just_outside = vec![trajectory_at(VerdictOutcome::Success, window_start - Duration::milliseconds(1))];
        let decision = gate.evaluate(&regime(), &just_outside, as_of);
        assert!(!decision.is_promote(), "a success just before the window start must not count: {decision:?}");

        // A success recorded ONE day INTO the window counts for the
        // original as_of...
        let borderline = vec![trajectory_at(VerdictOutcome::Success, window_start + Duration::days(1))];
        let decision_in_window = gate.evaluate(&regime(), &borderline, as_of);
        assert!(decision_in_window.is_promote(), "a success inside the window must count: {decision_in_window:?}");

        // ...but advancing as_of forward by more than a window's worth
        // pushes the window's start PAST that same sample's
        // recorded_at — the SAME sample, now excluded.
        let later_as_of = as_of + Duration::days(8);
        let decision_after_advance = gate.evaluate(&regime(), &borderline, later_as_of);
        assert!(
            !decision_after_advance.is_promote(),
            "advancing as_of past a sample's recorded_at must drop it out of the window: {decision_after_advance:?}"
        );
    }
}
