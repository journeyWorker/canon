//! `SqliteTier`: the hot, embedded-file tier (s32 `sqlite-hot-backend`)
//! — the SAME `records_history(kind, id, at, digest, body)` append-only
//! contract [`crate::pg_tier::PgTier`] implements, over sqlx's SQLite
//! driver instead of Postgres. s28 `rung-backend-capability` design D1
//! explicitly reserved "a second live-database vendor for `hot`" —
//! this is it: `Backend::Sqlite.class() == BackendClass::LiveDb`, so
//! the s28 class check accepts it anywhere a `postgres`-backed `hot`
//! rung was accepted, no new class-check logic (`crate::policy`
//! module doc, s32 proposal.md "What Changes"). No env-var
//! indirection: a local db file carries no secret, so `canon.yaml`
//! names a `path:` directly (`crate::policy::SqliteTierConfig`,
//! already resolved to an absolute path at parse time by
//! `TierPolicy::from_yaml_at`).
//!
//! **Append-only, digest-deduped, byte-for-byte the same semantics as
//! `PgTier`** (see that module's own doc comment for the full s21
//! rationale this mirrors verbatim): `records_history`'s
//! `(kind, id, digest)` primary key makes a byte-identical
//! resubmission a no-op (`WriteReceipt.deduped = true`, `INSERT …
//! ON CONFLICT (kind, id, digest) DO NOTHING`), while a write carrying
//! a NEW digest at the same `(kind, id)` is a genuine APPEND, never an
//! in-place overwrite. [`SqliteTier::read`] never pre-folds (s21 D5)
//! — it returns EVERY historical row for a kind (optionally
//! `since`-filtered), the SAME raw, unfolded contract
//! `GitTier`/`PgTier`/`R2Tier::read` honor.
//!
//! **WAL + a 5s busy timeout, applied at connect** (s32 tasks.md 1.2,
//! spec.md "SqliteTier honors the store contract"): WAL journal mode
//! lets concurrent readers proceed alongside one writer (canon's own
//! documented "single-writer caveat" — heavy multi-agent concurrent
//! WRITE load is what the Postgres swap is for, proposal.md), and the
//! busy timeout makes a transient writer-lock contention RETRY
//! instead of failing immediately with `SQLITE_BUSY`.
//!
//! **No schema namespace** (tasks.md 1.2: "`records_history` DDL
//! mirroring pg minus the schema namespace") — a sqlite db FILE is
//! already its own namespace; `PgTier`'s `{schema}.records_history`
//! table-qualification has no sqlite analog, so every SQL string here
//! is unqualified `records_history`. Column NAMES, the PRIMARY KEY,
//! and the dedup uniqueness are byte-for-byte the same as
//! [`crate::pg_tier`]'s DDL; column TYPE affinities differ
//! (`TEXT` instead of `TIMESTAMPTZ`/`JSONB`) only because sqlite has
//! no such native types — sqlx's chrono/json codecs already encode
//! `DateTime<Utc>`/`serde_json::Value` as RFC3339/JSON text for
//! sqlite, so `TEXT` is the exact wire representation either way.
//! Bind placeholders are sqlite's own positional `?` (not Postgres'
//! numbered `$1`/`$2`) — `sqlx`'s runtime `query()` API binds
//! `.bind()` calls to `?` occurrences in source order regardless of
//! backend, so this is a pure syntax difference, never a semantic one.
//!
//! SQL generation (this module's pure `*_sql` functions) is
//! unit-tested offline, unconditionally, exactly like `pg_tier.rs`.
//! Unlike `PgTier`/`R2Tier`, `SqliteTier` needs no live external
//! service and no `live-*` Cargo feature gate to test end to end:
//! sqlite is in-process, so [`SqliteTier::connect`] against a
//! `tempfile::tempdir()`-scoped path IS the offline integration test.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use canon_model::evidence::RawRecord;
use canon_model::envelope::RecordKind;
use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::partition::resolve_name;
use crate::policy::{Backend, Rung};
use crate::tier::{AgeReport, AgingRule, PurgeCount, RawWrite, RowRekey, StoreError, StoredRecord, StoredRow, Tier, TierQuery, TierReadResult, WriteReceipt};

fn create_table_sql() -> &'static str {
    "CREATE TABLE IF NOT EXISTS records_history (\
        kind TEXT NOT NULL, \
        id TEXT NOT NULL, \
        at TEXT NOT NULL, \
        digest TEXT NOT NULL, \
        body TEXT NOT NULL, \
        PRIMARY KEY (kind, id, digest)\
    )"
}

fn create_index_sql() -> &'static str {
    "CREATE INDEX IF NOT EXISTS records_history_kind_id_idx ON records_history (kind, id)"
}

/// [`crate::pg_tier::WRITE_BATCH_CHUNK_SIZE`]'s sqlite twin (s31
/// design D2 pattern, s32 tasks.md 1.2): the identical 500-row chunk
/// size — sqlite's own bind-parameter ceiling
/// (`SQLITE_LIMIT_VARIABLE_NUMBER`, default 32766) is comfortably
/// clear of `500 * 5 = 2500` params per chunk.
const WRITE_BATCH_CHUNK_SIZE: usize = 500;

/// The multi-row `INSERT` (mirrors
/// [`crate::pg_tier::insert_batch_sql`]) — `n` unnumbered `(?, ?, ?,
/// ?, ?)` tuples in ONE `INSERT … ON CONFLICT (kind, id, digest) DO
/// NOTHING`, the SAME per-row dedup semantics as the single-row form.
/// `RETURNING kind, id, digest` (not `insert_sql`'s bare `RETURNING
/// kind`) is deliberate: a multi-row statement's returned rows carry
/// no input-order guarantee, so the caller reconstructs each input
/// row's own `WriteReceipt.deduped` by testing SET MEMBERSHIP of its
/// full `(kind, id, digest)` identity, never by returned-row ordinal
/// position — identical rationale to the pg twin.
fn insert_batch_sql(n: usize) -> String {
    let values = vec!["(?, ?, ?, ?, ?)"; n].join(", ");
    format!(
        "INSERT INTO records_history (kind, id, at, digest, body) VALUES {values} \
         ON CONFLICT (kind, id, digest) DO NOTHING \
         RETURNING kind, id, digest"
    )
}

/// The stored body of one `(kind, id, digest)` row: what a write that
/// found its key taken compares itself against (R7).
fn select_body_sql() -> &'static str {
    "SELECT body FROM records_history WHERE kind = ? AND id = ? AND digest = ?"
}

fn select_sql(since: bool) -> &'static str {
    if since {
        "SELECT id, at, digest, body FROM records_history WHERE kind = ? AND at >= ? ORDER BY at"
    } else {
        "SELECT id, at, digest, body FROM records_history WHERE kind = ? ORDER BY at"
    }
}

fn select_older_than_sql() -> &'static str {
    "SELECT id, at, digest, body FROM records_history WHERE kind = ? AND at < ? ORDER BY at"
}
fn purge_count_sql() -> &'static str {
    "SELECT COUNT(*) AS count FROM records_history WHERE kind = ? AND at < ?"
}

fn purge_delete_sql() -> &'static str {
    "DELETE FROM records_history WHERE kind = ? AND at < ?"
}


/// Keyed by the FULL `(kind, id, digest)` row identity, never a bare
/// `(kind, id)` — same s21 task 3.5 rationale as
/// [`crate::pg_tier::delete_sql`]: several rows can share a bare
/// `(kind, id)` now, so [`Tier::age`]'s per-row sweep must delete
/// exactly the ONE version it just moved.
fn delete_sql() -> &'static str {
    "DELETE FROM records_history WHERE kind = ? AND id = ? AND digest = ?"
}

/// [`SqliteTier::connect`]'s busy timeout (s32 tasks.md 1.2's own
/// number): long enough that an ordinary write burst (a session's
/// worth of batched events) waits out a concurrent writer instead of
/// failing immediately with `SQLITE_BUSY`, short enough that a
/// genuinely stuck writer still surfaces as an error rather than
/// hanging forever.
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug)]
pub struct SqliteTier {
    path: PathBuf,
    pool: sqlx::SqlitePool,
    rt: tokio::runtime::Runtime,
    /// Sidecar files (`-wal`, `-shm`) that a read-only open created; removed
    /// on drop so a preview leaves the tree as it found it.
    created_sidecars: Vec<PathBuf>,
}

impl Drop for SqliteTier {
    fn drop(&mut self) {
        // Close every connection before returning: sqlx closes a dropped pool's connections on
        // their worker threads, so SQLite's last-close checkpoint (which removes `-wal`/`-shm`)
        // would otherwise run after the tier is gone, while the caller already reads the tree.
        self.rt.block_on(self.pool.close());
        for sidecar in &self.created_sidecars {
            let _ = std::fs::remove_file(sidecar);
        }
    }
}

impl SqliteTier {
    /// Open (creating if absent) the sqlite db file at `path`,
    /// creating any missing PARENT directories first (s32 tasks.md
    /// 1.2 — a fresh `canon init` repo has only `canon.yaml` written,
    /// no `canon/` directory yet), apply WAL journal mode + a
    /// [`BUSY_TIMEOUT`] busy timeout, then ensure `records_history`
    /// exists.
    ///
    /// Mirrors [`crate::pg_tier::PgTier::connect`]'s error-
    /// classification split exactly (s29 design D7): parent-dir
    /// creation, the initial pool connect, and applying the WAL/busy-
    /// timeout pragmas (which sqlx runs against every new physical
    /// connection as it's established — the FIRST of which happens
    /// eagerly, inside `connect_with`, never lazily on first query)
    /// are ALL folded into one "can this backend even be reached"
    /// availability fact — any failure among them classifies as
    /// [`StoreError::TierUnavailable`] (backend `sqlite`, reason
    /// naming `path` — sqlite has no DSN/env var to name instead), so
    /// `canon-cli`'s lenient tier builders can degrade an unopenable
    /// path exactly like they degrade an unreachable Postgres DSN.
    /// Only the post-connect `CREATE TABLE`/`CREATE INDEX` DDL keeps
    /// mapping to [`StoreError::Sql`] — a REACHABLE-but-broken db file
    /// (e.g. a `records_history` name collision with an incompatible
    /// existing table) is a genuine query failure, not an
    /// availability degrade.
    pub fn connect(path: &Path) -> Result<Self, StoreError> {
        Self::open(path, true)
    }

    /// [`SqliteTier::connect`] for a command that only reads: an absent db
    /// file (and only an absent one) is NOT created (no file, no parent directory, no WAL sidecar).
    /// The tier is backed by an empty in-memory table instead, so a query
    /// answers "no records" exactly as it would on a freshly created file,
    /// and the repo tree is left as the command found it.
    pub fn connect_for_read(path: &Path) -> Result<Self, StoreError> {
        // Only a path that does not exist reads as empty. Anything else there (a directory, an
        // unreadable or non-sqlite file) is opened for real and fails as an unavailable backend.
        let absent = matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
        Self::open(path, !absent)
    }

    /// Open an EXISTING db file strictly read-only (`SQLITE_OPEN_READ_ONLY`):
    /// no create, no journal-mode change, no `CREATE TABLE`, no write of any
    /// kind. A WAL-mode db still needs its `-wal`/`-shm` to read the latest
    /// rows; sidecars this open creates are removed when the tier drops, so
    /// the tree is byte-identical afterwards. An absent file reads as empty,
    /// like [`SqliteTier::connect_for_read`].
    pub fn connect_read_only(path: &Path) -> Result<Self, StoreError> {
        if matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
            return Self::open(path, false);
        }
        let rt = tokio::runtime::Runtime::new().map_err(StoreError::Io)?;
        let unavailable = |e: String| StoreError::tier_unavailable(Rung::Hot, Some(Backend::Sqlite), format!("{}: {e}", path.display()));
        let sidecar = |suffix: &str| {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            PathBuf::from(name)
        };
        let created_sidecars: Vec<PathBuf> = ["-wal", "-shm"].into_iter().map(sidecar).filter(|p| !p.exists()).collect();
        let opts = sqlx::sqlite::SqliteConnectOptions::new().filename(path).read_only(true).create_if_missing(false).busy_timeout(BUSY_TIMEOUT);
        let pool = rt.block_on(sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(opts)).map_err(|e| unavailable(e.to_string()))?;
        Ok(Self { path: path.to_path_buf(), pool, rt, created_sidecars })
    }

    fn open(path: &Path, on_disk: bool) -> Result<Self, StoreError> {
        let rt = tokio::runtime::Runtime::new().map_err(StoreError::Io)?;
        let path_owned = path.to_path_buf();
        let unavailable = |e: String| StoreError::tier_unavailable(Rung::Hot, Some(Backend::Sqlite), format!("{}: {e}", path.display()));
        let pool = rt.block_on(async {
            let pool = if on_disk {
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|e| unavailable(e.to_string()))?;
                    }
                }
                let opts = sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true)
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                    .busy_timeout(BUSY_TIMEOUT);
                sqlx::sqlite::SqlitePoolOptions::new().max_connections(5).connect_with(opts).await.map_err(|e| unavailable(e.to_string()))?
            } else {
                // One connection: every `:memory:` connection is its own database.
                let opts = sqlx::sqlite::SqliteConnectOptions::new().in_memory(true);
                sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(opts).await.map_err(|e| unavailable(e.to_string()))?
            };
            sqlx::query(create_table_sql()).execute(&pool).await.map_err(|e| StoreError::Sql(e.to_string()))?;
            sqlx::query(create_index_sql()).execute(&pool).await.map_err(|e| StoreError::Sql(e.to_string()))?;
            Ok::<_, StoreError>(pool)
        })?;
        Ok(Self { path: path_owned, pool, rt, created_sidecars: Vec::new() })
    }

    /// The resolved db-file path this tier was opened against —
    /// surfaced for a caller (`canon check-config`, diagnostics) that
    /// wants to print WHERE a `hot` rung's sqlite file actually lives.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append-only (mirrors [`crate::pg_tier::PgTier::write_row`]): a
    /// single-record batch, so one write and a batch of one share the one
    /// code path (and the one refusal of a same-key, different-content write).
    fn write_row(&self, record: &dyn StoredRecord) -> Result<WriteReceipt, StoreError> {
        Ok(self.write_rows(std::slice::from_ref(&record))?.remove(0))
    }

    /// The whole of one [`Tier::write_batch`] in ONE transaction (R7): every
    /// row is a plain `INSERT … ON CONFLICT (kind, id, digest) DO NOTHING`.
    /// A key that already holds the SAME canonical JSON is a no-op
    /// (`deduped: true`); a key that holds DIFFERENT canonical JSON is a
    /// [`StoreError::KeyConflict`] and the transaction rolls back, so none of
    /// the batch's rows is kept. A new digest at the same `(kind, id)` is a
    /// genuine append.
    fn write_rows(&self, records: &[&dyn StoredRecord]) -> Result<Vec<WriteReceipt>, StoreError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }

        // Resolve every row's persisted shape BEFORE opening the
        // transaction: a malformed record is an ordinary per-record `Err`,
        // never something that should abort a half-open transaction.
        let mut kinds = Vec::with_capacity(records.len());
        let mut keys = Vec::with_capacity(records.len());
        let mut ats = Vec::with_capacity(records.len());
        let mut digests = Vec::with_capacity(records.len());
        let mut bodies = Vec::with_capacity(records.len());
        for record in records {
            let kind = record.kind();
            let raw = record.to_raw();
            let name = resolve_name(kind, &raw.0)?;
            digests.push(name.digest);
            kinds.push(kind);
            ats.push(record.at());
            keys.push(name.key);
            bodies.push(raw.0);
        }

        let landed: HashSet<(String, String, String)> = self.rt.block_on(async {
            let mut tx = self.pool.begin().await.map_err(|e| StoreError::Sql(e.to_string()))?;
            let mut landed = HashSet::new();
            let mut seen: std::collections::HashMap<(String, String, String), usize> = std::collections::HashMap::new();
            for start in (0..records.len()).step_by(WRITE_BATCH_CHUNK_SIZE) {
                let end = (start + WRITE_BATCH_CHUNK_SIZE).min(records.len());
                let mut q = sqlx::query(sqlx::AssertSqlSafe(insert_batch_sql(end - start)));
                for i in start..end {
                    q = q.bind(kinds[i].as_str()).bind(&keys[i]).bind(ats[i]).bind(&digests[i]).bind(sqlx::types::Json(&bodies[i]));
                }
                for r in q.fetch_all(&mut *tx).await.map_err(|e| StoreError::Sql(e.to_string()))? {
                    landed.insert((r.get::<String, _>("kind"), r.get::<String, _>("id"), r.get::<String, _>("digest")));
                }
            }
            for i in 0..records.len() {
                let key = (kinds[i].as_str().to_string(), keys[i].clone(), digests[i].clone());
                if let Some(&first) = seen.get(&key) {
                    if bodies[first] != bodies[i] {
                        return Err(StoreError::KeyConflict { location: self.location(kinds[i], &keys[i], &digests[i]) });
                    }
                    continue;
                }
                seen.insert(key.clone(), i);
                if !landed.contains(&key) {
                    let stored = sqlx::query(select_body_sql())
                        .bind(kinds[i].as_str())
                        .bind(&keys[i])
                        .bind(&digests[i])
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(|e| StoreError::Sql(e.to_string()))?
                        .get::<sqlx::types::Json<serde_json::Value>, _>("body")
                        .0;
                    if stored != bodies[i] {
                        return Err(StoreError::KeyConflict { location: self.location(kinds[i], &keys[i], &digests[i]) });
                    }
                }
            }
            tx.commit().await.map_err(|e| StoreError::Sql(e.to_string()))?;
            Ok::<_, StoreError>(landed)
        })?;

        Ok((0..records.len())
            .map(|i| {
                let inserted = landed.contains(&(kinds[i].as_str().to_string(), keys[i].clone(), digests[i].clone()));
                WriteReceipt { kind: kinds[i], location: self.location(kinds[i], &keys[i], &digests[i]), digest: digests[i].clone(), deduped: !inserted }
            })
            .collect())
    }

    fn location(&self, kind: RecordKind, key: &str, digest: &str) -> String {
        format!("sqlite.records_history[{}={key}]_{digest}", kind.as_str())
    }

    /// Every row of `records_history`, in key order: what `canon migrate`
    /// reads to compute the rows' new keys.
    pub fn all_rows(&self) -> Result<Vec<StoredRow>, StoreError> {
        self.rt.block_on(async {
            let rows = sqlx::query("SELECT kind, id, at, digest, body FROM records_history ORDER BY kind, id, digest")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| StoreError::Sql(e.to_string()))?;
            Ok(rows
                .into_iter()
                .map(|r| StoredRow {
                    kind: r.get("kind"),
                    id: r.get("id"),
                    at: r.get("at"),
                    digest: r.get("digest"),
                    body: r.get::<sqlx::types::Json<serde_json::Value>, _>("body").0,
                })
                .collect())
        })
    }

    /// Re-key rows in ONE transaction (0.30 `canon migrate`): each row gets
    /// its new `(kind, id, digest)` and body. A new key already held by the
    /// same JSON is a no-op (the old row goes); held by other JSON it is a
    /// [`StoreError::KeyConflict`] and nothing of the call is kept.
    pub fn rekey(&self, changes: &[RowRekey]) -> Result<(), StoreError> {
        self.rt.block_on(async {
            let mut tx = self.pool.begin().await.map_err(|e| StoreError::Sql(e.to_string()))?;
            for change in changes {
                let held = sqlx::query(select_body_sql())
                    .bind(&change.to.0)
                    .bind(&change.to.1)
                    .bind(&change.to.2)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| StoreError::Sql(e.to_string()))?
                    .map(|r| r.get::<sqlx::types::Json<serde_json::Value>, _>("body").0);
                let moves_onto_itself = change.from == change.to;
                if let (Some(held), false) = (&held, moves_onto_itself) {
                    if *held != change.body {
                        return Err(StoreError::KeyConflict { location: self.location_of(&change.to) });
                    }
                    sqlx::query(delete_sql()).bind(&change.from.0).bind(&change.from.1).bind(&change.from.2).execute(&mut *tx).await.map_err(|e| StoreError::Sql(e.to_string()))?;
                    continue;
                }
                sqlx::query("UPDATE records_history SET kind = ?, id = ?, digest = ?, body = ? WHERE kind = ? AND id = ? AND digest = ?")
                    .bind(&change.to.0)
                    .bind(&change.to.1)
                    .bind(&change.to.2)
                    .bind(sqlx::types::Json(&change.body))
                    .bind(&change.from.0)
                    .bind(&change.from.1)
                    .bind(&change.from.2)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| StoreError::Sql(e.to_string()))?;
            }
            tx.commit().await.map_err(|e| StoreError::Sql(e.to_string()))?;
            Ok(())
        })
    }

    fn location_of(&self, key: &(String, String, String)) -> String {
        format!("sqlite.records_history[{}={}]_{}", key.0, key.1, key.2)
    }

    /// Fold the write-ahead log into the db file, so copying the file alone
    /// (the `canon migrate` backup) copies every row.
    pub fn checkpoint(&self) -> Result<(), StoreError> {
        self.rt.block_on(async { sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&self.pool).await.map(drop).map_err(|e| StoreError::Sql(e.to_string())) })
    }

    fn read_rows(&self, query: &TierQuery, older_than: Option<DateTime<Utc>>) -> Result<Vec<RawRecord>, StoreError> {
        self.rt.block_on(async {
            let sql = match (older_than, query.since) {
                (Some(_), _) => select_older_than_sql(),
                (None, Some(_)) => select_sql(true),
                (None, None) => select_sql(false),
            };
            let mut q = sqlx::query(sql).bind(query.kind.as_str());
            q = if let Some(cutoff) = older_than { q.bind(cutoff) } else if let Some(since) = query.since { q.bind(since) } else { q };
            let rows = q.fetch_all(&self.pool).await.map_err(|e| StoreError::Sql(e.to_string()))?;
            Ok(rows.into_iter().map(|r| RawRecord(r.get::<sqlx::types::Json<serde_json::Value>, _>("body").0)).collect())
        })
    }
}

impl Tier for SqliteTier {
    fn backend(&self) -> Backend {
        Backend::Sqlite
    }

    fn write(&self, record: &dyn StoredRecord) -> Result<WriteReceipt, StoreError> {
        self.write_row(record)
    }

    /// [`SqliteTier`]'s override of [`Tier::write_batch`] (s31 design
    /// D2, mirrors [`crate::pg_tier::PgTier::write_batch`]): chunks
    /// `records` into ≤[`WRITE_BATCH_CHUNK_SIZE`]-row groups, each
    /// persisted via [`Self::write_chunk`]'s single multi-row
    /// statement. Receipts come back in `records`' original order,
    /// byte-for-byte the same shape [`Self::write_row`]'s single-row
    /// path produces for each record.
    fn write_batch(&self, records: &[&dyn StoredRecord]) -> Result<Vec<WriteReceipt>, StoreError> {
        self.write_rows(records)
    }

    /// Never pre-folds (s21 D5) — returns every retained historical
    /// row for `query.kind` (optionally `since`-filtered), the SAME
    /// raw contract `GitTier`/`PgTier`/`R2Tier::read` honor.
    fn read(&self, query: &TierQuery) -> Result<TierReadResult, StoreError> {
        let records = self.read_rows(query, None)?;
        Ok(TierReadResult { records, violations: Vec::new() })
    }
    fn purge_keys(&self, kind: RecordKind, before: DateTime<Utc>) -> Result<Vec<String>, StoreError> {
        self.rt.block_on(async {
            let rows = sqlx::query("SELECT id, body FROM records_history WHERE kind = ? AND at < ? ORDER BY id, digest")
                .bind(kind.as_str())
                .bind(before)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| StoreError::Sql(e.to_string()))?;
            Ok(rows.iter()
                .map(|r| crate::partition::purge_id(kind, &RawRecord(r.get::<sqlx::types::Json<serde_json::Value>, _>("body").0), &r.get::<String, _>("id")))
                .collect())
        })
    }
    fn purge_before(&self, kind: RecordKind, before: DateTime<Utc>, dry_run: bool) -> Result<PurgeCount, StoreError> {
        self.rt.block_on(async {
            let matched = usize::try_from(sqlx::query(purge_count_sql())
                .bind(kind.as_str())
                .bind(before)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| StoreError::Sql(e.to_string()))?
                .get::<i64, _>("count"))
                .expect("a SQL COUNT is non-negative and fits usize");
            let deleted = if dry_run {
                0
            } else {
                usize::try_from(sqlx::query(purge_delete_sql())
                    .bind(kind.as_str())
                    .bind(before)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| StoreError::Sql(e.to_string()))?
                    .rows_affected())
                    .expect("deleted row count fits usize")
            };
            Ok(PurgeCount { matched, deleted })
        })
    }


    fn age(&self, rule: &AgingRule) -> Result<AgeReport, StoreError> {
        let cutoff = Utc::now() - rule.after;
        let candidates = self.read_rows(&TierQuery::kind(rule.kind), Some(cutoff))?;

        let mut moved = 0;
        let mut already_aged = 0;
        for raw in candidates {
            let name = resolve_name(rule.kind, &raw.0)?;
            let receipt = rule.destination.write(&RawWrite(raw))?;
            if receipt.deduped {
                already_aged += 1;
            } else {
                moved += 1;
            }
            self.rt.block_on(async {
                sqlx::query(delete_sql())
                    .bind(rule.kind.as_str())
                    .bind(&name.key)
                    .bind(&name.digest)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| StoreError::Sql(e.to_string()))
            })?;
        }
        Ok(AgeReport { kind: rule.kind, moved, already_aged })
    }
}

#[cfg(test)]
mod tests {
    use canon_model::envelope::{Actor, Envelope};
    use canon_model::ids::{ChangeId, RoleId, RunId};
    use canon_model::records::{Change, ChangeStatus, Trajectory};

    use super::*;

    fn actor() -> Actor {
        Actor::new("test-agent", RoleId::parse("implementer").unwrap())
    }

    fn change(at: DateTime<Utc>, id: &str) -> Change {
        Change::new(Envelope::new(1, RecordKind::Change, at, actor()), ChangeId::parse(id).unwrap(), "S32", "x", ChangeStatus::Proposed)
    }

    fn trajectory(at: DateTime<Utc>, reward: f64) -> Trajectory {
        Trajectory::new(Envelope::new(1, RecordKind::Trajectory, at, actor()), RunId::new(), None, None, None, None, Some(reward))
    }

    // ── SQL generation (offline, unconditional — mirrors pg_tier.rs's own unit tests) ──

    #[test]
    fn create_table_sql_is_unqualified_and_keyed_by_kind_id_digest() {
        let sql = create_table_sql();
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS records_history"), "no schema qualification, unlike pg: {sql}");
        assert!(sql.contains("PRIMARY KEY (kind, id, digest)"), "s21 D2: the idempotence key includes digest, never a bare (kind, id)");
        assert!(!sql.contains('.'), "sqlite has no schema namespace to qualify the table with: {sql}");
    }

    #[test]
    fn insert_batch_sql_binds_n_five_wide_tuples_and_returns_the_full_row_identity() {
        let sql = insert_batch_sql(3);
        assert!(sql.contains("ON CONFLICT (kind, id, digest) DO NOTHING"), "s21 D1: batched inserts stay append-only, never DO UPDATE");
        assert!(!sql.to_uppercase().contains("DO UPDATE"));
        assert!(sql.contains("VALUES (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?)"), "each row is its own 5-placeholder tuple: {sql}");
        assert!(sql.contains("RETURNING kind, id, digest"), "the caller reconstructs each row's own receipt by full identity, not by return-row ordinal");
        assert!(sql.contains("INSERT INTO records_history"));
    }

    #[test]
    fn insert_batch_sql_of_one_row_matches_insert_sql_shape() {
        let batch = insert_batch_sql(1);
        assert!(batch.contains("VALUES (?, ?, ?, ?, ?)"));
        assert!(batch.contains("ON CONFLICT (kind, id, digest) DO NOTHING"));
    }

    #[test]
    fn write_batch_chunk_size_is_five_hundred() {
        // s31 design D2's own number, mirrored from pg_tier — a
        // drifted constant would silently change the
        // transaction-per-chunk boundary.
        assert_eq!(WRITE_BATCH_CHUNK_SIZE, 500);
    }

    #[test]
    fn select_sql_filters_by_since_only_when_requested() {
        assert!(!select_sql(false).contains("at >="));
        assert!(select_sql(true).contains("at >= ?"));
    }

    #[test]
    fn select_sql_carries_no_dedup_or_fold_clause() {
        // s21 D5: the raw, unfolded multi-version read is the point —
        // no DISTINCT/GROUP BY collapsing rows before the caller's own
        // `fold_latest_by_key` gets to see every version.
        for sql in [select_sql(false), select_sql(true), select_older_than_sql()] {
            let upper = sql.to_uppercase();
            assert!(!upper.contains("DISTINCT"), "must not pre-fold: {sql}");
            assert!(!upper.contains("GROUP BY"), "must not pre-fold: {sql}");
        }
    }

    #[test]
    fn select_older_than_sql_filters_strictly_less_than() {
        assert!(select_older_than_sql().contains("at < ?"));
    }

    #[test]
    fn delete_sql_is_keyed_by_the_full_kind_id_digest_row_identity() {
        // s21 task 3.5: several rows can share a bare (kind, id) now —
        // aging must delete exactly the ONE version it just moved.
        assert!(delete_sql().contains("WHERE kind = ? AND id = ? AND digest = ?"));
    }

    // ── live, in-process behavior (offline — no `live-*` feature gate needed) ──

    /// s32 tasks.md 1.2: "connect opens/creates the db file (create
    /// parent dirs)" — a fresh `canon init`-shaped repo has only its
    /// canon.yaml written, no `canon/` directory yet.
    #[test]
    fn connect_creates_missing_parent_directories_and_the_db_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deeper/hot.db");
        assert!(!path.parent().unwrap().exists());
        let _tier = SqliteTier::connect(&path).unwrap();
        assert!(path.exists(), "the db file itself must exist after connect");
    }

    /// s32 spec.md "SqliteTier honors the store contract": "WAL
    /// journal mode and a busy timeout applied at connect" — verified
    /// by querying the pragmas back, not just trusting the builder
    /// call compiled.
    #[test]
    fn connect_applies_wal_journal_mode_and_the_busy_timeout_pragma() {
        let dir = tempfile::tempdir().unwrap();
        let tier = SqliteTier::connect(&dir.path().join("hot.db")).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (journal_mode, busy_timeout): (String, i64) = rt.block_on(async {
            let mode: (String,) = sqlx::query_as("PRAGMA journal_mode").fetch_one(&tier.pool).await.unwrap();
            let timeout: (i64,) = sqlx::query_as("PRAGMA busy_timeout").fetch_one(&tier.pool).await.unwrap();
            (mode.0, timeout.0)
        });
        assert_eq!(journal_mode.to_lowercase(), "wal");
        assert_eq!(busy_timeout, i64::try_from(BUSY_TIMEOUT.as_millis()).unwrap());
    }

    /// s32 tasks.md 1.3: "dedup no-op on byte-identical resubmission".
    #[test]
    fn resubmission_of_a_byte_identical_record_is_a_deduped_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let tier = SqliteTier::connect(&dir.path().join("hot.db")).unwrap();
        let record = change(Utc::now(), "s32-dedup-test");

        let first = tier.write(&record).unwrap();
        assert!(!first.deduped, "the first write of fresh content must not be deduped");

        let second = tier.write(&record).unwrap();
        assert!(second.deduped, "a byte-identical resubmission must be a no-op");
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.location, second.location);

        let result = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap();
        assert_eq!(result.records.len(), 1, "row count must be unchanged after the resubmission");
    }

    /// R7: a key held by DIFFERENT canonical JSON is a loud conflict, not a
    /// silent `DO NOTHING`.
    #[test]
    fn a_key_held_by_other_json_is_a_key_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let tier = SqliteTier::connect(&dir.path().join("hot.db")).unwrap();
        let record = change(Utc::now(), "r7-conflict");
        tier.write(&record).unwrap();
        tier.rt
            .block_on(async { sqlx::query("UPDATE records_history SET body = ?").bind(sqlx::types::Json(serde_json::json!({"tampered": true}))).execute(&tier.pool).await })
            .unwrap();

        let err = tier.write(&record).unwrap_err();
        assert!(matches!(err, StoreError::KeyConflict { .. }), "got {err:?}");
    }

    /// `canon migrate`'s rekey: a free target moves the row, a target held by
    /// other JSON conflicts and keeps the old row.
    #[test]
    fn rekey_moves_a_row_and_refuses_a_target_held_by_other_json() {
        let dir = tempfile::tempdir().unwrap();
        let tier = SqliteTier::connect(&dir.path().join("hot.db")).unwrap();
        let a = change(Utc::now(), "rekey-a");
        let b = change(Utc::now(), "rekey-b");
        let ra = tier.write(&a).unwrap();
        let rb = tier.write(&b).unwrap();
        let key_of = |id: &str, digest: &str| ("change".to_string(), id.to_string(), digest.to_string());
        let from = key_of("rekey-a", &ra.digest);

        let conflict = RowRekey { from: from.clone(), to: key_of("rekey-b", &rb.digest), body: a.to_raw().0 };
        let err = tier.rekey(&[conflict]).unwrap_err();
        assert!(matches!(err, StoreError::KeyConflict { .. }), "got {err:?}");
        assert_eq!(tier.read(&TierQuery::kind(RecordKind::Change)).unwrap().records.len(), 2, "a refused rekey changes nothing");

        let moved = RowRekey { from, to: key_of("rekey-a2", "deadbeef"), body: a.to_raw().0 };
        tier.rekey(&[moved]).unwrap();
        let ids: Vec<String> = tier
            .rt
            .block_on(async { sqlx::query("SELECT id FROM records_history ORDER BY id").fetch_all(&tier.pool).await })
            .unwrap()
            .into_iter()
            .map(|r| r.get::<String, _>("id"))
            .collect();
        assert_eq!(ids, vec!["rekey-a2".to_string(), "rekey-b".to_string()]);
    }

    /// A read-only open reads the rows and leaves the tree as it found it:
    /// no sidecar files, db bytes unchanged.
    #[test]
    fn connect_read_only_reads_rows_and_leaves_no_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("hot.db");
        let writer = SqliteTier::connect(&db).unwrap();
        writer.write(&change(Utc::now(), "ro-read")).unwrap();
        writer.checkpoint().unwrap();
        drop(writer);
        // A clean close of the last connection removes the sidecars.
        let _ = std::fs::remove_file(dir.path().join("hot.db-wal"));
        let _ = std::fs::remove_file(dir.path().join("hot.db-shm"));
        let before = std::fs::read(&db).unwrap();

        let tier = SqliteTier::connect_read_only(&db).unwrap();
        assert_eq!(tier.read(&TierQuery::kind(RecordKind::Change)).unwrap().records.len(), 1);
        drop(tier);

        let mut names: Vec<String> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, vec!["hot.db".to_string()], "no -wal/-shm left behind");
        assert_eq!(std::fs::read(&db).unwrap(), before, "the db file is byte-identical");
        assert!(SqliteTier::connect_read_only(&dir.path().join("absent.db")).unwrap().read(&TierQuery::kind(RecordKind::Change)).unwrap().records.is_empty());
        assert!(!dir.path().join("absent.db").exists());
    }

    /// s32 tasks.md 1.3: "write_batch == write loop" — a
    /// `write_batch` call over a corpus and an equivalent
    /// `write`-per-record loop over a disjoint corpus of the same
    /// shape produce the SAME receipt semantics (fresh write, never
    /// deduped) and leave the SAME number of records behind.
    #[test]
    fn write_batch_matches_a_write_loop_over_an_equivalent_corpus() {
        let dir = tempfile::tempdir().unwrap();
        let tier = SqliteTier::connect(&dir.path().join("hot.db")).unwrap();

        let looped: Vec<Trajectory> = (0..6).map(|_| trajectory(Utc::now(), 0.5)).collect();
        let batched: Vec<Trajectory> = (0..6).map(|_| trajectory(Utc::now(), 0.5)).collect();

        let loop_receipts: Vec<_> = looped.iter().map(|t| tier.write(t).unwrap()).collect();
        let batch_refs: Vec<&dyn StoredRecord> = batched.iter().map(|t| t as &dyn StoredRecord).collect();
        let batch_receipts = tier.write_batch(&batch_refs).unwrap();

        assert_eq!(loop_receipts.len(), batch_receipts.len());
        for (loop_r, batch_r) in loop_receipts.iter().zip(&batch_receipts) {
            assert!(!loop_r.deduped);
            assert!(!batch_r.deduped, "fresh distinct-run-id records must all land as new writes under both codepaths");
        }

        let after = tier.read(&TierQuery::kind(RecordKind::Trajectory)).unwrap();
        assert_eq!(after.records.len(), 12, "both corpora (6 looped + 6 batched) must be fully persisted, none dropped or duplicated");

        // Resubmitting the batched corpus as a SECOND write_batch call
        // must be a no-op for every row — batch dedup mirrors
        // single-row dedup exactly.
        let resubmit = tier.write_batch(&batch_refs).unwrap();
        assert!(resubmit.iter().all(|r| r.deduped));
        let still = tier.read(&TierQuery::kind(RecordKind::Trajectory)).unwrap();
        assert_eq!(still.records.len(), 12, "resubmission must never double-write");
    }

    /// s32 tasks.md 1.3: "TierQuery read-back" — records come back
    /// ordered by `at`, and a `since` bound excludes older rows.
    #[test]
    fn tier_query_read_back_preserves_ordering_and_the_since_filter() {
        let dir = tempfile::tempdir().unwrap();
        let tier = SqliteTier::connect(&dir.path().join("hot.db")).unwrap();

        let older = trajectory(Utc::now() - chrono::Duration::days(5), 0.1);
        let newer = trajectory(Utc::now(), 0.2);
        tier.write(&newer).unwrap();
        tier.write(&older).unwrap();

        let all = tier.read(&TierQuery::kind(RecordKind::Trajectory)).unwrap();
        assert_eq!(all.records.len(), 2);
        assert_eq!(all.records[0].0["reward"], 0.1, "results must be ordered by `at`, oldest first");
        assert_eq!(all.records[1].0["reward"], 0.2);

        let since_cutoff = Utc::now() - chrono::Duration::days(1);
        let recent = tier.read(&TierQuery::kind(RecordKind::Trajectory).since(since_cutoff)).unwrap();
        assert_eq!(recent.records.len(), 1, "a `since` bound must exclude the older row");
        assert_eq!(recent.records[0].0["reward"], 0.2);
    }

    /// s32 tasks.md 1.3: "records survive reconnect" — a fresh
    /// `SqliteTier::connect` against the SAME path sees everything a
    /// prior, now-dropped tier handle wrote.
    #[test]
    fn records_survive_reconnect() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hot.db");
        {
            let tier = SqliteTier::connect(&path).unwrap();
            tier.write(&change(Utc::now(), "s32-reconnect-test")).unwrap()
        };
        let reopened = SqliteTier::connect(&path).unwrap();
        let result = reopened.read(&TierQuery::kind(RecordKind::Change)).unwrap();
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].0["change_id"], "s32-reconnect-test");
    }

    /// s32 tasks.md 1.2: "error mapping into the existing StoreError
    /// taxonomy: unreachable/corrupt file → TierUnavailable-class
    /// reason naming the path". A parent path component that already
    /// exists as a plain FILE (not a directory) makes `create_dir_all`
    /// fail deterministically, offline, without depending on sqlite's
    /// own lazy-vs-eager file-format validation timing.
    #[test]
    fn an_unopenable_path_classifies_as_tier_unavailable_naming_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let blocking_file = dir.path().join("not-a-directory");
        std::fs::write(&blocking_file, b"i am a file, not a directory").unwrap();
        let db_path = blocking_file.join("hot.db");

        let Err(err) = SqliteTier::connect(&db_path) else {
            panic!("a parent path component that is a plain file must fail to open")
        };
        assert!(
            matches!(err, StoreError::TierUnavailable { backend: Some(Backend::Sqlite), .. }),
            "an unopenable path must classify as TierUnavailable(sqlite), got {err:?}"
        );
        assert!(err.to_string().contains(&db_path.display().to_string()), "the reason must name the path: {err}");
    }

    /// [`Tier::age`] parity: a record past its aging threshold moves
    /// to the destination tier and is removed from the source —
    /// mirrors `PgTier`'s own `age` behavior, exercised here with two
    /// `SqliteTier`s standing in for source/destination.
    #[test]
    fn age_moves_records_past_threshold_and_is_idempotent_on_rerun() {
        let source_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        let source = SqliteTier::connect(&source_dir.path().join("hot.db")).unwrap();
        let dest = std::sync::Arc::new(SqliteTier::connect(&dest_dir.path().join("cold.db")).unwrap());

        let old = trajectory(Utc::now() - chrono::Duration::days(30), 0.3);
        source.write(&old).unwrap();

        let report = source.age(&AgingRule { kind: RecordKind::Trajectory, after: chrono::Duration::days(1), destination: std::sync::Arc::<SqliteTier>::clone(&dest) }).unwrap();
        assert_eq!(report.moved, 1);
        assert_eq!(report.already_aged, 0);
        assert_eq!(source.read(&TierQuery::kind(RecordKind::Trajectory)).unwrap().records.len(), 0, "the source row must be gone after aging");
        assert_eq!(dest.read(&TierQuery::kind(RecordKind::Trajectory)).unwrap().records.len(), 1, "the destination must hold the moved row");

        let second = source.age(&AgingRule { kind: RecordKind::Trajectory, after: chrono::Duration::days(1), destination: dest }).unwrap();
        assert_eq!(second.moved, 0, "nothing left in the source to re-select");
        assert_eq!(second.already_aged, 0);
    }
}
