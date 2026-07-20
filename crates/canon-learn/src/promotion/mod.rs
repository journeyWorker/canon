//! The statistical-promotion seam (design D3/D4, task groups 2-4) —
//! [`PromotionGate`] + [`PromotionDecision`] + [`demote_strategy`],
//! established by S7Core (task group 1) so the wave-2 sub-agents build
//! against a stable contract without rework. [`crn`] (task group 2)
//! ports MaTTS's pure statistics core:
//! [`crn::CrnPromotionGate`]. [`occurrence`] (task group 3) is the
//! n-occurrence + zero-contradiction gate for non-replayable domains:
//! [`occurrence::OccurrencePromotionGate`]. [`demote`] (task group 4)
//! is `demote_strategy`'s real persistence body — WIDENED from
//! S7Core's original frozen 2-arg stub to take the persistence context
//! (`&dyn StrategyStore`, the git-tier root, the demotion policy) its
//! own doc comment already described as its job (`demote` module doc
//! has the full rationale). This directory layout (`promotion/mod.rs`,
//! plus one file per gate/concern) is what let each of the three land
//! as an independent submodule without touching this file's own
//! `PromotionGate`/`PromotionDecision`/`DemotionRecord` type
//! definitions.

use std::path::{Path, PathBuf};

use canon_model::ids::RegimeKey;
use chrono::{DateTime, Utc};

use crate::ids::{StrategyId, TrajectoryId};
use crate::trajectory::Trajectory;

pub mod crn;
pub mod demote;
pub mod occurrence;
pub mod promote;

pub use crn::CrnPromotionGate;
pub use demote::{DemotionPolicy, demote_strategy};
pub use occurrence::OccurrencePromotionGate;
pub use promote::{plan_promotion, promote_strategy, Promotion};

/// The git-tier file path for a promoted/demoted strategy:
/// `<git_tier_root>/<role>/<strategy_id>.md` (S6 design decision 4).
/// Shared by [`promote::promote_strategy`] (which writes it) and
/// [`demote::demote_strategy`] (which soft-flags/hard-deletes it) so
/// both ends of the promote/demote lifecycle agree on the layout by
/// construction, never a second per-module derivation.
pub(crate) fn git_tier_path(git_tier_root: &Path, role: &str, strategy_id: StrategyId) -> PathBuf {
    git_tier_root.join(role).join(format!("{strategy_id}.md"))
}

/// A promotion gate's verdict (design D3's `corroboratedEffect(batch) ->
/// bool`, widened to carry a human-readable `reason` — mirrors
/// MaTTS's `RewardResult`/`VarianceDecomposition` convention of
/// surfacing WHY a decision landed, not just a bare boolean,
/// per the donor's reward-computation audit,
/// "surface the factor breakdown, not just the scalar").
#[derive(Debug, Clone, PartialEq)]
pub enum PromotionDecision {
    /// Corroborating evidence clears the gate's threshold — the
    /// candidate strategy is eligible for promotion.
    Promote { reason: String },
    /// Insufficient or non-significant evidence (below `n_min`, a
    /// non-significant CRN contrast, a contradicting failure inside the
    /// observation window, …) — `reason` explains which.
    Reject { reason: String },
}

impl PromotionDecision {
    pub fn is_promote(&self) -> bool {
        matches!(self, Self::Promote { .. })
    }

    pub fn reason(&self) -> &str {
        match self {
            Self::Promote { reason } | Self::Reject { reason } => reason,
        }
    }
}

/// The seam CRN (`CrnPromotionGate`, task group 2) and occurrence
/// (`OccurrencePromotionGate`, task group 3) promotion gates both
/// implement (design D3). `evaluate` is a PURE function of
/// `regime_key`, already-resolved `samples`, AND `as_of` — neither
/// gate reads a store OR a wall clock directly, mirroring both
/// MaTTS's own "pure statistics core / sampling integration
/// layer" split (per the donor's MaTTS statistical-promotion
/// audit) and `webhook.rs`'s own
/// `check_no_rollback`/`evaluate_no_rollback_timer` discipline of
/// taking the evaluation instant as an explicit argument instead of
/// reading `Utc::now()` internally: synchronous, deterministic,
/// fixture-testable on synthetic `Trajectory` slices, no I/O. The
/// SAME `(regime_key, samples, as_of)` triple MUST yield the SAME
/// [`PromotionDecision`] every time, no matter when the call actually
/// runs — a live caller passes `Utc::now()` at the call site (see
/// [`evaluate_now`]) for "trailing window ending now" semantics; a
/// reconcile/replay caller passes a historical instant to reproduce a
/// past decision exactly.
///
/// `samples` is every [`Trajectory`] collected for `regime_key` so far
/// — typically a caller's `TrajectoryStore::query_by_regime_key(regime_
/// key)` result, resolved BEFORE calling `evaluate`. `OccurrencePromotionGate`
/// reads each sample's `verdict_record.outcome` directly (`n_min`
/// corroborating `Success` AND zero `Failure`/`RolledBack` inside its
/// trailing window ending at `as_of`). `CrnPromotionGate` additionally
/// needs paired common-random-number PANEL/CONFIG identity to run its
/// ANOVA-style decomposition — and its own decision is time-independent,
/// so it accepts `as_of` only to satisfy this trait's uniform signature
/// (`crn` module doc) — this trait does not bake that structure into
/// its own signature; a CRN-capable role's own trajectory-recording
/// caller encodes panel/config identity in `Trajectory::tags` (S6's own
/// frozen, free-form `Vec<String>` side channel), and `CrnPromotionGate::
/// evaluate` parses it back out. Keeping the trait itself agnostic to
/// which encoding a specific gate needs is what lets ONE signature serve
/// both gate shapes without over-fitting to either.
pub trait PromotionGate {
    fn evaluate(&self, regime_key: &RegimeKey, samples: &[Trajectory], as_of: DateTime<Utc>) -> PromotionDecision;
}

/// Convenience for a live caller that wants "trailing window ending
/// now" semantics without threading `Utc::now()` through by hand — the
/// trait method itself ([`PromotionGate::evaluate`]) stays pure; this
/// free function (not a trait method, per design: the trait's own
/// contract never reads a clock) is the ONE place in this crate a
/// live promotion check reads the wall clock, mirroring `webhook.rs`'s
/// own real-scheduler-passes-`Utc::now()` convention.
pub fn evaluate_now(gate: &dyn PromotionGate, regime_key: &RegimeKey, samples: &[Trajectory]) -> PromotionDecision {
    gate.evaluate(regime_key, samples, Utc::now())
}

/// Evidence a previously-promoted strategy was demoted (design D4) —
/// the PURE value [`demote_strategy`] constructs and returns. The full
/// write path (persisting an S1-envelope-shaped demotion evidence
/// record via [`crate::store::StrategyStore::mark_demoted`], and
/// soft-flagging/hard-deleting the git-tier `canon/strategies/<role>/
/// <id>.md` file per its [`demote::DemotionPolicy`]) lives in
/// [`demote`] (task group 4, S7 wave-2) — `demote_strategy`'s WIDENED
/// signature is what makes that write path reachable; this struct
/// stays the frozen return-value SHAPE S7Core (task group 1)
/// established.
#[derive(Debug, Clone, PartialEq)]
pub struct DemotionRecord {
    pub strategy_id: StrategyId,
    pub contradicting_trajectory_id: TrajectoryId,
    pub demoted_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
    use canon_model::ids::{RoleId, regime_key};
    use chrono::Duration;

    use super::*;
    use crate::verdict_outcome::{TrajectoryVerdict, VerdictOutcome};

    #[test]
    fn promotion_decision_reports_its_own_kind_and_reason() {
        let promote = PromotionDecision::Promote { reason: "n_min corroborated".to_string() };
        assert!(promote.is_promote());
        assert_eq!(promote.reason(), "n_min corroborated");

        let reject = PromotionDecision::Reject { reason: "below n_min".to_string() };
        assert!(!reject.is_promote());
        assert_eq!(reject.reason(), "below n_min");
    }

    /// [`evaluate_now`] is the ONE place in this crate that reads
    /// `Utc::now()` for a promotion decision — it must do nothing more
    /// than thread that instant into an otherwise-normal, pure
    /// `evaluate` call: calling it directly must equal calling
    /// `gate.evaluate(..., Utc::now())` by hand.
    #[test]
    fn evaluate_now_threads_the_current_instant_into_a_pure_evaluate_call() {
        let regime = RegimeKey::parse(regime_key("dev", "repo", "evaluate-now-fixture", "deadbeef")).unwrap();
        let verdict = VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
        let now = Utc::now();
        let sample = Trajectory::new(TrajectoryId::new(), regime.clone(), "task", "context", vec![verdict], now, vec![])
            .unwrap()
            .with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, VerdictOutcome::Success.default_reward()));
        let samples = [sample];

        let gate = OccurrencePromotionGate::new(1, Duration::days(1));
        let via_convenience = evaluate_now(&gate, &regime, &samples);
        let via_direct = gate.evaluate(&regime, &samples, Utc::now());

        assert_eq!(via_convenience, via_direct, "evaluate_now must thread Utc::now() into a normal evaluate call, nothing else");
        assert!(via_convenience.is_promote(), "{via_convenience:?}");
    }
}
