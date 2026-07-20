//! `canon selftest [--json]` (design §8 "Testing Strategy": "every
//! gate/check crate ships fixture corpora with rebindable roots + an
//! EXPECTED violations file; `canon selftest` runs all fixtures and
//! diffs") — the GENERALIZED, cross-crate self-test COMMAND + registration
//! seam. Six tasks.md rows (S3 6.9, S4 7.4, S9 7.2, S11 8.3, S12 7.4,
//! S13 6.4) were blocked on the fact that "no `canon selftest` command
//! exists"; this provides it. The command itself unblocks all six; a
//! given row is SATISFIED only once its crate's suite is registered
//! below — today that is S11 (`format-authority`), S13
//! (`policy-expressions`), S2/S5/S10, plus S3 (`session-ingest`), S4
//! (`artifact-ingest`), and S12 (`canon-context`). Only S9's
//! duckdb-dependent `canon-report` fixtures remain unregistered.
//!
//! s15 P5 (`spec-ledger-selftest`) adds a 9th suite,
//! `crate::inventory_selftest::selftest` — inventory-sync's own
//! two-sided exact-set-match fixture corpora plus a frozen-incident
//! divergence-fold fixture, registered the same way every other
//! in-crate suite is. s16 P6 (tasks.md 6.1) adds a 10th,
//! `canon_plugin::selftest` — canon-plugin's own SYNTHETIC
//! `plugin.yaml` + overlay-record fixture corpus exercising the P1-P3
//! manifest-resolve/overlay-write-validate/read-projection pipeline
//! end to end, registered the SAME way.
//!
//! s17 P4 (tasks.md §4.1) adds an 11th, `canon_ingest::plan_selftest::selftest`
//! — canon-ingest's own SYNTHETIC openspec change-tree fixture (live/
//! archive/malformed/proposal-only dirs, rebindable scratch root)
//! driving the REAL `plan_registry`-resolved `openspec` `PlanAdapter`
//! end to end, two-sided exact-set diffed against the emitted
//! Change/Task ids + statuses + named drop counts, registered the
//! SAME way.
//!
//! ADDITIVE alongside `canon gate selftest` (S5), which stays
//! canon-gate-scoped: this aggregator REGISTERS that one as a suite
//! (zero new gate logic) plus every other crate that exposes the shared
//! `pub fn selftest() -> Result<usize, Vec<String>>` fixture-suite
//! contract — `canon-store` (S2), `canon-vocab` (S10), `canon-policy`
//! (S13), `canon-fmt` (S11). Every suite runs against its OWN crate's
//! checked-in fixture corpus with a rebindable/compile-time-embedded
//! root — side-effect-free against the real repo, safe to run
//! unconditionally in CI.
//!
//! A suite whose crate does not YET expose the contract (`canon-ingest`
//! for S3/S4, `canon-report` for S9's duckdb-dependent fixtures) is
//! simply not registered here yet — the aggregator's shape makes adding
//! one a single `suites()` line, never a re-plumb.

use std::process::ExitCode;

/// One registered suite's outcome: `passed` counts the independent
/// fixture checks that cleared; `failures` carries one human-readable
/// line per failing check (empty ⇒ the suite is clean).
pub struct SuiteResult {
    pub name: &'static str,
    pub passed: usize,
    pub failures: Vec<String>,
}

impl SuiteResult {
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Every registered suite's outcome from one `canon selftest` run.
pub struct SelftestReport {
    pub suites: Vec<SuiteResult>,
}

impl SelftestReport {
    pub fn is_clean(&self) -> bool {
        self.suites.iter().all(SuiteResult::is_clean)
    }

    /// parity.py's own two-way contract, reused (matching `canon gate
    /// selftest`'s `SelftestReport::exit_code`): `0` clean, `1` on any
    /// suite failure.
    pub fn exit_code(&self) -> u8 {
        if self.is_clean() {
            0
        } else {
            1
        }
    }

    pub fn format_human(&self) -> String {
        let mut out = String::new();
        for suite in &self.suites {
            if suite.is_clean() {
                out.push_str(&format!("ok    {} ({} check(s))\n", suite.name, suite.passed));
                continue;
            }
            out.push_str(&format!("FAIL  {}\n", suite.name));
            for failure in &suite.failures {
                out.push_str(&format!("  {failure}\n"));
            }
        }
        out
    }
}

/// Adapter over `canon-gate`'s own richer `SelftestReport` (module doc:
/// zero new gate logic, just registration into the uniform contract) —
/// reduces its per-fixture missing/extra/parse-error diff to the shared
/// `Result<usize, Vec<String>>` shape.
fn gate_suite() -> Result<usize, Vec<String>> {
    let report = canon_gate::selftest::run();
    let failures: Vec<String> = report
        .outcomes
        .iter()
        .filter(|outcome| !outcome.is_clean())
        .map(|outcome| {
            let mut parts = Vec::new();
            if let Some(err) = &outcome.expected_parse_error {
                parts.push(format!("malformed EXPECTED file: {err}"));
            }
            if !outcome.missing.is_empty() {
                parts.push(format!("{} missing (under-detection)", outcome.missing.len()));
            }
            if !outcome.extra.is_empty() {
                parts.push(format!("{} extra (over-triggering)", outcome.extra.len()));
            }
            format!("{}: {}", outcome.class.as_str(), parts.join(", "))
        })
        .collect();
    if failures.is_empty() {
        Ok(report.outcomes.len())
    } else {
        Err(failures)
    }
}

/// A registered self-test suite: its display name paired with its
/// `Result<usize, Vec<String>>` fixture-check entry point.
type Suite = (&'static str, fn() -> Result<usize, Vec<String>>);

/// The registered suites, in run order. Adding a crate's fixture corpus
/// to `canon selftest` is a single line here once that crate exposes the
/// `pub fn selftest() -> Result<usize, Vec<String>>` contract.
fn suites() -> Vec<Suite> {
    vec![
        ("trust-spine-gate", gate_suite),
        ("tiered-storage", canon_store::selftest),
        ("typed-authoring-vocabulary", canon_vocab::selftest),
        ("policy-expressions", canon_policy::selftest),
        ("format-authority", canon_fmt::selftest),
        ("session-ingest", canon_ingest::selftest),
        ("canon-context", crate::context::selftest),
        ("artifact-ingest", crate::artifact_ingest::selftest),
        ("spec-ledger-selftest", crate::inventory_selftest::selftest),
        ("plugin-overlays", canon_plugin::selftest),
        ("plan-import", canon_ingest::plan_selftest::selftest),
    ]
}

/// Run every registered suite (never short-circuits — a failing suite
/// never hides a later one's result).
pub fn run() -> SelftestReport {
    let suites = suites()
        .into_iter()
        .map(|(name, run)| match run() {
            Ok(passed) => SuiteResult { name, passed, failures: Vec::new() },
            Err(failures) => SuiteResult { name, passed: 0, failures },
        })
        .collect();
    SelftestReport { suites }
}

/// `canon selftest`'s CLI entry: `0` when every suite is clean, `1` on
/// any failure. `--json` emits a machine-readable per-suite summary.
pub fn run_selftest(json: bool) -> ExitCode {
    let report = run();
    if json {
        let suites: Vec<serde_json::Value> = report
            .suites
            .iter()
            .map(|s| serde_json::json!({ "name": s.name, "clean": s.is_clean(), "passed": s.passed, "failures": s.failures }))
            .collect();
        let summary = serde_json::json!({ "clean": report.is_clean(), "suites": suites });
        println!("{}", serde_json::to_string_pretty(&summary).expect("selftest summary is always serializable"));
    } else {
        print!("{}", report.format_human());
    }
    ExitCode::from(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_suite_is_clean_against_its_own_fixture_corpus() {
        let report = run();
        assert!(
            report.is_clean(),
            "a registered canon selftest suite is failing against its own checked-in fixtures:\n{}",
            report.format_human()
        );
        // Wave-1/2 five + Wave-3 three (ingest/context/artifact) + s15
        // P5's spec-ledger-selftest + s16 P6's plugin-overlays + s17
        // P4's plan-import.
        assert_eq!(report.suites.len(), 11, "expected 11 registered suites, got {}", report.suites.len());
    }

    #[test]
    fn a_clean_report_exits_zero() {
        assert_eq!(run().exit_code(), 0);
    }
}
