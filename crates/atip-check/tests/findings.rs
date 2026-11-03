//! approaches/checker.md 2: the report lists every finding of a failed or unprovided check as `findings[]`, each with the
//! `kind` of the condition that failed, its `message`, and the `argv` of the call that decided it. Each test carries the id
//! of the scenario it proves (`atip.check.NN`).

mod support;

use serde_json::{json, Value};

use support::l1::{l1_bin, l1_cases};
use support::{atip_in, cases, cases_of, kind_verdict, reason_of_messages, Case, Harness, Output};

/// The messages of the findings of `check`.
fn messages(check: &Value) -> Vec<String> {
    check["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|finding| finding["message"].as_str().expect("message").to_string())
        .collect()
}

/// The argv of every invocation `check` shows under `observed`.
fn observed_argvs(check: &Value) -> Vec<Value> {
    check["observed"]
        .as_array()
        .expect("observed")
        .iter()
        .map(|call| call["argv"].clone())
        .collect()
}

#[test]
fn atip_check_77_each_finding_of_a_failed_check_carries_its_kind_its_message_and_the_argv_of_the_call() {
    let harness = Harness::new();
    // Every envelope the fake prints says 1.2 to a 1.1 claim: one finding for each call.
    let output = harness.check_config(&json!({"contractVersionSays": "1.2"}));
    let data = output.envelope()["data"].clone();
    kind_verdict(&data, "L0.contract-version", "version-is-claim").unwrap_or_else(|why| panic!("{why}\n{}", output.stdout));
    let check = output.check("L0.contract-version");
    assert_eq!(check["result"], "fail");
    let findings = check["findings"].as_array().expect("findings");
    let observed = observed_argvs(&check);
    assert!(findings.len() > 3, "several calls broke the condition: {}", output.stdout);
    for finding in findings {
        assert_eq!(finding["kind"], "version-is-claim");
        let argv = finding["argv"].as_array().expect("the call that decided it").iter().map(|word| word.as_str().expect("word")).collect::<Vec<_>>();
        assert!(observed.contains(&finding["argv"]), "the argv {argv:?} is one of the observed calls: {observed:?}");
        assert!(
            finding["message"].as_str().unwrap_or_default().starts_with(&format!("`{}`: ", argv.join(" "))),
            "the message is the sentence the reason is made of: {finding}"
        );
        assert!(finding.get("code").is_none(), "{finding}");
    }
    let reason = check["reason"].as_str().expect("reason");
    assert!(reason.starts_with(findings[0]["message"].as_str().expect("message")), "the reason did not change its shape: {reason}");
}

#[test]
fn atip_check_78_a_finding_that_no_single_call_decided_has_no_argv_and_no_finding_of_a_check_carries_a_code_of_its_own() {
    let harness = Harness::new();
    // The registry rows the contract fixes are judged from the registry, not from one call.
    let case = cases_of("L0.ok-exit").into_iter().find(|case| case.condition == "ok-exit/reserved-codes").expect("the case");
    let output = harness.check(&case.fixture(&harness), true);
    let data = output.envelope()["data"].clone();
    kind_verdict(&data, "L0.ok-exit", "reserved-codes").unwrap_or_else(|why| panic!("{why}\n{}", output.stdout));
    let check = output.check("L0.ok-exit");
    let findings = check["findings"].as_array().expect("findings");
    let reserved: Vec<&Value> = findings.iter().filter(|finding| finding["kind"] == "reserved-codes").collect();
    assert!(!reserved.is_empty(), "{}", output.stdout);
    for finding in reserved {
        assert!(finding.get("argv").is_none(), "no call decided it: {finding}");
        assert!(finding.get("code").is_none(), "{finding}");
    }
    // A finding that one call decided has an argv, and none of the findings of any check has a code.
    let output = harness.check_config(&json!({"contractVersionSays": "1.2"}));
    for check in output.envelope()["data"]["checks"].as_array().expect("checks") {
        for finding in check["findings"].as_array().into_iter().flatten() {
            assert!(finding.get("code").is_none(), "{finding}");
        }
    }
}

#[test]
fn atip_check_79_a_check_that_lacks_a_fixture_entry_lists_it_as_a_missing_fixture_entry_finding_and_a_passing_check_has_no_findings() {
    let harness = Harness::new();
    let mut fixture = harness.fixture(&json!({}));
    fixture.as_object_mut().expect("object").remove("commands");
    fixture.as_object_mut().expect("object").remove("probe");
    let output = harness.check(&fixture, true);
    let data = output.envelope()["data"].clone();
    let page = output.check("L0.page");
    assert_eq!(page["result"], "unprovided", "{}", output.stdout);
    assert_eq!(page["findings"], json!([{"kind": "missing-fixture-entry", "message": "commands"}]));
    let option = output.check("L0.unknown-option");
    assert_eq!(option["result"], "unprovided");
    assert_eq!(option["findings"], json!([{"kind": "missing-fixture-entry", "message": "probe"}]));
    kind_verdict(&data, "L0.page", "missing-fixture-entry").unwrap_or_else(|why| panic!("{why}"));
    // A check that passed has no `findings` member, not an empty one; nor has one that does not apply.
    let version = output.check("L0.version");
    assert_eq!(version["result"], "pass");
    assert!(version.get("findings").is_none(), "{version}");
    for check in data["checks"].as_array().expect("checks") {
        if !matches!(check["result"].as_str(), Some("fail" | "unprovided")) {
            assert!(check.get("findings").is_none(), "{check}");
        }
    }
}

#[test]
fn atip_check_80_every_finding_is_listed_and_the_and_n_more_of_the_reason_counts_the_rest_of_the_list() {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"contractVersionSays": "1.2"}));
    let check = output.check("L0.contract-version");
    let all = messages(&check);
    assert!(all.len() >= 5, "the fake plants five violations or more: {}", all.len());
    let calls = observed_argvs(&check);
    assert_eq!(all.len(), calls.len(), "one finding for each call that broke the condition");
    let reason = check["reason"].as_str().expect("reason");
    assert!(reason.ends_with(&format!("; and {} more", all.len() - 3)), "{reason}");
    assert_eq!(reason, reason_of_messages(&all));
    // Three findings or fewer: no `and N more`.
    let output = harness.check_config(&json!({"versionStderr": true}));
    let check = output.check("L0.version");
    assert!(!check["reason"].as_str().expect("reason").contains(" more"), "{}", output.stdout);
}

#[test]
fn atip_check_81_a_message_longer_than_240_characters_is_whole_in_findings_and_cut_only_in_the_reason() {
    let harness = Harness::new();
    let mut fixture = harness.fixture(&json!({"contractVersionSays": "1.2"}));
    let name = "n".repeat(300);
    fixture["commands"]["touch"]["ok"] = json!([["touch", "--name", name, "--json"]]);
    let output = harness.check(&fixture, true);
    let check = output.check("L0.contract-version");
    let all = messages(&check);
    let long: Vec<&String> = all.iter().filter(|message| message.chars().count() > 240).collect();
    assert!(!long.is_empty(), "a message names the long argv: {}", output.stdout);
    assert!(long.iter().all(|message| message.contains(&"n".repeat(300))), "findings[] is not cut");
    let reason = check["reason"].as_str().expect("reason");
    assert_eq!(reason, reason_of_messages(&all));
    assert!(!reason.contains(&"n".repeat(300)), "the reason is cut at 240 characters of each message");
    kind_verdict(&output.envelope()["data"], "L0.contract-version", "version-is-claim").unwrap_or_else(|why| panic!("{why}"));
}

#[test]
fn atip_check_82_the_text_report_carries_no_findings_and_a_cut_reason_says_where_the_rest_is() {
    let harness = Harness::new();
    let cut = harness.check(&harness.fixture(&json!({"contractVersionSays": "1.2"})), false);
    let line = cut.stderr.lines().find(|line| line.trim_start().starts_with("fail") && line.contains("L0.contract-version")).unwrap_or_else(|| panic!("{}", cut.stderr));
    assert!(line.contains("; and ") && line.ends_with(" more (all in --json findings)"), "{line}");
    assert!(!cut.stderr.contains("\"kind\"") && !cut.stderr.lines().any(|text| text.trim_start().starts_with("findings")), "{}", cut.stderr);
    // A reason that says everything is the text it was.
    let whole = harness.check(&harness.fixture(&json!({"versionStderr": true})), false);
    let line = whole.stderr.lines().find(|line| line.trim_start().starts_with("fail") && line.contains("L0.version ")).unwrap_or_else(|| panic!("{}", whole.stderr));
    assert!(line.trim_start().starts_with("fail") && !line.contains("findings"), "{line}");
    assert!(whole.stderr.contains("L0.version "), "{}", whole.stderr);
}

#[test]
fn atip_check_123_an_unprovided_check_in_the_text_report_keeps_its_line_and_lists_no_findings() {
    let harness = Harness::new();
    let mut fixture = harness.fixture(&json!({}));
    fixture.as_object_mut().expect("object").remove("commands");
    fixture.as_object_mut().expect("object").remove("probe");
    let json = harness.check(&fixture, true);
    let check = json.check("L0.page");
    assert_eq!(check["result"], "unprovided", "{}", json.stdout);
    let reason = check["reason"].as_str().expect("reason").to_string();
    // The text report: one line for the check, led by its verdict, with the reason the JSON report has and no pointer to a list that is cut.
    let text = harness.check(&fixture, false);
    let line = text.stderr.lines().find(|line| line.trim_start().starts_with("unprovided") && line.contains("L0.page")).unwrap_or_else(|| panic!("{}", text.stderr));
    assert!(line.ends_with(&reason), "the line carries the reason of the JSON report: {line} / {reason}");
    assert!(!line.contains("all in --json findings"), "{line}");
    assert!(reason.contains("commands"), "the reason names the entry the fixture lacks: {reason}");
    assert!(!text.stderr.contains("missing-fixture-entry") && !text.stderr.contains("\"kind\""), "the text carries no findings: {}", text.stderr);
    // The text and the JSON report give the same exit code.
    assert_eq!(text.code, json.code);
}

/// Runs `jobs` on six threads.
fn in_parallel(jobs: Vec<Box<dyn FnOnce() -> Result<(), String> + Send>>) -> Vec<String> {
    let queue = std::sync::Mutex::new(jobs);
    let failures = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| loop {
                let next = queue.lock().expect("queue").pop();
                let Some(job) = next else { break };
                if let Err(why) = job() {
                    failures.lock().expect("failures").push(why);
                }
            });
        }
    });
    failures.into_inner().expect("failures")
}

fn l0_job(case: Case) -> Box<dyn FnOnce() -> Result<(), String> + Send> {
    Box::new(move || {
        let harness = Harness::new();
        let output = harness.check(&case.fixture(&harness), true);
        let kind = case.condition.split('/').nth(1).unwrap_or_default();
        kind_verdict(&output.envelope()["data"], &case.owner(), kind).map_err(|why| format!("{}: {why}", case.condition))
    })
}

fn l1_job(case: Case) -> Box<dyn FnOnce() -> Result<(), String> + Send> {
    Box::new(move || {
        let harness = Harness::new();
        let mut fixture = harness.l1_fixture(&case.config);
        if let Some(tweak) = case.tweak {
            tweak(&mut fixture);
        }
        let fixture_path = harness.write("fixture.json", &fixture);
        let bin = l1_bin(&harness, &case.config);
        let work = harness.path("cwd");
        std::fs::create_dir_all(&work).expect("cwd");
        let tmp = harness.path("tmp");
        std::fs::create_dir_all(&tmp).expect("tmp");
        let output = atip_in(
            &work,
            &tmp,
            &["check", "--bin", bin.to_str().expect("utf8"), "--level", "L1", "--fixture", fixture_path.to_str().expect("utf8"), "--json"],
        );
        let owner = format!("L1.{}", case.condition.split('/').next().unwrap_or_default());
        let kind = case.condition.split('/').nth(1).unwrap_or_default();
        kind_verdict(&output.envelope()["data"], &owner, kind).map_err(|why| format!("{}: {why}", case.condition))
    })
}

#[test]
fn atip_check_83_every_seeded_violation_is_found_by_its_kind_in_the_findings_of_its_owner() {
    let mut jobs: Vec<Box<dyn FnOnce() -> Result<(), String> + Send>> = cases().into_iter().map(l0_job).collect();
    let l0 = jobs.len();
    jobs.extend(l1_cases().into_iter().filter(|case| case.condition != "tree/level").map(l1_job));
    assert!(jobs.len() > l0 && l0 > 70, "the seeded cases of both levels run: {l0} of {}", jobs.len());
    let failures = in_parallel(jobs);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn atip_check_84_a_renamed_condition_with_the_test_expectation_unchanged_fails_the_test() {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"contractVersionSays": "1.2"}));
    let data = output.envelope()["data"].clone();
    kind_verdict(&data, "L0.contract-version", "version-is-claim").expect("the condition's name is the kind");
    // The condition is renamed in the checker and the test is not: the report says the new name, the test still expects the old.
    let mut renamed = data.clone();
    for check in renamed["checks"].as_array_mut().expect("checks") {
        for finding in check.get_mut("findings").and_then(Value::as_array_mut).into_iter().flatten() {
            if finding["kind"] == "version-is-claim" {
                finding["kind"] = json!("version-is-the-claim");
            }
        }
    }
    let why = kind_verdict(&renamed, "L0.contract-version", "version-is-claim").expect_err("the old name is not in the report");
    assert!(why.contains("no finding of kind version-is-claim"), "{why}");
    // A finding with a member of its own, or a code that is not a refused call's, is refused as well.
    let mut extra = data.clone();
    extra["checks"].as_array_mut().expect("checks").iter_mut().find(|check| check["id"] == "L0.contract-version").expect("check")["findings"][0]["why"] = json!("x");
    assert!(kind_verdict(&extra, "L0.contract-version", "version-is-claim").is_err());
    let mut coded = data;
    coded["checks"].as_array_mut().expect("checks").iter_mut().find(|check| check["id"] == "L0.contract-version").expect("check")["findings"][0]["code"] = json!("USAGE");
    assert!(kind_verdict(&coded, "L0.contract-version", "version-is-claim").is_err());
}

#[test]
fn atip_check_85_the_results_the_level_and_the_exit_code_are_the_same_with_findings_as_without() {
    let harness = Harness::new();
    let verdicts = |output: &Output| -> (Option<i32>, Value, Vec<(String, String)>) {
        let document = output.envelope();
        (output.code, document["data"]["passed"].clone(), output.results())
    };
    // A conforming tool passes and no check of it has a `findings` member.
    let conforming = harness.check_config(&json!({}));
    assert_eq!((conforming.code, conforming.envelope()["data"]["passed"].clone()), (Some(0), json!(true)), "{}", conforming.stdout);
    assert!(conforming.envelope()["data"]["checks"].as_array().expect("checks").iter().all(|check| check.get("findings").is_none()));
    // A MUST violation fails its check and the level, and exits 1 with LEVEL_NOT_MET.
    let must = harness.check_config(&json!({"contractVersionSays": "1.2"}));
    let (code, passed, results) = verdicts(&must);
    assert_eq!((code, passed), (Some(1), json!(false)));
    let failing: Vec<&str> = results.iter().filter(|(_, result)| result == "fail").map(|(id, _)| id.as_str()).collect();
    assert_eq!(failing, ["L0.contract-version"]);
    assert_eq!(must.envelope()["error"]["code"], "LEVEL_NOT_MET");
    // A SHOULD violation is an advisory: the level is met and the exit is 0.
    let should = harness.check_config(&json!({"versionFormat": "bare"}));
    let (code, passed, results) = verdicts(&should);
    assert_eq!((code, passed), (Some(0), json!(true)), "{}", should.stdout);
    assert!(results.contains(&("L0.version-format".to_string(), "fail".to_string())));
    assert!(should.check("L0.version-format")["findings"].as_array().is_some_and(|findings| !findings.is_empty()));
    // A missing fixture entry is unprovided and fails the level.
    let mut fixture = harness.fixture(&json!({}));
    fixture.as_object_mut().expect("object").remove("commands");
    fixture.as_object_mut().expect("object").remove("probe");
    let lacking = harness.check(&fixture, true);
    assert_eq!(lacking.code, Some(1), "{}", lacking.stdout);
    assert_eq!(lacking.check("L0.page")["result"], "unprovided");
}
