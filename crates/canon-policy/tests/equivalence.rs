//! Task 6.1 / spec "A CEL policy.yaml derives the same required cells as
//! an equivalent static-map fixture" — the equivalence check itself now
//! lives in [`canon_policy::selftest::selftest`] (S12 task "Plus"'s
//! shared-contract fail-soft `Result<usize, Vec<String>>` entry point a
//! future `canon selftest` aggregator registers this crate's suite
//! through); this file is a thin `#[test]` wrapper over it, never a
//! second, independently-maintained copy of the same fixture logic
//! (mirrors `canon-vocab/tests/canon_core_selftest.rs`'s own precedent
//! for this exact shape).

#[test]
fn cel_and_static_map_agree_on_required_cells_across_the_fixture_corpus() {
    let result = canon_policy::selftest();
    assert!(result.is_ok(), "canon-policy selftest failures: {:#?}", result.err());
}
