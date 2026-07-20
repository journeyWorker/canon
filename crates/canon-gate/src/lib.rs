//! `canon-gate`: the trust spine — two-layer covered-vs-green evidence
//! gating over any `EvidenceRecord`-shaped corpus (S5, generalizing
//! the donor parity harness; the audited parity-harness pattern is the
//! source cited throughout
//! this crate's modules).
//!
//! # Wave scope
//! FOUNDATION landed [`failure_class`]/[`trust_ladder`]/[`policy`]/
//! [`context`] — the [`FAILURE_CLASSES`]/[`GateCtx`]/[`GateContext`]/
//! [`GateCheck`] seam every check builds against. Wave 2 landed the
//! concrete checks: [`coverage`] (D3a), [`ledger`] (D3b), [`staleness`]
//! (D4/A3), [`trust`] (D21's `TrustLadderCheck`/`ReleaseTrustCheck` +
//! the flag-clear ratchet), [`promote`] (O13 staging→committed),
//! [`checkbox`]/[`markers`] (`gated-task-completion`'s evidence-gated
//! flip + fabrication scanning), and [`hooks`] (the D8 hook-seam merge
//! logic + the generic pre-commit script). Wave 2-part2 (this commit)
//! completes S5: [`dispatch`] (the `GateCheck` set `canon gate check`
//! and this crate's own selftest both assemble from) and [`selftest`]
//! (the fixture corpus + exact-set-match oracle proving every
//! `FAILURE_CLASSES` string actually fires) — see the
//! `s5-trust-spine-gate` change's tasks. `canon-cli`'s `canon
//! gate check`/`task`/`promote`/`install-hooks`/`selftest` subcommands
//! (`crates/canon-cli/src/gate.rs`) are the CLI wiring over this crate's
//! library surface — this crate ships the gate logic, never a CLI
//! parser of its own.
//!
//! # s15 P3b: the former INTERFACE REQUESTS to canon-model are closed
//! Two gaps this crate's module docs used to name as open S1 requests
//! — `lifecycle`/`flagged` fields (design decision 2) and a staleness
//! surface-ref field — are CLOSED as of s15 P1/P3b:
//! `canon_model::EvidenceRecord` carries all five natively
//! (`lifecycle`/`flagged`/`evidence_sha`/`surface_ref`/`run_seq`,
//! design D9), and [`trust`]/[`staleness`] read them directly off
//! `ctx.evidence` — no more raw-JSON companion re-scans for these five
//! (see [`trust`]'s and [`staleness`]'s own module docs for the
//! three-way absent/well-formed/malformed read). [`trust_ladder`] now
//! only re-exports the two DATA types from `canon_model` so
//! [`TrustLadderState`]/[`TrustRung`]/[`TrustLevel`] keep compiling
//! unchanged. [`ReleaseTrustCheck`]'s `class` companion is NOT one of
//! the five migrated fields and stays a raw-JSON companion read
//! (`trust`'s own module doc).

pub mod checkbox;
pub mod context;
pub mod coverage;
pub mod dispatch;
pub mod failure_class;
pub mod hooks;
pub mod ledger;
pub mod markers;
pub mod policy;
pub mod promote;
pub mod report;
pub mod selftest;
pub mod staleness;
pub mod trust;
pub mod trust_ladder;

pub use checkbox::{gate_task, TaskFlipDecision};
pub use context::{GateCheck, GateContext, GateContextError, GateCtx};
pub use coverage::CoverageCheck;
pub use dispatch::check_set;
pub use failure_class::{FailureClass, Violation, FAILURE_CLASSES};
pub use hooks::{install_hooks, HookEntry, InstallOutcome, PRE_COMMIT_SCRIPT};
pub use ledger::{latest_verdicts, CellKey, LedgerCheck, LedgerEntry};
pub use markers::{evidence_note_of, scan_fake_markers, EvidenceNote, FABRICATION_BLOCKLIST};
pub use policy::{FromPolicyValue, PolicyDiagnostic, PolicyField, PolicyResolution, PolicyResolveError, StalenessPolicy};
pub use promote::{commit_divergence, divergence_staging_dir, promote, promote_divergence, stage_divergence, DivergenceCandidate, Promoted, PromoteReport, Refused};
pub use report::{GateFailureClass, GateReport, GateViolation};
pub use selftest::{FixtureOutcome, SelftestReport};
pub use staleness::StalenessCheck;
pub use trust::{attempt_clear, is_human_actor, FlagClearRejected, ReleaseTrustCheck, TrustLadderCheck, HUMAN_ROLE};
pub use canon_model::{FlaggedOverlay, TrustLifecycle};
pub use trust_ladder::{TrustLadderState, TrustLevel, TrustRung};
