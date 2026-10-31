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
        assert!(
            self.stdout.ends_with('\n') && self.stdout.matches('\n').count() == 1,
            "stdout is not one line: {:?}",
            self.stdout
        );
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("stdout is not JSON ({error}): {:?}", self.stdout))
    }

    /// `data.checks` as `(id, result)` pairs.
    pub fn results(&self) -> Vec<(String, String)> {
        let document = self.envelope();
        document["data"]["checks"]
            .as_array()
            .expect("data.checks")
            .iter()
            .map(|check| {
                (
                    check["id"].as_str().unwrap_or_default().to_string(),
                    check["result"].as_str().unwrap_or_default().to_string(),
                )
            })
            .collect()
    }
}

/// Runs `atip` with `args` in `cwd`, its scratch space in `tmp`.
pub fn atip_in(cwd: &Path, tmp: &Path, args: &[&str]) -> Output {
    let output = Command::new(ATIP)
        .args(args)
        .current_dir(cwd)
        .env("TMPDIR", tmp)
        .output()
        .expect("run atip");
    Output {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// A scratch directory with the fake tool's configuration and fixtures in it.
pub struct Harness {
    pub dir: tempfile::TempDir,
}

impl Harness {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    pub fn write(&self, name: &str, document: &Value) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, serde_json::to_string_pretty(document).expect("json"))
            .expect("write");
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
        let mut args = vec![
            "check",
            "--bin",
            FAKE_TOOL,
            "--level",
            "L0",
            "--fixture",
            fixture_path.to_str().expect("utf8"),
        ];
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

/// The registry rows a conforming fake tool has: the four reserved codes it can produce and `STATE_ODD`, a code with exit 1.
pub fn standard_registry() -> Value {
    json!([
        {"code": "USAGE", "exit": 2, "retryable": false, "meaning": "m", "fix": "f"},
        {"code": "UNKNOWN_COMMAND", "exit": 2, "retryable": false, "meaning": "m", "fix": "f"},
        {"code": "COMMAND_REQUIRED", "exit": 2, "retryable": false, "meaning": "m", "fix": "f"},
        {"code": "INTERNAL", "exit": 1, "retryable": false, "meaning": "m", "fix": "f"},
        {"code": "STATE_ODD", "exit": 1, "retryable": false, "meaning": "m", "fix": "f"}
    ])
}

/// One pass condition of one L0 check, and the fake tool configuration (and fixture change) that breaks that condition and no other.
pub struct Case {
    /// `<check without L0.>/<condition>`, as the checker names it.
    pub condition: &'static str,
    pub config: Value,
    /// A change to the fixture the case needs, when the configuration alone cannot break the condition.
    pub tweak: Option<fn(&mut Value)>,
}

impl Case {
    /// The check that owns the condition: `L0.<name>`.
    pub fn owner(&self) -> String {
        format!(
            "L0.{}",
            self.condition.split('/').next().unwrap_or_default()
        )
    }

    /// The fixture the case runs with.
    pub fn fixture(&self, harness: &Harness) -> Value {
        let mut fixture = harness.fixture(&self.config);
        if let Some(tweak) = self.tweak {
            tweak(&mut fixture);
        }
        fixture
    }
}

fn unknown_command_exit_1(fixture: &mut Value) {
    fixture["registry"][1]["exit"] = json!(1);
}

/// Every pass condition of the 19 L0 checks, each with the configuration that breaks it: the seeded violations of
/// approaches/checker.md 7.2, one per condition. The tests require that exactly the owner of the condition fails.
pub fn cases() -> Vec<Case> {
    let case = |condition, config| Case {
        condition,
        config,
        tweak: None,
    };
    vec![
        case("version/exit-0", json!({"versionExit": 1})),
        case(
            "version/stdout-non-empty",
            json!({"versionFormat": "empty"}),
        ),
        case(
            "version-format/tool-version-line",
            json!({"versionFormat": "bare"}),
        ),
        case(
            "unknown-command/envelope",
            json!({"unknownCommandText": true}),
        ),
        case(
            "unknown-command/code",
            json!({"unknownCommandCode": "USAGE"}),
        ),
        case(
            "unknown-command/expected",
            json!({"unknownCommandNoExpected": true}),
        ),
        case(
            "unknown-command/no-meta-command",
            json!({"unknownCommandMeta": true}),
        ),
        case(
            "unknown-command/schema-version-1-0",
            json!({"unknownCommandSchemaVersion": "2.0"}),
        ),
        case(
            "unknown-command/stderr-carries-no-error",
            json!({"mirrorStderrFor": ["unknown-command"]}),
        ),
        case(
            "unknown-command-text/exit-2",
            json!({"textUnknownCommandExit": 1}),
        ),
        case(
            "unknown-command-text/stdout-empty",
            json!({"textErrorOnStdout": true}),
        ),
        case(
            "unknown-command-text/stderr-non-empty",
            json!({"textErrorSilent": true}),
        ),
        case(
            "text-error-form/prefix",
            json!({"textErrorPrefix": "error"}),
        ),
        case("bare/envelope", json!({"bareText": true})),
        case("bare/code", json!({"bareCode": "USAGE"})),
        case(
            "bare/stderr-carries-no-error",
            json!({"mirrorStderrFor": ["bare"]}),
        ),
        case(
            "unknown-option/code-is-a-refusal",
            json!({"unknownOptionCode": "STATE_ODD"}),
        ),
        case(
            "unknown-option/field",
            json!({"unknownOptionOmitsField": true}),
        ),
        case(
            "unknown-option/meta-command",
            json!({"unknownOptionOmitsCommand": true}),
        ),
        case(
            "unknown-option/stderr-carries-no-error",
            json!({"mirrorStderrFor": ["unknown-option"]}),
        ),
        case(
            "json-mode-preparse/json-first",
            json!({"unknownOptionAfterJsonIsText": true}),
        ),
        case(
            "json-mode-preparse/json-last",
            json!({"unknownOptionBeforeJsonIsText": true}),
        ),
        case(
            "after-dashdash/not-an-envelope",
            json!({"jsonAfterDashdash": true}),
        ),
        case("ok-exit/exit-range", json!({"dashdashExit3": true})),
        case("ok-exit/ok-iff-exit-0", json!({"probeSuccessExit": 1})),
        case(
            "ok-exit/code-registered",
            json!({"unknownOptionCode": "NOT_REGISTERED"}),
        ),
        case(
            "ok-exit/exit-is-registered",
            json!({"unknownOptionExit": 1}),
        ),
        case(
            "ok-exit/retryable-is-registered",
            json!({"retryableMismatch": true}),
        ),
        case("ok-exit/no-data-on-refusal", json!({"dataOnRefusal": true})),
        Case {
            condition: "ok-exit/reserved-codes",
            config: json!({"unknownCommandExit": 1}),
            tweak: Some(unknown_command_exit_1),
        },
        case(
            "ok-exit/schema-errors-match",
            json!({"offersSchemaErrors": true, "schemaErrorsDrift": true}),
        ),
        case(
            "refusal-effects/refusal-changes-nothing",
            json!({"refusalWrites": true}),
        ),
        case("no-null/valid-envelope", json!({"hintNull": true})),
        case("compact/no-indentation", json!({"pretty": true})),
        case("contract/meta-contract", json!({"omitContract": true})),
        case(
            "one-document/one-document-one-newline",
            json!({"probeTwoDocuments": true}),
        ),
        case(
            "no-env-switch/baseline-is-envelope",
            json!({"probeNotEnvelope": true}),
        ),
        case(
            "no-env-switch/same-envelope-kind",
            json!({"outputFormatSwitchesJson": true}),
        ),
        case(
            "no-env-switch/same-result",
            json!({"envChangesResult": true}),
        ),
        case(
            "no-env-switch/same-command",
            json!({"envChangesCommand": true}),
        ),
        case(
            "no-env-switch/same-schema-version",
            json!({"envChangesSchemaVersion": true}),
        ),
        case(
            "no-env-switch/same-data-keys",
            json!({"envChangesDataKeys": true}),
        ),
        case("no-env-switch/no-prompt", json!({"promptOnStdin": true})),
        case(
            "no-env-switch-text/same-envelope-kind",
            json!({"ciSwitchesText": true}),
        ),
        case("next/entries-valid", json!({"nextEmptyArgv": true})),
        case("next/no-placeholder", json!({"nextPlaceholder": true})),
        case("next/json-flag", json!({"nextWithoutJson": true})),
        case(
            "next/command-matches-argv",
            json!({"nextCommandMismatch": true}),
        ),
        case("page/page-valid", json!({"pageBadShape": true})),
        case("page/next-cursor-entry", json!({"pageNoCursorEntry": true})),
    ]
}

/// The cases of the check `id` (`L0.<name>`).
pub fn cases_of(id: &str) -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|case| case.owner() == id)
        .collect()
}

/// Whether the report breaks the case's condition and nothing else: its owner fails with a reason and rule numbers,
/// and every other check passes (or does not apply). `Err` says why not.
pub fn case_verdict(data: &Value, owner: &str) -> Result<(), String> {
    let checks = data["checks"].as_array().ok_or("data.checks is missing")?;
    if checks.len() != 19 {
        return Err(format!("{} checks, expected 19", checks.len()));
    }
    for check in checks {
        let id = check["id"].as_str().unwrap_or_default();
        let result = check["result"].as_str().unwrap_or_default();
        if id == owner {
            if result != "fail" {
                return Err(format!("{id} is {result}, expected fail"));
            }
            if check["reason"].as_str().is_none_or(str::is_empty)
                || check["rules"].as_array().is_none_or(Vec::is_empty)
            {
                return Err(format!("{id} fails without a reason and rule numbers"));
            }
        } else if !matches!(result, "pass" | "unsupported") {
            return Err(format!("{id} is {result}, but only {owner} was seeded"));
        }
    }
    Ok(())
}
