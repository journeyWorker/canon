//! `canon-model`: canon's closed, versioned artifact-family types (S1).
//!
//! Every record kind carries the shared [`envelope::Envelope`]
//! (`{schema, kind, at, actor}`, never a bare `by` string); the nine
//! join-spine keys ([`ids`]) tie every kind together; [`evidence`]
//! implements "malformed evidence is no evidence" (skip + violation,
//! never panic); [`handoff`] is wire-compatible with the donor handoff queue's table.
//!
//! JSON-schema export ([`schema_export`]) and the generated join-spine
//! doc ([`join_spine_doc`]) are both driven by [`gen`], which the
//! `xtask` binary (`src/bin/xtask.rs`, `cargo xtask check-generated`)
//! and this crate's own tests both call — so drift between the
//! committed `JOIN_SPINE.md`/`schemas/*.schema.json` output and the
//! Rust source fails `cargo test --workspace`, not just a separate,
//! easy-to-forget CI step.

pub mod envelope;
pub mod evidence;
pub mod family;
pub mod fold;
pub mod gen;
pub mod handoff;
pub mod ids;
pub mod join_spine_doc;
pub mod paths;
pub mod records;
pub mod schema_export;
pub mod trust;

pub use envelope::{Actor, CanonRecord, Envelope, RecordKind};
pub use evidence::{EvidenceViolation, FailureClass, RawRecord, validate_evidence, validate_evidence_batch};
pub use fold::{BindingSnapshot, FoldedState, fold_to_current_state};
pub use handoff::{DomainId, GihoekTemplate, Handoff, HandoffBody, HandoffState, HandoffTemplate, TemplateRegistry};
pub use ids::{
    ChangeId, HandoffId, JoinKeyError, PrNumber, ProjectId, RegimeKey, RoleId, RunId, ScenarioId, Sha, SessionId,
    SpecDigest, SubjectId, TaskId, TotalOrder, regime_key,
};
pub use records::{
    Change, ChangeStatus, Divergence, DivergenceStatus, Event, EvidenceRecord, EvidenceVerdict, ProvenanceRef, Review, Run,
    RunStatus, Scenario, Session, StrategyItem, StrategyRef, Subject, SubjectStatus, Task, TaskStatus, Trajectory,
};
pub use trust::{FlaggedOverlay, TrustLifecycle};

#[cfg(test)]
mod fixtures;
