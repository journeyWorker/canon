//! [`ParquetStrategyStore`]: the distilled-tier `StrategyStore` impl —
//! same operator-local, Hive-nested, one-file-per-row shape as
//! [`crate::store::parquet_trajectory::ParquetTrajectoryStore`], under
//! a sibling `<learn_root>/strategies` directory. Unlike the raw
//! store, this one supports [`StrategyStore::delete_for_regime_key`] —
//! the ONLY deletion this crate's two stores ever perform, and only
//! ever on this (distilled) tier (design decision 3).
//!
//! [`crate::strategy::StrategyItem`] is directly `Serialize`/
//! `Deserialize` (no non-serde external type embedded, unlike
//! `Trajectory`'s `VerdictRow`), so this module's encode/decode is a
//! straight JSON-body round trip — no wire-mirror type needed.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use canon_model::ids::RegimeKey;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::error::LearnError;
use crate::ids::StrategyId;
use crate::store::StrategyStore;
use crate::store::path::namespace_dir;
use crate::strategy::{DemotionEvidence, StrategyItem};

fn arrow_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("regime_key", DataType::Utf8, false),
        Field::new("role", DataType::Utf8, false),
        Field::new("recorded_at", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),
    ]))
}

fn encode_strategy(item: &StrategyItem) -> Result<Vec<u8>, LearnError> {
    let schema = arrow_schema();
    let body = serde_json::to_string(item).map_err(|e| LearnError::Parquet(e.to_string()))?;
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec![item.id.to_string()])),
        Arc::new(StringArray::from(vec![item.regime_key.as_str().to_string()])),
        Arc::new(StringArray::from(vec![item.regime_key.role().to_string()])),
        Arc::new(StringArray::from(vec![item.recorded_at.to_rfc3339()])),
        Arc::new(StringArray::from(vec![body])),
    ];
    let batch = RecordBatch::try_new(schema.clone(), columns).map_err(|e| LearnError::Parquet(e.to_string()))?;

    let mut buf = Vec::new();
    let mut writer = ArrowWriter::try_new(&mut buf, schema, None).map_err(|e| LearnError::Parquet(e.to_string()))?;
    writer.write(&batch).map_err(|e| LearnError::Parquet(e.to_string()))?;
    writer.close().map_err(|e| LearnError::Parquet(e.to_string()))?;
    Ok(buf)
}

fn decode_strategies(file: File) -> Result<Vec<StrategyItem>, LearnError> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| LearnError::Parquet(e.to_string()))?
        .build()
        .map_err(|e| LearnError::Parquet(e.to_string()))?;

    let mut out = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|e| LearnError::Parquet(e.to_string()))?;
        let body_col = batch
            .column_by_name("body")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| LearnError::Parquet("strategy parquet batch missing `body` utf8 column".to_string()))?;
        for i in 0..batch.num_rows() {
            let item: StrategyItem = serde_json::from_str(body_col.value(i)).map_err(|e| LearnError::MalformedRow(e.to_string()))?;
            out.push(item);
        }
    }
    Ok(out)
}

/// Operator-local, parquet-backed [`StrategyStore`].
#[derive(Debug, Clone)]
pub struct ParquetStrategyStore {
    root: PathBuf,
}

impl ParquetStrategyStore {
    /// `root` is the resolved `<learn_root>/strategies` directory.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl StrategyStore for ParquetStrategyStore {
    fn append(&self, item: &StrategyItem) -> Result<(), LearnError> {
        if item.role.as_str() != item.regime_key.role() {
            return Err(LearnError::StrategyRoleMismatch {
                item_role: item.role.as_str().to_string(),
                regime_role: item.regime_key.role().to_string(),
            });
        }
        let dir = namespace_dir(&self.root, &item.regime_key)?;
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.parquet", item.id));
        fs::write(&path, encode_strategy(item)?)?;
        Ok(())
    }

    fn query_by_regime_key(&self, regime_key: &RegimeKey) -> Result<Vec<StrategyItem>, LearnError> {
        let dir = namespace_dir(&self.root, regime_key)?;
        read_strategies_in(&dir)
    }

    fn delete_for_regime_key(&self, regime_key: &RegimeKey) -> Result<usize, LearnError> {
        let dir = namespace_dir(&self.root, regime_key)?;
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut deleted = 0usize;
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "parquet") {
                fs::remove_file(&path)?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    fn find_by_id(&self, id: &StrategyId) -> Result<Option<StrategyItem>, LearnError> {
        Ok(find_strategy_file(&self.root, id)?.map(|(_, item)| item))
    }

    fn mark_demoted(&self, id: &StrategyId, demotion: DemotionEvidence) -> Result<(), LearnError> {
        let Some((path, item)) = find_strategy_file(&self.root, id)? else {
            return Err(LearnError::UnknownStrategyId(id.to_string()));
        };
        let item = item.with_demotion(demotion);
        fs::write(&path, encode_strategy(&item)?)?;
        Ok(())
    }
}

/// Recursively finds the ONE strategy file matching `id` under `root`
/// (Hive-nested `<role>/<repo>/<area>/<hash>/<id>.parquet` —
/// `find_by_id`/`mark_demoted` are keyed by `strategy_id` alone,
/// mirroring `parquet_trajectory::find_trajectory_file`'s exact
/// walk-the-whole-tree rationale: this store has no id->path index).
fn find_strategy_file(root: &Path, id: &StrategyId) -> Result<Option<(PathBuf, StrategyItem)>, LearnError> {
    find_strategy_file_in(root, &id.to_string())
}

fn find_strategy_file_in(dir: &Path, target_stem: &str) -> Result<Option<(PathBuf, StrategyItem)>, LearnError> {
    if !dir.is_dir() {
        return Ok(None);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_strategy_file_in(&path, target_stem)? {
                return Ok(Some(found));
            }
        } else if path.file_stem().and_then(|s| s.to_str()) == Some(target_stem) {
            let mut decoded = decode_strategies(File::open(&path)?)?;
            if let Some(item) = decoded.pop() {
                return Ok(Some((path, item)));
            }
        }
    }
    Ok(None)
}

fn read_strategies_in(dir: &Path) -> Result<Vec<StrategyItem>, LearnError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "parquet") {
            out.extend(decode_strategies(File::open(&path)?)?);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use canon_model::ids::RoleId;
    use chrono::Utc;

    use super::*;
    use crate::ids::{StrategyId, TrajectoryId};

    fn regime(role: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(role, "repo", "auth", "abc123")).unwrap()
    }

    fn strategy_with_role(regime_role: &str, item_role: &str, title: &str) -> StrategyItem {
        StrategyItem::new(
            StrategyId::new(),
            regime(regime_role),
            RoleId::parse(item_role).unwrap(),
            title,
            "description",
            "content",
            vec![TrajectoryId::new()],
            Utc::now(),
        )
    }

    fn strategy(role: &str, title: &str) -> StrategyItem {
        strategy_with_role(role, role, title)
    }

    #[test]
    fn append_then_query_round_trips_by_regime_key() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let item = strategy("dev", "prefer small diffs");
        store.append(&item).unwrap();
        assert_eq!(store.query_by_regime_key(&regime("dev")).unwrap(), vec![item]);
    }

    #[test]
    fn append_rejects_a_strategy_item_role_disagreeing_with_the_regime_key_role() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let smuggled = strategy_with_role("dev", "content", "smuggled content strategy");
        let err = store.append(&smuggled).unwrap_err();
        assert!(matches!(err, LearnError::StrategyRoleMismatch { .. }));
    }

    #[test]
    fn a_dev_scoped_query_never_returns_a_content_role_strategy_smuggled_in_via_a_rejected_append() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));

        // attempted cross-role smuggling: regime_key says `dev`, item.role says `content`
        let smuggled = strategy_with_role("dev", "content", "smuggled content strategy");
        assert!(store.append(&smuggled).is_err(), "cross-role write must be rejected, not silently persisted");

        // a legitimately role-agreeing item still round-trips through the same regime
        let legit = strategy("dev", "prefer small diffs");
        store.append(&legit).unwrap();

        let dev_items = store.query_by_regime_key(&regime("dev")).unwrap();
        assert_eq!(dev_items, vec![legit], "dev-scoped retrieve must never surface the rejected content-role item");
    }

    #[test]
    fn delete_for_regime_key_removes_only_that_namespace() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));
        store.append(&strategy("dev", "dev strategy")).unwrap();
        store.append(&strategy("content", "content strategy")).unwrap();

        let deleted = store.delete_for_regime_key(&regime("dev")).unwrap();
        assert_eq!(deleted, 1);
        assert!(store.query_by_regime_key(&regime("dev")).unwrap().is_empty());
        assert_eq!(store.query_by_regime_key(&regime("content")).unwrap().len(), 1, "other namespace untouched");
    }

    #[test]
    fn encoding_the_same_strategy_item_twice_is_byte_deterministic() {
        let item = strategy("dev", "prefer small diffs");
        assert_eq!(encode_strategy(&item).unwrap(), encode_strategy(&item).unwrap());
    }

    #[test]
    fn deleting_an_empty_namespace_is_a_zero_count_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));
        assert_eq!(store.delete_for_regime_key(&regime("dev")).unwrap(), 0);
    }
}
