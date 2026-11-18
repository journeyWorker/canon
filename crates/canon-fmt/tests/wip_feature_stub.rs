//! s19 `wip-feature-stub-class`: the empty-stub rendering of a `LayoutGrammar`
//! violation changes neither the scan count nor the number of violations.

use std::path::Path;

use canon_fmt::check;
use canon_fmt::report::FmtFailureClass;

const PROV: &str = "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"a-human\"}}";

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

#[test]
fn scan_cost_and_violation_count_are_unchanged_by_the_stub_rewording() {
    let dir = tempfile::tempdir().unwrap();
    let stub = format!("Feature: Checkout flow\n{PROV}\n");
    write(dir.path(), "kind=feature/area=world/checkout.feature", &stub);
    let wellformed = format!("Feature: world hotdeal\n{PROV}\n\n  @world.hotdeal.01\n  Scenario: Apply a hotdeal coupon\n{PROV}\n    Given a step\n");
    write(dir.path(), "kind=feature/area=world/hotdeal.feature", &wellformed);

    let report = check(dir.path());
    assert_eq!(report.files_checked, 2, "exactly the two files under kind=feature/ were scanned, no second pass: {report:#?}");
    assert_eq!(
        report.violations.iter().filter(|v| v.class == FmtFailureClass::LayoutGrammar).count(),
        1,
        "exactly one layout-grammar violation (the empty stub) — the well-formed sibling reports none: {report:#?}"
    );
}
