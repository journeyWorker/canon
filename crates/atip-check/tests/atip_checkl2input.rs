//! `atip check --level L2`, the `--input` and batch rows (approaches/checker.md 6, 7.2): the conforming L2 fake passes, and
//! every pass condition of the 15 rows, planted alone in the fake tool, fails the check that owns it and no other. Each test
//! carries the id of the scenario it proves (`atip.checkl2input.NN`).

mod support;

use serde_json::{json, Value};

use support::l2::{check_of, covers, input_cases, l2_config, l2_standard_config, not_passing, proves, run_case, run_config};
use support::{Case, Harness};

const INPUT_ROWS: [&str; 15] = [
    "L2.input-present",
    "L2.input-example",
    "L2.input-refusals",
    "L2.input-tty",
    "L2.input-usage",
    "L2.input-reasons",
    "L2.input-fragment",
    "L2.input-normalise",
    "L2.input-conflict",
    "L2.input-env",
    "L2.input-pointer",
    "L2.input-dash-values",
    "L2.batch-preflight",
    "L2.batch-incomplete",
    "L2.batch-ok",
];

fn case_of(condition: &str) -> Case {
    input_cases().into_iter().find(|case| case.condition == condition).unwrap_or_else(|| panic!("no case {condition}"))
}

fn results(data: &Value, ids: &[&str]) -> Vec<(String, String)> {
    ids.iter().map(|id| ((*id).to_string(), check_of(data, id)["result"].as_str().unwrap_or_default().to_string())).collect()
}

#[test]
fn atip_checkl2input_01_level_l2_runs_the_input_and_batch_rows_and_a_conforming_tool_meets_the_level() {
    let output = run_config(&l2_standard_config());
    let document = output.envelope();
    assert_eq!((output.code, document["ok"].as_bool()), (Some(0), Some(true)), "{}\n{}", not_passing(&document["data"]), output.stdout);
    assert_eq!(document["data"]["level"], "L2");
    for row in INPUT_ROWS {
        assert_eq!(check_of(&document["data"], row)["result"], "pass", "{row}: {}", not_passing(&document["data"]));
    }
    let harness = Harness::new();
    let config = l2_standard_config();
    let fixture = harness.l2_fixture(&config);
    let output = harness.check_l2_at(&config, &fixture, "L3");
    assert_eq!((output.code, output.envelope()["error"]["code"].as_str()), (Some(2), Some("USAGE")), "{}", output.stdout);
}

#[test]
fn atip_checkl2input_02_a_tree_whose_level_is_l1_fails_l1_tree_at_l2_and_a_write_without_input_fails_l2_input_present() {
    // The L1 fake prints a tree whose level is L1: at L2 its level is not met.
    let harness = Harness::new();
    let config = support::l1::l1_standard_config();
    let fixture = harness.l1_fixture(&config);
    let output = harness.check_l2_at(&config, &fixture, "L2");
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str()), (Some(1), Some("LEVEL_NOT_MET")), "{}", output.stdout);
    assert_eq!(check_of(&document["data"], "L1.tree")["result"], "fail", "{}", not_passing(&document["data"]));
    let present = run_case(&case_of("input-present/every-write-has-input"), "L2");
    assert_eq!(check_of(&present.envelope()["data"], "L2.input-present")["result"], "fail");
}

#[test]
fn atip_checkl2input_03_input_present_and_input_example_judge_the_documents_alone_and_each_condition_fails_only_its_owner() {
    proves(input_cases(), &["input-present", "input-example"]);
}

#[test]
fn atip_checkl2input_04_every_refusal_document_is_refused_with_exit_2_and_a_pointer_the_two_key_document_with_two_details() {
    proves(input_cases(), &["input-refusals"]);
    let output = run_config(&l2_standard_config());
    let refusals = check_of(&output.envelope()["data"], "L2.input-refusals").clone();
    let observed = refusals["observed"].as_array().map_or(0, Vec::len);
    assert!(observed > 5, "the refusal probes are observed: {refusals}");
}

#[test]
fn atip_checkl2input_05_a_tool_that_accepts_a_flag_only_key_in_a_document_fails_only_l2_input_refusals() {
    for key in ["commit", "limit"] {
        let case = Case { condition: "input-refusals/refused", config: l2_config(&json!({"inputAccepts": [key]})), tweak: None };
        let output = run_case(&case, "L2");
        support::l2::verdict(&output.envelope()["data"], "L2.input-refusals").unwrap_or_else(|why| panic!("{key}: {why}\n{}", not_passing(&output.envelope()["data"])));
    }
}

#[test]
fn atip_checkl2input_06_stdin_on_a_terminal_is_refused_on_input_and_each_condition_of_input_tty_fails_only_it() {
    proves(input_cases(), &["input-tty"]);
}

#[test]
fn atip_checkl2input_07_input_usage_is_a_must_for_a_1_1_claim_and_a_malformed_command_line_is_never_input_invalid() {
    proves(input_cases(), &["input-usage"]);
}

#[test]
fn atip_checkl2input_08_input_reasons_is_advisory_and_the_level_is_met() {
    proves(input_cases(), &["input-reasons"]);
    let output = run_case(&case_of("input-reasons/reserved-reason"), "L2");
    let document = output.envelope();
    assert_eq!((output.code, document["ok"].as_bool()), (Some(0), Some(true)), "{}", output.stdout);
    assert_eq!(check_of(&document["data"], "L2.input-reasons")["strength"], "SHOULD");
}

#[test]
fn atip_checkl2input_09_a_document_and_argv_split_three_ways_and_numeric_strings_give_the_argv_result() {
    proves(input_cases(), &["input-fragment", "input-normalise"]);
}

#[test]
fn atip_checkl2input_10_a_different_value_conflicts_with_the_pointer_and_the_same_value_written_another_way_succeeds() {
    proves(input_cases(), &["input-conflict"]);
}

#[test]
fn atip_checkl2input_11_an_env_value_applies_after_the_merge_only() {
    proves(input_cases(), &["input-env"]);
}

#[test]
fn atip_checkl2input_12_a_broken_constraint_or_a_missing_key_is_refused_with_the_right_code_and_a_document_pointer() {
    proves(input_cases(), &["input-pointer"]);
}

#[test]
fn atip_checkl2input_13_a_value_that_starts_with_a_dash_a_brace_or_a_bracket_is_a_value() {
    proves(input_cases(), &["input-dash-values"]);
}

#[test]
fn atip_checkl2input_14_a_number_beyond_the_safe_range_in_a_document_is_refused_and_l1_numbers_owns_it() {
    proves(input_cases(), &["numbers"]);
}

#[test]
fn atip_checkl2input_15_a_conforming_batch_tool_passes_the_three_batch_rows_with_the_counts_order_and_statuses() {
    let output = run_config(&l2_standard_config());
    let data = &output.envelope()["data"];
    for row in ["L2.batch-preflight", "L2.batch-incomplete", "L2.batch-ok"] {
        let check = check_of(data, row);
        assert_eq!(check["result"], "pass", "{row}: {check}");
        assert!(check["observed"].as_array().is_some_and(|observed| !observed.is_empty()), "{row} observed its invocations");
    }
}

#[test]
fn atip_checkl2input_16_a_first_item_failure_with_exit_2_and_a_preflight_refusal_as_batch_incomplete_each_fail_only_their_row() {
    proves(input_cases(), &["batch-preflight", "batch-incomplete", "batch-ok"]);
}

#[test]
fn atip_checkl2input_17_a_tool_with_no_batch_command_has_the_three_batch_rows_unsupported() {
    let case = Case { condition: "batch-ok/succeeded", config: l2_config(&json!({"noBatch": true})), tweak: None };
    let output = run_case(&case, "L2");
    let document = output.envelope();
    assert_eq!(output.code, Some(0), "{}\n{}", not_passing(&document["data"]), output.stdout);
    for row in ["L2.batch-preflight", "L2.batch-incomplete", "L2.batch-ok"] {
        assert_eq!(check_of(&document["data"], row)["result"], "unsupported", "{row}");
    }
}

#[test]
fn atip_checkl2input_18_a_fixture_without_batch_fails_at_or_first_makes_batch_incomplete_unprovided_and_exits_1() {
    let config = l2_standard_config();
    let harness = Harness::new();
    let mut fixture = harness.l2_fixture(&config);
    fixture["commands"]["member tag"].as_object_mut().expect("entry").remove("batchFailsAt");
    fixture["commands"]["member tag"].as_object_mut().expect("entry").remove("batchFailsFirst");
    let output = harness.check_l2(&config, &fixture);
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str()), (Some(1), Some("LEVEL_NOT_MET")), "{}", output.stdout);
    assert_eq!(check_of(&document["data"], "L2.batch-incomplete")["result"], "unprovided", "{}", not_passing(&document["data"]));
    assert!(results(&document["data"], &["L2.batch-preflight", "L2.batch-ok"]).iter().all(|(_, result)| result == "pass"));
    // An observed violation is still a failure, not unprovided.
    let violating = Harness::new();
    let broken = l2_config(&json!({"batchFirstFailPlain": true}));
    let mut fixture = violating.l2_fixture(&broken);
    fixture["commands"]["member tag"].as_object_mut().expect("entry").remove("batchFailsAt");
    let output = violating.check_l2(&broken, &fixture);
    assert_eq!(check_of(&output.envelope()["data"], "L2.batch-incomplete")["result"], "fail", "{}", output.stdout);
}

#[test]
fn atip_checkl2input_19_a_malformed_batch_fails_at_or_first_is_fixture_invalid_with_exit_2() {
    for (key, entry) in [
        ("batchFailsAt", json!([{"input": [{"operands": [3]}], "index": 4}])),
        ("batchFailsAt", json!([{"input": [], "index": 0}])),
        ("batchFailsFirst", json!([{"input": [{"operands": [3]}, {"operands": [4]}], "index": 1}])),
        ("batchFailsFirst", json!({"input": [{}], "index": 0})),
        ("batchFailsAt", json!([{"input": [{}], "index": 0, "bogus": 1}])),
    ] {
        let config = l2_standard_config();
        let harness = Harness::new();
        let mut fixture = harness.l2_fixture(&config);
        fixture["commands"]["member tag"][key] = entry.clone();
        let output = harness.check_l2(&config, &fixture);
        let document = output.envelope();
        assert_eq!((output.code, document["error"]["code"].as_str(), document["error"]["reason"].as_str()), (Some(2), Some("FIXTURE_INVALID"), Some("WRONG_SHAPE")), "{key} {entry}: {}", output.stdout);
        assert!(document["error"]["message"].as_str().unwrap_or_default().contains(key), "{}", output.stdout);
    }
}

#[test]
fn atip_checkl2input_20_a_write_the_fixture_does_not_list_is_named_unlisted_and_never_run_by_an_input_row() {
    let config = l2_standard_config();
    let harness = Harness::new();
    let mut fixture = harness.l2_fixture(&config);
    fixture["commands"].as_object_mut().expect("commands").remove("record put");
    let output = harness.check_l2(&config, &fixture);
    let data = &output.envelope()["data"];
    let refusals = check_of(data, "L2.input-refusals");
    assert!(refusals["unlisted"].as_array().is_some_and(|names| names.iter().any(|name| name == "record put")), "{refusals}");
    let calls: Vec<String> = refusals["observed"].as_array().into_iter().flatten().map(|call| call["argv"].to_string()).collect();
    assert!(calls.iter().all(|argv| !argv.contains("record")), "a command the fixture does not list is never run: {calls:?}");
}

#[test]
fn atip_checkl2input_21_every_condition_of_the_input_and_batch_rows_has_a_case_and_the_conforming_tool_judges_each() {
    covers(
        input_cases(),
        &[
            "input-present", "input-example", "input-refusals", "input-tty", "input-usage", "input-reasons", "input-fragment", "input-normalise", "input-conflict", "input-env",
            "input-pointer", "input-dash-values", "batch-preflight", "batch-incomplete", "batch-ok", "numbers",
        ],
        &[],
    );
}

#[test]
fn atip_checkl2input_22_the_conforming_fake_and_the_checker_itself_pass() {
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

#[test]
fn atip_checkl2input_15_a_rule_on_flag_only_arguments_is_never_broken_by_a_batch_item() {
    // `member tag` has --keep and --drop, flag-only, at most one: a document cannot carry either, so no item breaks the rule, and the
    // batch is not asked to refuse what the item does not break (canon's `export` with --limit and --all).
    let config = l2_config(&json!({"tagFlagOnlyRule": true}));
    let output = run_config(&config);
    let document = output.envelope();
    assert_eq!((output.code, document["ok"].as_bool()), (Some(0), Some(true)), "{}\n{}", not_passing(&document["data"]), output.stdout);
    let check = check_of(&document["data"], "L2.batch-preflight");
    assert_eq!(check["result"], "pass", "{check}");
    let calls: Vec<String> = check["observed"].as_array().into_iter().flatten().map(|call| call["argv"].to_string()).collect();
    assert!(calls.iter().all(|argv| !argv.contains("--keep") && !argv.contains("--drop")), "{calls:?}");
}

#[test]
fn atip_checkl2input_20_an_unlisted_write_never_decides_unsupported_and_leaves_the_batch_rows_unprovided() {
    let config = l2_standard_config();
    let harness = Harness::new();
    let mut fixture = harness.l2_fixture(&config);
    fixture["commands"].as_object_mut().expect("commands").remove("member tag");
    let output = harness.check_l2(&config, &fixture);
    let data = &output.envelope()["data"];
    assert_eq!(output.code, Some(1), "{}", output.stdout);
    for row in ["L2.batch-preflight", "L2.batch-incomplete", "L2.batch-ok"] {
        let check = check_of(data, row);
        assert_eq!(check["result"], "unprovided", "{row}: {check}");
        assert!(check.to_string().contains("member tag"), "{row} names the write: {check}");
    }
    // The same fixture for a tool with no batch at all: nothing to probe, and that is `unsupported`.
    let none = l2_config(&json!({"noBatch": true}));
    let mut fixture = harness.l2_fixture(&none);
    fixture["commands"].as_object_mut().expect("commands").remove("member tag");
    let output = harness.check_l2(&none, &fixture);
    for row in ["L2.batch-preflight", "L2.batch-incomplete", "L2.batch-ok"] {
        assert_eq!(check_of(&output.envelope()["data"], row)["result"], "unsupported", "{row}");
    }
    // A row whose commands are all unlisted is `unprovided`, as an input row: every write command with `input` but `init` is left out.
    let mut fixture = harness.l2_fixture(&config);
    let kept = ["items", "status", "schema", "member list", "member search", "reject", "init"];
    fixture["commands"].as_object_mut().expect("commands").retain(|id, _| kept.contains(&id.as_str()));
    let output = harness.check_l2(&config, &fixture);
    for row in ["L2.input-normalise", "L2.batch-ok"] {
        let document = output.envelope();
        let check = check_of(&document["data"], row);
        assert_ne!(check["result"], "unsupported", "{row}: {check}");
    }
}

#[test]
fn atip_checkl2input_04_the_exit_number_of_an_input_refusal_is_l0_ok_exits_alone() {
    // INPUT_INVALID answered with exit 1 where the registry says 2, the envelope right: only the owner of the exit fails.
    let output = run_config(&l2_config(&json!({"inputInvalidExit1": true})));
    let data = &output.envelope()["data"];
    support::l2::verdict(data, "L0.ok-exit").unwrap_or_else(|why| panic!("{why}\n{}", not_passing(data)));
}

#[test]
fn atip_checkl2input_04_input_too_large_is_refused_on_input_and_a_pointer_fails_only_l2_input_refusals() {
    let output = run_case(&Case { condition: "input-refusals/field", config: l2_config(&json!({"tooLargeField": "/0"})), tweak: None }, "L2");
    let data = &output.envelope()["data"];
    support::l2::verdict(data, "L2.input-refusals").unwrap_or_else(|why| panic!("{why}\n{}", not_passing(data)));
    assert!(check_of(data, "L2.input-refusals")["reason"].as_str().unwrap_or_default().contains("INPUT_TOO_LARGE") || check_of(data, "L2.input-refusals")["reason"].as_str().unwrap_or_default().contains("maxBytes"), "{}", check_of(data, "L2.input-refusals"));
}

#[test]
fn atip_checkl2input_07_a_finer_replacement_of_usage_names_an_argument_and_a_field_that_names_none_fails_l2_input_usage() {
    // Any registered exit-2 code of the tool's own whose field names an argument stands for `USAGE`; a reserved code (`PLAN_STALE`) has a situation of its own.
    let output = run_config(&l2_config(&json!({"inputUsageCode": "DEPLOYMENT_NOT_FOUND"})));
    let document = output.envelope();
    assert_eq!(check_of(&document["data"], "L2.input-usage")["result"], "pass", "{}", not_passing(&document["data"]));
    let output = run_case(&Case { condition: "input-usage/usage", config: l2_config(&json!({"inputUsageCode": "PLAN_STALE"})), tweak: None }, "L2");
    assert_eq!(check_of(&output.envelope()["data"], "L2.input-usage")["result"], "fail", "{}", not_passing(&output.envelope()["data"]));
    let output = run_case(&Case { condition: "input-usage/usage", config: l2_config(&json!({"inputUsageCode": "DEPLOYMENT_NOT_FOUND", "inputUsageField": "stdin"})), tweak: None }, "L2");
    let document = output.envelope();
    let check = check_of(&document["data"], "L2.input-usage");
    assert_eq!(check["result"], "fail", "{check}");
    assert!(check["reason"].as_str().unwrap_or_default().contains("registered finer replacement"), "{check}");
}

#[test]
fn atip_checkl2input_09_a_value_that_varies_between_two_runs_of_the_reference_is_not_compared_and_the_rest_of_data_still_is() {
    // `record put` prints a label with the nanoseconds of the run: the reference run twice on fresh state differs there, so /data/label is varying.
    let output = run_config(&l2_config(&json!({"dataStamp": true})));
    let document = output.envelope();
    for row in ["L2.input-fragment", "L2.input-normalise", "L2.input-env"] {
        assert_eq!(check_of(&document["data"], row)["result"], "pass", "{row}: {}", not_passing(&document["data"]));
    }
    // A real difference between the forms is still found.
    let output = run_case(&case_of("input-fragment/same-as-argv"), "L2");
    support::l2::verdict(&output.envelope()["data"], "L2.input-fragment").unwrap_or_else(|why| panic!("{why}"));
    // A difference in `data` alone, in a member that does not vary between two runs of the argv form (the label of the document form), fails the rows that
    // compare as L2.input-fragment does, on `/data`: the exit, `ok` and the code are the same, so only the comparison of `data` finds it, and removing
    // `/data` from the compared members lets it pass.
    let output = run_config(&l2_config(&json!({"dataFormDiffers": true})));
    let document = output.envelope();
    for row in ["L2.input-fragment", "L2.input-normalise"] {
        let check = check_of(&document["data"], row);
        assert_eq!(check["result"], "fail", "{row}: {check}");
        let reason = check["reason"].as_str().unwrap_or_default();
        assert!(reason.contains("/data:") && reason.contains("-from-document") && !reason.contains("exit ") && !reason.contains("/ok:") && !reason.contains("/error/code:"), "{row}: {reason}");
    }
}

#[test]
fn atip_checkl2input_11_only_an_option_that_is_the_sole_declarer_of_its_env_variable_is_probed_and_with_none_the_row_is_unsupported() {
    // Every option that reads the environment reads FAKE_SHARED: none is the sole declarer of its own, whatever its command.
    let output = run_config(&l2_config(&json!({"sharedEnv": true})));
    let document = output.envelope();
    let check = check_of(&document["data"], "L2.input-env");
    assert_eq!(check["result"], "unsupported", "{check}");
    assert!(check["reason"].as_str().unwrap_or_default().contains("sole declarer"), "{check}");
    // The conforming fake declares three distinct variables and the row judges them.
    let output = run_config(&l2_standard_config());
    assert_eq!(check_of(&output.envelope()["data"], "L2.input-env")["result"], "pass");
}

#[test]
fn atip_checkl2input_13_a_candidate_refused_by_a_precondition_cannot_tell_and_the_next_is_tried_and_a_refusal_naming_the_value_is_a_pass() {
    // `record put --label` refuses a dash value by a precondition (ALREADY_SEALED, exit 2): the first candidate cannot tell, `--note` decides.
    let output = run_config(&l2_config(&json!({"dashPrecondition": true})));
    let document = output.envelope();
    let check = check_of(&document["data"], "L2.input-dash-values");
    assert_eq!(check["result"], "pass", "{check}");
    let argvs: Vec<String> = check["observed"].as_array().into_iter().flatten().map(|call| call["argv"].to_string()).collect();
    assert!(argvs.iter().any(|argv| argv.contains("record") && argv.contains("note")), "the next candidate was run: {argvs:?}");
    // Refused with the argument as `field` and the value as `received`: the value was read as a value, a pass for that candidate.
    let output = run_config(&l2_config(&json!({"dashPrecondition": true, "dashPreconditionNamed": true})));
    assert_eq!(check_of(&output.envelope()["data"], "L2.input-dash-values")["result"], "pass");
    // A value read as an option is still found.
    let output = run_case(&case_of("input-dash-values/value-is-a-value"), "L2");
    support::l2::verdict(&output.envelope()["data"], "L2.input-dash-values").unwrap_or_else(|why| panic!("{why}"));
}

#[test]
fn atip_checkl2input_13_a_pointer_into_operands_names_an_operand_candidate_only() {
    // Both option candidates are refused by a precondition naming `/operands` and the value: for an option that is not its pointer, so neither
    // candidate decides and the row is unprovided (it was a pass when any `/operands` field counted).
    let config = l2_config(&json!({"dashPrecondition": true, "dashPreconditionNamed": true, "dashPreconditionBoth": true, "dashPreconditionField": "/operands"}));
    let output = run_config(&config);
    let document = output.envelope();
    let check = check_of(&document["data"], "L2.input-dash-values");
    assert_eq!(check["result"], "unprovided", "{check}");
    assert!(check.to_string().contains("ALREADY_SEALED"), "{check}");
    let named = run_config(&l2_config(&json!({"dashPrecondition": true, "dashPreconditionNamed": true, "dashPreconditionBoth": true})));
    assert_eq!(check_of(&named.envelope()["data"], "L2.input-dash-values")["result"], "pass");
}

#[test]
fn atip_checkl2input_13_a_framing_refusal_naming_the_value_is_not_a_value_read_as_a_value() {
    // A tool that reads `-x` as an option or `{x` as a document refuses with INPUT_INVALID, the field and the value as `received`: that is the failure.
    let config = l2_config(&json!({"dashPrecondition": true, "dashPreconditionNamed": true, "dashPreconditionCode": "INPUT_INVALID"}));
    let output = run_config(&config);
    let document = output.envelope();
    assert_eq!(check_of(&document["data"], "L2.input-dash-values")["result"], "fail", "{}", not_passing(&document["data"]));
}
