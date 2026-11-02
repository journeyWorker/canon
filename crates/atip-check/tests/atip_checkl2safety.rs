//! `atip check --level L2`, the safety rows (approaches/checker.md 4, 6, 7.2): the repeat of a write, the rules of a destructive
//! command and its plan, work that is accepted and not done, operation keys, and the effects of what must change nothing. The
//! conforming L2 fake passes, and every pass condition of the rows, planted alone in the fake tool, fails the check that owns it
//! and no other. Each test carries the id of the scenario it proves (`atip.checkl2safety.NN`).

mod support;

use serde_json::{json, Value};

use support::l2::{check_of, covers, l2_config, l2_standard_config, not_passing, proves, run_case, run_config, safety_cases, verdict};
use support::{Case, Harness};

const SAFETY_ROWS: [&str; 10] = [
    "L2.idempotency",
    "L2.destructive-safety",
    "L2.destructive-preview",
    "L2.destructive-preview-stdin",
    "L2.destructive-stale",
    "L2.destructive-half",
    "L2.destructive-commit",
    "L2.async",
    "L2.operation-key",
    "L2.effects",
];

fn case_of(condition: &str) -> Case {
    safety_cases().into_iter().find(|case| case.condition == condition).unwrap_or_else(|| panic!("no case {condition}"))
}

fn argvs(check: &Value) -> Vec<String> {
    check["observed"].as_array().into_iter().flatten().map(|call| call["argv"].as_array().map_or(String::new(), |argv| argv.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "))).collect()
}

#[test]
fn atip_checkl2safety_01_a_conforming_destructive_preview_names_the_plan_and_changes_nothing_and_the_stdin_preview_replays() {
    let output = run_config(&l2_standard_config());
    let document = output.envelope();
    assert_eq!((output.code, document["ok"].as_bool()), (Some(0), Some(true)), "{}\n{}", not_passing(&document["data"]), output.stdout);
    for row in SAFETY_ROWS {
        let check = check_of(&document["data"], row);
        assert_eq!(check["result"], "pass", "{row}: {check}");
    }
    let preview = argvs(check_of(&document["data"], "L2.destructive-preview"));
    assert!(preview.iter().any(|argv| argv == "purge --kind session --before 2030-01-01 --json"), "{preview:?}");
    let stdin = argvs(check_of(&document["data"], "L2.destructive-preview-stdin"));
    assert!(stdin.iter().any(|argv| argv.contains("--input=-")) && stdin.iter().any(|argv| argv.contains("--commit --plan-digest sha256:")), "{stdin:?}");
}

#[test]
fn atip_checkl2safety_02_a_preview_that_writes_fails_only_l2_destructive_preview_and_a_commit_that_is_not_flag_only_fails_l2_destructive_safety() {
    proves(safety_cases(), &["destructive-safety", "destructive-preview", "destructive-preview-stdin"]);
    // The change a preview made is not a refusal's: L0.refusal-effects is untouched.
    let output = run_case(&case_of("destructive-preview/preview-changes-nothing"), "L2");
    let data = &output.envelope()["data"];
    assert_eq!(check_of(data, "L0.refusal-effects")["result"], "pass", "{}", not_passing(data));
    let reason = check_of(data, "L2.destructive-preview")["reason"].as_str().unwrap_or_default().to_string();
    assert!(reason.contains(".purge-preview-cache"), "the report names the changed path: {reason}");
}

#[test]
fn atip_checkl2safety_03_a_one_character_digest_is_plan_stale_with_the_preview_next_and_the_right_commit_commits() {
    proves(safety_cases(), &["destructive-commit"]);
    let output = run_config(&l2_standard_config());
    let data = &output.envelope()["data"];
    let stale = argvs(check_of(data, "L2.destructive-stale"));
    assert!(stale.iter().any(|argv| argv.starts_with("purge --kind session --before 2030-01-01 --commit --plan-digest sha256:")), "{stale:?}");
    let commit = check_of(data, "L2.destructive-commit");
    assert_eq!(commit["result"], "pass");
    assert!(argvs(commit).iter().any(|argv| argv.contains("--commit --plan-digest")), "{commit}");
}

#[test]
fn atip_checkl2safety_04_a_stale_digest_accepted_or_a_commit_alone_that_commits_fails_its_row_and_the_pairs_own_code_passes() {
    proves(safety_cases(), &["destructive-stale", "destructive-half"]);
    let case = Case { condition: "destructive-half/half-refused", config: l2_config(&json!({"pairCode": true})), tweak: None };
    let output = run_case(&case, "L2");
    assert_eq!(output.code, Some(0), "{}\n{}", not_passing(&output.envelope()["data"]), output.stdout);
    assert_eq!(check_of(&output.envelope()["data"], "L2.destructive-half")["result"], "pass");
}

#[test]
fn atip_checkl2safety_05_the_three_idempotency_values_pass_and_a_second_run_with_a_new_effect_or_an_accepted_once_fails() {
    proves(safety_cases(), &["idempotency"]);
    let output = run_config(&l2_standard_config());
    let document = output.envelope();
    let observed = argvs(check_of(&document["data"], "L2.idempotency"));
    for command in ["init", "seal", "stamp hello", "record put"] {
        assert!(observed.iter().any(|argv| argv.starts_with(command)), "{command} is run twice: {observed:?}");
    }
    assert!(!observed.iter().any(|argv| argv.starts_with("log ")), "`appends` is not tested: {observed:?}");
}

#[test]
fn atip_checkl2safety_06_a_write_the_fixture_does_not_list_is_named_and_never_run() {
    let config = l2_standard_config();
    let harness = Harness::new();
    let mut fixture = harness.l2_fixture(&config);
    fixture["commands"].as_object_mut().expect("commands").remove("seal");
    let output = harness.check_l2(&config, &fixture);
    let document = output.envelope();
    let check = check_of(&document["data"], "L2.idempotency");
    assert!(check["unlisted"].as_array().is_some_and(|names| names.iter().any(|name| name == "seal")), "{check}");
    assert!(!argvs(check).iter().any(|argv| argv.starts_with("seal")), "{check}");
    assert_eq!(check["result"], "pass");
}

#[test]
fn atip_checkl2safety_07_a_dry_run_or_help_or_version_that_writes_fails_l2_effects_and_an_allowed_read_change_passes() {
    proves(safety_cases(), &["effects"]);
    let output = run_config(&l2_standard_config());
    let document = output.envelope();
    let check = check_of(&document["data"], "L2.effects");
    assert_eq!(check["result"], "pass", "{check}");
    assert!(argvs(check).iter().any(|argv| argv == "peek --json"), "the read with an allowed change is run: {check}");
    assert_eq!(check["strength"], "MUST");
}

#[test]
fn atip_checkl2safety_08_at_l0_and_l1_l2_effects_is_an_advisory_and_a_failure_leaves_the_level_met() {
    for (level, config) in [("L0", l2_standard_config()), ("L1", l2_standard_config())] {
        for knob in ["helpWrites", "versionWrites"] {
            let case = Case { condition: "effects/help-changes-nothing", config: l2_config(&json!({ knob: true, "treeLevel": level })), tweak: None };
            let _ = &config;
            let output = run_case(&case, level);
            let document = output.envelope();
            let check = check_of(&document["data"], "L2.effects");
            assert_eq!((check["result"].as_str(), check["strength"].as_str()), (Some("fail"), Some("SHOULD")), "{level} {knob}: {check}");
            assert_eq!((output.code, document["ok"].as_bool()), (Some(0), Some(true)), "{level} {knob}: {}\n{}", not_passing(&document["data"]), output.stdout);
            assert_eq!(document["data"]["checks"].as_array().map(Vec::len), Some(if level == "L0" { 29 } else { 61 }), "{level}");
        }
    }
}

#[test]
fn atip_checkl2safety_09_a_read_change_outside_allowed_effects_fails_and_the_report_names_the_path() {
    let output = run_case(&case_of("effects/read-changes-only-allowed"), "L2");
    let data = &output.envelope()["data"];
    verdict(data, "L2.effects").unwrap_or_else(|why| panic!("{why}"));
    assert!(check_of(data, "L2.effects")["reason"].as_str().unwrap_or_default().contains("peek-cache.txt"), "{}", check_of(data, "L2.effects"));
    // A note is needed too: with no `effectNote` even the listed path is a failure.
    let case = Case { condition: "effects/read-changes-only-allowed", config: l2_config(&support::l2::patch(&[("peek", vec![support::l2::remove("/effectNote")])])), tweak: None };
    let output = run_case(&case, "L2");
    let reason = check_of(&output.envelope()["data"], "L2.effects")["reason"].as_str().unwrap_or_default().to_string();
    assert!(reason.contains(".peek-access.log") && reason.contains("no `effectNote`"), "{reason}");
}

#[test]
fn atip_checkl2safety_10_the_worker_is_driven_to_done_failed_and_timeout_and_the_wait_exits_as_the_contract_says() {
    proves(safety_cases(), &["async"]);
    let output = run_config(&l2_standard_config());
    let document = output.envelope();
    let check = check_of(&document["data"], "L2.async");
    assert_eq!(check["result"], "pass", "{check}");
    let observed = argvs(check);
    assert!(observed.iter().any(|argv| argv.starts_with("deploy start staging")), "{observed:?}");
    assert!(observed.iter().filter(|argv| argv.starts_with("deploy status") && argv.contains("--wait")).count() >= 3, "{observed:?}");
    assert!(observed.iter().any(|argv| argv.contains("--timeout 1s")), "a wait that hangs is given a short time: {observed:?}");
}

#[test]
fn atip_checkl2safety_11_the_same_operation_key_returns_the_same_work_and_another_payload_is_operation_reused() {
    proves(safety_cases(), &["operation-key"]);
    let output = run_config(&l2_standard_config());
    let document = output.envelope();
    let observed = argvs(check_of(&document["data"], "L2.operation-key"));
    assert!(observed.iter().filter(|argv| argv.contains("--operation chk-one")).count() >= 3, "{observed:?}");
}

#[test]
fn atip_checkl2safety_12_a_tool_without_destructive_or_async_commands_has_those_rows_unsupported_and_a_start_that_claims_done_fails_async() {
    let harness = Harness::new();
    let config = support::l1::l1_standard_config();
    let fixture = harness.l1_fixture(&config);
    let output = harness.check_l2_at(&config, &fixture, "L2");
    let data = &output.envelope()["data"];
    for row in ["L2.destructive-safety", "L2.destructive-preview", "L2.destructive-preview-stdin", "L2.destructive-stale", "L2.destructive-half", "L2.destructive-commit", "L2.async", "L2.operation-key"] {
        assert_eq!(check_of(data, row)["result"], "unsupported", "{row}: {}", check_of(data, row));
    }
    let output = run_case(&case_of("async/start-is-not-complete"), "L2");
    verdict(&output.envelope()["data"], "L2.async").unwrap_or_else(|why| panic!("{why}"));
}

#[test]
fn atip_checkl2safety_13_every_condition_of_the_safety_rows_has_a_case_and_the_conforming_tool_judges_each() {
    covers(
        safety_cases(),
        &["idempotency", "destructive-safety", "destructive-preview", "destructive-preview-stdin", "destructive-stale", "destructive-half", "destructive-commit", "async", "operation-key", "effects"],
        &[],
    );
}

#[test]
fn atip_checkl2safety_14_the_conforming_fake_and_the_checker_itself_pass() {
    let output = run_config(&l2_standard_config());
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    let harness = Harness::new();
    let fixture = harness.path("fixture-copy.json");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("atip.fixture.json");
    std::fs::copy(&source, &fixture).expect("copy the fixture");
    let work = harness.path("cwd");
    std::fs::create_dir_all(&work).expect("cwd");
    std::fs::copy(&source, work.join("atip.fixture.json")).expect("copy next to the invocations");
    let tmp = harness.path("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let output = support::atip_in(&work, &tmp, &["check", "--bin", support::ATIP, "--level", "L0", "--fixture", fixture.to_str().expect("utf8"), "--json"]);
    assert_eq!((output.code, output.envelope()["data"]["passed"].as_bool()), (Some(0), Some(true)), "{}", output.stdout);
}
