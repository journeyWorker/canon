//! `canon-store`: `TierAdapter`-conforming git/pg/r2/sqlite storage
//! (S2, sqlite added s32 `sqlite-hot-backend`) —
//! `GitTier`/`PgTier`/`R2Tier`/`SqliteTier` behind the one
//! [`tier::Tier`] trait, [`policy::TierPolicy`] resolving
//! `canon.yaml`'s routing/aging rules, and [`registry::TierRegistry`]
//! as the ergonomic entry point `canon-ingest` (S3), `canon-gate`
//! (S5), and `canon-learn` (S6) write/read/age through.

pub mod atomic;
pub mod cursor;
pub mod fold;
pub mod git_tier;
pub mod partition;
pub mod pg_tier;
pub mod policy;
pub mod r2_tier;
pub mod registry;
pub mod sqlite_tier;
pub mod tier;

/// The atomic file-replacement primitive (the donor's atomic-write pattern C6, [`atomic::write_atomic`])
/// — write-to-temp + `fsync` + `rename` so a mid-write kill never leaves a
/// torn file. Re-exported for cross-crate reuse (e.g. `canon-cli`'s dispatch
/// side-channel write) so canon has ONE atomic-write implementation.
pub use atomic::write_atomic;

/// Per-source ingest watermark cursors (S3 §3, [`cursor::CursorStore`] /
/// [`cursor::SourceCursor`]) — canon-store owns the cursor type + atomic
/// IO; `canon-cli`'s `canon ingest sessions` drives read/gate/advance.
pub use cursor::{CursorDiff, CursorStore, FileSeen, SourceCursor};

/// The generic last-wins-by-`at` fold (design D11, [`fold::fold_latest_by_key`])
/// — hoisted here so `canon-gate::ledger::latest_verdicts` and every
/// other s15 consumer (sync's upsert-check, the divergence fold, the
/// flywheel) share one implementation instead of a fourth local copy.
pub use fold::fold_latest_by_key;

/// The `stg_/int_/mart_` DuckDB view layer (`sql/views.sql`, S2 design
/// D5 / S9 design D1), embedded at COMPILE time via `include_str!` —
/// the one canonical copy `canon-report` (S9) opens through `duckdb
/// -init` (`crates/canon-store/tests/e2e_write_age_query_duckdb.rs`'s
/// own established pattern), never a second, potentially-stale copy
/// re-read from disk at a caller-guessed relative path.
pub const VIEWS_SQL: &str = include_str!("../sql/views.sql");

/// The rebindable-root env var [`VIEWS_SQL`] reads (module header):
/// the git tier's `tiers.git.root`.
pub const CANON_GIT_ROOT_ENV: &str = "CANON_GIT_ROOT";

/// The rebindable-root env var [`VIEWS_SQL`] reads: the r2 tier's
/// local (or synced) parquet root.
pub const CANON_R2_ROOT_ENV: &str = "CANON_R2_ROOT";

/// The rebindable-root env var [`VIEWS_SQL`] reads (S9 addition): S6/
/// S7/S8's `canon-learn`-owned operator-local parquet store root
/// (`<repo>/canon/learn`, `crates/canon-learn/src/config.rs::
/// DEFAULT_LEARN_ROOT`) — the parent of both its `strategies/` and
/// `trajectories/` subdirectories.
pub const CANON_LEARN_ROOT_ENV: &str = "CANON_LEARN_ROOT";

/// canon-store's own shared-contract selftest entry point (Wave-2
/// `canon selftest` aggregator, per-crate registration): wraps
/// `fixtures/git-tier/{well-formed,misfiled}/` — the SAME rebindable
/// fixture corpus `tests/git_tier_fixtures.rs` exercises, now
/// registered here as the single source of truth so that test and any
/// future aggregator call through this one function rather than
/// duplicating the checks (design §8 testing strategy: "fixture
/// corpora with rebindable roots + an EXPECTED violations file", the
/// parity-harness D17 `GateCtx`-equivalent pattern). Never touches a
/// real repo, no network, side-effect-free.
///
/// `Ok(n)` reports how many independent fixture-corpus checks passed
/// (2 today: the well-formed corpus reads back clean; the misfiled
/// corpus's violation count exactly matches `EXPECTED-violations.json`).
/// `Err(_)` carries one human-readable line per failing check —
/// never panics.
pub fn selftest() -> Result<usize, Vec<String>> {
    let mut passed = 0usize;
    let mut failures = Vec::new();

    match selftest_well_formed_fixtures() {
        Ok(()) => passed += 1,
        Err(e) => failures.push(e),
    }
    match selftest_misfiled_fixtures() {
        Ok(()) => passed += 1,
        Err(e) => failures.push(e),
    }

    if failures.is_empty() {
        Ok(passed)
    } else {
        Err(failures)
    }
}

fn selftest_fixture_root(sub: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/git-tier").join(sub)
}

/// Mirrors `tests/git_tier_fixtures.rs::well_formed_fixtures_read_back_clean`'s
/// prior inline logic — now the one place that logic lives.
fn selftest_well_formed_fixtures() -> Result<(), String> {
    use canon_model::envelope::RecordKind;
    use tier::{Tier, TierQuery};

    let tier = git_tier::GitTier::new(selftest_fixture_root("well-formed"));

    let scenario = tier.read(&TierQuery::kind(RecordKind::Scenario)).map_err(|e| format!("well-formed corpus: reading `scenario` failed: {e}"))?;
    if !scenario.violations.is_empty() {
        return Err(format!("well-formed corpus: `scenario` fixture produced unexpected violations: {:?}", scenario.violations));
    }
    if scenario.records.len() != 1 || scenario.records[0].0["scenario_id"] != "world.firstbuy-hotdeal.26" {
        return Err(format!("well-formed corpus: expected exactly 1 `scenario` record `world.firstbuy-hotdeal.26`, got {:?}", scenario.records));
    }

    let change = tier.read(&TierQuery::kind(RecordKind::Change)).map_err(|e| format!("well-formed corpus: reading `change` failed: {e}"))?;
    if !change.violations.is_empty() {
        return Err(format!("well-formed corpus: `change` fixture produced unexpected violations: {:?}", change.violations));
    }
    if change.records.len() != 1 || change.records[0].0["change_id"] != "s2-tiered-storage" {
        return Err(format!("well-formed corpus: expected exactly 1 `change` record `s2-tiered-storage`, got {:?}", change.records));
    }

    Ok(())
}

/// Mirrors `tests/git_tier_fixtures.rs::
/// every_misfiled_fixture_is_excluded_and_reported_per_expected_violations`'s
/// prior inline logic — the exact-count diff against
/// `EXPECTED-violations.json` (git-tier-layout-enforcement spec).
fn selftest_misfiled_fixtures() -> Result<(), String> {
    use canon_model::envelope::RecordKind;
    use tier::{Tier, TierQuery};

    let expected_bytes = std::fs::read(selftest_fixture_root("").join("EXPECTED-violations.json"))
        .map_err(|e| format!("misfiled corpus: reading EXPECTED-violations.json failed: {e}"))?;
    let expected: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&expected_bytes).map_err(|e| format!("misfiled corpus: parsing EXPECTED-violations.json failed: {e}"))?;
    if expected.is_empty() {
        return Err("misfiled corpus: EXPECTED-violations.json is empty".to_string());
    }

    let tier = git_tier::GitTier::new(selftest_fixture_root("misfiled"));
    let mut violation_count = 0;
    for kind in [RecordKind::Scenario, RecordKind::Change, RecordKind::Task] {
        let result = tier.read(&TierQuery::kind(kind)).map_err(|e| format!("misfiled corpus: reading `{}` failed: {e}", kind.as_str()))?;
        if !result.records.is_empty() {
            return Err(format!(
                "misfiled corpus: `{}` wrongly accepted {} record(s) as valid, expected all excluded",
                kind.as_str(),
                result.records.len()
            ));
        }
        violation_count += result.violations.len();
    }

    if violation_count != expected.len() {
        return Err(format!("misfiled corpus: EXPECTED-violations.json names {} violation(s), git-tier scan produced {violation_count}", expected.len()));
    }

    for class in expected.values() {
        if class != "malformed" {
            return Err(format!("misfiled corpus: EXPECTED-violations.json entry with class `{class}` — every git-tier layout/malformed violation must be class `malformed`"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod selftest_tests {
    #[test]
    fn selftest_passes_against_the_shipped_fixture_corpus() {
        let result = super::selftest();
        assert!(result.is_ok(), "canon_store::selftest() failed: {:?}", result.err());
        assert_eq!(result.unwrap(), 2, "expected both the well-formed and misfiled fixture-corpus checks to pass");
    }
}
