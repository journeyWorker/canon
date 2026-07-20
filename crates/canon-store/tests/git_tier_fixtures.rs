//! `fixtures/git-tier/{well-formed,misfiled}/` — a rebindable fixture
//! corpus (design §8 testing strategy: "fixture corpora with rebindable
//! roots + an EXPECTED violations file", the parity-harness D17
//! `GateCtx`-equivalent pattern task 5.2 names explicitly) exercising
//! `GitTier::read` against REAL pre-planted files rather than only
//! records this same process just wrote (task 2.4).
//!
//! The two checks this file used to run inline (well-formed fixtures
//! read back clean; every misfiled fixture is excluded and its
//! violation count exactly matches `EXPECTED-violations.json`) now live
//! in `canon_store::selftest` — the shared per-crate selftest contract
//! the Wave-2 `canon selftest` aggregator registers directly. This
//! test's own body is a thin proof that `selftest()` itself stays
//! green, not a second copy of the checks.

#[test]
fn git_tier_fixture_corpus_selftest_passes() {
    let result = canon_store::selftest();
    assert!(result.is_ok(), "canon_store::selftest() failed: {:?}", result.err());
    assert_eq!(result.unwrap(), 2, "expected both the well-formed and misfiled fixture-corpus checks to pass");
}
