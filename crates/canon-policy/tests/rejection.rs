//! Task 6.2: the type-invalid-rejection fixture — one expression per
//! rejection class (undeclared identifier, undeclared field, wrong
//! function arity, wrong argument type, …), each asserted rejected at
//! write time with its "expected …" diagnostic (design D3). The check
//! itself now lives in [`canon_policy::selftest::selftest`] (S12 task
//! "Plus"'s shared-contract fail-soft `Result<usize, Vec<String>>` entry
//! point a future `canon selftest` aggregator registers this crate's
//! suite through); this file is a thin `#[test]` wrapper over it, never
//! a second, independently-maintained copy of the same fixture logic.

#[test]
fn every_rejection_class_is_caught_at_write_time_with_the_expected_diagnostic() {
    let result = canon_policy::selftest();
    assert!(result.is_ok(), "canon-policy selftest failures: {:#?}", result.err());
}
