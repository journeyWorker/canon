//! `atip check` against canon (0.34, `canon-0-34-atip-check-1-1` F6; 0.33 `canon-0-33-atip-check`).
//!
//! `tests/atip/canon.fixture.json` is the fixture the checker reads for canon: it claims 1.1; `setup` is `init` and two
//! subjects (so a listing has a second page); `probe` is `status`; `globalOptions` is `--repo .`; `commands` holds a paging call
//! that also emits a next step (`query`) and the refusal of a missing value (`finding close` without `--seq`); `registry`
//! and `warnings` are generated from `canon schema --errors --json` and `--warnings --json`, never written by hand.
//! `CANON_BLESS_ATIP_FIXTURE=1 cargo test -p canon-cli --test atip_check` rewrites it; the test fails when the
//! committed file differs from what the binary says. The CI job `atip-l0` and `just atip` run the same
//! `atip check` on the same binary and fixture.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use atip_check::app::check_binary;
use atip_check::cli::{CheckArgs, Level};
use serde_json::{json, Value};

const CANON: &str = env!("CARGO_BIN_EXE_canon");

/// The command line the CI job and the just recipe run, with the binaries' directory spelled `target/debug`.
const COMMAND: &str = "atip check --bin target/debug/canon --level L0 --fixture crates/canon-cli/tests/atip/canon.fixture.json";
/// The same for the checker on itself.
const SELF_COMMAND: &str = "atip check --bin target/debug/atip --level L0 --fixture crates/atip-check/atip.fixture.json";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/atip/canon.fixture.json")
}

/// The registry rows of a `canon schema --errors --json` answer as a fixture lists them: the code, the exit, `retryable` and
/// the names of the reasons (when it has any), no prose.
fn registry_rows(answer: &Value) -> Vec<Value> {
    let errors = answer["data"]["errors"].as_array().expect("data.errors of the --errors answer");
    errors
        .iter()
        .map(|row| {
            let mut kept = serde_json::Map::new();
            for key in ["code", "exit", "retryable"] {
                kept.insert(key.to_string(), row[key].clone());
            }
            let reasons: Vec<Value> = row["reasons"].as_array().into_iter().flatten().map(|reason| reason["reason"].clone()).collect();
            if !reasons.is_empty() {
                kept.insert("reasons".to_string(), Value::Array(reasons));
            }
            Value::Object(kept)
        })
        .collect()
}

/// The warning codes of a `canon schema --warnings --json` answer.
fn warning_codes(answer: &Value) -> Vec<Value> {
    let warnings = answer["data"]["warnings"].as_array().expect("data.warnings of the --warnings answer");
    warnings.iter().map(|row| row["code"].clone()).collect()
}

/// The answer of `canon <args> --json` in an empty directory.
fn answer(args: &[&str]) -> Value {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = Command::new(CANON).args(args).arg("--json").current_dir(dir.path()).output().expect("run canon");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).expect("the answer is JSON")
}

/// The hand-written part of the fixture: `prepare`, `setup`, `probe`, the commands and what the L0 and L1 checks read of them (`paged`, `fields`, `domainRejects`).
fn source_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/atip/canon.fixture.src.json")
}

/// The fixture as the binary says it should be: the registry and the warnings from `canon schema`, the rest as written in `canon.fixture.src.json`.
fn generate() -> Value {
    let mut fixture: Value = serde_json::from_str(&std::fs::read_to_string(source_path()).expect("tests/atip/canon.fixture.src.json")).expect("the source is JSON");
    fixture["registry"] = Value::Array(registry_rows(&answer(&["schema", "--errors"])));
    fixture["warnings"] = Value::Array(warning_codes(&answer(&["schema", "--warnings"])));
    fixture
}

fn rendered(fixture: &Value) -> String {
    serde_json::to_string_pretty(fixture).expect("json") + "\n"
}

/// Why `committed` is not `generated`, when it is not.
fn drift(committed: &str, generated: &Value) -> Option<String> {
    let want = rendered(generated);
    (committed != want).then(|| "the committed canon fixture differs from the one generated from `canon schema --errors --json`".to_string())
}

#[test]
fn atip_check_39_the_canon_fixtures_registry_and_warnings_equal_the_ones_generated_from_canon_schema() {
    let generated = generate();
    if std::env::var_os("CANON_BLESS_ATIP_FIXTURE").is_some() {
        std::fs::write(fixture_path(), rendered(&generated)).expect("write the fixture");
    }
    let committed = std::fs::read_to_string(fixture_path()).expect("tests/atip/canon.fixture.json");
    assert_eq!(drift(&committed, &generated), None, "run CANON_BLESS_ATIP_FIXTURE=1 cargo test -p canon-cli --test atip_check and commit the fixture");
    let fixture: Value = serde_json::from_str(&committed).expect("the fixture is JSON");
    assert_eq!((fixture["contractVersion"].clone(), fixture["probe"].clone(), fixture["globalOptions"].clone()), (json!("1.1"), json!("status"), json!([["--repo", "."]])));
    let mut keys: Vec<&String> = fixture.as_object().expect("object").keys().collect();
    keys.sort();
    assert_eq!(keys, ["commands", "contractVersion", "globalOptions", "prepare", "probe", "registry", "setup", "timeoutSeconds", "warnings"], "the L0 and L1 keys only");
    for row in fixture["registry"].as_array().expect("registry") {
        let mut members: Vec<&str> = row.as_object().expect("row").keys().map(String::as_str).collect();
        members.sort_unstable();
        assert!(members == ["code", "exit", "retryable"] || members == ["code", "exit", "reasons", "retryable"], "a registry row names codes and reasons and carries no prose: {members:?}");
    }
    assert!(!fixture["warnings"].as_array().expect("warnings").is_empty(), "canon has warnings to list");
    // The calls the L0 checks need: a page and a next step, and a refusal of a missing value.
    assert!(fixture["commands"]["query"]["ok"].as_array().is_some_and(|calls| !calls.is_empty()));
    assert!(fixture["commands"]["finding close"]["missingValue"].as_array().is_some_and(|calls| !calls.is_empty()));
}

#[test]
fn atip_check_40_a_stale_registry_in_the_canon_fixture_fails_the_generation_test() {
    let generated = generate();
    let committed: Value = serde_json::from_str(&std::fs::read_to_string(fixture_path()).expect("fixture")).expect("json");
    let stale = |change: &dyn Fn(&mut Value)| {
        let mut fixture = committed.clone();
        change(&mut fixture);
        rendered(&fixture)
    };
    for (name, text) in [
        ("a row dropped", stale(&|f| drop(f["registry"].as_array_mut().expect("registry").pop()))),
        ("an exit changed", stale(&|f| f["registry"][0]["exit"] = json!(if f["registry"][0]["exit"] == 1 { 2 } else { 1 }))),
        ("a retryable flipped", stale(&|f| f["registry"][0]["retryable"] = json!(!f["registry"][0]["retryable"].as_bool().unwrap_or(false)))),
        ("a member added", stale(&|f| f["registry"][0]["meaning"] = json!("by hand"))),
        ("a warning dropped", stale(&|f| drop(f["warnings"].as_array_mut().expect("warnings").pop()))),
        ("a call changed", stale(&|f| f["commands"]["query"]["ok"] = json!([["query"]]))),
        ("a key added", stale(&|f| f["env"] = json!({}))),
    ] {
        assert!(drift(&text, &generated).is_some(), "{name} must be noticed");
    }
    assert_eq!(drift(&rendered(&committed), &generated), None);
    // The prose of the answer, and any member outside a fixture row, is dropped by the generator; reasons are kept by name.
    let answer = json!({"data": {"errors": [{"code": "X", "exit": 2, "retryable": false, "meaning": "m", "fix": "f", "surplus": 1, "x-canon": {}, "reasons": [{"reason": "R", "meaning": "m", "fix": "f"}]}]}});
    assert_eq!(registry_rows(&answer), [json!({"code": "X", "exit": 2, "retryable": false, "reasons": ["R"]})]);
}

fn run_checker(bin: &Path) -> atip_check::report::Report {
    let args = CheckArgs { bin: bin.into(), level: Level::L0, fixture: fixture_path().into(), contract: None };
    check_binary(&args).unwrap_or_else(|failure| panic!("{}: {}", failure.code.info().code, failure.message))
}

#[test]
fn atip_check_41_canon_passes_the_26_applicable_of_the_28_l0_checks_through_the_committed_fixture() {
    let report = run_checker(Path::new(CANON));
    assert!(report.passed, "{}", report.text());
    assert_eq!(report.checks.len(), 28);
    assert_eq!(report.contract_version, "1.1");
    // The fixture has a paging call and a next-step call, so these two are judged, and only `L0.contract` (a 1.0 rule) does not apply.
    for id in ["L0.next", "L0.page"] {
        assert_eq!(report.find(id).map(|check| check.result), Some(atip_check::report::CheckResult::Pass), "{id}: {}", report.text());
    }
    assert_eq!(report.find("L0.contract").map(|check| check.result), Some(atip_check::report::CheckResult::Unsupported));
    // `L0.usage-hint` judges every `USAGE` without a `field`, and every `USAGE` canon prints names its field (7.1.3): none is observed, so it does not apply.
    let not_applicable: Vec<&str> = report.checks.iter().filter(|check| check.result == atip_check::report::CheckResult::Unsupported).map(|check| check.id).collect();
    assert_eq!(not_applicable, ["L0.contract", "L0.usage-hint"], "{}", report.text());
    let failing: Vec<&str> = report.checks.iter().filter(|check| !matches!(check.result, atip_check::report::CheckResult::Pass | atip_check::report::CheckResult::Unsupported)).map(|check| check.id).collect();
    assert!(failing.is_empty(), "the 21 MUST and 5 SHOULD checks all pass for canon: {failing:?}\n{}", report.text());
}

#[test]
fn atip_checkl1_31_canon_passes_every_applicable_l1_check_and_its_tree_says_l1() {
    let args = CheckArgs { bin: Path::new(CANON).into(), level: Level::L1, fixture: fixture_path().into(), contract: None };
    let report = check_binary(&args).unwrap_or_else(|failure| panic!("{}: {}", failure.code.info().code, failure.message));
    assert!(report.passed, "{}", report.text());
    assert_eq!((report.level, report.contract_version, report.checks.len()), ("L1", "1.1", 28 + 31));
    let failing: Vec<&str> = report.checks.iter().filter(|check| matches!(check.result, atip_check::report::CheckResult::Fail | atip_check::report::CheckResult::Unprovided)).map(|check| check.id).collect();
    // The one advisory canon does not meet: `--kind` names the set of ledger kinds on `gate promote` and the kinds `gate purge` can delete, which share no value (L1.one-meaning-values, a SHOULD).
    assert_eq!(failing, ["L1.one-meaning-values"], "no MUST check fails or is unprovided for canon, and the one advisory is the known one:\n{}", report.text());
    assert_eq!(report.advisories(), 1);
    // The tree says the level the claim is (L1.tree), and the fixture names every step it made outside canon.
    assert_eq!(report.find("L1.tree").map(|check| check.result), Some(atip_check::report::CheckResult::Pass), "{}", report.text());
    assert!(report.prepare.iter().all(|step| step.first().is_some_and(|program| program == "git")), "{:?}", report.prepare);
    assert_eq!(report.prepare.len(), 2);
    // The checks that probe the arguments and the data were exercised, not skipped for lack of a fixture entry.
    for id in ["L1.constraints", "L1.domains", "L1.patterns", "L1.numbers", "L1.validators", "L1.paging", "L1.fields", "L1.output-data"] {
        assert_eq!(report.find(id).map(|check| check.result), Some(atip_check::report::CheckResult::Pass), "{id}: {}", report.text());
    }
}

#[test]
fn atip_check_98_canon_lists_dashboard_under_not_run_and_passes_its_own_level_with_it() {
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(fixture_path()).expect("fixture")).expect("json");
    assert_eq!(fixture["notRun"], json!(["dashboard"]), "the fixture leaves the command that does not end out of the run");
    let args = CheckArgs { bin: Path::new(CANON).into(), level: Level::L1, fixture: fixture_path().into(), contract: None };
    let report = check_binary(&args).unwrap_or_else(|failure| panic!("{}: {}", failure.code.info().code, failure.message));
    assert!(report.passed, "{}", report.text());
    let data = serde_json::to_value(&report).expect("the report is JSON");
    // The checks that run commands say they left it out, and the other checks do not.
    for id in ["L1.example-runs", "L1.constraints", "L1.domains", "L1.validators", "L1.output-data", "L1.next", "L1.paging", "L1.fields", "L0.page"] {
        let check = data["checks"].as_array().expect("checks").iter().find(|check| check["id"] == id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!(check["notRun"], json!(["dashboard"]), "{id}");
    }
    let tree = data["checks"].as_array().expect("checks").iter().find(|check| check["id"] == "L1.tree").expect("L1.tree");
    assert!(tree.get("notRun").is_none(), "{tree}");
}

#[test]
fn atip_checkl1_34_a_canon_command_whose_example_does_not_parse_fails_only_l1_examples() {
    // canon with one planted violation: the documented example of `status` names `canonn`, a tool that is not this one, so the job that runs canon at L1 is red on `L1.examples` alone.
    let dir = tempfile::tempdir().expect("tempdir");
    let wrapper = dir.path().join("canon-with-a-broken-example");
    let script = format!("#!/bin/sh\nout=$('{CANON}' \"$@\")\ncode=$?\nif [ -n \"$out\" ]; then printf '%s\\n' \"$out\" | sed 's/\"canon status --json\"/\"canonn status --json\"/g'; fi\nexit $code\n");
    std::fs::write(&wrapper, script).expect("write");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let args = CheckArgs { bin: wrapper.clone().into(), level: Level::L1, fixture: fixture_path().into(), contract: None };
    let report = check_binary(&args).unwrap_or_else(|failure| panic!("{}: {}", failure.code.info().code, failure.message));
    assert!(!report.passed, "{}", report.text());
    let check = report.find("L1.examples").expect("L1.examples");
    assert_eq!(check.result, atip_check::report::CheckResult::Fail, "{}", report.text());
    assert!(check.reason.as_deref().unwrap_or_default().contains("canonn status --json"), "{:?}", check.reason);
    let failing: Vec<&str> = report.checks.iter().filter(|check| check.result == atip_check::report::CheckResult::Fail && check.strength == atip_check::report::Strength::Must).map(|check| check.id).collect();
    assert_eq!(failing, ["L1.examples"], "the broken example is the only MUST failure: {}", report.text());
}

#[test]
fn atip_check_42_a_canon_whose_error_prints_hint_null_fails_l0_no_null() {
    // canon with one planted violation: every error that has a hint also prints `"x-was-hint":null` before it, the way canon 0.32 printed a member it had no value for; the hint itself stays, so only L0.no-null owns the violation.
    let dir = tempfile::tempdir().expect("tempdir");
    let wrapper = dir.path().join("canon-with-null-hints");
    let script = format!("#!/bin/sh\nout=$('{CANON}' \"$@\")\ncode=$?\nif [ -n \"$out\" ]; then printf '%s\\n' \"$out\" | sed 's/\"hint\":\"/\"x-was-hint\":null,\"hint\":\"/'; fi\nexit $code\n");
    std::fs::write(&wrapper, script).expect("write");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let report = run_checker(&wrapper);
    assert!(!report.passed);
    let check = report.find("L0.no-null").expect("L0.no-null");
    assert_eq!(check.result, atip_check::report::CheckResult::Fail, "{}", report.text());
    assert!(check.reason.as_deref().unwrap_or_default().contains("hint"), "{:?}", check.reason);
    let failing: Vec<&str> = report.checks.iter().filter(|check| check.result == atip_check::report::CheckResult::Fail).map(|check| check.id).collect();
    assert_eq!(failing, ["L0.no-null"], "the planted null is the only thing wrong: {}", report.text());
}

#[test]
fn atip_check_75_a_canon_whose_hint_is_prose_starting_with_its_name_fails_l0_hint_form() {
    // canon with one planted violation: every "a working call: ..." hint is replaced by prose that starts with `canon ` and is not one command line.
    let dir = tempfile::tempdir().expect("tempdir");
    let wrapper = dir.path().join("canon-with-prose-hints");
    let script = format!("#!/bin/sh\nout=$('{CANON}' \"$@\")\ncode=$?\nif [ -n \"$out\" ]; then printf '%s\\n' \"$out\" | sed 's/\"hint\":\"a working call: /\"hint\":\"canon can'\"'\"'t run this; try: /'; fi\nexit $code\n");
    std::fs::write(&wrapper, script).expect("write");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let report = run_checker(&wrapper);
    assert!(!report.passed, "{}", report.text());
    let check = report.find("L0.hint-form").expect("L0.hint-form");
    assert_eq!(check.result, atip_check::report::CheckResult::Fail, "{}", report.text());
    assert!(check.reason.as_deref().unwrap_or_default().contains("not one command line"), "{:?}", check.reason);
    let failing: Vec<&str> = report.checks.iter().filter(|check| check.result == atip_check::report::CheckResult::Fail).map(|check| check.id).collect();
    assert_eq!(failing, ["L0.hint-form"], "the prose hint is the only thing wrong: {}", report.text());
}

/// The `run:` commands of the workflow job `job` in test.yml.
fn workflow_commands(job: &str) -> Vec<String> {
    let workflow = std::fs::read_to_string(repo_root().join(".github/workflows/test.yml")).expect("test.yml");
    let mut lines = workflow.lines().skip_while(|line| *line != format!("  {job}:"));
    assert!(lines.next().is_some(), "job {job} in test.yml");
    // The job ends at the next line indented by two spaces only: the next job, or the comment above it.
    lines.take_while(|line| !line.starts_with("  ") || line.starts_with("   ")).filter_map(|line| line.trim().strip_prefix("run: ")).map(str::to_string).collect()
}

fn just_recipe(name: &str) -> Vec<String> {
    let justfile = std::fs::read_to_string(repo_root().join("justfile")).expect("justfile");
    let mut lines = justfile.lines().skip_while(|line| !line.starts_with(&format!("{name}:")));
    assert!(lines.next().is_some(), "recipe {name} in the justfile");
    lines.take_while(|line| line.starts_with("    ")).map(|line| line.trim().replace("{{ target }}", "target")).collect()
}

#[test]
fn atip_check_43_the_ci_job_and_the_just_recipe_run_atip_check_against_the_canon_binary_with_the_same_command() {
    let job = workflow_commands("atip-l0");
    assert!(job.iter().any(|run| run.contains("cargo build") && run.contains("-p canon-cli") && run.contains("-p atip-check") && run.contains("--locked")), "the job builds both binaries: {job:?}");
    assert!(job.iter().any(|run| run == &format!("target/debug/{COMMAND}") || run == &format!("./target/debug/{COMMAND}")), "the job runs `{COMMAND}`: {job:?}");
    assert!(job.iter().any(|run| run.ends_with(SELF_COMMAND) && run.contains("target/debug/atip")), "the job runs the checker on itself: {job:?}");
    let recipe = just_recipe("atip");
    assert!(recipe.iter().any(|line| line == &format!("target/debug/{COMMAND}")), "the recipe runs the same command: {recipe:?}");
    assert!(recipe.iter().any(|line| line == &format!("target/debug/{SELF_COMMAND}")), "{recipe:?}");
    assert!(recipe.iter().any(|line| line.contains("cargo build") && line.contains("-p canon-cli") && line.contains("-p atip-check")), "{recipe:?}");
}
