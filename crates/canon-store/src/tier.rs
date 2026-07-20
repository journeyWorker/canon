//! The one storage trait every adapter conforms to (S2 design D1, task
//! 1.1) — `GitTier`, `PgTier`, `R2Tier` each implement [`Tier`]; a
//! caller (`canon-ingest`/S3, `canon-gate`/S5, `canon-learn`/S6) writes
//! and reads through this trait and [`TierRegistry`](crate::registry::TierRegistry),
//! never a tier-specific method reached from outside `canon-store`
//! (tier-adapter-trait spec, "One storage trait, three conforming
//! adapters").

use std::fmt;

use canon_model::envelope::{CanonRecord, RecordKind};
use canon_model::evidence::{EvidenceViolation, RawRecord};
use chrono::{DateTime, Duration, Utc};

/// Object-safe view over anything a [`Tier`] can write (design D1's
/// `&dyn StoredRecord`) — deliberately NOT `canon_model::CanonRecord`
/// itself, since `Serialize`/`Deserialize` are not `dyn`-safe. Every
/// `T: CanonRecord` gets this via the blanket impl below (the typed,
/// ingest-time write path); [`RawWrite`] wraps an untyped
/// [`RawRecord`] for the same trait (the path [`Tier::age`] uses to
/// move already-serialized content across tiers without
/// reconstructing a concrete Rust type) — a
/// distinct newtype, not a second `impl … for RawRecord`, because
/// Rust's coherence rules reject two potentially-overlapping
/// blanket/concrete impls of the same trait even though `RawRecord`
/// never implements `CanonRecord`.
pub trait StoredRecord {
    /// This record's kind — resolves `TierPolicy.routing`/`.aging`
    /// (never a caller-side literal-kind branch, tier-policy spec).
    fn kind(&self) -> RecordKind;
    /// This record's envelope `at` — the field every tier orders reads
    /// by and aging thresholds against.
    fn at(&self) -> DateTime<Utc>;
    /// The canonical JSON body a tier actually persists — envelope
    /// fields flattened alongside the record's own fields, exactly as
    /// `serde_json::to_value` produces for any `CanonRecord` (S1
    /// interface note: "ordinary `serde_json::to_value`/`from_value`,
    /// no custom serializer").
    fn to_raw(&self) -> RawRecord;
}

impl<T: CanonRecord> StoredRecord for T {
    fn kind(&self) -> RecordKind {
        T::KIND
    }

    fn at(&self) -> DateTime<Utc> {
        self.envelope().at
    }

    fn to_raw(&self) -> RawRecord {
        RawRecord(serde_json::to_value(self).expect("CanonRecord always serializes (S1 invariant)"))
    }
}

/// Parse the `at` field out of a raw record's JSON — shared by
/// [`RawWrite::at`] and any caller (e.g. `R2Tier::read`'s `since`
/// filter) that only has a bare [`RawRecord`], not a [`StoredRecord`]
/// (coherence forbids `impl StoredRecord for RawRecord` directly — see
/// [`RawWrite`]'s doc comment).
pub fn raw_record_at(raw: &RawRecord) -> DateTime<Utc> {
    raw.0
        .get("at")
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .expect("raw record reaching raw_record_at already passed validate_envelope_shape")
}

/// Untyped, already-serialized content ready for `Tier::write` — the
/// aging/migration write path (module doc: coherence forces this to be
/// a distinct newtype rather than a second `impl StoredRecord for
/// RawRecord`).
pub struct RawWrite(pub RawRecord);

impl StoredRecord for RawWrite {
    fn kind(&self) -> RecordKind {
        self.0
             .0
            .get("kind")
            .and_then(|v| v.as_str())
            .and_then(|s| RecordKind::ALL.into_iter().find(|k| k.as_str() == s))
            .expect("RawWrite reaching StoredRecord::kind already passed validate_envelope_shape")
    }

    fn at(&self) -> DateTime<Utc> {
        raw_record_at(&self.0)
    }

    fn to_raw(&self) -> RawRecord {
        self.0.clone()
    }
}

/// The outcome of one [`Tier::write`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteReceipt {
    pub kind: RecordKind,
    /// The tier-relative location the record was (or already was)
    /// stored at — a git-tier path, a pg-tier `(kind, id, digest)`
    /// composite key rendered as text (s21: `records_history` is
    /// append-only, so a version's own digest is part of its
    /// identity, exactly like a git-tier digest-suffixed path), or an
    /// r2-tier object key.
    pub location: String,
    /// The content digest (task 3.1's "digest-based idempotence"; see
    /// [`crate::partition::content_digest`]).
    pub digest: String,
    /// `true` when this write found the identical digest already
    /// present at the resolved location and performed no new write —
    /// the mechanism `Tier::age`'s re-run idempotence (tier-policy
    /// spec, tier-adapter-trait spec) and `R2Tier`'s duplicate-content
    /// aging writes rely on. `GitTier::write` never sets this: a
    /// git-tier duplicate-path write is a hard error instead (an
    /// operator/caller bug at fresh-authoring time, not a routine
    /// re-run — git-tier-layout-enforcement spec, "never silently
    /// overwriting").
    pub deduped: bool,
}

/// One `Tier::read` query: a kind (every tier resolution is per-kind,
/// tier-policy spec) and an optional `since` lower bound on `at`
/// (unified-query spec, "A tier-scoped query filters correctly").
#[derive(Debug, Clone)]
pub struct TierQuery {
    pub kind: RecordKind,
    pub since: Option<DateTime<Utc>>,
}

impl TierQuery {
    pub fn kind(kind: RecordKind) -> Self {
        Self { kind, since: None }
    }

    pub fn since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    /// Whether `at` satisfies this query's `since` bound (or there is
    /// none) — the one predicate every tier's `read` and the unified
    /// merge (`crate::registry`) both apply, so "at or after `since`"
    /// can never drift between call sites.
    pub fn matches(&self, at: DateTime<Utc>) -> bool {
        self.since.is_none_or(|since| at >= since)
    }
}

/// `Tier::read`'s result: every record that validated (content AND, for
/// `GitTier`, layout), plus every violation encountered along the way —
/// the soft-skip-reader / fail-loud-twin duality collapsed into ONE
/// validator with two consumption modes (parity-harness audit §3.2):
/// a caller that only wants records ignores `violations`; `canon gate`
/// (S5) reports them.
#[derive(Debug, Clone, Default)]
pub struct TierReadResult {
    pub records: Vec<RawRecord>,
    pub violations: Vec<EvidenceViolation>,
}

/// One `TierPolicy.aging` entry, resolved to a live destination handle
/// (design D3): move `kind` records older than `after` from the tier
/// `Tier::age` is called on to `destination`.
pub struct AgingRule {
    pub kind: RecordKind,
    pub after: Duration,
    pub destination: std::sync::Arc<dyn Tier>,
}

impl fmt::Debug for AgingRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgingRule").field("kind", &self.kind).field("after", &self.after).finish_non_exhaustive()
    }
}

/// `Tier::age`'s report (tier-policy spec: "a record past its aging
/// threshold moves tiers"; tier-adapter-trait spec: "aging is
/// idempotent under a duplicate run").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgeReport {
    pub kind: RecordKind,
    /// Records newly written to the destination tier this run.
    pub moved: usize,
    /// Records whose destination write was a digest-dedup no-op (an
    /// already-aged record re-selected by a prior interrupted run) —
    /// still removed from the source tier this run, but NOT counted in
    /// `moved` ("reports zero newly-aged records for that entry").
    pub already_aged: usize,
}

/// Every failure mode a `Tier` adapter can report — one shared enum so
/// `canon query`/`canon tier age`/`canon gate` (S5) match on a single
/// error type across all three adapters, rather than three
/// tier-specific error enums a caller would need to know about.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("layout violation: {0}")]
    Layout(#[from] EvidenceViolation),
    #[error("{kind:?}: write to an already-occupied path {location:?} was rejected (append-only; corrections are new records, `canon migrate` is the sole exception)")]
    DuplicatePath { kind: RecordKind, location: String },
    #[error("{kind:?}: no `TierPolicy.routing` entry (canon.yaml) — every write/read must resolve through the declarative policy, never a hardcoded default")]
    UnroutedKind { kind: RecordKind },
    #[error("{}", tier_unavailable_message(*rung, *backend, reason))]
    TierUnavailable { rung: crate::policy::Rung, backend: Option<crate::policy::Backend>, reason: String },
    #[error("{} backend is not attached ({reason})", backend.as_str())]
    BackendUnattached { backend: crate::policy::Backend, reason: String },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("policy: {0}")]
    Policy(String),
    #[error("sql: {0}")]
    Sql(String),
    #[error("object store: {0}")]
    ObjectStore(String),
    #[error("parquet/arrow: {0}")]
    Parquet(String),
}

/// The exact prose [`StoreError::TierUnavailable`]'s `Display` renders
/// (s27 design D5): `"{rung} tier ({backend}) is not attached
/// ({reason})"` when the rung's backend is known, `"{rung} tier is
/// not configured ({reason})"` when the rung has no `tiers.<rung>`
/// entry at all (`backend: None`) — the ONE place both shapes are
/// assembled, so `canon query`'s failure text and this crate's own
/// unit tests never drift apart.
fn tier_unavailable_message(rung: crate::policy::Rung, backend: Option<crate::policy::Backend>, reason: &str) -> String {
    match backend {
        Some(b) => format!("{} tier ({}) is not attached ({reason})", rung.as_str(), b.as_str()),
        None => format!("{} tier is not configured ({reason})", rung.as_str()),
    }
}

impl StoreError {
    /// Construct a [`StoreError::TierUnavailable`] — the one
    /// constructor every rung-aware caller (`TierRegistry::handle`,
    /// `canon-cli`'s tier builders) uses, so the field shape never
    /// drifts between call sites.
    pub fn tier_unavailable(rung: crate::policy::Rung, backend: Option<crate::policy::Backend>, reason: impl Into<String>) -> Self {
        StoreError::TierUnavailable { rung, backend, reason: reason.into() }
    }
}

/// One storage trait, three conforming adapters (tier-adapter-trait
/// spec's title requirement, design D1). `Send + Sync` so a resolved
/// tier can be shared behind an `Arc` as an [`AgingRule::destination`]
/// or across a multi-threaded caller.
pub trait Tier: Send + Sync {
    /// Which vendor backend this adapter implements (s27 design D4;
    /// for error messages / the unified-query fan-out's dedup-by-tier
    /// bookkeeping) — adapters are BACKEND implementations a rung's
    /// `tiers.<rung>.backend` config selects, never a rung's own
    /// identity.
    fn backend(&self) -> crate::policy::Backend;

    /// Persist `record`. Every adapter round-trips a written record
    /// (tier-adapter-trait spec): `read`ing it back by identity right
    /// after a successful `write` returns an equal record.
    fn write(&self, record: &dyn StoredRecord) -> Result<WriteReceipt, StoreError>;

    /// Persist every record in `records`, in `records`' order (s31
    /// design D2) — the provided default loops [`Self::write`], so
    /// `GitTier`/`R2Tier` need no override. `PgTier` overrides this
    /// with an actual chunked multi-row statement (see
    /// `crate::pg_tier::PgTier`'s `Tier` impl); every override's
    /// receipts MUST stay semantically identical to what this default
    /// loop would have produced (same `deduped`/`location`/`digest`
    /// per record, same order) — the s31 "batch == loop" equivalence
    /// tests hold every override to that bar. This is NOT itself a
    /// transaction: the first failing write short-circuits with its
    /// error, and every receipt already returned for `records`'
    /// prefix is already durably persisted (a `GitTier` override
    /// would inherit that non-transactional shape too, since none is
    /// defined here — only `PgTier` currently overrides, and its own
    /// per-CHUNK transaction is documented on that impl).
    fn write_batch(&self, records: &[&dyn StoredRecord]) -> Result<Vec<WriteReceipt>, StoreError> {
        records.iter().map(|r| self.write(*r)).collect()
    }

    /// Read every record of `query.kind` satisfying `query.since`,
    /// reporting (never panicking on) anything malformed or misfiled
    /// along the way.
    fn read(&self, query: &TierQuery) -> Result<TierReadResult, StoreError>;

    /// Move every `rule.kind` record older than `rule.after` (by `at`)
    /// from this tier to `rule.destination`, deleting the source copy
    /// only after the destination write confirms (tier-policy spec).
    fn age(&self, rule: &AgingRule) -> Result<AgeReport, StoreError>;
}
