//! s19 `wip-feature-stub-class` spec: `canon fmt --check`'s `LayoutGrammar`
//! message for the EXACT shape a fresh `canon feature new` writes (a
//! `Feature:` header + one paired provenance comment, ZERO `@`-tagged
//! scenarios) leads with "empty feature stub (not yet a valid corpus
//! entry)" instead of generic grammar-mismatch phrasing -- a rendering
//! change only: the class stays `layout-grammar`, and an UNRELATED
//! `LayoutGrammar` cause (a flat pre-migration path) keeps its original
//! phrasing untouched.

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
fn a_fresh_feature_new_shaped_stub_gets_the_reworded_wip_message() {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("Feature: Checkout flow\n{PROV}\n");
    write(dir.path(), "features/kind=feature/area=world/checkout.feature", &text);

    let report = check(dir.path());
    let hit = report
        .violations
        .iter()
        .find(|v| v.class == FmtFailureClass::LayoutGrammar && v.path.ends_with("checkout.feature"))
        .unwrap_or_else(|| panic!("expected a layout-grammar violation for the empty stub, got: {report:#?}"));

    assert!(
        hit.detail.starts_with("empty feature stub (not yet a valid corpus entry)"),
        "expected the reworded WIP-stub message, got: {}",
        hit.detail
    );
    // Rendering change only -- still `layout-grammar`, still reported
    // (S11's `--check` exit-code contract for this violation is
    // unchanged -- `canon-cli`'s own `a_bare_feature_new_stub_is_fmt_
    // dirty_until_scenario_new_adds_a_tagged_scenario` test pins the
    // nonzero exit code end to end).
    assert_eq!(hit.class, FmtFailureClass::LayoutGrammar);
}

#[test]
fn an_unrelated_layout_grammar_cause_keeps_its_original_phrasing() {
    let dir = tempfile::tempdir().unwrap();
    // A flat, pre-migration `features/<slug>.feature` path -- a REAL,
    // tagged scenario present (so `resolve_feature` succeeds and the
    // violation comes from `layout_problem`'s path-shape check, a
    // completely different code path than the empty-stub detector,
    // which only ever fires from the zero-scenario `Err(ResolveError)`
    // arm).
    let text = format!("Feature: idolive hub\n{PROV}\n\n  @idolive.hub.01\n  Scenario: Opens the hub\n{PROV}\n    Given a step\n");
    write(dir.path(), "features/idolive-hub.feature", &text);

    let report = check(dir.path());
    let hit = report
        .violations
        .iter()
        .find(|v| v.class == FmtFailureClass::LayoutGrammar && v.path.ends_with("idolive-hub.feature"))
        .unwrap_or_else(|| panic!("expected a layout-grammar violation for the flat pre-migration path, got: {report:#?}"));

    assert!(
        !hit.detail.starts_with("empty feature stub"),
        "an unrelated LayoutGrammar cause must NEVER get the empty-stub wording: {}",
        hit.detail
    );
}

#[test]
fn scan_cost_and_violation_count_are_unchanged_by_the_stub_rewording() {
    let dir = tempfile::tempdir().unwrap();
    let stub = format!("Feature: Checkout flow\n{PROV}\n");
    write(dir.path(), "features/kind=feature/area=world/checkout.feature", &stub);
    let wellformed = format!("Feature: world hotdeal\n{PROV}\n\n  @world.hotdeal.01\n  Scenario: Apply a hotdeal coupon\n{PROV}\n    Given a step\n");
    write(dir.path(), "features/kind=feature/area=world/hotdeal.feature", &wellformed);

    let report = check(dir.path());
    assert_eq!(report.files_checked, 2, "exactly the two files under features/ were scanned, no second pass: {report:#?}");
    assert_eq!(
        report.violations.iter().filter(|v| v.class == FmtFailureClass::LayoutGrammar).count(),
        1,
        "exactly one layout-grammar violation (the empty stub) — the well-formed sibling reports none: {report:#?}"
    );
}
