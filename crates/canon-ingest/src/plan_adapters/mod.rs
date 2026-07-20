//! Concrete `PlanAdapter` implementations — mirrors
//! `crate::artifact_adapters`'s per-adapter module layout. `openspec`
//! is s17's reference dialect; `superpowers` (s30
//! `plan-dialect-superpowers`) is the second, shipped against the
//! superpowers `writing-plans` skill's grammar. `donor-json` (design.md's
//! architecture diagram) stays deferred — its own deferral condition (a
//! concrete donor plan corpus) has not resolved — and lands as its own
//! sibling file, registered in `crate::plan_registry`, when it does.

pub mod openspec;
pub mod superpowers;
