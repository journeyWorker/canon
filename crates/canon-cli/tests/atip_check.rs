//! The fixture `atip check` reads for canon (0.34, `canon-0-34-atip-check-1-1` F6; 0.33 `canon-0-33-atip-check`; 0.36 `canon-0-36-atip-dep`).
//!
//! `tests/atip/canon.fixture.json` is the fixture the checker reads for canon: it claims 1.4 (0.36: `canon-0-36-claim-1-4`; 0.35: `canon-0-35-claim-l2`); `setup` is `init`, two
//! subjects (so a listing has a second page) and what the findings, scenarios and divergences of the entries need; `probe` is `status`; `globalOptions` is `--repo .`; `commands` holds a paging call
//! that also emits a next step (`query`) and the refusal of a missing value (`finding close` without `--seq`); `registry`
//! and `warnings` are generated from `canon schema --errors --json` and `--warnings --json`, never written by hand.
//! `CANON_BLESS_ATIP_FIXTURE=1 cargo test -p canon-cli --test atip_check` rewrites it; the test fails when the
//! committed file differs from what the binary says.
//!
//! The checker is not canon's: it lives in the atip repository (https://github.com/journeyWorker/atip, tag `atip-check-v0.1.4`) and canon runs it once, in the
//! CI job `atip` and in `just atip-deep`: `atip check --level L2 --contract 1.4 --deep` on the canon binary and this fixture (`just atip` runs it without `--deep`). No test of canon runs the checker.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

const CANON: &str = env!("CARGO_BIN_EXE_canon");

/// The command the CI job and the just recipe run, with the binary's directory spelled `target/debug`.
const COMMAND: &str = "atip check --bin target/debug/canon --level L2 --contract 1.4 --deep --fixture crates/canon-cli/tests/atip/canon.fixture.json";
/// The command that installs the checker, at the tag canon is held to, in the CI job and in the just recipe.
const INSTALL: &str = "cargo install --git https://github.com/journeyWorker/atip --tag atip-check-v0.1.4 atip-check --locked";

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
    assert_eq!((fixture["contractVersion"].clone(), fixture["probe"].clone(), fixture["globalOptions"].clone()), (json!("1.4"), json!("status"), json!([["--repo", "."]])));
    let mut keys: Vec<&String> = fixture.as_object().expect("object").keys().collect();
    keys.sort();
    assert_eq!(keys, ["commands", "contractVersion", "globalOptions", "notRun", "prepare", "probe", "registry", "setup", "timeoutSeconds", "warnings"], "the L2 keys are `commands` members (`preview`, `batchFailsAt`, `batchFailsFirst`, `allowedEffects`), not top-level ones");
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
    let job = workflow_commands("atip");
    assert!(job.iter().any(|run| run == INSTALL), "the job installs the checker at its tag: {job:?}");
    assert!(job.iter().any(|run| run.contains("cargo build") && run.contains("-p canon-cli") && !run.contains("atip-check") && run.contains("--locked")), "the job builds canon only: {job:?}");
    assert!(job.iter().any(|run| run == COMMAND), "the job runs `{COMMAND}`: {job:?}");
    assert_eq!(job.iter().filter(|run| run.starts_with("atip check")).count(), 1, "the job runs the checker once: {job:?}");
    let deep = just_recipe("atip-deep");
    assert!(deep.iter().any(|line| line == COMMAND), "the recipe atip-deep runs the job's command: {deep:?}");
    assert!(deep.iter().any(|line| line.contains("cargo build") && line.contains("-p canon-cli") && !line.contains("atip-check")), "{deep:?}");
    let quick = COMMAND.replace(" --deep", "");
    let recipe = just_recipe("atip");
    assert!(recipe.contains(&quick), "the recipe atip runs the job's command without --deep: {recipe:?}");
    assert!(recipe.iter().any(|line| line.contains("cargo build") && line.contains("-p canon-cli") && !line.contains("atip-check")), "{recipe:?}");
    assert_eq!(just_recipe("atip-install"), [INSTALL], "the recipe that installs the checker runs the job's command");
}
