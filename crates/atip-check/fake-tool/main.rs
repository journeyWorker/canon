//! A fake tool whose behaviour is a JSON configuration: the test subject of `atip check`'s own tests.
//!
//! The configuration file is named by `FAKE_ATIP_CONFIG` (the fixture's `env` passes it). With no configuration
//! the tool follows atip/1 1.1 L0 to the letter (`"claims": "1.0"` makes it a 1.0 tool). Each member below breaks
//! the one rule its name says; `tests/support/mod.rs` maps every condition of every L0 check to the configuration
//! that breaks it.
//!
//! Commands: `init` (writes `state.txt`; `initLog` appends a line to a file), `status` (the probe: prints a small
//! report with a `next` entry, a warning and a page; `--cursor c1` is the second page), `touch` (needs `--name`;
//! `--input` is refused with a reason), `reject` (a `USAGE` that names no argument), `team` (a group with the
//! command `add`), and `schema` (`--errors`, `--error`, `--warnings`, `--warning`). Everything else is unknown.

use std::io::{BufRead, IsTerminal, Read, Write};
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{json, Map, Value};

/// The registry the tool has, sorted by code: code, exit and the names of its reasons. `schema --errors --json`
/// prints it and the tests' fixture lists it.
const REGISTRY: [(&str, i64, &[&str]); 7] = [
    ("COMMAND_REQUIRED", 2, &[]),
    ("CURSOR_INVALID", 2, &[]),
    ("INPUT_INVALID", 2, &["NOT_JSON", "UNKNOWN_KEY"]),
    ("INTERNAL", 1, &[]),
    ("STATE_ODD", 1, &[]),
    ("UNKNOWN_COMMAND", 2, &[]),
    ("USAGE", 2, &[]),
];

/// The warning registry: codes that are only warnings.
const WARNINGS: [&str; 1] = ["STALE_VIEW"];

/// The options a command takes besides the tool-wide `--json` and `--repo`: name and whether it takes a value.
fn command_options(command: &str) -> &'static [(&'static str, bool)] {
    match command {
        "status" | "items" => &[("--cursor", true)],
        "touch" => &[("--name", true), ("--input", true)],
        "schema" => &[
            ("--errors", false),
            ("--warnings", false),
            ("--error", true),
            ("--warning", true),
        ],
        _ => &[],
    }
}

struct Tool {
    config: Map<String, Value>,
    json: bool,
}

impl Tool {
    fn flag(&self, name: &str) -> bool {
        self.config
            .get(name)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn text(&self, name: &str, default: &str) -> String {
        self.config
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or(default)
            .to_string()
    }

    fn number(&self, name: &str) -> Option<u64> {
        self.config.get(name).and_then(Value::as_u64)
    }

    /// Whether the member `name`, a list of kinds, names `kind`.
    fn lists(&self, name: &str, kind: &str) -> bool {
        self.config
            .get(name)
            .and_then(Value::as_array)
            .is_some_and(|kinds| kinds.iter().any(|item| item == kind))
    }

    fn name(&self) -> String {
        self.text("tool", "fake")
    }

    /// Whether the tool claims 1.1 (the default).
    fn is_1_1(&self) -> bool {
        self.text("claims", "1.1") == "1.1"
    }

    fn commands(&self) -> Vec<&'static str> {
        let mut commands = vec!["init", "items", "reject", "status", "team", "touch"];
        if !self.flag("noSchema") {
            commands.push("schema");
        }
        commands.sort_unstable();
        commands
    }

    fn registered_exit(code: &str) -> u8 {
        REGISTRY
            .iter()
            .find(|(name, _, _)| *name == code)
            .map_or(2, |(_, exit, _)| u8::try_from(*exit).unwrap_or(2))
    }

    fn meta(&self, command: Option<&str>) -> Map<String, Value> {
        let mut meta = Map::new();
        let schema_answer = command == Some("schema");
        let version = if schema_answer {
            self.text(
                "schemaAnswerVersion",
                if self.is_1_1() { "1.1" } else { "1.0" },
            )
        } else {
            "1.0".to_string()
        };
        meta.insert("schemaVersion".into(), json!(version));
        meta.insert("tool".into(), json!(self.name()));
        meta.insert("toolVersion".into(), json!("1.2.3"));
        if !self.flag("omitContract") {
            meta.insert("contract".into(), json!("atip/1"));
        }
        let markers = if self.is_1_1() {
            !self.flag("omitContractVersion")
        } else {
            self.flag("emitsMarkers")
        };
        if markers {
            meta.insert(
                "contractVersion".into(),
                json!(self.text("contractVersionSays", "1.1")),
            );
        }
        if self.flag("metaUnknownMember") {
            meta.insert("unchanged".into(), json!(true));
        }
        if let Some(command) = command {
            meta.insert("command".into(), json!(command));
        }
        meta
    }

    fn print_document(&self, document: &Value) {
        let text = if self.flag("pretty") {
            serde_json::to_string_pretty(document)
        } else {
            serde_json::to_string(document)
        }
        .unwrap_or_default();
        println!("{text}");
    }

    /// What goes to stderr besides the answer: the error again (a violation) or noise (only a recommendation).
    fn stderr_extras(&self, kind: &str, document: &Value) {
        if self.lists("mirrorStderrFor", kind)
            || (kind == "unknown-command" && self.flag("mirrorErrorOnStderr"))
        {
            eprintln!("{document}");
        }
        if self.lists("noiseStderrFor", kind) {
            eprintln!("warming up");
        }
    }

    /// The counter a refusal bumps when `refusalWritesRemote` says so: a file outside the directory and `HOME`,
    /// which the probe reports.
    fn remote_file(&self) -> Option<std::path::PathBuf> {
        std::env::var_os("FAKE_ATIP_REMOTE").map(|dir| std::path::Path::new(&dir).join("count"))
    }

    fn remote_count(&self) -> u64 {
        self.remote_file()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(0)
    }

    /// A refusal or failure: writes what a seeded refusal leaves behind, then prints the error in the mode the call is in.
    /// `kind` names the situation (`unknown-command`, `bare`, `unknown-option`, `usage`) for the knobs that break only it.
    fn fail(
        &self,
        kind: &str,
        code: &str,
        message: &str,
        command: Option<&str>,
        extras: &[(&str, Value)],
        exit: u8,
    ) -> ExitCode {
        if self.flag("refusalWrites") {
            std::fs::write("refused.txt", "written by a refusal").unwrap_or_default();
        }
        if self.flag("refusalWritesHome") {
            if let Some(home) = std::env::var_os("HOME") {
                std::fs::write(
                    std::path::Path::new(&home).join("telemetry.log"),
                    "refused\n",
                )
                .unwrap_or_default();
            }
        }
        if self.flag("refusalWritesRemote") && exit == 2 {
            if let Some(path) = self.remote_file() {
                std::fs::write(path, (self.remote_count() + 1).to_string()).unwrap_or_default();
            }
        }
        let mut error = Map::new();
        error.insert("code".into(), json!(code));
        error.insert("message".into(), json!(message));
        error.insert(
            "retryable".into(),
            json!(self.flag("retryableMismatch") && kind == "unknown-command"),
        );
        if self.flag("hintNull") {
            error.insert("hint".into(), Value::Null);
        }
        for (key, value) in extras {
            error.insert((*key).to_string(), value.clone());
        }
        let mut document = json!({"ok": false, "error": error, "meta": self.meta(command)});
        if self.flag("dataOnRefusal") && exit == 2 {
            document["data"] = json!({"partial": true});
        }
        if self.json {
            let as_text = (kind == "unknown-command" && self.flag("unknownCommandText"))
                || (kind == "bare" && self.flag("bareText"));
            if as_text {
                println!("{code}: {message}");
            } else {
                self.print_document(&document);
                if kind == "unknown-command" && self.flag("unknownCommandTwoDocuments") {
                    self.print_document(&document);
                }
            }
            if kind == "bare" && self.flag("bareNoNewline") {
                print!("{document}");
            }
            self.stderr_extras(kind, &document);
        } else {
            let prefix = self.text("textErrorPrefix", &self.name());
            let line = format!("{prefix}: {code}: {message}");
            if kind == "unknown-command" && self.flag("textErrorOnStdout") {
                println!("{line}");
            }
            if kind == "unknown-option" && (self.flag("unknownOptionAfterJsonIsText") || self.flag("unknownOptionBeforeJsonIsText")) {
                println!("{line}");
            } else if !(kind == "unknown-command" && self.flag("textErrorSilent")) {
                eprintln!("{line}");
            }
        }
        ExitCode::from(exit)
    }

    fn unknown_command(&self, word: &str, expected: &[&str], command: Option<&str>) -> ExitCode {
        let mut code = self.text("unknownCommandCode", "UNKNOWN_COMMAND");
        if !self.json {
            code = "UNKNOWN_COMMAND".to_string();
        }
        let mut extras: Vec<(&str, Value)> =
            vec![("received", json!(word)), ("field", json!("command"))];
        if !self.flag("unknownCommandNoExpected") {
            extras.push(("expected", json!(expected)));
        }
        let command = command.or_else(|| self.flag("unknownCommandMeta").then_some(word));
        let exit = if self.json {
            self.number("unknownCommandExit")
                .map_or(Self::registered_exit(&code), |exit| {
                    u8::try_from(exit).unwrap_or(2)
                })
        } else {
            self.number("textUnknownCommandExit")
                .map_or(2, |exit| u8::try_from(exit).unwrap_or(2))
        };
        self.fail_versioned(
            "unknown-command",
            &code,
            &format!("{word} is not a command"),
            command,
            &extras,
            exit,
        )
    }

    /// [`Tool::fail`], with the schema version of the unknown-command answer seeded when asked to.
    fn fail_versioned(
        &self,
        kind: &str,
        code: &str,
        message: &str,
        command: Option<&str>,
        extras: &[(&str, Value)],
        exit: u8,
    ) -> ExitCode {
        if kind == "unknown-command" && self.json {
            if let Some(version) = self
                .config
                .get("unknownCommandSchemaVersion")
                .and_then(Value::as_str)
            {
                // Rebuild the document with the other version: the one place a version differs from "1.0".
                let mut meta = self.meta(command);
                meta.insert("schemaVersion".into(), json!(version));
                let mut error = Map::new();
                error.insert("code".into(), json!(code));
                error.insert("message".into(), json!(message));
                error.insert("retryable".into(), json!(false));
                for (key, value) in extras {
                    error.insert((*key).to_string(), value.clone());
                }
                self.print_document(&json!({"ok": false, "error": error, "meta": meta}));
                return ExitCode::from(exit);
            }
        }
        self.fail(kind, code, message, command, extras, exit)
    }

    fn unknown_option(&self, option: &str, command: Option<&str>) -> ExitCode {
        let code = self.text("unknownOptionCode", "USAGE");
        let exit = if self.flag("exitOutOfRange") {
            3
        } else {
            self.number("unknownOptionExit")
                .map_or(Self::registered_exit(&code), |exit| {
                    u8::try_from(exit).unwrap_or(2)
                })
        };
        let mut extras: Vec<(&str, Value)> = Vec::new();
        if !self.flag("unknownOptionOmitsField") {
            extras.push(("field", json!(option)));
        } else {
            extras.push(("hint", json!(format!("{} schema {} --json", self.name(), command.unwrap_or_default()))));
        }
        let command = if self.flag("unknownOptionOmitsCommand") {
            None
        } else {
            command
        };
        self.fail(
            "unknown-option",
            &code,
            &format!("{option} is not an option"),
            command,
            &extras,
            exit,
        )
    }

    /// A refusal with a code, a field and the hint a seeded violation may change.
    fn refuse(&self, code: &str, message: &str, command: &str, extras: &[(&str, Value)]) -> ExitCode {
        self.fail("usage", code, message, Some(command), extras, Self::registered_exit(code))
    }

    fn success(&self, command: &str, data: Value) -> ExitCode {
        if self.flag("successWrites") {
            std::fs::write("success-cache.txt", "written by a success").unwrap_or_default();
        }
        if self.flag("dirtyMarker") && command != "init" {
            // What a successful call leaves behind: an invocation that starts in the same directory fails.
            std::fs::write("dirty.txt", "x").unwrap_or_default();
        }
        let ci = std::env::var_os("CI").is_some();
        if self.json {
            if command == "status" && self.flag("probeNotEnvelope") {
                println!("status: ok");
                return ExitCode::SUCCESS;
            }
            let mut data = data;
            let mut meta = self.meta(Some(command));
            if command == "status" || command == "items" {
                self.shape_status(command, &mut data, &mut meta, ci);
            }
            let document = json!({"ok": true, "data": data, "meta": meta});
            self.print_document(&document);
            if command == "status" && self.flag("probeTwoDocuments") {
                self.print_document(&document);
            }
            if command == "status" || command == "items" {
                self.stderr_extras("status", &document);
            }
        } else if self.flag("ciSwitchesText") && ci {
            self.print_document(
                &json!({"ok": true, "data": data, "meta": self.meta(Some(command))}),
            );
        } else {
            println!("{command}: ok");
        }
        ExitCode::from(if command == "status" && self.json {
            u8::try_from(self.number("probeSuccessExit").unwrap_or(0)).unwrap_or(0)
        } else {
            0
        })
    }

    /// The probe's answer: a `next` entry, a warning and a page with a cursor, and the knobs that break them or make them depend on the environment.
    fn shape_status(&self, command: &str, data: &mut Value, meta: &mut Map<String, Value>, ci: bool) {
        data["remote"] = json!(self.remote_count());
        if !self.flag("noNextNoPage") {
            let tool = self.name();
            let mut next = vec![
                json!({"command": format!("{tool} {command} --json"), "argv": [command, "--json"], "reason": "read the status again"}),
            ];
            if self.flag("nextEmptyArgv") {
                next[0] = json!({"command": tool.clone(), "argv": []});
            }
            if self.flag("nextPlaceholder") {
                next[0] = json!({"command": format!("{tool} touch <name> --json"), "argv": ["touch", "<name>", "--json"]});
            }
            if self.flag("nextWithoutJson") {
                next[0] = json!({"command": format!("{tool} {command}"), "argv": [command]});
            }
            if self.flag("nextCommandMismatch") {
                next[0] = json!({"command": format!("{tool} {command} --json"), "argv": [command, "--quiet", "--json"]});
            }
            if !self.flag("pageNoCursorEntry") {
                next.push(json!({"command": format!("{tool} {command} --json --cursor c1"), "argv": [command, "--json", "--cursor", "c1"], "reason": "the next page"}));
            }
            meta.insert("next".into(), Value::Array(next));
            let page = if self.flag("pageBadShape") {
                json!({"returned": -1, "nextCursor": "c1"})
            } else {
                json!({"returned": 1, "nextCursor": "c1"})
            };
            meta.insert("page".into(), page);
        }
        if !self.flag("noWarning") {
            let code = if self.flag("unregisteredWarning") {
                "NOT_A_WARNING".to_string()
            } else if self.is_1_1() {
                WARNINGS[0].to_string()
            } else {
                "STATE_ODD".to_string()
            };
            meta.insert(
                "warnings".into(),
                json!([{"code": code, "message": "the view may be stale"}]),
            );
        }
        if ci && self.flag("envChangesCommand") {
            meta.insert("command".into(), json!("status-ci"));
        }
        if ci && self.flag("envChangesSchemaVersion") {
            meta.insert("schemaVersion".into(), json!("1.1"));
        }
        if ci && self.flag("envChangesDataKeys") {
            data["ci"] = json!(true);
        }
    }

    /// `status` when the environment changes its result: fails with the registered `STATE_ODD` and the same data.
    fn status_failure_in_ci(&self) -> ExitCode {
        let mut error = Map::new();
        error.insert("code".into(), json!("STATE_ODD"));
        error.insert("message".into(), json!("odd state"));
        error.insert("retryable".into(), json!(false));
        let document = json!({"ok": false, "error": error, "data": {"items": 0, "remote": self.remote_count(), "state": "ok"}, "meta": self.meta(Some("status"))});
        self.print_document(&document);
        ExitCode::from(1)
    }

    /// What a tool does that a value is missing for: refuses, or (seeded) prompts on a terminal.
    fn name_missing(&self) -> ExitCode {
        let on_terminal = std::io::stdin().is_terminal();
        if on_terminal {
            if self.flag("promptOnMissing") {
                print!("Name: ");
                std::io::stdout().flush().unwrap_or_default();
                let mut line = String::new();
                std::io::stdin().lock().read_line(&mut line).unwrap_or_default();
            }
            if let Some(seconds) = self.number("sleepOnMissing") {
                std::thread::sleep(Duration::from_secs(seconds));
            }
            if let Some(mode) = self.config.get("rawOnMissing").and_then(Value::as_str) {
                wait_in_raw_mode(mode);
            }
        }
        let refusal = self.refuse(
            "USAGE",
            "--name is required",
            "touch",
            &[("field", json!("--name"))],
        );
        if on_terminal {
            if let Some(length) = self.number("chatterOnTerminal") {
                println!("{}END-OF-CHATTER", "x".repeat(usize::try_from(length).unwrap_or(0)));
            }
        }
        refusal
    }

    /// `schema` with `--errors`, `--error`, `--warnings` or `--warning`.
    fn schema(&self, options: &[(String, Option<String>)]) -> ExitCode {
        let has = |name: &str| options.iter().any(|(option, _)| option == name);
        let value = |name: &str| {
            options
                .iter()
                .find(|(option, _)| option == name)
                .and_then(|(_, value)| value.clone())
        };
        let is_1_1 = self.is_1_1();
        let with_kind = |kind: &str, mut data: Value| {
            if is_1_1 {
                data["kind"] = json!(kind);
            }
            data
        };
        if has("--errors") {
            let rows = self.error_rows();
            let kind = self.text("errorsKind", "errors");
            let mut data = json!({"errors": rows});
            if is_1_1 {
                data["kind"] = json!(kind);
            }
            return self.success("schema", data);
        }
        if has("--error") {
            let code = value("--error").unwrap_or_default();
            let mut rows = self.error_rows();
            if self.flag("errorAnswerDiffers") && code == "USAGE" {
                if let Some(row) = rows.iter_mut().find(|row| row["code"] == "USAGE") {
                    row["meaning"] = json!("another meaning");
                }
            }
            let warning_only = self.warning_rows().iter().any(|row| row["code"] == code.as_str());
            if let Some(row) = rows.into_iter().find(|row| row["code"] == code.as_str()) {
                return self.success("schema", with_kind("error", json!({"error": row})));
            }
            if is_1_1 && warning_only && !self.flag("warningOnlyErrorAnswers") {
                let tool = self.name();
                // The seeded ways to refuse it wrongly: another code, another field, no hint, another exit.
                let field = self.text("warningOnlyErrorField", "--error");
                let refusal_code = self.text("warningOnlyErrorCode", "USAGE");
                let mut extras = vec![("field", json!(field))];
                extras.push(("received", json!(code)));
                if !self.flag("warningOnlyErrorNoHint") {
                    extras.push(("hint", json!(format!("{tool} schema --warning {} --json", value("--error").unwrap_or_default()))));
                }
                let exit = self
                    .number("warningOnlyErrorExit")
                    .map_or(Self::registered_exit(&refusal_code), |exit| u8::try_from(exit).unwrap_or(2));
                return self.fail("usage", &refusal_code, "that is a warning code, not an error code", Some("schema"), &extras, exit);
            }
            if is_1_1 && warning_only {
                return self.success("schema", with_kind("error", json!({"error": {"code": code, "meaning": "m", "exit": 1, "retryable": false, "fix": "f"}})));
            }
            return self.refuse(
                "USAGE",
                "not an error code",
                "schema",
                &[("field", json!("--error")), ("received", json!(code))],
            );
        }
        if is_1_1 && has("--warnings") {
            let rows = self.warning_rows();
            let kind = self.text("warningsKind", "warnings");
            return self.success("schema", json!({"kind": kind, "warnings": rows}));
        }
        if is_1_1 && has("--warning") {
            let code = value("--warning").unwrap_or_default();
            let mut rows = self.warning_rows();
            if self.flag("warningAnswerDiffers") {
                rows[0]["meaning"] = json!("another meaning");
            }
            if let Some(row) = rows.into_iter().find(|row| row["code"] == code.as_str()) {
                return self.success("schema", with_kind("warning", json!({"warning": row})));
            }
            return self.refuse(
                "USAGE",
                "not a warning code",
                "schema",
                &[("field", json!("--warning")), ("received", json!(code))],
            );
        }
        if is_1_1 && options.is_empty() {
            if let Some(shape) = self.config.get("schemaTree").and_then(Value::as_str) {
                let mut commands = vec![json!({"command": "items", "path": ["items"], "kind": "command", "summary": "items", "effect": "read"})];
                if shape == "groups" {
                    commands.push(json!({"command": "team", "path": ["team"], "kind": "group", "summary": "teams"}));
                }
                return self.success("schema", json!({"kind": "tree", "level": "L1", "commands": commands}));
            }
        }
        self.refuse(
            "USAGE",
            "schema takes --errors here",
            "schema",
            &[("field", json!("--errors"))],
        )
    }

    /// The registry rows `schema --errors --json` prints, with one row's exit changed when the registry is seeded to drift.
    fn error_rows(&self) -> Vec<Value> {
        REGISTRY
            .iter()
            .map(|(code, exit, reasons)| {
                let exit = if self.flag("schemaErrorsDrift") && *code == "STATE_ODD" {
                    2
                } else if *code == "UNKNOWN_COMMAND" {
                    self.number("reservedUnknownCommandExit").map_or(*exit, |exit| i64::try_from(exit).unwrap_or(2))
                } else {
                    *exit
                };
                let mut row = json!({"code": code, "exit": exit, "retryable": false, "meaning": "m", "fix": "f"});
                if self.is_1_1() && !reasons.is_empty() {
                    row["reasons"] = json!(reasons
                        .iter()
                        .map(|reason| json!({"reason": reason, "meaning": "m", "fix": "f"}))
                        .collect::<Vec<_>>());
                }
                row
            })
            .collect::<Vec<_>>()
            .into_iter()
            .enumerate()
            .map(|(position, mut row)| {
                if position == 0 && self.flag("registryEntryInvalid") {
                    if let Some(object) = row.as_object_mut() {
                        object.remove("fix");
                    }
                }
                row
            })
            .collect()
    }

    fn warning_rows(&self) -> Vec<Value> {
        let mut rows: Vec<Value> = WARNINGS
            .iter()
            .map(|code| json!({"code": code, "meaning": "m", "fix": "f"}))
            .collect();
        if self.flag("warningsDrift") {
            rows.push(json!({"code": "EXTRA_WARNING", "meaning": "m", "fix": "f"}));
        }
        if self.flag("warningEntryInvalid") {
            rows[0]["exit"] = json!(1);
        }
        rows
    }
}

/// Puts the terminal in raw mode and waits for a key: `prompt` prints a prompt first, `react` ends on any byte
/// (the end-of-file character arrives as `0x04`), `wait` ignores every byte until `q`.
fn wait_in_raw_mode(mode: &str) {
    use nix::sys::termios::{cfmakeraw, tcgetattr, tcsetattr, SetArg};
    let stdin = std::io::stdin();
    if let Ok(mut settings) = tcgetattr(&stdin) {
        cfmakeraw(&mut settings);
        tcsetattr(&stdin, SetArg::TCSANOW, &settings).unwrap_or_default();
    }
    if mode == "prompt" {
        print!("Press a key: ");
        std::io::stdout().flush().unwrap_or_default();
    }
    let mut byte = [0u8; 1];
    loop {
        match stdin.lock().read(&mut byte) {
            Ok(1) if mode == "react" || byte[0] == b'q' => return,
            Ok(1) => {}
            _ => return,
        }
    }
}

fn main() -> ExitCode {
    let config: Map<String, Value> = std::env::var_os("FAKE_ATIP_CONFIG")
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut tool = Tool {
        config,
        json: false,
    };
    tool.json = json_mode(&tool, &args);
    if let Some(seconds) = tool.number("hangOnVersion") {
        if args.first().map(String::as_str) == Some("--version") {
            std::thread::sleep(Duration::from_secs(seconds));
        }
    }
    if tool.flag("dirtyMarker") && std::path::Path::new("dirty.txt").exists() {
        eprintln!("state leaked in from an earlier invocation");
        return ExitCode::from(70);
    }
    run(&tool, &args)
}

/// JSON mode: `--json` before the first `--`, decided before parsing (spec 4.4), unless a seeded violation says otherwise.
fn json_mode(tool: &Tool, args: &[String]) -> bool {
    if tool.flag("jsonAfterDashdash") {
        return args.iter().any(|arg| arg == "--json");
    }
    let before: Vec<&String> = args.iter().take_while(|arg| arg.as_str() != "--").collect();
    let mut json = before.iter().any(|arg| arg.as_str() == "--json");
    // Seeded: an unknown option after `--json`, or before it, is reported as plain text.
    if tool.flag("unknownOptionAfterJsonIsText")
        && before
            .iter()
            .skip_while(|arg| arg.as_str() != "--json")
            .any(|arg| is_unknown_option(arg))
    {
        json = false;
    }
    if tool.flag("unknownOptionBeforeJsonIsText")
        && before
            .iter()
            .take_while(|arg| arg.as_str() != "--json")
            .any(|arg| is_unknown_option(arg))
    {
        json = false;
    }
    if tool.flag("outputFormatSwitchesJson") && std::env::var_os("OUTPUT_FORMAT").is_some() {
        json = false;
    }
    if tool.flag("ttySwitchesJson") && std::io::stdout().is_terminal() {
        json = false;
    }
    json
}

/// Whether `arg` is an option that no command of the tool has.
fn is_unknown_option(arg: &str) -> bool {
    let known = [
        "--json",
        "--version",
        "--repo",
        "--errors",
        "--warnings",
        "--error",
        "--warning",
        "--cursor",
        "--name",
        "--input",
    ];
    arg.starts_with("--") && arg != "--" && !known.contains(&arg)
}

#[expect(
    unsafe_code,
    reason = "libc::raise is an unsafe FFI call; the seeded violation is a tool that dies by a signal"
)]
fn die_by_signal() {
    // SAFETY: raises SIGTERM on this process; nothing else runs afterwards.
    unsafe {
        libc::raise(libc::SIGTERM);
    }
}

fn run(tool: &Tool, args: &[String]) -> ExitCode {
    let mut words: Vec<String> = Vec::new();
    let mut options: Vec<(String, Option<String>)> = Vec::new();
    let mut repo_given = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--" => {
                // The seeded ways to end `<probe> -- --json`: by a signal, as a launcher reports one, or with an exit outside 0 to 2.
                if tool.flag("dashdashSignal") {
                    die_by_signal();
                }
                for (knob, exit) in [("dashdashExit130", 130), ("dashdashExit3", 3)] {
                    if tool.flag(knob) {
                        return ExitCode::from(exit);
                    }
                }
                break;
            }
            "--json" => {}
            "--version" if words.is_empty() => return version(tool),
            "--repo" => {
                repo_given = true;
                rest.next();
            }
            option if option.starts_with("--") => {
                let command = words.first().map(String::as_str).filter(|word| tool.commands().contains(word));
                let takes = command
                    .map(command_options)
                    .unwrap_or_default()
                    .iter()
                    .find(|(name, _)| *name == option && !(*name == "--cursor" && tool.flag("noPaging")));
                match takes {
                    Some((_, true)) => options.push((option.to_string(), rest.next().cloned())),
                    Some((_, false)) => options.push((option.to_string(), None)),
                    None if is_unknown_option(option) || command.is_some() => {
                        return tool.unknown_option(option, command)
                    }
                    None => options.push((option.to_string(), None)),
                }
            }
            word => words.push(word.to_string()),
        }
    }
    words.extend(rest.cloned());
    let Some(command) = words.first() else {
        let code = tool.text("bareCode", "COMMAND_REQUIRED");
        return tool.fail(
            "bare",
            &code,
            "a command is required",
            None,
            &[("expected", json!(tool.commands()))],
            Tool::registered_exit(&code),
        );
    };
    if !tool.commands().contains(&command.as_str()) {
        return tool.unknown_command(command, &tool.commands(), None);
    }
    let has = |name: &str| options.iter().any(|(option, _)| option == name);
    let value = |name: &str| {
        options
            .iter()
            .find(|(option, _)| option == name)
            .and_then(|(_, value)| value.clone())
    };
    if command == "team" {
        return team(tool, &words, repo_given);
    }
    if let Some(extra) = words.get(1) {
        return tool.fail(
            "usage",
            "USAGE",
            &format!("{command} takes no operand, got {extra}"),
            Some(command),
            &[("field", json!("ARGUMENT"))],
            2,
        );
    }
    match command.as_str() {
        "init" => {
            std::fs::write("state.txt", "initialised").unwrap_or_default();
            if let Some(log) = tool.config.get("initLog").and_then(Value::as_str) {
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log)
                    .unwrap_or_else(|_| std::process::exit(70));
                std::io::Write::write_all(&mut file, b"init\n").unwrap_or_default();
            }
            tool.success("init", json!({"initialised": true}))
        }
        "touch" => touch(tool, &value("--name"), &value("--input")),
        "reject" => {
            let mut extras: Vec<(&str, Value)> = Vec::new();
            if !tool.flag("usageNoHint") {
                extras.push(("hint", json!(format!("{} schema reject --json", tool.name()))));
            }
            tool.refuse("USAGE", "the backend rejected the request", "reject", &extras)
        }
        "schema" => tool.schema(&options),
        _ if has("--cursor") && tool.flag("cursorNeedsName") && value("--cursor").as_deref() == Some("x") => tool.refuse(
            "USAGE",
            "--name is required",
            command,
            &[("field", json!("--name"))],
        ),
        _ if has("--cursor") => match value("--cursor").as_deref() {
            Some("c1") => tool.success(command, json!({"items": 0, "state": "second page"})),
            other => tool.refuse(
                "CURSOR_INVALID",
                "that is not a cursor of this command",
                command,
                &[("field", json!("--cursor")), ("received", json!(other.unwrap_or_default()))],
            ),
        },
        _ if tool.json && tool.flag("envChangesResult") && std::env::var_os("CI").is_some() => {
            tool.status_failure_in_ci()
        }
        _ => tool.success(command, json!({"items": 0, "state": "ok"})),
    }
}

/// The group `team`: not runnable, with the one command `add`; the tool-wide `--repo` is no error on it.
fn team(tool: &Tool, words: &[String], repo_given: bool) -> ExitCode {
    match words.get(1).map(String::as_str) {
        None if tool.flag("groupRejectsGlobal") && repo_given => tool.refuse(
            "USAGE",
            "team takes no --repo",
            "team",
            &[("field", json!("--repo"))],
        ),
        None if tool.flag("groupRunnable") && !repo_given => tool.success("team", json!({"team": true})),
        None => tool.fail(
            "group",
            "COMMAND_REQUIRED",
            "team needs a command",
            Some("team"),
            &[("expected", json!(["add"]))],
            2,
        ),
        Some("add") if tool.flag("teamAddNoEnvelope") && tool.json => {
            println!("{}", json!({"ok": true, "data": {}}));
            ExitCode::SUCCESS
        }
        // A stream command (13.5): a start line, an event, an end line holding the envelope.
        Some("add") if tool.flag("teamAddStream") && tool.json => {
            let version = tool.text("streamStartContractVersion", "1.1");
            let mut start = json!({"type": "start", "contract": "atip/1", "tool": tool.name(), "toolVersion": "1.2.3", "command": "team add", "schemaVersion": "1.0"});
            if version != "none" {
                start["contractVersion"] = json!(version);
            }
            println!("{start}");
            println!("{}", json!({"type": "event", "data": {"step": "add"}}));
            let result = json!({"ok": true, "data": {"added": true}, "meta": tool.meta(Some("team add"))});
            println!("{}", json!({"type": "end", "result": result}));
            ExitCode::SUCCESS
        }
        // A document that is no envelope but carries an error code no registry has: only L0.envelope may judge it.
        Some("add") if tool.flag("teamAddNoEnvelopeCode") && tool.json => {
            println!("{}", json!({"ok": false, "error": {"code": "NOT_REGISTERED", "reason": "NOT_A_REASON", "message": "m", "retryable": false}}));
            ExitCode::SUCCESS
        }
        Some("add") => tool.success("team add", json!({"added": true})),
        Some(word) => tool.unknown_command(word, &["add"], Some("team")),
    }
}

/// `touch --name <NAME>`; `--input` is refused with a reason.
fn touch(tool: &Tool, name: &Option<String>, input: &Option<String>) -> ExitCode {
    if input.is_some() {
        let mut extras: Vec<(&str, Value)> = vec![("field", json!("--input"))];
        let reason = if tool.flag("unregisteredReason") {
            Some("NOT_A_REASON".to_string())
        } else if tool.is_1_1() || tool.flag("reasonIn10") {
            Some("NOT_JSON".to_string())
        } else {
            None
        };
        if let Some(reason) = reason {
            extras.push(("reason", json!(reason)));
        }
        if tool.flag("proseHintStartsWithTool") {
            extras.push(("hint", json!(format!("{} can't read this document; quote it in single quotes.", tool.name()))));
        }
        return tool.refuse("INPUT_INVALID", "--input is not JSON", "touch", &extras);
    }
    if name.is_none() {
        return tool.name_missing();
    }
    std::fs::write("touched.txt", "x").unwrap_or_default();
    tool.success("touch", json!({"touched": true}))
}

fn version(tool: &Tool) -> ExitCode {
    if tool.flag("versionStderr") {
        eprintln!("warming up");
    }
    match tool.text("versionFormat", "line").as_str() {
        "bare" => println!("1.2.3"),
        "empty" => {}
        _ => println!("{} 1.2.3", tool.name()),
    }
    ExitCode::from(u8::try_from(tool.number("versionExit").unwrap_or(0)).unwrap_or(0))
}
