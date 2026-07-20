//! The join-spine key newtypes (S1 design D3, tasks 2.1/2.2).
//!
//! Nine join-spine keys tie every canon record kind together:
//! `change_id`, `task_id`, `scenario_id`, `session_id`, `run_id`,
//! `handoff_id`, `sha`/`pr`, `regime_key`, `subject_id`. Each is a
//! distinct Rust
//! (join-spine spec, "Cross-kind join keys share one type per key") — so
//! a wrongly-shaped join is a compile-time type mismatch, not a runtime
//! string-format bug.
//!
//! Every newtype's grammar/joins text is declared exactly once, as the
//! literal arguments to the [`join_key_newtype!`] macro invocation below.
//! That single invocation expands into three artifacts that can never
//! drift relative to each other: the type's own rustdoc `///` comment,
//! its `GRAMMAR`/`JOINS` associated constants, and (via those constants)
//! both the generated `JOIN_SPINE.md` doc (`crate::join_spine_doc`) and
//! every join-key JSON-schema's `description` field (design D3's "no
//! hand-maintained schema drifting from the Rust types", applied to the
//! join-spine doc as well as the record-kind schemas).
//!
//! `RoleId` is not one of the eight join-spine keys (it identifies an
//! *agent role*, used by [`crate::envelope::Actor`] and inside
//! `regime_key`'s `<role>` segment) but follows the same
//! newtype-with-validated-grammar pattern.

use std::borrow::Cow;
use std::convert::TryFrom;
use std::fmt;

use schemars::{JsonSchema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use sha2::Digest;

/// A join-key newtype was constructed from a string that does not match
/// its documented grammar. Construction is the *only* rejection point
/// (join-spine spec: "construction validates the grammar and rejects
/// malformed input") — there is no bypass that stores an unvalidated
/// value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {key}: {value:?} does not match grammar `{grammar}`")]
pub struct JoinKeyError {
    pub key: &'static str,
    pub value: String,
    pub grammar: &'static str,
}

/// Declares one join-key (or role-id) newtype: a validated `String`
/// wrapper with serde `try_from`/`into` round-trip semantics (so a
/// malformed value can never even be *deserialized* into the type, let
/// alone constructed), `Display`, and a hand-written `JsonSchema` impl
/// (join-key newtypes are validated strings, not derive-friendly
/// structs — schemars has nothing useful to derive from a private
/// tuple field).
///
/// The actual grammar check is a separate, hand-written `fn parse` per
/// type below (kept out of the macro so each grammar reads as ordinary,
/// testable Rust rather than a macro-embedded closure).
macro_rules! join_key_newtype {
    ($name:ident, grammar: $grammar:literal, joins: $joins:literal) => {
        #[doc = concat!(
            "Join-spine key `", stringify!($name), "`.\n\n",
            "**Grammar:** ", $grammar, "\n\n",
            "**Joins:** ", $joins,
        )]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// This key's grammar, exactly as documented on the type
            /// (single source for the generated join-spine doc).
            pub const GRAMMAR: &'static str = $grammar;
            /// This key's "joins" relationship, exactly as documented
            /// on the type (single source for the generated join-spine
            /// doc).
            pub const JOINS: &'static str = $joins;

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = JoinKeyError;
            fn try_from(s: String) -> Result<Self, Self::Error> {
                $name::parse(s)
            }
        }

        impl From<$name> for String {
            fn from(v: $name) -> String {
                v.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                Cow::Borrowed(stringify!($name))
            }

            fn json_schema(_generator: &mut SchemaGenerator) -> schemars::Schema {
                json_schema!({
                    "type": "string",
                    "description": concat!($grammar, " — joins: ", $joins),
                })
            }
        }
    };
}

// ── grammar validators (hand-written, one per type; unit-tested below) ──

/// Shared kebab-slug shape check (`[a-z0-9]+(-[a-z0-9]+)*`, no leading/
/// trailing/double hyphen). `pub(crate)` so `records::Subject`'s
/// `domain` field can validate the SAME slug shape at parse without
/// re-implementing (or drifting from) the grammar `ChangeId`/`SubjectId`
/// already enforce.
pub(crate) fn is_kebab_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `[a-z0-9][a-z0-9-]*` — lowercase alnum + hyphen, first character
/// alphanumeric, no underscore (design D6: keeps the `__`
/// natural-key separator unambiguous alongside `Review`'s
/// `{project_id}__{scenario_id}__{pin}` composite key).
fn is_project_id(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `[a-z0-9-]+\.[a-z0-9-]+\.\d{2,}` — ported verbatim from the donor
/// parity harness's scenario-id / id-tag regexes (recommended as a
/// literal port by the donor adoption brief).
fn is_scenario_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    let ok_segment = |seg: &str| {
        !seg.is_empty() && seg.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    ok_segment(parts[0])
        && ok_segment(parts[1])
        && parts[2].len() >= 2
        && parts[2].chars().all(|c| c.is_ascii_digit())
}

/// `<change_id>#<n>` where `<n>` is the (possibly hierarchical,
/// `tasks.md`-style) task number: one or more dot-separated integers
/// (`1`, `1.1`, `6.2`), never renumbered independently of the grammar's
/// single `#` separator.
fn is_task_id(s: &str) -> bool {
    let Some((change, n)) = s.split_once('#') else {
        return false;
    };
    is_kebab_slug(change) && !n.is_empty() && n.split('.').all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_digit()))
}

/// The donor CLI's handoff-id grammar — `YYYYMMDD-HHmm-<topic-slug>-<nonce>`:
/// an 8-digit date, a 4-digit local time, a lowercase-alnum
/// hyphen-joined topic slug (`slugify`'s output, `"handoff"` when
/// empty), and a 4-char lowercase-alnum nonce.
fn is_handoff_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() < 4 {
        return false;
    }
    let all_digits = |s: &str, n: usize| s.len() == n && s.chars().all(|c| c.is_ascii_digit());
    let lower_alnum = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let nonce = parts[parts.len() - 1];
    let slug_tokens = &parts[2..parts.len() - 1];
    all_digits(parts[0], 8)
        && all_digits(parts[1], 4)
        && lower_alnum(nonce)
        && nonce.len() == 4
        && !slug_tokens.is_empty()
        && slug_tokens.iter().all(|t| lower_alnum(t))
}

/// 40 lowercase-hex chars — a full git commit SHA-1, never a
/// non-standard-length prefix (parity-harness audit's adoption note:
/// "reject a non-40-char `app_sha` at parse/schema time rather than the
/// donor's non-blocking advisory").
fn is_sha(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// 64 lowercase-hex chars — a full sha256 digest over a `.feature`
/// spec file's raw bytes (design D4), deliberately a DISTINCT type
/// from [`Sha`]'s 40-hex git commit sha1 — the two grammars must
/// never be interchangeable even though both are "a hex digest".
fn is_spec_digest(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// `<role>/<repo>/<area>/<hash>` — four non-empty, whitespace-free,
/// `/`-free segments, the last a lowercase-hex digest (6-64 chars,
/// matching the range of digest lengths already in use elsewhere in
/// canon's donor corpus, from `parity.py`'s 12-hex `_digest` truncation
/// up to a full 64-hex sha256).
fn is_regime_key(s: &str) -> bool {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 4 {
        return false;
    }
    let plain_segment = |seg: &str| !seg.is_empty() && !seg.chars().any(|c| c.is_whitespace() || c == '/');
    let hex_digest =
        |seg: &str| (6..=64).contains(&seg.len()) && seg.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
    parts[..3].iter().all(|p| plain_segment(p)) && hex_digest(parts[3])
}

/// `session_id` deliberately validates only "non-empty opaque token",
/// never a single fixed shape (e.g. strict UUID parsing). The vendored
/// adapter code proves the derivation *source* is not uniform across
/// agent CLIs — Claude Code and Codex trust the transcript filename
/// stem (`claudecode.rs:370-374`, `codex.rs:232-237`), while omp/pi
/// trust only an in-file session-header `id` field and never parse the
/// filename (`pi.rs:102-127`) — so a `SessionId` newtype that hard-codes
/// "UUID" would reject a correctly-derived omp/pi session id.
/// The vendored launcher project's own adoption brief flags this
/// explicitly; its recommendation is that
/// *which* source each adapter trusts is an S3 (`canon-ingest`)
/// per-adapter decision, not a grammar this type should encode — S1
/// owns only the type's shape-validation (opaque, non-empty, no control
/// characters), not provenance.
fn is_session_id(s: &str) -> bool {
    !s.is_empty() && s.trim() == s && !s.chars().any(|c| c.is_control())
}

join_key_newtype!(
    ChangeId,
    grammar: "openspec change slug: kebab-case (`[a-z0-9]+(-[a-z0-9]+)*`), the directory name under `openspec/changes/`",
    joins: "change ↔ tasks ↔ specs"
);

join_key_newtype!(
    TaskId,
    grammar: "`<change_id>#<n>`, where `<n>` is one or more dot-separated integers (`1`, `1.1`, `6.2`) matching `tasks.md`'s own numbering",
    joins: "task ↔ evidence ↔ trajectory"
);

join_key_newtype!(
    ScenarioId,
    grammar: "`<area>.<surface>.<nn>` — `[a-z0-9-]+\\.[a-z0-9-]+\\.\\d{2,}`; never renumbered once assigned",
    joins: "spec ↔ test ↔ ledger ↔ divergence"
);

join_key_newtype!(
    SessionId,
    grammar: "opaque, non-empty agent-CLI session token; derivation source varies by adapter (Claude Code/Codex: transcript filename stem; omp/pi: in-file header `id` field) — this type validates shape only, never a single fixed format",
    joins: "session ↔ cost ↔ run ↔ trajectory"
);

join_key_newtype!(
    HandoffId,
    grammar: "the donor CLI's handoff-id grammar: `YYYYMMDD-HHmm-<topic-slug>-<nonce>`",
    joins: "handoff ↔ session ↔ change"
);

join_key_newtype!(
    Sha,
    grammar: "git commit SHA-1: exactly 40 lowercase hex characters",
    joins: "reward signals ↔ trajectory"
);

join_key_newtype!(
    RegimeKey,
    grammar: "`<role>/<repo>/<area>/<hash>`, four `/`-separated segments, the last a lowercase-hex digest",
    joins: "strategy write ↔ retrieval (identical at both ends)"
);

join_key_newtype!(
    RoleId,
    grammar: "kebab-case role slug (`[a-z0-9]+(-[a-z0-9]+)*`), e.g. `implementer`, `reviewer`",
    joins: "not a join-spine key; used by `Actor.role` and `regime_key`'s `<role>` segment"
);

join_key_newtype!(
    ProjectId,
    grammar: "`[a-z0-9][a-z0-9-]*` — lowercase alnum + hyphen, first character alphanumeric, no underscore (keeps the `__` natural-key separator unambiguous)",
    joins: "not a join-spine key; composite identity prefix on Scenario/Review/Divergence natural keys (`<project_id>__<scenario_id>…`), optional on EvidenceRecord"
);

join_key_newtype!(
    SpecDigest,
    grammar: "sha256-hex: exactly 64 lowercase hex characters, over a `.feature` file's raw bytes",
    joins: "not a join-spine key; Scenario.source_digest, the inventory-sync freshness signal"
);

join_key_newtype!(
    SubjectId,
    grammar: "subject slug: kebab-case (`[a-z0-9]+(-[a-z0-9]+)*`), the durable product-unit identifier",
    joins: "subject ↔ change ↔ scenario"
);

impl ChangeId {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_kebab_slug(&s) {
            return Err(JoinKeyError { key: "ChangeId", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }
}

impl SubjectId {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_kebab_slug(&s) {
            return Err(JoinKeyError { key: "SubjectId", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }
}

impl TaskId {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_task_id(&s) {
            return Err(JoinKeyError { key: "TaskId", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }

    /// Decompose to the owning `ChangeId` — the substring before `#`,
    /// which `is_task_id` already validated as a well-formed change
    /// slug. No separate `change_id` field is stored anywhere a `Task`
    /// or `EvidenceRecord` carries a `TaskId`; this is the single
    /// source, mirroring `ScenarioId::area()`'s decomposition pattern.
    pub fn change_id(&self) -> ChangeId {
        let (change, _n) = self.0.split_once('#').expect("is_task_id validated a '#' separator");
        ChangeId(change.to_string())
    }
}

impl ScenarioId {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_scenario_id(&s) {
            return Err(JoinKeyError { key: "ScenarioId", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }

    /// `world.firstbuy-hotdeal.26` → `world` — the scenario id's FIRST
    /// dot-segment, i.e. the Hive `area=` partition key. MUST be
    /// recomputed from the id, never trusted from a source directory
    /// (the donor parity harness computes area as `sid.split(".", 1)[0]`;
    /// the donor ledger-reader audit
    /// documents six real cases where a directory and its scenarios'
    /// computed area diverge).
    pub fn area(&self) -> &str {
        self.0.split('.').next().expect("is_scenario_id validated 3 dot-segments")
    }

    /// The middle dot-segment (`world.firstbuy-hotdeal.26` → `firstbuy-hotdeal`).
    pub fn surface(&self) -> &str {
        self.0.split('.').nth(1).expect("is_scenario_id validated 3 dot-segments")
    }

    /// The trailing numeric segment, kept as a string to preserve
    /// leading zeros (`world.place-lock.01` → `"01"`).
    pub fn nn(&self) -> &str {
        self.0.split('.').nth(2).expect("is_scenario_id validated 3 dot-segments")
    }

    /// `world.place-lock.01` → `world-place-lock` — the audit surface
    /// key (`tools/parity.py::_surface_key_of`: `area` + `-` + `surface`,
    /// hyphen-joined, NOT the raw `area.surface` dotted form).
    pub fn surface_key(&self) -> String {
        format!("{}-{}", self.area(), self.surface())
    }
}

impl SessionId {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_session_id(&s) {
            return Err(JoinKeyError { key: "SessionId", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }
}

impl HandoffId {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_handoff_id(&s) {
            return Err(JoinKeyError { key: "HandoffId", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }
}

impl Sha {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_sha(&s) {
            return Err(JoinKeyError { key: "Sha", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }
}

impl RegimeKey {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_regime_key(&s) {
            return Err(JoinKeyError { key: "RegimeKey", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }

    pub fn role(&self) -> &str {
        self.0.split('/').next().expect("is_regime_key validated 4 segments")
    }

    pub fn repo(&self) -> &str {
        self.0.split('/').nth(1).expect("is_regime_key validated 4 segments")
    }

    pub fn area(&self) -> &str {
        self.0.split('/').nth(2).expect("is_regime_key validated 4 segments")
    }

    pub fn hash(&self) -> &str {
        self.0.split('/').nth(3).expect("is_regime_key validated 4 segments")
    }
}

/// Canonicalize one `regime_key` segment (role/repo/area) for
/// [`regime_key`]'s write==read identity guarantee: trimmed,
/// lowercased, and every run of whitespace or `/` collapsed to a
/// single `-`, so two callers deriving the same logical value from
/// differently-cased or -spaced source data (`"Dev"` vs `"dev"`,
/// `"join spine"` vs `"join-spine"`) still produce the IDENTICAL
/// segment. Never applied to `hash` — see [`regime_key`]'s doc comment.
fn canonicalize_regime_segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_was_dash = false;
    for ch in raw.trim().chars() {
        if ch.is_whitespace() || ch == '/' {
            if !last_was_dash && !out.is_empty() {
                out.push('-');
                last_was_dash = true;
            }
        } else {
            out.extend(ch.to_lowercase());
            last_was_dash = false;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// The single canonical `regime_key` serialization (S1 join-spine
/// grammar `<role>/<repo>/<area>/<hash>`; design D5 / S6 design
/// decision 2's "Canonical regime key reuses the donor tuning project's
/// write/read-identity discipline exactly") — S4's verdict emission
/// (write path) and S6/S7/S8's promotion/retrieval queries (read
/// paths) all call this ONE function, never a second per-caller
/// derivation; that is the entire mechanism by which "write the same
/// key you'll read" holds across four different crates. `role` LEADS
/// the tuple (the primary retrieval axis, S6 design decision 2): a
/// lookup scoped to one role is always a prefix scan over this key's
/// first segment, and two verdicts/strategies sharing `role`+`repo`+
/// `area` always share the identical `<role>/<repo>/<area>/` prefix
/// (review-verdict-mapping spec, "two verdicts from the same area and
/// role share a regime key prefix").
///
/// `role`/`repo`/`area` are canonicalized (lowercased, trimmed,
/// whitespace/`/` collapsed to `-`) before joining — the write==read
/// identity guarantee only holds if that normalization runs on EVERY
/// call site, so it lives here, not in each caller. `hash` is passed
/// through only lowercased/trimmed, never re-hashed here: the caller
/// owns its own digest computation (S4's `content_digest` pattern,
/// S6's trajectory digest) — this function's job is the canonical
/// join, not the hashing.
///
/// The returned `String` is not automatically a valid [`RegimeKey`] if
/// any segment is empty or `hash` is not a 6-64-char lowercase hex
/// digest — a caller that needs a validated key still routes the
/// result through [`RegimeKey::parse`] (exactly what this module's
/// unit tests do to lock the canonical format).
pub fn regime_key(role: &str, repo: &str, area: &str, hash: &str) -> String {
    format!(
        "{}/{}/{}/{}",
        canonicalize_regime_segment(role),
        canonicalize_regime_segment(repo),
        canonicalize_regime_segment(area),
        hash.trim().to_lowercase(),
    )
}

impl RoleId {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_kebab_slug(&s) {
            return Err(JoinKeyError { key: "RoleId", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }
}

impl ProjectId {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_project_id(&s) {
            return Err(JoinKeyError { key: "ProjectId", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }
}

impl SpecDigest {
    pub fn parse(s: impl Into<String>) -> Result<Self, JoinKeyError> {
        let s = s.into();
        if !is_spec_digest(&s) {
            return Err(JoinKeyError { key: "SpecDigest", value: s, grammar: Self::GRAMMAR });
        }
        Ok(Self(s))
    }

    /// sha256-hex over raw bytes (design D4: `Scenario.source_digest`
    /// is a full sha256-hex over the `.feature` file's bytes) — the
    /// ONE place that hashing happens, so a `sync` caller never
    /// hand-rolls its own hex-encoding of a `Sha256::digest` output.
    pub fn of(bytes: &[u8]) -> Self {
        let hash = sha2::Sha256::digest(bytes);
        let mut hex = String::with_capacity(64);
        for byte in hash.iter() {
            hex.push_str(&format!("{byte:02x}"));
        }
        Self(hex)
    }
}

/// `run_id`: a ULID (join-spine table: "run ↔ events ↔ manifest").
/// Wraps [`ulid::Ulid`] directly rather than a validated `String` — a
/// ULID's own parser is the grammar check, and its canonical
/// Crockford-base32 `Display`/`FromStr` are already exactly what the
/// join-spine table's grammar cell means by "ULID".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RunId(ulid::Ulid);

impl RunId {
    pub const GRAMMAR: &'static str = "ULID (Crockford base32, 26 chars) — `ulid::Ulid`";
    pub const JOINS: &'static str = "run ↔ events ↔ manifest";

    pub fn new() -> Self {
        Self(ulid::Ulid::new())
    }

    pub fn parse(s: impl AsRef<str>) -> Result<Self, JoinKeyError> {
        ulid::Ulid::from_string(s.as_ref())
            .map(Self)
            .map_err(|_| JoinKeyError { key: "RunId", value: s.as_ref().to_string(), grammar: Self::GRAMMAR })
    }

    pub fn as_ulid(&self) -> ulid::Ulid {
        self.0
    }
}

impl Default for RunId {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<String> for RunId {
    type Error = JoinKeyError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        RunId::parse(s)
    }
}

impl From<RunId> for String {
    fn from(v: RunId) -> String {
        v.0.to_string()
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl JsonSchema for RunId {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("RunId")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> schemars::Schema {
        json_schema!({
            "type": "string",
            "pattern": "^[0-7][0-9A-HJKMNP-TV-Z]{25}$",
            "description": "ULID (Crockford base32, 26 chars) — joins: run ↔ events ↔ manifest",
        })
    }
}

/// `pr`: a GitHub pull-request number. Half of the combined `sha`/`pr`
/// join-spine row (["Joins": "reward signals ↔ trajectory"] on
/// [`Sha`]) — a positive integer, never a bare `u32` threaded through
/// signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct PrNumber(u32);

impl PrNumber {
    pub fn parse(n: u32) -> Result<Self, JoinKeyError> {
        if n == 0 {
            return Err(JoinKeyError { key: "PrNumber", value: n.to_string(), grammar: "positive integer (git/GitHub PR number)" });
        }
        Ok(Self(n))
    }

    pub fn get(&self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for PrNumber {
    type Error = JoinKeyError;
    fn try_from(n: u32) -> Result<Self, Self::Error> {
        PrNumber::parse(n)
    }
}

impl From<PrNumber> for u32 {
    fn from(v: PrNumber) -> u32 {
        v.0
    }
}

impl fmt::Display for PrNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl JsonSchema for PrNumber {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("PrNumber")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> schemars::Schema {
        json_schema!({
            "type": "integer",
            "minimum": 1,
            "description": "GitHub PR number — joins: reward signals ↔ trajectory",
        })
    }
}

/// A monotonic ordinal used to totally order records within a fold
/// group (design D8) — e.g. `Divergence.run_seq`, assigned by
/// `canon-gate::promote`. Wraps `u64` directly: every value is valid
/// (there is no grammar to reject, unlike the `String` join-key
/// newtypes above), but it stays a DISTINCT type from a bare `u64` so
/// a `round` tiebreak (a separate, un-wrapped `u32`) can never be
/// silently compared against it as if it were the same ordering axis
/// (`fold_to_current_state`'s "round is a tiebreak only, never an
/// independent `Ord` axis" rule, design D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TotalOrder(u64);

impl TotalOrder {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for TotalOrder {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<TotalOrder> for u64 {
    fn from(value: TotalOrder) -> u64 {
        value.0
    }
}

impl fmt::Display for TotalOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl JsonSchema for TotalOrder {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("TotalOrder")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> schemars::Schema {
        json_schema!({
            "type": "integer",
            "format": "uint64",
            "minimum": 0,
            "description": "monotonic fold-ordering rank (e.g. Divergence.run_seq) — the SOLE primary sort key within a fold group; never compared against a round tiebreak",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_id_accepts_real_slugs() {
        for slug in ["s1-state-model-join-spine", "s13-policy-expressions", "s6-role-strategy-memory"] {
            assert!(ChangeId::parse(slug).is_ok(), "{slug} should be a valid ChangeId");
        }
    }

    #[test]
    fn change_id_rejects_malformed() {
        for bad in ["", "Has-Caps", "trailing-", "-leading", "double--dash", "has space"] {
            assert!(ChangeId::parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn subject_id_accepts_and_rejects_like_a_kebab_slug() {
        for good in ["subject-domain-loop", "payments", "auth-v2", "a1b2"] {
            assert!(SubjectId::parse(good).is_ok(), "{good} should be a valid SubjectId");
        }
        for bad in ["", "Has-Caps", "trailing-", "-leading", "double--dash", "has space", "under_score"] {
            assert!(SubjectId::parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn task_id_parses_hierarchical_numbers() {
        let id = TaskId::parse("s1-state-model-join-spine#6.2").unwrap();
        assert_eq!(id.change_id().as_str(), "s1-state-model-join-spine");
        assert!(TaskId::parse("s1-state-model-join-spine#1").is_ok());
        assert!(TaskId::parse("no-hash-here").is_err());
        assert!(TaskId::parse("bad slug#1").is_err());
    }

    #[test]
    fn scenario_id_grammar_and_decomposition() {
        let id = ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap();
        assert_eq!(id.area(), "world");
        assert_eq!(id.surface(), "firstbuy-hotdeal");
        assert_eq!(id.nn(), "26");
        assert_eq!(id.surface_key(), "world-firstbuy-hotdeal");
    }

    #[test]
    fn scenario_id_rejects_malformed() {
        for bad in ["world.place-lock.1", "world.place-lock", "World.place-lock.01", "world..01", "world.place-lock.0a"] {
            assert!(ScenarioId::parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn session_id_accepts_uuid_and_non_uuid_shapes() {
        // Claude Code / Codex: filename-stem UUID.
        assert!(SessionId::parse("f47ac10b-58cc-4372-a567-0e02b2c3d479").is_ok());
        // omp/pi: opaque in-file header id, no UUID shape required.
        assert!(SessionId::parse("pi-session-042").is_ok());
        assert!(SessionId::parse("").is_err());
        assert!(SessionId::parse(" leading-space").is_err());
        assert!(SessionId::parse("has\tcontrol").is_err());
    }

    #[test]
    fn handoff_id_matches_drum_grammar() {
        assert!(HandoffId::parse("20260710-1432-fix-the-thing-a1b2").is_ok());
        assert!(HandoffId::parse("20260710-1432-handoff-a1b2").is_ok());
        assert!(HandoffId::parse("not-a-handoff-id").is_err());
        assert!(HandoffId::parse("20260710-1432-fix-the-thing-A1B2").is_err()); // nonce must be lowercase
    }

    #[test]
    fn sha_requires_exactly_40_hex_chars() {
        assert!(Sha::parse("8c81f9e13e9bda0a6a5ee29ba1b6b5137e7bf552").is_ok());
        assert!(Sha::parse("8c81f9e").is_err());
        assert!(Sha::parse("8C81F9E13E9BDA0A6A5EE29BA1B6B5137E7BF552").is_err());
    }

    #[test]
    fn regime_key_grammar() {
        let key = RegimeKey::parse("implementer/canon/join-spine/9c93d024b1a2").unwrap();
        assert_eq!(key.role(), "implementer");
        assert_eq!(key.repo(), "canon");
        assert_eq!(key.area(), "join-spine");
        assert_eq!(key.hash(), "9c93d024b1a2");
        assert!(RegimeKey::parse("only/three/parts").is_err());
        assert!(RegimeKey::parse("a/b/c/NOTHEX").is_err());
    }

    #[test]
    fn regime_key_canonical_format_and_role_leads() {
        let key = regime_key("dev", "canon", "join-spine", "9c93d024b1a2");
        assert_eq!(key, "dev/canon/join-spine/9c93d024b1a2");
        // `role` leads the tuple: the first segment is always the role.
        assert_eq!(key.split('/').next(), Some("dev"));
        // Round-trips through the validated newtype (locks the format
        // against `RegimeKey`'s own grammar, not just string equality).
        let parsed = RegimeKey::parse(&key).unwrap();
        assert_eq!(parsed.role(), "dev");
        assert_eq!(parsed.repo(), "canon");
        assert_eq!(parsed.area(), "join-spine");
        assert_eq!(parsed.hash(), "9c93d024b1a2");
    }

    #[test]
    fn regime_key_write_read_identity_across_casing_and_whitespace() {
        // Two callers deriving the SAME logical (role, repo, area, hash)
        // from differently-cased/-spaced source data must still land on
        // the identical key — the write==read identity guarantee.
        let canonical = regime_key("dev", "canon", "join spine", "9C93D024B1A2");
        let messy = regime_key(" Dev ", "Canon", "Join-Spine", "9c93d024b1a2");
        assert_eq!(canonical, messy);
        assert_eq!(canonical, "dev/canon/join-spine/9c93d024b1a2");
    }

    #[test]
    fn regime_key_role_scoped_prefix() {
        // Two regimes differing ONLY in role never collide...
        let dev_key = regime_key("dev", "canon", "ledger", "abcdef");
        let design_key = regime_key("design", "canon", "ledger", "abcdef");
        assert_ne!(dev_key, design_key);
        // ...but two regimes sharing role+repo+area share the identical
        // `<role>/<repo>/<area>/` prefix regardless of hash (S4
        // review-verdict-mapping spec: "two verdicts from the same area
        // and role share a regime key prefix").
        let dev_key_2 = regime_key("dev", "canon", "ledger", "123456");
        let prefix = "dev/canon/ledger/";
        assert!(dev_key.starts_with(prefix));
        assert!(dev_key_2.starts_with(prefix));
    }

    #[test]
    fn run_id_round_trips_ulid() {
        let id = RunId::new();
        let s: String = id.into();
        assert_eq!(RunId::parse(&s).unwrap(), id);
        assert!(RunId::parse("not-a-ulid").is_err());
    }

    #[test]
    fn pr_number_rejects_zero() {
        assert!(PrNumber::parse(0).is_err());
        assert_eq!(PrNumber::parse(42).unwrap().get(), 42);
    }

    #[test]
    fn deserialize_rejects_malformed_join_key_json() {
        let err = serde_json::from_str::<ChangeId>("\"Not Valid\"").unwrap_err();
        assert!(err.to_string().contains("invalid ChangeId") || err.to_string().contains("does not match grammar"));
    }

    #[test]
    fn project_id_accepts_lowercase_alnum_hyphen() {
        for good in ["root", "app-a", "app-b", "world-app", "a1b2"] {
            assert!(ProjectId::parse(good).is_ok(), "{good} should be a valid ProjectId");
        }
    }

    #[test]
    fn project_id_rejects_underscore_uppercase_empty_leading_hyphen() {
        for bad in ["world_app", "World", "", "-leading", "a_b", "App"] {
            assert!(ProjectId::parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn spec_digest_of_computes_stable_sha256_hex() {
        let digest = SpecDigest::of(b"hello world");
        assert_eq!(digest.as_str(), "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
        assert!(SpecDigest::parse(digest.as_str()).is_ok());
        assert!(SpecDigest::parse("too-short").is_err());
        // Distinct grammar from `Sha`'s 40-hex git sha1 — a well-formed
        // `Sha` value is too short to be a `SpecDigest`.
        assert!(SpecDigest::parse("8c81f9e13e9bda0a6a5ee29ba1b6b5137e7bf552").is_err());
    }

    #[test]
    fn total_order_compares_as_a_plain_integer_and_round_trips() {
        assert!(TotalOrder::new(4) > TotalOrder::new(3));
        assert_eq!(TotalOrder::from(7).get(), 7);
        let json = serde_json::to_value(TotalOrder::new(9)).unwrap();
        assert_eq!(json, serde_json::json!(9));
        assert_eq!(serde_json::from_value::<TotalOrder>(json).unwrap(), TotalOrder::new(9));
    }
}
