//! The two store traits (raw `TrajectoryStore`, distilled
//! `StrategyStore`) + this change's one shipped backend
//! ([`parquet_trajectory::ParquetTrajectoryStore`] /
//! [`parquet_strategy::ParquetStrategyStore`]).
//!
//! **OQ2 (parquet-vs-LanceDB), resolved for this change: parquet-first.**
//! The design doc's own risk section flags that the donor's LanceDB
//! pattern store has ZERO production
//! callers — the donor service never imports its pattern-store tag, and
//! its in-memory consumers both hard-wire the in-memory store instead — so
//! EVERY reasoning-bank write across the donor's whole harness is
//! process-local and lost on restart
//! (per the donor's own learning-stack synthesis).
//! Parquet starts ahead precisely because it has no "wire a real
//! caller" step LanceDB's own donor never completed: an operator-local
//! parquet file IS the durable store the moment it's written, no
//! separate Layer-composition step to forget. Both trait below are
//! deliberately narrow (`append`/`query_by_regime_key`[/`delete_for_
//! regime_key`]) — no embedding/vector-similarity method leaks into the
//! trait surface — so a later `LanceDbTrajectoryStore`/
//! `LanceDbStrategyStore` adapter is a pure ADDITIVE impl of the same
//! trait, never a call-site rewrite (mirrors the design doc's own
//! pattern-store-tag-port rationale, risk section: "LanceDB usage is
//! isolated behind a `TrajectoryStore` trait … so a future swap … is a
//! single adapter").

mod parquet_strategy;
mod parquet_trajectory;
mod path;

pub use parquet_strategy::ParquetStrategyStore;
pub use parquet_trajectory::ParquetTrajectoryStore;

use canon_model::ids::RegimeKey;

use crate::error::LearnError;
use crate::ids::{StrategyId, TrajectoryId};
use crate::strategy::StrategyItem;
use crate::trajectory::Trajectory;
use crate::verdict_outcome::TrajectoryVerdict;

/// The raw, cold-tier trajectory store: append-only, immutable
/// (design decision 3 — nothing in this crate ever mutates or deletes
/// a stored `Trajectory`; [`crate::rebuild::rebuild_namespace`] only
/// ever touches [`StrategyStore`]).
///
/// **Documented seam for a future vector-backed impl**: a
/// `LanceDbTrajectoryStore` implementing this same trait (embedding +
/// cosine-similarity search internally, `query_by_regime_key` still
/// the exact-match entry point every read path calls) can replace or
/// sit alongside [`ParquetTrajectoryStore`] without any caller of this
/// trait changing — see this module's doc comment for the OQ2
/// rationale.
pub trait TrajectoryStore {
    /// Persists one trajectory. Never silently drops or dedups —
    /// two `append` calls with the same `id` are a caller error the
    /// concrete impl is free to reject or overwrite; this crate's own
    /// callers always mint a fresh [`crate::ids::TrajectoryId`] per
    /// trajectory.
    fn append(&self, trajectory: &Trajectory) -> Result<(), LearnError>;

    /// Every trajectory recorded under EXACTLY this `regime_key` (the
    /// full `<role>/<repo>/<area>/<hash>` join, not a role-only
    /// prefix scan — design decision 2's write/read key-identity
    /// guarantee: a trajectory recorded under a regime is always
    /// found by the identical regime tuple at read time). Returns an
    /// empty `Vec`, never an error, when nothing has been recorded for
    /// this regime yet.
    fn query_by_regime_key(&self, regime_key: &RegimeKey) -> Result<Vec<Trajectory>, LearnError>;

    /// Locates one trajectory by id alone, scanning every regime this
    /// store holds (S7 design D2/task 1.3: `mark_trajectory_verdict` is
    /// keyed by `trajectory_id` alone, mirroring the donor's own
    /// reasoning-bank verdict-write contract — the caller does not
    /// separately track `regime_key`). Returns `Ok(None)`, never an
    /// error, when nothing matches.
    fn find_by_id(&self, id: &TrajectoryId) -> Result<Option<Trajectory>, LearnError>;

    /// Persists an updated [`TrajectoryVerdict`] for the trajectory
    /// matching `id` — the ONLY mutation this trait allows on an
    /// already-appended row (design decision 3 scoped raw-tier
    /// immutability to a trajectory's EVIDENCE content: `task`/
    /// `context`/`verdicts`/`tags` never change after `append`;
    /// `mark_verdict` only ever touches the `verdict_record` layered on
    /// top, and only [`crate::mark_verdict::mark_trajectory_verdict`]
    /// calls it — `rebuild_namespace` never does). Returns
    /// `Err(LearnError::UnknownTrajectoryId)` when `id` matches nothing
    /// — never a silent no-op (contrast the donor's own in-memory
    /// verdict-write, per the donor's documented dev-reward
    /// backfill failure mode).
    fn mark_verdict(&self, id: &TrajectoryId, verdict: TrajectoryVerdict) -> Result<(), LearnError>;
}

/// The distilled, warm-tier strategy store: the ONLY store
/// [`crate::rebuild::rebuild_namespace`] ever deletes from (design
/// decision 3's non-destructive delete-rebuild — raw `Trajectory` rows
/// are never touched by this trait's methods).
///
/// Same documented-seam contract as [`TrajectoryStore`]: a future
/// vector-backed impl of this trait is a pure additive adapter.
pub trait StrategyStore {
    fn append(&self, item: &StrategyItem) -> Result<(), LearnError>;

    /// Every strategy item recorded under EXACTLY this `regime_key`
    /// (same full-tuple semantics as
    /// [`TrajectoryStore::query_by_regime_key`]) — the read side of
    /// the apply loop ([`crate::retrieve::retrieve`] wraps this with a
    /// deterministic ordering).
    fn query_by_regime_key(&self, regime_key: &RegimeKey) -> Result<Vec<StrategyItem>, LearnError>;

    /// Deletes every strategy item for `regime_key` — the deletion
    /// half of [`crate::rebuild::rebuild_namespace`]'s delete-rebuild.
    /// Returns the count deleted. A `regime_key` with nothing stored
    /// returns `Ok(0)`, never an error.
    fn delete_for_regime_key(&self, regime_key: &RegimeKey) -> Result<usize, LearnError>;

    /// Locates one strategy item by id alone, scanning every regime
    /// this store holds (S7 wave-2 design D4/task 4.1: `demote_strategy`
    /// is keyed by `strategy_id` alone, mirroring
    /// [`TrajectoryStore::find_by_id`]'s own "caller does not
    /// separately track `regime_key`" contract). Returns `Ok(None)`,
    /// never an error, when nothing matches.
    fn find_by_id(&self, id: &StrategyId) -> Result<Option<StrategyItem>, LearnError>;

    /// Persists demotion evidence onto the strategy item matching
    /// `id` — the ONLY mutation this trait allows on an already-
    /// appended row besides deletion (design decision 3 scoped the
    /// distilled tier's raw EVIDENCE content — `title`/`description`/
    /// `content`/`source_trajectory_ids` — as otherwise immutable;
    /// `mark_demoted` only ever layers [`StrategyItem::demotion`] on
    /// top, mirroring [`TrajectoryStore::mark_verdict`]'s exact
    /// precedent). Returns `Err(LearnError::UnknownStrategyId)` when
    /// `id` matches nothing — never a silent no-op (same "fail loud"
    /// discipline `mark_verdict` already established).
    fn mark_demoted(&self, id: &StrategyId, demotion: crate::strategy::DemotionEvidence) -> Result<(), LearnError>;
}
