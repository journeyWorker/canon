//! `canon-learn`'s own row ids — `TrajectoryId`/`StrategyId`, both
//! ULIDs, mirroring `canon-model::ids::RunId`'s exact pattern (a ULID's
//! own parser is the grammar check; its canonical Crockford-base32
//! `Display`/`FromStr` is already what "join key" means here). These
//! are NOT join-spine keys (the join spine's own eight keys live in
//! `canon-model`) — they identify a row within THIS crate's two
//! stores, referenced by `StrategyItem::source_trajectory_ids`
//! provenance.
//!
//! # Why a `StrategyId` is DERIVED, never minted
//! A [`TrajectoryId`] names a raw capture that is written once and
//! never deleted, so a freshly-minted ULID identifies it forever.
//! A [`StrategyId`] names a DERIVED row: [`crate::rebuild::
//! rebuild_namespace`] deletes every strategy item for a regime and
//! re-distills it from the retained raw trajectories on every `canon
//! ingest artifacts` that touches that regime. While these ids were
//! minted with `StrategyId::new()`, that routine rebuild silently
//! re-keyed the whole distilled layer, which broke three things at
//! once:
//!
//! 1. `mart_flywheel_funnel`'s `retrieved`/`applied` stages
//!    (`crates/canon-store/sql/views.sql`, s40 `plan-vs-actual-diff`)
//!    inner-join a recorded `Run.injected_guidance` `StrategyRef`
//!    against the CURRENT items, so every historical retrieval
//!    evaporated at the next rebuild and both stages fell to zero.
//! 2. `canon learn promote <ULID>` targeted a value that changed under
//!    the operator's feet — the promoted `.canon/strategies/<role>/
//!    <id>.md` file's `id` stopped resolving in the parquet store, so
//!    a later `canon learn demote` could no longer find its subject.
//! 3. `StrategyItem::source_trajectory_ids` provenance pointed FORWARD
//!    from a re-keyed row, but nothing pointed back: no consumer could
//!    tell that the new id and the old id named the same distillation.
//!
//! [`StrategyId::derive`] fixes the identity rather than any one of
//! those joins: the id is a pure function of the distilled row's own
//! content ([`StrategyIdentity`]), so a rebuild that re-derives the
//! same strategy reproduces the same id and every recorded reference
//! keeps resolving. It is also VERIFIABLE — a holder of a persisted
//! parquet row or a promoted markdown file can recompute the id from
//! the fields the file itself carries (see
//! [`crate::strategy::StrategyItem::identity`]).
//!
//! # Migration: every existing `StrategyId` changes value, once
//! No compatibility shim ships with this, and none is wanted — canon
//! has no external consumers, so a one-time re-keying is cheaper than
//! a permanent alias table. What an operator with an existing tree
//! should expect:
//!
//! - `.canon/learn/strategies/**/<old-ULID>.parquet` — self-healing.
//!   The next `canon ingest artifacts` touching that regime runs
//!   [`crate::rebuild::rebuild_namespace`], which deletes the whole
//!   regime's items and rewrites them under derived ids. A regime that
//!   is never re-ingested keeps its old minted ids and stays
//!   internally consistent; the two schemes never mix WITHIN a regime,
//!   because the delete is regime-wide and atomic with the rewrite.
//! - `.canon/strategies/<role>/<old-ULID>.md` (the git tier) — NOT
//!   self-healing, and the one thing that needs a human. Promotion
//!   names the file after the id, so after the owning regime is
//!   rebuilt the old file is orphaned: `canon learn promote
//!   <old-ULID>` and `canon learn demote <old-ULID>` both fail
//!   `UnknownStrategyId`, and the still-valid strategy now lives under
//!   a new id. Remedy: delete the stale `<old-ULID>.md` files and
//!   re-run `canon learn promote` against the current ids (`canon
//!   retrieve --role <r> --json` lists them). Nothing is lost — the
//!   raw trajectories these were distilled from are never deleted.
//! - `Run.injected_guidance` snapshots recorded BEFORE the cutover
//!   still cite minted ids, so they stop resolving once their regime
//!   is rebuilt, exactly as they already did on every rebuild. The
//!   funnel's `retrieved`/`applied` therefore describe post-cutover
//!   retrievals; from the cutover forward they no longer decay.

use std::fmt;

use canon_model::ids::{RegimeKey, RoleId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ulid::Ulid;

use crate::error::LearnError;

macro_rules! ulid_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(Ulid);

        impl $name {
            /// A fresh, time-sortable id (ULIDs embed a millisecond
            /// timestamp — two ids minted in the same call sort in
            /// generation order, a useful property for a raw/append
            /// tier even before `recorded_at` is consulted).
            pub fn new() -> Self {
                Self(Ulid::new())
            }

            pub fn parse(s: &str) -> Result<Self, LearnError> {
                Ulid::from_string(s).map(Self).map_err(|e| LearnError::InvalidId { value: s.to_string(), reason: e.to_string() })
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = LearnError;
            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::parse(&s)
            }
        }

        impl From<$name> for String {
            fn from(v: $name) -> String {
                v.0.to_string()
            }
        }
    };
}

ulid_id!(TrajectoryId, "Identifies one raw [`crate::trajectory::Trajectory`] row.");

ulid_id!(
    StrategyId,
    "Identifies one distilled [`crate::strategy::StrategyItem`] row. \
     Production distillation NEVER calls `StrategyId::new()` — it calls \
     [`StrategyId::derive`], so a rebuild reproduces the id (module \
     doc). `new()` survives for fixtures that only need *an* id and \
     never round-trip it through a rebuild."
);

/// The content-identity of a distilled [`crate::strategy::StrategyItem`]
/// — every field that answers "WHICH strategy is this", and nothing a
/// re-derivation could perturb. A named borrow struct rather than a
/// seven-slot positional argument list: [`StrategyId::derive`] and
/// [`crate::strategy::StrategyItem::from_identity`] must agree on the
/// hash preimage exactly, and a positional tuple of five `&str`s is a
/// transposition waiting to happen.
///
/// Deliberately EXCLUDES [`crate::strategy::StrategyItem::demotion`]:
/// demotion is a later soft-flag written in place by
/// [`crate::store::StrategyStore::mark_demoted`], so folding it into
/// the identity would re-key a row at demotion time and reintroduce
/// exactly the dangling-reference failure this derivation exists to
/// prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategyIdentity<'a> {
    pub regime_key: &'a RegimeKey,
    pub role: &'a RoleId,
    pub title: &'a str,
    pub description: &'a str,
    pub content: &'a str,
    pub source_trajectory_ids: &'a [TrajectoryId],
    /// The item's own `recorded_at`. Feeds the ULID's 48-bit time
    /// component (below), so it MUST itself be re-derivable — which is
    /// why [`crate::distill::distill_trajectory`] stamps the SOURCE
    /// trajectory's `recorded_at` rather than reading the wall clock.
    pub recorded_at: DateTime<Utc>,
}

/// Domain-separation tag: the first absorbed field, so a `StrategyId`
/// preimage can never collide with some other sha256 preimage in this
/// workspace that happens to concatenate the same bytes.
const STRATEGY_ID_DOMAIN: &[u8] = b"canon-learn/strategy-id/v1";

/// Absorbs one field length-prefixed (8-byte big-endian length, then
/// the bytes). `title`/`description`/`content` are free text lifted
/// straight out of an artifact, so a plain separator byte is NOT safe:
/// a title ending in the separator and a description starting after it
/// would hash identically to the un-split pair. Length-prefixing makes
/// the preimage unambiguously parseable, so distinct field tuples have
/// distinct preimages by construction and the only remaining collision
/// risk is sha256's own.
fn absorb(hasher: &mut Sha256, field: &[u8]) {
    hasher.update((field.len() as u64).to_be_bytes());
    hasher.update(field);
}

impl StrategyId {
    /// The deterministic id for `identity` — a pure function, no clock,
    /// no randomness, no ambient state (module doc's three broken
    /// joins).
    ///
    /// Construction mirrors `canon-ingest::normalize::
    /// deterministic_run_id` verbatim, the workspace's existing
    /// deterministic-ULID precedent: `Ulid::from_parts(<the item's own
    /// `recorded_at` in epoch millis>, <the low 128 bits of a sha256
    /// over the length-prefixed identity>)`. Keeping the ULID grammar
    /// is load-bearing — [`StrategyId::parse`], the `<id>.parquet`
    /// store filename, the `.canon/strategies/<role>/<id>.md` promoted
    /// filename and `canon learn promote`'s clap `value_parser` all
    /// stay byte-compatible; only the VALUES change.
    ///
    /// Collision argument: `Ulid::from_parts` keeps 80 bits of the
    /// supplied random component, and two ids collide only when they
    /// share both the same millisecond and the same 80 hash bits. Over
    /// a corpus of `n` distilled items the birthday bound is
    /// `n^2 / 2^81`, i.e. below `10^-12` at a million items — and that
    /// is the UNCONDITIONAL bound, ignoring the millisecond partition
    /// that must also match. Two items with a byte-identical identity
    /// SHARE an id on purpose: they are the same distillation, and
    /// [`crate::distill::distill_trajectory`] collapses them rather
    /// than storing one row twice under one filename.
    pub fn derive(identity: &StrategyIdentity<'_>) -> Self {
        let mut hasher = Sha256::new();
        absorb(&mut hasher, STRATEGY_ID_DOMAIN);
        absorb(&mut hasher, identity.regime_key.as_str().as_bytes());
        absorb(&mut hasher, identity.role.as_str().as_bytes());
        absorb(&mut hasher, identity.title.as_bytes());
        absorb(&mut hasher, identity.description.as_bytes());
        absorb(&mut hasher, identity.content.as_bytes());
        absorb(&mut hasher, &(identity.source_trajectory_ids.len() as u64).to_be_bytes());
        for source in identity.source_trajectory_ids {
            absorb(&mut hasher, source.to_string().as_bytes());
        }
        let digest = hasher.finalize();
        let random = u128::from_be_bytes(digest[0..16].try_into().expect("a sha256 digest is 32 bytes, so its first 16 always convert"));
        // Negative epoch millis (a pre-1970 `recorded_at`, only
        // reachable from a hand-built fixture) clamp to 0 rather than
        // wrapping into a nonsense u64 — the same `.max(0)` guard
        // `deterministic_run_id` applies.
        let millis = identity.recorded_at.timestamp_millis().max(0) as u64;
        Self(Ulid::from_parts(millis, random))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trajectory_id_display_parse_round_trips() {
        let id = TrajectoryId::new();
        let s = id.to_string();
        assert_eq!(TrajectoryId::parse(&s).unwrap(), id);
    }

    #[test]
    fn strategy_id_serde_round_trips() {
        let id = StrategyId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: StrategyId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn two_freshly_minted_ids_are_distinct() {
        assert_ne!(TrajectoryId::new(), TrajectoryId::new());
    }

    #[test]
    fn malformed_id_is_rejected() {
        assert!(TrajectoryId::parse("not-a-ulid").is_err());
    }

    /// Fixture identity fields, owned so each test can perturb exactly
    /// one of them and observe the id move.
    struct Fields {
        regime_key: RegimeKey,
        role: RoleId,
        title: String,
        description: String,
        content: String,
        source_trajectory_ids: Vec<TrajectoryId>,
        recorded_at: DateTime<Utc>,
    }

    impl Fields {
        fn new() -> Self {
            Self {
                regime_key: RegimeKey::parse(canon_model::ids::regime_key("dev", "repo", "auth", "abc123")).unwrap(),
                role: RoleId::parse("dev").unwrap(),
                title: "batch the writes".to_string(),
                description: "Validated strategy distilled from trajectory X.".to_string(),
                content: "ctx text".to_string(),
                source_trajectory_ids: vec![TrajectoryId::parse("01ARZ3NDEKTSV4RRFFQ69G5FB0").unwrap()],
                recorded_at: DateTime::parse_from_rfc3339("2026-01-04T09:00:00Z").unwrap().with_timezone(&Utc),
            }
        }

        fn identity(&self) -> StrategyIdentity<'_> {
            StrategyIdentity {
                regime_key: &self.regime_key,
                role: &self.role,
                title: &self.title,
                description: &self.description,
                content: &self.content,
                source_trajectory_ids: &self.source_trajectory_ids,
                recorded_at: self.recorded_at,
            }
        }

        fn id(&self) -> StrategyId {
            StrategyId::derive(&self.identity())
        }
    }

    /// The whole point: no clock, no randomness. Two independent
    /// derivations over equal fields agree, which is what makes a
    /// rebuild reproduce the id a `Run.injected_guidance` snapshot
    /// recorded.
    #[test]
    fn derive_is_a_pure_function_of_the_identity() {
        assert_eq!(Fields::new().id(), Fields::new().id());
    }

    /// Every identity field is load-bearing — a derivation that ignored
    /// one would silently alias two genuinely different strategies onto
    /// one `<id>.parquet` filename.
    #[test]
    fn perturbing_any_single_identity_field_moves_the_id() {
        let base = Fields::new().id();

        let mut f = Fields::new();
        f.regime_key = RegimeKey::parse(canon_model::ids::regime_key("dev", "repo", "auth", "def456")).unwrap();
        assert_ne!(f.id(), base, "regime_key");

        let mut f = Fields::new();
        f.role = RoleId::parse("review").unwrap();
        assert_ne!(f.id(), base, "role");

        let mut f = Fields::new();
        f.title = format!("avoid: {}", f.title);
        assert_ne!(f.id(), base, "title");

        let mut f = Fields::new();
        f.description = "Guardrail distilled from a failure trajectory X.".to_string();
        assert_ne!(f.id(), base, "description");

        let mut f = Fields::new();
        f.content = format!("Pitfall: {}", f.content);
        assert_ne!(f.id(), base, "content");

        let mut f = Fields::new();
        f.source_trajectory_ids = vec![TrajectoryId::parse("01ARZ3NDEKTSV4RRFFQ69G5FB1").unwrap()];
        assert_ne!(f.id(), base, "source_trajectory_ids");

        let mut f = Fields::new();
        f.recorded_at = f.recorded_at.checked_add_signed(chrono::Duration::milliseconds(1)).unwrap();
        assert_ne!(f.id(), base, "recorded_at");
    }

    /// The length-prefixed preimage (`absorb`'s own doc): shifting a
    /// character across a field boundary must not produce the same
    /// hash. A naive separator-joined preimage fails this.
    #[test]
    fn field_boundaries_are_unambiguous_in_the_preimage() {
        let mut left = Fields::new();
        left.title = "ab".to_string();
        left.description = "c".to_string();

        let mut right = Fields::new();
        right.title = "a".to_string();
        right.description = "bc".to_string();

        assert_ne!(left.id(), right.id());
    }

    /// A derived id is still a ULID: `parse`/`Display` round-trip, so
    /// `canon learn promote <ULID>`'s clap `value_parser` and the
    /// `<id>.parquet` / `<id>.md` filenames keep working unchanged.
    #[test]
    fn a_derived_id_is_still_a_parseable_ulid() {
        let id = Fields::new().id();
        let text = id.to_string();
        assert_eq!(text.len(), 26, "Crockford-base32 ULID, got {text:?}");
        assert_eq!(StrategyId::parse(&text).unwrap(), id);
    }
}
