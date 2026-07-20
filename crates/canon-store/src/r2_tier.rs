//! `R2Tier`: the cold, shared tier (design D1/D3) — parquet via `arrow`/
//! `parquet`, written directly (no DuckLake/DuckDB catalog process at
//! write time — a prior session-store storage audit's own risk note:
//! canon's write path may not go through DuckDB at all, only the
//! resulting file LAYOUT needs to be DuckLake/`read_parquet`-compatible)
//! to a `canon.yaml`-configured bucket/prefix (default `canon/`), one
//! Hive-partitioned parquet object per write:
//! `<prefix>kind={kind}/[area={area}/]<natural_key>__<digest12>.parquet`
//! — the SAME [`crate::partition::hive_object_key`] coordinate scheme
//! `GitTier` uses, so a git-tier and r2-tier listing of the same kind
//! are directory-shape-identical modulo extension.
//!
//! Storage access goes through [`object_store::ObjectStore`] — a LOCAL
//! filesystem-backed store for fully offline tests
//! ([`R2Tier::local`]) and an S3-compatible store (MinIO by default,
//! against the repo-root `docker-compose.yml` stack; any other
//! S3-compatible endpoint including real Cloudflare R2 by overriding
//! its env config) for live use ([`R2Tier::connect_live`], gated by
//! the `live-r2` feature at every call site in this crate's own
//! tests, per the S2 assignment's local-first constraint). The TIER
//! LOGIC (path derivation, parquet encode/decode, digest-dedup) is
//! identical in both cases — only which `ObjectStore` impl backs it
//! differs.

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use canon_model::envelope::RecordKind;
use canon_model::evidence::{validate_envelope_shape, EvidenceViolation, RawRecord};
use canon_model::FailureClass;
use futures::StreamExt;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;

use crate::partition::{content_digest12, hive_object_key, resolve_partition, validate_body, validate_kind_matches_content};
use crate::policy::Backend;
use crate::tier::{raw_record_at, AgeReport, AgingRule, StoreError, StoredRecord, Tier, TierQuery, TierReadResult, WriteReceipt};

/// The one Arrow/parquet schema every r2-tier object uses — uniform
/// across all twelve record kinds (design's Risk-section "content-
/// trusted extraction" principle, applied here by materializing
/// `kind`/`natural_key`/`at`/`digest` as real typed columns straight
/// from the record's own content at write time, rather than asking a
/// downstream reader to re-derive them from `body` or trust the object
/// key's path).
fn arrow_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("kind", DataType::Utf8, false),
        Field::new("natural_key", DataType::Utf8, false),
        Field::new("at", DataType::Utf8, false),
        Field::new("digest", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),
    ]))
}

fn encode_row(kind: RecordKind, natural_key: &str, at: &str, digest: &str, body_json: &str) -> Result<Vec<u8>, StoreError> {
    let schema = arrow_schema();
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec![kind.as_str()])),
        Arc::new(StringArray::from(vec![natural_key])),
        Arc::new(StringArray::from(vec![at])),
        Arc::new(StringArray::from(vec![digest])),
        Arc::new(StringArray::from(vec![body_json])),
    ];
    let batch = RecordBatch::try_new(schema.clone(), columns).map_err(|e| StoreError::Parquet(e.to_string()))?;

    let mut buf = Vec::new();
    let mut writer = ArrowWriter::try_new(&mut buf, schema, None).map_err(|e| StoreError::Parquet(e.to_string()))?;
    writer.write(&batch).map_err(|e| StoreError::Parquet(e.to_string()))?;
    writer.close().map_err(|e| StoreError::Parquet(e.to_string()))?;
    Ok(buf)
}

/// One r2-tier object's rows, decoded and validated (design D4):
/// `body` must parse as JSON and pass the SAME envelope-shape / per-
/// kind-body checks `GitTier::scan_kind_where` runs before trusting a
/// row enough to reach the panic-based [`raw_record_at`]
/// (`canon_model::evidence::validate_envelope_shape`,
/// [`crate::partition::validate_kind_matches_content`],
/// [`crate::partition::validate_body`]), plus a `digest` column
/// agreeing with the content digest [`R2Tier::write`] itself computed
/// at write time. A row failing any of those checks becomes one
/// [`EvidenceViolation`] naming `path` (only `decode_rows`'s caller
/// knows the object's own location) rather than aborting the whole
/// object's read — the exact soft-skip contract
/// `TierReadResult.violations` documents. The parquet CONTAINER
/// itself failing to parse (corrupt/foreign bytes, a missing expected
/// column) is a distinct, non-row-scoped failure with nothing to skip
/// past — still a `StoreError`, exactly as before this change.
fn decode_rows(bytes: bytes::Bytes, path: &ObjectPath, kind: RecordKind) -> Result<(Vec<RawRecord>, Vec<EvidenceViolation>), StoreError> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(bytes).map_err(|e| StoreError::Parquet(e.to_string()))?.build().map_err(|e| StoreError::Parquet(e.to_string()))?;

    let mut records = Vec::new();
    let mut violations = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|e| StoreError::Parquet(e.to_string()))?;
        let body_col = batch
            .column_by_name("body")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| StoreError::Parquet("parquet batch missing `body` utf8 column".to_string()))?;
        let digest_col = batch
            .column_by_name("digest")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| StoreError::Parquet("parquet batch missing `digest` utf8 column".to_string()))?;
        for i in 0..batch.num_rows() {
            match validate_row(kind, body_col.value(i), digest_col.value(i)) {
                Ok(record) => records.push(record),
                Err(violation) => violations.push(EvidenceViolation::new(FailureClass::Malformed, path.to_string(), violation.to_string())),
            }
        }
    }
    Ok((records, violations))
}

/// [`decode_rows`]'s per-row validator: parse `body_json`, confirm its
/// envelope/content is well-formed for `kind`, and confirm `digest`
/// (the row's own parquet column, computed by
/// [`crate::partition::content_digest12`] at write time) still agrees
/// with the content — a stale/tampered `digest` column is exactly as
/// much a violation as a stale/tampered `body`. Design D4's "kind/id/
/// digest presence" wording maps onto three checks: `kind` via
/// `validate_envelope_shape` + `validate_kind_matches_content`,
/// per-kind "id" (each kind's own natural-key field — `canon-model`
/// gives no record a bare `id` field of its own) via `validate_body`'s
/// full `Deserialize` attempt, and `digest` via the comparison below.
fn validate_row(kind: RecordKind, body_json: &str, digest: &str) -> Result<RawRecord, EvidenceViolation> {
    let json: serde_json::Value = serde_json::from_str(body_json).map_err(|e| EvidenceViolation::new(FailureClass::Malformed, "body", e.to_string()))?;
    let raw = RawRecord(json);
    validate_envelope_shape(&raw)?;
    validate_kind_matches_content(kind, &raw.0)?;
    validate_body(kind, &raw)?;

    let expected_digest = content_digest12(&raw.0);
    if digest.is_empty() || digest != expected_digest {
        return Err(EvidenceViolation::new(
            FailureClass::Malformed,
            "digest",
            format!("stored digest `{digest}` disagrees with content digest `{expected_digest}`"),
        ));
    }
    Ok(raw)
}

/// [`R2Tier::connect_live`]'s bucket-name resolution cascade
/// (data-stores Pattern 6 — "Env-var credential fallback chain:
/// specific → shared → generic", the donor data-stores adoption brief
/// §Pattern 6): (1) `bucket_env` —
/// `canon.yaml`'s `tiers.r2.bucket_env`-named override, tier 1,
/// unchanged from before this cascade; (2) else `S3_BUCKET` — a
/// generic bucket-name convention (the donor data-stores
/// credentials-config-distribution notes §2.2 names this exact generic
/// name; unlike that same section's `S3_ACCESS_KEY`/
/// `S3_ACCESS_SECRET_KEY`/`S3_HOST` — flagged there as documented-but-
/// dead code in the donor's own `r2SecretFromEnv()` and explicitly
/// NOT ported, §5 action 4 — `S3_BUCKET` is a distinct, still-live
/// convention this cascade legitimately adds), tier 2; (3) else
/// `None` — deliberately NO local-default bucket name: silently
/// defaulting a BUCKET (unlike a DSN) risks the exact "wrong-bucket
/// footgun" the donor's own `R2_ICEBERG_*` vs generic-`S3_*` credential
/// separation warns against (credentials-config-distribution.md §3.2)
/// — [`R2Tier::connect_live`] turns a `None` here into its existing
/// hard-failure contract, unchanged.
///
/// Parameterized over `lookup` (rather than reading `std::env::var`
/// directly) so this ordering is unit-testable without racing real
/// process env vars across the crate's parallel test threads.
fn resolve_r2_bucket(bucket_env: &str, lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    lookup(bucket_env).or_else(|| lookup("S3_BUCKET"))
}

/// The four fields [`resolve_s3_connection`] resolves before
/// [`R2Tier::connect_live`] builds an `AmazonS3Builder` — a small
/// owned struct rather than a raw tuple so both the doc above and the
/// call site name each field.
#[derive(Debug)]
struct S3Connection {
    endpoint: String,
    access_key_id: String,
    secret_access_key: String,
    region: String,
}

/// [`R2Tier::connect_live`]'s S3 connection builder (design D1,
/// mirrors [`crate::pg_tier::resolve_pg_dsn`]'s `lookup`-parameterized
/// pattern): reads `CANON_S3_ENDPOINT`/`CANON_S3_ACCESS_KEY`/
/// `CANON_S3_SECRET_KEY`/`CANON_S3_REGION` through `lookup` rather
/// than `std::env::var` directly, so BOTH the `strict` and non-strict
/// branches below are unit-testable without racing real process env
/// vars across the crate's parallel test threads —
/// [`R2Tier::connect_live`] is the only real caller, passing
/// `std::env::var` itself and `strict = cfg!(not(debug_assertions))`
/// (the same release/debug boundary `canon-cli/src/tiers.rs`'s
/// `CANON_R2_LOCAL_ROOT` substitution already uses — D1 reuses it
/// rather than inventing a `--dev` flag or a new env var).
///
/// - `strict = true` (a release build): `CANON_S3_ENDPOINT`,
///   `CANON_S3_ACCESS_KEY`, and `CANON_S3_SECRET_KEY` are ALL
///   required — a missing one fails with ONE
///   `StoreError::BackendUnattached` naming EVERY unset var in one
///   message (an operator fixes the whole set in one round trip, not
///   one var per failure), never silently resolving to the
///   docker-compose MinIO defaults.
/// - `strict = false` (a debug build/test): the same three vars keep
///   the zero-env docker-compose MinIO defaults
///   (`http://127.0.0.1:59000`, `canon`/`canoncanon`), byte-identical
///   to pre-s29 behavior.
/// - `CANON_S3_REGION` keeps its `us-east-1` default in BOTH modes —
///   a wrong region is not the silent-misdirection risk a defaulted
///   loopback endpoint/credential pair is.
fn resolve_s3_connection(strict: bool, lookup: impl Fn(&str) -> Option<String>) -> Result<S3Connection, StoreError> {
    let region = lookup("CANON_S3_REGION").unwrap_or_else(|| "us-east-1".to_string());
    if !strict {
        return Ok(S3Connection {
            endpoint: lookup("CANON_S3_ENDPOINT").unwrap_or_else(|| "http://127.0.0.1:59000".to_string()),
            access_key_id: lookup("CANON_S3_ACCESS_KEY").unwrap_or_else(|| "canon".to_string()),
            secret_access_key: lookup("CANON_S3_SECRET_KEY").unwrap_or_else(|| "canoncanon".to_string()),
            region,
        });
    }

    let endpoint = lookup("CANON_S3_ENDPOINT");
    let access_key_id = lookup("CANON_S3_ACCESS_KEY");
    let secret_access_key = lookup("CANON_S3_SECRET_KEY");
    let missing: Vec<&str> = [
        (endpoint.is_none(), "CANON_S3_ENDPOINT"),
        (access_key_id.is_none(), "CANON_S3_ACCESS_KEY"),
        (secret_access_key.is_none(), "CANON_S3_SECRET_KEY"),
    ]
    .into_iter()
    .filter_map(|(unset, name)| unset.then_some(name))
    .collect();
    if !missing.is_empty() {
        return Err(StoreError::BackendUnattached {
            backend: Backend::S3,
            reason: format!("strict S3 attach requires {} (a release build never defaults S3 credentials, design D1)", missing.join(", ")),
        });
    }

    Ok(S3Connection { endpoint: endpoint.unwrap(), access_key_id: access_key_id.unwrap(), secret_access_key: secret_access_key.unwrap(), region })
}

pub struct R2Tier {
    store: Arc<dyn ObjectStore>,
    prefix: String,
    rt: tokio::runtime::Runtime,
}

impl std::fmt::Debug for R2Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("R2Tier").field("prefix", &self.prefix).finish_non_exhaustive()
    }
}

impl R2Tier {
    pub fn with_object_store(store: Arc<dyn ObjectStore>, prefix: impl Into<String>) -> Result<Self, StoreError> {
        let rt = tokio::runtime::Runtime::new().map_err(StoreError::Io)?;
        Ok(Self { store, prefix: prefix.into(), rt })
    }

    /// A local-filesystem-backed r2 tier — the offline test/dev
    /// substitute for a real R2 bucket (design §9 local-first: the
    /// tier LOGIC — Hive path derivation, parquet encode/decode,
    /// digest-dedup — is exercised identically here and against live
    /// R2, only the `ObjectStore` backend differs).
    pub fn local(root: impl AsRef<std::path::Path>, prefix: impl Into<String>) -> Result<Self, StoreError> {
        std::fs::create_dir_all(root.as_ref())?;
        let fs = object_store::local::LocalFileSystem::new_with_prefix(root.as_ref()).map_err(|e| StoreError::ObjectStore(e.to_string()))?;
        Self::with_object_store(Arc::new(fs), prefix)
    }

    /// Attach to an S3-compatible object store — MinIO by default (the
    /// local docker-compose stack this crate's `live-r2` tests target;
    /// repo-root `docker-compose.yml`, operator directive 2026-07-10's
    /// local-first retrofit), or any other S3-compatible endpoint
    /// (including real Cloudflare R2 — set `CANON_S3_ENDPOINT` to
    /// `https://<account_id>.r2.cloudflarestorage.com` and
    /// `CANON_S3_REGION=auto`) by overriding the env vars below.
    ///
    /// `bucket` resolves via [`resolve_r2_bucket`]'s cascade (data-stores
    /// Pattern 6, "Env-var credential fallback chain: specific → shared
    /// → generic"): (1) `bucket_env` — `canon.yaml`'s
    /// `tiers.r2.bucket_env`-named env var, unchanged from before this
    /// retrofit — tier 1; (2) else `S3_BUCKET` — a generic bucket-name
    /// convention a consumer repo may already have exported for OTHER
    /// S3-compatible tooling, tier 2; (3) else still a hard, no-default
    /// failure (`bucket_env` names an application-chosen, per-repo env
    /// var; a wrong or missing bucket is exactly the "explicitly-
    /// configured tier that can't attach is a startup-time hard
    /// failure" case, a prior session-store storage audit §3.2 — never
    /// silently defaulted to a made-up bucket name, and load-bearing
    /// for `canon-cli`'s own release-build safety net).
    ///
    /// The CONNECTION itself (endpoint/credentials/region) resolves via
    /// [`resolve_s3_connection`], `strict = cfg!(not(debug_assertions))`
    /// (design D1 — the same release/debug boundary `canon-cli/src/
    /// tiers.rs`'s `CANON_R2_LOCAL_ROOT` substitution already uses,
    /// reused rather than a new `--dev` flag or env var):
    /// - a RELEASE build (`strict = true`) requires `CANON_S3_ENDPOINT`,
    ///   `CANON_S3_ACCESS_KEY`, and `CANON_S3_SECRET_KEY` ALL set — a
    ///   missing one fails attachment with ONE `BackendUnattached`
    ///   naming EVERY unset var, so a release build never silently
    ///   builds a client pointed at `http://127.0.0.1:59000` with
    ///   `canon`/`canoncanon` (spec: "A release build never attaches
    ///   S3 with defaulted credentials").
    /// - a DEBUG build/test (`strict = false`) keeps the zero-env
    ///   docker-compose MinIO defaults, byte-identical to pre-s29
    ///   behavior — a local dev/CI run against the compose stack still
    ///   needs zero exported env vars beyond a bucket name:
    ///   - `CANON_S3_ENDPOINT` (default `http://127.0.0.1:59000`, the
    ///     compose `minio` service's published port)
    ///   - `CANON_S3_ACCESS_KEY` / `CANON_S3_SECRET_KEY` (default the
    ///     compose `minio` root creds, `canon`/`canoncanon`)
    /// - `CANON_S3_REGION` (default `us-east-1`; irrelevant to MinIO,
    ///   present for real-S3/R2 compatibility) keeps its default in
    ///   BOTH modes — a wrong region is not the silent-misdirection
    ///   risk a defaulted loopback endpoint/credential pair is (design
    ///   D1).
    ///
    /// Always path-style (`with_virtual_hosted_style_request(false)`).
    /// HTTP-permissive ONLY when the resolved endpoint itself starts
    /// with `http://` (`with_allow_http(endpoint.starts_with("http://"))`)
    /// — plaintext stays possible for an operator who explicitly
    /// configured a plaintext endpoint (MinIO), and impossible as an
    /// ambient default once `CANON_S3_ENDPOINT` is an operator-supplied
    /// `https://` URL. A genuinely unreachable endpoint surfaces as a
    /// `write`/`read`-time `StoreError::ObjectStore`, not a
    /// `connect_live`-time one — building an S3 client never itself
    /// performs network I/O (`object_store`'s own contract; see
    /// `tests/r2_tier_live.rs`'s own TCP probe for the "is MinIO even
    /// up" check this implies).
    pub fn connect_live(bucket_env: &str, prefix: &str) -> Result<Self, StoreError> {
        let bucket = resolve_r2_bucket(bucket_env, |name| std::env::var(name).ok())
            .ok_or_else(|| StoreError::BackendUnattached { backend: Backend::S3, reason: format!("`{bucket_env}` (and generic `S3_BUCKET`) are both unset") })?;
        let conn = resolve_s3_connection(cfg!(not(debug_assertions)), |name| std::env::var(name).ok())?;

        let store = object_store::aws::AmazonS3Builder::new()
            .with_bucket_name(&bucket)
            .with_allow_http(conn.endpoint.starts_with("http://"))
            .with_endpoint(conn.endpoint)
            .with_region(conn.region)
            .with_access_key_id(conn.access_key_id)
            .with_secret_access_key(conn.secret_access_key)
            .with_virtual_hosted_style_request(false)
            .build()
            .map_err(|e| StoreError::ObjectStore(e.to_string()))?;

        Self::with_object_store(Arc::new(store), prefix.to_string())
    }

    fn object_path(&self, relative: &std::path::Path) -> ObjectPath {
        ObjectPath::from(format!("{}{}", self.prefix, relative.display()))
    }

    /// `head` distinguishes absence from failure (design D5): only
    /// `object_store::Error::NotFound` means "not written yet" — any
    /// other HEAD error (e.g. credentials that permit PUT but deny
    /// HEAD) propagates as `StoreError::ObjectStore` rather than
    /// masquerading as `false`, which [`Tier::write`]'s dedupe-probe
    /// call site would otherwise silently misread as "safe to
    /// (re-)PUT and report `deduped: false`" (spec: "A denied HEAD
    /// does not masquerade as a fresh write").
    fn exists(&self, path: &ObjectPath) -> Result<bool, StoreError> {
        self.rt.block_on(async {
            match self.store.head(path).await {
                Ok(_) => Ok(true),
                Err(object_store::Error::NotFound { .. }) => Ok(false),
                Err(e) => Err(StoreError::ObjectStore(e.to_string())),
            }
        })
    }

    fn list_kind(&self, kind: RecordKind) -> Result<Vec<ObjectPath>, StoreError> {
        let list_prefix = ObjectPath::from(format!("{}kind={}", self.prefix, kind.as_str()));
        self.rt.block_on(async {
            let mut stream = self.store.list(Some(&list_prefix));
            let mut paths = Vec::new();
            while let Some(meta) = stream.next().await {
                let meta = meta.map_err(|e| StoreError::ObjectStore(e.to_string()))?;
                paths.push(meta.location);
            }
            Ok(paths)
        })
    }
}

impl Tier for R2Tier {
    fn backend(&self) -> Backend {
        Backend::S3
    }

    fn write(&self, record: &dyn StoredRecord) -> Result<WriteReceipt, StoreError> {
        let kind = record.kind();
        let raw = record.to_raw();
        let relative = hive_object_key(kind, &raw.0, "parquet")?;
        let object_path = self.object_path(&relative);
        let digest = content_digest12(&raw.0);

        if self.exists(&object_path)? {
            // Digest is part of the key (module doc) — an object
            // already present at this exact path is, by construction,
            // byte-identical content. This is `R2Tier`'s aging-idempotence
            // no-op (tier-policy spec: "the second run performs no
            // duplicate write... reports zero newly-aged records").
            return Ok(WriteReceipt { kind, location: object_path.to_string(), digest, deduped: true });
        }

        let key = resolve_partition(kind, &raw.0)?;
        let at = record.at().to_rfc3339();
        let body_json = serde_json::to_string(&raw.0)?;
        let bytes = encode_row(kind, &key.natural_key, &at, &digest, &body_json)?;

        self.rt.block_on(async {
            self.store.put(&object_path, PutPayload::from(bytes)).await.map_err(|e| StoreError::ObjectStore(e.to_string()))
        })?;

        Ok(WriteReceipt { kind, location: object_path.to_string(), digest, deduped: false })
    }

    fn read(&self, query: &TierQuery) -> Result<TierReadResult, StoreError> {
        let mut records = Vec::new();
        let mut violations = Vec::new();
        for path in self.list_kind(query.kind)? {
            let bytes = self.rt.block_on(async {
                let result = self.store.get(&path).await.map_err(|e| StoreError::ObjectStore(e.to_string()))?;
                result.bytes().await.map_err(|e| StoreError::ObjectStore(e.to_string()))
            })?;
            match decode_rows(bytes, &path, query.kind) {
                Ok((rows, row_violations)) => {
                    violations.extend(row_violations);
                    for record in rows {
                        if query.matches(raw_record_at(&record)) {
                            records.push(record);
                        }
                    }
                }
                Err(e) => violations.push(EvidenceViolation::new(FailureClass::Malformed, path.to_string(), e.to_string())),
            }
        }
        Ok(TierReadResult { records, violations })
    }

    fn age(&self, rule: &AgingRule) -> Result<AgeReport, StoreError> {
        // No `aging` entry in this repo's `canon.yaml` ever routes OUT
        // of `r2` (design context table: r2 is the terminal cold tier) —
        // a correct, documented no-op mirroring `GitTier::age` (see its
        // doc comment for the identical rationale), not an
        // unimplemented panic.
        let _ = rule;
        Ok(AgeReport { kind: rule.kind, moved: 0, already_aged: 0 })
    }
}


#[cfg(test)]
mod tests {
    use canon_model::envelope::{Actor, Envelope};
    use canon_model::ids::{ChangeId, RoleId, RunId};
    use canon_model::records::{Change, ChangeStatus, Trajectory};
    use chrono::Utc;

    use super::*;

    fn actor() -> Actor {
        Actor::new("test-agent", RoleId::parse("implementer").unwrap())
    }

    /// The Rust test harness runs tests in this file's own binary
    /// concurrently on separate threads (no `#[serial]` crate — this
    /// crate adds no new dev-dependencies); `S3_BUCKET` (this cascade's
    /// own new generic tier-2 name) is a real PROCESS-wide env var
    /// touched by more than one test below, so those tests must
    /// serialize against each other via this lock rather than race on
    /// `set_var`/`remove_var`. Every OTHER env var this file's tests
    /// touch (`CANON_S3_*`, each test's own uniquely-named `bucket_env`)
    /// is still touched by exactly one test, so it needs no lock.
    static S3_BUCKET_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn write_then_read_round_trips_via_local_object_store() {
        let dir = tempfile::tempdir().unwrap();
        let tier = R2Tier::local(dir.path(), "canon/").unwrap();
        let trajectory = Trajectory::new(
            Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor()),
            RunId::default(),
            None,
            None,
            None,
            None,
            Some(0.9),
        );
        let receipt = tier.write(&trajectory).unwrap();
        assert!(!receipt.deduped);
        assert!(receipt.location.starts_with("canon/kind=trajectory/"));

        let result = tier.read(&TierQuery::kind(RecordKind::Trajectory)).unwrap();
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].0["reward"], 0.9);
    }

    #[test]
    fn parquet_bytes_actually_land_on_local_disk_in_hive_layout() {
        let dir = tempfile::tempdir().unwrap();
        let tier = R2Tier::local(dir.path(), "canon/").unwrap();
        let change = Change::new(
            Envelope::new(1, RecordKind::Change, Utc::now(), actor()),
            ChangeId::parse("s2-tiered-storage").unwrap(),
            "S2",
            "x",
            ChangeStatus::Proposed,
        );
        tier.write(&change).unwrap();

        let found = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(Result::ok)
            .find(|e| e.path().extension().is_some_and(|ext| ext == "parquet"));
        assert!(found.is_some(), "expected a real .parquet file under kind=change/ on local disk");
        assert!(found.unwrap().path().to_string_lossy().contains("kind=change"));
    }

    #[test]
    fn duplicate_content_write_is_deduped_not_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let tier = R2Tier::local(dir.path(), "canon/").unwrap();
        let envelope = Envelope::new(1, RecordKind::Change, Utc::now(), actor());
        let change = Change::new(envelope, ChangeId::parse("s2-tiered-storage").unwrap(), "S2", "x", ChangeStatus::Proposed);
        let first = tier.write(&change).unwrap();
        assert!(!first.deduped);
        let second = tier.write(&change).unwrap();
        assert!(second.deduped, "identical content re-write must be a digest-dedup no-op, aging-idempotence contract");
        assert_eq!(first.digest, second.digest);

        let result = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap();
        assert_eq!(result.records.len(), 1, "no duplicate object written");
    }

    #[test]
    fn connect_live_without_bucket_env_fails_loud_not_silently() {
        // Env-var isolation: this test's own bucket_env name is unique
        // (never set anywhere else in this process); `S3_BUCKET` is
        // NOT unique (the tier-2 fallback test below also touches it),
        // so both serialize on `S3_BUCKET_ENV_LOCK`. Bucket resolution
        // is the ONE part of `connect_live`'s contract this retrofit
        // deliberately left with no default (module doc, `connect_live`'s
        // own doc comment) — `canon-cli`'s release-build safety net
        // depends on this staying a hard failure, never silently
        // defaulted, when BOTH the caller's own `bucket_env`-named var
        // AND the generic `S3_BUCKET` tier-2 fallback are unset.
        let _guard = S3_BUCKET_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::remove_var("CANON_R2_TEST_BUCKET_UNSET");
        std::env::remove_var("S3_BUCKET");
        let err = R2Tier::connect_live("CANON_R2_TEST_BUCKET_UNSET", "canon/").unwrap_err();
        assert!(matches!(err, StoreError::BackendUnattached { backend: Backend::S3, .. }));
    }

    #[test]
    fn connect_live_falls_back_to_docker_compose_defaults_when_s3_env_is_unset() {
        // Building an S3 client never performs network I/O
        // (`object_store`'s own contract, `connect_live`'s doc comment)
        // — so this stays a genuinely offline test even though it
        // exercises the docker-compose-default fallback path. Env-var
        // isolation: clear the four vars this test's own contract
        // covers; nothing else in this crate's test suite touches them.
        for var in ["CANON_S3_ENDPOINT", "CANON_S3_ACCESS_KEY", "CANON_S3_SECRET_KEY", "CANON_S3_REGION"] {
            std::env::remove_var(var);
        }
        std::env::set_var("CANON_R2_TEST_BUCKET_DEFAULTS", "canon");
        let result = R2Tier::connect_live("CANON_R2_TEST_BUCKET_DEFAULTS", "canon/");
        std::env::remove_var("CANON_R2_TEST_BUCKET_DEFAULTS");
        result.expect("connect_live must build an S3 client from docker-compose defaults with zero CANON_S3_* env vars set");
    }

    #[test]
    fn connect_live_falls_back_to_generic_s3_bucket_when_configured_name_is_unset() {
        // Tier-2 integration proof: the caller's own `bucket_env` name
        // is unset, but the generic `S3_BUCKET` fallback IS set —
        // `connect_live` must still succeed (never fail loud) by
        // resolving through tier 2, not just tier 1/tier 3. Serializes
        // on `S3_BUCKET_ENV_LOCK` against the fail-loud test above,
        // the only other test touching `S3_BUCKET`.
        let _guard = S3_BUCKET_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for var in ["CANON_S3_ENDPOINT", "CANON_S3_ACCESS_KEY", "CANON_S3_SECRET_KEY", "CANON_S3_REGION"] {
            std::env::remove_var(var);
        }
        std::env::remove_var("CANON_R2_TEST_BUCKET_TIER2_UNSET");
        std::env::set_var("S3_BUCKET", "canon");
        let result = R2Tier::connect_live("CANON_R2_TEST_BUCKET_TIER2_UNSET", "canon/");
        std::env::remove_var("S3_BUCKET");
        result.expect("connect_live must fall back to the generic S3_BUCKET env var when the configured bucket_env is unset");
    }

    #[test]
    fn resolve_r2_bucket_tier1_configured_name_wins_when_set() {
        let bucket = resolve_r2_bucket("MY_BUCKET_ENV", |name| match name {
            "MY_BUCKET_ENV" => Some("tier1-bucket".to_string()),
            "S3_BUCKET" => Some("tier2-bucket".to_string()),
            _ => None,
        });
        assert_eq!(bucket.as_deref(), Some("tier1-bucket"));
    }

    #[test]
    fn resolve_r2_bucket_tier2_generic_s3_bucket_used_when_configured_name_unset() {
        let bucket = resolve_r2_bucket("MY_BUCKET_ENV", |name| match name {
            "S3_BUCKET" => Some("tier2-bucket".to_string()),
            _ => None,
        });
        assert_eq!(bucket.as_deref(), Some("tier2-bucket"));
    }

    #[test]
    fn resolve_r2_bucket_tier3_none_when_both_unset() {
        let bucket = resolve_r2_bucket("MY_BUCKET_ENV", |_| None);
        assert_eq!(bucket, None);
    }

    #[test]
    fn read_reports_a_violation_and_keeps_reading_when_a_row_body_is_empty() {
        // design D4 / spec.md "R2 reads degrade malformed rows to
        // violations": a tampered/truncated object whose parquet
        // `body` decodes to `{}` (valid JSON, but none of
        // `validate_envelope_shape`'s required fields) must become
        // exactly one `EvidenceViolation` naming the object path —
        // never a `raw_record_at` `.expect()` panic, and never abort
        // the read of the OTHER, well-formed object under the same
        // kind.
        let dir = tempfile::tempdir().unwrap();
        let tier = R2Tier::local(dir.path(), "canon/").unwrap();
        let envelope = Envelope::new(1, RecordKind::Change, Utc::now(), actor());
        let change = Change::new(envelope, ChangeId::parse("s2-tiered-storage").unwrap(), "S2", "x", ChangeStatus::Proposed);
        tier.write(&change).unwrap();

        let bogus_bytes = encode_row(RecordKind::Change, "bogus", &Utc::now().to_rfc3339(), "000000000000", "{}").unwrap();
        let bogus_path = ObjectPath::from("canon/kind=change/bogus__000000000000.parquet");
        tier.rt.block_on(async { tier.store.put(&bogus_path, PutPayload::from(bogus_bytes)).await.unwrap() });

        let result = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap();
        assert_eq!(result.records.len(), 1, "the well-formed object still reads");
        assert_eq!(result.violations.len(), 1, "the malformed row becomes exactly one violation, not a panic");
        assert!(result.violations[0].subject.contains("bogus"), "violation names the object path: {:?}", result.violations[0]);
    }

    #[test]
    fn resolve_s3_connection_non_strict_keeps_docker_compose_defaults_when_unset() {
        let conn = resolve_s3_connection(false, |_| None).unwrap();
        assert_eq!(conn.endpoint, "http://127.0.0.1:59000");
        assert_eq!(conn.access_key_id, "canon");
        assert_eq!(conn.secret_access_key, "canoncanon");
        assert_eq!(conn.region, "us-east-1");
    }

    #[test]
    fn resolve_s3_connection_strict_succeeds_when_all_three_vars_are_set() {
        let conn = resolve_s3_connection(true, |name| match name {
            "CANON_S3_ENDPOINT" => Some("https://r2.example.com".to_string()),
            "CANON_S3_ACCESS_KEY" => Some("ak".to_string()),
            "CANON_S3_SECRET_KEY" => Some("sk".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(conn.endpoint, "https://r2.example.com");
        assert_eq!(conn.region, "us-east-1", "CANON_S3_REGION keeps its default even in strict mode (design D1)");
    }

    #[test]
    fn resolve_s3_connection_strict_names_every_unset_var_in_one_error() {
        // design D1 / spec.md "Release build with bucket but no
        // credentials fails loud": a release build with all three
        // vars unset must fail with ONE `BackendUnattached` naming
        // EVERY unset variable, not just the first.
        let err = resolve_s3_connection(true, |_| None).unwrap_err();
        let StoreError::BackendUnattached { backend, reason } = err else {
            panic!("expected BackendUnattached, got {err:?}");
        };
        assert_eq!(backend, Backend::S3);
        for var in ["CANON_S3_ENDPOINT", "CANON_S3_ACCESS_KEY", "CANON_S3_SECRET_KEY"] {
            assert!(reason.contains(var), "reason must name {var}: {reason}");
        }
    }

    #[test]
    fn resolve_s3_connection_strict_names_only_the_missing_vars() {
        let err = resolve_s3_connection(true, |name| (name == "CANON_S3_ENDPOINT").then(|| "https://r2.example.com".to_string())).unwrap_err();
        let StoreError::BackendUnattached { reason, .. } = err else { panic!("expected BackendUnattached") };
        assert!(!reason.contains("CANON_S3_ENDPOINT"), "the one SET var must not be named: {reason}");
        assert!(reason.contains("CANON_S3_ACCESS_KEY") && reason.contains("CANON_S3_SECRET_KEY"), "both unset vars must be named: {reason}");
    }

    /// A minimal [`ObjectStore`] whose `head` (via `ObjectStoreExt`'s
    /// blanket `get_opts`-based default) always fails with a
    /// non-`NotFound` error — simulates credentials that permit PUT
    /// but deny HEAD (design D5 / spec.md "A denied HEAD does not
    /// masquerade as a fresh write"). Every other method is
    /// unreachable from `R2Tier::exists`'s single `head` call, so
    /// they're left `unimplemented!` rather than faked.
    #[derive(Debug)]
    struct HeadDeniedStore;

    impl std::fmt::Display for HeadDeniedStore {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "HeadDeniedStore")
        }
    }

    #[async_trait::async_trait]
    impl ObjectStore for HeadDeniedStore {
        async fn put_opts(&self, _location: &ObjectPath, _payload: PutPayload, _opts: object_store::PutOptions) -> object_store::Result<object_store::PutResult> {
            unimplemented!("not exercised by the exists()-only test this store backs")
        }

        async fn put_multipart_opts(&self, _location: &ObjectPath, _opts: object_store::PutMultipartOptions) -> object_store::Result<Box<dyn object_store::MultipartUpload>> {
            unimplemented!("not exercised by the exists()-only test this store backs")
        }

        async fn get_opts(&self, _location: &ObjectPath, _options: object_store::GetOptions) -> object_store::Result<object_store::GetResult> {
            // `head`'s own default impl (`object_store`'s
            // `ObjectStoreExt`) calls `get_opts` with
            // `with_head(true)` — this is the ONE call
            // `R2Tier::exists` makes against this store.
            Err(object_store::Error::Generic { store: "mock", source: "permission denied (HEAD)".into() })
        }

        fn delete_stream(&self, _locations: futures::stream::BoxStream<'static, object_store::Result<ObjectPath>>) -> futures::stream::BoxStream<'static, object_store::Result<ObjectPath>> {
            unimplemented!("not exercised by the exists()-only test this store backs")
        }

        fn list(&self, _prefix: Option<&ObjectPath>) -> futures::stream::BoxStream<'static, object_store::Result<object_store::ObjectMeta>> {
            unimplemented!("not exercised by the exists()-only test this store backs")
        }

        async fn list_with_delimiter(&self, _prefix: Option<&ObjectPath>) -> object_store::Result<object_store::ListResult> {
            unimplemented!("not exercised by the exists()-only test this store backs")
        }

        async fn copy_opts(&self, _from: &ObjectPath, _to: &ObjectPath, _options: object_store::CopyOptions) -> object_store::Result<()> {
            unimplemented!("not exercised by the exists()-only test this store backs")
        }
    }

    #[test]
    fn exists_propagates_non_not_found_head_errors_instead_of_reporting_absent() {
        // design D5 / spec.md "A denied HEAD does not masquerade as a
        // fresh write": only `object_store::Error::NotFound` may
        // collapse to `Ok(false)` — every other HEAD failure (a
        // permission error here) must propagate as
        // `StoreError::ObjectStore` so `Tier::write`'s dedupe probe
        // never re-PUTs an object it couldn't actually confirm was
        // absent.
        let tier = R2Tier::with_object_store(Arc::new(HeadDeniedStore), "canon/").unwrap();
        let path = ObjectPath::from("canon/kind=change/whatever__000000000000.parquet");
        let err = tier.exists(&path).unwrap_err();
        assert!(matches!(err, StoreError::ObjectStore(_)), "expected StoreError::ObjectStore, got {err:?}");
    }

    #[test]
    fn exists_reports_false_only_for_not_found_via_local_backend() {
        // The positive/negative-space companion to the mock-store
        // test above, exercised against the REAL `LocalFileSystem`
        // backend so the `NotFound`-maps-to-`false` half of D5's
        // contract is proven against an actual `ObjectStore` impl,
        // not just documented.
        let dir = tempfile::tempdir().unwrap();
        let tier = R2Tier::local(dir.path(), "canon/").unwrap();
        let missing = ObjectPath::from("canon/kind=change/does-not-exist__000000000000.parquet");
        assert!(!tier.exists(&missing).unwrap());

        let envelope = Envelope::new(1, RecordKind::Change, Utc::now(), actor());
        let change = Change::new(envelope, ChangeId::parse("s2-tiered-storage").unwrap(), "S2", "x", ChangeStatus::Proposed);
        let receipt = tier.write(&change).unwrap();
        let written = ObjectPath::from(receipt.location);
        assert!(tier.exists(&written).unwrap());
    }
}
