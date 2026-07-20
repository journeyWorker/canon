//! `store_trajectory`: the registry-gated write path (spec.md
//! "Role-namespaced trajectory store" — "the system SHALL … reject a
//! write carrying an unregistered role at write time"). Layered ON TOP
//! of [`TrajectoryStore::append`] rather than folded into it — the
//! trait itself stays a pure storage seam (any future
//! `LanceDbTrajectoryStore` gets registry-gating for free by going
//! through this function, without the trait itself depending on
//! [`RoleRegistry`]).

use crate::error::LearnError;
use crate::role::RoleRegistry;
use crate::store::TrajectoryStore;
use crate::trajectory::Trajectory;

/// Validates `trajectory`'s role against `registry`, THEN persists it.
/// Fails loud (`Err(LearnError::UnregisteredRole)`, no partial write)
/// when the role is not registered — never a silent no-op.
pub fn store_trajectory(registry: &RoleRegistry, store: &dyn TrajectoryStore, trajectory: &Trajectory) -> Result<(), LearnError> {
    let role = trajectory.role()?;
    registry.validate(&role)?;
    store.append(trajectory)
}

#[cfg(test)]
mod tests {
    use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
    use canon_model::ids::{RegimeKey, RoleId};
    use chrono::Utc;

    use super::*;
    use crate::ids::TrajectoryId;
    use crate::store::ParquetTrajectoryStore;

    fn regime(role: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(role, "repo", "auth", "abc123")).unwrap()
    }

    fn trajectory(role: &str) -> Trajectory {
        let verdict = VerdictRow { role: RoleId::parse(role).unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
        Trajectory::new(TrajectoryId::new(), regime(role), "task", "ctx", vec![verdict], Utc::now(), vec![]).unwrap()
    }

    #[test]
    fn a_built_in_role_writes_successfully() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let registry = RoleRegistry::builtin();
        store_trajectory(&registry, &store, &trajectory("dev")).unwrap();
        assert_eq!(store.query_by_regime_key(&regime("dev")).unwrap().len(), 1);
    }

    #[test]
    fn an_unregistered_role_is_rejected_and_nothing_is_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let registry = RoleRegistry::builtin();
        let err = store_trajectory(&registry, &store, &trajectory("triage")).unwrap_err();
        assert!(matches!(err, LearnError::UnregisteredRole(role) if role == "triage"));
        assert_eq!(store.query_by_regime_key(&regime("triage")).unwrap().len(), 0, "rejected write must not persist");
    }
}
