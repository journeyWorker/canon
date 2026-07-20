//! `mark_trajectory_verdict`: completing S6's `TrajectoryStore`
//! write-back surface (design D2, task 1.3) — the canon-side equivalent
//! of the donor's reasoning-bank verdict-write contract. This
//! is the FIRST real caller of [`crate::store::TrajectoryStore::
//! mark_verdict`] besides its own tests.

use crate::error::LearnError;
use crate::ids::TrajectoryId;
use crate::store::TrajectoryStore;
use crate::verdict_outcome::{TrajectoryVerdict, VerdictOutcome};

/// Writes a covering verdict + `[0, 1]`-clamped reward back onto a
/// stored trajectory (design D2). `reward` is re-clamped here
/// regardless of what a caller's reward function already clamped —
/// this is the LAST gate before persistence, so it holds even if a
/// caller bypasses [`crate::reward::RewardRegistry`] entirely.
///
/// Rejects [`VerdictOutcome::Pending`] outright
/// ([`LearnError::CannotMarkVerdictPending`]): `Pending` is the
/// trajectory's own UNSET default (`TrajectoryVerdict::pending`), never
/// a value a "covering verdict" write can set — allowing it would let a
/// caller re-open an already-resolved trajectory, violating design D2's
/// "must not leave a trajectory pending once a covering verdict
/// arrives". Rejects an unmatched `trajectory_id`
/// ([`LearnError::UnknownTrajectoryId`]) rather than silently no-oping —
/// the donor's own in-memory verdict-write silently no-ops on an unmatched
/// id with "no error, no log distinguishing 'verdict written' from 'no
/// matching trajectory'" (the donor's own documented failure mode)
/// — canon's own S1
/// "malformed/unmatched evidence is no evidence: skip + violation,
/// never crash" principle applies here as "fail loud", not "silently
/// vanish".
pub fn mark_trajectory_verdict(
    store: &dyn TrajectoryStore,
    trajectory_id: &TrajectoryId,
    outcome: VerdictOutcome,
    reward: f64,
) -> Result<TrajectoryVerdict, LearnError> {
    if outcome == VerdictOutcome::Pending {
        return Err(LearnError::CannotMarkVerdictPending);
    }
    let verdict = TrajectoryVerdict::new(outcome, reward);
    store.mark_verdict(trajectory_id, verdict)?;
    Ok(verdict)
}

#[cfg(test)]
mod tests {
    use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
    use canon_model::ids::{RegimeKey, RoleId};
    use chrono::Utc;

    use super::*;
    use crate::store::ParquetTrajectoryStore;
    use crate::trajectory::Trajectory;

    fn regime() -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key("dev", "repo", "auth", "abc123")).unwrap()
    }

    fn trajectory() -> Trajectory {
        let verdict = VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
        Trajectory::new(TrajectoryId::new(), regime(), "task", "ctx", vec![verdict], Utc::now(), vec![]).unwrap()
    }

    #[test]
    fn a_freshly_stored_trajectory_starts_pending_at_the_default_reward() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory();
        store.append(&t).unwrap();

        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert!(found.verdict_record.is_pending());
        assert_eq!(found.verdict_record.reward, 0.5);
    }

    #[test]
    fn mark_trajectory_verdict_flips_pending_to_the_covering_outcome_and_persists_reward() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory();
        store.append(&t).unwrap();

        let written = mark_trajectory_verdict(&store, &t.id, VerdictOutcome::Success, 0.9).unwrap();
        assert_eq!(written, TrajectoryVerdict::new(VerdictOutcome::Success, 0.9));

        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert!(!found.verdict_record.is_pending());
        assert_eq!(found.verdict_record.outcome, VerdictOutcome::Success);
        assert_eq!(found.verdict_record.reward, 0.9);
    }

    #[test]
    fn reward_is_re_clamped_even_if_the_caller_passes_an_out_of_range_value() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory();
        store.append(&t).unwrap();

        let written = mark_trajectory_verdict(&store, &t.id, VerdictOutcome::Failure, -8.0).unwrap();
        assert_eq!(written.reward, 0.0);
        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert_eq!(found.verdict_record.reward, 0.0);
    }

    #[test]
    fn marking_pending_is_rejected_never_reopens_a_resolved_trajectory() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory();
        store.append(&t).unwrap();
        mark_trajectory_verdict(&store, &t.id, VerdictOutcome::Success, 0.9).unwrap();

        let err = mark_trajectory_verdict(&store, &t.id, VerdictOutcome::Pending, 0.5).unwrap_err();
        assert!(matches!(err, LearnError::CannotMarkVerdictPending));

        // The prior resolved verdict must survive the rejected call untouched.
        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert_eq!(found.verdict_record.outcome, VerdictOutcome::Success);
    }

    #[test]
    fn an_unknown_trajectory_id_is_a_loud_error_never_a_silent_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let unknown_id = TrajectoryId::new();

        let err = mark_trajectory_verdict(&store, &unknown_id, VerdictOutcome::Success, 0.9).unwrap_err();
        assert!(matches!(err, LearnError::UnknownTrajectoryId(id) if id == unknown_id.to_string()));
    }

    #[test]
    fn a_rolled_back_outcome_persists_distinctly_from_failure() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory();
        store.append(&t).unwrap();

        mark_trajectory_verdict(&store, &t.id, VerdictOutcome::RolledBack, 0.1).unwrap();
        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert_eq!(found.verdict_record.outcome, VerdictOutcome::RolledBack);
    }
}
