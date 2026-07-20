//! Concrete `ArtifactAdapter` implementations — mirrors
//! `crate::adapters`'s per-client `SessionAdapter` module layout.
//! `ledger`/`divergence`/`handoff`/`openspec_task` are the four S4
//! wave-2 verdict-deriving adapters `crate::artifact_adapter::ArtifactAdapter`
//! FOUNDATION freezes. `review`/`native_divergence` are the two S15
//! P4 NATIVE verdict records-source adapters (design D7) reading
//! canon's OWN `Review`/`Divergence` tiers — a distinct pair from
//! `divergence` (the S4 raw-manifest `Path`-kind adapter); both are
//! registered in `crate::artifact_registry`.

pub mod divergence;
pub mod handoff;
pub mod ledger;
pub mod native_divergence;
pub mod openspec_task;
pub mod review;
