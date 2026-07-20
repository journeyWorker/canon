//! [`ParquetTrajectoryStore`]: the raw-tier `TrajectoryStore` impl —
//! operator-local parquet files under a `canon.yaml`-configured learn
//! root (`crate::config::LearnConfig::root`), one file per trajectory,
//! Hive-nested `<root>/<role>/<repo>/<area>/<hash>/<id>.parquet`
//! (`crate::store::path::namespace_dir`) — mirrors
//! `canon-store::r2_tier::R2Tier`'s "typed key columns + one JSON
//! `body` blob column, one object per write" encoding, adapted to a
//! plain local directory instead of `ObjectStore`.
//!
//! [`canon_ingest::verdict::VerdictRow`]/`Polarity`/`Becomes` carry no
//! `serde` impls (S4's frozen shape) — this module owns the ONE wire
//! mirror ([`VerdictRowWire`]) that bridges them to/from JSON, rather
//! than [`crate::trajectory::Trajectory`] itself depending on serde
//! for a type it doesn't control.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_model::ids::{RegimeKey, RoleId};
use chrono::{DateTime, Utc};
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::{Deserialize, Serialize};

use crate::error::LearnError;
use crate::ids::TrajectoryId;
use crate::store::TrajectoryStore;
use crate::store::path::namespace_dir;
use crate::trajectory::Trajectory;
use crate::verdict_outcome::{TrajectoryVerdict, VerdictOutcome};

#[derive(Debug, Serialize, Deserialize)]
struct VerdictRowWire {
    role: String,
    polarity: String,
    becomes: String,
}

impl VerdictRowWire {
    fn from_verdict_row(v: &VerdictRow) -> Self {
        Self { role: v.role.as_str().to_string(), polarity: v.polarity.as_str().to_string(), becomes: v.becomes.as_str().to_string() }
    }

    fn into_verdict_row(self) -> Result<VerdictRow, LearnError> {
        let role = RoleId::parse(self.role).map_err(LearnError::from)?;
        let polarity = match self.polarity.as_str() {
            "failure" => Polarity::Failure,
            "success" => Polarity::Success,
            "corrective" => Polarity::Corrective,
            other => return Err(LearnError::MalformedRow(format!("unknown VerdictRow polarity {other:?}"))),
        };
        let becomes = match self.becomes.as_str() {
            "guardrail candidate" => Becomes::GuardrailCandidate,
            "strategy candidate" => Becomes::StrategyCandidate,
            "guardrail (what the sample caught)" => Becomes::GuardrailWhatTheSampleCaught,
            other => return Err(LearnError::MalformedRow(format!("unknown VerdictRow becomes {other:?}"))),
        };
        Ok(VerdictRow { role, polarity, becomes })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TrajectoryWire {
    id: String,
    regime_key: String,
    task: String,
    context: String,
    verdicts: Vec<VerdictRowWire>,
    recorded_at: DateTime<Utc>,
    tags: Vec<String>,
    /// The S7-level rolled-up outcome (design D2's `TrajectoryVerdict`).
    /// `#[serde(default)]` is load-bearing: an S6-era row written BEFORE
    /// this field existed carries no `outcome`/`reward` keys in its
    /// JSON `body` at all, and MUST keep deserializing — missing
    /// `outcome` reads as `None`, which [`TrajectoryWire::into_trajectory`]
    /// folds to `TrajectoryVerdict::pending()`, exactly the state an
    /// S6-era row implicitly had (no verdict write-back existed yet).
    #[serde(default)]
    outcome: Option<VerdictOutcome>,
    /// Same backward-compat contract as `outcome` — missing `reward`
    /// reads as `None`, not `0.0`.
    #[serde(default)]
    reward: Option<f64>,
}

impl TrajectoryWire {
    fn from_trajectory(t: &Trajectory) -> Self {
        Self {
            id: t.id.to_string(),
            regime_key: t.regime_key.as_str().to_string(),
            task: t.task.clone(),
            context: t.context.clone(),
            verdicts: t.verdicts.iter().map(VerdictRowWire::from_verdict_row).collect(),
            recorded_at: t.recorded_at,
            tags: t.tags.clone(),
            outcome: Some(t.verdict_record.outcome),
            reward: Some(t.verdict_record.reward),
        }
    }

    fn into_trajectory(self) -> Result<Trajectory, LearnError> {
        let id = TrajectoryId::parse(&self.id)?;
        let regime_key = RegimeKey::parse(self.regime_key).map_err(LearnError::from)?;
        let verdicts = self.verdicts.into_iter().map(VerdictRowWire::into_verdict_row).collect::<Result<Vec<_>, _>>()?;
        // Backward compat (S6-era rows with no outcome/reward at all):
        // missing outcome -> Pending; a reward without its outcome is
        // never trusted alone (only a fully-populated pair overrides
        // the Pending default).
        let verdict_record = match self.outcome {
            Some(outcome) => TrajectoryVerdict::new(outcome, self.reward.unwrap_or_else(|| outcome.default_reward())),
            None => TrajectoryVerdict::pending(),
        };
        Trajectory::new(id, regime_key, self.task, self.context, verdicts, self.recorded_at, self.tags)
            .map(|t| t.with_verdict_record(verdict_record))
            .map_err(|e| LearnError::MalformedRow(e.to_string()))
    }
}

fn arrow_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("regime_key", DataType::Utf8, false),
        Field::new("role", DataType::Utf8, false),
        Field::new("recorded_at", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),
    ]))
}

fn encode_trajectory(trajectory: &Trajectory) -> Result<Vec<u8>, LearnError> {
    let schema = arrow_schema();
    let body = serde_json::to_string(&TrajectoryWire::from_trajectory(trajectory)).map_err(|e| LearnError::Parquet(e.to_string()))?;
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec![trajectory.id.to_string()])),
        Arc::new(StringArray::from(vec![trajectory.regime_key.as_str().to_string()])),
        Arc::new(StringArray::from(vec![trajectory.regime_key.role().to_string()])),
        Arc::new(StringArray::from(vec![trajectory.recorded_at.to_rfc3339()])),
        Arc::new(StringArray::from(vec![body])),
    ];
    let batch = RecordBatch::try_new(schema.clone(), columns).map_err(|e| LearnError::Parquet(e.to_string()))?;

    let mut buf = Vec::new();
    let mut writer = ArrowWriter::try_new(&mut buf, schema, None).map_err(|e| LearnError::Parquet(e.to_string()))?;
    writer.write(&batch).map_err(|e| LearnError::Parquet(e.to_string()))?;
    writer.close().map_err(|e| LearnError::Parquet(e.to_string()))?;
    Ok(buf)
}

fn decode_trajectories(file: File) -> Result<Vec<Trajectory>, LearnError> {
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
            .ok_or_else(|| LearnError::Parquet("trajectory parquet batch missing `body` utf8 column".to_string()))?;
        for i in 0..batch.num_rows() {
            let wire: TrajectoryWire = serde_json::from_str(body_col.value(i)).map_err(|e| LearnError::MalformedRow(e.to_string()))?;
            out.push(wire.into_trajectory()?);
        }
    }
    Ok(out)
}

/// Operator-local, parquet-backed [`TrajectoryStore`] (OQ2 resolved
/// parquet-first — see `crate::store` module doc).
#[derive(Debug, Clone)]
pub struct ParquetTrajectoryStore {
    root: PathBuf,
}

impl ParquetTrajectoryStore {
    /// `root` is the resolved `<learn_root>/trajectories` directory
    /// (an ordinary local directory — no network, no credentials;
    /// mirrors `canon-store::GitTier::new`'s "works identically in a
    /// fixture tmpdir or a real consumer repo checkout").
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn file_path(&self, regime_key: &RegimeKey, id: &TrajectoryId) -> Result<PathBuf, LearnError> {
        Ok(namespace_dir(&self.root, regime_key)?.join(format!("{id}.parquet")))
    }
}

impl TrajectoryStore for ParquetTrajectoryStore {
    fn append(&self, trajectory: &Trajectory) -> Result<(), LearnError> {
        let path = self.file_path(&trajectory.regime_key, &trajectory.id)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = encode_trajectory(trajectory)?;
        fs::write(&path, bytes)?;
        Ok(())
    }

    fn query_by_regime_key(&self, regime_key: &RegimeKey) -> Result<Vec<Trajectory>, LearnError> {
        let dir = namespace_dir(&self.root, regime_key)?;
        read_trajectories_in(&dir)
    }

    fn find_by_id(&self, id: &TrajectoryId) -> Result<Option<Trajectory>, LearnError> {
        Ok(find_trajectory_file(&self.root, id)?.map(|(_, trajectory)| trajectory))
    }

    fn mark_verdict(&self, id: &TrajectoryId, verdict: TrajectoryVerdict) -> Result<(), LearnError> {
        let Some((path, mut trajectory)) = find_trajectory_file(&self.root, id)? else {
            return Err(LearnError::UnknownTrajectoryId(id.to_string()));
        };
        trajectory.verdict_record = verdict;
        let bytes = encode_trajectory(&trajectory)?;
        fs::write(&path, bytes)?;
        Ok(())
    }
}

/// Recursively finds the ONE trajectory file matching `id` under
/// `root` (Hive-nested `<role>/<repo>/<area>/<hash>/<id>.parquet` —
/// `mark_trajectory_verdict` is keyed by `trajectory_id` alone, so
/// unlike `query_by_regime_key` this walks the whole tree rather than
/// one namespace directory; a local operator store has no id->path
/// index, matching the donor's reasoning-bank in-memory verdict-write
/// linear-scan cost ("O(namespaces)"). `file_path`
/// already names every trajectory file `<id>.parquet`, so matching on
/// the file STEM finds the right file without decoding every sibling
/// first — the match is decoded once, to confirm and to return the
/// value, never twice.
fn find_trajectory_file(root: &Path, id: &TrajectoryId) -> Result<Option<(PathBuf, Trajectory)>, LearnError> {
    let target_stem = id.to_string();
    find_trajectory_file_in(root, &target_stem)
}

fn find_trajectory_file_in(dir: &Path, target_stem: &str) -> Result<Option<(PathBuf, Trajectory)>, LearnError> {
    if !dir.is_dir() {
        return Ok(None);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_trajectory_file_in(&path, target_stem)? {
                return Ok(Some(found));
            }
        } else if path.file_stem().and_then(|s| s.to_str()) == Some(target_stem) {
            let mut decoded = decode_trajectories(File::open(&path)?)?;
            if let Some(trajectory) = decoded.pop() {
                return Ok(Some((path, trajectory)));
            }
        }
    }
    Ok(None)
}

fn read_trajectories_in(dir: &Path) -> Result<Vec<Trajectory>, LearnError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "parquet") {
            out.extend(decode_trajectories(File::open(&path)?)?);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use canon_ingest::verdict::{Becomes, Polarity};

    use super::*;

    fn regime(role: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(role, "repo", "auth", "abc123")).unwrap()
    }

    fn trajectory(role: &str, task: &str) -> Trajectory {
        let verdict = VerdictRow { role: RoleId::parse(role).unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
        Trajectory::new(TrajectoryId::new(), regime(role), task, "ctx", vec![verdict], Utc::now(), vec!["fixture".to_string()]).unwrap()
    }

    #[test]
    fn append_then_query_round_trips_by_regime_key() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let t = trajectory("dev", "fix the bug");
        store.append(&t).unwrap();

        let found = store.query_by_regime_key(&regime("dev")).unwrap();
        assert_eq!(found, vec![t]);
    }

    #[test]
    fn a_different_regime_key_never_sees_another_regimes_trajectories() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        store.append(&trajectory("dev", "dev task")).unwrap();
        store.append(&trajectory("content", "content task")).unwrap();

        assert_eq!(store.query_by_regime_key(&regime("dev")).unwrap().len(), 1);
        assert_eq!(store.query_by_regime_key(&regime("content")).unwrap().len(), 1);
    }

    #[test]
    fn querying_an_empty_namespace_returns_an_empty_vec_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        assert_eq!(store.query_by_regime_key(&regime("dev")).unwrap(), Vec::new());
    }

    #[test]
    fn encoding_the_same_trajectory_twice_is_byte_deterministic() {
        let t = trajectory("dev", "same task");
        assert_eq!(encode_trajectory(&t).unwrap(), encode_trajectory(&t).unwrap());
    }

    #[test]
    fn appending_two_trajectories_under_the_same_regime_key_keeps_both() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        store.append(&trajectory("dev", "first")).unwrap();
        store.append(&trajectory("dev", "second")).unwrap();
        assert_eq!(store.query_by_regime_key(&regime("dev")).unwrap().len(), 2);
    }

    #[test]
    fn find_by_id_locates_a_trajectory_without_knowing_its_regime_key() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let t = trajectory("dev", "findme");
        store.append(&t).unwrap();
        assert_eq!(store.find_by_id(&t.id).unwrap(), Some(t));
    }

    #[test]
    fn find_by_id_returns_none_for_an_unknown_id_never_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        assert_eq!(store.find_by_id(&TrajectoryId::new()).unwrap(), None);
    }

    #[test]
    fn mark_verdict_rewrites_only_the_matching_trajectorys_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let a = trajectory("dev", "a");
        let b = trajectory("dev", "b");
        store.append(&a).unwrap();
        store.append(&b).unwrap();

        store.mark_verdict(&a.id, TrajectoryVerdict::new(VerdictOutcome::Success, 0.9)).unwrap();

        let found_a = store.find_by_id(&a.id).unwrap().unwrap();
        let found_b = store.find_by_id(&b.id).unwrap().unwrap();
        assert_eq!(found_a.verdict_record, TrajectoryVerdict::new(VerdictOutcome::Success, 0.9));
        assert!(found_b.verdict_record.is_pending(), "sibling trajectory's file must be untouched");
    }

    #[test]
    fn mark_verdict_on_an_unknown_id_is_an_error_not_a_silent_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let err = store.mark_verdict(&TrajectoryId::new(), TrajectoryVerdict::pending()).unwrap_err();
        assert!(matches!(err, LearnError::UnknownTrajectoryId(_)));
    }

    /// Hand-builds an S6-era trajectory parquet file — the exact JSON
    /// `body` shape `TrajectoryWire` produced BEFORE this change added
    /// `outcome`/`reward` (no such keys at all), through the SAME
    /// arrow/parquet encoding `encode_trajectory` uses. This is the only
    /// way to construct a truly pre-S7 row in a test, since the current
    /// `TrajectoryWire`/`encode_trajectory` always writes both fields
    /// now.
    fn encode_legacy_trajectory(t: &Trajectory) -> Vec<u8> {
        let schema = arrow_schema();
        let legacy_body = serde_json::json!({
            "id": t.id.to_string(),
            "regime_key": t.regime_key.as_str().to_string(),
            "task": t.task,
            "context": t.context,
            "verdicts": t.verdicts.iter().map(VerdictRowWire::from_verdict_row).collect::<Vec<_>>(),
            "recorded_at": t.recorded_at,
            "tags": t.tags,
            // Deliberately NO "outcome"/"reward" keys — this IS the
            // S6-era shape the backward-compat contract must still read.
        });
        let body = serde_json::to_string(&legacy_body).unwrap();
        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(vec![t.id.to_string()])),
            Arc::new(StringArray::from(vec![t.regime_key.as_str().to_string()])),
            Arc::new(StringArray::from(vec![t.regime_key.role().to_string()])),
            Arc::new(StringArray::from(vec![t.recorded_at.to_rfc3339()])),
            Arc::new(StringArray::from(vec![body])),
        ];
        let batch = RecordBatch::try_new(schema.clone(), columns).unwrap();
        let mut buf = Vec::new();
        let mut writer = ArrowWriter::try_new(&mut buf, schema, None).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
        buf
    }

    /// The locking test guardrails require: (1) an S6-era row (no
    /// outcome/reward) reads back cleanly as `Pending`; (2) it can THEN
    /// be marked via `mark_trajectory_verdict`; (3) a subsequent
    /// `rebuild_namespace` leaves the raw trajectory bytes
    /// byte-identical to what `mark_trajectory_verdict` just wrote —
    /// `mark_trajectory_verdict` is the ONLY path allowed to rewrite a
    /// trajectory file; `rebuild_namespace` never does (it only reads
    /// trajectories and deletes/re-derives the SEPARATE strategy tier).
    #[test]
    fn s6_era_row_reads_pending_can_be_marked_and_rebuild_never_touches_it_afterward() {
        let dir = tempfile::tempdir().unwrap();
        let traj_root = dir.path().join("trajectories");
        let trajectory_store = ParquetTrajectoryStore::open(&traj_root);
        let strategy_store = crate::store::ParquetStrategyStore::open(dir.path().join("strategies"));

        let t = trajectory("dev", "legacy task");
        let path = trajectory_store.file_path(&t.regime_key, &t.id).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, encode_legacy_trajectory(&t)).unwrap();

        // (1) S6-era row reads back cleanly, as Pending.
        let found = trajectory_store.find_by_id(&t.id).unwrap().unwrap();
        assert!(found.verdict_record.is_pending(), "an S6-era row with no outcome/reward keys must read back Pending, not error");
        assert_eq!(found.verdict_record.reward, 0.5);

        // (2) It can then be marked via mark_trajectory_verdict.
        crate::mark_verdict::mark_trajectory_verdict(&trajectory_store, &t.id, VerdictOutcome::Success, 0.9).unwrap();
        let marked = trajectory_store.find_by_id(&t.id).unwrap().unwrap();
        assert_eq!(marked.verdict_record, TrajectoryVerdict::new(VerdictOutcome::Success, 0.9));
        let bytes_after_mark = fs::read(&path).unwrap();

        // (3) rebuild_namespace must leave those bytes byte-identical.
        crate::rebuild::rebuild_namespace(&trajectory_store, &strategy_store, &t.regime_key).unwrap();
        let bytes_after_rebuild = fs::read(&path).unwrap();
        assert_eq!(
            bytes_after_mark, bytes_after_rebuild,
            "rebuild_namespace must never rewrite trajectory bytes mark_trajectory_verdict just wrote"
        );
    }
}
