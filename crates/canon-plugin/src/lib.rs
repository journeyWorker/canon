//! `canon-plugin` (s16 P1+P2+P3, `openspec/changes/s16-plugin-extensibility/`):
//! canon's ledger-overlay plugin manifest -- `plugin.yaml` schema/loader
//! scanning `canon/plugins/<id>/plugin.yaml`, ONE `resolve_plugin_snapshot`
//! capability-snapshot resolution, the overlay field [`Type`] structural-
//! shape checker (P1), the overlay write + validation path (P2):
//! [`overlay::OverlayEnvelope`], [`overlay::validate_overlay_body`], and
//! [`overlay::write_overlay`] (validate -> derive `natural_key` -> call
//! `canon_store::git_tier::GitTier::write_namespaced`) -- and the
//! read-time projection path (P3): [`project::project_overlay`], a PURE,
//! in-memory join of a core `Scenario` slice against a scanned overlay
//! record slice, never writing to a core record.
//!
//! # Mirrors canon-vocab, without depending on it (design.md D2/R4)
//!
//! `canon-vocab` (S10) already proved this architecture -- a `plugin.yaml`
//! manifest, ONE `resolve_snapshot(project_dir, profile) ->
//! (CapabilitySnapshot, Vec<Diagnostic>)` entry point, fail-soft/total/
//! never-panics -- for canon's task-atom authoring vocabulary
//! (`canon/vocab/<id>/plugin.yaml`: directives + enums). s16 builds the
//! SAME architecture for a genuinely DIFFERENT manifest content-domain:
//! ledger-record overlays (`canon/plugins/<id>/plugin.yaml`: namespace +
//! overlay declarations attached to a core record kind). Design.md D2
//! rejects conflating the two into one crate ("one crate now serves two
//! unrelated vocabularies -- exactly the 'second, independently-computed
//! view' both architectures explicitly forbid, just relocated to a worse
//! place"), so every module here is written INDEPENDENTLY, by
//! INSPIRATION against canon-vocab's shape (cited per-module below), never
//! by importing the `canon-vocab` crate. The two `plugin.yaml` surfaces
//! never share a directory, a schema, or a Rust type.
//!
//! # The one resolution entry point (mirrors design.md D3's canon-vocab
//! precedent)
//!
//! [`resolve_plugin_snapshot::resolve_plugin_snapshot`] is the ONLY
//! plugin-snapshot resolution in this crate. P2's [`overlay::write_overlay`]
//! and P3's [`project::project_overlay`] both consume this SAME function's
//! output (an [`OverlayDecl`]) -- no second, independently-computed
//! plugin view exists anywhere in this crate or its consumers.
//!
//! # Module map
//!
//! - [`diagnostic`]: [`Diagnostic`]/[`Severity`] -- canon-plugin's own
//!   small finding type (NOT `canon_vocab::checker::Diagnostic`; see the
//!   module doc for why).
//! - [`manifest`]: `plugin.yaml` schema ([`manifest::schema`]), the
//!   overlay field [`Type`] ([`manifest::types`]), the kebab-token
//!   grammar ([`manifest::grammar`]), the directory loader
//!   ([`manifest::loader`]), and resolution-time validation + snapshot
//!   assembly ([`manifest::resolve`]) into [`PluginSnapshot`].
//! - [`resolve_plugin_snapshot`][]: THE
//!   `resolve_plugin_snapshot(project_dir)` entry point.
//! - [`overlay`] (P2): [`overlay::OverlayEnvelope`] (canon-plugin's own
//!   record envelope), [`overlay::validate_overlay_body`] (the
//!   manifest-schema body validator), and [`overlay::write_overlay`] (the
//!   plugin-aware writer) -- canon-plugin's ONLY `canon-store` WRITE
//!   dependency is `GitTier::write_namespaced`/`scan_namespaced_kind`,
//!   never the typed core `Tier::write`/`read` path.
//! - [`project`] (P3): [`project::project_overlay`] -- the pure,
//!   fail-soft, in-memory read-time projection; core records are a
//!   read-only input, never rewritten (design.md D3). `canon query
//!   --plugin <id>` (canon-cli) is this function's own consumer, out of
//!   this crate.
//! - [`selftest`] (P6): [`selftest::selftest`] -- the `canon selftest`
//!   shared-contract entry point, a SYNTHETIC fixture corpus exercising
//!   the full P1-P3 pipeline above end to end with a two-sided
//!   exact-set diagnostic oracle (module doc there).
//!
//! # Scope: P1 (manifest) + P2 (overlay write/validate) + P3 (projection) + P6 (selftest)
//!
//! No porting plugin (P4), no corpus-authoring scaffold (P5) -- those
//! live in `canon-cli` (`crates/canon-cli/src/{plugin_sync,query,tiers}.rs`,
//! `crates/canon-cli/src/main.rs`). This crate builds/tests green
//! standalone; nothing outside `crates/canon-plugin/**`,
//! `crates/canon-store/src/{git_tier,partition}.rs`,
//! `crates/canon-cli/src/{main.rs,query.rs,tiers.rs,plugin_sync.rs,
//! selftest.rs}`, and the root `Cargo.toml`'s workspace member list is
//! touched by this change.

pub mod diagnostic;
pub mod manifest;
pub mod overlay;
pub mod project;
pub mod resolve_plugin_snapshot;
pub mod selftest;

pub use diagnostic::{Diagnostic, Severity};
pub use manifest::schema::{AttachesTo, FieldDecl, OverlayEntry, PluginManifest};
pub use manifest::snapshot::{OverlayDecl, PluginSnapshot, ResolvedPlugin};
pub use manifest::types::{Type, type_accepts};
pub use overlay::{OverlayEnvelope, OverlayWriteError, compose_overlay_body, validate_overlay_body, write_overlay};
pub use project::project_overlay;
pub use resolve_plugin_snapshot::resolve_plugin_snapshot;
pub use selftest::selftest;
