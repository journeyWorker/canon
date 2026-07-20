//! `canon-learn`'s single error type. Every fallible operation in this
//! crate returns `Result<_, LearnError>` — no per-module ad hoc error
//! enum (mirrors `canon-store`'s `StoreError` / `canon-model`'s
//! `JoinKeyError` "one error type per crate" convention).

use canon_model::ids::JoinKeyError;

#[derive(Debug, thiserror::Error)]
pub enum LearnError {
    /// A write carried a `role` not present in the active
    /// [`crate::role::RoleRegistry`] (design decision 1 / risk
    /// section: "canon-learn rejects an unregistered role at write
    /// time — fail loud, not fail soft"). Distinct from
    /// [`LearnError::JoinKey`] — the role slug itself may be
    /// perfectly well-formed and still be unregistered.
    #[error("role {0:?} is not registered in this repo's role registry (canon.yaml `learn.roles`, or the built-in set)")]
    UnregisteredRole(String),

    /// A [`crate::trajectory::Trajectory`] was constructed with zero
    /// [`canon_ingest::verdict::VerdictRow`]s — "the VerdictRow(s) …
    /// that produced the outcome" is never empty; a trajectory with no
    /// verdict evidence is not a trajectory.
    #[error("trajectory carries zero VerdictRows — at least one is required")]
    EmptyVerdicts,

    /// One of a trajectory's `VerdictRow`s was tagged with a role that
    /// does not match the trajectory's own `regime_key`'s `role`
    /// segment — the regime key's role is the single retrieval axis
    /// (design decision 2), so every verdict folded into one trajectory
    /// must agree with it.
    #[error("verdict role {verdict_role:?} does not match this trajectory's regime_key role {regime_role:?}")]
    VerdictRoleMismatch { verdict_role: String, regime_role: String },

    /// A [`crate::strategy::StrategyItem`] was constructed (or
    /// re-appended) with a `role` that does not match its own
    /// `regime_key`'s `role` segment — the distilled-tier mirror of
    /// [`LearnError::VerdictRoleMismatch`]. `regime_key`'s role is the
    /// single retrieval axis (design decision 2), so
    /// [`crate::store::parquet_strategy::ParquetStrategyStore::append`]
    /// rejects the write rather than let a `dev`-keyed strategy
    /// silently carry a `content` role (or vice versa) — the same
    /// cross-role isolation the trajectory write path already
    /// enforces.
    #[error("strategy item role {item_role:?} does not match its regime_key role {regime_role:?}")]
    StrategyRoleMismatch { item_role: String, regime_role: String },

    /// A join-spine key (`RegimeKey`/`RoleId`/…) failed its grammar
    /// check — passed through verbatim from `canon-model`.
    #[error(transparent)]
    JoinKey(#[from] JoinKeyError),

    /// A `TrajectoryId`/`StrategyId` (ULID) string failed to parse.
    #[error("invalid id {value:?}: {reason}")]
    InvalidId { value: String, reason: String },

    /// A regime-key segment is unsafe to use as a filesystem path
    /// component (defense-in-depth against a malformed/adversarial
    /// segment reaching the parquet-store's Hive-style directory
    /// layout — never expected in practice, since `regime_key()`
    /// canonicalizes every segment, but checked here rather than
    /// trusted blindly).
    #[error("regime_key segment {0:?} is not a safe path component")]
    UnsafePathSegment(String),

    /// The on-disk wire encoding of a stored row (the parquet `body`
    /// JSON blob) failed to decode back into a `Trajectory`/
    /// `StrategyItem` — a corrupt or hand-edited file, never produced
    /// by this crate's own writer.
    #[error("malformed stored row: {0}")]
    MalformedRow(String),

    /// `mark_trajectory_verdict` was called with a `trajectory_id` that
    /// matches no stored row — never a silent no-op (S7 design D2 /
    /// the donor's documented failure mode: the donor's own in-memory
    /// verdict-write silently no-ops on an
    /// unmatched id; canon fails loud instead).
    #[error("mark_trajectory_verdict: no stored trajectory matches id {0:?}")]
    UnknownTrajectoryId(String),

    /// `demote_strategy` (or `StrategyStore::find_by_id`/`mark_demoted`)
    /// was called with a `strategy_id` that matches no stored row —
    /// same "fail loud, never silently no-op" discipline as
    /// [`LearnError::UnknownTrajectoryId`] (S7 wave-2 design D4).
    #[error("demote_strategy: no stored strategy matches id {0:?}")]
    UnknownStrategyId(String),

    /// `mark_trajectory_verdict` was called with `VerdictOutcome::
    /// Pending` — `Pending` is only the trajectory's unset default,
    /// never a value a covering-verdict write may set (S7 design D2:
    /// "must not leave a trajectory pending once a covering verdict
    /// arrives" — allowing this call would let a caller re-open an
    /// already-resolved trajectory).
    #[error("mark_trajectory_verdict cannot set VerdictOutcome::Pending — Pending is only the unset default, never a covering-verdict write")]
    CannotMarkVerdictPending,

    /// `canon.yaml`'s `learn:` section failed to parse.
    #[error("canon.yaml `learn:` section: {0}")]
    Config(String),

    /// Arrow/parquet encode or decode failure.
    #[error("parquet: {0}")]
    Parquet(String),

    /// Local-filesystem I/O failure (the parquet store's operator-local
    /// files).
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// A CRN pure-statistics-core input violated its own shape
    /// contract — ragged `samples_by_config` rows (`decompose_band_
    /// variance`), mismatched paired-panel lengths (`paired_contrast`),
    /// or a non-positive `panel_size` (`seed_panels`). Mirrors
    /// MaTTS's own `throw new Error(...)` guard on these same
    /// three inputs (per the donor's MaTTS statistical-promotion
    /// audit) — a caller contract violation,
    /// never a "gracefully degrade" case (unlike `configEffectReal`/
    /// `significant` reading `false` on a too-small-but-well-formed
    /// sample, which is NOT an error).
    #[error("CRN statistics input: {0}")]
    InvalidCrnInput(String),
}
