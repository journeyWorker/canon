//! `TierPolicy`: `canon.yaml`'s declarative `tiers`/`routing`/`aging`
//! sections (tier-policy spec, S2 design D3) — the ONE place a record
//! kind's physical tier and aging rule are decided; every `Tier::write`/
//! `Tier::read`/`canon tier age` resolves through this, never a
//! hardcoded per-kind branch (tier-policy spec's title requirement).
//!
//! # Rung/backend split (s27 `tier-role-backend-split`, design D1/D3)
//! `canon.yaml`'s `routing`/`aging` sections name a [`Rung`] — a
//! capability role on canon's storage ladder (local diffable files
//! → hot live-queryable state → cold bulk archive), never a vendor
//! product. Which vendor [`Backend`] currently implements a rung is a
//! SEPARATE declaration, `tiers.<rung>.backend`. This is a HARD,
//! non-additive revision of S2's original `TierKind { Git, Pg, R2 }`
//! shape (which conflated the two): `TierPolicy::from_yaml` accepts
//! ONLY the rung/backend shape below — a legacy `git`/`pg`/`r2` value
//! used where a rung is expected fails loud with a rung-vocabulary
//! hint (see [`Rung::parse`]), never a silent alias or a deprecation-
//! warning-then-still-works path (operator directive: canon has never
//! been pushed, so there is no external shape to preserve). See
//! `openspec/changes/s27-tier-role-backend-split/design.md` for the
//! full rationale (D1-D6).
//!
//! Routing/aging map keys are [`canon_model::envelope::RecordKind::as_str`]'s
//! stable snake_case wire strings (`"evidence_record"`, `"strategy_item"`,
//! …) — the same string `#[serde(rename_all = "snake_case")]` already
//! produces on the wire, not a second kebab-case config vocabulary. (The
//! design doc's own `canon.yaml` illustration writes `evidence-record`/
//! `strategy-item`; that is prose sugar, not a locked-in second casing —
//! reusing the one already-tested wire string avoids two conventions
//! that could silently drift apart.)
//!
//! # Backend capability class (s28 `rung-backend-capability`, design D1)
//! A further, ORTHOGONAL split on top of s27's rung/backend one: every
//! [`Backend`] belongs to exactly one [`BackendClass`] (`LocalFile`/
//! `LiveDb`/`ObjectStore`) — the kind of storage medium it physically
//! is, independent of `Backend::read_directly_by_report` (D2 below, a
//! report-specific readability fact, NOT a compatibility fact). Every
//! [`Rung`] declares the ONE `BackendClass` it expects
//! (`Rung::expected_backend_class`); `TierPolicy::from_yaml` now
//! REJECTS a `tiers.<rung>` entry whose configured backend's class
//! does not match — s27 left `local`/`hot`/`cold` accepting ANY
//! backend (e.g. `tiers.local: { backend: postgres }` parsed), which
//! is incoherent: a `local` (diffable-file) rung backed by a live
//! database, or a `hot` (live-queryable) rung backed by git, makes no
//! sense as a "swap the vendor, keep the role" story. This is a
//! SEPARATE axis from `Backend::read_directly_by_report` — both
//! happen to single out git today, but one is a compatibility gate at
//! parse time (D1) and the other is `canon-report`'s own inclusion
//! signal (D2); they must never be collapsed into one method.
//!
//! # Correcting the report-inclusion signal (s28 design D2)
//! s27's `Backend::offline_file_readable()` returned `true` for `S3`
//! — WRONG: `canon report`'s marts read ONLY `canon-report`'s own
//! local roots (the git ledger, a local `canon/r2` parquet directory,
//! and `canon/learn` parquet), never a live S3 bucket. `canon tier age`
//! writes cold/S3 records to the LIVE bucket, not to local
//! `canon/r2` — so an S3-routed kind's data is NOT read directly by
//! the report; it appears only if a local `canon/r2` mirror is
//! separately materialized, which canon has no automatic sync for
//! today. Renamed to `Backend::read_directly_by_report()`
//! (git → `true`; postgres/S3 → `false`) to say exactly this, no
//! more: see `crates/canon-report/src/tier_boundary.rs`'s module doc
//! for the full report-side consequence (D3).
//!
//! # A second `LiveDb` backend (s32 `sqlite-hot-backend`)
//! s28 design D1 explicitly reserved "a second live-database vendor
//! for `hot`" — `Backend::Sqlite` is it: `.class() ==
//! BackendClass::LiveDb`, exactly like `Backend::Postgres`, so it is
//! accepted anywhere the s28 class check accepts a `LiveDb` backend
//! with NO new class-check logic. Configured by `tiers.<rung>: {
//! backend: sqlite, path: <p> }` — `path` is REQUIRED (a missing one
//! fails loud naming the field) and carries no env-var indirection (a
//! local db file has no secret to keep out of `canon.yaml`), unlike
//! `postgres.dsn_env`/`s3.bucket_env`. [`TierPolicy::from_yaml_at`]
//! resolves a relative `path` against the caller-supplied canon.yaml
//! directory, so [`BackendConfig::Sqlite`]'s `path` is ALREADY
//! absolute by the time any caller (`crate::sqlite_tier::SqliteTier::connect`)
//! sees it — unlike `GitTierConfig::root`, which stays relative and
//! is resolved later, at the CLI layer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use canon_model::envelope::RecordKind;
use serde::Deserialize;

/// The capability RUNG a record kind routes/ages to — canon's storage
/// ladder's role (S2 design §4's own diagram, made literal): local
/// diffable files → hot live-queryable state → cold bulk archive.
/// Independent of which vendor [`Backend`] currently implements it
/// (s27 design D1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rung {
    /// Local, diffable files — git-native history. Default backend: git.
    Local,
    /// Hot, live-queryable state.
    Hot,
    /// Cold, bulk object-store archive.
    Cold,
}

impl Rung {
    pub fn as_str(self) -> &'static str {
        match self {
            Rung::Local => "local",
            Rung::Hot => "hot",
            Rung::Cold => "cold",
        }
    }

    /// Loud, hint-carrying rejection of a legacy backend name used
    /// where a rung is expected (s27 design D3) — `git`/`pg`/`r2` are
    /// BACKEND names now, never a valid `routing`/`aging`/`tiers` key.
    pub fn parse(s: &str) -> Result<Self, PolicyError> {
        match s {
            "local" => Ok(Rung::Local),
            "hot" => Ok(Rung::Hot),
            "cold" => Ok(Rung::Cold),
            "git" | "pg" | "r2" => Err(PolicyError(format!(
                "`{s}` is a BACKEND name, not a rung — canon.yaml's `routing`/`aging`/`tiers` keys now name a capability rung (local/hot/cold); declare the backend separately via `tiers.<rung>.backend: {s}`"
            ))),
            other => Err(PolicyError(format!("unknown rung `{other}` (expected one of local/hot/cold)"))),
        }
    }

    /// The [`BackendClass`] `tiers.<rung>`'s configured backend MUST
    /// belong to (s28 design D1) — `TierPolicy::from_yaml` rejects any
    /// other class with a loud, hint-carrying `PolicyError`. Today this
    /// pins `local`→git, `hot`→postgres, `cold`→s3 (the SAME default
    /// pairing s27 already documented as "today's convention"), but
    /// now as an ENFORCED compatibility constraint, not just a
    /// convention comment — any future same-class backend (e.g. a
    /// second live-database vendor for `hot`) remains swappable, an
    /// incompatible one (e.g. `hot` backed by git) does not parse.
    pub fn expected_backend_class(self) -> BackendClass {
        match self {
            Rung::Local => BackendClass::LocalFile,
            Rung::Hot => BackendClass::LiveDb,
            Rung::Cold => BackendClass::ObjectStore,
        }
    }
}

/// The I/O CAPABILITY CLASS a [`Backend`] belongs to (s28
/// `rung-backend-capability` design D1) — orthogonal to
/// `Backend::read_directly_by_report` (D2): this is a COMPATIBILITY
/// classification `TierPolicy::from_yaml` validates a rung's
/// configured backend against (`Rung::expected_backend_class`), never
/// a report-readability fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendClass {
    /// A local, diffable file store — no live connection, no bucket.
    LocalFile,
    /// A live, queryable database connection.
    LiveDb,
    /// A live object-store bucket.
    ObjectStore,
}

impl BackendClass {
    /// The canonical backend(s) this class names in a validation hint
    /// (e.g. "a local-file backend (`git`)") — s32 `sqlite-hot-backend`
    /// gave `LiveDb` a SECOND same-class backend (postgres, sqlite),
    /// so this returns every example for a class, already
    /// slash-joined and backtick-quoted, never silently picking just
    /// one as if it were the only option.
    fn example_backends_hint(self) -> &'static str {
        match self {
            BackendClass::LocalFile => "`git`",
            BackendClass::LiveDb => "`postgres`/`sqlite`",
            BackendClass::ObjectStore => "`s3`",
        }
    }

    /// The human-readable phrase `TierPolicy::from_yaml`'s class-
    /// mismatch `PolicyError` names this class by.
    fn describe(self) -> &'static str {
        match self {
            BackendClass::LocalFile => "a local-file backend",
            BackendClass::LiveDb => "a live-database backend",
            BackendClass::ObjectStore => "an object-store backend",
        }
    }
}

/// The vendor BACKEND currently implementing a rung (s27 design D1/D4)
/// — the identity `TierKind` used to conflate with the rung itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Backend {
    Git,
    Postgres,
    S3,
    /// s32 `sqlite-hot-backend`: a local, file-backed live database —
    /// the SAME `BackendClass::LiveDb` capability class as `Postgres`
    /// (design D1: "a second live-database vendor for `hot`"), so it
    /// is accepted anywhere the s28 class check accepts a `LiveDb`
    /// backend, no new class-check logic required.
    Sqlite,
}

impl Backend {
    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Git => "git",
            Backend::Postgres => "postgres",
            Backend::S3 => "s3",
            Backend::Sqlite => "sqlite",
        }
    }

    pub fn parse(s: &str) -> Result<Self, PolicyError> {
        match s {
            "git" => Ok(Backend::Git),
            "postgres" => Ok(Backend::Postgres),
            "s3" => Ok(Backend::S3),
            "sqlite" => Ok(Backend::Sqlite),
            other => Err(PolicyError(format!("unknown backend `{other}` (expected one of git/postgres/s3/sqlite)"))),
        }
    }

    /// The [`BackendClass`] this backend belongs to (s28 design D1) —
    /// the ONE method `TierPolicy::from_yaml`'s rung/backend
    /// compatibility validation reads.
    pub fn class(self) -> BackendClass {
        match self {
            Backend::Git => BackendClass::LocalFile,
            Backend::Postgres => BackendClass::LiveDb,
            Backend::S3 => BackendClass::ObjectStore,
            Backend::Sqlite => BackendClass::LiveDb,
        }
    }

    /// s28 design D2: does `canon report` read this backend's OWN
    /// store DIRECTLY (DuckDB `read_text`/`read_parquet` over
    /// `canon-report`'s local roots, no live connection)? Git's
    /// Hive-laid-out JSON ledger IS one of those local roots — `true`.
    /// Postgres (a live server) and S3 (a live bucket) are each NOT:
    /// their own store is never opened by `canon-report`: a Postgres-
    /// backed rung's data lives exclusively behind a live DSN, and an
    /// S3-backed rung's data is written to the LIVE bucket by `canon
    /// tier age` (`crates/canon-store/src/r2_tier.rs`), never to the
    /// local `canon/r2` parquet directory the report's `stg_r2_records`
    /// view scans — that local directory is a SEPARATE, non-automatic
    /// mirror an operator may or may not keep in sync. Distinct from
    /// `Backend::class` (D1: a parse-time COMPATIBILITY classification)
    /// even though both currently single out git — this is
    /// `canon-report`'s own report-INCLUSION signal, the ONE method
    /// every backend-conditioned report-visibility decision in the
    /// codebase reads; no second ad hoc "is this backend readable"
    /// check exists anywhere else.
    pub fn read_directly_by_report(self) -> bool {
        match self {
            Backend::Git => true,
            Backend::Postgres | Backend::S3 | Backend::Sqlite => false,
        }
    }

    /// The short, backend-generic reason [`crate::tier::StoreError::TierUnavailable`]
    /// reports when a rung tagged with this backend is configured but
    /// not (yet) attached — a live-DSN/bucket detail is filled in by
    /// whichever call site actually resolved the failure; this is the
    /// FALLBACK a caller with no more specific reason at hand uses.
    pub fn default_unattached_reason(self) -> &'static str {
        match self {
            Backend::Git => "not configured",
            Backend::Postgres => "no live DSN",
            Backend::S3 => "no live bucket",
            Backend::Sqlite => "no db file",
        }
    }
}

/// `canon.yaml`'s `tiers.<rung>` block body, tagged on its own
/// `backend:` key (s27 design D1) — the per-backend field sets
/// (`git`: `root`; `postgres`: `dsn_env`/`schema`; `s3`: `bucket_env`/
/// `prefix`; `sqlite`: `path`, s32 `sqlite-hot-backend`) are
/// byte-for-byte the pre-s27 `GitTierConfigRaw`/`PgTierConfigRaw`/
/// `R2TierConfigRaw` field sets (sqlite's own added fresh by s32),
/// only relocated under this internally-tagged enum.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "backend", rename_all = "snake_case")]
enum BackendConfigRaw {
    Git {
        root: PathBuf,
    },
    Postgres {
        dsn_env: String,
        #[serde(default = "default_pg_schema")]
        schema: String,
    },
    S3 {
        bucket_env: String,
        #[serde(default = "default_r2_prefix")]
        prefix: String,
    },
    /// s32 `sqlite-hot-backend`: `path` is REQUIRED (no default) — a
    /// `tiers.<rung>` sqlite block missing it fails loud via serde's
    /// own "missing field `path`" error, wrapped by
    /// `decode_backend_config`'s `canon.yaml's tiers.{rung}: {e}`
    /// prefix (spec: "fail loud at parse with a hint naming the
    /// field").
    Sqlite {
        path: PathBuf,
    },
}

fn default_pg_schema() -> String {
    "canon_v1".to_string()
}

fn default_r2_prefix() -> String {
    "canon/".to_string()
}

#[derive(Debug, Clone, Deserialize)]
struct AgingRuleRaw {
    after: String,
    to: String,
}

/// `canon.yaml`'s top-level shape, narrowed to the keys S2 owns
/// (`tiers`/`routing`/`aging`) — every other top-level key (S1's
/// `handoff_templates:`, a later spec's own section) is ignored by
/// `#[serde(default)]` + no `deny_unknown_fields`, mirroring
/// `canon_model::handoff::HandoffTemplatesManifest`'s exact "S1 owns
/// only this narrow slice" convention. `tiers` is kept as raw
/// [`serde_yaml::Value`]s (s27 design D1), NOT decoded straight into
/// [`BackendConfigRaw`], so [`TierPolicy::from_yaml`] can validate the
/// MAP KEY (a rung name, via [`Rung::parse`]) FIRST — this is what
/// lets a legacy `tiers.git`/`tiers.pg`/`tiers.r2` top-level key
/// produce the SAME loud, hint-carrying error a legacy `routing`/
/// `aging` value produces, rather than an opaque "missing field
/// `backend`" serde error from deserializing straight into the tagged
/// enum.
#[derive(Debug, Clone, Default, Deserialize)]
struct TierPolicyRaw {
    #[serde(default)]
    tiers: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    routing: HashMap<String, String>,
    #[serde(default)]
    aging: HashMap<String, AgingRuleRaw>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("canon.yaml TierPolicy: {0}")]
pub struct PolicyError(pub String);

#[derive(Debug, Clone)]
pub struct GitTierConfig {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PgTierConfig {
    pub dsn_env: String,
    pub schema: String,
}

#[derive(Debug, Clone)]
pub struct R2TierConfig {
    pub bucket_env: String,
    pub prefix: String,
}

/// s32 `sqlite-hot-backend`: `path` is resolved to an ALREADY-
/// ABSOLUTE `PathBuf` by [`TierPolicy::from_yaml_at`] (relative paths
/// joined against the canon.yaml directory) — unlike
/// [`GitTierConfig::root`], which stays exactly as `canon.yaml` wrote
/// it and is resolved later by the CLI (`canon-cli::tiers::
/// project_dir`). `SqliteTier::connect` (`crate::sqlite_tier`)
/// therefore never needs to know the canon.yaml location itself.
#[derive(Debug, Clone)]
pub struct SqliteTierConfig {
    pub path: PathBuf,
}

/// A rung's resolved `tiers.<rung>` block — its `backend:` tag plus
/// that backend's own config payload (s27 design D1). The `enum` is
/// structurally free (any variant), but [`TierPolicy::from_yaml`]
/// REJECTS a rung/backend pairing whose classes disagree (s28
/// `rung-backend-capability` D1): `local` requires a
/// [`BackendClass::LocalFile`] backend (git), `hot` a
/// [`BackendClass::LiveDb`] (postgres), `cold` a
/// [`BackendClass::ObjectStore`] (s3) — the `backend:` tag stays
/// explicit so a future same-class backend can be swapped in, but a
/// cross-class pairing (e.g. `local` → postgres) is a loud config
/// error, not a valid-but-unusual state.
#[derive(Debug, Clone)]
pub enum BackendConfig {
    Git(GitTierConfig),
    Postgres(PgTierConfig),
    S3(R2TierConfig),
    Sqlite(SqliteTierConfig),
}

impl BackendConfig {
    pub fn backend(&self) -> Backend {
        match self {
            BackendConfig::Git(_) => Backend::Git,
            BackendConfig::Postgres(_) => Backend::Postgres,
            BackendConfig::S3(_) => Backend::S3,
            BackendConfig::Sqlite(_) => Backend::Sqlite,
        }
    }
}

impl From<BackendConfigRaw> for BackendConfig {
    fn from(raw: BackendConfigRaw) -> Self {
        match raw {
            BackendConfigRaw::Git { root } => BackendConfig::Git(GitTierConfig { root }),
            BackendConfigRaw::Postgres { dsn_env, schema } => BackendConfig::Postgres(PgTierConfig { dsn_env, schema }),
            BackendConfigRaw::S3 { bucket_env, prefix } => BackendConfig::S3(R2TierConfig { bucket_env, prefix }),
            BackendConfigRaw::Sqlite { path } => BackendConfig::Sqlite(SqliteTierConfig { path }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgingRuleConfig {
    pub after: chrono::Duration,
    pub to: Rung,
}

/// A parsed, validated `TierPolicy` — `canon.yaml`'s `tiers`/`routing`/
/// `aging` sections resolved into typed values (durations parsed,
/// rung/kind names validated), ready for [`crate::registry::TierRegistry::new`].
#[derive(Debug, Clone)]
pub struct TierPolicy {
    pub tiers: HashMap<Rung, BackendConfig>,
    pub routing: HashMap<RecordKind, Rung>,
    pub aging: HashMap<RecordKind, AgingRuleConfig>,
}

/// `"30d"` / `"7d"` (D3's own example values) → a `chrono::Duration` —
/// `d`(days)/`h`(hours)/`m`(minutes)/`s`(seconds) suffix, one integer
/// magnitude, hand-written (matching this codebase's existing
/// hand-written-grammar-validator style, `canon_model::ids`) rather than
/// pulling in a duration-parsing crate for a one-suffix-character need.
///
/// s29 `store-hardening` design D3: a negative magnitude (`-1d`, a
/// future-dated aging cutoff) is rejected before construction, and
/// the `chrono::Duration::try_*` constructors replace the panicking
/// `Duration::{days,hours,minutes,seconds}` — an out-of-range magnitude
/// (e.g. `i64::MAX` days) becomes a `PolicyError` naming the literal
/// instead of a panic.
fn parse_duration_shorthand(s: &str) -> Result<chrono::Duration, PolicyError> {
    let trimmed = s.trim();
    let (digits, unit) = trimmed.split_at(trimmed.len().saturating_sub(1));
    let magnitude: i64 = digits
        .parse()
        .map_err(|_| PolicyError(format!("aging `after: {s}` is not `<integer><d|h|m|s>` (e.g. `30d`)")))?;
    if magnitude < 0 {
        return Err(PolicyError(format!("aging `after: {s}` is negative — a duration magnitude must be non-negative")));
    }
    let out_of_range = || PolicyError(format!("aging `after: {s}` is out of range for a `chrono::Duration`"));
    match unit {
        "d" => chrono::Duration::try_days(magnitude).ok_or_else(out_of_range),
        "h" => chrono::Duration::try_hours(magnitude).ok_or_else(out_of_range),
        "m" => chrono::Duration::try_minutes(magnitude).ok_or_else(out_of_range),
        "s" => chrono::Duration::try_seconds(magnitude).ok_or_else(out_of_range),
        other => Err(PolicyError(format!("aging `after: {s}` has unknown unit `{other}` (expected d/h/m/s)"))),
    }
}

fn parse_kind(s: &str) -> Result<RecordKind, PolicyError> {
    RecordKind::ALL
        .into_iter()
        .find(|k| k.as_str() == s)
        .ok_or_else(|| PolicyError(format!("routing/aging key `{s}` is not one of canon-model's twelve record kinds")))
}

/// Decode one `tiers.<rung_key>` raw YAML value into a [`BackendConfig`]
/// (s27 design D1/D3 mechanism 3): a block with no `backend:` key at
/// all (the pre-s27 `{ root: ... }`/`{ dsn_env: ..., schema: ... }`
/// bodies had none) fails with an explicit, friendly `PolicyError`
/// naming the required key — never a raw, unmodified `serde_yaml`
/// "missing field `backend`" message. `canon_yaml_dir` is used ONLY
/// to resolve a decoded [`BackendConfig::Sqlite`]'s `path` to an
/// already-absolute form (s32 design: unlike `GitTierConfig::root`,
/// which stays relative and is resolved later by the CLI) — every
/// other backend variant ignores it.
fn decode_backend_config(rung_key: &str, value: serde_yaml::Value, canon_yaml_dir: &Path) -> Result<BackendConfig, PolicyError> {
    let has_backend_tag = value.as_mapping().is_some_and(|m| m.contains_key(serde_yaml::Value::String("backend".to_string())));
    if !has_backend_tag {
        return Err(PolicyError(format!(
            "canon.yaml's `tiers.{rung_key}` block is missing its required `backend: git|postgres|s3|sqlite` key"
        )));
    }
    let raw: BackendConfigRaw = serde_yaml::from_value(value).map_err(|e| PolicyError(format!("canon.yaml's `tiers.{rung_key}`: {e}")))?;
    let mut cfg: BackendConfig = raw.into();
    if let BackendConfig::Sqlite(sqlite) = &mut cfg {
        sqlite.path = resolve_sqlite_path(&sqlite.path, canon_yaml_dir);
    }
    Ok(cfg)
}

/// [`SqliteTierConfig::path`] resolution (s32 design): absolute as
/// given, else joined onto `canon_yaml_dir`, else (when even THAT
/// join stays relative — `canon_yaml_dir` itself was relative, e.g.
/// [`TierPolicy::from_yaml`]'s own `.` default) joined onto the
/// process's current directory. Never touches the filesystem (no
/// `canonicalize`): the db file legitimately may not exist yet at
/// parse time (a fresh `canon init` repo's `canon/hot.db`), so this
/// is a pure path-algebra join, not an existence-requiring resolve.
fn resolve_sqlite_path(path: &Path, canon_yaml_dir: &Path) -> PathBuf {
    let joined = if path.is_absolute() { path.to_path_buf() } else { canon_yaml_dir.join(path) };
    if joined.is_absolute() {
        joined
    } else {
        std::env::current_dir().map(|cwd| cwd.join(&joined)).unwrap_or(joined)
    }
}

impl TierPolicy {
    /// Parse `canon_yaml`, resolving any [`BackendConfig::Sqlite`]
    /// path against the process's current directory (s32 design: the
    /// convenience default for a caller — most of this crate's own
    /// tests, `report.rs`, `plans.rs` — that never touches a sqlite
    /// path and doesn't care). A caller that DOES know `canon.yaml`'s
    /// own directory (every real CLI invocation) should call
    /// [`Self::from_yaml_at`] instead, so a relative `tiers.<rung>.
    /// path:` resolves against the repo, not an arbitrary CWD. See
    /// [`Self::from_yaml_at`] for the full fail-loud contract this
    /// delegates to.
    pub fn from_yaml(canon_yaml: &str) -> Result<Self, PolicyError> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::from_yaml_at(canon_yaml, &cwd)
    }

    /// [`Self::from_yaml`], but resolving a [`BackendConfig::Sqlite`]
    /// path against `canon_yaml_dir` (s32 `sqlite-hot-backend` design:
    /// "relative paths resolve against the canon.yaml directory") —
    /// the SAME directory `canon-cli::tiers::project_dir` already
    /// computes for `GitTierConfig::root`'s own CLI-side resolution,
    /// threaded in here instead so a sqlite path is ALREADY absolute
    /// by the time [`BackendConfig::Sqlite`] reaches any caller —
    /// unlike `root`, which stays relative until the CLI joins it.
    /// Every other fail-loud rule is unchanged: an unknown
    /// `routing`/`aging` kind name, an unknown `tiers.*`/
    /// `aging.*.to`/`routing.*` rung name (including a legacy
    /// `git`/`pg`/`r2` backend name used in any of those three
    /// positions, s27 design D3), a `tiers.<rung>` block missing its
    /// `backend:` tag, a `tiers.<rung>` block whose backend's
    /// [`BackendClass`] does not match `Rung::expected_backend_class`
    /// (s28 design D1 — e.g. `tiers.local: { backend: postgres }`),
    /// an `aging.<kind>` entry with no matching `routing.<kind>`
    /// entry, an `aging.<kind>.to` rung that is not strictly colder
    /// than `routing.<kind>` under the total order `local < hot <
    /// cold` (s29 `store-hardening` design D2 — same-rung and
    /// backward aging both reject; a `cold`-routed kind can carry no
    /// aging rule at all, since `cold` has no colder rung), or a
    /// malformed/negative/out-of-range `aging.*.after` duration (s29
    /// design D3).
    pub fn from_yaml_at(canon_yaml: &str, canon_yaml_dir: &Path) -> Result<Self, PolicyError> {
        let raw: TierPolicyRaw = serde_yaml::from_str(canon_yaml).map_err(|e| PolicyError(e.to_string()))?;

        let mut tiers = HashMap::new();
        for (rung_key, value) in raw.tiers {
            let rung = Rung::parse(&rung_key)?;
            let cfg = decode_backend_config(&rung_key, value, canon_yaml_dir)?;
            let actual = cfg.backend();
            let expected_class = rung.expected_backend_class();
            if actual.class() != expected_class {
                return Err(PolicyError(format!(
                    "canon.yaml `tiers.{rung_key}`: backend `{}` is {}, but the `{rung_key}` rung expects {} ({})",
                    actual.as_str(),
                    actual.class().describe(),
                    expected_class.describe(),
                    expected_class.example_backends_hint(),
                )));
            }
            tiers.insert(rung, cfg);
        }

        let mut routing = HashMap::new();
        for (kind_str, rung_str) in &raw.routing {
            routing.insert(parse_kind(kind_str)?, Rung::parse(rung_str)?);
        }

        let mut aging = HashMap::new();
        for (kind_str, rule) in &raw.aging {
            let kind = parse_kind(kind_str)?;
            let to = Rung::parse(&rule.to)?;
            let routed = routing.get(&kind).copied().ok_or_else(|| {
                PolicyError(format!(
                    "canon.yaml `aging.{kind_str}` has no matching `routing.{kind_str}` entry — an aging rule for an unrouted kind is meaningless"
                ))
            })?;
            if to <= routed {
                return Err(PolicyError(format!(
                    "canon.yaml `aging.{kind_str}.to: {}` is not strictly colder than `routing.{kind_str}: {}` — aging must move forward under the rule `local < hot < cold`",
                    to.as_str(),
                    routed.as_str(),
                )));
            }
            aging.insert(kind, AgingRuleConfig { after: parse_duration_shorthand(&rule.after)?, to });
        }

        Ok(Self { tiers, routing, aging })
    }

    /// The rung `kind` resolves to for a FRESH write — `Err` (never a
    /// silent default) when `canon.yaml` has no `routing` entry for it
    /// (tier-policy spec's title requirement, checked here so `Tier`
    /// adapters never see an unrouted kind).
    pub fn tier_for(&self, kind: RecordKind) -> Result<Rung, crate::tier::StoreError> {
        self.routing.get(&kind).copied().ok_or(crate::tier::StoreError::UnroutedKind { kind })
    }

    /// The `local` rung's git config, when its backend is `git`
    /// (today's convention — mirrors
    /// [`crate::registry::TierRegistry::git`]'s identical convenience
    /// accessor for the live-adapter equivalent). `None` when
    /// `local` is unconfigured or backed by something other than
    /// git — a best-effort convenience for a caller (e.g. `canon
    /// gate`'s default ledger-root resolution, `canon report`'s
    /// default git-root resolution) that only cares about the
    /// conventional git-backed local rung, never a substitute for
    /// [`Self::tier_for`]'s own loud, routing-driven resolution.
    pub fn local_git(&self) -> Option<&GitTierConfig> {
        match self.tiers.get(&Rung::Local) {
            Some(BackendConfig::Git(cfg)) => Some(cfg),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
handoff_templates:
  - 기획
tiers:
  local: { backend: git, root: canon/ledger }
  hot:       { backend: postgres, dsn_env: CANON_PG_DSN, schema: canon_v1 }
  cold:      { backend: s3, bucket_env: CANON_R2_BUCKET, prefix: "canon/" }
routing:
  evidence_record: local
  strategy_item: local
  handoff: hot
  session: hot
  event: hot
  trajectory: cold
aging:
  handoff: { after: 30d, to: cold }
  event:   { after: 7d,  to: cold }
"#;

    #[test]
    fn parses_routing_and_aging_and_ignores_other_top_level_keys() {
        let policy = TierPolicy::from_yaml(SAMPLE).unwrap();
        assert_eq!(policy.tier_for(RecordKind::Handoff).unwrap(), Rung::Hot);
        assert_eq!(policy.tier_for(RecordKind::Trajectory).unwrap(), Rung::Cold);
        let handoff_aging = policy.aging.get(&RecordKind::Handoff).unwrap();
        assert_eq!(handoff_aging.after, chrono::Duration::days(30));
        assert_eq!(handoff_aging.to, Rung::Cold);
        assert_eq!(policy.tiers.get(&Rung::Hot).unwrap().backend(), Backend::Postgres);
    }

    #[test]
    fn routing_change_requires_no_code_change_it_is_just_a_different_string() {
        // s29 design D2: `handoff` carries an aging rule to `cold`, so
        // moving its routing to `cold` would now be a same-rung aging
        // rejection (a SEPARATE, correctly-tested case) — `session`
        // has no aging rule and exercises the same "just a string"
        // routing-change property without colliding with D2.
        let moved = SAMPLE.replace("session: hot", "session: cold");
        let policy = TierPolicy::from_yaml(&moved).unwrap();
        assert_eq!(policy.tier_for(RecordKind::Session).unwrap(), Rung::Cold);
    }

    #[test]
    fn unrouted_kind_is_a_loud_error_not_a_silent_default() {
        let policy = TierPolicy::from_yaml(SAMPLE).unwrap();
        let err = policy.tier_for(RecordKind::Change).unwrap_err();
        assert!(matches!(err, crate::tier::StoreError::UnroutedKind { kind: RecordKind::Change }));
    }

    #[test]
    fn unknown_kind_name_in_routing_fails_loud() {
        let bad = SAMPLE.replace("evidence_record: local", "not-a-kind: local");
        let err = TierPolicy::from_yaml(&bad).unwrap_err();
        assert!(err.0.contains("not-a-kind"));
    }

    #[test]
    fn malformed_duration_fails_loud() {
        let bad = SAMPLE.replace("after: 30d", "after: thirty-days");
        let err = TierPolicy::from_yaml(&bad).unwrap_err();
        assert!(err.0.contains("thirty-days"));
    }

    /// s29 `store-hardening` spec scenario "Same-rung aging is
    /// rejected instead of deleting records": `routing.task: hot`
    /// with `aging.task: { after: 30d, to: hot }` must reject — an
    /// aging rule whose destination equals its source rung would let
    /// `canon tier age` delete the only copy of the record (dedupe-
    /// then-delete against the record's own current tier).
    #[test]
    fn same_rung_aging_is_rejected_instead_of_deleting_records() {
        let yaml = r#"
routing:
  task: hot
aging:
  task: { after: 30d, to: hot }
"#;
        let err = TierPolicy::from_yaml(yaml).unwrap_err();
        assert!(err.0.contains("task"), "{err}");
        assert!(err.0.contains("hot"), "{err}");
        assert!(err.0.contains("local < hot < cold"), "{err}");
    }

    /// s29 `store-hardening` spec scenario "Backward aging is
    /// rejected instead of silently dead": `routing.scenario: cold`
    /// with `aging.scenario: { after: 30d, to: hot }` must reject with
    /// the same `PolicyError` class — a `cold → hot` rule can never
    /// fire (`canon tier age` only moves records forward), so it
    /// would silently do nothing forever.
    #[test]
    fn backward_aging_is_rejected_instead_of_silently_dead() {
        let yaml = r#"
routing:
  scenario: cold
aging:
  scenario: { after: 30d, to: hot }
"#;
        let err = TierPolicy::from_yaml(yaml).unwrap_err();
        assert!(err.0.contains("scenario"), "{err}");
        assert!(err.0.contains("cold"), "{err}");
        assert!(err.0.contains("hot"), "{err}");
        assert!(err.0.contains("local < hot < cold"), "{err}");
    }

    /// s29 design D2: an `aging.<kind>` entry for a kind with no
    /// matching `routing.<kind>` entry is loud, not silently ignored
    /// — an aging rule for an unrouted kind was already meaningless
    /// (there is no "current tier" to age FROM).
    #[test]
    fn unrouted_kind_aging_rejects() {
        let yaml = r#"
routing:
  task: hot
aging:
  event: { after: 7d, to: cold }
"#;
        let err = TierPolicy::from_yaml(yaml).unwrap_err();
        assert!(err.0.contains("event"), "{err}");
        assert!(err.0.contains("routing.event"), "{err}");
    }

    /// s29 design D2 counter-case: a valid forward `hot → cold` aging
    /// rule still parses — D2 rejects same-rung/backward/unrouted, not
    /// every aging rule.
    #[test]
    fn valid_hot_to_cold_aging_still_parses() {
        let yaml = r#"
routing:
  event: hot
aging:
  event: { after: 7d, to: cold }
"#;
        let policy = TierPolicy::from_yaml(yaml).unwrap();
        assert_eq!(policy.aging.get(&RecordKind::Event).unwrap().to, Rung::Cold);
    }

    /// s29 design D2 regression guard: this repo's own committed
    /// `canon.yaml` (read from the repo root, not a fixture) must
    /// still parse under the new forward-only aging validation — its
    /// `handoff`/`event` aging rules are both `hot → cold`.
    #[test]
    fn committed_canon_yaml_still_parses_under_forward_only_aging() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../canon.yaml");
        let yaml = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        TierPolicy::from_yaml(&yaml).unwrap_or_else(|e| panic!("repo canon.yaml must parse: {e}"));
    }

    /// s29 `store-hardening` spec scenario "Negative and overflow
    /// durations are policy errors": `-1d` rejects before
    /// construction.
    #[test]
    fn negative_duration_magnitude_rejects() {
        let bad = SAMPLE.replace("after: 30d", "after: -1d");
        let err = TierPolicy::from_yaml(&bad).unwrap_err();
        assert!(err.0.contains("-1d"), "{err}");
        assert!(err.0.contains("negative"), "{err}");
    }

    /// s29 spec scenario: an out-of-range magnitude (`i64::MAX` days,
    /// which overflows `chrono::Duration`'s internal millisecond
    /// representation) maps to a `PolicyError` naming the literal
    /// instead of panicking via the old `Duration::days` path.
    #[test]
    fn overflowing_duration_magnitude_rejects_without_panicking() {
        let bad = SAMPLE.replace("after: 30d", "after: 9223372036854775807d");
        let err = TierPolicy::from_yaml(&bad).unwrap_err();
        assert!(err.0.contains("9223372036854775807d"), "{err}");
    }

    /// s29 design D3 counter-case: a valid, in-range duration (`30d`,
    /// D3's own worked example) still parses via the `try_days` path.
    #[test]
    fn valid_thirty_day_duration_still_parses() {
        let duration = parse_duration_shorthand("30d").unwrap();
        assert_eq!(duration, chrono::Duration::days(30));
    }

    /// s28 `rung-backend-capability` spec scenario: a rung's
    /// configured backend must belong to that rung's expected
    /// `BackendClass` — s27 left `local`/`hot`/`cold` accepting ANY
    /// backend (this exact `cold: { backend: postgres }` yaml used to
    /// parse successfully, s27's own `any_rung_may_be_tagged_with_any_backend`
    /// scenario, D2 design); s28 now rejects the class mismatch loud.
    #[test]
    fn class_mismatched_backend_fails_loud_with_a_hint() {
        let yaml = r#"
tiers:
  cold: { backend: postgres, dsn_env: CANON_PG_DSN_COLD, schema: canon_cold }
routing:
  trajectory: cold
"#;
        let err = TierPolicy::from_yaml(yaml).unwrap_err();
        assert!(err.0.contains("tiers.cold"), "{err}");
        assert!(err.0.contains("backend `postgres`"), "{err}");
        assert!(err.0.contains("live-database backend"), "{err}");
        assert!(err.0.contains("the `cold` rung expects"), "{err}");
        assert!(err.0.contains("object-store backend"), "{err}");
        assert!(err.0.contains("`s3`"), "{err}");
    }

    /// s28 spec scenario: every class-correct rung/backend pairing
    /// parses — local/git (`LocalFile`), hot/postgres (`LiveDb`),
    /// cold/s3 (`ObjectStore`).
    #[test]
    fn each_class_correct_rung_backend_pairing_parses() {
        for (rung_yaml, expected_backend) in [
            ("local: { backend: git, root: canon/ledger }", Backend::Git),
            ("hot: { backend: postgres, dsn_env: CANON_PG_DSN_CLASS_OK, schema: canon_v1 }", Backend::Postgres),
            ("cold: { backend: s3, bucket_env: CANON_R2_BUCKET_CLASS_OK, prefix: \"canon/\" }", Backend::S3),
        ] {
            let rung_key = rung_yaml.split(':').next().unwrap();
            let yaml = format!("tiers:\n  {rung_yaml}\nrouting:\n  change: {rung_key}\n");
            let policy = TierPolicy::from_yaml(&yaml).unwrap_or_else(|e| panic!("{rung_key} yaml must parse: {e}"));
            let rung = Rung::parse(rung_key).unwrap();
            assert_eq!(policy.tiers.get(&rung).unwrap().backend(), expected_backend);
        }
    }

    /// s28 design D1 counter-case (migrated from a would-be
    /// `cold: { backend: postgres }` / `hot: { backend: s3 }`
    /// production-config fixture — no longer constructible via
    /// `TierPolicy::from_yaml` once D1's class validation lands):
    /// `Backend::class()` per backend.
    #[test]
    fn backend_class_matches_the_designed_pairing() {
        assert_eq!(Backend::Git.class(), BackendClass::LocalFile);
        assert_eq!(Backend::Postgres.class(), BackendClass::LiveDb);
        assert_eq!(Backend::S3.class(), BackendClass::ObjectStore);
    }

    /// s28 design D2 counter-case: `Backend::read_directly_by_report()`
    /// is `true` ONLY for git — S3 flips from s27's (wrong) `true` to
    /// `false`, matching the corrected report-inclusion signal.
    #[test]
    fn read_directly_by_report_is_true_only_for_git() {
        assert!(Backend::Git.read_directly_by_report());
        assert!(!Backend::Postgres.read_directly_by_report());
        assert!(!Backend::S3.read_directly_by_report());
    }

    /// s27 spec scenario: a legacy `pg` routing value fails loud with
    /// a rung-vocabulary hint.
    #[test]
    fn legacy_pg_routing_value_fails_loud_with_a_hint() {
        let bad = SAMPLE.replace("handoff: hot", "handoff: pg");
        let err = TierPolicy::from_yaml(&bad).unwrap_err();
        assert!(err.0.contains("pg"), "{err}");
        assert!(err.0.contains("BACKEND name, not a rung"), "{err}");
        assert!(err.0.contains("local/hot/cold"), "{err}");
    }

    /// s27 spec scenario: a legacy `r2` aging destination fails loud
    /// with the same class of hint.
    #[test]
    fn legacy_r2_aging_destination_fails_loud_with_a_hint() {
        let bad = SAMPLE.replace("to: cold", "to: r2");
        let err = TierPolicy::from_yaml(&bad).unwrap_err();
        assert!(err.0.contains("r2"), "{err}");
        assert!(err.0.contains("BACKEND name, not a rung"), "{err}");
    }

    /// s27 spec scenario: a legacy `git`-named top-level `tiers` key
    /// fails loud with the identical hint class, not a separately-
    /// worded serde error.
    #[test]
    fn legacy_git_named_top_level_tiers_key_fails_loud_with_the_same_hint() {
        let bad = r#"
tiers:
  git: { backend: git, root: canon/ledger }
routing:
  change: local
"#;
        let err = TierPolicy::from_yaml(bad).unwrap_err();
        assert!(err.0.contains("git"), "{err}");
        assert!(err.0.contains("BACKEND name, not a rung"), "{err}");
    }

    /// s27 spec scenario: a rung block missing its `backend:` tag
    /// fails loud naming the required key explicitly.
    #[test]
    fn rung_block_missing_backend_tag_fails_loud_naming_the_required_key() {
        let bad = r#"
tiers:
  hot: { dsn_env: CANON_PG_DSN, schema: canon_v1 }
routing:
  task: hot
"#;
        let err = TierPolicy::from_yaml(bad).unwrap_err();
        assert!(err.0.contains("backend: git|postgres|s3|sqlite"), "{err}");
        assert!(err.0.contains("tiers.hot"), "{err}");
    }

    /// s32 `sqlite-hot-backend` spec scenario "Hot rung on sqlite
    /// passes the class check": `hot: { backend: sqlite, path: ... }`
    /// parses cleanly (s28's class check accepts it for free, since
    /// `Backend::Sqlite.class() == BackendClass::LiveDb` — no new
    /// class-check logic).
    #[test]
    fn hot_rung_on_sqlite_parses_and_resolves_to_a_sqlite_backend() {
        let yaml = r#"
tiers:
  hot: { backend: sqlite, path: canon/hot.db }
routing:
  session: hot
"#;
        let policy = TierPolicy::from_yaml(yaml).unwrap_or_else(|e| panic!("hot/sqlite yaml must parse: {e}"));
        assert_eq!(policy.tiers.get(&Rung::Hot).unwrap().backend(), Backend::Sqlite);
        assert_eq!(policy.tier_for(RecordKind::Session).unwrap(), Rung::Hot);
    }

    /// s32 spec scenario "Sqlite on a file-class rung still fails":
    /// `local: { backend: sqlite, ... }` fails with the SAME s28
    /// class-mismatch error a `local: { backend: postgres }` would
    /// produce — sqlite is `LiveDb`, `local` expects `LocalFile`.
    #[test]
    fn sqlite_on_the_local_rung_fails_the_class_check() {
        let yaml = r#"
tiers:
  local: { backend: sqlite, path: x.db }
routing:
  change: local
"#;
        let err = TierPolicy::from_yaml(yaml).unwrap_err();
        assert!(err.0.contains("tiers.local"), "{err}");
        assert!(err.0.contains("backend `sqlite`"), "{err}");
        assert!(err.0.contains("live-database backend"), "{err}");
        assert!(err.0.contains("the `local` rung expects"), "{err}");
        assert!(err.0.contains("local-file backend"), "{err}");
        assert!(err.0.contains("`git`"), "{err}");
    }

    /// s32 spec: a `tiers.<rung>` sqlite block missing `path:` fails
    /// loud at parse, naming the field — never a raw unmodified serde
    /// error the operator has to decode themselves.
    #[test]
    fn sqlite_missing_path_fails_loud_naming_the_field() {
        let yaml = r#"
tiers:
  hot: { backend: sqlite }
routing:
  session: hot
"#;
        let err = TierPolicy::from_yaml(yaml).unwrap_err();
        assert!(err.0.contains("path"), "{err}");
        assert!(err.0.contains("tiers.hot"), "{err}");
    }

    /// s32 design: `BackendConfig::Sqlite.path` is ALREADY absolute
    /// once parsed — a relative `path:` resolves against the caller-
    /// supplied canon.yaml directory (`from_yaml_at`), never left for
    /// a later caller to join, unlike `GitTierConfig::root`.
    #[test]
    fn from_yaml_at_resolves_a_relative_sqlite_path_against_the_given_directory() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
tiers:
  hot: { backend: sqlite, path: canon/hot.db }
routing:
  session: hot
"#;
        let policy = TierPolicy::from_yaml_at(yaml, dir.path()).unwrap();
        let BackendConfig::Sqlite(cfg) = policy.tiers.get(&Rung::Hot).unwrap() else { panic!("expected a sqlite config") };
        assert!(cfg.path.is_absolute(), "path must already be absolute: {}", cfg.path.display());
        assert_eq!(cfg.path, dir.path().join("canon/hot.db"));
    }

    /// s32 design counter-case: an ALREADY-absolute `path:` passes
    /// through unchanged — `from_yaml_at` never re-roots a path that
    /// was never relative to begin with.
    #[test]
    fn from_yaml_at_leaves_an_already_absolute_sqlite_path_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let absolute = dir.path().join("elsewhere/hot.db");
        let yaml = format!("tiers:\n  hot: {{ backend: sqlite, path: \"{}\" }}\nrouting:\n  session: hot\n", absolute.display());
        let other_dir = tempfile::tempdir().unwrap();
        let policy = TierPolicy::from_yaml_at(&yaml, other_dir.path()).unwrap();
        let BackendConfig::Sqlite(cfg) = policy.tiers.get(&Rung::Hot).unwrap() else { panic!("expected a sqlite config") };
        assert_eq!(cfg.path, absolute);
    }
}
