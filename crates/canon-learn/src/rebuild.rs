//! `rebuild_namespace`: the non-destructive delete-rebuild primitive
//! (design decision 3, spec.md "Non-destructive distillation") —
//! deletes ONLY `regime_key`'s [`StrategyItem`] rows and re-derives
//! them from the untouched, retained raw [`Trajectory`] rows. Mirrors
//! the donor's reasoning-bank `rebuildStrategies` almost verbatim:
//! read raw -> delete distilled -> re-distill -> re-store distilled.
//! [`TrajectoryStore`] never appears on the delete side of this
//! function — there is no code path here (or anywhere in this crate)
//! that can delete a raw trajectory.

use std::collections::HashMap;

use canon_model::ids::RegimeKey;

use crate::distill::distill_namespace;
use crate::error::LearnError;
use crate::store::{StrategyStore, TrajectoryStore};
use crate::strategy::{StrategyItem, StrategyLifecycle};

/// Rebuilds the strategy layer for `regime_key`: queries every raw
/// trajectory for it, deletes every existing strategy item for it,
/// re-distills from the (just-read, unmodified) trajectories, and
/// stores the freshly-distilled items. Returns the newly-stored items.
///
/// A FIXPOINT over unchanged evidence, not merely a refresh:
/// [`distill_namespace`] derives every field — the [`crate::ids::
/// StrategyId`] included — from the retained raw trajectory, so
/// re-running this over the same trajectories rewrites byte-identical
/// rows under identical `<id>.parquet` filenames. That is what lets a
/// recorded `Run.injected_guidance` `StrategyRef`, a promoted
/// `.canon/strategies/<role>/<id>.md`, and `mart_flywheel_funnel`'s
/// `retrieved`/`applied` stages survive the ordinary `canon ingest
/// artifacts` that calls this function (`crate::ids` module doc).
pub fn rebuild_namespace(
    trajectory_store: &dyn TrajectoryStore,
    strategy_store: &dyn StrategyStore,
    regime_key: &RegimeKey,
) -> Result<Vec<StrategyItem>, LearnError> {
    let trajectories = trajectory_store.query_by_regime_key(regime_key)?;
    let existing = strategy_store.query_by_regime_key(regime_key)?;
    let prior: HashMap<_, _> = existing.into_iter().map(|item| (item.id, (item.lifecycle, item.demotion))).collect();
    strategy_store.delete_for_regime_key(regime_key)?;

    let mut items = distill_namespace(regime_key, &trajectories);
    for item in &mut items {
        if let Some((lifecycle, demotion)) = prior.get(&item.id) {
            item.lifecycle = *lifecycle;
            item.demotion = demotion.clone();
        } else {
            item.lifecycle = Some(StrategyLifecycle::Quarantined);
        }
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

    /// The acceptance property for `mart_flywheel_funnel`'s
    /// `retrieved`/`applied` stages
    /// (`crates/canon-store/sql/views.sql`): those stages inner-join a
    /// recorded `Run.injected_guidance` `StrategyRef` against the
    /// CURRENT `stg_strategy_items`, so a rebuild that re-keyed the
    /// distilled layer dropped every historical retrieval and both
    /// stages fell to zero. This test performs exactly that join —
    /// snapshot the ids, rebuild, resolve the snapshot again — so it
    /// fails on any regression back to a minted id.
    #[test]
    fn a_rebuild_preserves_every_recorded_strategy_reference() {
        let dir = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));

        trajectory_store.append(&trajectory("first task")).unwrap();
        trajectory_store.append(&trajectory("second task")).unwrap();

        // The `Run.injected_guidance` snapshot a dispatch would have
        // recorded: strategy ids, frozen at retrieval time.
        let injected: Vec<crate::ids::StrategyId> =
            rebuild_namespace(&trajectory_store, &strategy_store, &regime()).unwrap().iter().map(|i| i.id).collect();
        assert_eq!(injected.len(), 2, "fixture distilled two strategies");

        // A routine later `canon ingest artifacts` over the same regime.
        rebuild_namespace(&trajectory_store, &strategy_store, &regime()).unwrap();

        let current: Vec<crate::ids::StrategyId> = strategy_store.query_by_regime_key(&regime()).unwrap().iter().map(|i| i.id).collect();
        for id in &injected {
            assert!(current.contains(id), "recorded StrategyRef {id} no longer resolves after a rebuild — `retrieved`/`applied` would fall to 0");
        }
    }

    /// The second bug the derived id closes: `canon learn promote
    /// <ULID>` used to target a value that changed under the operator's
    /// feet, so a promoted `.canon/strategies/<role>/<id>.md` was
    /// orphaned by the next ingest and `canon learn demote` could no
    /// longer resolve its subject.
    #[test]
    fn a_promoted_strategy_id_still_resolves_after_a_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let git_tier = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));

        trajectory_store.append(&trajectory("first task")).unwrap();
        rebuild_namespace(&trajectory_store, &strategy_store, &regime()).unwrap();

        let promoted = strategy_store.query_by_regime_key(&regime()).unwrap().remove(0).id;
        let promotion = crate::promotion::promote_strategy(&strategy_store, &promoted, git_tier.path()).unwrap();
        assert!(promotion.path.exists(), "promotion wrote its git-tier file");

        rebuild_namespace(&trajectory_store, &strategy_store, &regime()).unwrap();

        assert!(
            strategy_store.find_by_id(&promoted).unwrap().is_some(),
            "the promoted id {promoted} must still name a distilled row, or its git-tier file is orphaned"
        );
        assert_eq!(
            crate::promotion::plan_promotion(&strategy_store, &promoted, git_tier.path()).unwrap().path,
            promotion.path,
            "re-promoting after a rebuild must target the same git-tier file, never a second one"
        );
    }

    /// Rebuild is a FIXPOINT over the whole row, not just over its id:
    /// no wall clock enters distillation, so unchanged evidence
    /// re-derives identical `recorded_at`, `source_trajectory_ids` and
    /// text as well. Compared on decoded rows rather than parquet
    /// bytes — this asserts canon's own determinism, never the arrow
    /// writer's encoding stability.
    #[test]
    fn rebuilding_unchanged_evidence_reproduces_identical_strategy_rows() {
        let dir = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));

        trajectory_store.append(&trajectory("first task")).unwrap();
        trajectory_store.append(&trajectory("second task")).unwrap();

        let sorted_rows = || {
            let mut rows = strategy_store.query_by_regime_key(&regime()).unwrap();
            rows.sort_by_key(|item| item.id);
            rows
        };

        rebuild_namespace(&trajectory_store, &strategy_store, &regime()).unwrap();
        let before = sorted_rows();
        assert_eq!(before.len(), 2, "fixture distilled two strategies");

        rebuild_namespace(&trajectory_store, &strategy_store, &regime()).unwrap();
        assert_eq!(before, sorted_rows());
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
