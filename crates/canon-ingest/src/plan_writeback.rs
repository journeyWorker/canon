//! `PlanWriteBack` — the per-dialect, OPTIONAL write-back capability
//! alongside [`crate::plan_adapter::PlanAdapter`] (s35 `gate-plan-
//! dialect-seam`, design D1).
//!
//! # Why a seam, not a hardcoded path
//! Before s35, `canon gate task <task_id>` (`crates/canon-cli/src/
//! gate.rs`) hardcoded `<repo>/openspec/changes/<change_id>/tasks.md`
//! (and its `tasks.vocab.yaml` sibling): the trust spine itself was
//! coupled to ONE plan dialect's directory layout, so a consumer whose
//! plans are a different dialect could not use the evidence-gated flip
//! at all. This trait moves that dialect knowledge back where every
//! other on-disk-shape decision already lives — the plan adapter — and
//! leaves `canon gate` dialect-agnostic: it resolves the task's plan
//! source from `canon.yaml`'s `plans:` sources, runs the UNCHANGED pure
//! `canon_gate::gate_task` evidence decision, and delegates the file
//! mutation to the resolved dialect's `PlanWriteBack`.
//!
//! # Three capabilities, all layout/grammar — never evidence
//! - [`PlanWriteBack::locate_task`] resolves WHICH document carries a
//!   task's row (directory layout), `None` when no such document exists
//!   for that source. This is deliberately a FILE-existence question,
//!   not a row-existence one: whether the specific `<n>` row is present
//!   IN the located document is [`PlanWriteBack::flip_task`]'s concern
//!   ([`WriteBackError::RowNotFound`]), so a `locate_task` hit followed
//!   by a row-not-found flip stays a gate-red "no matching row", never a
//!   "no plan source found it" usage error.
//! - [`PlanWriteBack::flip_task`] is the dialect-owned document
//!   mutation: flip the row's checkbox `[ ]`→`[x]` and append the
//!   caller-supplied evidence note. IDEMPOTENT — an already-`[x]` row is
//!   a no-op ([`FlipDocOutcome::flipped`]` == false`, document returned
//!   byte-identical). An unknown `task_id` (no row for it) is a TYPED
//!   error ([`WriteBackError::RowNotFound`]), never a silent no-op. A
//!   dialect that cannot round-trip its own plan docs safely returns
//!   [`WriteBackError::Unsupported`] naming itself — loud, documented,
//!   never a silent no-op that would leave an operator believing a flip
//!   landed.
//! - [`PlanWriteBack::typed_atoms_path`] resolves WHERE a change's S10
//!   `tasks.vocab.yaml` typed-task file lives for this dialect — also
//!   dialect-owned layout. `None` for a dialect with no typed-vocabulary
//!   convention at all (the CLI then falls straight through to the
//!   untyped free evidence path).
//!
//! The trait never sees an `EvidenceRecord`, a verdict, or a policy —
//! the evidence DECISION stays entirely in `canon-gate`
//! (`gate_task` -> approved evidence-note text | violations), which this
//! crate has no dependency on (operator directive, preserved: canon-gate
//! and canon-ingest both depend only on canon-model; `canon-cli`, which
//! already depends on both, is the one place they meet).

use std::path::PathBuf;

use canon_model::ids::{ChangeId, TaskId};

/// The plan document one [`PlanWriteBack::locate_task`] call resolved a
/// task's row to — a single `document_path` today (every dialect canon
/// ships is a file-tree source), kept as a struct rather than a bare
/// `PathBuf` so a future non-path-based location can add a field
/// without a breaking change to the trait method's return type
/// (mirrors [`crate::plan_adapter::PlanSourceHandle`]'s own reasoning).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanTaskLocation {
    /// The file that carries (or would carry) this task's row.
    pub document_path: PathBuf,
}

/// The result of one [`PlanWriteBack::flip_task`] attempt on a located,
/// readable document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlipDocOutcome {
    /// The (possibly updated) full document text. Byte-identical to the
    /// input whenever `flipped` is `false` — an already-`[x]` row is
    /// never touched (fail closed, never a partial write).
    pub document: String,
    /// `true` only when the row's checkbox actually flipped `[ ]`→`[x]`
    /// in this call. `false` for an idempotent no-op on an already-`[x]`
    /// row.
    pub flipped: bool,
}

/// A [`PlanWriteBack::flip_task`] attempt that could not produce a
/// mutated document.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteBackError {
    /// The located document carries no row for this `task_id` — the
    /// "unknown task_id" case, reported not silently ignored. The
    /// wording carries "no matching row" so the CLI's stderr stays
    /// compatible with the pre-s35 `canon gate task` message.
    #[error("task {0} has no matching row in this plan document")]
    RowNotFound(TaskId),
    /// This dialect does not support evidence-gated write-back: its plan
    /// docs cannot be flipped in place safely (e.g. the superpowers
    /// `writing-plans` dialect, whose `### Task N:` sections have no
    /// canonical per-row evidence-suffix convention to round-trip). Loud
    /// and documented — never a silent no-op.
    #[error("plan dialect `{dialect}` does not support evidence-gated write-back (WriteBackUnsupported)")]
    Unsupported { dialect: &'static str },
}

/// One plan-dialect's OPTIONAL write-back capability (s35 design D1).
/// Registered alongside its [`crate::plan_adapter::PlanAdapter`] in
/// [`crate::plan_registry`] as `Option<&'static dyn PlanWriteBack>` — a
/// dialect that cannot (or does not yet) support the evidence-gated flip
/// simply registers `None` there, and `canon gate task` reports a loud
/// "this source's dialect has no write-back" rather than guessing.
pub trait PlanWriteBack: Send + Sync {
    /// Which document (if any) under `root` carries `task_id`'s row —
    /// FILE existence, not row existence (module doc). `None` means this
    /// source does not hold the task's change at all; the CLI moves on
    /// to the next configured source.
    fn locate_task(&self, root: &std::path::Path, task_id: &TaskId) -> Option<PlanTaskLocation>;

    /// Flip `task_id`'s row in `document` to `- [x] ` with `evidence_note`
    /// appended — idempotent no-op on an already-`[x]` row, typed
    /// [`WriteBackError::RowNotFound`] when the document has no such row
    /// (module doc). `document` is the already-read file text; the CLI
    /// owns the read/write I/O around this pure transformation.
    fn flip_task(&self, document: &str, task_id: &TaskId, evidence_note: &str) -> Result<FlipDocOutcome, WriteBackError>;

    /// Where this dialect's S10 `tasks.vocab.yaml` typed-task file for
    /// `change_id` lives under `root` — `None` for a dialect with no
    /// typed-vocabulary convention. The returned path need not exist;
    /// the caller treats an absent file as "no typed atom, use the free
    /// path" identically to a `None` return.
    fn typed_atoms_path(&self, root: &std::path::Path, change_id: &ChangeId) -> Option<PathBuf>;
}
