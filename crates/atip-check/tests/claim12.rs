//! `atip check` claims atip/1 1.2 (approaches/checker.md 2 and 5-7): `--contract 1.2`, the rows that only a 1.2 claim runs, and the
//! checker's own declaration. Each test carries the id of the scenario it proves (`atip.check.NN`).

mod support;

use serde_json::{json, Value};

use support::{atip_in, Harness, Output, ATIP};

fn result_of(output: &Output, id: &str) -> String {
    output.check(id)["result"].as_str().unwrap_or_default().to_string()
}

/// The checks of the report that are not `pass`, `unsupported` or `entailed`.
fn failing(output: &Output) -> Vec<String> {
    output
        .results()
        .into_iter()
        .filter(|(_, result)| matches!(result.as_str(), "fail" | "unprovided"))
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn atip_check_99_contract_1_2_from_the_option_or_the_fixture_is_a_claim_of_1_2_and_the_report_carries_it() {
    let harness = Harness::new();
    let config = json!({"claims": "1.2"});
    // The fixture alone.
    let fixture = harness.fixture(&config);
    assert_eq!(fixture["contractVersion"], "1.2");
    let output = harness.check(&fixture, true);
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    assert_eq!(output.envelope()["data"]["contractVersion"], "1.2");
    assert_eq!(output.envelope()["data"]["checks"].as_array().map(Vec::len), Some(28));
    assert!(failing(&output).is_empty(), "{:?}", output.results());
    // The option alone.
    let mut without = fixture.clone();
    without.as_object_mut().expect("object").remove("contractVersion");
    let output = harness.check_with(&without, true, &["--contract", "1.2"]);
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    assert_eq!(output.envelope()["data"]["contractVersion"], "1.2");
    // The same claim in both places.
    assert_eq!(harness.check_with(&fixture, true, &["--contract", "1.2"]).code, Some(0));
    // The rules that applied to 1.1 apply to 1.2: a 1.2 tool whose ordinary answer lacks the markers fails `L0.contract-version`.
    let lacking = harness.check_config(&json!({"claims": "1.2", "omitContractVersion": true}));
    assert_eq!(lacking.code, Some(1), "{}", lacking.stdout);
    assert_eq!(failing(&lacking), ["L0.contract-version"]);
    // `L0.hint-form` is a 1.1 and later rule and runs for a claim of 1.2.
    let prose = harness.check_config(&json!({"claims": "1.2", "proseHintStartsWithTool": true}));
    assert_eq!(failing(&prose), ["L0.hint-form"], "{}", prose.stdout);
}

#[test]
fn atip_check_100_two_claims_that_differ_or_a_minor_the_checker_does_not_know_are_refused_before_a_binary_runs() {
    let harness = Harness::new();
    let marker = harness.path("init.log");
    let fixture = harness.fixture(&json!({"claims": "1.2", "initLog": marker}));
    for given in ["1.1", "1.0"] {
        let output = harness.check_with(&fixture, true, &["--contract", given]);
        let document = output.envelope();
        assert_eq!((output.code, document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("--contract")), "{given}: {}", output.stdout);
        assert_eq!(document["error"]["expected"], json!(["1.2"]));
        assert!(!marker.exists(), "the setup ran although the claims differ");
    }
    let mut without = fixture.clone();
    without.as_object_mut().expect("object").remove("contractVersion");
    let output = harness.check_with(&without, true, &["--contract", "1.3"]);
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("--contract")), "{}", output.stdout);
    assert_eq!(document["error"]["expected"], json!(["1.0", "1.1", "1.2"]));
    assert!(!marker.exists());
    // A fixture that claims another minor is refused as a fixture.
    let mut other = fixture.clone();
    other["contractVersion"] = json!("1.3");
    let output = harness.check(&other, true);
    assert_eq!((output.code, output.envelope()["error"]["code"].as_str()), (Some(2), Some("FIXTURE_INVALID")), "{}", output.stdout);
    assert!(output.envelope()["error"]["message"].as_str().unwrap_or_default().contains("1.2"), "{}", output.stdout);
    assert!(!marker.exists());
}

#[test]
fn atip_check_101_a_1_2_claim_whose_plain_answer_says_1_1_fails_l0_contract_version() {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"claims": "1.2", "contractVersionSays": "1.1"}));
    assert_eq!(output.code, Some(1), "{}", output.stdout);
    assert_eq!(failing(&output), ["L0.contract-version"], "{:?}", output.results());
    let check = output.check("L0.contract-version");
    assert!(check["reason"].as_str().unwrap_or_default().contains("the claim is \"1.2\""), "{check}");
    assert!(check["findings"].as_array().is_some_and(|findings| findings.iter().all(|finding| finding["kind"] == "version-is-claim")), "{check}");
    // The same answers are right for a 1.1 claim.
    assert_eq!(harness.check_config(&json!({"contractVersionSays": "1.1"})).code, Some(0));
}

#[test]
fn atip_check_102_the_checker_declares_1_2_and_passes_its_own_l0_checks_as_a_1_2_claim() {
    let harness = Harness::new();
    let (cwd, tmp) = (harness.path("cwd"), harness.path("tmp"));
    std::fs::create_dir_all(&cwd).expect("cwd");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("atip.fixture.json");
    let fixture = fixture.to_str().expect("utf8");
    let args = ["check", "--bin", ATIP, "--level", "L0", "--fixture", fixture, "--contract", "1.2", "--json"];
    let output = atip_in(&cwd, &tmp, &args);
    assert_eq!(output.code, Some(0), "{}{}", output.stdout, output.stderr);
    let document = output.envelope();
    assert_eq!(document["meta"]["contractVersion"], "1.2");
    assert_eq!(document["data"]["contractVersion"], "1.2");
    assert_eq!(document["data"]["passed"], true);
    // The fixture is the one that declares it, and the checker run as a 1.1 claim is refused for the difference.
    let committed: Value = serde_json::from_str(&std::fs::read_to_string(fixture).expect("fixture")).expect("json");
    assert_eq!(committed["contractVersion"], "1.2");
    let mut other = args.to_vec();
    other[8] = "1.1";
    let refused = atip_in(&cwd, &tmp, &other);
    assert_eq!((refused.code, refused.envelope()["error"]["code"].as_str()), (Some(2), Some("USAGE")), "{}", refused.stdout);
    // The help and the messages say the three minors and name no list of two.
    let help = atip_in(&cwd, &tmp, &["check", "--help"]);
    let text = format!("{}{}", help.stdout, help.stderr);
    assert!(text.contains("1.0, 1.1 or 1.2"), "{text}");
    for old in ["1.0 or 1.1", "1.0 and 1.1", "1.0, 1.1\n"] {
        assert!(!text.contains(old), "{old:?} in {text}");
    }
    for source in ["cli.rs", "fixture.rs", "app.rs", "claim.rs"] {
        let code = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(source)).expect("source");
        assert!(!code.contains("1.0 or 1.1") && !code.contains("1.0 or `1.1`"), "{source} names two minors");
    }
}
