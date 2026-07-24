//! [`Roots`]: the three rebindable physical-source roots
//! `canon_store::VIEWS_SQL` reads (`crates/canon-store/sql/views.sql`'s
//! own module header) — `canon-report`'s query driver ([`crate::query`])
//! sets these as env vars before every `duckdb -init` invocation, the
//! exact `CANON_GIT_ROOT`/`CANON_R2_ROOT` pattern
//! `crates/canon-store/tests/e2e_write_age_query_duckdb.rs` already
//! established, plus S9's own `CANON_LEARN_ROOT` addition.
//!
//! # Empty-glob seeding
//! DuckDB's `read_parquet(...)` (unlike `read_text(...)`, which
//! `stg_git_records` uses) hard-errors when its glob matches ZERO
//! files — verified against `sql/views.sql` directly, 2026-07-11: a
//! completely empty `CANON_R2_ROOT`/`CANON_LEARN_ROOT` (a fresh repo
//! that has never routed a record to r2, or never distilled a
//! strategy/trajectory yet — both entirely normal early states, not
//! error conditions) would otherwise abort EVERY query, including ones
//! that never touch those views. [`Roots::ensure_seeded`] writes one
//! zero-row placeholder parquet file per glob root, at the EXACT
//! directory depth each source's own `read_parquet` glob expects
//! (`sql/views.sql`'s `stg_r2_records` — `kind=*/**/*.parquet`, any
//! depth under one `kind=` level; `stg_strategy_items`/
//! `stg_trajectories` — `strategies|trajectories/*/*/*/*/*.parquet`,
//! exactly FOUR directory levels — `role/repo/area/hash`,
//! `canon_learn::store::path::namespace_dir`'s own shape — before the
//! file, mirroring `crates/canon-store/tests/
//! e2e_write_age_query_duckdb.rs`'s own `_seed/_seed/_seed/_seed/
//! _seed.parquet` precedent). A seed at the WRONG depth still leaves
//! the glob matching zero files — the exact "abort EVERY query" this
//! module doc calls out, just deferred to first use — so the glob
//! always resolves, mirroring this codebase's
//! established "fail-soft on an empty/missing corpus, never crash"
//! posture (`canon_policy::PolicyResolution`'s `Missing`-diagnostic
//! precedent, `GateContext::load`'s "still resolves a full surface").
//! Never touches `CANON_GIT_ROOT` (`read_text` already tolerates zero
//! matches, confirmed by direct `duckdb` experimentation).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{ArrayRef, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

use crate::error::ReportError;

/// The `kind`/`id` marker every seeded placeholder row set carries —
/// never a value any real record/strategy/trajectory kind can produce
/// (every real one is a lowercase `RecordKind::as_str()` or a real
/// `StrategyId`/`TrajectoryId`, neither of which starts with `_`).
const SEED_MARKER: &str = "_canon_report_seed";

/// The three physical-source roots [`canon_store::VIEWS_SQL`] reads.
#[derive(Debug, Clone)]
pub struct Roots {
    /// `canon.yaml`'s `tiers.git.root` — the git tier's Hive-laid-out
    /// `kind=*/**/*.json` directory.
    pub git_root: PathBuf,
    /// `canon.yaml`'s `tiers.r2.*` — the r2 tier's local (or synced)
    /// `kind=*/**/*.parquet` directory.
    pub r2_root: PathBuf,
    /// `canon-learn`'s `DEFAULT_LEARN_ROOT` (`<repo>/.canon/learn`) —
    /// the parent of its `strategies/`/`trajectories/` parquet stores.
    pub learn_root: PathBuf,
}

fn r2_schema() -> Arc<Schema> {
    // Mirrors `canon_store::r2_tier`'s own `arrow_schema()` exactly
    // (verified against `crates/canon-store/src/r2_tier.rs`,
    // 2026-07-11) — a seeded row must satisfy `stg_r2_records`'s
    // column expectations, not just "some parquet file".
    Arc::new(Schema::new(vec![
        Field::new("kind", DataType::Utf8, false),
        Field::new("natural_key", DataType::Utf8, false),
        Field::new("at", DataType::Utf8, false),
        Field::new("digest", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),
    ]))
}

fn learn_store_schema() -> Arc<Schema> {
    // Mirrors `canon_learn::store::{parquet_strategy,parquet_trajectory}`'s
    // shared `arrow_schema()` exactly (verified against
    // `crates/canon-learn/src/store/parquet_{strategy,trajectory}.rs`,
    // 2026-07-11) — both stores use the identical 5-column shape.
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("regime_key", DataType::Utf8, false),
        Field::new("role", DataType::Utf8, false),
        Field::new("recorded_at", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),
    ]))
}

/// Recursively checks whether `dir` contains at least one `*.parquet`
/// file — a plain walk, not a full glob match, since every seeding
/// decision here only needs "is this source truly empty", not a
/// precise re-implementation of DuckDB's own glob semantics.
fn has_any_parquet(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else { return false };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if has_any_parquet(&path) {
                return true;
            }
        } else if path.extension().is_some_and(|ext| ext == "parquet") {
            return true;
        }
    }
    false
}

fn write_seed_parquet(path: &Path, schema: Arc<Schema>, columns: Vec<ArrayRef>) -> Result<(), ReportError> {
    fs::create_dir_all(path.parent().expect("seed path always has a parent"))?;
    let batch = RecordBatch::try_new(schema.clone(), columns).map_err(|e| ReportError::Seed(e.to_string()))?;
    let file = fs::File::create(path)?;
    let mut writer = ArrowWriter::try_new(file, schema, None).map_err(|e| ReportError::Seed(e.to_string()))?;
    writer.write(&batch).map_err(|e| ReportError::Seed(e.to_string()))?;
    writer.close().map_err(|e| ReportError::Seed(e.to_string()))?;
    Ok(())
}

impl Roots {
    pub fn new(git_root: impl Into<PathBuf>, r2_root: impl Into<PathBuf>, learn_root: impl Into<PathBuf>) -> Self {
        Self { git_root: git_root.into(), r2_root: r2_root.into(), learn_root: learn_root.into() }
    }

    /// Creates every root directory (idempotent — a real, already-
    /// populated repo checkout is left untouched beyond `mkdir -p`)
    /// and seeds a zero-row placeholder parquet file into any
    /// `read_parquet`-backed source (`r2_root`, `learn_root/
    /// strategies`, `learn_root/trajectories`) that currently has NONE
    /// — module doc's "empty-glob seeding" rationale. Zero-row means
    /// the seed contributes NOTHING to any mart's aggregate output; it
    /// exists purely so `read_parquet`'s glob resolves at all.
    pub fn ensure_seeded(&self) -> Result<(), ReportError> {
        fs::create_dir_all(&self.git_root)?;

        if !has_any_parquet(&self.r2_root) {
            let empty: ArrayRef = Arc::new(StringArray::from(Vec::<&str>::new()));
            write_seed_parquet(
                &self.r2_root.join(format!("kind={SEED_MARKER}")).join(format!("{SEED_MARKER}.parquet")),
                r2_schema(),
                vec![empty.clone(), empty.clone(), empty.clone(), empty.clone(), empty],
            )?;
        }

        let strategies_root = self.learn_root.join("strategies");
        if !has_any_parquet(&strategies_root) {
            let empty: ArrayRef = Arc::new(StringArray::from(Vec::<&str>::new()));
            write_seed_parquet(
                &strategies_root.join(SEED_MARKER).join(SEED_MARKER).join(SEED_MARKER).join(SEED_MARKER).join(format!("{SEED_MARKER}.parquet")),
                learn_store_schema(),
                vec![empty.clone(), empty.clone(), empty.clone(), empty.clone(), empty],
            )?;
        }

        let trajectories_root = self.learn_root.join("trajectories");
        if !has_any_parquet(&trajectories_root) {
            let empty: ArrayRef = Arc::new(StringArray::from(Vec::<&str>::new()));
            write_seed_parquet(
                &trajectories_root.join(SEED_MARKER).join(SEED_MARKER).join(SEED_MARKER).join(SEED_MARKER).join(format!("{SEED_MARKER}.parquet")),
                learn_store_schema(),
                vec![empty.clone(), empty.clone(), empty.clone(), empty.clone(), empty],
            )?;
        }

        Ok(())
    }

    /// The `(env var, value)` pairs [`crate::query::run_query`] sets
    /// before every `duckdb` invocation.
    pub fn env_pairs(&self) -> [(&'static str, String); 3] {
        [
            (canon_store::CANON_GIT_ROOT_ENV, self.git_root.display().to_string()),
            (canon_store::CANON_R2_ROOT_ENV, self.r2_root.display().to_string()),
            (canon_store::CANON_LEARN_ROOT_ENV, self.learn_root.display().to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_seeded_creates_matching_glob_files_for_every_empty_parquet_source() {
        let dir = tempfile::tempdir().unwrap();
        let roots = Roots::new(dir.path().join("git"), dir.path().join("r2"), dir.path().join("learn"));
        roots.ensure_seeded().unwrap();

        assert!(has_any_parquet(&roots.r2_root), "r2 root must be seeded when empty");
        assert!(has_any_parquet(&roots.learn_root.join("strategies")), "strategies store must be seeded when empty");
        assert!(has_any_parquet(&roots.learn_root.join("trajectories")), "trajectories store must be seeded when empty");
    }

    #[test]
    fn ensure_seeded_never_touches_an_already_populated_source() {
        let dir = tempfile::tempdir().unwrap();
        let roots = Roots::new(dir.path().join("git"), dir.path().join("r2"), dir.path().join("learn"));
        let real = roots.r2_root.join("kind=change").join("real.parquet");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        write_seed_parquet(&real, r2_schema(), {
            let one: ArrayRef = Arc::new(StringArray::from(vec!["x"]));
            vec![one.clone(), one.clone(), one.clone(), one.clone(), one]
        })
        .unwrap();

        roots.ensure_seeded().unwrap();

        // Only the one real file — no seed marker written alongside it.
        let mut names = Vec::new();
        for entry in walk(&roots.r2_root) {
            names.push(entry.file_name().unwrap().to_string_lossy().to_string());
        }
        assert_eq!(names, vec!["real.parquet"], "an already-populated source must not be seeded");
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
        out
    }
}
