//! Concrete `PlanAdapter` implementations — mirrors
//! `crate::artifact_adapters`'s per-adapter module layout. `openspec`
//! is s17's reference dialect; `superpowers` (s30
//! `plan-dialect-superpowers`) is the second, shipped against the
//! superpowers `writing-plans` skill's grammar. `donor-json` (design.md's
//! architecture diagram) stays deferred — its own deferral condition (a
//! concrete donor plan corpus) has not resolved — and lands as its own
//! sibling file, registered in `crate::plan_registry`, when it does.
//! `canon` is canon's own plan source, `.canon/changes/` (0.17 D2),
//! kept out of the registry ([`crate::plan_registry::native`]).

pub mod canon;
pub mod openspec;
pub mod superpowers;
