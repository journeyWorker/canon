//! Shared by the integration tests: the `atip` binary, the fake tool, fixtures, and the seeded violations.

#![allow(dead_code, reason = "each integration test uses a part of this module")]

pub mod l1;
pub mod l2;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

pub const ATIP: &str = env!("CARGO_BIN_EXE_atip");
pub const FAKE_TOOL: &str = env!("CARGO_BIN_EXE_atip-fake-tool");

/// The number of checks of a run at L0: the 28 L0 checks of contract 1.1 and `L2.effects`, which every level reports (an advisory below L2).
pub const CHECKS: usize = 29;

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

    /// The check `id` of `data.checks`.
    pub fn check(&self, id: &str) -> Value {
        self.envelope()["data"]["checks"]
            .as_array()
            .expect("data.checks")
            .iter()
            .find(|check| check["id"] == id)
            .cloned()
            .unwrap_or_else(|| panic!("{id} is not in data.checks"))
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
    /// A fake tool that claims 1.0 (`"claims": "1.0"`) has the 1.0 registry and no `contractVersion`.
    pub fn fixture(&self, config: &Value) -> Value {
        let config_path = self.write("fake-config.json", config);
        let remote = self.path("remote");
        std::fs::create_dir_all(&remote).expect("remote");
        let is_1_1 = config["claims"] != "1.0";
        let mut fixture = json!({
            "setup": [["init"]],
            "env": {"FAKE_ATIP_CONFIG": config_path, "FAKE_ATIP_REMOTE": remote},
            "timeoutSeconds": 20,
            "probe": "status",
            "globalOptions": [["--repo", "."]],
            "registry": standard_registry(is_1_1),
            "commands": {
                "items": {"ok": [["items", "--json"]]},
                "touch": {
                    "missingValue": [["touch", "--json"]],
                    "refused": [["touch", "--input", "nope", "--json"]]
                },
                "team add": {"ok": [["team", "add", "--json"]]},
                "reject": {"refused": [["reject", "--json"]]}
            }
        });
        if is_1_1 {
            fixture["contractVersion"] = json!(config["claims"].as_str().unwrap_or("1.1"));
            fixture["warnings"] = json!(["STALE_VIEW"]);
        }
        fixture
    }

    /// `atip check --bin <fake tool> --level L0 --fixture <fixture> [--json]`, with `extra` arguments after.
    pub fn check_with(&self, fixture: &Value, json: bool, extra: &[&str]) -> Output {
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
        args.extend(extra);
        if json {
            args.push("--json");
        }
        let work = self.path("cwd");
        std::fs::create_dir_all(&work).expect("cwd");
        let tmp = self.path("tmp");
        std::fs::create_dir_all(&tmp).expect("tmp");
        atip_in(&work, &tmp, &args)
    }

    /// `atip check --bin <fake tool> --level L0 --fixture <fixture> [--json]`.
    pub fn check(&self, fixture: &Value, json: bool) -> Output {
        self.check_with(fixture, json, &[])
    }

    /// The check run as JSON against a fake tool configured by `config`.
    pub fn check_config(&self, config: &Value) -> Output {
        self.check(&self.fixture(config), true)
    }
}

/// The registry rows a conforming fake tool has, by name only: the reserved codes it can produce, `STATE_ODD`, a
/// code with exit 1, `CURSOR_INVALID` and `INPUT_INVALID` with its reasons (a 1.1 tool).
pub fn standard_registry(with_reasons: bool) -> Value {
    let mut rows = json!([
        {"code": "COMMAND_REQUIRED", "exit": 2, "retryable": false},
        {"code": "CURSOR_INVALID", "exit": 2, "retryable": false},
        {"code": "INPUT_INVALID", "exit": 2, "retryable": false},
        {"code": "INTERNAL", "exit": 1, "retryable": false},
        {"code": "STATE_ODD", "exit": 1, "retryable": false},
        {"code": "UNKNOWN_COMMAND", "exit": 2, "retryable": false},
        {"code": "USAGE", "exit": 2, "retryable": false}
    ]);
    if with_reasons {
        rows[2]["reasons"] = json!(["NOT_JSON", "UNKNOWN_KEY"]);
    }
    rows
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

    /// Whether the fake tool of the case claims 1.1.
    pub fn is_1_1(&self) -> bool {
        self.config["claims"] != "1.0"
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
    let rows = fixture["registry"].as_array_mut().expect("registry");
    for row in rows {
        if row["code"] == "UNKNOWN_COMMAND" {
            row["exit"] = json!(1);
        }
    }
}

/// Every pass condition of the 28 L0 checks, each with the configuration that breaks it: the seeded violations of
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
        case("version/stderr-empty", json!({"versionStderr": true})),
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
        case("bare/code", json!({"bareCode": "UNKNOWN_COMMAND"})),
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
            config: json!({"unknownCommandExit": 1, "reservedUnknownCommandExit": 1}),
            tweak: Some(unknown_command_exit_1),
        },
        case("no-null/no-null", json!({"hintNull": true})),
        case("envelope/valid-envelope", json!({"metaUnknownMember": true})),
        case("compact/no-indentation", json!({"pretty": true})),
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
        case(
            "no-env-switch-text/same-envelope-kind",
            json!({"ciSwitchesText": true}),
        ),
        case(
            "contract/meta-contract",
            json!({"claims": "1.0", "omitContract": true}),
        ),
        case(
            "contract-version/markers-present",
            json!({"omitContractVersion": true}),
        ),
        case(
            "contract-version/version-is-claim",
            json!({"contractVersionSays": "1.2"}),
        ),
        case(
            "contract-version/no-contract-version",
            json!({"claims": "1.0", "emitsMarkers": true}),
        ),
        case(
            "contract-version/no-later-members",
            json!({"claims": "1.0", "reasonIn10": true}),
        ),
        case(
            "contract-version/schema-version-is-claim",
            json!({"schemaAnswerVersion": "1.0"}),
        ),
        case(
            "stderr-result/stderr-carries-no-result",
            json!({"mirrorStderrFor": ["unknown-command"]}),
        ),
        case(
            "stderr-quiet/stderr-empty",
            json!({"noiseStderrFor": ["status"]}),
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
        case(
            "refusal-effects/refusal-changes-nothing",
            json!({"refusalWrites": true}),
        ),
        case(
            "refusal-effects/refusal-changes-no-remote-state",
            json!({"refusalWritesRemote": true}),
        ),
        case(
            "no-prompt/refuses-on-a-terminal",
            json!({"promptOnMissing": true}),
        ),
        case("registry/kind", json!({"errorsKind": "list"})),
        case(
            "registry/entries-valid",
            json!({"registryEntryInvalid": true}),
        ),
        case(
            "registry/error-answers",
            json!({"errorAnswerDiffers": true}),
        ),
        case(
            "registry/registry-is-the-fixtures",
            json!({"schemaErrorsDrift": true}),
        ),
        case("registry/warnings-kind", json!({"warningsKind": "list"})),
        case(
            "registry/warning-entries-valid",
            json!({"warningEntryInvalid": true}),
        ),
        case(
            "registry/warning-answers",
            json!({"warningAnswerDiffers": true}),
        ),
        case(
            "registry/warning-only-refused",
            json!({"warningOnlyErrorAnswers": true}),
        ),
        case(
            "registry/warning-only-code",
            json!({"warningOnlyErrorCode": "INPUT_INVALID"}),
        ),
        case(
            "registry/warning-only-field",
            json!({"warningOnlyErrorField": "--errors"}),
        ),
        case(
            "registry/warning-only-hint",
            json!({"warningOnlyErrorNoHint": true}),
        ),
        case(
            "registry/warnings-are-the-fixtures",
            json!({"warningsDrift": true}),
        ),
        case(
            "registry/emitted-codes-registered",
            json!({"unknownOptionCode": "NOT_REGISTERED"}),
        ),
        case(
            "registry/emitted-reasons-registered",
            json!({"unregisteredReason": true}),
        ),
        case(
            "registry/emitted-warnings-registered",
            json!({"unregisteredWarning": true}),
        ),
        case("usage-hint/hint-names-the-schema", json!({"usageNoHint": true})),
        case(
            "hint-form/hint-is-one-command-line",
            json!({"proseHintStartsWithTool": true}),
        ),
        case(
            "global-on-group/group-is-not-runnable",
            json!({"groupRunnable": true}),
        ),
        case(
            "global-on-group/option-after-group-is-no-error",
            json!({"groupRejectsGlobal": true}),
        ),
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
    if checks.len() != CHECKS {
        return Err(format!("{} checks, expected {CHECKS}", checks.len()));
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

/// What `reason` says of `messages` (approaches/checker.md 2): the first three, each cut at 240 characters, and how many more.
pub fn reason_of_messages(messages: &[String]) -> String {
    let cut = |message: &String| {
        let mut kept: String = message.chars().take(240).collect();
        if message.chars().count() > 240 {
            kept.push('…');
        }
        kept
    };
    let mut reason = messages.iter().take(3).map(cut).collect::<Vec<_>>().join("; ");
    if messages.len() > 3 {
        reason.push_str(&format!("; and {} more", messages.len() - 3));
    }
    reason
}

/// The `findings[]` of the report (approaches/checker.md 2): only a failed or unprovided check has them; each has `kind` and
/// `message` and no member but `argv` and `code` besides; a failed check's `reason` is the first three messages; a `code`
/// belongs to a refused fixture call; and the owner of the violated condition has a finding of kind `kind`.
pub fn kind_verdict(data: &Value, owner: &str, kind: &str) -> Result<(), String> {
    let checks = data["checks"].as_array().ok_or("data.checks is missing")?;
    for check in checks {
        let id = check["id"].as_str().unwrap_or_default();
        let result = check["result"].as_str().unwrap_or_default();
        let findings = check.get("findings");
        if !matches!(result, "fail" | "unprovided") {
            if findings.is_some() {
                return Err(format!("{id} is {result} and has findings"));
            }
            continue;
        }
        let findings = findings.and_then(Value::as_array).ok_or(format!("{id} is {result} without findings"))?;
        if findings.is_empty() {
            return Err(format!("{id} has an empty findings"));
        }
        for finding in findings {
            let object = finding.as_object().ok_or(format!("{id}: a finding is not an object"))?;
            if let Some(member) = object.keys().find(|key| !matches!(key.as_str(), "kind" | "message" | "argv" | "code")) {
                return Err(format!("{id}: a finding has the member {member}"));
            }
            if finding["kind"].as_str().is_none_or(str::is_empty) || finding["message"].as_str().is_none_or(str::is_empty) {
                return Err(format!("{id}: a finding without kind or message: {finding}"));
            }
            if object.contains_key("argv") && !finding["argv"].as_array().is_some_and(|argv| !argv.is_empty() && argv.iter().all(Value::is_string)) {
                return Err(format!("{id}: argv is not a list of words: {finding}"));
            }
            if object.contains_key("code") && finding["kind"] != "refused-precondition" {
                return Err(format!("{id}: a code on {}", finding["kind"]));
            }
        }
        if result == "fail" {
            let messages: Vec<String> = findings.iter().map(|finding| finding["message"].as_str().unwrap_or_default().to_string()).collect();
            let reason = check["reason"].as_str().unwrap_or_default();
            if reason != reason_of_messages(&messages) {
                return Err(format!("{id}: reason {reason:?} is not the first three messages of {messages:?}"));
            }
        }
    }
    let found = checks
        .iter()
        .find(|check| check["id"] == owner)
        .and_then(|check| check["findings"].as_array())
        .ok_or(format!("{owner} has no findings"))?;
    if !found.iter().any(|finding| finding["kind"] == kind) {
        let kinds: Vec<&str> = found.iter().filter_map(|finding| finding["kind"].as_str()).collect();
        return Err(format!("{owner} has no finding of kind {kind}: {kinds:?}"));
    }
    Ok(())
}
