//! `rebuild_namespace`: the non-destructive delete-rebuild primitive
//! (design decision 3, spec.md "Non-destructive distillation") —
//! deletes ONLY `regime_key`'s [`StrategyItem`] rows and re-derives
//! them from the untouched, retained raw [`Trajectory`] rows. Mirrors
//! the donor's reasoning-bank `rebuildStrategies` almost verbatim:
//! read raw -> delete distilled -> re-distill -> re-store distilled.
//! [`TrajectoryStore`] never appears on the delete side of this
//! function — there is no code path here (or anywhere in this crate)
//! that can delete a raw trajectory.

use canon_model::ids::RegimeKey;

use crate::distill::distill_namespace;
use crate::error::LearnError;
use crate::store::{StrategyStore, TrajectoryStore};
use crate::strategy::StrategyItem;

/// Rebuilds the strategy layer for `regime_key`: queries every raw
/// trajectory for it, deletes every existing strategy item for it,
/// re-distills from the (just-read, unmodified) trajectories, and
/// stores the freshly-distilled items. Returns the newly-stored items.
pub fn rebuild_namespace(
    trajectory_store: &dyn TrajectoryStore,
    strategy_store: &dyn StrategyStore,
    regime_key: &RegimeKey,
) -> Result<Vec<StrategyItem>, LearnError> {
    let trajectories = trajectory_store.query_by_regime_key(regime_key)?;
    strategy_store.delete_for_regime_key(regime_key)?;

    let items = distill_namespace(regime_key, &trajectories);
    for item in &items {
        strategy_store.append(item)?;
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
    use canon_model::ids::RoleId;
    use chrono::Utc;

    use super::*;
    use crate::ids::TrajectoryId;
    use crate::store::{ParquetStrategyStore, ParquetTrajectoryStore};
    use crate::trajectory::Trajectory;

    fn regime() -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key("dev", "repo", "auth", "abc123")).unwrap()
    }

    fn trajectory(task: &str) -> Trajectory {
        let verdict = VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
        Trajectory::new(TrajectoryId::new(), regime(), task, "ctx", vec![verdict], Utc::now(), vec![]).unwrap()
    }

    #[test]
    fn rebuild_is_non_destructive_raw_trajectories_survive_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let traj_root = dir.path().join("trajectories");
        let trajectory_store = ParquetTrajectoryStore::open(&traj_root);
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));

        trajectory_store.append(&trajectory("first task")).unwrap();
        trajectory_store.append(&trajectory("second task")).unwrap();

        // Snapshot every raw trajectory FILE's bytes before rebuild.
        let file_bytes_before = read_all_files_sorted(&traj_root);
        assert_eq!(file_bytes_before.len(), 2, "fixture wrote two trajectory files");

        let first_pass = rebuild_namespace(&trajectory_store, &strategy_store, &regime()).unwrap();
        assert_eq!(first_pass.len(), 2, "one strategy item per trajectory's single verdict");

        let file_bytes_after_first_rebuild = read_all_files_sorted(&traj_root);
        assert_eq!(file_bytes_before, file_bytes_after_first_rebuild, "rebuild must never touch raw trajectory bytes");

        // Rebuilding again must delete-and-redistill the strategy layer
        // (not accumulate duplicates) while STILL never touching raw.
        let second_pass = rebuild_namespace(&trajectory_store, &strategy_store, &regime()).unwrap();
        assert_eq!(second_pass.len(), 2, "re-derived from the same two retained trajectories");
        assert_eq!(
            strategy_store.query_by_regime_key(&regime()).unwrap().len(),
            2,
            "delete-rebuild replaces, never accumulates, the distilled layer"
        );

        let file_bytes_after_second_rebuild = read_all_files_sorted(&traj_root);
        assert_eq!(file_bytes_before, file_bytes_after_second_rebuild, "second rebuild also never touches raw trajectory bytes");
    }

    #[test]
    fn rebuild_on_an_empty_namespace_yields_no_strategies_and_no_error() {
        let dir = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let items = rebuild_namespace(&trajectory_store, &strategy_store, &regime()).unwrap();
        assert!(items.is_empty());
    }

    fn read_all_files_sorted(root: &std::path::Path) -> Vec<Vec<u8>> {
        let mut paths = Vec::new();
        collect_files(root, &mut paths);
        paths.sort();
        paths.into_iter().map(|p| std::fs::read(p).unwrap()).collect()
    }

    fn collect_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, out);
            } else {
                out.push(path);
            }
        }
    }
}
