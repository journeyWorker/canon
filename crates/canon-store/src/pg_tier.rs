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

use crate::partition::{content_digest12, resolve_partition};
use crate::policy::{Backend, Rung};
use crate::tier::{AgeReport, AgingRule, RawWrite, StoreError, StoredRecord, Tier, TierQuery, TierReadResult, WriteReceipt};

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

/// INSERT-only (s21 D1/D2): `ON CONFLICT (kind, id, digest) DO
/// NOTHING` — a byte-identical resubmission at the same key never
/// lands a second row and never updates the existing one; ANY new
/// digest is a genuine new row, an append, never an overwrite. Uses
/// `RETURNING kind` (rather than a computed boolean column, the
/// pre-s21 `existed_with_same_digest` shape) purely as the "did this
/// row actually land" signal: a `DO NOTHING`-suppressed insert returns
/// zero rows, a landed insert returns exactly one — [`PgTier::write_row`]
/// reads `deduped` off row PRESENCE, not a computed column.
fn insert_sql(schema: &str) -> String {
    format!(
        "INSERT INTO {schema}.records_history (kind, id, at, digest, body) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (kind, id, digest) DO NOTHING \
         RETURNING kind"
    )
}

/// [`PgTier::write_batch`]'s chunk size (s31 design D2): 500 rows per
/// multi-row `INSERT` statement — large enough that a typical ingest
/// pass's batch (a session's worth of events) lands in a single
/// statement, small enough to stay well under Postgres' bind-parameter
/// limit (`n * 5` params per chunk; 500 rows = 2500 params, comfortably
/// under the wire protocol's 65535 cap) and to keep each chunk's
/// transaction short-lived.
const WRITE_BATCH_CHUNK_SIZE: usize = 500;

/// Multi-row form of [`insert_sql`] (s31 design D2: `PgTier::write_batch`'s
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
            sqlx::query(sqlx::AssertSqlSafe(create_schema_sql(schema))).execute(&pool).await.map_err(|e| StoreError::Sql(e.to_string()))?;
            sqlx::query(sqlx::AssertSqlSafe(create_table_sql(schema))).execute(&pool).await.map_err(|e| StoreError::Sql(e.to_string()))?;
            sqlx::query(sqlx::AssertSqlSafe(create_index_sql(schema))).execute(&pool).await.map_err(|e| StoreError::Sql(e.to_string()))?;
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

    /// Append-only (s21 D1/D2): every write is a plain `INSERT …
    /// ON CONFLICT (kind, id, digest) DO NOTHING` — a byte-identical
    /// resubmission at the same key is a no-op (`deduped: true`, row
    /// count unchanged); ANY new digest at the same `(kind, id)`
    /// lands as a brand-new row, never an overwrite of a prior
    /// version. `location` carries the digest too (mirrors
    /// `GitTier`'s digest-suffixed path), since a pg-tier location is
    /// now per-VERSION identity, not per-key.
    fn write_row(&self, record: &dyn StoredRecord) -> Result<WriteReceipt, StoreError> {
        let kind = record.kind();
        let raw = record.to_raw();
        let key = resolve_partition(kind, &raw.0)?;
        let digest = content_digest12(&raw.0);
        let at = record.at();

        let inserted = self.rt.block_on(async {
            let row = sqlx::query(sqlx::AssertSqlSafe(insert_sql(&self.schema)))
                .bind(kind.as_str())
                .bind(&key.natural_key)
                .bind(at)
                .bind(&digest)
                .bind(sqlx::types::Json(&raw.0))
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| StoreError::Sql(e.to_string()))?;
            Ok::<bool, StoreError>(row.is_some())
        })?;

        Ok(WriteReceipt {
            kind,
            location: format!("{}.records_history[{}={}]__{}", self.schema, kind.as_str(), key.natural_key, digest),
            digest,
            deduped: !inserted,
        })
    }

    /// One ≤[`WRITE_BATCH_CHUNK_SIZE`]-row chunk of `Tier::write_batch`
    /// (s31 design D2): a SINGLE multi-row `INSERT … ON CONFLICT DO
    /// NOTHING` inside its OWN transaction — one transaction per
    /// chunk, never one per row and never one spanning multiple
    /// chunks. Per-row semantics mirror [`Self::write_row`] exactly
    /// (s21 append-only contract): a byte-identical resubmission lands
    /// zero new rows for that row's `(kind, id, digest)`, and its
    /// receipt reports `deduped: true`, byte-for-byte the same
    /// `WriteReceipt` shape `write_row` would have produced for it.
    fn write_chunk(&self, records: &[&dyn StoredRecord]) -> Result<Vec<WriteReceipt>, StoreError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }

        // Resolve every row's persisted shape BEFORE opening the
        // transaction: a malformed record (`resolve_partition` error)
        // is an ordinary per-record `Err`, never something that should
        // abort a half-open transaction.
        let mut kinds = Vec::with_capacity(records.len());
        let mut keys = Vec::with_capacity(records.len());
        let mut ats = Vec::with_capacity(records.len());
        let mut digests = Vec::with_capacity(records.len());
        let mut bodies = Vec::with_capacity(records.len());
        for record in records {
            let kind = record.kind();
            let raw = record.to_raw();
            let key = resolve_partition(kind, &raw.0)?;
            digests.push(content_digest12(&raw.0));
            kinds.push(kind);
            ats.push(record.at());
            keys.push(key.natural_key);
            bodies.push(raw.0);
        }

        let landed: HashSet<(String, String, String)> = self.rt.block_on(async {
            let mut tx = self.pool.begin().await.map_err(|e| StoreError::Sql(e.to_string()))?;
            let mut q = sqlx::query(sqlx::AssertSqlSafe(insert_batch_sql(&self.schema, records.len())));
            for i in 0..records.len() {
                q = q.bind(kinds[i].as_str()).bind(&keys[i]).bind(ats[i]).bind(&digests[i]).bind(sqlx::types::Json(&bodies[i]));
            }
            let rows = q.fetch_all(&mut *tx).await.map_err(|e| StoreError::Sql(e.to_string()))?;
            let landed = rows
                .into_iter()
                .map(|r| (r.get::<String, _>("kind"), r.get::<String, _>("id"), r.get::<String, _>("digest")))
                .collect();
            tx.commit().await.map_err(|e| StoreError::Sql(e.to_string()))?;
            Ok::<_, StoreError>(landed)
        })?;

        Ok((0..records.len())
            .map(|i| {
                let inserted = landed.contains(&(kinds[i].as_str().to_string(), keys[i].clone(), digests[i].clone()));
                WriteReceipt {
                    kind: kinds[i],
                    location: format!("{}.records_history[{}={}]__{}", self.schema, kinds[i].as_str(), keys[i], digests[i]),
                    digest: digests[i].clone(),
                    deduped: !inserted,
                }
            })
            .collect())
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
        let mut receipts = Vec::with_capacity(records.len());
        for chunk in records.chunks(WRITE_BATCH_CHUNK_SIZE) {
            receipts.extend(self.write_chunk(chunk)?);
        }
        Ok(receipts)
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

    fn age(&self, rule: &AgingRule) -> Result<AgeReport, StoreError> {
        let cutoff = Utc::now() - rule.after;
        let candidates = self.read_rows(&TierQuery::kind(rule.kind), Some(cutoff))?;

        let mut moved = 0;
        let mut already_aged = 0;
        for raw in candidates {
            let key = resolve_partition(rule.kind, &raw.0)?;
            // Computed BEFORE `raw` moves into `RawWrite` below — the
            // per-ROW identity `delete_sql` deletes by (s21 D1/task
            // 3.5: several rows can now share this `(kind, id)`, so
            // aging must delete exactly the ONE version it just moved,
            // never every version under this key).
            let digest = content_digest12(&raw.0);
            let receipt = rule.destination.write(&RawWrite(raw))?;
            if receipt.deduped {
                already_aged += 1;
            } else {
                moved += 1;
            }
            self.rt.block_on(async {
                sqlx::query(sqlx::AssertSqlSafe(delete_sql(&self.schema)))
                    .bind(rule.kind.as_str())
                    .bind(&key.natural_key)
                    .bind(&digest)
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
    fn insert_sql_conflicts_on_kind_id_digest_and_does_nothing_never_updates() {
        let sql = insert_sql("canon_v1");
        assert!(sql.contains("ON CONFLICT (kind, id, digest) DO NOTHING"), "s21 D1: append-only, never an UPDATE on conflict");
        assert!(sql.contains("INSERT INTO canon_v1.records_history"));
        assert!(!sql.to_uppercase().contains("DO UPDATE"), "must never fall back to an upsert-in-place");
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
        assert!(validate_schema_ident("canon_v1").is_ok());
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
        let err = match PgTier::connect("postgres://user@127.0.0.1:1/db", "canon_v1") {
            Ok(_) => panic!("an unroutable DSN must fail to connect"),
            Err(e) => e,
        };
        assert!(
            matches!(err, StoreError::TierUnavailable { backend: Some(Backend::Postgres), .. }),
            "a connect-time outage must classify as TierUnavailable(postgres), got {err:?}"
        );
    }
}
