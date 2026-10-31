//! approaches/checker.md 7.2: a check that never fails proves nothing, so every pass condition of every L0 check
//! is run against a fake tool with one planted violation of it. The tests require that the check that owns the
//! condition fails and that no other check does (one owner per pass condition), and that the same run with the
//! condition, or the whole check, switched off no longer fails: so a test that cannot notice a missing condition
//! cannot pass. Each test carries the id of the scenario it proves.

mod support;

use std::collections::BTreeSet;

use serde_json::{json, Value};

use atip_check::app::check_binary_mutated;
use atip_check::cli::{CheckArgs, Level};
use support::{case_verdict, cases, cases_of, Case, Harness, FAKE_TOOL};

/// Every case of the check `id`: `atip check` exits as the strength says, fails that check and no other.
fn proves(id: &str) {
    let owned = cases_of(id);
    assert!(
        !owned.is_empty(),
        "{id} has a seeded violation for each of its conditions"
    );
    for case in owned {
        let harness = Harness::new();
        let output = harness.check(&case.fixture(&harness), true);
        let document = output.envelope();
        case_verdict(&document["data"], id)
            .unwrap_or_else(|why| panic!("{}: {why}\n{}", case.condition, output.stdout));
        let must = document["data"]["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .any(|check| check["id"] == id && check["strength"] == "MUST");
        if must {
            assert_eq!(
                (
                    output.code,
                    document["ok"].as_bool(),
                    document["error"]["code"].as_str()
                ),
                (Some(1), Some(false), Some("LEVEL_NOT_MET")),
                "{}: {}",
                case.condition,
                output.stdout
            );
            assert_eq!(document["data"]["passed"], false);
            assert!(
                document["error"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains(id),
                "the error names the failing check: {}",
                document["error"]["message"]
            );
        } else {
            assert_eq!(
                (
                    output.code,
                    document["ok"].as_bool(),
                    document["data"]["passed"].as_bool()
                ),
                (Some(0), Some(true), Some(true)),
                "{}: a SHOULD failure is an advisory: {}",
                case.condition,
                output.stdout
            );
        }
    }
}

#[test]
fn atip_check_07_l0_version_fails_when_version_exits_nonzero_or_prints_nothing() {
    proves("L0.version");
}

#[test]
fn atip_check_08_l0_version_format_reports_an_advisory_when_version_is_not_one_line_of_tool_and_version(
) {
    proves("L0.version-format");
}

#[test]
fn atip_check_09_l0_unknown_command_fails_on_each_part_of_the_unknown_command_envelope() {
    proves("L0.unknown-command");
}

#[test]
fn atip_check_10_l0_unknown_command_text_fails_on_the_exit_or_either_stream_of_the_text_mode_error()
{
    proves("L0.unknown-command-text");
}

#[test]
fn atip_check_11_l0_text_error_form_reports_an_advisory_when_stderr_does_not_start_with_the_tool_and_code(
) {
    proves("L0.text-error-form");
}

#[test]
fn atip_check_12_l0_bare_fails_when_a_call_with_no_command_is_not_a_command_required_envelope() {
    proves("L0.bare");
}

#[test]
fn atip_check_13_l0_unknown_option_fails_on_the_code_the_field_or_the_command_of_the_envelope() {
    proves("L0.unknown-option");
    // A USAGE registered with exit 1 is not a refusal code either.
    let harness = Harness::new();
    let mut fixture = harness.fixture(&json!({}));
    fixture["registry"][0]["exit"] = json!(1);
    let output = harness.check(&fixture, true);
    let check = output.envelope()["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["id"] == "L0.unknown-option")
        .cloned()
        .expect("check");
    assert_eq!(check["result"], "fail", "{check}");
}

#[test]
fn atip_check_14_l0_json_mode_preparse_fails_when_json_before_or_after_an_unknown_option_does_not_select_json(
) {
    proves("L0.json-mode-preparse");
}

#[test]
fn atip_check_15_l0_after_dashdash_fails_when_json_after_dashdash_still_selects_json() {
    proves("L0.after-dashdash");
}

#[test]
fn atip_check_16_l0_ok_exit_fails_on_an_exit_outside_0_to_2_or_a_failure_that_disagrees_with_the_registry(
) {
    proves("L0.ok-exit");
}

#[test]
fn atip_check_17_l0_no_null_fails_when_an_error_carries_hint_null() {
    proves("L0.no-null");
}

#[test]
fn atip_check_18_l0_compact_reports_an_advisory_when_the_envelope_is_indented() {
    proves("L0.compact");
}

#[test]
fn atip_check_19_l0_one_document_fails_when_stdout_is_not_exactly_one_document_and_one_newline() {
    proves("L0.one-document");
    // The unknown command printing two documents, and the bare call no newline: the same rule on the other calls it owns.
    let harness = Harness::new();
    for config in [
        json!({"unknownCommandTwoDocuments": true}),
        json!({"bareNoNewline": true}),
    ] {
        let output = harness.check_config(&config);
        case_verdict(&output.envelope()["data"], "L0.one-document")
            .unwrap_or_else(|why| panic!("{config}: {why}"));
    }
}

#[test]
fn atip_check_20_l0_no_env_switch_fails_when_the_environment_a_terminal_or_a_wait_for_input_changes_the_probe(
) {
    proves("L0.no-env-switch");
    // The terminal alone: a tool that drops JSON when stdout is a terminal.
    let harness = Harness::new();
    let output = harness.check_config(&json!({"ttySwitchesJson": true}));
    case_verdict(&output.envelope()["data"], "L0.no-env-switch").expect("only no-env-switch fails");
    let check = output.envelope()["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["id"] == "L0.no-env-switch")
        .cloned()
        .expect("check");
    assert!(
        check["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("stdout on a terminal"),
        "{check}"
    );
}

#[test]
fn atip_check_21_l0_no_env_switch_text_fails_when_ci_turns_text_mode_into_json() {
    proves("L0.no-env-switch-text");
}

#[test]
fn atip_check_22_l0_contract_reports_an_advisory_when_meta_contract_is_missing() {
    proves("L0.contract");
}

#[test]
fn atip_check_30_l0_refusal_effects_fails_when_a_refusal_writes_in_the_working_directory_or_in_home(
) {
    proves("L0.refusal-effects");
    let harness = Harness::new();
    let output = harness.check_config(&json!({"refusalWritesHome": true}));
    case_verdict(&output.envelope()["data"], "L0.refusal-effects")
        .expect("only refusal-effects fails");
    let check = output.envelope()["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["id"] == "L0.refusal-effects")
        .cloned()
        .expect("check");
    assert!(
        check["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("~/telemetry.log"),
        "{check}"
    );
}

#[test]
fn atip_check_44_l0_next_fails_on_an_empty_argv_a_placeholder_a_missing_json_flag_or_a_command_that_is_not_its_argv(
) {
    proves("L0.next");
}

#[test]
fn atip_check_45_l0_page_fails_on_a_malformed_page_or_a_next_cursor_with_no_cursor_step() {
    proves("L0.page");
}

/// The check's id, the verdict on the planted violation and the verdict with the check forced to pass.
type Forced = (String, Result<(), String>, Result<(), String>);

fn args_for(harness: &Harness, fixture: &Value) -> CheckArgs {
    CheckArgs {
        bin: FAKE_TOOL.into(),
        level: Level::L0,
        fixture: harness.write("fixture.json", fixture).into(),
    }
}

/// Runs `jobs` on at most six threads and returns their results in order.
fn in_parallel<T: Send>(jobs: Vec<Box<dyn FnOnce() -> T + Send>>) -> Vec<T> {
    let queue = std::sync::Mutex::new(jobs.into_iter().enumerate().collect::<Vec<_>>());
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| loop {
                let next = queue.lock().expect("queue").pop();
                let Some((index, job)) = next else { break };
                let value = job();
                results.lock().expect("results").push((index, value));
            });
        }
    });
    let mut results = results.into_inner().expect("results");
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, value)| value).collect()
}

#[test]
fn atip_check_56_each_pass_condition_fails_exactly_its_owner_and_a_run_with_the_condition_switched_off_does_not(
) {
    // Every condition a conforming run judges has a case, and every case names a condition that is judged.
    let harness = Harness::new();
    let args = args_for(
        &harness,
        &harness.fixture(&json!({"offersSchemaErrors": true})),
    );
    let (report, judged): (_, BTreeSet<String>) =
        check_binary_mutated(&args, &[], &[]).expect("a report");
    assert!(report.passed, "{}", report.text());
    let named: BTreeSet<String> = cases()
        .iter()
        .map(|case| case.condition.to_string())
        .collect();
    assert_eq!(
        judged, named,
        "the conditions the checker judges and the conditions the tests plant are the same set"
    );
    // For each case: planted, its owner fails and nothing else; skipped, the owner no longer fails (the other conditions of the check hold).
    let jobs: Vec<Box<dyn FnOnce() -> Result<(), String> + Send>> = cases()
        .into_iter()
        .map(
            |case: Case| -> Box<dyn FnOnce() -> Result<(), String> + Send> {
                Box::new(move || {
                    let harness = Harness::new();
                    let args = args_for(&harness, &case.fixture(&harness));
                    let owner = case.owner();
                    let (planted, _) = check_binary_mutated(&args, &[], &[])
                        .map_err(|failure| format!("{}: {}", case.condition, failure.message))?;
                    case_verdict(&serde_json::to_value(&planted).expect("json"), &owner)
                        .map_err(|why| format!("{}: {why}", case.condition))?;
                    let (skipped, _) = check_binary_mutated(&args, &[], &[case.condition])
                        .map_err(|failure| format!("{}: {}", case.condition, failure.message))?;
                    let owner_result = skipped.find(&owner).map(|check| check.result);
                    if owner_result == Some(atip_check::report::CheckResult::Fail) {
                        return Err(format!(
                            "{}: with the condition switched off {owner} still fails: {:?}",
                            case.condition,
                            skipped.find(&owner).and_then(|check| check.reason.clone())
                        ));
                    }
                    if !skipped
                        .checks
                        .iter()
                        .all(|check| check.result != atip_check::report::CheckResult::Fail)
                    {
                        return Err(format!(
                            "{}: with the condition switched off something else fails: {}",
                            case.condition,
                            skipped.text()
                        ));
                    }
                    Ok(())
                })
            },
        )
        .collect();
    let failures: Vec<String> = in_parallel(jobs)
        .into_iter()
        .filter_map(Result::err)
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn atip_check_36_a_check_forced_to_always_pass_makes_its_seeded_violation_test_fail() {
    let owners: Vec<String> = [
        "L0.version",
        "L0.version-format",
        "L0.unknown-command",
        "L0.unknown-command-text",
        "L0.text-error-form",
        "L0.bare",
        "L0.unknown-option",
        "L0.json-mode-preparse",
        "L0.after-dashdash",
        "L0.ok-exit",
        "L0.refusal-effects",
        "L0.no-null",
        "L0.compact",
        "L0.one-document",
        "L0.no-env-switch",
        "L0.no-env-switch-text",
        "L0.next",
        "L0.page",
        "L0.contract",
    ]
    .map(String::from)
    .to_vec();
    assert_eq!(
        owners
            .iter()
            .filter(|owner| cases_of(owner).is_empty())
            .count(),
        0,
        "a seeded violation for every L0 check"
    );
    let jobs: Vec<Box<dyn FnOnce() -> Forced + Send>> = owners
        .into_iter()
        .map(|owner| -> Box<dyn FnOnce() -> Forced + Send> {
            Box::new(move || {
                let case = cases_of(&owner).into_iter().next().expect("a case");
                let harness = Harness::new();
                let args = args_for(&harness, &case.fixture(&harness));
                let (real, _) = check_binary_mutated(&args, &[], &[]).expect("a report");
                let (forced, _) =
                    check_binary_mutated(&args, &[owner.as_str()], &[]).expect("a report");
                let verdict = |report: &atip_check::report::Report| {
                    case_verdict(&serde_json::to_value(report).expect("json"), &owner)
                };
                (owner.clone(), verdict(&real), verdict(&forced))
            })
        })
        .collect();
    for (owner, real, forced) in in_parallel(jobs) {
        assert_eq!(
            real,
            Ok(()),
            "{owner}: the seeded violation is judged as planned"
        );
        let why = forced.expect_err(&format!(
            "{owner}: forcing the check to pass must make its seeded test fail"
        ));
        assert!(
            why.contains(&owner) && why.contains("expected fail"),
            "{owner}: {why}"
        );
    }
}
