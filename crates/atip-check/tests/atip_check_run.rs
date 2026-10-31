//! `atip check` as a command: the run, its report, its refusals, the fixture and the levels.
//! Each test carries the id of the scenario it proves; the seeded violations of the 16 checks are in `seeded_violations.rs`.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde_json::{json, Value};

use atip_check::checks::ids;
use atip_check::schema::schemas;
use support::{atip_in, seeded_for, Harness, Output, ATIP, FAKE_TOOL};

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).expect("read_dir").map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

fn check_of<'a>(data: &'a Value, id: &str) -> &'a Value {
    data["checks"].as_array().expect("data.checks").iter().find(|check| check["id"] == id).unwrap_or_else(|| panic!("no check {id}"))
}

fn failing(output: &Output) -> Vec<String> {
    output.results().into_iter().filter(|(_, result)| result != "pass").map(|(id, _)| id).collect()
}

#[test]
fn atip_check_01_a_conforming_tool_passes_the_16_l0_checks_with_exit_0() {
    let harness = Harness::new();
    let output = harness.check_config(&json!({}));
    assert_eq!(output.code, Some(0), "{}{}", output.stdout, output.stderr);
    assert!(output.stderr.is_empty());
    let document = output.envelope();
    assert_eq!((document["ok"].clone(), document["data"]["level"].clone(), document["data"]["passed"].clone()), (json!(true), json!("L0"), json!(true)));
    let results = output.results();
    assert_eq!(results.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ids(), "the 16 L0 ids in the table's order");
    assert!(results.iter().all(|(_, result)| result == "pass"), "{results:?}");
    let must: Vec<&Value> = document["data"]["checks"].as_array().expect("checks").iter().filter(|check| check["strength"] == "MUST").collect();
    assert_eq!(must.len(), 12);
    assert!(must.iter().all(|check| check["result"] == "pass"));
    assert_eq!(document["data"]["checks"].as_array().expect("checks").iter().filter(|check| check["strength"] == "SHOULD").count(), 4);
    for check in document["data"]["checks"].as_array().expect("checks") {
        assert!(check["rules"].as_array().is_some_and(|rules| !rules.is_empty()), "every check names its rule numbers: {check}");
    }
}

#[test]
fn atip_check_02_a_tool_that_breaks_one_rule_exits_1_with_level_not_met_and_the_failing_check_in_data() {
    let harness = Harness::new();
    let output = harness.check_config(&seeded_for("L0.no-null").config);
    assert_eq!(output.code, Some(1));
    assert!(output.stderr.is_empty());
    let document = output.envelope();
    assert_eq!(document["ok"], false);
    assert_eq!(document["error"]["code"], "LEVEL_NOT_MET");
    assert_eq!(document["error"]["retryable"], false);
    let check = check_of(&document["data"], "L0.no-null");
    assert_eq!(check["result"], "fail");
    assert_eq!(check["strength"], "MUST");
    assert_eq!(check["rules"], json!(["6.5"]));
    assert!(check["reason"].as_str().unwrap_or_default().contains("hint"), "the reason names what the schema rejected: {check}");
    assert_eq!(check["observed"][0]["argv"], json!(["nosuch-xyz", "--json"]), "the report names the invocation that decided it");
    assert!(check["observed"][0]["stdout"].as_str().unwrap_or_default().contains("\"hint\":null"));
    assert_eq!(document["data"]["passed"], false);
}

#[test]
fn atip_check_03_both_outcomes_of_atip_check_print_one_envelope_that_passes_the_contract_schema() {
    let harness = Harness::new();
    let passing = harness.check_config(&json!({}));
    let failing = harness.check_config(&seeded_for("L0.bare").config);
    let tmp = harness.path("tmp");
    let refused = atip_in(&harness.path("cwd"), &tmp, &["check", "--level", "L1", "--bin", "x", "--fixture", "y", "--json"]);
    let unknown = atip_in(&harness.path("cwd"), &tmp, &["nosuch-xyz", "--json"]);
    for (output, ok) in [(&passing, true), (&failing, false), (&refused, false), (&unknown, false)] {
        let document = output.envelope();
        assert_eq!(document["ok"], ok);
        let problems = schemas().envelope_problems(&document);
        assert!(problems.is_empty(), "{problems:?}\n{}", output.stdout);
        assert_eq!(document["meta"]["tool"], "atip");
        assert_eq!(document["meta"]["contract"], "atip/1");
        if document["data"].get("checks").is_none() {
            assert!(!output.stdout.contains("null") && !output.stdout.contains("[]"), "no null and no empty array: {}", output.stdout);
        }
    }
    assert_eq!(passing.envelope()["meta"]["command"], "check");
    assert!(unknown.envelope()["meta"].get("command").is_none(), "no word resolved");
}

#[test]
fn atip_check_04_a_binary_that_does_not_exist_or_cannot_run_exits_2_with_binary_unusable() {
    let harness = Harness::new();
    let fixture = harness.write("fixture.json", &harness.fixture(&json!({})));
    let fixture = fixture.to_str().expect("utf8");
    let plain_file = harness.path("not-executable");
    std::fs::write(&plain_file, "#!/bin/sh\n").expect("write");
    std::fs::set_permissions(&plain_file, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let not_a_binary = harness.path("garbage");
    std::fs::write(&not_a_binary, "#!/nonexistent/interpreter\n").expect("write");
    std::fs::set_permissions(&not_a_binary, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let cwd = harness.path("cwd");
    std::fs::create_dir_all(&cwd).expect("cwd");
    let tmp = harness.path("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let missing = harness.path("no-such-binary");
    for bin in [missing.to_str().expect("utf8"), plain_file.to_str().expect("utf8"), harness.dir.path().to_str().expect("utf8"), not_a_binary.to_str().expect("utf8"), "no-such-binary-on-path-xyz"] {
        let output = atip_in(&cwd, &tmp, &["check", "--bin", bin, "--level", "L0", "--fixture", fixture, "--json"]);
        assert_eq!(output.code, Some(2), "{bin}: {}", output.stdout);
        let document = output.envelope();
        assert_eq!((document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some("BINARY_UNUSABLE"), Some("--bin")), "{bin}: {}", output.stdout);
        assert!(document.get("data").is_none(), "a refusal carries no data");
    }
    assert!(listing(&cwd).is_empty() && listing(&tmp).is_empty(), "a refusal writes nothing: {:?} {:?}", listing(&cwd), listing(&tmp));
    // A bare name is found on PATH, as a shell would.
    let dir = Path::new(FAKE_TOOL).parent().expect("dir");
    let found = std::process::Command::new(ATIP).args(["check", "--bin", "atip-fake-tool", "--level", "L0", "--fixture", fixture]).env("PATH", dir).env("TMPDIR", &tmp).current_dir(&cwd).output().expect("run");
    assert_eq!(found.status.code(), Some(0), "{}", String::from_utf8_lossy(&found.stderr));
}

#[test]
fn atip_check_05_text_mode_prints_a_line_per_check_and_a_summary_and_a_failed_level_prints_on_stderr_with_stdout_empty() {
    let harness = Harness::new();
    let passing = harness.check(&harness.fixture(&json!({})), false);
    assert_eq!(passing.code, Some(0));
    assert!(passing.stderr.is_empty());
    assert_eq!(passing.stdout.lines().count(), 17, "16 checks and one summary:\n{}", passing.stdout);
    assert!(passing.stdout.lines().next().expect("line").starts_with("pass       MUST   L0.version (spec 4.6)"), "{}", passing.stdout);
    assert!(passing.stdout.lines().last().expect("line").starts_with("L0 met: 12 MUST passed"), "{}", passing.stdout);
    let failing = harness.check(&harness.fixture(&seeded_for("L0.version").config), false);
    assert_eq!(failing.code, Some(1));
    assert!(failing.stdout.is_empty(), "a failure in text mode prints nothing on stdout: {:?}", failing.stdout);
    assert!(failing.stderr.starts_with("atip: LEVEL_NOT_MET: L0 is not met: 1 MUST checks fail or are unprovided (L0.version)"), "{}", failing.stderr);
    assert!(failing.stderr.contains("fail       MUST   L0.version"), "{}", failing.stderr);
    assert!(failing.stderr.lines().last().expect("line").contains("L0 not met: 11 MUST passed, 1 MUST failed"), "{}", failing.stderr);
}

#[test]
fn atip_check_06_a_malformed_atip_command_line_is_refused_naming_the_argument_at_fault() {
    let harness = Harness::new();
    let (cwd, tmp) = (harness.path("cwd"), harness.path("tmp"));
    std::fs::create_dir_all(&cwd).expect("cwd");
    std::fs::create_dir_all(&tmp).expect("tmp");
    /// argv, the error code, the `field` it names, and the `meta.command` it carries.
    type Case<'a> = (&'a [&'a str], &'a str, Option<&'a str>, Option<&'a str>);
    let cases: [Case; 9] = [
        (&["nosuch-xyz", "--json"], "UNKNOWN_COMMAND", Some("command"), None),
        (&["--json"], "COMMAND_REQUIRED", None, None),
        (&["check", "--bogus-xyz", "--json"], "USAGE", Some("--bogus-xyz"), Some("check")),
        (&["check", "--json", "--bogus-xyz"], "USAGE", Some("--bogus-xyz"), Some("check")),
        (&["check", "--level", "L0", "--fixture", "f", "--json"], "USAGE", Some("--bin"), Some("check")),
        (&["check", "--bin", "b", "--level", "L0", "--json"], "USAGE", Some("--fixture"), Some("check")),
        (&["check", "--bin", "b", "--fixture", "f", "--json"], "USAGE", Some("--level"), Some("check")),
        (&["check", "--bin", "b", "--level", "L0", "--fixture", "f", "--json", "stray"], "USAGE", Some("ARGUMENT"), Some("check")),
        (&["--bogus-xyz", "--json"], "USAGE", Some("--bogus-xyz"), None),
    ];
    for (args, code, field, command) in cases {
        let output = atip_in(&cwd, &tmp, args);
        assert_eq!(output.code, Some(2), "{args:?}: {}", output.stdout);
        let document = output.envelope();
        assert_eq!(document["error"]["code"], code, "{args:?}");
        assert_eq!(document["error"]["field"].as_str(), field, "{args:?}");
        assert_eq!(document["meta"]["command"].as_str(), command, "{args:?}");
        assert_eq!(document["meta"]["schemaVersion"], "1.0");
        assert!(output.stderr.is_empty(), "stderr stays empty in JSON mode: {args:?}");
    }
    let unknown = atip_in(&cwd, &tmp, &["nosuch-xyz", "--json"]).envelope();
    assert_eq!(unknown["error"]["expected"], json!(["check"]));
    // Text mode: nothing on stdout, the error form on stderr; `--json` after `--` is an operand, not a mode.
    for args in [&["nosuch-xyz"][..], &["check", "--bogus-xyz"], &["check", "--", "--json"]] {
        let output = atip_in(&cwd, &tmp, args);
        assert_eq!(output.code, Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {}", output.stdout);
        assert!(output.stderr.starts_with("atip: "), "{args:?}: {}", output.stderr);
    }
    assert!(atip_in(&cwd, &tmp, &["nosuch-xyz"]).stderr.starts_with("atip: UNKNOWN_COMMAND: "));
    assert_eq!(atip_in(&cwd, &tmp, &["--version"]).stdout, format!("atip {}\n", env!("CARGO_PKG_VERSION")));
    assert!(atip_in(&cwd, &tmp, &["--help"]).stdout.starts_with("atip: "));
    assert!(atip_in(&cwd, &tmp, &["check", "--help"]).stdout.contains("--fixture"));
    assert!(listing(&cwd).is_empty() && listing(&tmp).is_empty(), "a refusal writes nothing");
}

#[test]
fn atip_check_23_a_failing_should_check_is_an_advisory_the_level_is_still_met_and_the_exit_is_0() {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"versionFormat": "bare", "textErrorPrefix": "error", "pretty": true, "omitContract": true}));
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    let document = output.envelope();
    assert_eq!((document["ok"].clone(), document["data"]["passed"].clone()), (json!(true), json!(true)));
    let advisories: Vec<&str> = document["data"]["checks"].as_array().expect("checks").iter().filter(|check| check["result"] == "fail").map(|check| check["id"].as_str().unwrap_or_default()).collect();
    assert_eq!(advisories, ["L0.version-format", "L0.text-error-form", "L0.compact", "L0.contract"]);
    assert!(document["data"]["checks"].as_array().expect("checks").iter().filter(|check| check["strength"] == "MUST").all(|check| check["result"] == "pass"));
    let text = harness.check(&harness.fixture(&json!({"versionFormat": "bare"})), false);
    assert_eq!(text.code, Some(0));
    assert!(text.stdout.lines().last().expect("line").contains("1 SHOULD advisories"), "{}", text.stdout);
}

#[test]
fn atip_check_24_the_setup_runs_once_and_every_invocation_starts_from_a_fresh_copy_of_its_state() {
    let harness = Harness::new();
    let log = harness.path("init.log");
    // `dirtyMarker`: every call but `init` fails when the directory already holds what an earlier call left.
    let output = harness.check_config(&json!({"dirtyMarker": true, "initLog": log}));
    assert_eq!(output.code, Some(0), "state leaked between invocations: {}", output.stdout);
    assert_eq!(std::fs::read_to_string(&log).expect("init log"), "init\n", "setup ran once, not once per invocation");
    // Without the fresh copies the same tool would fail: the first run of the marker tool in one directory leaves its marker.
    let work = harness.path("one-directory");
    std::fs::create_dir_all(&work).expect("dir");
    let config = harness.write("marker-config.json", &json!({"dirtyMarker": true}));
    let run = || std::process::Command::new(FAKE_TOOL).arg("status").current_dir(&work).env("FAKE_ATIP_CONFIG", &config).status().expect("run").code();
    assert_eq!((run(), run()), (Some(0), Some(70)));
}

#[test]
fn atip_check_25_a_fixture_without_a_probe_or_a_registry_leaves_the_checks_that_need_them_unprovided_and_fails_the_level() {
    let harness = Harness::new();
    let full = harness.fixture(&json!({}));
    for (drop, unprovided) in [
        ("probe", vec!["L0.unknown-option", "L0.json-mode-preparse", "L0.after-dashdash", "L0.one-document", "L0.no-env-switch", "L0.no-env-switch-text"]),
        ("registry", vec!["L0.unknown-option", "L0.ok-exit"]),
    ] {
        let mut fixture = full.clone();
        fixture.as_object_mut().expect("object").remove(drop);
        let output = harness.check(&fixture, true);
        assert_eq!(output.code, Some(1), "without `{drop}`: {}", output.stdout);
        let document = output.envelope();
        assert_eq!(document["error"]["code"], "LEVEL_NOT_MET");
        let got: Vec<&str> = document["data"]["checks"].as_array().expect("checks").iter().filter(|check| check["result"] == "unprovided").map(|check| check["id"].as_str().unwrap_or_default()).collect();
        assert_eq!(got, unprovided, "without `{drop}`");
        for id in &unprovided {
            let reason = check_of(&document["data"], id)["reason"].as_str().unwrap_or_default().to_string();
            assert!(reason.contains(&format!("`{drop}`")), "{id}: the reason names the missing entry: {reason}");
        }
        assert!(document["data"]["checks"].as_array().expect("checks").iter().all(|check| check["result"] == "pass" || check["result"] == "unprovided"), "nothing else changes");
        assert_eq!(document["data"]["passed"], false);
    }
    let mut neither = full;
    neither.as_object_mut().expect("object").remove("probe");
    neither.as_object_mut().expect("object").remove("registry");
    let output = harness.check(&neither, true);
    assert_eq!((output.code, output.results().iter().filter(|(_, result)| result == "unprovided").count()), (Some(1), 7));
    // A tool that fails a rule is a failure even when the fixture lacks the entry that would name the code.
    let mut bare = harness.fixture(&json!({"exitOutOfRange": true}));
    bare.as_object_mut().expect("object").remove("registry");
    assert_eq!(check_of(&harness.check(&bare, true).envelope()["data"], "L0.ok-exit")["result"], "fail");
}

#[test]
fn atip_check_26_an_invocation_that_runs_over_the_time_limit_fails_its_check_with_the_reason() {
    let harness = Harness::new();
    let mut fixture = harness.fixture(&json!({"hangOnVersion": 30}));
    fixture["timeoutSeconds"] = json!(1);
    let started = std::time::Instant::now();
    let output = harness.check(&fixture, true);
    assert!(started.elapsed() < std::time::Duration::from_secs(15), "the hung tool is killed, not waited for");
    assert_eq!(output.code, Some(1));
    let document = output.envelope();
    let version = check_of(&document["data"], "L0.version");
    assert_eq!(version["result"], "fail");
    assert!(version["reason"].as_str().unwrap_or_default().contains("ran over the 1 second limit"), "{version}");
    assert_eq!(version["observed"][0]["timedOut"], true);
    assert_eq!(failing(&output), ["L0.version", "L0.version-format"], "the other checks are not affected by the slow call; the format reads the same call");
}

#[test]
fn atip_check_27_an_unreadable_or_wrongly_shaped_fixture_or_a_failing_setup_command_exits_2_with_fixture_invalid() {
    let harness = Harness::new();
    let cwd = harness.path("cwd");
    std::fs::create_dir_all(&cwd).expect("cwd");
    let tmp = harness.path("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let valid = harness.fixture(&json!({}));
    let with = |change: &dyn Fn(&mut Value)| {
        let mut fixture = valid.clone();
        change(&mut fixture);
        fixture
    };
    let bad: Vec<(&str, Value)> = vec![
        ("setup is a string", with(&|f| f["setup"] = json!("init"))),
        ("setup command is empty", with(&|f| f["setup"] = json!([[]]))),
        ("env value is a number", with(&|f| f["env"]["X"] = json!(1))),
        ("env sets HOME", with(&|f| f["env"]["HOME"] = json!("/x"))),
        ("timeout is zero", with(&|f| f["timeoutSeconds"] = json!(0))),
        ("probe is not a command id", with(&|f| f["probe"] = json!("Not A Command"))),
        ("registry row has a lower-case code", with(&|f| f["registry"][0]["code"] = json!("usage"))),
        ("registry row has exit 3", with(&|f| f["registry"][0]["exit"] = json!(3))),
        ("registry repeats a code", with(&|f| f["registry"][1]["code"] = json!("USAGE"))),
        ("the fixture is an array", json!([])),
        ("setup command fails", with(&|f| f["setup"] = json!([["nosuch-xyz"]]))),
    ];
    for (name, fixture) in bad {
        let path = harness.write("bad.json", &fixture);
        let output = atip_in(&cwd, &tmp, &["check", "--bin", FAKE_TOOL, "--level", "L0", "--fixture", path.to_str().expect("utf8"), "--json"]);
        assert_eq!(output.code, Some(2), "{name}: {}", output.stdout);
        let document = output.envelope();
        assert_eq!((document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some("FIXTURE_INVALID"), Some("--fixture")), "{name}: {}", output.stdout);
        assert!(document.get("data").is_none(), "{name}");
    }
    let not_json = harness.path("not-json.json");
    std::fs::write(&not_json, "{ nope").expect("write");
    for path in [not_json, harness.path("missing.json"), harness.dir.path().to_path_buf()] {
        let output = atip_in(&cwd, &tmp, &["check", "--bin", FAKE_TOOL, "--level", "L0", "--fixture", path.to_str().expect("utf8"), "--json"]);
        assert_eq!((output.code, output.envelope()["error"]["code"].as_str()), (Some(2), Some("FIXTURE_INVALID")), "{}", path.display());
    }
    assert!(listing(&cwd).is_empty() && listing(&tmp).is_empty(), "a refusal writes nothing, its scratch space included: {:?} {:?}", listing(&cwd), listing(&tmp));
}

#[test]
fn atip_check_29_the_fixture_keys_commands_search_and_domain_rejects_are_ignored_at_l0() {
    let harness = Harness::new();
    let mut fixture = harness.fixture(&json!({}));
    fixture["commands"] = json!({"finding close": {"ok": [["finding", "close"]], "domainRejects": {"tag": ["Core"]}}, "purge": {"preview": 3}});
    fixture["search"] = json!([{"query": "close a review finding", "expect": "finding close"}]);
    fixture["domainRejects"] = json!(null);
    fixture["anyOtherKey"] = json!([1, 2, 3]);
    let with_keys = harness.check(&fixture, true);
    let without = harness.check(&harness.fixture(&json!({})), true);
    assert_eq!((with_keys.code, without.code), (Some(0), Some(0)));
    assert_eq!(with_keys.results(), without.results());
}

#[test]
fn atip_check_30_a_refusal_that_writes_a_file_in_the_working_directory_or_in_home_fails_and_the_report_names_the_paths() {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"refusalWrites": true, "refusalWritesHome": true}));
    assert_eq!(output.code, Some(1));
    let document = output.envelope();
    assert_eq!(failing(&output), ["L0.ok-exit"], "only the check that owns the rule fails");
    let check = check_of(&document["data"], "L0.ok-exit");
    let reason = check["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("./refused.txt") && reason.contains("~/telemetry.log"), "the paths are named, the working directory as ./ and HOME as ~/: {reason}");
    assert!(reason.contains("(exit 2)"), "{reason}");
    assert!(check["observed"][0]["changed"].as_array().is_some_and(|paths| paths.contains(&json!("./refused.txt"))), "{check}");
    assert_eq!(check["rules"], json!(["6.2", "6.4", "7.3.2", "7.3.3", "8.1", "8.2"]));
    let home_only = harness.check_config(&json!({"refusalWritesHome": true}));
    assert!(check_of(&home_only.envelope()["data"], "L0.ok-exit")["reason"].as_str().unwrap_or_default().contains("~/telemetry.log"));
}

#[test]
fn atip_check_31_a_refusal_that_writes_nothing_passes_and_a_successful_call_that_writes_a_file_is_not_judged_at_l0() {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"successWrites": true}));
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    assert!(output.results().iter().all(|(_, result)| result == "pass"));
    let quiet = harness.check_config(&json!({}));
    assert_eq!(check_of(&quiet.envelope()["data"], "L0.ok-exit")["result"], "pass");
}

#[test]
fn atip_check_32_level_l1_and_l2_exit_2_with_level_unsupported_naming_the_release_and_l9_is_usage() {
    let harness = Harness::new();
    let fixture = harness.write("fixture.json", &harness.fixture(&json!({})));
    let (cwd, tmp) = (harness.path("cwd"), harness.path("tmp"));
    std::fs::create_dir_all(&cwd).expect("cwd");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let run = |level: &str, json: bool| {
        let mut args = vec!["check", "--bin", FAKE_TOOL, "--level", level, "--fixture", fixture.to_str().expect("utf8")];
        if json {
            args.push("--json");
        }
        atip_in(&cwd, &tmp, &args)
    };
    for (level, release) in [("L1", "0.34"), ("L2", "0.35")] {
        let output = run(level, true);
        assert_eq!(output.code, Some(2));
        let document = output.envelope();
        assert_eq!(document["error"]["code"], "LEVEL_UNSUPPORTED");
        assert!(document["error"]["hint"].as_str().unwrap_or_default().contains(release), "{level}: {}", document["error"]["hint"]);
        assert_eq!((document["error"]["field"].as_str(), document["error"]["received"].as_str()), (Some("--level"), Some(level)));
        assert_eq!(document["error"]["expected"], json!(["L0"]));
        let text = run(level, false);
        assert_eq!((text.code, text.stdout.is_empty()), (Some(2), true));
        assert!(text.stderr.starts_with("atip: LEVEL_UNSUPPORTED: "), "{}", text.stderr);
        assert!(text.stderr.contains(release));
    }
    for level in ["L9", "0", "l0", ""] {
        let output = run(level, true);
        assert_eq!(output.code, Some(2), "{level:?}");
        let document = output.envelope();
        assert_eq!((document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some("USAGE"), Some("--level")), "{level:?}");
        assert_eq!(document["error"]["expected"], json!(["L0", "L1", "L2"]));
    }
    assert_eq!(run("L0", true).code, Some(0), "the supported level still runs");
}

#[test]
fn atip_check_33_a_refused_level_runs_no_binary_and_writes_nothing() {
    let harness = Harness::new();
    let marker = harness.path("marker");
    let script = harness.path("marks-when-run");
    std::fs::write(&script, format!("#!/bin/sh\ntouch {}\nexit 0\n", marker.display())).expect("write");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let fixture = harness.write("fixture.json", &json!({"setup": [["anything"]], "probe": "status"}));
    let (cwd, tmp) = (harness.path("cwd"), harness.path("tmp"));
    std::fs::create_dir_all(&cwd).expect("cwd");
    std::fs::create_dir_all(&tmp).expect("tmp");
    for level in ["L1", "L2", "L9"] {
        let output = atip_in(&cwd, &tmp, &["check", "--bin", script.to_str().expect("utf8"), "--level", level, "--fixture", fixture.to_str().expect("utf8"), "--json"]);
        assert_eq!(output.code, Some(2), "{level}");
        assert!(!marker.exists(), "{level}: the binary was run");
        assert!(listing(&cwd).is_empty() && listing(&tmp).is_empty(), "{level}: something was written: {:?} {:?}", listing(&cwd), listing(&tmp));
    }
    // A level the checker does run does start the binary, so the marker is the test's proof of the above.
    let output = atip_in(&cwd, &tmp, &["check", "--bin", script.to_str().expect("utf8"), "--level", "L0", "--fixture", fixture.to_str().expect("utf8"), "--json"]);
    assert_eq!(output.code, Some(1), "the script prints nothing, so the level is not met, but it ran");
    assert!(marker.exists(), "L0 starts the binary");
}
