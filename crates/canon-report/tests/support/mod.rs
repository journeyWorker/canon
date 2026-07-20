//! Shared test-only glue: pulls the fixture-corpus builder in from
//! `crates/canon-report/fixtures/corpus.rs` (task 2.6's own named
//! location) so every `tests/*.rs` integration binary can `mod
//! support; use support::corpus;` instead of duplicating the `#[path]`
//! attribute per test file.

#[path = "../../fixtures/corpus.rs"]
pub mod corpus;

/// Skips a test cleanly (never fails) when the `duckdb` CLI is not on
/// `PATH` — the exact precedent `crates/canon-store/tests/
/// e2e_write_age_query_duckdb.rs` already established for this
/// workspace, so `cargo test --workspace` stays green on a machine
/// lacking the binary.
pub fn duckdb_available() -> bool {
    std::process::Command::new("duckdb").arg("--version").output().is_ok()
}
