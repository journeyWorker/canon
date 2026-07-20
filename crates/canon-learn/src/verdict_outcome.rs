//! [`VerdictOutcome`] + [`TrajectoryVerdict`]: the S7-level trajectory
//! outcome/reward pair (design D2), distinct from
//! [`crate::trajectory::Trajectory::verdicts`]' raw
//! [`canon_ingest::verdict::VerdictRow`] evidence list. A `VerdictRow`
//! is S4's already-derived `{role, polarity, becomes}` classification
//! of ONE source event; a [`TrajectoryVerdict`] is S7's own rolled-up
//! judgment of the WHOLE trajectory — `pending` until
//! [`crate::mark_verdict::mark_trajectory_verdict`] writes a covering
//! verdict, generalizing the donor's reasoning-bank verdict-write
//! contract.

use serde::{Deserialize, Serialize};

/// A trajectory's rolled-up outcome (design D2: `pending | success |
/// failure | rolled-back`, mirroring the donor's verdict-write contract
/// exactly). Distinct from [`canon_ingest::verdict::Polarity`] (S4's
/// per-event `failure | success | corrective` classification) — this
/// enum is S7's own, coarser, whole-trajectory judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerdictOutcome {
    /// No covering verdict event has arrived yet — the default state
    /// every [`crate::trajectory::Trajectory`] starts in.
    Pending,
    Success,
    Failure,
    /// The donor has no direct default for this state (design D2's
    /// 0.9/0.3/0.5 triad only names `success`/`failure`/`pending`) —
    /// canon mirrors the `dev` reward function's own rollback FLOOR
    /// (0.1, `crate::reward::FAILURE_FLOOR`): a rollback is a stronger
    /// negative signal than a bare `failure`, so its default reward
    /// must never read higher than `failure`'s.
    RolledBack,
}

impl VerdictOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::RolledBack => "rolled-back",
        }
    }

    /// Canon's DEFAULT reward convention (design D2, ported from
    /// the donor's reasoning-bank default: 0.9 success / 0.3 failure /
    /// 0.5 pending, from its post-edit-trajectory module header) —
    /// the value [`TrajectoryVerdict::pending`] seeds, and the
    /// fallback a role's registered reward function may override but
    /// never escape `[0, 1]` doing so.
    pub fn default_reward(self) -> f64 {
        match self {
            Self::Success => 0.9,
            Self::Failure => 0.3,
            Self::Pending => 0.5,
            Self::RolledBack => 0.1,
        }
    }
}

impl std::fmt::Display for VerdictOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Clamps a reward into `[0, 1]` — NEVER panics. `f64::clamp` itself
/// returns a NaN input unchanged rather than panicking, which would
/// silently violate the `[0, 1]` invariant this whole module exists to
/// hold, so a NaN reward is treated as "unknown" and degrades to the
/// `Pending` default (0.5) instead of propagating.
pub fn clamp_reward(reward: f64) -> f64 {
    if reward.is_nan() { VerdictOutcome::Pending.default_reward() } else { reward.clamp(0.0, 1.0) }
}

/// The verdict+reward pair carried on a stored [`crate::trajectory::
/// Trajectory`] (design D2). Construction ALWAYS clamps `reward` into
/// `[0, 1]` — there is no bypass that stores an out-of-range or NaN
/// value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryVerdict {
    pub outcome: VerdictOutcome,
    pub reward: f64,
}

impl TrajectoryVerdict {
    pub fn new(outcome: VerdictOutcome, reward: f64) -> Self {
        Self { outcome, reward: clamp_reward(reward) }
    }

    /// The unset default every trajectory starts at: `Pending` at the
    /// default 0.5 reward (design D2's default convention).
    pub fn pending() -> Self {
        Self::new(VerdictOutcome::Pending, VerdictOutcome::Pending.default_reward())
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.outcome, VerdictOutcome::Pending)
    }
}

impl Default for TrajectoryVerdict {
    fn default() -> Self {
        Self::pending()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_is_the_default_convention() {
        let v = TrajectoryVerdict::pending();
        assert_eq!(v.outcome, VerdictOutcome::Pending);
        assert_eq!(v.reward, 0.5);
        assert!(v.is_pending());
        assert_eq!(TrajectoryVerdict::default(), v);
    }

    #[test]
    fn default_reward_convention_matches_design_d2() {
        assert_eq!(VerdictOutcome::Success.default_reward(), 0.9);
        assert_eq!(VerdictOutcome::Failure.default_reward(), 0.3);
        assert_eq!(VerdictOutcome::Pending.default_reward(), 0.5);
    }

    #[test]
    fn construction_clamps_above_one() {
        let v = TrajectoryVerdict::new(VerdictOutcome::Success, 5.0);
        assert_eq!(v.reward, 1.0);
    }

    #[test]
    fn construction_clamps_below_zero() {
        let v = TrajectoryVerdict::new(VerdictOutcome::Failure, -3.0);
        assert_eq!(v.reward, 0.0);
    }

    #[test]
    fn construction_never_panics_on_nan() {
        let v = TrajectoryVerdict::new(VerdictOutcome::Pending, f64::NAN);
        assert_eq!(v.reward, 0.5);
    }

    #[test]
    fn in_range_reward_is_preserved_exactly() {
        let v = TrajectoryVerdict::new(VerdictOutcome::Success, 0.73);
        assert_eq!(v.reward, 0.73);
    }

    #[test]
    fn serde_round_trips_including_kebab_case_rolled_back() {
        let v = TrajectoryVerdict::new(VerdictOutcome::RolledBack, 0.1);
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"rolled-back\""));
        let back: TrajectoryVerdict = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v);
    }
}
