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
//! that never touch those views. [`Roots::seeded_in`] writes one
//! zero-row placeholder parquet file per glob root into a per-invocation
//! scratch directory (a report never writes into the repo it reads), at the EXACT
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
//!
//! # The sqlite hot tier (0.18, #29)
//! `canon init` routes `task`/`handoff`/`session`/`run`/`event` to a
//! sqlite `hot` rung. Its db is a local file that needs no credential,
//! so the report reads it: [`Roots::hot_db`] names the file
//! ([`hot_db_from_canon_yaml`]), and [`Roots::materialize_hot`] writes
//! every `records_history` row as one git-shaped JSON file under a
//! per-invocation scratch directory the query driver points
//! `CANON_HOT_ROOT` at (`stg_hot_records`). DuckDB's own sqlite
//! extension would be fetched over the network on first use, which a
//! report must never do; this read goes through canon-store's
//! `SqliteTier`, the same reader `canon query` uses. A db file that does
//! not exist yet is no rows, never created.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{ArrayRef, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

use canon_model::envelope::RecordKind;
use canon_store::partition::content_digest;
use canon_store::policy::{BackendConfig, TierPolicy};
use canon_store::sqlite_tier::SqliteTier;
use canon_store::tier::{Tier as _, TierQuery};

use crate::error::ReportError;

/// The `kind`/`id` marker every seeded placeholder row set carries —
/// never a value any real record/strategy/trajectory kind can produce
/// (every real one is a lowercase `RecordKind::as_str()` or a real
/// `StrategyId`/`TrajectoryId`, neither of which starts with `_`).
const SEED_MARKER: &str = "_canon_report_seed";

/// The physical-source roots [`canon_store::VIEWS_SQL`] reads.
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
    /// The sqlite hot tier's db file, when `canon.yaml` backs a rung
    /// with sqlite (module doc, 0.18 #29). `None` reads no hot rows.
    pub hot_db: Option<PathBuf>,
}

/// The sqlite db file `<repo_root>/canon.yaml` backs a rung with,
/// resolved against the file's own directory
/// ([`TierPolicy::from_yaml_at`]). `None` when the file is absent,
/// does not parse, or backs no rung with sqlite — fail-soft like
/// [`crate::tier_boundary::kinds_not_read_directly`], since a broken
/// `canon.yaml` is `canon gate check`'s to report.
pub fn hot_db_from_canon_yaml(repo_root: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(repo_root.join("canon.yaml")).ok()?;
    let policy = TierPolicy::from_yaml_at(&text, repo_root).ok()?;
    let mut paths: Vec<PathBuf> = policy
        .tiers
        .values()
        .filter_map(|cfg| match cfg {
            BackendConfig::Sqlite(sqlite) => Some(sqlite.path.clone()),
            _ => None,
        })
        .collect();
    paths.sort();
    paths.into_iter().next()
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

/// `link` as a directory symlink to `target` (a copy where the platform has none).
fn link_dir(target: &Path, link: &Path) -> Result<(), ReportError> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
    };
    #[cfg(not(unix))]
    {
        fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
            fs::create_dir_all(to)?;
            for entry in fs::read_dir(from)? {
                let entry = entry?;
                let dest = to.join(entry.file_name());
                if entry.path().is_dir() { copy(&entry.path(), &dest)? } else { fs::copy(entry.path(), dest)?; }
            }
            Ok(())
        }
        copy(target, link)?;
    }
    Ok(())
}

fn write_seed_parquet(path: &Path, schema: Arc<Schema>, columns: Vec<ArrayRef>) -> Result<(), ReportError> {
    fs::create_dir_all(path.parent().expect("seed path always has a parent"))?;
    let batch = RecordBatch::try_new(Arc::clone(&schema), columns).map_err(|e| ReportError::Seed(e.to_string()))?;
    let file = fs::File::create(path)?;
    let mut writer = ArrowWriter::try_new(file, schema, None).map_err(|e| ReportError::Seed(e.to_string()))?;
    writer.write(&batch).map_err(|e| ReportError::Seed(e.to_string()))?;
    writer.close().map_err(|e| ReportError::Seed(e.to_string()))?;
    Ok(())
}

impl Roots {
    pub fn new(git_root: impl Into<PathBuf>, r2_root: impl Into<PathBuf>, learn_root: impl Into<PathBuf>) -> Self {
        Self { git_root: git_root.into(), r2_root: r2_root.into(), learn_root: learn_root.into(), hot_db: None }
    }

    /// These roots, also reading the sqlite hot tier at `hot_db`.
    pub fn with_hot_db(mut self, hot_db: Option<PathBuf>) -> Self {
        self.hot_db = hot_db;
        self
    }

    /// Writes every row of the sqlite hot tier ([`Self::hot_db`]) under
    /// `dir` as `kind=<kind>/<n>_<digest8>.json`, the layout
    /// `stg_hot_records` reads. The file text is the body as canon writes a
    /// ledger record (sorted-key pretty JSON), so the views' own
    /// `canon_body_digest` of it is [`content_digest`] of the body, the
    /// value a git record has, and the two tiers fold against each other.
    /// The name is for humans; the views never read the digest from it. Nothing is
    /// written when there is no hot db or nothing exists at its path yet (as `canon query` reads it):
    /// opening a missing db would create it. Anything that does exist there
    /// is opened as a SQLite file, so a directory or a file that is no
    /// database makes the hot tier unavailable ([`ReportError::HotTier`],
    /// naming the path) instead of reading as no hot records.
    pub fn materialize_hot(&self, dir: &Path) -> Result<(), ReportError> {
        let Some(db) = self.hot_db.as_deref().filter(|db| fs::symlink_metadata(db).is_ok()) else {
            return Ok(());
        };
        let hot_error = |source| ReportError::HotTier { path: db.to_path_buf(), source };
        let tier = SqliteTier::connect(db).map_err(hot_error)?;
        for kind in RecordKind::ALL {
            let rows = tier.read(&TierQuery::kind(kind)).map_err(hot_error)?.records;
            if rows.is_empty() {
                continue;
            }
            let kind_dir = dir.join(format!("kind={}", kind.as_str()));
            fs::create_dir_all(&kind_dir)?;
            for (n, raw) in rows.iter().enumerate() {
                let body = serde_json::to_vec_pretty(&raw.0)?;
                fs::write(kind_dir.join(format!("{n}_{}.json", content_digest(&raw.0))), body)?;
            }
        }
        Ok(())
    }

    /// These roots with every source a `read_parquet`/`read_json` glob
    /// needs but the repo does not have yet replaced by a seeded copy under
    /// `scratch` — module doc's "empty-glob seeding" rationale, without
    /// touching the repo: a report only reads, so an empty r2/learn source
    /// (or a missing ledger directory) is answered from `scratch`, which
    /// the caller removes after the query.
    ///
    /// A source that already holds a `*.parquet` file (or a ledger
    /// directory that exists) is used as it is. Where only one of the
    /// learn stores is empty, the other is linked into the scratch learn
    /// root rather than copied. A zero-row seed contributes NOTHING to any
    /// mart's aggregate output; it exists purely so the glob resolves.
    pub fn seeded_in(&self, scratch: &Path) -> Result<Roots, ReportError> {
        let mut roots = self.clone();
        if !self.git_root.is_dir() {
            roots.git_root = scratch.join("ledger");
            fs::create_dir_all(&roots.git_root)?;
        }

        if !has_any_parquet(&self.r2_root) {
            roots.r2_root = scratch.join("r2");
            let empty: ArrayRef = Arc::new(StringArray::from(Vec::<&str>::new()));
            write_seed_parquet(
                &roots.r2_root.join(format!("kind={SEED_MARKER}")).join(format!("{SEED_MARKER}.parquet")),
                r2_schema(),
                vec![Arc::clone(&empty), Arc::clone(&empty), Arc::clone(&empty), Arc::clone(&empty), empty],
            )?;
        }

        let stores = ["strategies", "trajectories"];
        if stores.iter().any(|store| !has_any_parquet(&self.learn_root.join(store))) {
            roots.learn_root = scratch.join("learn");
            for store in stores {
                let real = self.learn_root.join(store);
                let seeded = roots.learn_root.join(store);
                if has_any_parquet(&real) {
                    fs::create_dir_all(&roots.learn_root)?;
                    link_dir(&real, &seeded)?;
                } else {
                    let empty: ArrayRef = Arc::new(StringArray::from(Vec::<&str>::new()));
                    write_seed_parquet(
                        &seeded.join(SEED_MARKER).join(SEED_MARKER).join(SEED_MARKER).join(SEED_MARKER).join(format!("{SEED_MARKER}.parquet")),
                        learn_store_schema(),
                        vec![Arc::clone(&empty), Arc::clone(&empty), Arc::clone(&empty), Arc::clone(&empty), empty],
                    )?;
                }
            }
        }
        Ok(roots)
    }

    /// The `(env var, value)` pairs [`crate::query::run_query`] sets
    /// before every `duckdb` invocation. The archive root rides along
    /// when the git root is a ledger directory with an archive beside it
    /// (`canon_store::archive::archive_root_of`); without one the views
    /// read no archive.
    pub fn env_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = vec![
            (canon_store::CANON_GIT_ROOT_ENV, self.git_root.display().to_string()),
            (canon_store::CANON_R2_ROOT_ENV, self.r2_root.display().to_string()),
            (canon_store::CANON_LEARN_ROOT_ENV, self.learn_root.display().to_string()),
        ];
        if let Some(archive) = canon_store::archive::archive_root_of(&self.git_root) {
            pairs.push((canon_store::CANON_ARCHIVE_ROOT_ENV, archive.display().to_string()));
        }
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_in_replaces_every_empty_parquet_source_with_a_seeded_scratch_copy_and_leaves_the_repo_alone() {
        let dir = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let roots = Roots::new(dir.path().join("git"), dir.path().join("r2"), dir.path().join("learn"));
        let seeded = roots.seeded_in(scratch.path()).unwrap();

        assert!(has_any_parquet(&seeded.r2_root), "r2 root must be seeded when empty");
        assert!(has_any_parquet(&seeded.learn_root.join("strategies")), "strategies store must be seeded when empty");
        assert!(has_any_parquet(&seeded.learn_root.join("trajectories")), "trajectories store must be seeded when empty");
        assert!(seeded.git_root.is_dir(), "a missing ledger directory is answered by an empty one");
        assert!(fs::read_dir(dir.path()).unwrap().next().is_none(), "nothing is written under the repo roots");
    }

    #[test]
    fn seeded_in_uses_an_already_populated_source_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let roots = Roots::new(dir.path().join("git"), dir.path().join("r2"), dir.path().join("learn"));
        let real = roots.r2_root.join("kind=change").join("real.parquet");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        write_seed_parquet(&real, r2_schema(), {
            let one: ArrayRef = Arc::new(StringArray::from(vec!["x"]));
            vec![Arc::clone(&one), Arc::clone(&one), Arc::clone(&one), Arc::clone(&one), one]
        })
        .unwrap();

        let seeded = roots.seeded_in(scratch.path()).unwrap();

        assert_eq!(seeded.r2_root, roots.r2_root, "a populated source is not replaced");
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
