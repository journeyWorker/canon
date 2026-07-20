//! `canon-fmt`: the artifact-family format authority (S11
//! `s11-format-authority-migration`) — `canon fmt --check` validates
//! an EXTERNAL consumer-repo corpus shape (ledger/divergences/
//! features/inventory/policy.yaml) against `canon-model`'s family
//! schemas + layout descriptors (`canon_model::family`). This crate
//! never touches canon's OWN storage tiers (`canon-store`'s job, S2),
//! and never reads a live consumer-repo checkout on its own — every
//! test exercises a corpus root callers pass in (a fixture tmpdir, or
//! whatever path `canon fmt --check <root>` is invoked with).
//!
//! `canon migrate` — a one-shot rewrite of an existing corpus onto this
//! format — is explicitly OUT of scope for this crate (operator
//! directive: a throwaway migration script is a separate, later
//! concern per consumer repo; `canon-fmt` only ever validates).

pub mod check;
pub mod gherkin;
pub mod refparse;
pub mod report;
pub mod resolve;
pub mod schema_registry;
pub mod sha;
pub mod util;

pub use check::check;
pub use report::{FmtFailureClass, FmtReport, Violation};


/// canon-fmt's shared-contract selftest entry point (Wave-2 `canon
/// selftest` aggregator, per-crate registration, S11 task 8.3): runs
/// `canon fmt --check` against this crate's checked-in consumer-repo
/// fixture corpus and asserts every audited drift category the corpus
/// was built to exercise is detected — the SAME check
/// `tests/fixtures_check.rs` exercises, now the one source of truth for
/// the expected-category list. `Ok(n)` = all `n` expected categories
/// observed; `Err(_)` names each category the fixture corpus failed to
/// surface. Never panics; the fixture path is embedded at compile time
/// (no live consumer-repo read).
pub fn selftest() -> Result<usize, Vec<String>> {
    use report::FmtFailureClass::*;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/consumer-corpus/pre/spec");
    let report = check::check(&root);
    let observed: std::collections::BTreeSet<&str> = report.observed_classes().iter().map(|c| c.as_str()).collect();
    let expected =
        [LayoutGrammar, MissingEnvelope, MissingProvenance, MissingActor, UnspecifiedEvidence, FreeTextRef, JoinedRef, AbbreviatedSha, OneWayBackref, MissingJoinIdentity];
    let failures: Vec<String> =
        expected.iter().filter(|c| !observed.contains(c.as_str())).map(|c| format!("fixture corpus failed to surface `{}`", c.as_str())).collect();
    if failures.is_empty() {
        Ok(expected.len())
    } else {
        Err(failures)
    }
}