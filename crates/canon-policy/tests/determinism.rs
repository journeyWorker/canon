//! Task 6.3: the determinism fixture — evaluate a set of CEL
//! expressions against fixed input facts repeatedly and assert
//! byte-identical results (design D4/D5's risk-mitigation "mechanical
//! purity/determinism smoke test"). The check itself now lives in
//! [`canon_policy::selftest::selftest`] (S12 task "Plus"'s
//! shared-contract fail-soft `Result<usize, Vec<String>>` entry point a
//! future `canon selftest` aggregator registers this crate's suite
//! through); this file is a thin `#[test]` wrapper over it, never a
//! second, independently-maintained copy of the same fixture logic.

#[test]
fn repeated_evaluation_of_the_same_expression_against_the_same_facts_is_byte_identical() {
    let result = canon_policy::selftest();
    assert!(result.is_ok(), "canon-policy selftest failures: {:#?}", result.err());
}
