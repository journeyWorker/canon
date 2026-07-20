//! The raw tier: [`Trajectory`] — a captured trace, generalizing
//! the donor harness's `PatternTrajectory`
//! (id/namespace/task/input/action/output/verdict/reward/recordedAt/tags,
//! deliberately single-step — "the donor's harness runs are one tool call →
//! one trajectory") onto canon's join spine: the namespace column
//! becomes the canonical `regime_key` (design decision 2), and the
//! bare `verdict: PatternVerdict` string becomes the actual
//! [`VerdictRow`](canon_ingest::verdict::VerdictRow)(s) S4 already
//! derived, carried verbatim rather than re-collapsed into a
//! `success|failure|pending` enum.

use canon_ingest::verdict::VerdictRow;
use canon_model::ids::{RegimeKey, RoleId};
use chrono::{DateTime, Utc};

use crate::error::LearnError;
use crate::ids::TrajectoryId;
use crate::verdict_outcome::TrajectoryVerdict;

/// One captured trace: the [`VerdictRow`]s a S4 artifact-ingest wave
/// derived for this outcome, plus the reasoning/context that produced
/// it — raw, immutable, cold tier (design decision 3). Keyed by
/// [`RegimeKey`] (design decision 2's `<role>/<repo>/<area>/<hash>`,
/// the SAME `canon_model::ids::regime_key` serialization every write
/// and read path in canon reuses).
#[derive(Debug, Clone, PartialEq)]
pub struct Trajectory {
    pub id: TrajectoryId,
    pub regime_key: RegimeKey,
    /// What was attempted — the short task description
    /// (`PatternTrajectory.task`'s analog; the distiller's `title`
    /// source for a successful strategy candidate).
    pub task: String,
    /// The reasoning/context narrative that produced the outcome
    /// (`PatternTrajectory.input`/`.output`'s analog, collapsed to one
    /// field per this crate's own scope — see module doc's
    /// single-step rationale).
    pub context: String,
    /// The VerdictRow(s) this trajectory carries — never empty
    /// ([`Trajectory::new`] rejects an empty list). One trajectory MAY
    /// carry more than one verdict (e.g. a code-review finding
    /// followed by its later remediation, both folded onto the same
    /// regime); [`crate::distill::distill_trajectory`] emits up to one
    /// distilled item per verdict.
    pub verdicts: Vec<VerdictRow>,
    pub recorded_at: DateTime<Utc>,
    pub tags: Vec<String>,
    /// The S7-level rolled-up outcome+reward (design D2) —
    /// `TrajectoryVerdict::pending()` until
    /// [`crate::mark_verdict::mark_trajectory_verdict`] writes a
    /// covering verdict. Not a [`Trajectory::new`] constructor
    /// parameter (every freshly-constructed trajectory starts
    /// `Pending`, matching the two-phase reward-write model
    /// the donor's dev-reward backfill documents) — use
    /// [`Trajectory::with_verdict_record`] to seed a non-default value
    /// (e.g. test fixtures).
    pub verdict_record: TrajectoryVerdict,
}

impl Trajectory {
    /// Constructs a trajectory, validating the two invariants a
    /// well-formed regime-keyed trace must hold:
    ///
    /// - at least one [`VerdictRow`] ([`LearnError::EmptyVerdicts`]);
    /// - every verdict's `role` agrees with `regime_key`'s own `role`
    ///   segment ([`LearnError::VerdictRoleMismatch`]) — `regime_key`'s
    ///   role is the single retrieval axis (design decision 2: "a
    ///   `dev` trajectory must never surface as a similar-regime hit
    ///   for a `content` role"), so a trajectory whose verdicts
    ///   disagree with its own key would silently violate that at
    ///   read time.
    pub fn new(
        id: TrajectoryId,
        regime_key: RegimeKey,
        task: impl Into<String>,
        context: impl Into<String>,
        verdicts: Vec<VerdictRow>,
        recorded_at: DateTime<Utc>,
        tags: Vec<String>,
    ) -> Result<Self, LearnError> {
        if verdicts.is_empty() {
            return Err(LearnError::EmptyVerdicts);
        }
        for v in &verdicts {
            if v.role.as_str() != regime_key.role() {
                return Err(LearnError::VerdictRoleMismatch {
                    verdict_role: v.role.as_str().to_string(),
                    regime_role: regime_key.role().to_string(),
                });
            }
        }
        Ok(Self { id, regime_key, task: task.into(), context: context.into(), verdicts, recorded_at, tags, verdict_record: TrajectoryVerdict::pending() })
    }

    pub fn role(&self) -> Result<RoleId, LearnError> {
        RoleId::parse(self.regime_key.role()).map_err(LearnError::from)
    }

    /// Builder-style override for [`Trajectory::verdict_record`] — the
    /// constructor always seeds `Pending`; this is the escape hatch for
    /// a caller (typically a test fixture) that wants a pre-resolved
    /// trajectory without a separate `mark_trajectory_verdict` round
    /// trip.
    pub fn with_verdict_record(mut self, verdict_record: TrajectoryVerdict) -> Self {
        self.verdict_record = verdict_record;
        self
    }
}

#[cfg(test)]
mod tests {
    use canon_ingest::verdict::{Becomes, Polarity};

    use super::*;

    fn verdict(role: &str) -> VerdictRow {
        VerdictRow { role: RoleId::parse(role).unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate }
    }

    fn regime(role: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(role, "repo", "auth", "abc123")).unwrap()
    }

    #[test]
    fn rejects_empty_verdicts() {
        let err = Trajectory::new(TrajectoryId::new(), regime("dev"), "t", "c", vec![], Utc::now(), vec![]).unwrap_err();
        assert!(matches!(err, LearnError::EmptyVerdicts));
    }

    #[test]
    fn rejects_a_verdict_role_disagreeing_with_the_regime_key_role() {
        let err =
            Trajectory::new(TrajectoryId::new(), regime("dev"), "t", "c", vec![verdict("content")], Utc::now(), vec![])
                .unwrap_err();
        assert!(matches!(err, LearnError::VerdictRoleMismatch { .. }));
    }

    #[test]
    fn accepts_verdicts_agreeing_with_the_regime_key_role() {
        let trajectory =
            Trajectory::new(TrajectoryId::new(), regime("dev"), "t", "c", vec![verdict("dev")], Utc::now(), vec![]).unwrap();
        assert_eq!(trajectory.role().unwrap(), RoleId::parse("dev").unwrap());
    }
}
