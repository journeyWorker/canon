//! `canon-ingest`: the plan-dialect adapters behind `canon ingest plans`,
//! the artifact adapters behind `canon ingest records`, and the brief
//! and markdown readers the workflow shares.
//!
//! [`artifact_adapter::ArtifactAdapter`] is a DISTINCT, verdict-deriving
//! adapter family (S4): reads a review/CI/handoff/task-state artifact
//! -> normalized [`artifact_adapter::ArtifactEvent`]s keyed by the S1
//! join spine, which [`verdict::derive_verdict`] folds into an optional
//! `{role, polarity, becomes}` verdict. [`artifact_registry`] is its
//! (currently empty) adapter registry — S4's FOUNDATION wave freezes
//! this trait + the verdict-mapping table + `canon_model::ids::regime_key`;
//! wave-2 appends the four concrete adapters (ledger, divergence,
//! handoff, openspec-task) without touching any of it.
//!
//! `artifact_adapters` holds the concrete implementations themselves,
//! registered in [`artifact_registry`].
//!
//! [`plan_adapter::PlanAdapter`] is the third connector family (s17
//! P1, design D1): reads a foreign PLAN dialect (an openspec change
//! dir, s17's reference dialect) and normalizes it into `Change`/
//! `Task` record CANDIDATES — plan STATE, not verdict events, and a
//! DISTINCT job from `ArtifactAdapter`'s openspec-task reader even
//! though both read `openspec/changes/**` (design R5). `plan_registry`
//! is its static registry; `plan_adapters` holds the concrete dialect
//! adapters (mirrors `artifact_adapters`'s per-adapter layout).
//! [`task_rows`] is canon's own dialect-neutral checkbox-row grammar
//! (s35 `gate-plan-dialect-seam`, design D2) — the SINGLE reader +
//! writer `artifact_adapters::openspec_task`, `plan_adapters::{openspec,
//! superpowers}`, and `plan_writeback` all share, formerly split
//! between `canon-gate::checkbox` and this crate's own read-only mirror.
//! [`plan_writeback::PlanWriteBack`] is s35's per-dialect write-back
//! capability (locate/flip/typed-atoms-path); canon-gate keeps only the
//! dialect-free evidence decision.
//!
//! This crate has no storage dependency (pure scan/parse domain logic).

pub mod artifact_adapter;
pub mod artifact_adapters;
pub mod artifact_registry;
pub mod brief;
pub mod digest;
pub mod markdown;
pub mod plan_writeback;
pub mod task_rows;
pub mod plan_adapter;
pub mod plan_adapters;
pub mod plan_registry;
pub mod plan_selftest;
pub mod scanner;
pub mod verdict;

pub use artifact_adapter::{
    ArtifactAdapter, ArtifactEvent, ArtifactEventKind, ArtifactJoinKey, ArtifactParseOutcome, ArtifactSourceConfig, ArtifactSourceHandle,
};
pub use artifact_registry::{ArtifactAdapterEntry, registry as artifact_adapter_registry};
pub use plan_adapter::{PlanAdapter, PlanParseOutcome, PlanSourceConfig, PlanSourceHandle};
pub use plan_registry::{NATIVE_DIALECT, PlanAdapterEntry, find as find_plan_adapter, native as native_plan_adapter, registry as plan_adapter_registry};
pub use plan_writeback::{FlipDocOutcome, PlanTaskLocation, PlanWriteBack, WriteBackError, reject_multi_line_note};
pub use verdict::{Becomes, Polarity, Verdict, VerdictRow, attach_regime_key, derive_verdict};
