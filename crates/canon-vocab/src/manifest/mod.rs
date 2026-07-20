//! Source-imported from the donor manifest layer (design.md open-Q1's
//! "leaf-plus-one-hop": depends on the donor span primitives alone, nothing
//! from the donor checker/CEL/syntax layers) as canon-owned modules — no
//! donor crate dependency, per this change's explicit constraint (canon must
//! stay a standalone repo). Every module doc comment below names its donor
//! source and states exactly what was pruned and why; nothing here silently
//! diverges from its cited source without a documented reason.

pub mod assemble;
pub mod loader;
pub mod project;
pub mod resolve;
pub mod schema;
pub mod snapshot;
pub mod types;
