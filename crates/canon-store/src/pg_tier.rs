//! `PgTier`: the hot, shared tier (design D1/D3, redesigned append-only
//! by s21 `cross-tier-supersession` D1/D2) — sqlx against a
//! `canon.yaml`-configured DSN/schema (default `canon_v1`), one
//! `records_history(kind, id, at, digest, body)` table keyed by
//! `(kind, id, digest)`.
//!
//! **Genuinely append-only, exactly like `GitTier`/`R2Tier` (s21 D1).**
//! The pre-s21 `records` table (`PRIMARY KEY (kind, id)`, unconditional
//! `ON CONFLICT DO UPDATE`) was last-WRITE-wins, not last-`at`-wins: an
//! out-of-order write carrying an OLDER `at` could silently overwrite a
//! row holding a NEWER `at`, and the overwritten version was gone —
//! no history survived to recover it (s21 proposal.md "The correctness
//! gap"). `records_history`'s `(kind, id, digest)` primary key mirrors
//! `GitTier`'s digest-suffixed-path idempotence exactly (s21 D2): a
//! byte-identical resubmission at the same key is a no-op
//! (`WriteReceipt.deduped = true`, `INSERT … ON CONFLICT (kind, id,
//! digest) DO NOTHING`), while a write carrying a NEW digest at the
//! same `(kind, id)` — e.g. a `Handoff`'s `state` advancing through its
//! closed state machine, S1's `Handoff::transition_to` — is a genuine
//! APPEND, never an in-place overwrite: every version any caller ever
//! wrote stays retrievable. `id` is each kind's natural key (the same
//! [`crate::partition::resolve_partition`] extraction `GitTier` uses).
//!
//! **`PgTier::read` never pre-folds (s21 D5).** It returns EVERY
//! historical row for a kind (optionally `since`-filtered) — the SAME
//! raw, unfolded contract `GitTier`/`R2Tier` already honor, making
//! `Tier::read` uniform across all three adapters. Resolving "the
//! current value for a key" is exclusively the CALLER's job, via
//! [`crate::fold::fold_latest_by_key`] — no tier-specific pre-fold
//! lives here or anywhere else in this module.
//!
//! **The pre-s21 `records` table is deliberately left un-migrated**
//! (s21 design.md R1): `PgTier::connect`/`connect_live` create
//! `records_history` fresh (`CREATE TABLE IF NOT EXISTS`, this
//! codebase's only DDL convention — no migration tooling exists here)
//! and never touch a pre-existing `records` table. A developer with
//! dev-stage data already written to the old table sees an EMPTY
//! `records_history` on first connect after this change — accepted,
//! see s21 design.md's Risks for the full rationale.
//!
//! SQL generation (this module's pure `*_sql` functions) is unit-tested
//! offline, unconditionally; actually executing against a live Postgres
//! instance is exercised only by `tests/pg_tier_live.rs`, gated behind
//! the `live-pg` Cargo feature AND a reachable Postgres at runtime
//! (S2 assignment constraint: `cargo test --workspace` — no extra
//! flags — must never attempt network I/O). [`PgTier::connect_live`]
//! resolves `CANON_PG_DSN` with a docker-compose default (repo-root
//! `docker-compose.yml`, operator directive 2026-07-10) so a local
//! live-tier run needs no exported env vars; `canon-cli` itself still
//! resolves `canon.yaml`'s own `tiers.pg.dsn_env`-named var and calls
//! [`PgTier::connect`] directly (fail-loud, no default).

use std::collections::HashSet;

use canon_model::evidence::RawRecord;
use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::partition::resolve_name;
use crate::policy::{Backend, Rung};
use crate::tier::{AgeReport, AgingRule, PurgeCount, RawWrite, RowRekey, StoreError, StoredRecord, StoredRow, Tier, TierQuery, TierReadResult, WriteReceipt};

/// `schema` comes from `canon.yaml` (an operator-controlled config
/// file, not runtime user input) but is still interpolated into DDL/
/// table-qualified SQL text (sqlx has no bind-parameter form for
/// identifiers) — validated here to `[a-z0-9_]+` so a malformed
/// `canon.yaml` fails loud rather than producing injectable SQL.
/// `pub` so a config-validating caller (e.g. `canon ingest plans`'s
/// lenient tier builder) can reject a malformed `tiers.pg.schema`
/// BEFORE any DSN lookup — a bad schema is a fatal config error even
/// when no live DSN is present, never masked by an unset-DSN degrade.
pub fn validate_schema_ident(schema: &str) -> Result<(), StoreError> {
    if !schema.is_empty() && schema.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        Ok(())
    } else {
        Err(StoreError::Policy(format!("pg schema `{schema}` must match `[a-z0-9_]+`")))
    }
}

/// The advisory-lock key that serializes [`PgTier::connect`]'s bootstrap
/// DDL, derived from the schema name so two different schemas never
/// block each other.
///
/// Postgres's `IF NOT EXISTS` is NOT atomic against a concurrent
/// creator: two sessions both pass the existence check, both try to
/// insert the catalog row, and the loser gets `duplicate key value
/// violates unique constraint "pg_type_typname_nsp_index"`. Since every
/// `connect` runs the three DDL statements, two `canon` processes
/// reaching a FRESH schema at the same time make one of them fail at
/// startup — and team-scale concurrency is the entire reason the `hot`
/// rung can be postgres at all (s32 `sqlite-hot-backend`: sqlite covers
/// the single operator, postgres is the swap for a real server). CI hit
/// this as a flake in `pg_tier_live.rs`, which is the same race with two
/// test threads rather than two operators.
fn bootstrap_lock_key(schema: &str) -> i64 {
    // Stable across processes and machines — never `DefaultHasher`,
    // whose output is not guaranteed between builds.
    let digest = <sha2::Sha256 as sha2::Digest>::digest(schema.as_bytes());
    i64::from_be_bytes(digest[..8].try_into().expect("sha256 yields at least 8 bytes"))
}

fn create_schema_sql(schema: &str) -> String {
    format!("CREATE SCHEMA IF NOT EXISTS {schema}")
}

fn create_table_sql(schema: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {schema}.records_history (\
            kind TEXT NOT NULL, \
            id TEXT NOT NULL, \
            at TIMESTAMPTZ NOT NULL, \
            digest TEXT NOT NULL, \
            body JSONB NOT NULL, \
            PRIMARY KEY (kind, id, digest)\
        )"
    )
}

fn create_index_sql(schema: &str) -> String {
    format!("CREATE INDEX IF NOT EXISTS records_history_kind_id_idx ON {schema}.records_history (kind, id)")
}

/// [`PgTier::write_batch`]'s chunk size (s31 design D2): 500 rows per
/// multi-row `INSERT` statement — large enough that a typical ingest
/// pass's batch (a session's worth of events) lands in a single
/// statement, small enough to stay well under Postgres' bind-parameter
/// limit (`n * 5` params per chunk; 500 rows = 2500 params, comfortably
/// under the wire protocol's 65535 cap) and to keep each chunk's
/// transaction short-lived.
const WRITE_BATCH_CHUNK_SIZE: usize = 500;

/// The multi-row `INSERT` (s31 design D2: `PgTier::write_batch`'s
/// 500-row chunk statement) — `n` `($1..$5), ($6..$10), …` VALUES
/// tuples in ONE `INSERT … ON CONFLICT (kind, id, digest) DO NOTHING`,
/// the SAME per-row dedup semantics as the single-row form. `RETURNING
/// kind, id, digest` (rather than `insert_sql`'s bare `RETURNING
/// kind`) is deliberate: a multi-row statement's returned rows carry
/// no input-order guarantee, so the caller reconstructs each input
/// row's own `WriteReceipt.deduped` by testing SET MEMBERSHIP of its
/// full `(kind, id, digest)` identity against the returned rows,
/// never by returned-row ordinal position.
fn insert_batch_sql(schema: &str, n: usize) -> String {
    let mut values = Vec::with_capacity(n);
    for i in 0..n {
        let base = i * 5;
        values.push(format!("(${}, ${}, ${}, ${}, ${})", base + 1, base + 2, base + 3, base + 4, base + 5));
    }
    format!(
        "INSERT INTO {schema}.records_history (kind, id, at, digest, body) VALUES {} \
         ON CONFLICT (kind, id, digest) DO NOTHING \
         RETURNING kind, id, digest",
        values.join(", ")
    )
}

/// The stored body of one `(kind, id, digest)` row: what a write that
/// found its key taken compares itself against (R7). `body` is JSONB, so the
/// comparison is of the parsed JSON values, not of bytes.
fn select_body_sql(schema: &str) -> String {
    format!("SELECT body FROM {schema}.records_history WHERE kind = $1 AND id = $2 AND digest = $3")
}

/// `canon migrate`'s re-key of one row (R7 conflict handling is in
/// [`PgTier::rekey`]): the key, body and nothing else change.
fn rekey_sql(schema: &str) -> String {
    format!("UPDATE {schema}.records_history SET kind = $1, id = $2, digest = $3, body = $4 WHERE kind = $5 AND id = $6 AND digest = $7")
}

fn all_rows_sql(schema: &str) -> String {
    format!("SELECT kind, id, to_char(at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS at, digest, body FROM {schema}.records_history ORDER BY kind, id, digest")
}

fn select_sql(schema: &str, since: bool) -> String {
    if since {
        format!("SELECT id, at, digest, body FROM {schema}.records_history WHERE kind = $1 AND at >= $2 ORDER BY at")
    } else {
        format!("SELECT id, at, digest, body FROM {schema}.records_history WHERE kind = $1 ORDER BY at")
    }
}

fn select_older_than_sql(schema: &str) -> String {
    format!("SELECT id, at, digest, body FROM {schema}.records_history WHERE kind = $1 AND at < $2 ORDER BY at")
}
fn purge_count_sql(schema: &str) -> String {
    format!("SELECT COUNT(*) AS count FROM {schema}.records_history WHERE kind = $1 AND at < $2")
}

fn purge_delete_sql(schema: &str) -> String {
    format!("DELETE FROM {schema}.records_history WHERE kind = $1 AND at < $2")
}


/// Keyed by the FULL `(kind, id, digest)` identity, never a bare
/// `(kind, id)` (s21 D1/task 3.5): `records_history` can hold several
/// rows under the same `(kind, id)` now, so `Tier::age`'s per-row sweep
/// must delete exactly the ONE row it just moved — deleting by `(kind,
/// id)` alone would drop every OTHER version still under its aging
/// threshold right along with it.
fn delete_sql(schema: &str) -> String {
    format!("DELETE FROM {schema}.records_history WHERE kind = $1 AND id = $2 AND digest = $3")
}

/// [`PgTier::connect_live`]'s DSN resolution cascade (data-stores
/// Pattern 6 — "Env-var credential fallback chain: specific → shared →
/// generic", the donor data-stores adoption brief §Pattern 6 / the donor
/// data-stores credentials-config-distribution notes §2.1,3.1):
/// (1) `CANON_PG_DSN` —
/// canon's own service-specific override, and exactly the name
/// `canon.yaml`'s shipped default `tiers.pg.dsn_env` resolves to
/// (`policy.rs`/`registry.rs`'s own example configs) — kept as tier 1,
/// unchanged from before this cascade, so any caller already exporting
/// it sees no regression; (2) else `DATABASE_URL` — the donor's own
/// chain terminus, the single most portable Postgres convention a
/// consumer repo is likely to already have exported (a prior
/// session/event store reads exactly this name, no fallback chain of its
/// own); (3) else the existing docker-compose local default —
/// unchanged, so a zero-env-var local dev/CI run still works.
///
/// Parameterized over `lookup` (rather than reading `std::env::var`
/// directly) so this ordering is unit-testable without racing real
/// process env vars across the crate's parallel test threads —
/// [`PgTier::connect_live`] is the only real caller, passing
/// `std::env::var` itself.
fn resolve_pg_dsn(lookup: impl Fn(&str) -> Option<String>) -> String {
    lookup("CANON_PG_DSN").or_else(|| lookup("DATABASE_URL")).unwrap_or_else(|| "postgres://canon:canon@127.0.0.1:55432/canon_v1".to_string())
}

#[derive(Debug)]
pub struct PgTier {
    schema: String,
    pool: sqlx::PgPool,
    rt: tokio::runtime::Runtime,
}

impl PgTier {
    /// Connect to `dsn` and ensure `schema`'s `records` table/index
    /// exist — the "explicitly-configured tier that can't attach is a
    /// startup-time hard failure, never a silent tier-skip" contract
    /// (a prior session-store storage audit §3.2, adopted verbatim for
    /// canon's pg tier). Requires network; every call site in this
    /// crate's own tests is behind `live-pg`.
    ///
    /// **s29 design D7**: the INITIAL `PgPoolOptions::connect` below —
    /// EAGER in sqlx (it acquires one real connection before
    /// returning; never lazy) — classifies its failure as
    /// [`StoreError::TierUnavailable`] (backend `postgres`, reason =
    /// the sqlx error's own `Display`), never `Sql`: a genuine outage
    /// (host down, connection refused, auth rejected before any query
    /// runs) is an AVAILABILITY fact, so `canon-cli`'s lenient
    /// builders (`attach_postgres`'s `TierUnavailable`-only catch)
    /// degrade it exactly like an unset DSN, without string-matching
    /// sqlx errors. `Rung::Hot` is hardcoded here rather than threaded
    /// through as a parameter: `TierPolicy::from_yaml`'s rung/backend
    /// class validation (s28 D1) already enforces that a
    /// postgres-backed rung can ONLY ever be `hot` — this constructor
    /// has no other rung it could legitimately mean. The three
    /// post-connect `CREATE SCHEMA`/`CREATE TABLE`/`CREATE INDEX` DDL
    /// statements below keep mapping to `Sql` — a REACHABLE-but-broken
    /// database (bad permissions, a rejected DDL) is a genuine query
    /// failure, not an availability degrade, and must stay loud
    /// through the strict path.
    pub fn connect(dsn: &str, schema: &str) -> Result<Self, StoreError> {
        validate_schema_ident(schema)?;
        let rt = tokio::runtime::Runtime::new().map_err(StoreError::Io)?;
        let pool = rt.block_on(async {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect(dsn)
                .await
                .map_err(|e| StoreError::tier_unavailable(Rung::Hot, Some(Backend::Postgres), e.to_string()))?;
            // All three DDL statements run inside ONE transaction
            // holding `pg_advisory_xact_lock`, so a concurrent
            // bootstrap of the same schema waits instead of racing the
            // catalog (see `bootstrap_lock_key`). The lock is
            // transaction-scoped, so it is released by commit OR by a
            // failed/aborted connect — never leaked to the pool.
            let mut tx = pool.begin().await.map_err(|e| StoreError::Sql(e.to_string()))?;
            sqlx::query("SELECT pg_advisory_xact_lock($1)")
                .bind(bootstrap_lock_key(schema))
                .execute(&mut *tx)
                .await
                .map_err(|e| StoreError::Sql(e.to_string()))?;
            sqlx::query(sqlx::AssertSqlSafe(create_schema_sql(schema))).execute(&mut *tx).await.map_err(|e| StoreError::Sql(e.to_string()))?;
            sqlx::query(sqlx::AssertSqlSafe(create_table_sql(schema))).execute(&mut *tx).await.map_err(|e| StoreError::Sql(e.to_string()))?;
            sqlx::query(sqlx::AssertSqlSafe(create_index_sql(schema))).execute(&mut *tx).await.map_err(|e| StoreError::Sql(e.to_string()))?;
            tx.commit().await.map_err(|e| StoreError::Sql(e.to_string()))?;
            Ok::<_, StoreError>(pool)
        })?;
        Ok(Self { schema: schema.to_string(), pool, rt })
    }

    /// [`Self::connect`], but resolving `dsn` via [`resolve_pg_dsn`]'s
    /// 3-tier cascade (data-stores Pattern 6, "Env-var credential
    /// fallback chain: specific → shared → generic" — a prior session
    /// store's chain
    /// `SESSION_DB_DUCKLAKE_METADATA_DSN` → `DASHBOARD_DB_DSN` →
    /// `DATABASE_URL`) instead of taking an explicit DSN string.
    /// `canon-cli`'s own `build_tiers` resolves `canon.yaml`'s
    /// `tiers.pg.dsn_env`-named var itself and calls [`Self::connect`]
    /// directly (fail-loud, no default, when that var is unset) — this
    /// constructor is the live-tier-test/local-dev-convenience
    /// counterpart to [`R2Tier::connect_live`](crate::r2_tier::R2Tier::connect_live),
    /// not a replacement for that CLI wiring.
    pub fn connect_live(schema: &str) -> Result<Self, StoreError> {
        let dsn = resolve_pg_dsn(|name| std::env::var(name).ok());
        Self::connect(&dsn, schema)
    }

    /// Append-only (s21 D1/D2): a single-record batch, so one write and a
    /// batch of one share the one code path.
    fn write_row(&self, record: &dyn StoredRecord) -> Result<WriteReceipt, StoreError> {
        Ok(self.write_rows(std::slice::from_ref(&record))?.remove(0))
    }

    /// The whole of one [`Tier::write_batch`] in ONE transaction (R7): every
    /// row is a plain `INSERT … ON CONFLICT (kind, id, digest) DO NOTHING`.
    /// A key that already holds the SAME JSON value is a no-op (`deduped:
    /// true`); a key that holds a DIFFERENT value is a
    /// [`StoreError::KeyConflict`] and the transaction rolls back, so none of
    /// the batch's rows (in any chunk) is kept. A new digest at the same
    /// `(kind, id)` is a genuine append. `location` carries the digest too
    /// (mirrors `GitTier`'s digest-suffixed path).
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
                let mut q = sqlx::query(sqlx::AssertSqlSafe(insert_batch_sql(&self.schema, end - start)));
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
                    let stored = sqlx::query(sqlx::AssertSqlSafe(select_body_sql(&self.schema)))
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

    fn location(&self, kind: canon_model::envelope::RecordKind, key: &str, digest: &str) -> String {
        format!("{}.records_history[{}={key}]_{digest}", self.schema, kind.as_str())
    }

    /// Every row of `records_history`, in key order (`canon migrate` reads it
    /// to compute the rows' new keys).
    pub fn all_rows(&self) -> Result<Vec<StoredRow>, StoreError> {
        self.rt.block_on(async {
            let rows = sqlx::query(sqlx::AssertSqlSafe(all_rows_sql(&self.schema))).fetch_all(&self.pool).await.map_err(|e| StoreError::Sql(e.to_string()))?;
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

    /// Re-key rows in ONE transaction (`canon migrate`): each row gets its new
    /// `(kind, id, digest)` and body; a new key held by the same JSON drops the
    /// old row, held by other JSON is a [`StoreError::KeyConflict`] and nothing
    /// of the call is kept.
    pub fn rekey(&self, changes: &[RowRekey]) -> Result<(), StoreError> {
        self.rt.block_on(async {
            let mut tx = self.pool.begin().await.map_err(|e| StoreError::Sql(e.to_string()))?;
            for change in changes {
                let held = sqlx::query(sqlx::AssertSqlSafe(select_body_sql(&self.schema)))
                    .bind(&change.to.0)
                    .bind(&change.to.1)
                    .bind(&change.to.2)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| StoreError::Sql(e.to_string()))?
                    .map(|r| r.get::<sqlx::types::Json<serde_json::Value>, _>("body").0);
                if let (Some(held), true) = (&held, change.from != change.to) {
                    if *held != change.body {
                        return Err(StoreError::KeyConflict { location: self.location(canon_model::envelope::RecordKind::ALL.into_iter().find(|k| k.as_str() == change.to.0).unwrap_or(canon_model::envelope::RecordKind::Change), &change.to.1, &change.to.2) });
                    }
                    sqlx::query(sqlx::AssertSqlSafe(delete_sql(&self.schema)))
                        .bind(&change.from.0)
                        .bind(&change.from.1)
                        .bind(&change.from.2)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| StoreError::Sql(e.to_string()))?;
                    continue;
                }
                sqlx::query(sqlx::AssertSqlSafe(rekey_sql(&self.schema)))
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

    fn read_rows(&self, query: &TierQuery, older_than: Option<DateTime<Utc>>) -> Result<Vec<RawRecord>, StoreError> {
        self.rt.block_on(async {
            let sql = match (older_than, query.since) {
                (Some(_), _) => select_older_than_sql(&self.schema),
                (None, Some(_)) => select_sql(&self.schema, true),
                (None, None) => select_sql(&self.schema, false),
            };
            let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(query.kind.as_str());
            q = if let Some(cutoff) = older_than { q.bind(cutoff) } else if let Some(since) = query.since { q.bind(since) } else { q };
            let rows = q.fetch_all(&self.pool).await.map_err(|e| StoreError::Sql(e.to_string()))?;
            Ok(rows.into_iter().map(|r| RawRecord(r.get::<sqlx::types::Json<serde_json::Value>, _>("body").0)).collect())
        })
    }
}

impl Tier for PgTier {
    fn backend(&self) -> Backend {
        Backend::Postgres
    }

    fn write(&self, record: &dyn StoredRecord) -> Result<WriteReceipt, StoreError> {
        self.write_row(record)
    }

    /// `PgTier`'s override of [`Tier::write_batch`] (s31 design D2):
    /// chunks `records` into ≤[`WRITE_BATCH_CHUNK_SIZE`]-row groups,
    /// each persisted via [`Self::write_chunk`]'s single multi-row
    /// statement inside its own transaction. Receipts come back in
    /// `records`' original order, byte-for-byte the same shape
    /// [`Self::write_row`]'s single-row path produces for each record
    /// — the s31 "batch == loop" equivalence tests hold this override
    /// to that bar.
    fn write_batch(&self, records: &[&dyn StoredRecord]) -> Result<Vec<WriteReceipt>, StoreError> {
        self.write_rows(records)
    }

    /// Never pre-folds (s21 D5) — returns every retained historical
    /// row for `query.kind` (optionally `since`-filtered), the SAME
    /// raw contract `GitTier`/`R2Tier::read` honor. Resolving "the
    /// current value for a key" is the caller's job
    /// ([`crate::fold::fold_latest_by_key`]).
    fn read(&self, query: &TierQuery) -> Result<TierReadResult, StoreError> {
        let records = self.read_rows(query, None)?;
        Ok(TierReadResult { records, violations: Vec::new() })
    }
    fn purge_keys(&self, kind: canon_model::envelope::RecordKind, before: DateTime<Utc>) -> Result<Vec<String>, StoreError> {
        self.rt.block_on(async {
            let rows = sqlx::query(sqlx::AssertSqlSafe(format!("SELECT id, body FROM {}.records_history WHERE kind = $1 AND at < $2 ORDER BY id, digest", self.schema)))
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
    fn purge_before(&self, kind: canon_model::envelope::RecordKind, before: DateTime<Utc>, dry_run: bool) -> Result<PurgeCount, StoreError> {
        self.rt.block_on(async {
            let matched = usize::try_from(sqlx::query(sqlx::AssertSqlSafe(purge_count_sql(&self.schema)))
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
                usize::try_from(sqlx::query(sqlx::AssertSqlSafe(purge_delete_sql(&self.schema)))
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
            // Computed BEFORE `raw` moves into `RawWrite` below — the
            // per-ROW identity `delete_sql` deletes by (s21 D1/task
            // 3.5: several rows can now share this `(kind, id)`, so
            // aging must delete exactly the ONE version it just moved,
            // never every version under this key).
            let name = resolve_name(rule.kind, &raw.0)?;
            let receipt = rule.destination.write(&RawWrite(raw))?;
            if receipt.deduped {
                already_aged += 1;
            } else {
                moved += 1;
            }
            self.rt.block_on(async {
                sqlx::query(sqlx::AssertSqlSafe(delete_sql(&self.schema)))
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
    use super::*;

    #[test]
    fn create_table_sql_is_idempotent_schema_qualified_and_keyed_by_kind_id_digest() {
        let sql = create_table_sql("canon_v1");
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS canon_v1.records_history"));
        assert!(sql.contains("PRIMARY KEY (kind, id, digest)"), "s21 D2: the idempotence key includes digest, never a bare (kind, id)");
    }

    #[test]
    fn insert_batch_sql_binds_n_five_wide_tuples_and_returns_the_full_row_identity() {
        let sql = insert_batch_sql("canon_v1", 3);
        assert!(sql.contains("ON CONFLICT (kind, id, digest) DO NOTHING"), "s21 D1: batched inserts stay append-only, never DO UPDATE");
        assert!(!sql.to_uppercase().contains("DO UPDATE"));
        assert!(sql.contains("VALUES ($1, $2, $3, $4, $5), ($6, $7, $8, $9, $10), ($11, $12, $13, $14, $15)"), "each row is its own 5-placeholder tuple, numbered contiguously: {sql}");
        assert!(sql.contains("RETURNING kind, id, digest"), "the caller reconstructs each row's own receipt by full identity, not by return-row ordinal");
        assert!(sql.contains("INSERT INTO canon_v1.records_history"));
    }

    #[test]
    fn a_key_conflict_is_decided_by_comparing_the_stored_jsonb_body_to_the_incoming_one() {
        // R7: `DO NOTHING` alone would report a digest collision as deduped;
        // the writer reads the held row's body by its FULL identity and
        // compares parsed JSON values (JSONB keeps no bytes).
        let sql = select_body_sql("canon_v1");
        assert!(sql.contains("SELECT body FROM canon_v1.records_history"));
        assert!(sql.contains("kind = $1 AND id = $2 AND digest = $3"), "{sql}");
    }

    #[test]
    fn rekey_sql_changes_the_key_and_body_of_exactly_one_row_identity() {
        let sql = rekey_sql("canon_v1");
        assert!(sql.starts_with("UPDATE canon_v1.records_history SET kind = $1, id = $2, digest = $3, body = $4"), "{sql}");
        assert!(sql.ends_with("WHERE kind = $5 AND id = $6 AND digest = $7"), "the old row is selected by its full identity: {sql}");
        assert!(!sql.contains(" at "), "`at` never changes");
    }

    #[test]
    fn insert_batch_sql_of_one_row_matches_insert_sql_shape() {
        let batch = insert_batch_sql("canon_v1", 1);
        assert!(batch.contains("VALUES ($1, $2, $3, $4, $5)"));
        assert!(batch.contains("ON CONFLICT (kind, id, digest) DO NOTHING"));
    }

    #[test]
    fn write_batch_chunk_size_is_five_hundred() {
        // s31 design D2's own number — a drifted constant would
        // silently change the transaction-per-chunk boundary.
        assert_eq!(WRITE_BATCH_CHUNK_SIZE, 500);
    }

    #[test]
    fn select_sql_filters_by_since_only_when_requested() {
        assert!(!select_sql("canon_v1", false).contains("at >="));
        assert!(select_sql("canon_v1", true).contains("at >= $2"));
    }

    #[test]
    fn select_sql_carries_no_dedup_or_fold_clause() {
        // s21 D5: the raw, unfolded multi-version read is the point —
        // no DISTINCT ON/GROUP BY collapsing rows before the caller's
        // own `fold_latest_by_key` gets to see every version.
        for sql in [select_sql("canon_v1", false), select_sql("canon_v1", true), select_older_than_sql("canon_v1")] {
            let upper = sql.to_uppercase();
            assert!(!upper.contains("DISTINCT"), "must not pre-fold: {sql}");
            assert!(!upper.contains("GROUP BY"), "must not pre-fold: {sql}");
        }
    }

    #[test]
    fn select_older_than_sql_filters_strictly_less_than() {
        assert!(select_older_than_sql("canon_v1").contains("at < $2"));
    }

    #[test]
    fn delete_sql_is_keyed_by_the_full_kind_id_digest_row_identity() {
        // s21 task 3.5: several rows can share a bare (kind, id) now —
        // aging must delete exactly the ONE version it just moved.
        let sql = delete_sql("canon_v1");
        assert!(sql.contains("WHERE kind = $1 AND id = $2 AND digest = $3"));
    }

    #[test]
    fn schema_identifier_is_validated_against_sql_injection() {
        validate_schema_ident("canon_v1").unwrap();
        assert!(validate_schema_ident("canon_v1; DROP TABLE users; --").is_err());
        assert!(validate_schema_ident("").is_err());
    }

    #[test]
    fn resolve_pg_dsn_tier1_service_specific_override_wins_when_set() {
        let dsn = resolve_pg_dsn(|name| match name {
            "CANON_PG_DSN" => Some("postgres://tier1-specific".to_string()),
            "DATABASE_URL" => Some("postgres://tier2-generic".to_string()),
            _ => None,
        });
        assert_eq!(dsn, "postgres://tier1-specific");
    }

    #[test]
    fn resolve_pg_dsn_tier2_generic_database_url_used_when_service_specific_unset() {
        let dsn = resolve_pg_dsn(|name| match name {
            "DATABASE_URL" => Some("postgres://tier2-generic".to_string()),
            _ => None,
        });
        assert_eq!(dsn, "postgres://tier2-generic");
    }

    #[test]
    fn resolve_pg_dsn_tier3_docker_compose_local_default_when_both_unset() {
        let dsn = resolve_pg_dsn(|_| None);
        assert_eq!(dsn, "postgres://canon:canon@127.0.0.1:55432/canon_v1");
    }

    /// s29 design D7: the INITIAL `PgPoolOptions::connect` failure
    /// (sqlx's `connect()` is EAGER, not lazy — it acquires one real
    /// connection before returning) must classify as
    /// `TierUnavailable`, never `Sql`, so `canon-cli::tiers::
    /// attach_postgres`'s `TierUnavailable`-only lenient-degrade catch
    /// genuinely covers a live outage instead of hard-failing a
    /// documented "degrade, don't abort" path. Port 1 on loopback
    /// refuses the connection immediately (no listener, no real
    /// network hop out of the sandbox), so this stays offline and
    /// fast — no live Postgres, no `live-pg` feature.
    #[test]
    fn connect_time_outage_classifies_as_tier_unavailable_not_sql() {
        let Err(err) = PgTier::connect("postgres://user@127.0.0.1:1/db", "canon_v1") else {
            panic!("an unroutable DSN must fail to connect")
        };
        assert!(
            matches!(err, StoreError::TierUnavailable { backend: Some(Backend::Postgres), .. }),
            "a connect-time outage must classify as TierUnavailable(postgres), got {err:?}"
        );
    }
}
