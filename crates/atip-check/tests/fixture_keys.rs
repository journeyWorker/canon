//! approaches/checker.md 5: the help line an option opens, a fixture invocation that the tool refuses (a precondition is not a
//! stateless refusal), and the fixture key `notRun`. Each test carries the id of the scenario it proves (`atip.check.NN`).

mod support;

use serde_json::{json, Value};

use support::l1::l1_config;
use support::{cases_of, Harness, Output};

fn result_of(output: &Output, id: &str) -> String {
    output.check(id)["result"].as_str().unwrap_or_default().to_string()
}

/// The checks of the report that have `result`.
fn with_result(output: &Output, result: &str) -> Vec<String> {
    output
        .results()
        .into_iter()
        .filter(|(_, found)| found == result)
        .map(|(id, _)| id)
        .collect()
}

/// A fixture registry with one more exit 2 row.
fn with_code(fixture: &mut Value, code: &str) {
    fixture["registry"].as_array_mut().expect("registry").push(json!({"code": code, "exit": 2, "retryable": false}));
}

/// The refusal a tool without a token makes, as a configuration: `commands` need the variable `FAKE_TOKEN`.
fn needs_token(code: &str, commands: &[&str]) -> Value {
    json!({
        "refuseWith": {"code": code, "commands": commands, "unlessEnv": "FAKE_TOKEN"},
        "extraCodes": [{"code": code, "exit": 2}]
    })
}

/// `config` with the members of `extra` set.
fn merged(config: &Value, extra: &Value) -> Value {
    let mut merged = config.clone();
    for (key, value) in extra.as_object().expect("object") {
        merged[key] = value.clone();
    }
    merged
}

/// The finding of `check` of kind `kind`.
fn finding(check: &Value, kind: &str) -> Value {
    check["findings"]
        .as_array()
        .and_then(|findings| findings.iter().find(|finding| finding["kind"] == kind))
        .cloned()
        .unwrap_or_else(|| panic!("{} has no finding of kind {kind}: {check}", check["id"]))
}

#[test]
fn atip_check_86_an_option_whose_description_names_json_does_not_change_the_default_read_for_json() {
    let harness = Harness::new();
    // The `--limit` line says `--json` in its description and shows a default; the `--json` line shows none.
    let config = l1_config(&json!({"helpMentions": {"limit": "--json"}}));
    let output = harness.check_l1(&config, &harness.l1_fixture(&config));
    assert_eq!(result_of(&output, "L1.defaults"), "pass", "{}", output.check("L1.defaults"));
    assert_eq!(output.code, Some(0), "{}", output.stdout);
}

#[test]
fn atip_check_88_a_name_inside_a_description_never_takes_the_default_of_another_option_whatever_default_it_would_have() {
    let harness = Harness::new();
    // A bullet before the options names `--limit` with the default 99, and the `--sort` line says `--limit` too: the `--limit` line is
    // the one that gives its default (50), and the check compares that one.
    let config = l1_config(&json!({"helpBullet": true, "helpMentions": {"sort": "--limit"}}));
    let output = harness.check_l1(&config, &harness.l1_fixture(&config));
    assert_eq!(result_of(&output, "L1.defaults"), "pass", "{}", output.check("L1.defaults"));
    // A default the `--limit` line itself gets wrong is still a failure of the first option, and only of that one.
    let wrong = l1_config(&json!({"helpBullet": true, "helpMentions": {"sort": "--limit"}, "helpDefaultDiffers": true}));
    let output = harness.check_l1(&wrong, &harness.l1_fixture(&wrong));
    let check = output.check("L1.defaults");
    assert_eq!(check["result"], "fail", "{check}");
    let reason = check["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("--limit with the default \"51\""), "{reason}");
    assert!(!reason.contains("--sort") && !reason.contains("--json"), "{reason}");
}

#[test]
fn atip_check_89_a_fixture_call_refused_with_a_registered_precondition_code_makes_the_checks_that_run_it_unprovided_and_names_the_code() {
    let harness = Harness::new();
    // L0: `items` is listed to succeed and to page, and the tool wants a token.
    let config = needs_token("NEEDS_TOKEN", &["items"]);
    let mut fixture = harness.fixture(&config);
    with_code(&mut fixture, "NEEDS_TOKEN");
    fixture["commands"]["items"] = json!({"ok": [["items", "--json"]], "paged": [["items", "--json"]]});
    let output = harness.check(&fixture, true);
    assert_eq!(output.code, Some(1), "{}", output.stdout);
    assert_eq!(output.envelope()["error"]["code"], "LEVEL_NOT_MET");
    assert!(with_result(&output, "fail").is_empty(), "{:?}", output.results());
    let unprovided = with_result(&output, "unprovided");
    assert_eq!(unprovided, ["L0.one-document", "L0.page"], "{:?}", output.results());
    for id in &unprovided {
        let check = output.check(id);
        let found = finding(&check, "refused-precondition");
        assert_eq!(found["code"], "NEEDS_TOKEN");
        assert_eq!(found["argv"], json!(["items", "--json"]));
        assert!(check["reason"].as_str().unwrap_or_default().contains("NEEDS_TOKEN"), "{check}");
    }
    // L1: `member list` is the paging command, and the tool wants the token for it.
    let mut config = l1_config(&needs_token("NEEDS_TOKEN", &["member list"]));
    config["docPatch"] = json!({"member list": [{"op": "add", "path": "/errors/-", "value": "NEEDS_TOKEN"}]});
    let output = harness.check_l1(&config, &harness.l1_fixture(&config));
    assert_eq!(output.code, Some(1), "{}", output.stdout);
    let unprovided = with_result(&output, "unprovided");
    for id in ["L0.page", "L0.one-document", "L1.paging", "L1.output-data", "L1.fields"] {
        assert!(unprovided.contains(&id.to_string()), "{id} is unprovided: {:?}", output.results());
        let found = finding(&output.check(id), "refused-precondition");
        assert_eq!(found["code"], "NEEDS_TOKEN", "{id}");
        assert_eq!(found["argv"].as_array().and_then(|argv| argv.first()), Some(&json!("member")), "{id}: {found}");
    }
    assert!(with_result(&output, "fail").is_empty(), "{:?}", output.results());
}

#[test]
fn atip_check_90_the_same_checks_pass_when_the_environment_removes_the_refusal() {
    let harness = Harness::new();
    let config = needs_token("NEEDS_TOKEN", &["items"]);
    let mut fixture = harness.fixture(&config);
    with_code(&mut fixture, "NEEDS_TOKEN");
    fixture["commands"]["items"] = json!({"ok": [["items", "--json"]], "paged": [["items", "--json"]]});
    fixture["env"]["FAKE_TOKEN"] = json!("1");
    let output = harness.check(&fixture, true);
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    assert_eq!(result_of(&output, "L0.one-document"), "pass");
    assert!(with_result(&output, "unprovided").is_empty(), "{:?}", output.results());
    let mut config = l1_config(&needs_token("NEEDS_TOKEN", &["member list"]));
    config["docPatch"] = json!({"member list": [{"op": "add", "path": "/errors/-", "value": "NEEDS_TOKEN"}]});
    let mut fixture = harness.l1_fixture(&config);
    fixture["env"]["FAKE_TOKEN"] = json!("1");
    let output = harness.check_l1(&config, &fixture);
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    for id in ["L0.page", "L1.paging", "L1.output-data", "L1.fields"] {
        assert_eq!(result_of(&output, id), "pass", "{id}");
    }
}

#[test]
fn atip_check_91_a_violation_seen_before_the_refusal_stays_a_fail_and_the_cursor_invalid_of_l1_paging_is_not_a_precondition() {
    let harness = Harness::new();
    // The probe's page is malformed (seen before anything is refused); `items` is then refused with the precondition code.
    let case = cases_of("L0.page").into_iter().find(|case| case.condition == "page/page-valid").expect("the case");
    let config = merged(&case.config, &needs_token("NEEDS_TOKEN", &["items"]));
    let mut fixture = harness.fixture(&config);
    with_code(&mut fixture, "NEEDS_TOKEN");
    fixture["commands"]["items"] = json!({"ok": [["items", "--json"]], "paged": [["items", "--json"]]});
    let output = harness.check(&fixture, true);
    assert_eq!(result_of(&output, "L0.page"), "fail", "{}", output.check("L0.page"));
    assert_eq!(finding(&output.check("L0.page"), "page-valid")["kind"], "page-valid");
    assert!(output.check("L0.page")["findings"].as_array().is_some_and(|findings| findings.iter().all(|found| found["kind"] != "refused-precondition")));
    // The conforming L1 tool is refused with CURSOR_INVALID by the checker's own probes, a registered exit 2 code: that is L1.paging's subject.
    let config = l1_config(&json!({}));
    let output = harness.check_l1(&config, &harness.l1_fixture(&config));
    assert_eq!(result_of(&output, "L1.paging"), "pass", "{}", output.check("L1.paging"));
    let calls = output.check("L1.paging")["observed"].as_array().cloned().unwrap_or_default();
    assert!(
        calls.iter().any(|call| call["exit"] == 2 && call["stdout"].as_str().is_some_and(|text| text.contains("CURSOR_INVALID"))),
        "L1.paging provoked CURSOR_INVALID: {calls:?}"
    );
    assert_eq!(output.code, Some(0), "{}", output.stdout);
}

#[test]
fn atip_check_92_a_fixture_call_refused_before_any_state_is_read_fails_the_check_that_runs_it() {
    let harness = Harness::new();
    // L0: the reserved codes of the stateless steps.
    for code in ["USAGE", "UNKNOWN_COMMAND", "INPUT_INVALID"] {
        let config = json!({"refuseWith": {"code": code, "commands": ["items"]}});
        let output = harness.check_config(&config);
        let check = output.check("L0.one-document");
        assert_eq!(check["result"], "fail", "{code}: {}", output.stdout);
        let found = finding(&check, "ok-is-not-refused");
        assert_eq!(found["argv"], json!(["items", "--json"]));
        assert!(found["message"].as_str().unwrap_or_default().contains(code), "{found}");
        assert!(with_result(&output, "unprovided").is_empty(), "{code}: {:?}", output.results());
    }
    // L1: a code a constraint declares, and a code the checker saw refuse a domain.
    let harness = Harness::new();
    let constraint = l1_config(&json!({"refuseWith": {"code": "SORT_CONFLICT", "commands": ["member search"]}}));
    let output = harness.check_l1(&constraint, &harness.l1_fixture(&constraint));
    assert_eq!(result_of(&output, "L0.one-document"), "fail", "{}", output.stdout);
    assert_eq!(finding(&output.check("L0.one-document"), "ok-is-not-refused")["argv"][0], "member");
    assert!(with_result(&output, "unprovided").is_empty(), "{:?}", output.results());
    let mut domain = l1_config(&json!({
        "domainCode": "RANGE_INVALID",
        "extraCodes": [{"code": "RANGE_INVALID", "exit": 2}],
        "refuseWith": {"code": "RANGE_INVALID", "commands": ["member search"]},
    }));
    domain["docPatch"] = json!({"member search": [{"op": "add", "path": "/errors/-", "value": "RANGE_INVALID"}]});
    let output = harness.check_l1(&domain, &harness.l1_fixture(&domain));
    assert_eq!(result_of(&output, "L0.one-document"), "fail", "{}", output.stdout);
    assert_eq!(finding(&output.check("L0.one-document"), "ok-is-not-refused")["argv"][0], "member");
    assert!(with_result(&output, "unprovided").is_empty(), "{:?}", output.results());
}

#[test]
fn atip_check_93_at_l0_with_no_registry_a_strange_exit_2_code_is_unprovided_and_a_reserved_one_fails() {
    let harness = Harness::new();
    let run = |code: &str| {
        let mut config = json!({"noSchema": true});
        config["refuseWith"] = json!({"code": code, "commands": ["items"]});
        let mut fixture = harness.fixture(&config);
        fixture.as_object_mut().expect("object").remove("registry");
        harness.check(&fixture, true)
    };
    let strange = run("NEEDS_TOKEN");
    assert_eq!(result_of(&strange, "L0.one-document"), "unprovided", "{}", strange.stdout);
    assert_eq!(finding(&strange.check("L0.one-document"), "refused-precondition")["code"], "NEEDS_TOKEN");
    assert_eq!(strange.code, Some(1), "the level is not met: {}", strange.stdout);
    let reserved = run("USAGE");
    assert_eq!(result_of(&reserved, "L0.one-document"), "fail", "{}", reserved.stdout);
    assert_eq!(reserved.code, Some(1));
}

#[test]
fn atip_check_94_a_precondition_code_named_like_a_validator_code_is_the_validators_and_fails() {
    let harness = Harness::new();
    // DATE_INVALID is registered with exit 2, as a precondition code would be, and `member search` declares it on a validator.
    let config = l1_config(&json!({"refuseWith": {"code": "DATE_INVALID", "commands": ["member search"]}}));
    let output = harness.check_l1(&config, &harness.l1_fixture(&config));
    let check = output.check("L0.one-document");
    assert_eq!(check["result"], "fail", "{}", output.stdout);
    assert_eq!(finding(&check, "ok-is-not-refused")["argv"][0], "member");
    assert!(with_result(&output, "unprovided").is_empty(), "{:?}", output.results());
}
