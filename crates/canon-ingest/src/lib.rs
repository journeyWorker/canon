//! `canon-ingest`: the session-adapter registry (S3 Wave 1) + the
//! artifact-adapter foundation (S4 FOUNDATION wave).
//!
//! Scans an agent-CLI's on-disk transcript store, parses each file
//! into the shared [`adapter::UnifiedRow`] normalization target
//! (mirrors the donor's per-message unified row), and folds that into
//! canon-model's `Session`/`Run`/`Event` join spine
//! ([`normalize::normalize_rows`]). [`adapter::SessionAdapter`] is the
//! frozen contract Wave 2's claude/codex/hermes adapters implement
//! against — see that module's doc comment before changing it.
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
//! `artifact_adapters` holds the four concrete implementations
//! themselves (mirrors `adapters`'s per-client layout for
//! `SessionAdapter`), registered in [`artifact_registry`].
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
//! `crates/canon-cli`'s `canon ingest sessions` subcommand
//! (`src/ingest.rs`) is the only caller that reaches [`registry`] +
//! canon-store's write path together; this crate itself has no
//! storage dependency (pure scan/parse/normalize domain logic).

pub mod adapter;
pub mod adapters;
pub mod artifact_adapter;
pub mod artifact_adapters;
pub mod artifact_registry;
pub mod normalize;
pub mod plan_writeback;
pub mod task_rows;
pub mod plan_adapter;
pub mod plan_adapters;
pub mod plan_registry;
pub mod plan_selftest;
pub mod registry;
pub mod selftest;
pub mod scanner;
pub mod verdict;

pub use adapter::{CostSource, DirectiveRow, ParseOutcome, SessionAdapter, TokenBreakdown, UnifiedRow};
pub use artifact_adapter::{
    ArtifactAdapter, ArtifactEvent, ArtifactEventKind, ArtifactJoinKey, ArtifactParseOutcome, ArtifactSourceConfig, ArtifactSourceHandle,
};
pub use artifact_registry::{ArtifactAdapterEntry, registry as artifact_adapter_registry};
pub use normalize::{NormalizeOutcome, NormalizedSession, normalize_rows};
pub use plan_adapter::{PlanAdapter, PlanParseOutcome, PlanSourceConfig, PlanSourceHandle};
pub use plan_registry::{PlanAdapterEntry, find as find_plan_adapter, registry as plan_adapter_registry};
pub use plan_writeback::{FlipDocOutcome, PlanTaskLocation, PlanWriteBack, WriteBackError};
pub use registry::{AdapterEntry, AdapterScanResult, enumerate, parse_files, registry, scan_and_parse};
pub use selftest::selftest;
pub use verdict::{Becomes, Polarity, Verdict, VerdictRow, attach_regime_key, derive_verdict};
