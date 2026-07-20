//! Per-client `SessionAdapter` implementations. Wave 1 ships `omp`
//! only; Wave 2 adds `claude`, `codex`, `hermes` as sibling modules
//! registered in `crate::registry`.

pub mod omp;
pub mod hermes;
pub mod claude;
pub mod codex;
