//! S11 task 8: the fixture corpus under `fixtures/consumer-corpus/pre/`
//! reproduces a donor project's real, audited drift shapes (abbreviated
//! `app_sha`, flat-not-Hive `features/`/`inventory/`, `;`- AND
//! `,`-joined `port_ref`, free-text `upstream_ref`, the fourth ad-hoc
//! `assets.lock` format, envelope-less `policy.yaml`, a one-way
//! divergence back-ref) — every sample grounded in a real corresponding
//! sample read from a donor project's `spec/**` (read-only), not
//! invented. These tests exercise `canon fmt --check` against it end to
//! end.

use std::path::{Path, PathBuf};

use canon_fmt::check;
use canon_fmt::report::FmtFailureClass;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/consumer-corpus/pre/spec")
}

#[test]
fn fmt_check_detects_every_audited_gap_category_on_fixtures() {
    // The expected-category list is owned by `canon_fmt::selftest` (the
    // one source of truth the `canon selftest` aggregator also calls) —
    // this test is just its `cargo test` entry point.
    if let Err(missing) = canon_fmt::selftest() {
        panic!("fixture corpus missing audited gap categories: {missing:?}");
    }
}

#[test]
fn fmt_check_flags_the_pre_migration_features_dir_as_layout_violation() {
    let report = check(&fixture_root());
    let hit = report
        .violations
        .iter()
        .find(|v| v.class == FmtFailureClass::LayoutGrammar && v.path.to_string_lossy().contains("features/idolive"));
    assert!(hit.is_some(), "expected a features/ layout violation, got: {report:#?}");
}

#[test]
fn fmt_check_flags_assets_lock_as_a_fourth_ad_hoc_format() {
    let report = check(&fixture_root());
    let hit = report.violations.iter().find(|v| v.path.ends_with("assets.lock"));
    assert!(hit.is_some());
    assert_eq!(hit.unwrap().class, FmtFailureClass::LayoutGrammar);
}
