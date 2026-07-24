//! canon.core selftest (design.md §8, tasks 2.6/3.4/4.6/5.4): proves the
//! REAL, checked-in `.canon/vocab/canon.core/` manifest resolves clean, a
//! good atoms fixture validates against it, a bad fixture per diagnostic
//! class produces exactly that class, and compile/round-trip is proven
//! against the SAME real manifest — not a synthetic in-crate snapshot.
//!
//! The resolve/validate/compile round-trip itself lives in
//! [`canon_vocab::selftest::selftest`] (S10 wave contract: a fail-soft
//! `Result<usize, Vec<String>>` library call a future `canon selftest`
//! aggregator can run without a `cargo test` harness) — this file is now a
//! thin `#[test]` wrapper over it, never a second, independently-maintained
//! copy of the same fixture logic.

#[test]
fn canon_vocab_selftest_passes_against_the_real_repo_fixture_corpus() {
    let result = canon_vocab::selftest();
    assert!(result.is_ok(), "canon-vocab selftest failures: {:#?}", result.err());
}
