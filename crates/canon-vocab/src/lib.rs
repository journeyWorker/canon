//! `canon-vocab` (S10 foundation, `openspec/changes/
//! s10-typed-authoring-vocabulary/`): canon's typed authoring vocabulary —
//! the donor vocabulary system's plugin/manifest/resolution/checker
//! architecture retargeted at
//! canon's task-atom + handoff-template domain (design doc §5 S10, design.md
//! D1-D6).
//!
//! # Lift mechanism (design.md open-Q1, RESOLVED)
//!
//! Per the crate-graph audit: the donor manifest layer + the donor span
//! primitives (the leaf-plus-one-hop pair — the donor manifest layer depends
//! on the donor span primitives alone, nothing from the donor checker/CEL/
//! syntax layers) are SOURCE-IMPORTED as canon-owned modules ([`span`],
//! [`manifest`]) — NOT a git/path dependency on the donor monorepo (canon
//! must stay a standalone repo, this change's own explicit constraint). The
//! donor checker (fold_env/check() shape, "expected one of" sourcing
//! discipline) and the donor CLI's `run_context`/`build_input` are
//! INSPIRE-only: [`checker`] and [`resolve_snapshot`] port their ARCHITECTURE
//! against canon's own types, taking NO crate dependency on the donor
//! checker/CLI. The donor's syntax/CEL/tree-sitter/LSP layers are skipped
//! entirely — no task-atom/handoff-template analog exists for a scene-script
//! DSL parser or an embedded CEL expression layer (D2 Non-Goals).
//!
//! # The one resolution entry point (D3)
//!
//! [`resolve_snapshot::resolve_snapshot`] is the ONLY capability-snapshot
//! resolution in this crate — [`atom::validate_atoms`]/[`checker::
//! check_directive`] (the checker), and later S12's `canon context` and a
//! documented (not built) LSP extension point, all consume its output. No
//! second, independently-computed vocabulary view exists anywhere in this
//! crate.
//!
//! # Module map
//!
//! - [`span`]: lifted `TextIndex`/`Position`/`Span`/`Severity` primitives
//!   (the donor span primitives).
//! - [`manifest`]: `plugin.yaml`/`directives/*.yaml`/`enums.yaml` loading +
//!   plugin activation + snapshot assembly (the donor manifest layer, retargeted).
//! - [`policy_bridge`]: D4's evidence-kind domain, resolved from S5's OWN
//!   `canon_gate::PolicyResolution` (read-only; see its module doc for why
//!   this is NOT `canon-policy` despite this change's own assignment naming
//!   it that).
//! - [`resolve_snapshot`]: THE `resolve_snapshot(project_dir, profile)`
//!   entry point (D3).
//! - [`checker`]: `check_directive`-lineage validation + [`checker::
//!   Diagnostic`] (D2/D6).
//! - [`atom`]: the `{id, tag, attrs}` YAML atom/handoff-body record + file
//!   parser (D2 — NOT the donor vocabulary system's scene-DSL grammar).
//! - [`compile`]: typed task atom -> S1 [`canon_model::Task`] compile +
//!   decompile + round-trip (D2/D4).
//! - [`handoff_compile`]: vocabulary-defined handoff body -> S1
//!   [`canon_model::HandoffBody`] compile + render (D5).
//! - [`selftest`]: [`selftest::selftest`], the fail-soft `Result<usize,
//!   Vec<String>>` fixture-suite entry point a `canon selftest` aggregator
//!   (S10 wave contract, not built by this crate) registers this crate's
//!   corpus through — the SAME resolve/validate/compile round-trip
//!   `tests/canon_core_selftest.rs` exercises as a plain `#[test]`.
//!
//! # Scope: this change is the FOUNDATION (S10 part1)
//!
//! `canon-cli`'s `canon context` typed surface, `canon gate task`'s typed
//! evidence path (task 4.4), and the pilot consumer-repo change (task 7.1)
//! are S10 part2 — deferred, not implemented here. This crate builds/tests
//! green standalone; nothing outside `crates/canon-vocab/**` and
//! `.canon/vocab/canon.core/**` is wired to it yet.

pub mod atom;
pub mod checker;
pub mod compile;
pub mod handoff_compile;
pub mod manifest;
pub mod policy_bridge;
pub mod resolve_snapshot;
pub mod selftest;
pub mod span;

pub use atom::{AtomRecord, ParseError};
pub use checker::Diagnostic;
pub use compile::{compile_task, decompile_task, DecompileError};
pub use handoff_compile::{compile_handoff_body, render_handoff_body};
pub use manifest::snapshot::CapabilitySnapshot;
pub use resolve_snapshot::resolve_snapshot;
pub use selftest::selftest;
