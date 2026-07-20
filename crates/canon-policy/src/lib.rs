//! `canon-policy` (S13): canon's single policy-expression language engine
//! — CEL parse/type-check/evaluate over bindings generated from
//! canon-model's schema data (design decision 12, design doc §5 S13).
//!
//! # Why `cel`, not `cel-interpreter` (design D1 — upstream package rename)
//!
//! This change's own proposal/design docs name the dependency
//! `cel-interpreter` (crates.io) and assert it is "a structurally
//! independent implementation from the donor CEL layer's `cel-parser` 0.10.1
//! dependency — a separate crate, separate maintainer, no shared code
//! path" (design.md Risks section). That was accurate when written but
//! is no longer true of the crates.io package literally named
//! `cel-interpreter`: verified against its published `Cargo.toml`
//! (`cel-interpreter` 0.10.0, the crate's last release under that name,
//! 2025-07-23), it depends on `cel-parser = { path = "../antlr", version
//! = "0.10.1" }` — the EXACT crate and version this change's own spec
//! requires excluding (`specs/policy-expression-engine/spec.md`:
//! "the donor CEL crates and `cel-parser` are absent from every
//! transitive dependency"). Depending on the literal `cel-interpreter`
//! package would therefore fail that spec scenario immediately.
//!
//! The upstream project (`cel-rust/cel-rust` on GitHub — same repository
//! the design doc cites) renamed its published crates.io package from
//! `cel-interpreter` to `cel` starting with v0.11.0 (October 2025); `cel`
//! is the actively maintained continuation (v0.14.0 as of this change,
//! same authors, same repository) and has no `cel-parser` or donor-CEL-crate
//! dependency anywhere in its graph — its own parser is
//! folded into the `cel` crate directly. `canon-policy` therefore depends
//! on `cel`, satisfying the spec's actual dependency-exclusion invariant
//! (verified by `cargo tree`, task 1.2) rather than the now-stale crate
//! name. Every other design-doc claim about the API (`Program::compile`/
//! `execute`/`references()`, `Context::add_function`, `ExecutionError`'s
//! variants being runtime-only) still holds against `cel` 0.14.0 —
//! verified directly against its source, not assumed.
//!
//! # Module map
//!
//! - [`registry`]: [`registry::SchemaRegistry`] + [`registry::CelType`] —
//!   walks canon-model's JSON Schema export into CEL-typed fields.
//!   [`registry::SchemaRegistry::enum_domain`] (S12 task 6.1) is the
//!   single reusable "closed enum member list for (kind, field)"
//!   accessor — `canon-fmt`'s schema-violation diagnostics (S12 task
//!   6.2) source their "expected one of: …" member list from it too,
//!   never a second, hand-maintained list.
//! - [`bindings`]: [`bindings::BindingSet`] + [`bindings::bindings_for`] —
//!   the schema-derived variable/function surface a CEL expression may
//!   use (design D2).
//! - [`functions`]: the fixed, reviewed pure-function allowlist (design
//!   D4) — see its module doc for the purity audit (task 4.4).
//! - [`diagnostics`]: [`diagnostics::Diagnostic`] — the "expected …"
//!   write-time rejection shape (design D3, S12 D6 precedent).
//! - [`validate`]: [`validate::compile`] — write-time parse + AST-walk
//!   type-check (design D3); the only way to obtain a
//!   [`validate::CompiledPolicy`].
//! - [`eval`]: [`eval::evaluate`] — purity/totality/eval-budget-bounded
//!   execution (design D5).
//! - [`selftest`]: [`selftest::selftest`] — the shared-contract
//!   fail-soft `Result<usize, Vec<String>>` entry point a `canon
//!   selftest` aggregator registers this crate's own CEL fixture
//!   flows (`tests/equivalence.rs`/`tests/rejection.rs`/
//!   `tests/determinism.rs`) through.
//!
//! # The non-CEL boundary is structural (design D7)
//!
//! This crate exposes exactly two operations on a CEL expression: write-
//! time validation ([`validate::compile`]) and evaluation over caller-
//! supplied record data ([`eval::evaluate`]). There is no scripting hook,
//! no way to register a consumer-supplied function ([`functions::register`]
//! is `pub(crate)`, not part of this crate's public API), and no
//! generic "run this string as code" entry point beyond those two typed
//! operations:
//! - **Reward functions (S7) stay versioned Rust.** This crate has no
//!   reward-scoring entry point of any kind.
//! - **Ingest transforms (S3/S4) stay Rust adapters.** `canon-ingest`
//!   normalization and S4's verdict-mapping table have nothing to call
//!   here beyond the same closed `record`-in-`bool`/`value`-out surface
//!   every other consumer uses.
//! - **Evidence records (S1) never carry an expression field** —
//!   `canon-model::EvidenceRecord`'s schema (`crates/canon-model/src/
//!   records.rs`) has no string/CEL-typed field; this crate never adds
//!   one (out of `canon-policy`'s own territory in any case).
//!
//! # Intended consumers (named, not wired — proposal.md's own scope line)
//!
//! S13 ships the engine, its invariants, and its fixture corpus only.
//! Five already-authored, already-`--strict`-valid changes are this
//! crate's intended callers, each adopting `canon-policy` in its own
//! follow-up change:
//! - **S5** (policy routing): a `policy.yaml` predicate replacing tag/
//!   fact static routing.
//! - **S2** (tier aging): `age_days(record.at) > N`-shaped thresholds
//!   replacing the static duration map.
//! - **S4** (verdict-mapping guards): CEL guards on the fixed verdict-
//!   mapping table's edge cases.
//! - **S8** (retrieval filters): CEL-scoped retrieval queries.
//! - **S1 / S10** (`applies_when`): the Handoff template / typed-task-
//!   atom conditional-section language, closed-identifier CEL in the
//!   same shape as an internal monorepo's spaces-lens `applies_when:` precedent (design.md
//!   Context section).
//!
//! No consumer crate is modified by this change — this doc comment is
//! the forward reference proposal.md itself specifies, not a wiring
//! commitment.
//!
//! # `canon context` integration (design D6 — not yet wireable)
//!
//! Design D6 has S12's `resolve_surface`/`AuthoringSurface` (in
//! `canon-cli`) gain a `policy` section populated by
//! [`bindings::bindings_for`]. As of this change, S12
//! (`openspec/changes/s12-canon-context`) is proposal-only — no
//! `resolve_surface`/`AuthoringSurface` exists anywhere in the repo yet —
//! and `canon-cli` is outside this change's territory (the S11Finish/
//! S13Policy split for this batch). [`bindings::bindings_for`] is the
//! exact call D6 describes S12 making; wiring it into `canon-cli` is
//! S12's own future change, not this one.

pub mod bindings;
pub mod diagnostics;
pub mod eval;
mod functions;
pub mod registry;
pub mod selftest;
pub mod validate;

pub use bindings::{bindings_for, BindingSet, FunctionSig};
pub use diagnostics::Diagnostic;
pub use eval::{evaluate, EvalBudget, PolicyError, PolicyValue};
pub use registry::{CelType, SchemaRegistry};
pub use selftest::selftest;
pub use validate::{compile, CompiledPolicy};
