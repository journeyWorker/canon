//! `atip check --level L2`, the rows that are the entry of an agent into the tool (approaches/checker.md 6, 7.1, 7.2): `L2.skills`,
//! `L2.guide-commands`, `L2.help-guide`, `L2.stream` and `L2.search`. The conforming L2 fake passes, and every pass condition of
//! the rows, planted alone in the fake tool, fails the check that owns it and no other. Each test carries the id of the scenario it
//! proves (`atip.checkl2guides.NN`).

mod support;

use serde_json::{json, Value};

use support::l2::{check_of, covers, guide_cases, l2_config, l2_standard_config, not_passing, proves, run_case, run_config, verdict};
use support::{Case, Harness};

fn case_of(condition: &str) -> Case {
    guide_cases().into_iter().find(|case| case.condition == condition).unwrap_or_else(|| panic!("no case {condition}"))
}

fn argvs(check: &Value) -> Vec<String> {
    check["observed"].as_array().into_iter().flatten().map(|call| call["argv"].as_array().map_or(String::new(), |argv| argv.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "))).collect()
}

#[test]
fn atip_checkl2guides_01_a_conforming_tool_passes_skills_show_each_guide_and_an_unknown_name() {
    let output = run_config(&l2_standard_config());
    let document = output.envelope();
    assert_eq!((output.code, document["ok"].as_bool()), (Some(0), Some(true)), "{}\n{}", not_passing(&document["data"]), output.stdout);
    let skills = check_of(&document["data"], "L2.skills");
    assert_eq!(skills["result"], "pass", "{skills}");
    let observed = argvs(skills);
    for call in ["skills show --json", "skills show overview --json", "skills show safety --json", "skills show nosuch-xyz --json"] {
        assert!(observed.iter().any(|argv| argv == call), "{call} is run: {observed:?}");
    }
    assert_eq!(check_of(&document["data"], "L2.help-guide")["result"], "pass");
}

#[test]
fn atip_checkl2guides_02_a_guide_name_left_out_of_the_list_and_a_skills_show_that_needs_state_or_writes_fail_only_l2_skills() {
    proves(guide_cases(), &["skills"]);
}

#[test]
fn atip_checkl2guides_03_an_unknown_guide_name_is_refused_with_the_guide_names_and_without_them_it_fails() {
    for condition in ["skills/unknown-lists-the-guides", "skills/unknown-refused"] {
        let output = run_case(&case_of(condition), "L2");
        let document = output.envelope();
        verdict(&document["data"], "L2.skills").unwrap_or_else(|why| panic!("{condition}: {why}\n{}", not_passing(&document["data"])));
    }
}

#[test]
fn atip_checkl2guides_04_a_guide_line_with_a_wrong_option_fails_only_l2_guide_commands_naming_the_guide_and_the_line() {
    proves(guide_cases(), &["guide-commands"]);
    let output = run_case(&case_of("guide-commands/line-parses"), "L2");
    let document = output.envelope();
    let check = check_of(&document["data"], "L2.guide-commands");
    let findings = check["findings"].as_array().cloned().unwrap_or_default();
    assert!(findings.iter().any(|finding| finding["message"].as_str().is_some_and(|message| message.contains("guide overview line") && message.contains("bogus-option"))), "{check}");
}

#[test]
fn atip_checkl2guides_05_a_line_with_a_pipe_is_skipped_and_reported_and_a_placeholder_is_replaced_by_a_valid_value() {
    let output = run_config(&l2_standard_config());
    let document = output.envelope();
    let check = check_of(&document["data"], "L2.guide-commands");
    assert_eq!(check["result"], "pass", "{check}");
    let notes = check["notes"].to_string();
    assert!(notes.contains("guide overview line") && notes.contains("unsupported"), "the pipe line is reported: {check}");
    // `<SORT>` is an enumeration: its first value; `<MEMBER>` an integer: 1. The line `member list --team <TEAM> --sort <SORT>` parsed.
}

#[test]
fn atip_checkl2guides_06_only_lines_that_start_with_the_tool_name_are_targets() {
    let harness = Harness::new();
    let config = l2_standard_config();
    let fixture = harness.l2_fixture(&config);
    let output = harness.check_l2(&config, &fixture);
    let document = output.envelope();
    let check = check_of(&document["data"], "L2.guide-commands");
    assert_eq!(check["result"], "pass", "{check}");
    // The guide has prose that names the tool and a line that does not start with it: neither is judged.
    assert!(!check.to_string().contains("prose"), "{check}");
    assert!(atip_check::l2::guide::lines("```\nother status --json\n  fake status --json\n```\nfake status --json\n", "fake").len() == 1);
}

#[test]
fn atip_checkl2guides_07_a_root_help_first_line_without_skills_show_fails_only_l2_help_guide() {
    proves(guide_cases(), &["help-guide"]);
}

#[test]
fn atip_checkl2guides_08_a_conforming_stream_passes_for_every_declared_output_and_names_the_output_on_both_lines() {
    let output = run_config(&l2_standard_config());
    let document = output.envelope();
    let stream = check_of(&document["data"], "L2.stream");
    assert_eq!(stream["result"], "pass", "{stream}");
    let observed = argvs(stream);
    for call in ["build --json", "build --quiet --json", "build --target broken --json"] {
        assert!(observed.iter().any(|argv| argv == call), "{call} is run: {observed:?}");
    }
}

#[test]
fn atip_checkl2guides_09_a_stream_without_an_end_line_an_event_that_breaks_its_schema_and_a_start_before_the_usage_error_fail_only_l2_stream() {
    proves(guide_cases(), &["stream"]);
}

#[test]
fn atip_checkl2guides_10_a_tool_without_a_stream_command_has_l2_stream_unsupported_and_a_server_command_is_left_out() {
    let harness = Harness::new();
    let config = support::l1::l1_standard_config();
    let fixture = harness.l1_fixture(&config);
    let output = harness.check_l2_at(&config, &fixture, "L2");
    let document = output.envelope();
    assert_eq!(check_of(&document["data"], "L2.stream")["result"], "unsupported", "{}", check_of(&document["data"], "L2.stream"));
    assert_eq!(check_of(&document["data"], "L2.search")["result"], "unsupported");
    // A command the fixture names under `notRun` (it says in its document that it does not end on its own) is run once by the L0 part, and never by an L2 row.
    let config = l2_config(&json!({"serveMode": "serve"}));
    let harness = Harness::new();
    let mut fixture = harness.l2_fixture(&config);
    fixture["notRun"] = json!(["serve"]);
    let output = harness.check_l2(&config, &fixture);
    let document = output.envelope();
    assert_eq!(output.code, Some(0), "{}\n{}", not_passing(&document["data"]), output.stdout);
    for row in ["L2.stream", "L2.effects", "L2.skills", "L2.guide-commands", "L2.idempotency", "L2.operation-key"] {
        assert!(!argvs(check_of(&document["data"], row)).iter().any(|argv| argv.starts_with("serve")), "{row} never runs a notRun command");
    }
}

#[test]
fn atip_checkl2guides_11_the_question_of_the_fixture_finds_the_expected_command_in_the_first_five_and_the_sixth_is_an_advisory() {
    let output = run_config(&l2_standard_config());
    let document = output.envelope();
    let search = check_of(&document["data"], "L2.search");
    assert_eq!((search["result"].as_str(), search["strength"].as_str()), (Some("pass"), Some("SHOULD")), "{search}");
    let case = case_of("search/expected-in-the-first-five");
    let output = run_case(&case, "L2");
    let document = output.envelope();
    assert_eq!((output.code, document["ok"].as_bool()), (Some(0), Some(true)), "an advisory leaves the level met: {}", output.stdout);
    verdict(&document["data"], "L2.search").unwrap_or_else(|why| panic!("{why}"));
    proves(guide_cases(), &["search"]);
}

#[test]
fn atip_checkl2guides_12_a_tool_without_schema_search_has_l2_search_unsupported_and_one_with_it_and_no_question_is_unprovided() {
    let harness = Harness::new();
    let config = support::l1::l1_standard_config();
    let fixture = harness.l1_fixture(&config);
    let output = harness.check_l2_at(&config, &fixture, "L2");
    assert_eq!(check_of(&output.envelope()["data"], "L2.search")["result"], "unsupported");
    let config = l2_standard_config();
    let harness = Harness::new();
    let mut fixture = harness.l2_fixture(&config);
    fixture.as_object_mut().expect("fixture").remove("search");
    let output = harness.check_l2(&config, &fixture);
    let document = output.envelope();
    assert_eq!(check_of(&document["data"], "L2.search")["result"], "unprovided", "{}", check_of(&document["data"], "L2.search"));
    assert_eq!((output.code, document["ok"].as_bool()), (Some(0), Some(true)), "a SHOULD row does not decide the level: {}", output.stdout);
    let _ = l2_config(&json!({}));
}

#[test]
fn atip_checkl2guides_13_every_condition_of_the_guide_rows_has_a_case_and_the_conforming_tool_judges_each() {
    covers(guide_cases(), &["stream", "skills", "guide-commands", "search", "help-guide"], &[]);
}

#[test]
fn atip_checkl2guides_14_the_conforming_fake_and_the_checker_itself_pass() {
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
fn atip_checkl2guides_01_a_skills_show_that_writes_fails_l2_skills_alone_and_l2_effects_leaves_it_to_it() {
    // The fixture lists `skills show` under `ok` and the tool, run with the fixture's environment, writes a file: one check fails, never two.
    let case = Case { condition: "skills/writes-nothing", config: l2_config(&json!({"skillsPutsFile": true})), tweak: None }.with_tweak(|fixture| {
        fixture["commands"]["skills show"] = json!({"ok": [["skills", "show", "--json"]]});
    });
    let output = run_case(&case, "L2");
    let data = &output.envelope()["data"];
    verdict(data, "L2.skills").unwrap_or_else(|why| panic!("{why}\n{}", not_passing(data)));
    let skills = check_of(data, "L2.skills");
    assert!(skills["reason"].as_str().unwrap_or_default().contains("skills show"), "{skills}");
    assert!(!argvs(check_of(data, "L2.effects")).iter().any(|argv| argv == "skills show --json"), "L2.effects does not run the fixture's skills show");
}
