//! 0.17 D1: a spec root holds `kind=feature/area=<area>/<surface>.feature`
//! directly. A root still holding the pre-0.17 `features/kind=feature/`
//! corpus is refused with one `layout-grammar` violation that names the
//! root and `canon migrate`; its files are never read as the corpus.

use std::path::Path;

use canon_fmt::check;
use canon_fmt::report::FmtFailureClass;

const PROV: &str = "  # canon: {\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{\"agent_id\":\"a-human\"}}";

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn hotdeal() -> String {
    format!("Feature: world hotdeal\n{PROV}\n\n  @world.hotdeal.01\n  Scenario: Apply a hotdeal coupon\n{PROV}\n    Given a step\n")
}

#[test]
fn the_new_layout_is_clean() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "kind=feature/area=world/hotdeal.feature", &hotdeal());
    let report = check(dir.path());
    assert!(report.is_clean(), "{report:#?}");
    assert_eq!(report.files_checked, 1);
}

#[test]
fn a_root_inside_a_dot_directory_is_walked() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path().join(".canon/specs");
    write(&root, "kind=feature/area=world/hotdeal.feature", &hotdeal());
    let report = check(&root);
    assert!(report.is_clean(), "{report:#?}");
    assert_eq!(report.files_checked, 1, "a hidden root must be walked like any other");
}

#[test]
fn the_pre_0_17_layout_is_one_violation_naming_the_root_and_canon_migrate() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "features/kind=feature/area=world/hotdeal.feature", &hotdeal());
    write(dir.path(), "features/kind=feature/area=world/coupon.feature", &hotdeal().replace("hotdeal", "coupon"));
    let report = check(dir.path());
    assert_eq!(report.violations.len(), 1, "one violation per root, not per file: {report:#?}");
    let v = &report.violations[0];
    assert_eq!(v.class, FmtFailureClass::LayoutGrammar);
    assert_eq!(v.path, Path::new("features"));
    assert!(v.detail.contains(&format!("spec root `{}`", dir.path().display())), "{}", v.detail);
    assert!(v.detail.contains("features/kind=feature/area=world/coupon.feature"), "names the first file in path order: {}", v.detail);
    assert!(v.detail.contains("`canon migrate`"), "{}", v.detail);
    assert_eq!(report.files_checked, 0, "the old layout is refused, never read");
}

#[test]
fn an_empty_leftover_features_dir_is_not_a_refusal() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("features/kind=feature/area=world")).unwrap();
    write(dir.path(), "kind=feature/area=world/hotdeal.feature", &hotdeal());
    assert!(check(dir.path()).is_clean());
}
