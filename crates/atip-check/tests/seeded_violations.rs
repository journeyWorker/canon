//! approaches/checker.md 7.2: a check that never fails proves nothing, so every L0 check is run against a fake
//! tool with one planted violation of its rule. The named test fails the check and leaves the other MUST checks passing.
//! Each test carries the id of the scenario it proves.

mod support;

use serde_json::json;

use atip_check::app::check_binary_forcing;
use atip_check::cli::{CheckArgs, Level};
use support::{seeded, seeded_for, seeded_verdict, Harness, FAKE_TOOL};

/// The seeded violation of `id`: `atip check` exits as the strength says, fails that check and no other MUST check.
fn proves(id: &str) {
    let seeded = seeded_for(id);
    let harness = Harness::new();
    let output = harness.check_config(&seeded.config);
    let document = output.envelope();
    seeded_verdict(&document["data"], &seeded).unwrap_or_else(|why| panic!("{why}\n{}", output.stdout));
    let must = document["data"]["checks"].as_array().expect("checks").iter().any(|check| check["id"] == id && check["strength"] == "MUST");
    if must {
        assert_eq!((output.code, document["ok"].as_bool(), document["error"]["code"].as_str()), (Some(1), Some(false), Some("LEVEL_NOT_MET")), "{}", output.stdout);
        assert_eq!(document["data"]["passed"], false);
        assert!(document["error"]["message"].as_str().unwrap_or_default().contains(id), "the error names the failing check: {}", document["error"]["message"]);
    } else {
        assert_eq!((output.code, document["ok"].as_bool(), document["data"]["passed"].as_bool()), (Some(0), Some(true), Some(true)), "a SHOULD failure is an advisory: {}", output.stdout);
    }
}

#[test]
fn atip_check_07_l0_version_fails_when_version_exits_nonzero_prints_nothing_or_writes_to_stderr() {
    proves("L0.version");
}

#[test]
fn atip_check_08_l0_version_format_reports_an_advisory_when_version_is_not_one_line_of_tool_and_version() {
    proves("L0.version-format");
}

#[test]
fn atip_check_09_l0_unknown_command_fails_when_the_unknown_command_envelope_also_goes_to_stderr() {
    proves("L0.unknown-command");
}

#[test]
fn atip_check_10_l0_unknown_command_text_fails_when_the_text_mode_error_goes_to_stdout() {
    proves("L0.unknown-command-text");
}

#[test]
fn atip_check_11_l0_text_error_form_reports_an_advisory_when_stderr_does_not_start_with_the_tool_and_code() {
    proves("L0.text-error-form");
}

#[test]
fn atip_check_12_l0_bare_fails_when_a_call_with_no_command_is_not_command_required() {
    proves("L0.bare");
}

#[test]
fn atip_check_13_l0_unknown_option_fails_when_the_unknown_option_envelope_leaves_out_the_field() {
    proves("L0.unknown-option");
    // The envelope's other parts are judged too: the command it names, and a registered exit-2 code.
    let harness = Harness::new();
    for (config, needle) in [(json!({"unknownOptionOmitsCommand": true}), "meta.command"), (json!({"unknownOptionOmitsField": true}), "error.field")] {
        let output = harness.check_config(&config);
        let check = output.envelope()["data"]["checks"].as_array().expect("checks").iter().find(|check| check["id"] == "L0.unknown-option").cloned().expect("check");
        assert_eq!(check["result"], "fail", "{config}");
        assert!(check["reason"].as_str().unwrap_or_default().contains(needle), "{config}: {check}");
    }
    let mut fixture = harness.fixture(&json!({}));
    fixture["registry"][0]["exit"] = json!(1);
    let output = harness.check(&fixture, true);
    let check = output.envelope()["data"]["checks"].as_array().expect("checks").iter().find(|check| check["id"] == "L0.unknown-option").cloned().expect("check");
    assert_eq!(check["result"], "fail", "a USAGE registered with exit 1 is not a registered exit-2 code: {check}");
}

#[test]
fn atip_check_14_l0_json_mode_preparse_fails_when_json_after_an_unknown_option_does_not_select_json() {
    proves("L0.json-mode-preparse");
}

#[test]
fn atip_check_15_l0_after_dashdash_fails_when_json_after_dashdash_still_selects_json() {
    proves("L0.after-dashdash");
}

#[test]
fn atip_check_16_l0_ok_exit_fails_when_an_unknown_option_exits_one_or_a_failure_disagrees_with_the_registry() {
    proves("L0.ok-exit");
    // The other ways to break the rule, each failing this check alone.
    let harness = Harness::new();
    for (config, needle) in [
        (json!({"exitOutOfRange": true}), "outside 0, 1 and 2"),
        (json!({"retryableMismatch": true}), "`retryable` true, the registry says false"),
        (json!({"dataOnRefusal": true}), "carries `data`"),
        (json!({"refusalWrites": true}), "./refused.txt"),
    ] {
        let output = harness.check_config(&config);
        let data = &output.envelope()["data"];
        let seeded = support::Seeded { id: "L0.ok-exit", config: config.clone(), collateral: &["L0.unknown-command"] };
        let check = data["checks"].as_array().expect("checks").iter().find(|check| check["id"] == "L0.ok-exit").cloned().expect("check");
        assert_eq!(check["result"], "fail", "{config}: {check}");
        assert!(check["reason"].as_str().unwrap_or_default().contains(needle), "{config}: {check}");
        let others: Vec<&str> = data["checks"].as_array().expect("checks").iter().filter(|other| other["strength"] == "MUST" && other["id"] != "L0.ok-exit" && other["result"] != "pass").map(|other| other["id"].as_str().unwrap_or_default()).collect();
        // `dataOnRefusal` also puts `data` into the unknown-command envelope, which that check states too.
        assert!(others.iter().all(|id| seeded.collateral.contains(id)), "{config}: other MUST checks fail: {others:?}");
    }
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
fn atip_check_19_l0_one_document_fails_when_the_probe_prints_two_json_documents() {
    proves("L0.one-document");
}

#[test]
fn atip_check_20_l0_no_env_switch_fails_when_an_environment_variable_or_a_terminal_turns_json_into_text() {
    proves("L0.no-env-switch");
    // The terminal alone: a tool that drops JSON when stdout is a terminal.
    let harness = Harness::new();
    let output = harness.check_config(&json!({"ttySwitchesJson": true}));
    let check = output.envelope()["data"]["checks"].as_array().expect("checks").iter().find(|check| check["id"] == "L0.no-env-switch").cloned().expect("check");
    assert_eq!(check["result"], "fail", "{check}");
    assert!(check["reason"].as_str().unwrap_or_default().contains("stdout on a terminal"), "{check}");
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
fn atip_check_36_a_check_forced_to_always_pass_makes_its_seeded_violation_test_fail() {
    assert_eq!(seeded().len(), 16, "a seeded violation for every L0 check");
    let mut noticed = Vec::new();
    std::thread::scope(|scope| {
        let runs: Vec<_> = seeded()
            .into_iter()
            .map(|seeded| {
                scope.spawn(move || {
                    let harness = Harness::new();
                    let fixture_path = harness.write("fixture.json", &harness.fixture(&seeded.config));
                    let args = CheckArgs { bin: FAKE_TOOL.into(), level: Level::L0, fixture: fixture_path.into() };
                    // Unforced, the seeded test's judgement holds; with the check forced to pass, it must not.
                    let real = serde_json::to_value(check_binary_forcing(&args, &[]).expect("a report")).expect("json");
                    let forced = serde_json::to_value(check_binary_forcing(&args, &[seeded.id]).expect("a report")).expect("json");
                    (seeded.id, seeded_verdict(&real, &seeded), seeded_verdict(&forced, &seeded))
                })
            })
            .collect();
        for run in runs {
            noticed.push(run.join().expect("run"));
        }
    });
    for (id, real, forced) in noticed {
        assert_eq!(real, Ok(()), "{id}: the seeded violation is judged as planned");
        let why = forced.expect_err(&format!("{id}: forcing the check to pass must make its seeded test fail"));
        assert!(why.contains(id) && why.contains("expected fail"), "{id}: {why}");
    }
}
