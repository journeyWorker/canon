//! approaches/checker.md 5 (atip/1 1.2.1): the fixture key `notRun`. A command that does not end on its own runs once, with `--json`, up to
//! the end of its first JSON document; the checker confirms it is still running and signals it (SIGTERM, SIGKILL after a grace period). The
//! document goes to the envelope checks and `L1.output-data`; its other runs that would succeed are skipped and reported. Each test carries the
//! id of the scenario it proves (`atip.check.95-97` and `atip.check.114-121`).

mod support;

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use support::l1::{l1_config, l1_config_1_2};
use support::{Harness, Output};

fn result_of(output: &Output, id: &str) -> String {
    output.check(id)["result"].as_str().unwrap_or_default().to_string()
}

/// The checks of the report that failed or are unprovided.
fn failing(output: &Output) -> Vec<String> {
    output
        .results()
        .into_iter()
        .filter(|(_, result)| matches!(result.as_str(), "fail" | "unprovided"))
        .map(|(id, _)| id)
        .collect()
}

/// The lines of the fake tool's invocation log: the arguments of each run, joined by spaces.
fn run_log(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path).unwrap_or_default().lines().map(str::to_string).collect()
}

fn serve_lines(path: &std::path::Path) -> Vec<String> {
    run_log(path).into_iter().filter(|line| line.starts_with("serve")).collect()
}

/// The L0 fixture of a tool with a server `serve`: `commands.serve` lists `ok` invocations, and `serve` is under `notRun`.
fn l0_fixture(harness: &Harness, config: &Value, ok: &[&[&str]]) -> Value {
    let mut fixture = harness.fixture(config);
    fixture["commands"]["serve"] = json!({"ok": ok});
    fixture["notRun"] = json!(["serve"]);
    fixture["timeoutSeconds"] = json!(15);
    fixture
}

/// The L1 fixture of a tool with a server `serve` under `notRun` and no entry of its own.
fn l1_fixture(harness: &Harness, config: &Value) -> Value {
    let mut fixture = harness.l1_fixture(config);
    fixture["notRun"] = json!(["serve"]);
    fixture["timeoutSeconds"] = json!(15);
    fixture
}

fn observed_served(output: &Output, id: &str) -> Vec<Value> {
    output.check(id)["observed"].as_array().cloned().unwrap_or_default().into_iter().filter(|call| call["served"] == true).collect()
}

#[test]
fn atip_check_95_a_command_named_in_not_run_runs_once_up_to_its_first_document_and_the_checker_ends_it_by_a_signal() {
    let harness = Harness::new();
    let log = harness.path("runs.log");
    let config = json!({"serveMode": "serve", "invocationLog": log});
    let fixture = l0_fixture(&harness, &config, &[&["serve", "--json"], &["serve", "--json", "--port", "9"]]);
    let started = Instant::now();
    let output = harness.check(&fixture, true);
    assert_eq!(output.code, Some(0), "{}", output.stdout.chars().take(400).collect::<String>());
    assert!(started.elapsed() < Duration::from_secs(120), "the run did not wait for a server that never ends: {:?}", started.elapsed());
    // One run: the first `ok` invocation, with `--json`; the second is not run.
    // The runs that the contract still makes (refusals the checker expects) may follow; no run of it that succeeds does, but the one.
    let lines = serve_lines(&log);
    assert_eq!(lines.iter().filter(|line| line.as_str() == "serve --json").count(), 1, "{lines:?}");
    assert!(lines.iter().all(|line| !line.contains("--port")), "the second ok is not run: {lines:?}");
    // Its document is judged by the envelope checks, and the run ended by the checker's signal is no exit to judge.
    let served = observed_served(&output, "L0.envelope");
    assert_eq!(served.len(), 1, "{}", output.check("L0.envelope"));
    assert_eq!(served[0]["argv"], json!(["serve", "--json"]));
    assert_eq!(served[0]["signal"], 15, "ended by SIGTERM: {}", served[0]);
    assert!(served[0]["stdout"].as_str().is_some_and(|text| text.contains("\"serving\":true")), "{}", served[0]);
    for id in ["L0.envelope", "L0.no-null", "L0.contract-version", "L0.next", "L0.ok-exit", "L0.one-document"] {
        assert_ne!(result_of(&output, id), "fail", "{id}");
    }
    // The report names the command and the run it skipped.
    let check = output.check("L0.page");
    assert_eq!(check["notRun"], json!(["serve"]), "{check}");
    assert!(check["skipped"].as_array().is_some_and(|skipped| skipped.iter().any(|run| run.as_str().is_some_and(|text| text.starts_with("serve: `serve --json --port 9`") && text.ends_with(": notRun")))), "{check}");
}

#[test]
fn atip_check_114_with_no_ok_invocation_the_first_example_is_the_one_run_and_the_other_examples_are_skipped_and_reported() {
    let harness = Harness::new();
    let log = harness.path("runs.log");
    let config = l1_config(&json!({"serveMode": "serve", "invocationLog": log}));
    let fixture = l1_fixture(&harness, &config);
    let output = harness.check_l1(&config, &fixture);
    assert_eq!(output.code, Some(0), "{:?} {}", failing(&output), output.stdout.chars().take(300).collect::<String>());
    // `serve --json` (the first example) ran once; the second example (`--port 8080`) and the in-range values of the domain did not.
    let lines = serve_lines(&log);
    assert_eq!(lines.iter().filter(|line| line.as_str() == "serve --json").count(), 1, "{lines:?}");
    assert!(lines.iter().all(|line| !line.contains("--port 8080") && !line.contains("--port=8080")), "no run of the second example: {lines:?}");
    let served = observed_served(&output, "L1.output-data");
    assert_eq!(served.len(), 1, "{}", output.check("L1.output-data"));
    assert_eq!(served[0]["argv"], json!(["serve", "--json"]));
    for (id, what) in [("L1.example-runs", "an example"), ("L1.output-data", "an example")] {
        let skipped = output.check(id)["skipped"].as_array().cloned().unwrap_or_default();
        assert!(skipped.iter().any(|run| run.as_str().is_some_and(|text| text.contains("serve --port 8080 --json") && text.contains(what) && text.ends_with("notRun"))), "{id}: {skipped:?}");
    }
    assert!(output.check("L1.domains")["skipped"].as_array().is_some_and(|skipped| skipped.iter().any(|run| run.as_str().is_some_and(|text| text.contains("a value inside the domain")))), "{}", output.check("L1.domains"));
}

#[test]
fn atip_check_115_a_command_that_ignores_sigterm_is_killed_after_the_grace_period_and_the_checker_does_not_hang() {
    let harness = Harness::new();
    let config = json!({"serveMode": "ignoreTerm"});
    let fixture = l0_fixture(&harness, &config, &[&["serve", "--json"]]);
    let started = Instant::now();
    let output = harness.check(&fixture, true);
    let elapsed = started.elapsed();
    assert_eq!(output.code, Some(0), "{}", output.stdout.chars().take(400).collect::<String>());
    let served = observed_served(&output, "L0.envelope");
    assert_eq!(served.len(), 1, "{}", output.check("L0.envelope"));
    assert_eq!(served[0]["signal"], 9, "SIGKILL after the grace period: {}", served[0]);
    assert!(elapsed < Duration::from_secs(60), "{elapsed:?}");
}

#[test]
fn atip_check_116_a_command_that_ends_on_its_own_before_or_after_its_first_document_is_a_fixture_error_and_no_check_runs() {
    for mode in ["exitBefore", "exitAfter"] {
        let harness = Harness::new();
        let marker = harness.path("init.log");
        let config = json!({"serveMode": mode, "initLog": marker});
        let fixture = l0_fixture(&harness, &config, &[&["serve", "--json"]]);
        let output = harness.check(&fixture, true);
        let document = output.envelope();
        assert_eq!((output.code, document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some(2), Some("FIXTURE_INVALID"), Some("--fixture")), "{mode}: {}", output.stdout);
        let message = document["error"]["message"].as_str().unwrap_or_default();
        assert!(message.contains("serve") && message.contains("ended on its own"), "{mode}: {message}");
        assert!(document.get("data").is_none(), "{mode}: no check reports: {}", output.stdout);
    }
    // At L1 the same, with the first example as the run.
    let harness = Harness::new();
    let config = l1_config(&json!({"serveMode": "exitAfter"}));
    let output = harness.check_l1(&config, &l1_fixture(&harness, &config));
    assert_eq!((output.code, output.envelope()["error"]["code"].as_str()), (Some(2), Some("FIXTURE_INVALID")), "{}", output.stdout);
    assert!(output.envelope().get("data").is_none());
}

#[test]
fn atip_check_117_a_command_that_prints_no_document_before_the_time_limit_fails_l0_envelope_and_only_it() {
    let harness = Harness::new();
    let config = json!({"serveMode": "silent"});
    let mut fixture = l0_fixture(&harness, &config, &[&["serve", "--json"]]);
    fixture["timeoutSeconds"] = json!(3);
    let started = Instant::now();
    let output = harness.check(&fixture, true);
    assert!(started.elapsed() < Duration::from_secs(120), "{:?}", started.elapsed());
    assert_eq!(failing(&output), ["L0.envelope"], "{:?}", output.results());
    assert_eq!(output.code, Some(1), "{}", output.stdout.chars().take(300).collect::<String>());
    let check = output.check("L0.envelope");
    let found = check["findings"].as_array().and_then(|findings| findings.iter().find(|finding| finding["kind"] == "document-before-timeout")).expect("a finding");
    assert_eq!(found["argv"], json!(["serve", "--json"]));
    assert!(found["message"].as_str().unwrap_or_default().contains("no JSON document"), "{found}");
}

#[test]
fn atip_check_118_a_bad_first_document_fails_the_check_that_owns_the_fault_and_no_other() {
    // L0: a null in `meta`, a version that is not the claim, and a next step with no `--json`.
    let cases: [(&str, Value, &str); 3] = [
        ("null", json!({"metaPatch": {"serve": [{"op": "set", "path": "/note", "value": null}]}}), "L0.no-null"),
        ("version", json!({"metaPatch": {"serve": [{"op": "set", "path": "/contractVersion", "value": "1.0"}]}}), "L0.contract-version"),
        (
            "next",
            json!({"metaPatch": {"serve": [{"op": "set", "path": "/next", "value": [{"command": "fake serve", "argv": ["serve"], "reason": "again"}]}]}}),
            "L0.next",
        ),
    ];
    for (name, extra, owner) in cases {
        let harness = Harness::new();
        let mut config = json!({"serveMode": "serve"});
        for (key, value) in extra.as_object().expect("object") {
            config[key] = value.clone();
        }
        let fixture = l0_fixture(&harness, &config, &[&["serve", "--json"]]);
        let output = harness.check(&fixture, true);
        assert_eq!(failing(&output), [owner], "{name}: {:?}\n{}", output.results(), output.stdout.chars().take(500).collect::<String>());
    }
    // L1: data that the output schema of the command refuses.
    let harness = Harness::new();
    let config = l1_config(&json!({"serveMode": "serve", "dataPatch": {"serve": [{"op": "remove", "path": "/serving"}]}}));
    let output = harness.check_l1(&config, &l1_fixture(&harness, &config));
    assert_eq!(failing(&output), ["L1.output-data"], "{:?}", output.results());
}

#[test]
fn atip_check_119_with_no_ok_invocation_and_no_example_the_checks_that_judge_its_document_are_unprovided_and_the_level_is_not_met() {
    // L0: no `ok` entry (at L0 there are no examples).
    let harness = Harness::new();
    let config = json!({"serveMode": "serve"});
    let mut fixture = harness.fixture(&config);
    fixture["notRun"] = json!(["serve"]);
    let output = harness.check(&fixture, true);
    assert_eq!(output.code, Some(1), "{}", output.stdout.chars().take(300).collect::<String>());
    for id in ["L0.envelope", "L0.no-null", "L0.contract-version", "L0.next"] {
        let check = output.check(id);
        assert_eq!(check["result"], "unprovided", "{id}: {check}");
        assert!(check["findings"].as_array().is_some_and(|findings| findings.iter().any(|finding| finding["kind"] == "missing-fixture-entry" && finding["message"] == "commands.serve.ok")), "{id}: {check}");
    }
    // L1: no `ok` entry and no example.
    let harness = Harness::new();
    let config = l1_config(&json!({"serveMode": "serve", "noExamples": true}));
    let output = harness.check_l1(&config, &l1_fixture(&harness, &config));
    assert_eq!(output.code, Some(1), "{}", output.stdout.chars().take(300).collect::<String>());
    assert_eq!(result_of(&output, "L1.output-data"), "unprovided", "{}", output.check("L1.output-data"));
    assert_eq!(result_of(&output, "L0.envelope"), "unprovided");
}

#[test]
fn atip_check_120_the_runs_of_a_not_run_command_that_are_refused_still_run_and_a_domain_it_does_not_enforce_fails_l1_domains() {
    let harness = Harness::new();
    let log = harness.path("runs.log");
    let config = l1_config_1_2(&json!({"serveMode": "serve", "invocationLog": log}));
    let output = harness.check_l1(&config, &l1_fixture(&harness, &config));
    assert_eq!(output.code, Some(0), "{:?}", failing(&output));
    let lines = serve_lines(&log);
    // The values outside the domain of `--port` (0 and 65536) are refused, so they ran; the pairs of faults of L1.check-order ran too.
    assert!(lines.iter().any(|line| line.contains("--port") && (line.contains("=0") || line.ends_with(" 0") || line.contains(" 0 "))), "{lines:?}");
    let about_serve: Vec<String> = run_log(&log).into_iter().filter(|line| line.contains("serve")).collect();
    assert!(about_serve.iter().any(|line| line.contains("--no-such-a") && line.contains("--no-such-b")), "{about_serve:?}");
    assert_eq!(result_of(&output, "L1.domains"), "pass", "{}", output.check("L1.domains"));
    assert_eq!(result_of(&output, "L1.check-order"), "pass");
    // A server that takes a port outside the domain serves on: the run is killed at the time limit and L1.domains fails.
    let harness = Harness::new();
    let config = l1_config(&json!({"serveMode": "serve", "acceptDomain": {"serve": ["port"]}}));
    let mut fixture = l1_fixture(&harness, &config);
    fixture["timeoutSeconds"] = json!(4);
    let output = harness.check_l1(&config, &fixture);
    assert_eq!(result_of(&output, "L1.domains"), "fail", "{:?}", output.results());
}

#[test]
fn atip_check_121_a_check_whose_every_run_was_skipped_is_unprovided_so_listing_commands_cannot_pass_a_level() {
    // Only `serve` has examples, two of them: the first is its one run, the second is skipped, and `L1.example-runs` has no run left.
    let harness = Harness::new();
    let config = l1_config(&json!({"serveMode": "serve", "examplesOnlyServe": true}));
    let output = harness.check_l1(&config, &l1_fixture(&harness, &config));
    let check = output.check("L1.example-runs");
    assert_eq!(check["result"], "unprovided", "{check}");
    assert!(check["findings"].as_array().is_some_and(|findings| findings.iter().any(|finding| finding["kind"] == "all-runs-not-run")), "{check}");
    assert!(check["reason"].as_str().unwrap_or_default().contains("notRun"), "{check}");
    assert_eq!(output.code, Some(1), "{:?}", failing(&output));
}

#[test]
fn atip_check_96_a_not_run_command_that_is_the_probe_or_has_no_effect_note_or_is_not_in_the_tools_documents_is_fixture_invalid_and_no_check_runs() {
    let harness = Harness::new();
    let refused = |output: &Output, what: &str| {
        let document = output.envelope();
        assert_eq!((output.code, document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some(2), Some("FIXTURE_INVALID"), Some("--fixture")), "{what}: {}", output.stdout);
        assert!(document.get("data").is_none(), "{what}: no check reported: {}", output.stdout);
        document["error"]["message"].as_str().unwrap_or_default().to_string()
    };
    // The wrong shapes, the probe, a repeat.
    let marker = harness.path("init.log");
    let mut fixture = harness.fixture(&json!({"initLog": marker}));
    for (wrong, what) in [(json!("serve"), "a string"), (json!([1]), "a number"), (json!(["Serve"]), "a spelling"), (json!(["a", "a"]), "a repeat"), (json!(["status"]), "the probe")] {
        fixture["notRun"] = wrong;
        let output = harness.check(&fixture, true);
        let message = refused(&output, what);
        assert!(message.contains("notRun"), "{what}: {message}");
        assert!(!marker.exists(), "{what}: the binary was not run");
    }
    // At L1 the tool's documents say what there is: an id they lack, and a command whose document has no `effectNote`.
    let config = l1_config(&json!({"serveMode": "serve"}));
    let mut fixture = harness.l1_fixture(&config);
    fixture["notRun"] = json!(["serve", "unknown-server"]);
    let message = refused(&harness.check_l1(&config, &fixture), "an unknown id");
    assert!(message.contains("unknown-server"), "{message}");
    let mut fixture = harness.l1_fixture(&config);
    fixture["commands"].as_object_mut().expect("commands").remove("member list");
    fixture["notRun"] = json!(["member list"]);
    let message = refused(&harness.check_l1(&config, &fixture), "no effectNote");
    assert!(message.contains("effectNote") && message.contains("member list"), "{message}");
    let bare = l1_config(&json!({"serveMode": "serve", "serveNoEffectNote": true}));
    let message = refused(&harness.check_l1(&bare, &l1_fixture(&harness, &bare)), "no effectNote on the server");
    assert!(message.contains("effectNote") && message.contains("serve"), "{message}");
    // At L0 there are no documents to compare with: an id the tool does not have is taken as given (and, with no run to make, is unprovided).
    let mut fixture = harness.fixture(&json!({}));
    fixture["notRun"] = json!(["serve"]);
    assert_eq!(harness.check(&fixture, true).code, Some(1));
}

#[test]
fn atip_check_97_the_example_syntax_check_still_judges_the_examples_of_a_not_run_command_and_the_command_is_never_missing_from_the_report() {
    let harness = Harness::new();
    let case = support::l1::l1_cases().into_iter().find(|case| case.condition == "examples/grammar").expect("the case");
    // The grammar case plants a malformed example in the documents of `member list`; `serve` is the command under `notRun`.
    let mut config = case.config.clone();
    config["serveMode"] = json!("serve");
    let mut fixture = harness.l1_fixture(&config);
    fixture["notRun"] = json!(["serve"]);
    fixture["timeoutSeconds"] = json!(15);
    let output = harness.check_l1(&config, &fixture);
    let check = output.check("L1.examples");
    assert_eq!(check["result"], "fail", "{}", output.stdout.chars().take(400).collect::<String>());
    assert_eq!(check["findings"][0]["kind"], "grammar");
    // Every check that runs commands says the command was left out of its other runs, and the report lists the one run it did make.
    for id in ["L0.page", "L1.example-runs", "L1.output-data", "L1.domains"] {
        assert_eq!(output.check(id)["notRun"], json!(["serve"]), "{id}");
    }
    assert!(output.check("L1.tree").get("notRun").is_none());
    assert_eq!(observed_served(&output, "L0.envelope").len(), 1);
}

#[test]
fn atip_check_116_a_not_run_command_refused_at_its_start_by_a_precondition_is_unprovided_with_its_code_and_any_other_refusal_is_a_fixture_error() {
    let harness = Harness::new();
    let config = json!({"serveMode": "refuseStart"});
    let fixture = l0_fixture(&harness, &config, &[&["serve", "--json"]]);
    let output = harness.check(&fixture, true);
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str()), (Some(1), Some("LEVEL_NOT_MET")), "{}", output.stdout);
    let check = output.check("L0.envelope");
    assert_eq!(check["result"], "unprovided", "{check}");
    assert!(check["findings"].as_array().is_some_and(|findings| findings.iter().any(|found| found["code"] == "CURSOR_INVALID")), "{check}");
    // A refusal of a step that reads no state (here USAGE) is a fixture error, as ending on its own is.
    let harness = Harness::new();
    let config = json!({"serveMode": "refuseStart", "serveRefuseCode": "USAGE"});
    let fixture = l0_fixture(&harness, &config, &[&["serve", "--json"]]);
    let output = harness.check(&fixture, true);
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str()), (Some(2), Some("FIXTURE_INVALID")), "{}", output.stdout);
}

#[test]
fn atip_check_116_a_not_run_refusal_naming_an_argument_or_an_unregistered_code_is_a_fixture_error() {
    for (key, value) in [("serveRefuseField", "--port"), ("serveRefuseCode", "NO_SUCH_CODE")] {
        let harness = Harness::new();
        let mut config = json!({"serveMode": "refuseStart"});
        config[key] = json!(value);
        let fixture = l0_fixture(&harness, &config, &[&["serve", "--json"]]);
        let output = harness.check(&fixture, true);
        let document = output.envelope();
        assert_eq!((output.code, document["error"]["code"].as_str()), (Some(2), Some("FIXTURE_INVALID")), "{key}: {}", output.stdout);
    }
}

/// The run ended as a fixture error: exit 2, `FIXTURE_INVALID` on `--fixture`, no check reported; its message.
fn fixture_error(output: &Output, what: &str) -> String {
    let document = output.envelope();
    assert_eq!((output.code, document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some(2), Some("FIXTURE_INVALID"), Some("--fixture")), "{what}: {}", output.stdout);
    assert!(document.get("data").is_none(), "{what}: no check reports: {}", output.stdout);
    document["error"]["message"].as_str().unwrap_or_default().to_string()
}

#[test]
fn atip_check_116_a_refusal_of_a_step_that_reads_no_state_is_a_fixture_error_whatever_its_code_when_the_code_is_registered() {
    // Registered with exit 2 and refused with no `field`: only the class of the code says the step read no state (checker.md 1.3.1 section 5).
    let extra = json!([{"code": "UNKNOWN_OPTION", "exit": 2}, {"code": "OPTION_REQUIRED", "exit": 2}, {"code": "OPTION_INVALID", "exit": 2}]);
    for code in ["UNKNOWN_COMMAND", "COMMAND_REQUIRED", "INPUT_INVALID", "UNKNOWN_OPTION", "OPTION_REQUIRED", "OPTION_INVALID"] {
        let harness = Harness::new();
        let config = json!({"serveMode": "refuseStart", "serveRefuseCode": code, "extraCodes": extra});
        let fixture = l0_fixture(&harness, &config, &[&["serve", "--json"]]);
        let output = harness.check(&fixture, true);
        let message = fixture_error(&output, code);
        assert!(message.contains("serve") && message.contains("ended on its own"), "{code}: {message}");
    }
    // The control: a registered code of no such class, refused the same way, is unprovided with its code and the level is not met.
    let harness = Harness::new();
    let config = json!({"serveMode": "refuseStart", "serveRefuseCode": "CURSOR_INVALID", "extraCodes": extra});
    let fixture = l0_fixture(&harness, &config, &[&["serve", "--json"]]);
    let output = harness.check(&fixture, true);
    assert_eq!((output.code, output.envelope()["error"]["code"].as_str(), result_of(&output, "L0.envelope").as_str()), (Some(1), Some("LEVEL_NOT_MET"), "unprovided"), "{}", output.stdout);
}

#[test]
fn atip_check_116_a_not_run_command_that_exits_1_with_a_registered_code_has_ended_on_its_own_and_is_a_fixture_error() {
    // Exit 1 is not a refusal before the work (spec 8.3): the registered precondition code does not make it unprovided.
    let harness = Harness::new();
    let config = json!({"serveMode": "refuseStart", "serveRefuseExit": 1});
    let fixture = l0_fixture(&harness, &config, &[&["serve", "--json"]]);
    let output = harness.check(&fixture, true);
    let message = fixture_error(&output, "exit 1");
    assert!(message.contains("serve") && message.contains("ended on its own"), "{message}");
}

#[test]
fn atip_check_116_a_not_run_refusal_with_a_code_the_command_document_declares_is_a_fixture_error_at_l1_and_unprovided_without_the_document() {
    // `CURSOR_INVALID` is a precondition code for `serve` (the control); declared on a constraint or on a validator of its document it is a refusal
    // of a step that reads no state: the fixture invocation is wrong.
    let declared = [
        ("constraint", json!([{"op": "set", "path": "/constraints", "value": [{"rule": "atLeastOne", "names": ["port"], "code": "CURSOR_INVALID"}]}])),
        ("validator", json!([{"op": "set", "path": "/validators", "value": [{"arguments": ["port"], "code": "CURSOR_INVALID", "description": "The port is free", "rejects": ["{tool} serve --port 1 --json"]}]}])),
    ];
    let harness = Harness::new();
    let config = l1_config(&json!({"serveMode": "refuseStart"}));
    let output = harness.check_l1(&config, &l1_fixture(&harness, &config));
    assert_eq!((output.code, output.envelope()["error"]["code"].as_str()), (Some(1), Some("LEVEL_NOT_MET")), "the control: {}", output.stdout.chars().take(400).collect::<String>());
    for (name, ops) in declared {
        let harness = Harness::new();
        let config = l1_config(&json!({"serveMode": "refuseStart", "docPatch": {"serve": ops}}));
        let output = harness.check_l1(&config, &l1_fixture(&harness, &config));
        let message = fixture_error(&output, name);
        assert!(message.contains("serve") && message.contains("ended on its own"), "{name}: {message}");
    }
}

#[test]
fn atip_check_116_a_not_run_command_whose_effect_note_is_empty_or_blank_is_a_fixture_error_at_l1() {
    for (name, note) in [("empty", ""), ("blank", "  \t ")] {
        let harness = Harness::new();
        let config = l1_config(&json!({"serveMode": "serve", "docPatch": {"serve": [{"op": "set", "path": "/effectNote", "value": note}]}}));
        let output = harness.check_l1(&config, &l1_fixture(&harness, &config));
        let message = fixture_error(&output, name);
        assert!(message.contains("serve") && message.contains("effectNote"), "{name}: {message}");
    }
}
