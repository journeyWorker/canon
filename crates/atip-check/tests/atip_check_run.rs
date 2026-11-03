//! `atip check` as a command: the run, its report, its refusals, the fixture and the levels.
//! Each test carries the id of the scenario it proves; the seeded violations of the 28 checks are in `seeded_violations.rs`.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde_json::{json, Value};

use atip_check::checks::ids;
use atip_check::schema::schemas;
use support::{atip_in, Harness, Output, ATIP, FAKE_TOOL};

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read_dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn check_of<'a>(data: &'a Value, id: &str) -> &'a Value {
    data["checks"]
        .as_array()
        .expect("data.checks")
        .iter()
        .find(|check| check["id"] == id)
        .unwrap_or_else(|| panic!("no check {id}"))
}

/// The checks that failed: not those that passed, did not apply or lacked a fixture entry.
fn failing(output: &Output) -> Vec<String> {
    output
        .results()
        .into_iter()
        .filter(|(_, result)| result == "fail")
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn atip_check_01_a_conforming_1_1_tool_passes_the_27_applicable_of_the_28_l0_checks_with_exit_0() {
    let harness = Harness::new();
    let output = harness.check_config(&json!({}));
    assert_eq!(output.code, Some(0), "{}{}", output.stdout, output.stderr);
    assert!(output.stderr.is_empty());
    let document = output.envelope();
    assert_eq!(
        (
            document["ok"].clone(),
            document["data"]["level"].clone(),
            document["data"]["passed"].clone()
        ),
        (json!(true), json!("L0"), json!(true))
    );
    let results = output.results();
    assert_eq!(
        results
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        ids().into_iter().chain(["L2.effects"]).collect::<Vec<_>>(),
        "the 28 L0 ids in the table's order, and L2.effects, which every level reports"
    );
    // L0.contract is a 1.0 rule: for a claim of 1.1 L0.contract-version owns the presence of the marker.
    assert!(
        results
            .iter()
            .all(|(id, result)| if id == "L0.contract" { result == "unsupported" } else { result == "pass" }),
        "{results:?}"
    );
    assert_eq!(document["data"]["contractVersion"], "1.1");
    let must: Vec<&Value> = document["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .filter(|check| check["strength"] == "MUST")
        .collect();
    assert_eq!(must.len(), 21);
    assert!(must.iter().all(|check| check["result"] == "pass"));
    assert_eq!(
        document["data"]["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .filter(|check| check["strength"] == "SHOULD")
            .count(),
        8
    );
    for check in document["data"]["checks"].as_array().expect("checks") {
        assert!(
            check["rules"]
                .as_array()
                .is_some_and(|rules| !rules.is_empty()),
            "every check names its rule numbers: {check}"
        );
    }
}

#[test]
fn atip_check_02_a_tool_that_breaks_one_rule_exits_1_with_level_not_met_and_the_failing_check_in_data(
) {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"hintNull": true}));
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
    assert!(
        check["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("hint"),
        "the reason names what the schema rejected: {check}"
    );
    assert_eq!(
        check["observed"][0]["argv"],
        json!(["nosuch-xyz", "--json"]),
        "the report names the invocation that decided it"
    );
    assert!(check["observed"][0]["stdout"]
        .as_str()
        .unwrap_or_default()
        .contains("\"hint\":null"));
    assert_eq!(document["data"]["passed"], false);
}

#[test]
fn atip_check_03_both_outcomes_of_atip_check_print_one_envelope_that_passes_the_contract_schema() {
    let harness = Harness::new();
    let passing = harness.check_config(&json!({}));
    let failing = harness.check_config(&json!({"bareCode": "USAGE"}));
    let tmp = harness.path("tmp");
    let refused = atip_in(
        &harness.path("cwd"),
        &tmp,
        &[
            "check",
            "--level",
            "L1",
            "--bin",
            "x",
            "--fixture",
            "y",
            "--json",
        ],
    );
    let unknown = atip_in(&harness.path("cwd"), &tmp, &["nosuch-xyz", "--json"]);
    for (output, ok) in [
        (&passing, true),
        (&failing, false),
        (&refused, false),
        (&unknown, false),
    ] {
        let document = output.envelope();
        assert_eq!(document["ok"], ok);
        let problems = schemas().envelope_problems(&document);
        assert!(problems.is_empty(), "{problems:?}\n{}", output.stdout);
        assert_eq!(document["meta"]["tool"], "atip");
        assert_eq!(document["meta"]["contract"], "atip/1");
        if document["data"].get("checks").is_none() {
            assert!(
                !output.stdout.contains("null") && !output.stdout.contains("[]"),
                "no null and no empty array: {}",
                output.stdout
            );
        }
    }
    assert_eq!(passing.envelope()["meta"]["command"], "check");
    assert!(
        unknown.envelope()["meta"].get("command").is_none(),
        "no word resolved"
    );
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
    for bin in [
        missing.to_str().expect("utf8"),
        plain_file.to_str().expect("utf8"),
        harness.dir.path().to_str().expect("utf8"),
        not_a_binary.to_str().expect("utf8"),
        "no-such-binary-on-path-xyz",
    ] {
        let output = atip_in(
            &cwd,
            &tmp,
            &[
                "check",
                "--bin",
                bin,
                "--level",
                "L0",
                "--fixture",
                fixture,
                "--json",
            ],
        );
        assert_eq!(output.code, Some(2), "{bin}: {}", output.stdout);
        let document = output.envelope();
        assert_eq!(
            (
                document["error"]["code"].as_str(),
                document["error"]["field"].as_str()
            ),
            (Some("BINARY_UNUSABLE"), Some("--bin")),
            "{bin}: {}",
            output.stdout
        );
        assert!(document.get("data").is_none(), "a refusal carries no data");
    }
    assert!(
        listing(&cwd).is_empty() && listing(&tmp).is_empty(),
        "a refusal writes nothing: {:?} {:?}",
        listing(&cwd),
        listing(&tmp)
    );
    // A bare name is found on PATH, as a shell would.
    let dir = Path::new(FAKE_TOOL).parent().expect("dir");
    let found = std::process::Command::new(ATIP)
        .args([
            "check",
            "--bin",
            "atip-fake-tool",
            "--level",
            "L0",
            "--fixture",
            fixture,
        ])
        .env("PATH", dir)
        .env("TMPDIR", &tmp)
        .current_dir(&cwd)
        .output()
        .expect("run");
    assert_eq!(
        found.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&found.stderr)
    );
}

#[test]
fn atip_check_05_text_mode_prints_a_line_per_check_and_a_summary_and_a_failed_level_prints_on_stderr_with_stdout_empty(
) {
    let harness = Harness::new();
    let passing = harness.check(&harness.fixture(&json!({})), false);
    assert_eq!(passing.code, Some(0));
    assert!(passing.stderr.is_empty());
    assert_eq!(
        passing.stdout.lines().count(),
        30,
        "29 checks and one summary:\n{}",
        passing.stdout
    );
    assert!(
        passing
            .stdout
            .lines()
            .next()
            .expect("line")
            .starts_with("pass       MUST   L0.version (spec 4.6)"),
        "{}",
        passing.stdout
    );
    assert!(
        passing
            .stdout
            .lines()
            .last()
            .expect("line")
            .starts_with("L0 met: 21 MUST passed"),
        "{}",
        passing.stdout
    );
    let failing = harness.check(&harness.fixture(&json!({"versionExit": 1})), false);
    assert_eq!(failing.code, Some(1));
    assert!(
        failing.stdout.is_empty(),
        "a failure in text mode prints nothing on stdout: {:?}",
        failing.stdout
    );
    assert!(
        failing.stderr.starts_with(
            "atip: LEVEL_NOT_MET: L0 is not met: 1 MUST checks fail or are unprovided (L0.version)"
        ),
        "{}",
        failing.stderr
    );
    assert!(
        failing.stderr.contains("fail       MUST   L0.version"),
        "{}",
        failing.stderr
    );
    assert!(
        failing
            .stderr
            .lines()
            .last()
            .expect("line")
            .contains("L0 not met: 20 MUST passed, 1 MUST failed"),
        "{}",
        failing.stderr
    );
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
        (
            &["nosuch-xyz", "--json"],
            "UNKNOWN_COMMAND",
            Some("command"),
            None,
        ),
        (&["--json"], "COMMAND_REQUIRED", None, None),
        (
            &["check", "--bogus-xyz", "--json"],
            "USAGE",
            Some("--bogus-xyz"),
            Some("check"),
        ),
        (
            &["check", "--json", "--bogus-xyz"],
            "USAGE",
            Some("--bogus-xyz"),
            Some("check"),
        ),
        (
            &["check", "--level", "L0", "--fixture", "f", "--json"],
            "USAGE",
            Some("--bin"),
            Some("check"),
        ),
        (
            &["check", "--bin", "b", "--level", "L0", "--json"],
            "USAGE",
            Some("--fixture"),
            Some("check"),
        ),
        (
            &["check", "--bin", "b", "--fixture", "f", "--json"],
            "USAGE",
            Some("--level"),
            Some("check"),
        ),
        (
            &[
                "check",
                "--bin",
                "b",
                "--level",
                "L0",
                "--fixture",
                "f",
                "--json",
                "stray",
            ],
            "USAGE",
            Some("ARGUMENT"),
            Some("check"),
        ),
        (
            &["--bogus-xyz", "--json"],
            "USAGE",
            Some("--bogus-xyz"),
            None,
        ),
    ];
    for (args, code, field, command) in cases {
        let output = atip_in(&cwd, &tmp, args);
        assert_eq!(output.code, Some(2), "{args:?}: {}", output.stdout);
        let document = output.envelope();
        assert_eq!(document["error"]["code"], code, "{args:?}");
        assert_eq!(document["error"]["field"].as_str(), field, "{args:?}");
        assert_eq!(document["meta"]["command"].as_str(), command, "{args:?}");
        assert_eq!(document["meta"]["schemaVersion"], "1.0");
        assert!(
            output.stderr.is_empty(),
            "stderr stays empty in JSON mode: {args:?}"
        );
    }
    let unknown = atip_in(&cwd, &tmp, &["nosuch-xyz", "--json"]).envelope();
    assert_eq!(unknown["error"]["expected"], json!(["check"]));
    // Text mode: nothing on stdout, the error form on stderr; `--json` after `--` is an operand, not a mode.
    for args in [
        &["nosuch-xyz"][..],
        &["check", "--bogus-xyz"],
        &["check", "--", "--json"],
    ] {
        let output = atip_in(&cwd, &tmp, args);
        assert_eq!(output.code, Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {}", output.stdout);
        assert!(
            output.stderr.starts_with("atip: "),
            "{args:?}: {}",
            output.stderr
        );
    }
    assert!(atip_in(&cwd, &tmp, &["nosuch-xyz"])
        .stderr
        .starts_with("atip: UNKNOWN_COMMAND: "));
    assert_eq!(
        atip_in(&cwd, &tmp, &["--version"]).stdout,
        format!("atip {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(atip_in(&cwd, &tmp, &["--help"])
        .stdout
        .starts_with("atip: "));
    assert!(atip_in(&cwd, &tmp, &["check", "--help"])
        .stdout
        .contains("--fixture"));
    assert!(
        listing(&cwd).is_empty() && listing(&tmp).is_empty(),
        "a refusal writes nothing"
    );
}

#[test]
fn atip_check_23_a_failing_should_check_is_a_recommendation_the_level_is_still_met_and_the_exit_is_0(
) {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"claims": "1.0", "versionFormat": "bare", "textErrorPrefix": "error", "pretty": true, "omitContract": true, "noiseStderrFor": ["status"], "usageNoHint": true, "groupRejectsGlobal": true}));
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    let document = output.envelope();
    assert_eq!(
        (document["ok"].clone(), document["data"]["passed"].clone()),
        (json!(true), json!(true))
    );
    let advisories: Vec<&str> = document["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .filter(|check| check["result"] == "fail")
        .map(|check| check["id"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        advisories,
        [
            "L0.version-format",
            "L0.text-error-form",
            "L0.compact",
            "L0.contract",
            "L0.stderr-quiet",
            "L0.usage-hint",
            "L0.global-on-group"
        ]
    );
    assert_eq!(
        document["meta"]["warnings"][0]["code"], "SHOULD_FAILED",
        "a passed level with advisories says so in meta.warnings: {document}"
    );
    assert!(document["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .filter(|check| check["strength"] == "MUST")
        .all(|check| matches!(check["result"].as_str(), Some("pass" | "unsupported"))));
    let text = harness.check(&harness.fixture(&json!({"versionFormat": "bare"})), false);
    assert_eq!(text.code, Some(0));
    assert!(
        text.stdout
            .lines()
            .last()
            .expect("line")
            .contains("1 SHOULD advisories"),
        "{}",
        text.stdout
    );
}

#[test]
fn atip_check_24_the_setup_runs_once_and_every_invocation_starts_from_a_fresh_copy_of_its_state() {
    let harness = Harness::new();
    let log = harness.path("init.log");
    // `dirtyMarker`: every call but `init` fails when the directory already holds what an earlier call left.
    let output = harness.check_config(&json!({"dirtyMarker": true, "initLog": log}));
    assert_eq!(
        output.code,
        Some(0),
        "state leaked between invocations: {}",
        output.stdout
    );
    assert_eq!(
        std::fs::read_to_string(&log).expect("init log"),
        "init\n",
        "setup ran once, not once per invocation"
    );
    // Without the fresh copies the same tool would fail: the first run of the marker tool in one directory leaves its marker.
    let work = harness.path("one-directory");
    std::fs::create_dir_all(&work).expect("dir");
    let config = harness.write("marker-config.json", &json!({"dirtyMarker": true}));
    let run = || {
        std::process::Command::new(FAKE_TOOL)
            .arg("status")
            .current_dir(&work)
            .env("FAKE_ATIP_CONFIG", &config)
            .status()
            .expect("run")
            .code()
    };
    assert_eq!((run(), run()), (Some(0), Some(70)));
}

#[test]
fn atip_check_25_a_fixture_without_a_probe_a_registry_or_commands_leaves_the_checks_that_need_them_unprovided_and_fails_the_level(
) {
    let harness = Harness::new();
    let unprovided_of = |output: &Output| -> Vec<String> {
        output
            .results()
            .into_iter()
            .filter(|(_, result)| result == "unprovided")
            .map(|(id, _)| id)
            .collect()
    };
    // (the config of the fake tool, the fixture entry dropped, the checks that are then unprovided)
    for (config, drop, unprovided) in [
        (
            json!({}),
            "probe",
            vec![
                "L0.unknown-option",
                "L0.json-mode-preparse",
                "L0.after-dashdash",
                "L0.one-document",
                "L0.no-env-switch",
                "L0.no-env-switch-text",
            ],
        ),
        // The tool prints its registry: only the comparison with the fixture is unprovided (spec 17.3).
        (json!({}), "registry", vec!["L0.registry"]),
        // A tool that prints none has the fixture's registry or nothing to judge exits by.
        (json!({"noSchema": true}), "registry", vec!["L0.ok-exit"]),
        // The probe answers with a page, so L0.page has one; nothing refuses a missing value without `commands`.
        (json!({}), "commands", vec!["L0.no-prompt", "L0.global-on-group", "L2.effects"]),
    ] {
        let full = harness.fixture(&config);
        let mut fixture = full.clone();
        fixture.as_object_mut().expect("object").remove(drop);
        let output = harness.check(&fixture, true);
        assert_eq!(output.code, Some(1), "without `{drop}`: {}", output.stdout);
        let document = output.envelope();
        assert_eq!(document["error"]["code"], "LEVEL_NOT_MET");
        assert_eq!(unprovided_of(&output), unprovided, "without `{drop}`");
        for id in &unprovided {
            let reason = check_of(&document["data"], id)["reason"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            assert!(
                reason.contains(&format!("`{drop}`")),
                "{id}: the reason names the missing entry: {reason}"
            );
        }
        assert!(
            document["data"]["checks"]
                .as_array()
                .expect("checks")
                .iter()
                .all(|check| matches!(
                    check["result"].as_str(),
                    Some("pass" | "unprovided" | "unsupported")
                )),
            "nothing else changes: {}",
            output.stdout
        );
        assert_eq!(document["data"]["passed"], false);
    }
    let mut neither = harness.fixture(&json!({}));
    for key in ["probe", "registry", "commands"] {
        neither.as_object_mut().expect("object").remove(key);
    }
    let output = harness.check(&neither, true);
    assert_eq!((output.code, unprovided_of(&output).len()), (Some(1), 11), "ten checks of L0 and L2.effects");
    // A SHOULD check that lacks its entry (`globalOptions`) is unprovided and fails nothing.
    let mut without_global = harness.fixture(&json!({}));
    without_global.as_object_mut().expect("object").remove("globalOptions");
    let output = harness.check(&without_global, true);
    assert_eq!((output.code, unprovided_of(&output)), (Some(0), vec!["L0.global-on-group".to_string()]));
}

#[test]
fn atip_check_52_a_violation_the_run_observed_is_a_failure_even_when_the_fixture_lacks_the_entry() {
    let harness = Harness::new();
    // A tool that fails a rule is a failure even when the fixture lacks the entry that would name the code.
    let mut bare = harness.fixture(&json!({"exitOutOfRange": true, "noSchema": true}));
    bare.as_object_mut().expect("object").remove("registry");
    assert_eq!(
        check_of(&harness.check(&bare, true).envelope()["data"], "L0.ok-exit")["result"],
        "fail",
        "an exit of 3 is wrong whatever the registry"
    );
    // `L0.registry` fails on a code the tool's own registry lacks, with no fixture registry at all.
    let mut no_registry = harness.fixture(&json!({"unknownOptionCode": "NOT_REGISTERED"}));
    no_registry.as_object_mut().expect("object").remove("registry");
    assert_eq!(
        check_of(&harness.check(&no_registry, true).envelope()["data"], "L0.registry")["result"],
        "fail"
    );
    // A fixture without `commands`: a `missingValue` the run never sees is unprovided, not a pass.
    let mut no_commands = harness.fixture(&json!({}));
    no_commands.as_object_mut().expect("object").remove("commands");
    assert_eq!(
        check_of(&harness.check(&no_commands, true).envelope()["data"], "L0.no-prompt")["result"],
        "unprovided"
    );
}

#[test]
fn atip_check_26_an_invocation_that_runs_over_the_time_limit_fails_its_check_with_the_reason() {
    let harness = Harness::new();
    let mut fixture = harness.fixture(&json!({"hangOnVersion": 30}));
    fixture["timeoutSeconds"] = json!(1);
    let started = std::time::Instant::now();
    let output = harness.check(&fixture, true);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(15),
        "the hung tool is killed, not waited for"
    );
    assert_eq!(output.code, Some(1));
    let document = output.envelope();
    let version = check_of(&document["data"], "L0.version");
    assert_eq!(version["result"], "fail");
    assert!(
        version["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("ran over the 1 second limit"),
        "{version}"
    );
    assert_eq!(version["observed"][0]["timedOut"], true);
    assert_eq!(
        failing(&output),
        ["L0.version"],
        "the format check reads the same call but the call is the version check's"
    );
}

#[test]
fn atip_check_27_an_unreadable_or_wrongly_shaped_fixture_or_a_failing_setup_command_exits_2_with_fixture_invalid(
) {
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
        (
            "setup command is empty",
            with(&|f| f["setup"] = json!([[]])),
        ),
        ("env value is a number", with(&|f| f["env"]["X"] = json!(1))),
        ("env sets HOME", with(&|f| f["env"]["HOME"] = json!("/x"))),
        ("timeout is zero", with(&|f| f["timeoutSeconds"] = json!(0))),
        (
            "probe is not a command id",
            with(&|f| f["probe"] = json!("Not A Command")),
        ),
        (
            "registry row has a lower-case code",
            with(&|f| f["registry"][0]["code"] = json!("usage")),
        ),
        (
            "registry row has exit 3",
            with(&|f| f["registry"][0]["exit"] = json!(3)),
        ),
        (
            "registry repeats a code",
            with(&|f| f["registry"][1]["code"] = json!("USAGE")),
        ),
        ("the fixture is an array", json!([])),
        (
            "setup command fails",
            with(&|f| f["setup"] = json!([["nosuch-xyz"]])),
        ),
    ];
    for (name, fixture) in bad {
        let path = harness.write("bad.json", &fixture);
        let output = atip_in(
            &cwd,
            &tmp,
            &[
                "check",
                "--bin",
                FAKE_TOOL,
                "--level",
                "L0",
                "--fixture",
                path.to_str().expect("utf8"),
                "--json",
            ],
        );
        assert_eq!(output.code, Some(2), "{name}: {}", output.stdout);
        let document = output.envelope();
        assert_eq!(
            (
                document["error"]["code"].as_str(),
                document["error"]["field"].as_str()
            ),
            (Some("FIXTURE_INVALID"), Some("--fixture")),
            "{name}: {}",
            output.stdout
        );
        assert!(document.get("data").is_none(), "{name}");
    }
    let not_json = harness.path("not-json.json");
    std::fs::write(&not_json, "{ nope").expect("write");
    for path in [
        not_json,
        harness.path("missing.json"),
        harness.dir.path().to_path_buf(),
    ] {
        let output = atip_in(
            &cwd,
            &tmp,
            &[
                "check",
                "--bin",
                FAKE_TOOL,
                "--level",
                "L0",
                "--fixture",
                path.to_str().expect("utf8"),
                "--json",
            ],
        );
        assert_eq!(
            (output.code, output.envelope()["error"]["code"].as_str()),
            (Some(2), Some("FIXTURE_INVALID")),
            "{}",
            path.display()
        );
    }
    assert!(
        listing(&cwd).is_empty() && listing(&tmp).is_empty(),
        "a refusal writes nothing, its scratch space included: {:?} {:?}",
        listing(&cwd),
        listing(&tmp)
    );
}

#[test]
fn atip_check_29_the_l0_fixture_keys_are_read_and_the_keys_of_l1_and_l2_are_ignored() {
    let harness = Harness::new();
    let mut fixture = harness.fixture(&json!({}));
    // L1 and L2 keys of a command entry, a command entry the L0 checks do not need, and top-level members the checker does not know.
    fixture["commands"]["items"]["domainRejects"] = json!({"tag": ["Core"]});
    fixture["commands"]["items"]["fields"] = json!([["id"]]);
    // The L2 keys are read at L2 (and checked for their shape wherever the fixture is read) and are not run at L0.
    fixture["commands"]["items"]["batchFailsAt"] = json!([{"input": [{"bogus": 1}], "index": 0}]);
    fixture["commands"]["purge"] = json!({"preview": [["purge", "--json"]], "async": {"start": ["purge", "--json"], "worker": "W", "outcomes": ["done"]}, "allowedEffects": [".x"]});
    fixture["search"] = json!([{"query": "close a review finding", "expect": "finding close"}]);
    fixture["domainRejects"] = json!(null);
    fixture["anyOtherKey"] = json!([1, 2, 3]);
    let with_keys = harness.check(&fixture, true);
    let without = harness.check(&harness.fixture(&json!({})), true);
    assert_eq!((with_keys.code, without.code), (Some(0), Some(0)), "{}", with_keys.stdout);
    assert_eq!(with_keys.results().len(), without.results().len());
    // `purge` has no `ok`, `refused` or `missingValue`: the run makes no call for it but the cursor probe.
    let results = |output: &Output| -> Vec<(String, String)> {
        output
            .results()
            .into_iter()
            .filter(|(id, _)| id != "L0.global-on-group")
            .collect()
    };
    assert_eq!(results(&with_keys), results(&without));
    // The claim comes from the fixture's `contractVersion`: the same fixture without it is a claim of 1.0, which the 1.1 tool breaks.
    let mut unclaimed = harness.fixture(&json!({}));
    unclaimed.as_object_mut().expect("object").remove("contractVersion");
    let output = harness.check(&unclaimed, true);
    assert_eq!(output.code, Some(1), "{}", output.stdout);
    assert_eq!(output.check("L0.contract-version")["result"], "fail");
    assert_eq!(output.envelope()["data"]["contractVersion"], "1.0");
}

#[test]
fn atip_check_31_a_refusal_that_writes_nothing_passes_and_a_successful_call_that_writes_a_file_is_not_judged_at_l0(
) {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"successWrites": true}));
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    assert!(output
        .results()
        .iter()
        .all(|(id, result)| result == "pass" || (id == "L0.contract" && result == "unsupported")));
    let quiet = harness.check_config(&json!({}));
    assert_eq!(
        check_of(&quiet.envelope()["data"], "L0.ok-exit")["result"],
        "pass"
    );
}

#[test]
fn atip_check_32_level_l2_runs_the_checks_and_is_never_unsupported_and_l9_is_usage()
{
    let harness = Harness::new();
    let fixture = harness.write("fixture.json", &harness.fixture(&json!({})));
    let (cwd, tmp) = (harness.path("cwd"), harness.path("tmp"));
    std::fs::create_dir_all(&cwd).expect("cwd");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let run = |level: &str, json: bool| {
        let mut args = vec![
            "check",
            "--bin",
            FAKE_TOOL,
            "--level",
            level,
            "--fixture",
            fixture.to_str().expect("utf8"),
        ];
        if json {
            args.push("--json");
        }
        atip_in(&cwd, &tmp, &args)
    };
    // A tool that prints no tree of level L2 is a tool whose level is not met; the level itself is known.
    let output = run("L2", true);
    assert_eq!(output.code, Some(1), "{}", output.stdout);
    let document = output.envelope();
    assert_eq!(document["error"]["code"], "LEVEL_NOT_MET");
    assert_eq!(document["data"]["level"], "L2");
    let text = run("L2", false);
    assert!(text.stderr.starts_with("atip: LEVEL_NOT_MET: "), "{}", text.stderr);
    for level in ["L9", "0", "l0", ""] {
        let output = run(level, true);
        assert_eq!(output.code, Some(2), "{level:?}");
        let document = output.envelope();
        assert_eq!(
            (
                document["error"]["code"].as_str(),
                document["error"]["field"].as_str()
            ),
            (Some("USAGE"), Some("--level")),
            "{level:?}"
        );
        assert_eq!(document["error"]["expected"], json!(["L0", "L1", "L2"]));
    }
    assert_eq!(
        run("L0", true).code,
        Some(0),
        "the supported level still runs"
    );
}

#[test]
fn atip_check_33_a_refused_level_runs_no_binary_and_writes_nothing() {
    let harness = Harness::new();
    let marker = harness.path("marker");
    let script = harness.path("marks-when-run");
    std::fs::write(
        &script,
        format!("#!/bin/sh\ntouch {}\nexit 0\n", marker.display()),
    )
    .expect("write");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let fixture = harness.write(
        "fixture.json",
        &json!({"setup": [["anything"]], "probe": "status"}),
    );
    let (cwd, tmp) = (harness.path("cwd"), harness.path("tmp"));
    std::fs::create_dir_all(&cwd).expect("cwd");
    std::fs::create_dir_all(&tmp).expect("tmp");
    for level in ["L3", "L9"] {
        let output = atip_in(
            &cwd,
            &tmp,
            &[
                "check",
                "--bin",
                script.to_str().expect("utf8"),
                "--level",
                level,
                "--fixture",
                fixture.to_str().expect("utf8"),
                "--json",
            ],
        );
        assert_eq!(output.code, Some(2), "{level}");
        assert!(!marker.exists(), "{level}: the binary was run");
        assert!(
            listing(&cwd).is_empty() && listing(&tmp).is_empty(),
            "{level}: something was written: {:?} {:?}",
            listing(&cwd),
            listing(&tmp)
        );
    }
    // A level the checker does run does start the binary, so the marker is the test's proof of the above.
    let output = atip_in(
        &cwd,
        &tmp,
        &[
            "check",
            "--bin",
            script.to_str().expect("utf8"),
            "--level",
            "L0",
            "--fixture",
            fixture.to_str().expect("utf8"),
            "--json",
        ],
    );
    assert_eq!(
        output.code,
        Some(1),
        "the script prints nothing, so the level is not met, but it ran"
    );
    assert!(marker.exists(), "L0 starts the binary");
}

fn result_of(output: &Output, id: &str) -> String {
    check_of(&output.envelope()["data"], id)["result"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn atip_check_46_l0_next_and_l0_page_do_not_apply_when_no_envelope_carries_a_next_step_or_a_page() {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"noNextNoPage": true, "noPaging": true}));
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    assert_eq!(
        (result_of(&output, "L0.next"), result_of(&output, "L0.page")),
        ("unsupported".to_string(), "unsupported".to_string())
    );
    let document = output.envelope();
    assert_eq!(
        document["data"]["passed"], true,
        "a check that does not apply never fails the level"
    );
    assert!(output
        .results()
        .iter()
        .filter(|(id, _)| id != "L0.next" && id != "L0.page" && id != "L0.contract")
        .all(|(_, result)| result == "pass"));
    // With a next step and a page in the probe's answer both are judged.
    let judged = harness.check_config(&json!({}));
    assert_eq!(
        (result_of(&judged, "L0.next"), result_of(&judged, "L0.page")),
        ("pass".to_string(), "pass".to_string())
    );
    assert_eq!(
        check_of(&judged.envelope()["data"], "L0.next")["rules"],
        json!(["6.8"])
    );
    assert_eq!(
        check_of(&judged.envelope()["data"], "L0.page")["rules"],
        json!(["6.9"])
    );
}

#[test]
fn atip_check_73_whether_the_tool_pages_is_decided_by_asking_each_command_for_a_cursor_and_not_by_the_fixture() {
    let harness = Harness::new();
    // The tool pages (`items --cursor x` is `CURSOR_INVALID`) but no answer carried a page: the paged call is missing.
    let withheld = harness.check_config(&json!({"noNextNoPage": true}));
    assert_eq!((withheld.code, result_of(&withheld, "L0.page")), (Some(1), "unprovided".to_string()));
    assert!(
        check_of(&withheld.envelope()["data"], "L0.page")["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("list an invocation that pages under `paged`")
    );
    // A tool that pages and shows no page in a fixture that lists none is unprovided as well, with `commands` or without.
    let mut no_commands = harness.fixture(&json!({"noNextNoPage": true}));
    no_commands.as_object_mut().expect("object").remove("commands");
    let output = harness.check(&no_commands, true);
    assert_eq!(result_of(&output, "L0.page"), "unprovided");
    assert!(check_of(&output.envelope()["data"], "L0.page")["reason"]
        .as_str()
        .unwrap_or_default()
        .contains("`commands`"));
    // The paging command is listed without an `ok` invocation that pages, next to one that does not page: asked for a cursor it says
    // CURSOR_INVALID, so the tool pages and the missing paged call is unprovided, never unsupported.
    let mut listed = harness.fixture(&json!({"noNextNoPage": true}));
    listed["commands"] = json!({
        "items": {"refused": [["items", "--bogus-xyz", "--json"]]},
        "team add": {"ok": [["team", "add", "--json"]]}
    });
    let output = harness.check(&listed, true);
    assert_eq!((output.code, result_of(&output, "L0.page")), (Some(1), "unprovided".to_string()), "{}", output.stdout);
    // A paging command listed without an `ok` whose cursor probe is inconclusive (it needs an operand first) is not a refusal of
    // `--cursor`: another command's refusal is not enough for unsupported.
    let mut inconclusive = harness.fixture(&json!({"noNextNoPage": true, "cursorNeedsName": true}));
    inconclusive["commands"] = json!({
        "items": {"refused": [["items", "--bogus-xyz", "--json"]]},
        "team add": {"ok": [["team", "add", "--json"]]}
    });
    let output = harness.check(&inconclusive, true);
    assert_eq!((output.code, result_of(&output, "L0.page")), (Some(1), "unprovided".to_string()), "{}", output.stdout);
    // A tool that refuses `--cursor` as an option it does not have, for every command the fixture lists, does not page.
    let never = harness.check_config(&json!({"noNextNoPage": true, "noPaging": true}));
    assert_eq!((never.code, result_of(&never, "L0.page")), (Some(0), "unsupported".to_string()));
    // The cursor probe ran: `items --json --cursor x`.
    assert!(
        never.envelope()["data"]["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .any(|check| check["id"] == "L0.ok-exit" || check["id"] == "L0.envelope"),
    );
}

#[test]
fn atip_check_47_stderr_output_is_a_recommendation_and_an_error_or_result_on_stderr_fails_its_check(
) {
    let harness = Harness::new();
    let noisy = harness.check_config(&json!({"noiseStderrFor": ["unknown-command", "bare", "unknown-option", "status"]}));
    assert_eq!(
        noisy.code,
        Some(0),
        "stderr noise alone fails nothing: {}",
        noisy.stdout
    );
    assert_eq!(failing(&noisy), vec!["L0.stderr-quiet".to_string()], "{}", noisy.stdout);
    let quiet = check_of(&noisy.envelope()["data"], "L0.stderr-quiet").clone();
    assert_eq!(quiet["strength"], "SHOULD");
    assert!(quiet["reason"].as_str().unwrap_or_default().contains("warming up"), "{quiet}");
    assert_eq!(noisy.envelope()["meta"]["warnings"][0]["code"], "SHOULD_FAILED");
    // The version output is stdout only: stderr there fails L0.version, a MUST.
    let version = harness.check_config(&json!({"versionStderr": true}));
    assert_eq!((version.code, failing(&version)), (Some(1), vec!["L0.version".to_string()]));
    // The error or the result on stderr fails L0.stderr-result; L0.stderr-quiet leaves it to it.
    let carried = harness.check_config(&json!({"mirrorStderrFor": ["unknown-command"]}));
    assert_eq!(
        (carried.code, failing(&carried)),
        (Some(1), vec!["L0.stderr-result".to_string()])
    );
    // So does the error on stderr with nothing on stdout: a tool that prints its refusal only there.
    let only_stderr = harness.check_config(&json!({"mirrorStderrFor": ["status"], "probeNotEnvelope": true}));
    assert!(failing(&only_stderr).contains(&"L0.stderr-result".to_string()), "{}", only_stderr.stdout);
}

#[test]
fn atip_check_48_a_signal_exit_of_either_form_is_accepted_and_an_exit_of_3_is_not() {
    let harness = Harness::new();
    for (config, form) in [
        (json!({"dashdashSignal": true}), "wait status"),
        (json!({"dashdashExit130": true}), "128+n"),
    ] {
        let output = harness.check_config(&config);
        assert_eq!(output.code, Some(0), "{form}: {}", output.stdout);
        assert!(
            output
                .results()
                .iter()
                .all(|(id, result)| result == "pass" || (id == "L0.contract" && result == "unsupported")),
            "{form}"
        );
        let observed =
            check_of(&output.envelope()["data"], "L0.after-dashdash")["observed"][0].clone();
        match form {
            "wait status" => assert_eq!(observed["signal"], 15, "{observed}"),
            _ => assert_eq!(observed["exit"], 130, "{observed}"),
        }
    }
    let three = harness.check_config(&json!({"dashdashExit3": true}));
    assert_eq!(
        (three.code, failing(&three)),
        (Some(1), vec!["L0.ok-exit".to_string()])
    );
    let reason = check_of(&three.envelope()["data"], "L0.ok-exit")["reason"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(reason.contains("exited 3"), "{reason}");
}

#[test]
fn atip_check_49_the_registries_the_tool_prints_are_compared_with_the_fixtures_and_one_it_does_not_print_is_not(
) {
    let harness = Harness::new();
    let offered = harness.check_config(&json!({}));
    assert_eq!(offered.code, Some(0), "{}", offered.stdout);
    assert_eq!(result_of(&offered, "L0.registry"), "pass");
    let reason_of = |output: &Output| {
        check_of(&output.envelope()["data"], "L0.registry")["reason"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    let drifted = harness.check_config(&json!({"schemaErrorsDrift": true}));
    assert_eq!((drifted.code, failing(&drifted)), (Some(1), vec!["L0.registry".to_string()]));
    let reason = reason_of(&drifted);
    assert!(reason.contains("STATE_ODD") && reason.contains("exit 2"), "the differing code is named: {reason}");
    // A fixture row the tool does not have, and a tool row the fixture does not have.
    let mut fixture = harness.fixture(&json!({}));
    fixture["registry"].as_array_mut().expect("registry").push(json!({"code": "ONLY_HERE", "exit": 2, "retryable": false}));
    assert!(reason_of(&harness.check(&fixture, true)).contains("ONLY_HERE is only in the fixture"));
    let mut fixture = harness.fixture(&json!({}));
    let rows = fixture["registry"].as_array_mut().expect("registry");
    rows.retain(|row| row["code"] != "STATE_ODD");
    assert!(reason_of(&harness.check(&fixture, true)).contains("STATE_ODD is only in the tool"));
    // The retryable of a code, and the reasons under it.
    let mut fixture = harness.fixture(&json!({}));
    fixture["registry"][3]["retryable"] = json!(true);
    assert!(reason_of(&harness.check(&fixture, true)).contains("retryable"));
    let mut fixture = harness.fixture(&json!({}));
    fixture["registry"][2]["reasons"] = json!(["NOT_JSON"]);
    let reason = reason_of(&harness.check(&fixture, true));
    assert!(reason.contains("INPUT_INVALID") && reason.contains("UNKNOWN_KEY"), "{reason}");
    // The warning codes.
    let mut fixture = harness.fixture(&json!({}));
    fixture["warnings"] = json!(["STALE_VIEW", "OTHER_WARNING"]);
    assert!(reason_of(&harness.check(&fixture, true)).contains("OTHER_WARNING"));
    // Not offered (the tool has no `schema`): nothing to compare, the check does not apply, and the level passes.
    let none = harness.check_config(&json!({"noSchema": true}));
    assert_eq!(none.code, Some(0), "{}", none.stdout);
    assert_eq!(result_of(&none, "L0.registry"), "unsupported");
    // A tool that offers `schema --errors` and prints a code, a reason or a warning no registry has, fails with no fixture to compare.
    let mut bare = harness.fixture(&json!({"unregisteredReason": true}));
    for key in ["registry", "warnings"] {
        bare.as_object_mut().expect("object").remove(key);
    }
    let output = harness.check(&bare, true);
    assert_eq!(result_of(&output, "L0.registry"), "fail");
    assert!(reason_of(&output).contains("NOT_A_REASON"), "{}", reason_of(&output));
}

/// The fake tool run with `config` against a fixture whose time limit is `seconds`; the checks that failed or were unprovided, and the time it took.
fn prompt_run(harness: &Harness, config: &Value) -> (Output, std::time::Duration) {
    let mut fixture = harness.fixture(config);
    fixture["timeoutSeconds"] = json!(20);
    let started = std::time::Instant::now();
    let output = harness.check(&fixture, true);
    (output, started.elapsed())
}

#[test]
fn atip_check_50_a_tool_that_waits_on_a_terminal_for_a_missing_value_fails_and_one_that_refuses_passes() {
    let harness = Harness::new();
    // A tool that refuses and ends passes (the conforming run), and the terminal run is part of the report.
    let conforming = harness.check_config(&json!({}));
    assert_eq!(result_of(&conforming, "L0.no-prompt"), "pass");
    let observed = check_of(&conforming.envelope()["data"], "L0.no-prompt")["observed"].clone();
    assert!(
        observed.as_array().is_some_and(|calls| calls.iter().any(|call| call["terminal"] == true && call["exit"] == 2)),
        "{observed}"
    );
    // A tool that reads a line from a canonical-mode terminal: it ends only after the end-of-file character.
    let (reads, took) = prompt_run(&harness, &json!({"promptOnMissing": true}));
    assert_eq!((reads.code, failing(&reads)), (Some(1), vec!["L0.no-prompt".to_string()]), "{}", reads.stdout);
    let reason = check_of(&reads.envelope()["data"], "L0.no-prompt")["reason"].as_str().unwrap_or_default().to_string();
    assert!(reason.contains("waits for input"), "{reason}");
    assert!(took < std::time::Duration::from_secs(15), "the deadline is twice the control run plus two seconds, not the fixture's limit: {took:?}");
    let terminal = check_of(&reads.envelope()["data"], "L0.no-prompt")["observed"][0].clone();
    assert_eq!(terminal["atDeadline"]["reactedToEof"], true, "{terminal}");
    // A tool that switches to raw mode, prints a prompt and waits for a key: the prompt shows it waits.
    let (prompts, _) = prompt_run(&harness, &json!({"rawOnMissing": "prompt"}));
    assert_eq!((prompts.code, failing(&prompts)), (Some(1), vec!["L0.no-prompt".to_string()]), "{}", prompts.stdout);
    assert!(check_of(&prompts.envelope()["data"], "L0.no-prompt")["reason"].as_str().unwrap_or_default().contains("asks for a value"));
    // A raw-mode tool that prints nothing and ends on any byte reacts to the end-of-file character (`0x04`).
    let (reacts, _) = prompt_run(&harness, &json!({"rawOnMissing": "react"}));
    assert_eq!((reacts.code, failing(&reacts)), (Some(1), vec!["L0.no-prompt".to_string()]), "{}", reacts.stdout);
    // A raw-mode tool that ignores every byte and prints nothing, and a tool that sleeps without reading stdin, are slow, not shown to wait: unprovided.
    for config in [json!({"rawOnMissing": "wait"}), json!({"sleepOnMissing": 60})] {
        let (slow, took) = prompt_run(&harness, &config);
        assert_eq!(result_of(&slow, "L0.no-prompt"), "unprovided", "{config}: {}", slow.stdout);
        assert_eq!(slow.code, Some(1), "{config}");
        assert!(took < std::time::Duration::from_secs(20), "{config}: {took:?}");
        let reason = check_of(&slow.envelope()["data"], "L0.no-prompt")["reason"].as_str().unwrap_or_default().to_string();
        assert!(reason.contains("slow, not shown to wait for input"), "{reason}");
    }
}

#[test]
fn atip_check_62_the_output_of_a_terminal_run_is_read_to_its_end_and_its_tail_is_in_the_report() {
    let harness = Harness::new();
    // A tool that prints a refusal and then 60000 characters of chatter ending in a marker.
    let (output, _) = prompt_run(&harness, &json!({"chatterOnTerminal": 60000}));
    assert_eq!((output.code, failing(&output)), (Some(1), vec!["L0.no-prompt".to_string()]), "{}", output.stdout);
    let check = check_of(&output.envelope()["data"], "L0.no-prompt").clone();
    let terminal = check["observed"][0].clone();
    assert_eq!(terminal["terminal"], true, "{check}");
    assert!(terminal["stdout"].as_str().unwrap_or_default().ends_with("END-OF-CHATTER\n"), "the tail of a long output stays in the report: {terminal}");
}

#[test]
fn atip_check_63_a_fixture_without_a_missing_value_invocation_leaves_no_prompt_unprovided() {
    let harness = Harness::new();
    let mut fixture = harness.fixture(&json!({}));
    fixture["commands"]["touch"].as_object_mut().expect("touch").remove("missingValue");
    let output = harness.check(&fixture, true);
    assert_eq!((output.code, result_of(&output, "L0.no-prompt")), (Some(1), "unprovided".to_string()));
    assert!(check_of(&output.envelope()["data"], "L0.no-prompt")["reason"].as_str().unwrap_or_default().contains("missingValue"));
    // A `missingValue` that the tool does not refuse is not a control run to compare with: unprovided too, with the reason.
    let mut fixture = harness.fixture(&json!({}));
    fixture["commands"]["touch"]["missingValue"] = json!([["touch", "--name", "x", "--json"]]);
    let output = harness.check(&fixture, true);
    assert_eq!(result_of(&output, "L0.no-prompt"), "unprovided");
    assert!(check_of(&output.envelope()["data"], "L0.no-prompt")["reason"].as_str().unwrap_or_default().contains("control run"));
}

#[test]
fn atip_check_52_a_violation_seen_is_a_failure_even_when_the_fixture_lacks_the_entry_and_nothing_seen_is_unprovided(
) {
    let harness = Harness::new();
    // No probe, and the unknown command prints two documents: one-document saw it on the call it makes without the probe.
    let mut no_probe = harness.fixture(&json!({"unknownCommandTwoDocuments": true}));
    no_probe.as_object_mut().expect("object").remove("probe");
    let failed = harness.check(&no_probe, true);
    assert_eq!(result_of(&failed, "L0.one-document"), "fail");
    assert_eq!(result_of(&failed, "L0.after-dashdash"), "unprovided");
    assert_eq!(
        (failed.code, failed.envelope()["error"]["code"].as_str()),
        (Some(1), Some("LEVEL_NOT_MET"))
    );
    // No registry and no `schema`: nothing to compare an exit with, so nothing wrong seen is unprovided.
    let mut no_registry = harness.fixture(&json!({"noSchema": true}));
    no_registry.as_object_mut().expect("object").remove("registry");
    let output = harness.check(&no_registry, true);
    assert_eq!(result_of(&output, "L0.ok-exit"), "unprovided");
}

#[test]
fn atip_check_74_a_refusal_that_changes_what_the_probe_reports_fails_refusal_effects() {
    let harness = Harness::new();
    let output = harness.check_config(&json!({"refusalWritesRemote": true}));
    assert_eq!((output.code, failing(&output)), (Some(1), vec!["L0.refusal-effects".to_string()]), "{}", output.stdout);
    let reason = check_of(&output.envelope()["data"], "L0.refusal-effects")["reason"].as_str().unwrap_or_default().to_string();
    assert!(reason.contains("the probe then reports") && reason.contains("\"remote\""), "{reason}");
    // No probe, no remote state: the same tool is not caught by the refusal rule, and the missing probe is unprovided elsewhere.
    let mut fixture = harness.fixture(&json!({"refusalWritesRemote": true}));
    fixture.as_object_mut().expect("object").remove("probe");
    assert_eq!(result_of(&harness.check(&fixture, true), "L0.refusal-effects"), "pass");
    // A probe whose answer is not the same twice (here: the time) cannot show a change and is not held against the tool.
    assert_eq!(result_of(&harness.check_config(&json!({})), "L0.refusal-effects"), "pass");
}

#[test]
fn atip_check_53_a_completed_run_leaves_the_working_directory_home_and_the_scratch_space_as_it_found_them(
) {
    let harness = Harness::new();
    let fixture = harness.write("fixture.json", &harness.fixture(&json!({})));
    let (cwd, tmp, home) = (
        harness.path("cwd"),
        harness.path("tmp"),
        harness.path("home"),
    );
    for dir in [&cwd, &tmp, &home] {
        std::fs::create_dir_all(dir).expect("dir");
    }
    let output = std::process::Command::new(ATIP)
        .args([
            "check",
            "--bin",
            FAKE_TOOL,
            "--level",
            "L0",
            "--fixture",
            fixture.to_str().expect("utf8"),
            "--json",
        ])
        .current_dir(&cwd)
        .env("TMPDIR", &tmp)
        .env("HOME", &home)
        .output()
        .expect("run atip");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    for (name, dir) in [
        ("working directory", &cwd),
        ("scratch space", &tmp),
        ("home", &home),
    ] {
        assert!(
            listing(dir).is_empty(),
            "a completed run left something in the {name}: {:?}",
            listing(dir)
        );
    }
    // A failed level leaves nothing either.
    let failing = std::process::Command::new(ATIP)
        .args([
            "check",
            "--bin",
            FAKE_TOOL,
            "--level",
            "L0",
            "--fixture",
            harness
                .write("bad.json", &harness.fixture(&json!({"hintNull": true})))
                .to_str()
                .expect("utf8"),
        ])
        .current_dir(&cwd)
        .env("TMPDIR", &tmp)
        .env("HOME", &home)
        .output()
        .expect("run atip");
    assert_eq!(failing.status.code(), Some(1));
    assert!(listing(&cwd).is_empty() && listing(&tmp).is_empty() && listing(&home).is_empty());
}

#[test]
fn atip_check_51_a_probe_whose_data_keys_change_with_the_environment_or_that_is_no_envelope_fails_no_env_switch() {
    let harness = Harness::new();
    let keys = harness.check_config(&json!({"envChangesDataKeys": true}));
    assert_eq!((keys.code, failing(&keys)), (Some(1), vec!["L0.no-env-switch".to_string()]), "{}", keys.stdout);
    let reason = check_of(&keys.envelope()["data"], "L0.no-env-switch")["reason"].as_str().unwrap_or_default().to_string();
    assert!(reason.contains("data keys"), "{reason}");
    let not_envelope = harness.check_config(&json!({"probeNotEnvelope": true}));
    let reason = check_of(&not_envelope.envelope()["data"], "L0.no-env-switch")["reason"].as_str().unwrap_or_default().to_string();
    assert_eq!(result_of(&not_envelope, "L0.no-env-switch"), "fail");
    assert!(reason.contains("without a variation did not print an envelope"), "{reason}");
}

#[test]
fn atip_check_57_the_contract_option_and_the_fixtures_contract_version_pick_the_claim_and_two_that_differ_are_refused_before_a_binary_runs() {
    let harness = Harness::new();
    let mut fixture = harness.fixture(&json!({}));
    fixture.as_object_mut().expect("object").remove("contractVersion");
    // The option alone claims 1.1: 28 checks, the 21 MUST of the applicable 27 pass and L0.contract does not apply.
    let output = harness.check_with(&fixture, true, &["--contract", "1.1"]);
    assert_eq!(output.code, Some(0), "{}", output.stdout);
    let document = output.envelope();
    assert_eq!(document["data"]["contractVersion"], "1.1");
    assert_eq!(document["data"]["checks"].as_array().map(Vec::len), Some(29));
    assert_eq!(result_of(&output, "L0.contract"), "unsupported");
    let must_passed = document["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .filter(|check| check["strength"] == "MUST" && check["result"] == "pass")
        .count();
    assert_eq!(must_passed, 21);
    // Nothing claimed is 1.0, which a 1.1 tool breaks (it prints members of the later minor).
    let unclaimed = harness.check(&fixture, true);
    assert_eq!(unclaimed.envelope()["data"]["contractVersion"], "1.0");
    assert_eq!(result_of(&unclaimed, "L0.contract-version"), "fail");
    // The same claim in both places is fine.
    let fixture_11 = harness.fixture(&json!({}));
    assert_eq!(harness.check_with(&fixture_11, true, &["--contract", "1.1"]).code, Some(0));
    // Two that differ are USAGE naming the option, and no binary runs: not even the fixture's setup.
    let marker = harness.path("init.log");
    let mut watched = harness.fixture(&json!({"initLog": marker}));
    let different = harness.check_with(&watched, true, &["--contract", "1.0"]);
    assert_eq!(different.code, Some(2), "{}", different.stdout);
    let document = different.envelope();
    assert_eq!((document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some("USAGE"), Some("--contract")));
    assert!(!marker.exists(), "the setup ran although the claims differ");
    // A minor the checker does not know is refused as well, naming the minors it knows.
    watched.as_object_mut().expect("object").remove("contractVersion");
    let unknown = harness.check_with(&watched, true, &["--contract", "1.9"]);
    let document = unknown.envelope();
    assert_eq!((unknown.code, document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("--contract")), "{}", unknown.stdout);
    assert_eq!(document["error"]["expected"], json!(["1.0", "1.1", "1.2"]));
    assert!(!marker.exists());
}

#[test]
fn atip_check_58_a_tool_that_claims_1_0_is_held_to_no_member_of_1_1_and_to_no_hint_form() {
    let harness = Harness::new();
    // A 1.0 tool whose prose hint starts with its name passes: L0.hint-form does not apply to a 1.0 claim.
    let prose = harness.check_config(&json!({"claims": "1.0", "proseHintStartsWithTool": true}));
    assert_eq!(prose.code, Some(0), "{}", prose.stdout);
    assert_eq!(result_of(&prose, "L0.hint-form"), "unsupported");
    assert_eq!(prose.envelope()["data"]["contractVersion"], "1.0");
    // The same tool claiming 1.1 fails L0.hint-form.
    let claimed = harness.check_with(&harness.fixture(&json!({"claims": "1.0", "proseHintStartsWithTool": true})), true, &["--contract", "1.1"]);
    assert_eq!(claimed.code, Some(1), "{}", claimed.stdout);
    assert_eq!(result_of(&claimed, "L0.hint-form"), "fail");
    // A 1.0 tool that prints error.reason fails L0.contract-version and nothing else.
    let reason = harness.check_config(&json!({"claims": "1.0", "reasonIn10": true}));
    assert_eq!((reason.code, failing(&reason)), (Some(1), vec!["L0.contract-version".to_string()]), "{}", reason.stdout);
    let text = check_of(&reason.envelope()["data"], "L0.contract-version")["reason"].as_str().unwrap_or_default().to_string();
    assert!(text.contains("error.reason"), "{text}");
}

#[test]
fn atip_check_76_a_violation_of_a_1_1_check_is_read_in_the_result_as_its_owner_with_the_rule_and_the_reason() {
    let harness = Harness::new();
    // One planted violation for each of four checks that 1.1 added: the report says which check owns it, which rule broke and why,
    // and every other check passes or does not apply.
    for (config, owner, rule, reason_has) in [
        (json!({"omitContractVersion": true}), "L0.contract-version", "15.7", "contractVersion"),
        (json!({"mirrorStderrFor": ["unknown-command"]}), "L0.stderr-result", "5.2", "stderr"),
        (json!({"metaUnknownMember": true}), "L0.envelope", "6.1-6.6", "unchanged"),
        (json!({"unregisteredReason": true}), "L0.registry", "7.6.2", "NOT_A_REASON"),
    ] {
        let output = harness.check_config(&config);
        assert_eq!(output.code, Some(1), "{config}: {}", output.stdout);
        let document = output.envelope();
        assert_eq!(document["error"]["code"], "LEVEL_NOT_MET", "{config}");
        assert!(
            document["error"]["message"].as_str().unwrap_or_default().contains(owner),
            "the error names the failing check: {}",
            document["error"]["message"]
        );
        let check = check_of(&document["data"], owner);
        assert_eq!(check["result"], "fail", "{config}");
        let rules = check["rules"].as_array().expect("rules");
        assert!(rules.iter().any(|item| item == rule), "{owner} names the rule {rule}: {rules:?}");
        let reason = check["reason"].as_str().unwrap_or_default();
        assert!(reason.contains(reason_has), "{owner}: the reason says what broke ({reason_has}): {reason}");
        assert!(check["observed"].as_array().is_some_and(|calls| !calls.is_empty()), "{owner} shows the invocation that decided it");
        support::case_verdict(&document["data"], owner).unwrap_or_else(|why| panic!("{config}: {why}"));
        // The text form reads the same: the failing check's line carries its rules and reason.
        let text = harness.check(&harness.fixture(&config), false);
        assert!(
            text.stderr.lines().any(|line| line.trim_start().starts_with("fail") && line.contains(owner) && line.contains(rule)),
            "{owner}: {}",
            text.stderr
        );
    }
}
