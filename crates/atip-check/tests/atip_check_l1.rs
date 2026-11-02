//! `atip check --level L1` (approaches/checker.md 6, 7.2): the conforming L1 fake passes, and every pass condition of every L1
//! check, planted alone in the fake tool, fails the check that owns it and no other. Each test carries the id of the scenario it
//! proves (`atip.checkl1.NN`); the scenarios that belong to an L0 check that this change reached for contract 1.1.1 say so.

mod support;

use std::collections::BTreeSet;

use serde_json::{json, Value};

use atip_check::app::check_binary_mutated;
use atip_check::cli::{CheckArgs, Level};
use support::l1::{l1_bin, l1_cases, l1_config, l1_engine_difference_case, l1_standard_config};
use support::{atip_in, kind_verdict, Case, Harness, Output};

/// The L0 checks of a run and the L1 ones that apply to a 1.1 claim.
const L0_CHECKS: usize = 28;
const L1_CHECKS: usize = 31;
/// `L2.effects` is in every report (a SHOULD below L2).
const EFFECTS: usize = 1;
/// Conditions a case plants that the conforming fake gives no occasion to judge (a code in both registries, which the conforming fake has none of).
const EXEMPT_FROM_CONFORMING_RUN: &[&str] = &["warnings/meaning-agrees"];

/// `atip check --level <level> --json` of the fake tool a configuration makes, with the fixture of `fixture`.
fn run_fixture(harness: &Harness, config: &Value, fixture: &Value, level: &str) -> Output {
    run_bin(harness, &l1_bin(harness, config), fixture, level)
}

/// The same for the executable `bin`.
fn run_bin(harness: &Harness, bin: &std::path::Path, fixture: &Value, level: &str) -> Output {
    let fixture_path = harness.write("fixture.json", fixture);
    let work = harness.path("cwd");
    std::fs::create_dir_all(&work).expect("cwd");
    let tmp = harness.path("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp");
    atip_in(
        &work,
        &tmp,
        &["check", "--bin", bin.to_str().expect("utf8"), "--level", level, "--fixture", fixture_path.to_str().expect("utf8"), "--json"],
    )
}

fn run_case(case: &Case) -> Output {
    let harness = Harness::new();
    let mut fixture = harness.l1_fixture(&case.config);
    if let Some(tweak) = case.tweak {
        tweak(&mut fixture);
    }
    run_fixture(&harness, &case.config, &fixture, "L1")
}

fn run_config(config: &Value) -> Output {
    let harness = Harness::new();
    let fixture = harness.l1_fixture(config);
    run_fixture(&harness, config, &fixture, "L1")
}

fn check_of<'a>(data: &'a Value, id: &str) -> &'a Value {
    data["checks"]
        .as_array()
        .and_then(|checks| checks.iter().find(|check| check["id"] == id))
        .unwrap_or_else(|| panic!("{id} is not in data.checks"))
}

/// The owner is `fail` with a reason and rule numbers, and every other check passes or does not apply.
fn verdict(data: &Value, owner: &str) -> Result<(), String> {
    let checks = data["checks"].as_array().ok_or("data.checks is missing")?;
    if checks.len() != L0_CHECKS + L1_CHECKS + EFFECTS {
        return Err(format!("{} checks, expected {}", checks.len(), L0_CHECKS + L1_CHECKS + EFFECTS));
    }
    for check in checks {
        let id = check["id"].as_str().unwrap_or_default();
        let result = check["result"].as_str().unwrap_or_default();
        if id == owner {
            if result != "fail" {
                return Err(format!("{id} is {result}, expected fail"));
            }
            if check["reason"].as_str().is_none_or(str::is_empty) || check["rules"].as_array().is_none_or(Vec::is_empty) {
                return Err(format!("{id} fails without a reason and rule numbers"));
            }
        } else if !matches!(result, "pass" | "unsupported") {
            return Err(format!("{id} is {result}, but only {owner} was seeded"));
        }
    }
    Ok(())
}

fn owner_of(case: &Case) -> String {
    format!("L1.{}", case.condition.split('/').next().unwrap_or_default())
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

/// Every case of each of the checks `ids` (`tree`, `walk`, ...): `atip check --level L1` exits as the strength says, fails that check and no other.
fn proves(ids: &[&str]) {
    let cases: Vec<Case> = l1_cases()
        .into_iter()
        .filter(|case| ids.contains(&case.condition.split('/').next().unwrap_or_default()))
        // `tree/level` needs an L2 claim over an L1 tree, where the L2 rows fail too: `atip_checkl1_03` and the case loop of `atip_checkl1_28` prove it.
        .filter(|case| case.condition != "tree/level")
        .collect();
    for id in ids {
        assert!(
            cases.iter().any(|case| case.condition.split('/').next() == Some(id)),
            "L1.{id} has a seeded violation for each of its conditions"
        );
    }
    let jobs: Vec<Box<dyn FnOnce() -> Result<(), String> + Send>> = cases
        .into_iter()
        .map(|case| -> Box<dyn FnOnce() -> Result<(), String> + Send> {
            Box::new(move || {
                let owner = owner_of(&case);
                let output = run_case(&case);
                let document = output.envelope();
                verdict(&document["data"], &owner).map_err(|why| format!("{}: {why}\n{}", case.condition, output.stdout.chars().take(600).collect::<String>()))?;
                kind_verdict(&document["data"], &owner, case.condition.split('/').nth(1).unwrap_or_default()).map_err(|why| format!("{}: {why}", case.condition))?;
                let must = check_of(&document["data"], &owner)["strength"] == "MUST";
                let expected = if must { (Some(1), Some(false)) } else { (Some(0), Some(true)) };
                let got = (output.code, document["ok"].as_bool());
                if got != expected {
                    return Err(format!("{}: exit {got:?}, expected {expected:?}", case.condition));
                }
                if must && document["error"]["code"] != "LEVEL_NOT_MET" {
                    return Err(format!("{}: a MUST failure is LEVEL_NOT_MET", case.condition));
                }
                if must && !document["error"]["message"].as_str().unwrap_or_default().contains(&owner) {
                    return Err(format!("{}: the error names the failing check", case.condition));
                }
                Ok(())
            })
        })
        .collect();
    let failures: Vec<String> = in_parallel(jobs).into_iter().filter_map(Result::err).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn atip_checkl1_01_a_conforming_1_1_tool_passes_the_28_l0_and_the_31_l1_checks_with_25_and_21_musts() {
    let output = run_config(&l1_standard_config());
    let document = output.envelope();
    assert_eq!((output.code, document["ok"].as_bool(), document["data"]["passed"].as_bool()), (Some(0), Some(true), Some(true)), "{}", output.stdout);
    let data = &document["data"];
    assert_eq!((data["level"].as_str(), data["contractVersion"].as_str()), (Some("L1"), Some("1.1")));
    let checks = data["checks"].as_array().expect("checks");
    assert_eq!(checks.len(), L0_CHECKS + L1_CHECKS + EFFECTS);
    let ids: Vec<&str> = checks.iter().filter_map(|check| check["id"].as_str()).collect();
    assert_eq!(ids.iter().filter(|id| id.starts_with("L0.")).count(), L0_CHECKS);
    let l1: Vec<&str> = ids.iter().copied().filter(|id| id.starts_with("L1.")).collect();
    assert_eq!(l1, atip_check::checks::l1_ids().to_vec(), "the L1 checks in the order of approaches/checker.md");
    let applicable = |strength: &str, prefix: &str| {
        checks.iter().filter(|check| check["id"].as_str().is_some_and(|id| id.starts_with(prefix)) && check["strength"] == strength && check["result"] != "unsupported").count()
    };
    assert_eq!((applicable("MUST", "L1."), applicable("SHOULD", "L1.")), (25, 6));
    assert!(checks.iter().all(|check| matches!(check["result"].as_str(), Some("pass" | "unsupported"))), "{}", output.stdout);
    // `--contract 1.1` says the same.
    let harness = Harness::new();
    let config = l1_standard_config();
    let fixture_path = harness.write("fixture.json", &harness.l1_fixture(&config));
    let work = harness.path("cwd");
    std::fs::create_dir_all(&work).expect("cwd");
    let tmp = harness.path("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let same = atip_in(&work, &tmp, &["check", "--bin", support::FAKE_TOOL, "--level", "L1", "--contract", "1.1", "--fixture", fixture_path.to_str().expect("utf8"), "--json"]);
    assert_eq!(same.code, Some(0), "{}", same.stdout);
}

#[test]
fn atip_checkl1_02_level_l2_runs_and_an_unknown_level_is_usage() {
    let harness = Harness::new();
    let config = l1_standard_config();
    let fixture = harness.l1_fixture(&config);
    // L2 is a level of the checker: a tool that declares L1 does not meet it (its tree says L1), and nothing says it is unsupported.
    let output = run_fixture(&harness, &config, &fixture, "L2");
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str()), (Some(1), Some("LEVEL_NOT_MET")), "{}", output.stdout);
    assert_eq!(document["data"]["level"], "L2");
    assert_eq!(check_of(&document["data"], "L1.tree")["result"], "fail", "{}", output.stdout);
    let output = run_fixture(&harness, &config, &fixture, "L9");
    assert_eq!((output.code, output.envelope()["error"]["code"].as_str()), (Some(2), Some("USAGE")), "{}", output.stdout);
}

#[test]
fn atip_checkl1_03_a_tree_whose_level_is_not_the_claim_and_a_tool_without_schema_fail_l1_tree() {
    let mismatched = l1_cases().into_iter().find(|case| case.condition == "tree/level").expect("case");
    // The tree says L1 and the claim is L2.
    let harness = Harness::new();
    let output = harness.check_l2_at(&mismatched.config, &harness.l1_fixture(&mismatched.config), "L2");
    assert_eq!(check_of(&output.envelope()["data"], "L1.tree")["result"], "fail", "{}", output.stdout);
    assert!(check_of(&output.envelope()["data"], "L1.tree")["reason"].as_str().unwrap_or_default().contains("below the claimed level"), "{}", output.stdout);
    // A tool with no `schema` at all, claiming L1: `L1.tree` is `answered`'s failure; the other checks that need the tree do not each fail for it.
    let answered = l1_cases().into_iter().find(|case| case.condition == "tree/answered").expect("case");
    let output = run_case(&answered);
    let data = &output.envelope()["data"];
    assert_eq!(check_of(data, "L1.tree")["result"], "fail", "{}", output.stdout);
    assert_eq!((output.code, output.envelope()["error"]["code"].as_str()), (Some(1), Some("LEVEL_NOT_MET")));
}

#[test]
fn atip_checkl1_04_each_condition_of_l1_tree_no_state_walk_quoted_path_and_schema_errors_fails_only_its_owner() {
    proves(&["tree", "no-state", "walk", "quoted-path", "schema-errors"]);
}

#[test]
fn atip_checkl1_05_each_condition_of_the_registry_reasons_warnings_and_group_checks_fails_only_its_owner() {
    proves(&["registry", "reasons", "reasons-reserved", "warnings", "reserved-applicable", "codes-known", "groups-not-runnable"]);
}

#[test]
fn atip_checkl1_06_each_condition_of_grammar_depth_one_meaning_help_line_defaults_and_deterministic_fails_only_its_owner() {
    proves(&["grammar", "depth", "one-meaning", "one-meaning-values", "help-line", "defaults", "deterministic"]);
}

#[test]
fn atip_checkl1_07_each_condition_of_examples_example_runs_and_hint_command_fails_only_its_owner_and_the_advisory_keeps_the_level() {
    proves(&["examples", "example-runs", "hint-command"]);
}

#[test]
fn atip_checkl1_08_a_1_0_claim_has_no_warning_registry_and_no_hint_rule_and_a_warning_registry_that_disagrees_fails() {
    let output = run_config(&l1_config(&json!({"claims": "1.0"})));
    let document = output.envelope();
    // (The fake's L1 documents are 1.1 ones, so `L0.contract-version` fails it for printing them under a 1.0 claim; what is judged here is the two checks that do not apply.)
    for id in ["L1.warnings", "L1.hint-command"] {
        assert_eq!(check_of(&document["data"], id)["result"], "unsupported", "{id}: {}", output.stdout);
    }
    // A tool that offers a warning registry and a code whose meaning differs between the two registries fails.
    let differing = l1_cases().into_iter().find(|case| case.condition == "warnings/meaning-agrees").expect("case");
    let output = run_case(&differing);
    verdict(&output.envelope()["data"], "L1.warnings").unwrap_or_else(|why| panic!("{why}"));
}

#[test]
fn atip_checkl1_09_constraints_declared_and_not_enforced_fail_only_l1_constraints_and_an_entailed_rule_passes_naming_what_entails_it() {
    proves(&["constraints"]);
    let output = run_config(&l1_standard_config());
    let document = output.envelope();
    let check = check_of(&document["data"], "L1.constraints");
    assert_eq!(check["result"], "pass", "{}", output.stdout);
    let notes = check["notes"].as_array().expect("an entailed rule is named in notes");
    assert!(notes.iter().filter_map(Value::as_str).any(|note| note.contains("is entailed by")), "{notes:?}");
}

#[test]
fn atip_checkl1_10_a_domain_that_admits_a_bound_or_refuses_a_value_inside_it_fails_only_l1_domains() {
    proves(&["domains"]);
}

#[test]
fn atip_checkl1_11_a_pattern_outside_the_restricted_form_or_over_rusts_size_limit_fails_only_l1_patterns() {
    proves(&["patterns"]);
}

#[test]
fn atip_checkl1_12_a_unicode_version_difference_between_the_two_engines_passes_and_the_report_names_both_versions_and_the_code_point() {
    let case = l1_engine_difference_case();
    let output = run_case(&case);
    let document = output.envelope();
    assert_eq!((output.code, document["data"]["passed"].as_bool()), (Some(0), Some(true)), "{}", output.stdout);
    let check = check_of(&document["data"], "L1.patterns");
    assert_eq!(check["result"], "pass", "{}", output.stdout);
    let notes: Vec<&str> = check["notes"].as_array().expect("notes").iter().filter_map(Value::as_str).collect();
    assert!(
        notes.iter().any(|note| note.contains("U+11DB0") && note.contains("Unicode 17.0") && note.contains("Unicode 16.0.0")),
        "{notes:?}"
    );
}

#[test]
fn atip_checkl1_13_an_integer_one_beyond_the_safe_range_that_is_accepted_rounded_fails_only_l1_numbers() {
    proves(&["numbers"]);
}

#[test]
fn atip_checkl1_14_a_validator_that_does_not_refuse_or_answers_a_precondition_code_without_setup_fails_only_l1_validators() {
    proves(&["validators"]);
    // The `rejects` lines run in an empty directory, without `prepare` or `setup`: a tool that answers another code for them when the state a
    // `prepare` step made is missing (a precondition read before the validator) fails `L1.validators` and nothing else, though the fixture
    // makes that state for every other call. The same wrapper over a tool whose validators come first changes nothing.
    let harness = Harness::new();
    let config = l1_standard_config();
    let mut fixture = harness.l1_fixture(&config);
    fixture["prepare"] = json!([["/bin/sh", "-c", "echo state > prepared.marker"]]);
    let seeded = harness.write("seeded-config.json", &l1_config(&json!({"validatorCodeUsage": true})));
    let script = |alternative: &str| {
        format!(
            "#!/bin/sh\ncase \" $* \" in *2026-02-30*) [ -e prepared.marker ] || FAKE_ATIP_CONFIG='{alternative}' ;; esac\nexport FAKE_ATIP_CONFIG\nexec '{}' \"$@\"\n",
            support::FAKE_TOOL
        )
    };
    let wrapper = harness.path("precondition-first");
    std::fs::write(&wrapper, script(seeded.to_str().expect("utf8"))).expect("write the wrapper");
    std::fs::set_permissions(&wrapper, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("chmod");
    let output = run_bin(&harness, &wrapper, &fixture, "L1");
    verdict(&output.envelope()["data"], "L1.validators").unwrap_or_else(|why| panic!("{why}\n{}", output.stdout));
    assert!(check_of(&output.envelope()["data"], "L1.validators")["reason"].as_str().unwrap_or_default().contains("2026-02-30"), "{}", output.stdout);
    let control = harness.path("validators-first");
    std::fs::write(&control, script(&harness.path("fake-config.json").to_string_lossy())).expect("write the wrapper");
    std::fs::set_permissions(&control, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("chmod");
    let output = run_bin(&harness, &control, &fixture, "L1");
    assert_eq!((output.code, output.envelope()["data"]["passed"].as_bool()), (Some(0), Some(true)), "{}", output.stdout);
    assert_eq!(check_of(&output.envelope()["data"], "L1.validators")["result"], "pass", "{}", output.stdout);
}

#[test]
fn atip_checkl1_15_a_write_command_the_fixture_does_not_list_is_never_run_and_the_report_names_it_unlisted_without_making_any_check_unprovided() {
    let harness = Harness::new();
    let config = l1_standard_config();
    let listed = run_fixture(&harness, &config, &harness.l1_fixture(&config), "L1");
    let listed_document = listed.envelope();
    let unlisted_of = |document: &Value, id: &str| -> Vec<String> {
        check_of(&document["data"], id)["unlisted"].as_array().map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
    };
    assert!(!unlisted_of(&listed_document, "L1.constraints").contains(&"member tag".to_string()), "{}", listed.stdout);
    let mut fixture = harness.l1_fixture(&config);
    fixture["commands"].as_object_mut().expect("commands").remove("member tag");
    let output = run_fixture(&harness, &config, &fixture, "L1");
    let document = output.envelope();
    // `member tag` writes `tags/<id>.json` into the working directory of the sequence: nothing in this run's logs calls it.
    let ran_it = document["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .flat_map(|check| check["observed"].as_array().cloned().unwrap_or_default())
        .any(|entry| entry["argv"].as_array().is_some_and(|argv| argv.iter().take(2).filter_map(Value::as_str).eq(["member", "tag"])));
    assert!(!ran_it, "the write command is never run");
    // It is outside the population of the three checks that probe a command's declarations: none of them is unprovided or fails for it, the run passes,
    // and each names the command, so a fixture cannot hide a write silently.
    assert_eq!((output.code, document["data"]["passed"].as_bool()), (Some(0), Some(true)), "{}", output.stdout);
    for id in ["L1.constraints", "L1.domains", "L1.validators"] {
        assert_eq!(check_of(&document["data"], id)["result"], "pass", "{id}: {}", output.stdout);
        assert!(unlisted_of(&document, id).contains(&"member tag".to_string()), "{id} names the unlisted write: {}", output.stdout);
    }
    assert!(document["data"]["checks"].as_array().expect("checks").iter().all(|check| check["result"] != "unprovided"), "{}", output.stdout);
    assert!(check_of(&document["data"], "L1.paging").get("unlisted").is_none(), "only the three checks carry the list");
}

#[test]
fn atip_checkl1_16_each_condition_of_output_data_and_next_fails_only_its_owner() {
    proves(&["output-data", "next"]);
}

#[test]
fn atip_checkl1_17_a_cursor_reused_with_a_changed_argument_that_is_accepted_or_a_page_that_repeats_fails_only_l1_paging() {
    proves(&["paging"]);
}

#[test]
fn atip_checkl1_18_paging_is_decided_by_the_command_documents_a_paged_entry_is_used_first_and_a_paging_command_without_either_is_unprovided() {
    // From the documents: `member list` declares `--cursor`; its `paged` entry is used and the run passes.
    let harness = Harness::new();
    let config = l1_standard_config();
    let mut fixture = harness.l1_fixture(&config);
    let output = run_fixture(&harness, &config, &fixture, "L1");
    assert_eq!(check_of(&output.envelope()["data"], "L1.paging")["result"], "pass", "{}", output.stdout);
    // No `paged` entry: the first page the run observed with a `nextCursor` (here the one of the read example of the command) stands in for it.
    fixture["commands"]["member list"].as_object_mut().expect("entry").remove("paged");
    let observed = run_fixture(&harness, &config, &fixture, "L1");
    assert_eq!(check_of(&observed.envelope()["data"], "L1.paging")["result"], "pass", "{}", observed.stdout);
    // A command whose document declares `--cursor` and that no run ever saw page: `unprovided`, which fails the level.
    let option = json!({"name": "cursor", "valueType": "string", "valueName": "CURSOR", "required": false, "repeatable": false, "default": null, "help": "A seeded option"});
    let config = l1_config(&json!({"docPatch": {"items": [{"op": "add", "path": "/options/-", "value": option}]}}));
    let harness = Harness::new();
    let fixture = harness.l1_fixture(&config);
    let none = run_fixture(&harness, &config, &fixture, "L1");
    let document = none.envelope();
    assert_eq!(check_of(&document["data"], "L1.paging")["result"], "unprovided", "{}", none.stdout);
    assert_eq!((none.code, document["error"]["code"].as_str()), (Some(1), Some("LEVEL_NOT_MET")));
}

#[test]
fn atip_checkl1_19_each_condition_of_fields_fails_only_l1_fields() {
    proves(&["fields"]);
}

#[test]
fn atip_checkl1_20_a_tool_whose_documents_declare_neither_fields_nor_cursor_has_no_paging_and_no_fields_to_check() {
    let removal = |name: &str| {
        json!({"op": "remove", "path": format!("/options/name={name}")})
    };
    let config = l1_config(&json!({"docPatch": {"member list": [removal("cursor"), removal("fields"), removal("all")]}}));
    let harness = Harness::new();
    let mut fixture = harness.l1_fixture(&config);
    fixture["commands"]["member list"].as_object_mut().expect("entry").remove("fields");
    fixture["commands"]["member list"].as_object_mut().expect("entry").remove("paged");
    let output = run_fixture(&harness, &config, &fixture, "L1");
    let data = output.envelope()["data"].clone();
    for id in ["L1.paging", "L1.fields"] {
        assert!(matches!(check_of(&data, id)["result"].as_str(), Some("unsupported")), "{id}: {}", output.stdout);
    }
}

#[test]
fn atip_checkl1_21_a_fixture_without_the_entries_a_must_check_needs_makes_it_unprovided_and_the_exit_1_and_a_violation_seen_still_fails() {
    let harness = Harness::new();
    let config = l1_standard_config();
    let mut fixture = harness.l1_fixture(&config);
    fixture["commands"] = json!({"items": {"ok": [["items", "--json"]]}});
    let output = run_fixture(&harness, &config, &fixture, "L1");
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str()), (Some(1), Some("LEVEL_NOT_MET")), "{}", output.stdout);
    assert!(
        document["data"]["checks"].as_array().expect("checks").iter().any(|check| check["strength"] == "MUST" && check["result"] == "unprovided"),
        "{}",
        output.stdout
    );
    // A violation the run observed is a failure whatever the fixture lacks.
    let seeded = l1_cases().into_iter().find(|case| case.condition == "grammar/no-id-suffix").expect("case");
    let harness = Harness::new();
    let mut fixture = harness.l1_fixture(&seeded.config);
    fixture["commands"] = json!({"items": {"ok": [["items", "--json"]]}});
    let output = run_fixture(&harness, &seeded.config, &fixture, "L1");
    assert_eq!(check_of(&output.envelope()["data"], "L1.grammar")["result"], "fail", "{}", output.stdout);
}

#[test]
fn atip_checkl1_22_a_fixture_with_a_malformed_l1_entry_is_fixture_invalid_wrong_shape_and_no_binary_runs() {
    let harness = Harness::new();
    let config = l1_standard_config();
    let base = harness.l1_fixture(&config);
    let broken: Vec<(Value, &str)> = vec![
        (json!({"commands": {"member list": {"domainRejects": "x"}}}), "domainRejects"),
        (json!({"commands": {"member list": {"domainRejects": {"--tag": ["x"]}}}}), "domainRejects"),
        (json!({"commands": {"member list": {"domainRejects": {"tag": []}}}}), "domainRejects"),
        (json!({"commands": {"member list": {"paged": "x"}}}), "paged"),
        (json!({"commands": {"member list": {"paged": [[]]}}}), "paged"),
        (json!({"commands": {"member list": {"fields": [[3]]}}}), "fields"),
        (json!({"prepare": "git init"}), "prepare"),
        (json!({"prepare": [[]]}), "prepare"),
        (json!({"registry": "from-the-tool"}), "registry"),
        (json!({"registry": "from-schema", "warnings": ["STALE_VIEW"]}), "warnings"),
    ];
    for (patch, member) in broken {
        let mut fixture = base.clone();
        for (key, value) in patch.as_object().expect("object") {
            if key == "commands" {
                let commands = value.as_object().expect("commands");
                for (id, entry) in commands {
                    for (name, member) in entry.as_object().expect("entry") {
                        fixture["commands"][id][name] = member.clone();
                    }
                }
            } else {
                fixture[key] = value.clone();
            }
        }
        let output = run_fixture(&harness, &config, &fixture, "L1");
        let document = output.envelope();
        assert_eq!((output.code, document["error"]["code"].as_str(), document["error"]["reason"].as_str()), (Some(2), Some("FIXTURE_INVALID"), Some("WRONG_SHAPE")), "{member}: {}", output.stdout);
        assert!(document["error"]["message"].as_str().unwrap_or_default().contains(member), "{member}: {}", output.stdout);
    }
}

#[test]
fn atip_checkl1_23_prepare_programs_run_before_setup_and_their_effects_are_not_judged_and_a_failing_one_is_prepare_failed() {
    let harness = Harness::new();
    let config = l1_standard_config();
    let mut fixture = harness.l1_fixture(&config);
    // A file the tool never writes: if it were judged as a change of a refusal the run would fail `L0.refusal-effects`.
    fixture["prepare"] = json!([["/bin/sh", "-c", "echo environment > prepared.txt"]]);
    let output = run_fixture(&harness, &config, &fixture, "L1");
    assert_eq!((output.code, output.envelope()["data"]["passed"].as_bool()), (Some(0), Some(true)), "{}", output.stdout);
    assert_eq!(check_of(&output.envelope()["data"], "L0.refusal-effects")["result"], "pass", "{}", output.stdout);
    // The report lists every prepare step with its argv (contract 1.1.2).
    assert_eq!(output.envelope()["data"]["prepare"], json!([["/bin/sh", "-c", "echo environment > prepared.txt"]]), "{}", output.stdout);
    // A `setup` step that exits non-zero is a fixture error too: `SETUP_FAILED`, exit 2, and no check result at all.
    fixture["prepare"] = json!([]);
    fixture["setup"] = json!([["reject", "--json"]]);
    let output = run_fixture(&harness, &config, &fixture, "L1");
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str(), document["error"]["reason"].as_str()), (Some(2), Some("FIXTURE_INVALID"), Some("SETUP_FAILED")), "{}", output.stdout);
    assert!(document.get("data").is_none_or(Value::is_null), "a failing setup reports no check: {}", output.stdout);
    fixture["setup"] = json!([]);
    fixture["prepare"] = json!([["/bin/sh", "-c", "echo environment > prepared.txt"]]);
    // A prepare step that runs the binary under test is a fixture error whatever it does: the tool's own state is `setup`'s.
    fixture["prepare"] = json!([[l1_bin(&harness, &config).to_str().expect("utf8"), "init", "--json"]]);
    let output = run_fixture(&harness, &config, &fixture, "L1");
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str(), document["error"]["reason"].as_str()), (Some(2), Some("FIXTURE_INVALID"), Some("PREPARE_FAILED")), "{}", output.stdout);
    assert!(document["error"]["message"].as_str().unwrap_or_default().contains("runs the binary under test"), "{}", output.stdout);
    assert!(document["data"].is_null() || document.get("data").is_none(), "a fixture error reports no check: {}", output.stdout);
    fixture["prepare"] = json!([["/bin/sh", "-c", "echo no repository >&2; exit 3"]]);
    let output = run_fixture(&harness, &config, &fixture, "L1");
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str(), document["error"]["reason"].as_str()), (Some(2), Some("FIXTURE_INVALID"), Some("PREPARE_FAILED")), "{}", output.stdout);
    assert!(document["error"]["message"].as_str().unwrap_or_default().contains("no repository"), "{}", output.stdout);
}

#[test]
fn atip_checkl1_24_a_registry_taken_from_the_schema_is_not_compared_and_a_code_the_tool_does_not_list_fails_l0_registry() {
    let harness = Harness::new();
    let config = l1_standard_config();
    let mut fixture = harness.l1_fixture(&config);
    fixture["registry"] = json!("from-schema");
    fixture.as_object_mut().expect("object").remove("warnings");
    let output = run_fixture(&harness, &config, &fixture, "L1");
    assert_eq!((output.code, check_of(&output.envelope()["data"], "L0.registry")["result"].as_str()), (Some(0), Some("pass")), "{}", output.stdout);
    // A tool whose schema does not list a code it emits fails against its own list.
    let unregistered = l1_cases().into_iter().find(|case| case.condition == "registry/commands-registered").expect("case");
    let harness = Harness::new();
    let mut fixture = harness.l1_fixture(&unregistered.config);
    fixture["registry"] = json!("from-schema");
    fixture.as_object_mut().expect("object").remove("warnings");
    let output = run_fixture(&harness, &unregistered.config, &fixture, "L1");
    assert_eq!(check_of(&output.envelope()["data"], "L1.registry")["result"], "fail", "{}", output.stdout);
}

#[test]
fn atip_checkl1_28_a_check_forced_to_pass_makes_its_seeded_case_fail_and_every_condition_a_conforming_run_judges_has_a_case() {
    // The conditions a conforming L1 run judges, and the conditions the cases plant (the L0 ones are `seeded_violations.rs`'s).
    let harness = Harness::new();
    let config = l1_standard_config();
    let fixture = harness.l1_fixture(&config);
    let args = CheckArgs {
        bin: l1_bin(&harness, &config).into(),
        level: Level::L1,
        fixture: harness.write("fixture.json", &fixture).into(),
        contract: None,
    };
    let (report, judged) = check_binary_mutated(&args, &[], &[]).expect("a report");
    assert!(report.passed, "{}", report.text());
    let l0_args = CheckArgs { level: Level::L0, ..args.clone() };
    let (_, judged_l0) = check_binary_mutated(&l0_args, &[], &[]).expect("a report");
    // The conditions of `L2.effects` are planted by `atip_checkl2safety`: the row runs at every level.
    let judged: BTreeSet<String> = judged.difference(&judged_l0).filter(|condition| !condition.starts_with("effects/")).cloned().collect();
    let mut named: BTreeSet<String> = l1_cases().iter().map(|case| case.condition.to_string()).collect();
    named.insert(l1_engine_difference_case().condition.to_string());
    let l1_judged = judged;
    let unplanted: Vec<&String> = l1_judged.difference(&named).collect();
    let unjudged: Vec<&String> = named.difference(&l1_judged).collect();
    assert!(unplanted.is_empty(), "judged by a conforming run and not planted by any case: {unplanted:?}");
    // A conforming run judges every condition a case plants but those that need a different tool: a condition a conforming tool has nothing to judge for.
    println!("planted and not judged by the conforming run: {unjudged:?}");
    assert!(unjudged.iter().all(|condition| EXEMPT_FROM_CONFORMING_RUN.contains(&condition.as_str())), "planted, never judged by a conforming run, and not named as exempt: {unjudged:?}");
    // Every case: with its own condition switched off, the owner no longer fails (so the condition is independently proven: the planted document breaks nothing else
    // the owner judges). For the first case of each check, the owner forced to pass no longer fails either.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let firsts: Vec<(Case, bool)> = l1_cases().into_iter().map(|case| { let first = seen.insert(owner_of(&case)); (case, first) }).collect();
    let jobs: Vec<Box<dyn FnOnce() -> Result<(), String> + Send>> = firsts
        .into_iter()
        .map(|(case, first)| -> Box<dyn FnOnce() -> Result<(), String> + Send> {
            Box::new(move || {
                let harness = Harness::new();
                let mut fixture = harness.l1_fixture(&case.config);
                if let Some(tweak) = case.tweak {
                    tweak(&mut fixture);
                }
                let args = CheckArgs {
                    bin: l1_bin(&harness, &case.config).into(),
                    level: if case.condition == "tree/level" { Level::L2 } else { Level::L1 },
                    fixture: harness.write("fixture.json", &fixture).into(),
                    contract: None,
                };
                let owner = owner_of(&case);
                if first {
                    let (forced, _) = check_binary_mutated(&args, &[owner.as_str()], &[]).map_err(|failure| format!("{}: {}", case.condition, failure.message))?;
                    if forced.find(&owner).map(|check| check.result) == Some(atip_check::report::CheckResult::Fail) {
                        return Err(format!("{}: forced to pass, {owner} still fails", case.condition));
                    }
                }
                let (skipped, _) = check_binary_mutated(&args, &[], &[case.condition]).map_err(|failure| format!("{}: {}", case.condition, failure.message))?;
                if skipped.find(&owner).map(|check| check.result) == Some(atip_check::report::CheckResult::Fail) {
                    return Err(format!("{}: with the condition switched off {owner} still fails", case.condition));
                }
                Ok(())
            })
        })
        .collect();
    let failures: Vec<String> = in_parallel(jobs).into_iter().filter_map(Result::err).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn atip_checkl1_29_the_checker_run_on_itself_at_l0_with_its_own_fixture_passes() {
    let harness = Harness::new();
    let fixture = harness.path("fixture-copy.json");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("atip.fixture.json");
    std::fs::copy(&source, &fixture).expect("copy the fixture");
    let work = harness.path("cwd");
    std::fs::create_dir_all(&work).expect("cwd");
    std::fs::copy(&source, work.join("atip.fixture.json")).expect("copy next to the invocations");
    let tmp = harness.path("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let output = atip_in(&work, &tmp, &["check", "--bin", support::ATIP, "--level", "L0", "--fixture", fixture.to_str().expect("utf8"), "--json"]);
    assert_eq!((output.code, output.envelope()["data"]["passed"].as_bool()), (Some(0), Some(true)), "{}", output.stdout);
}

#[test]
fn atip_checkl1_30_canons_fixture_claims_l1_and_the_ci_job_and_the_just_recipe_run_it_and_a_broken_example_turns_the_job_red() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(root.join("crates/canon-cli/tests/atip/canon.fixture.json")).expect("canon fixture")).expect("JSON");
    let parsed = atip_check::fixture::parse(&fixture).expect("canon's fixture is one the checker reads");
    assert!(!parsed.prepare.is_empty() && parsed.prepare[0][0] == "git", "{:?}", parsed.prepare);
    let commands = parsed.commands.expect("commands");
    let paged: Vec<&String> = commands.iter().filter(|(_, entry)| !entry.paged.is_empty()).map(|(id, _)| id).collect();
    assert_eq!(paged, ["divergence list", "export", "finding list", "query"]);
    assert!(commands.values().any(|entry| !entry.fields.is_empty()));
    let workflow = std::fs::read_to_string(root.join(".github/workflows/test.yml")).expect("workflow");
    assert!(workflow.contains("atip-l1:") && workflow.contains("--level L1 --fixture crates/canon-cli/tests/atip/canon.fixture.json"), "the CI job runs canon at L1");
    let just = std::fs::read_to_string(root.join("justfile")).expect("justfile");
    assert!(just.contains("--level L1 --fixture crates/canon-cli/tests/atip/canon.fixture.json"), "just atip runs canon at L1");
    // The red path: an example that does not parse fails `L1.examples`, so the job fails.
    let case = l1_cases().into_iter().find(|case| case.condition == "examples/grammar").expect("case");
    let output = run_case(&case);
    verdict(&output.envelope()["data"], "L1.examples").unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(output.code, Some(1));
}

#[test]
fn atip_checkl1_32_l0_page_and_l1_paging_follow_the_paged_key_and_the_documents_or_the_observation_at_l0() {
    // At L0 a listed command that refuses `--cursor x` like an unknown option does not page; a fixture that lists no command is `unprovided`.
    let harness = Harness::new();
    let config = l1_standard_config();
    let mut fixture = harness.l1_fixture(&config);
    let l0 = run_fixture(&harness, &config, &fixture, "L0");
    let l0_document = l0.envelope();
    assert_eq!(check_of(&l0_document["data"], "L0.page")["result"], "pass", "{}", l0.stdout);
    fixture["commands"] = json!({"items": {"ok": [["items", "--json"]]}});
    let none = run_fixture(&harness, &config, &fixture, "L0");
    assert_eq!(check_of(&none.envelope()["data"], "L0.page")["result"], "unsupported", "{}", none.stdout);
    // A listed command that pages (it answers `CURSOR_INVALID` for a cursor it did not issue) and no `paged` entry and no nextCursor: unprovided.
    fixture["commands"] = json!({"member list": {"refused": [["member", "list", "--limit", "0", "--json"]]}});
    let pages = run_fixture(&harness, &config, &fixture, "L0");
    assert_eq!(check_of(&pages.envelope()["data"], "L0.page")["result"], "unprovided", "{}", pages.stdout);
    // A `paged` entry that does not return a nextCursor fails `L0.page`.
    fixture["commands"] = json!({"member list": {"paged": [["member", "list", "--team", "ghost", "--json"]]}});
    let bad = run_fixture(&harness, &config, &fixture, "L0");
    assert_eq!(check_of(&bad.envelope()["data"], "L0.page")["result"], "fail", "{}", bad.stdout);
}

#[test]
fn atip_checkl1_33_from_l1_a_group_that_runs_is_l1_groups_not_runnable_alone_and_a_finer_usage_code_is_usage() {
    let case = l1_cases().into_iter().find(|case| case.condition == "groups-not-runnable/command-required").expect("case");
    // The fixture's one group is the one that runs (`team`), so `L0.global-on-group` has a group to run.
    let harness = Harness::new();
    let mut fixture = harness.l1_fixture(&case.config);
    fixture["commands"] = json!({"team add": {"ok": [["team", "add", "--json"]]}, "items": {"ok": [["items", "--json"]]}});
    let output = run_fixture(&harness, &case.config, &fixture, "L1");
    assert_eq!(check_of(&output.envelope()["data"], "L1.groups-not-runnable")["result"], "fail", "{}", output.stdout);
    assert_eq!(check_of(&output.envelope()["data"], "L0.global-on-group")["result"], "unsupported", "{}", output.stdout);
    // The same group run at L0 is `L0.global-on-group`'s failure.
    let l0 = run_fixture(&harness, &case.config, &fixture, "L0");
    assert_eq!(check_of(&l0.envelope()["data"], "L0.global-on-group")["result"], "fail", "{}", l0.stdout);
}

/// Prints the verdict of every case whose condition starts with `$CASE` (all of them by default) and, for a mismatch, the checks that failed; run by hand.
#[test]
#[ignore = "drafts the verdicts of the cases by hand"]
fn draft_every_case() {
    let only = std::env::var("CASE").unwrap_or_default();
    for case in l1_cases().into_iter().filter(|case| case.condition.starts_with(&only)) {
        let output = run_case(&case);
        let document = output.envelope();
        let owner = owner_of(&case);
        let failed: Vec<String> = document["data"]["checks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|check| check["result"] == "fail")
            .map(|check| check["id"].as_str().unwrap_or_default().to_string())
            .collect();
        let ok = failed == [owner.clone()];
        println!("{} {} owner {owner} failed {failed:?}", if ok { "ok" } else { "MISMATCH" }, case.condition);
        if !ok || std::env::var("VERBOSE").is_ok() {
            println!("    exit {:?} {}", output.code, output.stdout.chars().take(500).collect::<String>());
            for check in document["data"]["checks"].as_array().into_iter().flatten() {
                if check["id"] == owner.as_str() || check["id"] == "L0.global-on-group" || check["result"] == "fail" || check["result"] == "unprovided" {
                    println!("    {} {}: {}", check["id"], check["result"], check["reason"].as_str().unwrap_or_default().chars().take(2500).collect::<String>());
                }
            }
        }
    }
}

/// Prints the checks that are not `pass` for the conforming fake with `$CONFIG` (a JSON object) merged into the standard configuration; run by hand.
#[test]
#[ignore = "drafts a run by hand"]
fn draft_config() {
    let extra: Value = serde_json::from_str(&std::env::var("CONFIG").unwrap_or_else(|_| "{}".to_string())).expect("CONFIG is JSON");
    let output = run_config(&l1_config(&extra));
    let document = output.envelope();
    println!("exit {:?} {}", output.code, document["error"]["message"]);
    for check in document["data"]["checks"].as_array().into_iter().flatten() {
        if check["result"] != "pass" {
            println!("    {} {} {}: {}", check["id"], check["strength"], check["result"], check["reason"].as_str().unwrap_or_default().chars().take(500).collect::<String>());
        }
    }
}

#[test]
fn atip_checkl1_03_a_tree_that_declares_a_higher_level_than_the_claim_meets_it() {
    // Levels are cumulative: an L1 claim against a tree that says L2 passes L1.tree.
    let case = Case { condition: "tree/level", config: l1_config(&json!({"treeLevel": "L2"})), tweak: None };
    let output = run_case(&case);
    let data = &output.envelope()["data"];
    assert_eq!(check_of(data, "L1.tree")["result"], "pass", "{}", output.stdout);
}
