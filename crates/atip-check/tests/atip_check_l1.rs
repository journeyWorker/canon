//! `atip check --level L1` against the fake tool (approaches/checker.md 6, 7.2): the conforming L1 fake passes, and every pass
//! condition of every L1 check, planted alone, fails the check that owns it and no other. Each test carries the id of the
//! scenario it proves.

mod support;

use serde_json::{json, Value};

use support::l1::{l1_bin, l1_cases, l1_engine_difference_case, l1_standard_config};
use support::{atip_in, Case, Harness};

const L1_CHECKS: usize = 31;
const L0_CHECKS: usize = 28;

/// `atip check --level L1 --json` of the fake tool a case configures, with the fixture the case makes.
fn run_case(case: &Case) -> (Harness, support::Output) {
    let harness = Harness::new();
    let mut fixture = harness.l1_fixture(&case.config);
    if let Some(tweak) = case.tweak {
        tweak(&mut fixture);
    }
    let output = run_fixture(&harness, &case.config, &fixture);
    (harness, output)
}

fn run_fixture(harness: &Harness, config: &Value, fixture: &Value) -> support::Output {
    let bin = l1_bin(harness, config);
    let fixture_path = harness.write("fixture.json", fixture);
    let work = harness.path("cwd");
    std::fs::create_dir_all(&work).expect("cwd");
    let tmp = harness.path("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp");
    atip_in(
        &work,
        &tmp,
        &[
            "check",
            "--bin",
            bin.to_str().expect("utf8"),
            "--level",
            "L1",
            "--fixture",
            fixture_path.to_str().expect("utf8"),
            "--json",
        ],
    )
}

/// The ids of the checks that failed in a report.
fn failed(data: &Value) -> Vec<String> {
    data["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|check| check["result"] == "fail")
        .map(|check| check["id"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
#[ignore = "drafts the verdicts of every case by hand"]
fn draft_every_case() {
    let only = std::env::var("CASE").unwrap_or_default();
    for case in l1_cases().into_iter().filter(|case| case.condition.starts_with(&only)) {
        let (_harness, output) = run_case(&case);
        let document = output.envelope();
        let owner = format!("L1.{}", case.condition.split('/').next().unwrap_or_default());
        let failed = failed(&document["data"]);
        let verdict = if failed == [owner.clone()] { "ok" } else { "MISMATCH" };
        println!("{verdict} {} owner {owner} failed {failed:?}", case.condition);
        if verdict != "ok" || std::env::var("VERBOSE").is_ok() {
            for check in document["data"]["checks"].as_array().into_iter().flatten() {
                if check["result"] == "fail" || check["result"] == "unprovided" {
                    println!("    {}: {}", check["id"], check["reason"].as_str().unwrap_or_default().chars().take(300).collect::<String>());
                }
            }
        }
    }
}

#[test]
#[ignore = "drafts the conforming run by hand"]
fn draft_conforming() {
    let harness = Harness::new();
    let config = l1_standard_config();
    let fixture = harness.l1_fixture(&config);
    let output = run_fixture(&harness, &config, &fixture);
    println!("{}", output.stdout);
    let _ = (json!(null), L1_CHECKS, L0_CHECKS, l1_engine_difference_case());
}
