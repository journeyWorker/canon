//! The ledger-overlay `plugin.yaml` manifest: raw schema ([`schema`]),
//! the overlay field [`types::Type`], the kebab-token [`grammar`], the
//! directory [`loader`], and resolution-time validation + snapshot
//! assembly ([`resolve`]) into [`snapshot::PluginSnapshot`].
//!
//! Structurally mirrors `canon_vocab::manifest`'s module split (`crates/
//! canon-vocab/src/manifest/{loader,schema,types}.rs`) by INSPIRATION --
//! no `canon-vocab` crate dependency (design.md D2/R4). Two modules
//! canon-vocab's manifest carries have no analog here and are
//! deliberately absent: `project.rs` (a `canon.project.yaml` profile/
//! `vocabDir` override -- s16's plugin directory is a fixed `canon/
//! plugins/` relative to `project_dir`, no per-project override) and
//! `resolve.rs`'s activation-graph/`depends` machinery (every installed
//! ledger-overlay plugin is always active; there is no profile that
//! selectively activates a subset). What canon-vocab calls `assemble.rs`
//! (folding loaded packages into one snapshot) and `resolve.rs`
//! (validating what's active) are combined here into one [`resolve`]
//! module, since canon-plugin has no separate activation step to
//! validate.

pub mod grammar;
pub mod loader;
pub mod resolve;
pub mod schema;
pub mod snapshot;
pub mod types;
