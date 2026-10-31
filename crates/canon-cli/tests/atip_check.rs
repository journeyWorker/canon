//! `atip check` against canon (0.33, `canon-0-33-atip-check` F6).
//!
//! `tests/atip/canon.fixture.json` is the fixture the checker reads for canon: `setup` is `init`, `probe` is
//! `status`, and `registry` is generated from `canon schema --errors --json`, never written by hand.
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

/// The registry rows of a `canon schema --errors --json` answer: the five members a registry row has, and nothing else.
fn registry_rows(answer: &Value) -> Vec<Value> {
    let errors = answer["data"]["errors"].as_array().expect("data.errors of the --errors answer");
    errors
        .iter()
        .map(|row| {
            let mut kept = serde_json::Map::new();
            for key in ["code", "exit", "retryable", "meaning", "fix"] {
                kept.insert(key.to_string(), row[key].clone());
            }
            Value::Object(kept)
        })
        .collect()
}

/// The fixture as the binary says it should be: `setup` `init`, `probe` `status`, the registry from `canon schema --errors --json`.
fn generate() -> Value {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = Command::new(CANON).args(["schema", "--errors", "--json"]).current_dir(dir.path()).output().expect("run canon");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let answer: Value = serde_json::from_slice(&output.stdout).expect("the --errors answer is JSON");
    json!({"setup": [["init"]], "probe": "status", "registry": registry_rows(&answer)})
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
fn atip_check_39_the_canon_fixtures_registry_equals_the_one_generated_from_canon_schema_errors() {
    let generated = generate();
    if std::env::var_os("CANON_BLESS_ATIP_FIXTURE").is_some() {
        std::fs::write(fixture_path(), rendered(&generated)).expect("write the fixture");
    }
    let committed = std::fs::read_to_string(fixture_path()).expect("tests/atip/canon.fixture.json");
    assert_eq!(drift(&committed, &generated), None, "run CANON_BLESS_ATIP_FIXTURE=1 cargo test -p canon-cli --test atip_check and commit the fixture");
    let fixture: Value = serde_json::from_str(&committed).expect("the fixture is JSON");
    assert_eq!((fixture["setup"].clone(), fixture["probe"].clone()), (json!([["init"]]), json!("status")));
    let mut keys: Vec<&String> = fixture.as_object().expect("object").keys().collect();
    keys.sort();
    assert_eq!(keys, ["probe", "registry", "setup"], "the L0 keys only");
    for row in fixture["registry"].as_array().expect("registry") {
        let mut members: Vec<&str> = row.as_object().expect("row").keys().map(String::as_str).collect();
        members.sort_unstable();
        assert_eq!(members, ["code", "exit", "fix", "meaning", "retryable"]);
    }
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
        ("a fix reworded", stale(&|f| f["registry"][0]["fix"] = json!("reworded by hand"))),
        ("a member added", stale(&|f| f["registry"][0]["x-note"] = json!("by hand"))),
        ("a key added", stale(&|f| f["commands"] = json!({}))),
    ] {
        assert!(drift(&text, &generated).is_some(), "{name} must be noticed");
    }
    assert_eq!(drift(&rendered(&committed), &generated), None);
    // A member of the answer outside the five a registry row has is dropped by the generator.
    let answer = json!({"data": {"errors": [{"code": "X", "exit": 2, "retryable": false, "meaning": "m", "fix": "f", "surplus": 1, "x-canon": {}}]}});
    assert_eq!(registry_rows(&answer), [json!({"code": "X", "exit": 2, "retryable": false, "meaning": "m", "fix": "f"})]);
}

fn run_checker(bin: &Path) -> atip_check::report::Report {
    let args = CheckArgs { bin: bin.into(), level: Level::L0, fixture: fixture_path().into() };
    check_binary(&args).unwrap_or_else(|failure| panic!("{}: {}", failure.code.info().code, failure.message))
}

#[test]
fn atip_check_41_canon_passes_the_16_l0_checks_through_the_committed_fixture() {
    let report = run_checker(Path::new(CANON));
    assert!(report.passed, "{}", report.text());
    assert_eq!(report.checks.len(), 16);
    let failing: Vec<&str> = report.checks.iter().filter(|check| check.result != atip_check::report::CheckResult::Pass).map(|check| check.id).collect();
    assert!(failing.is_empty(), "MUST and SHOULD checks all pass for canon: {failing:?}\n{}", report.text());
}

#[test]
fn atip_check_42_a_canon_whose_error_prints_hint_null_fails_l0_no_null() {
    // canon with one planted violation: every error that has a hint prints `"hint":null` instead (and keeps the text under `x-was-hint`), the way canon 0.32 printed a hint it had none for.
    let dir = tempfile::tempdir().expect("tempdir");
    let wrapper = dir.path().join("canon-with-null-hints");
    let script = format!("#!/bin/sh\nout=$('{CANON}' \"$@\")\ncode=$?\nif [ -n \"$out\" ]; then printf '%s\\n' \"$out\" | sed 's/\"hint\":\"/\"hint\":null,\"x-was-hint\":\"/'; fi\nexit $code\n");
    std::fs::write(&wrapper, script).expect("write");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let report = run_checker(&wrapper);
    assert!(!report.passed);
    let check = report.find("L0.no-null").expect("L0.no-null");
    assert_eq!(check.result, atip_check::report::CheckResult::Fail, "{}", report.text());
    assert!(check.reason.as_deref().unwrap_or_default().contains("hint"), "{:?}", check.reason);
    let failing: Vec<&str> = report.checks.iter().filter(|check| check.result != atip_check::report::CheckResult::Pass).map(|check| check.id).collect();
    assert_eq!(failing, ["L0.no-null"], "the planted null is the only thing wrong: {}", report.text());
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
