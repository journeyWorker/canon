//! S11 ReviewS11 findings #1 (Critical: the registered family schema
//! must actually gate `canon fmt --check`) and #2 (inventory's optional
//! `surface=<surface>/` Hive segment) — end to end through the public
//! [`check`] entrypoint, on a small, deliberately SYNTHETIC fixture
//! corpus (unlike `fixtures/consumer-corpus/pre`, which only ever
//! reproduces REAL donor drift; these fixtures exist purely to pin
//! edge cases that corpus doesn't exercise).

use std::path::{Path, PathBuf};

use canon_fmt::check;
use canon_fmt::report::FmtFailureClass;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/schema-and-surface-checks/spec")
}

#[test]
fn a_conforming_path_run_record_missing_schema_and_at_is_a_schema_violation() {
    let report = check(&fixture_root());
    let hit = report.violations.iter().find(|v| {
        v.class == FmtFailureClass::SchemaViolation && v.path.ends_with("20260709T090000-unit-2745ca4c8-aaaaaa.json")
    });
    assert!(hit.is_some(), "expected a schema-violation for the run record missing `schema`/`at`, got: {report:#?}");

    // The path itself is conforming (zero partition keys, valid
    // TimestampedJson leaf) — this record must NOT also show up as a
    // layout-grammar violation, proving the schema check is doing real,
    // additional work distinct from the pre-existing layout pass.
    let layout_hit = report
        .violations
        .iter()
        .find(|v| v.class == FmtFailureClass::LayoutGrammar && v.path.ends_with("20260709T090000-unit-2745ca4c8-aaaaaa.json"));
    assert!(layout_hit.is_none(), "did not expect a layout-grammar violation on a path-conforming record: {report:#?}");
}

#[test]
fn a_run_record_with_wrong_type_scenario_ids_is_a_schema_violation() {
    let report = check(&fixture_root());
    let hit = report.violations.iter().find(|v| {
        v.class == FmtFailureClass::SchemaViolation && v.path.ends_with("20260709T091500-unit-2745ca4c8-bbbbbb.json")
    });
    assert!(hit.is_some(), "expected a schema-violation for the run record whose `scenario_ids` is a bare string, got: {report:#?}");
}

#[test]
fn an_unrecognized_kind_string_reports_no_registered_schema() {
    // Proves the registry LOOKUP itself is on the path — never a silent
    // pass for a `kind` the closed `FamilyKind` registry doesn't know.
    let report = check(&fixture_root());
    let hit = report.violations.iter().find(|v| {
        v.class == FmtFailureClass::SchemaViolation
            && v.path.ends_with("20260709T093000-unit-2745ca4c8-cccccc.json")
            && v.detail.contains("no registered schema for kind")
    });
    assert!(hit.is_some(), "expected a `no registered schema for kind` violation, got: {report:#?}");
}

#[test]
fn inventory_file_with_surface_segment_matching_content_is_accepted() {
    let report = check(&fixture_root());
    let hit = report
        .violations
        .iter()
        .find(|v| v.class == FmtFailureClass::LayoutGrammar && v.path.to_string_lossy().contains("surface=hub/idolive-hub.yaml"));
    assert!(hit.is_none(), "did not expect a layout-grammar violation for a surface-nested inventory file whose content agrees: {report:#?}");
}

#[test]
fn inventory_file_without_a_surface_segment_is_still_accepted() {
    let report = check(&fixture_root());
    let hit = report
        .violations
        .iter()
        .find(|v| v.class == FmtFailureClass::LayoutGrammar && v.path.to_string_lossy().contains("idolive-no-surface-segment.yaml"));
    assert!(hit.is_none(), "did not expect a layout-grammar violation for an inventory file omitting the optional surface segment: {report:#?}");
}

#[test]
fn inventory_file_with_a_surface_segment_disagreeing_with_content_is_rejected() {
    let report = check(&fixture_root());
    let hit = report.violations.iter().find(|v| {
        v.class == FmtFailureClass::LayoutGrammar && v.path.to_string_lossy().contains("surface=lounge/idolive-hub-wrong-surface.yaml")
    });
    assert!(hit.is_some(), "expected a layout-grammar violation for a surface segment that disagrees with the file's own content: {report:#?}");
}

#[test]
fn inventory_file_nested_deeper_than_the_optional_segment_is_still_rejected() {
    let report = check(&fixture_root());
    let hit = report.violations.iter().find(|v| {
        v.class == FmtFailureClass::LayoutGrammar && v.path.to_string_lossy().contains("surface=hub/extra/idolive-hub-too-deep.yaml")
    });
    assert!(hit.is_some(), "expected a layout-grammar violation for a path nested deeper than the optional surface segment allows: {report:#?}");
}
