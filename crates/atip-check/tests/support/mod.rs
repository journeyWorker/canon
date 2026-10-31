//! Shared by the integration tests: the `atip` binary, the fake tool, fixtures, and the seeded violations.

#![allow(dead_code, reason = "each integration test uses a part of this module")]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

pub const ATIP: &str = env!("CARGO_BIN_EXE_atip");
pub const FAKE_TOOL: &str = env!("CARGO_BIN_EXE_atip-fake-tool");

/// The result of one run of `atip`.
#[derive(Debug)]
pub struct Output {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// The one document on stdout, which must be exactly one line of JSON.
    pub fn envelope(&self) -> Value {
        assert!(self.stdout.ends_with('\n') && self.stdout.matches('\n').count() == 1, "stdout is not one line: {:?}", self.stdout);
        serde_json::from_str(&self.stdout).unwrap_or_else(|error| panic!("stdout is not JSON ({error}): {:?}", self.stdout))
    }

    /// `data.checks` as `(id, result)` pairs.
    pub fn results(&self) -> Vec<(String, String)> {
        let document = self.envelope();
        document["data"]["checks"].as_array().expect("data.checks").iter().map(|check| (check["id"].as_str().unwrap_or_default().to_string(), check["result"].as_str().unwrap_or_default().to_string())).collect()
    }
}

/// Runs `atip` with `args` in `cwd`, its scratch space in `tmp`.
pub fn atip_in(cwd: &Path, tmp: &Path, args: &[&str]) -> Output {
    let output = Command::new(ATIP).args(args).current_dir(cwd).env("TMPDIR", tmp).output().expect("run atip");
    Output { code: output.status.code(), stdout: String::from_utf8_lossy(&output.stdout).into_owned(), stderr: String::from_utf8_lossy(&output.stderr).into_owned() }
}

/// A scratch directory with the fake tool's configuration and fixtures in it.
pub struct Harness {
    pub dir: tempfile::TempDir,
}

impl Harness {
    pub fn new() -> Self {
        Self { dir: tempfile::tempdir().expect("tempdir") }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    pub fn write(&self, name: &str, document: &Value) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, serde_json::to_string_pretty(document).expect("json")).expect("write");
        path
    }

    /// The fixture of a fake tool configured by `config`: the standard one with `FAKE_ATIP_CONFIG` pointing at it.
    pub fn fixture(&self, config: &Value) -> Value {
        let config_path = self.write("fake-config.json", config);
        json!({
            "setup": [["init"]],
            "env": {"FAKE_ATIP_CONFIG": config_path},
            "timeoutSeconds": 20,
            "probe": "status",
            "registry": standard_registry(),
        })
    }

    /// `atip check --bin <fake tool> --level L0 --fixture <fixture> [--json]`.
    pub fn check(&self, fixture: &Value, json: bool) -> Output {
        let fixture_path = self.write("fixture.json", fixture);
        let mut args = vec!["check", "--bin", FAKE_TOOL, "--level", "L0", "--fixture", fixture_path.to_str().expect("utf8")];
        if json {
            args.push("--json");
        }
        let work = self.path("cwd");
        std::fs::create_dir_all(&work).expect("cwd");
        let tmp = self.path("tmp");
        std::fs::create_dir_all(&tmp).expect("tmp");
        atip_in(&work, &tmp, &args)
    }

    /// The check run as JSON against a fake tool configured by `config`.
    pub fn check_config(&self, config: &Value) -> Output {
        self.check(&self.fixture(config), true)
    }
}

/// The registry rows a conforming fake tool has: the four reserved codes it can produce.
pub fn standard_registry() -> Value {
    json!([
        {"code": "USAGE", "exit": 2, "retryable": false, "meaning": "the command line does not parse", "fix": "Run the command with --help."},
        {"code": "UNKNOWN_COMMAND", "exit": 2, "retryable": false, "meaning": "a command word is not in the tree", "fix": "Pick one of error.expected."},
        {"code": "COMMAND_REQUIRED", "exit": 2, "retryable": false, "meaning": "a command is required", "fix": "Pick one of error.expected."},
        {"code": "INTERNAL", "exit": 1, "retryable": false, "meaning": "a bug in the tool", "fix": "Report it."}
    ])
}

/// One L0 check, the fake tool configuration that breaks exactly that rule, and the other checks that rule also breaks.
pub struct Seeded {
    pub id: &'static str,
    pub config: Value,
    /// Checks that fail with it because they state the same rule (all SHOULD): see the table in each test.
    pub collateral: &'static [&'static str],
}

/// The seeded violations: for each of the 16 L0 checks, the configuration that breaks that one rule.
pub fn seeded() -> Vec<Seeded> {
    let case = |id, config, collateral| Seeded { id, config, collateral };
    vec![
        case("L0.version", json!({"versionStderr": true}), &[]),
        case("L0.version-format", json!({"versionFormat": "bare"}), &[]),
        case("L0.unknown-command", json!({"mirrorErrorOnStderr": true}), &[]),
        case("L0.unknown-command-text", json!({"textErrorOnStdout": true}), &["L0.text-error-form"]),
        case("L0.text-error-form", json!({"textErrorPrefix": "error"}), &[]),
        case("L0.bare", json!({"bareCode": "USAGE"}), &[]),
        case("L0.unknown-option", json!({"unknownOptionOmitsField": true}), &[]),
        case("L0.json-mode-preparse", json!({"unknownOptionAfterJsonIsText": true}), &[]),
        case("L0.after-dashdash", json!({"jsonAfterDashdash": true}), &[]),
        case("L0.ok-exit", json!({"unknownOptionExit": 1}), &[]),
        case("L0.no-null", json!({"hintNull": true}), &[]),
        case("L0.compact", json!({"pretty": true}), &[]),
        case("L0.one-document", json!({"probeTwoDocuments": true}), &[]),
        case("L0.no-env-switch", json!({"outputFormatSwitchesJson": true}), &[]),
        case("L0.no-env-switch-text", json!({"ciSwitchesText": true}), &[]),
        case("L0.contract", json!({"omitContract": true}), &[]),
    ]
}

pub fn seeded_for(id: &str) -> Seeded {
    seeded().into_iter().find(|seeded| seeded.id == id).unwrap_or_else(|| panic!("no seeded violation for {id}"))
}

/// Whether the report breaks exactly the seeded rule: the seeded check fails with a reason and rules, and every other
/// MUST check passes (collateral SHOULD checks named by the case may fail with it). `Err` says why not.
pub fn seeded_verdict(data: &Value, seeded: &Seeded) -> Result<(), String> {
    let checks = data["checks"].as_array().ok_or("data.checks is missing")?;
    if checks.len() != 16 {
        return Err(format!("{} checks, expected 16", checks.len()));
    }
    for check in checks {
        let id = check["id"].as_str().unwrap_or_default();
        let result = check["result"].as_str().unwrap_or_default();
        if id == seeded.id {
            if result != "fail" {
                return Err(format!("{id} is {result}, expected fail"));
            }
            if check["reason"].as_str().is_none_or(str::is_empty) || check["rules"].as_array().is_none_or(Vec::is_empty) {
                return Err(format!("{id} fails without a reason and rule numbers"));
            }
        } else if !seeded.collateral.contains(&id) && result != "pass" {
            return Err(format!("{id} is {result}, but only {} was seeded", seeded.id));
        }
    }
    Ok(())
}
