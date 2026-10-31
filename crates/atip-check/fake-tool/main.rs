//! A fake tool whose behaviour is a JSON configuration: the test subject of `atip check`'s own tests.
//!
//! The configuration file is named by `FAKE_ATIP_CONFIG` (the fixture's `env` passes it). With no configuration
//! the tool follows atip/1 L0 to the letter. Each member below breaks the one rule its name says;
//! `tests/support/mod.rs` maps every condition of every L0 check to the configuration that breaks it.
//!
//! Commands: `init` (writes `state.txt`; `initLog` appends a line to a file), `status` (the probe: prints a
//! small report with a `next` entry and a page), `touch` (writes `touched.txt` and succeeds), and `schema`
//! (only with `offersSchemaErrors`: `schema --errors --json` prints the registry). Everything else is unknown.

use std::io::{BufRead, IsTerminal};
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{json, Map, Value};

/// The registry the tool has: what `schema --errors --json` prints and what the tests' fixture lists.
const REGISTRY: [(&str, i64); 5] = [
    ("USAGE", 2),
    ("UNKNOWN_COMMAND", 2),
    ("COMMAND_REQUIRED", 2),
    ("INTERNAL", 1),
    ("STATE_ODD", 1),
];

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

    fn commands(&self) -> Vec<&'static str> {
        let mut commands = vec!["init", "status", "touch"];
        if self.flag("offersSchemaErrors") {
            commands.push("schema");
        }
        commands
    }

    fn registered_exit(code: &str) -> u8 {
        REGISTRY
            .iter()
            .find(|(name, _)| *name == code)
            .map_or(2, |(_, exit)| u8::try_from(*exit).unwrap_or(2))
    }

    fn meta(&self, command: Option<&str>) -> Map<String, Value> {
        let mut meta = Map::new();
        meta.insert("schemaVersion".into(), json!("1.0"));
        meta.insert("tool".into(), json!(self.name()));
        meta.insert("toolVersion".into(), json!("1.2.3"));
        if !self.flag("omitContract") {
            meta.insert("contract".into(), json!("atip/1"));
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
            if !(kind == "unknown-command" && self.flag("textErrorSilent")) {
                eprintln!("{line}");
            }
        }
        ExitCode::from(exit)
    }

    fn unknown_command(&self, word: &str) -> ExitCode {
        let mut code = self.text("unknownCommandCode", "UNKNOWN_COMMAND");
        if !self.json {
            code = "UNKNOWN_COMMAND".to_string();
        }
        let mut extras: Vec<(&str, Value)> =
            vec![("received", json!(word)), ("field", json!("command"))];
        if !self.flag("unknownCommandNoExpected") {
            extras.push(("expected", json!(self.commands())));
        }
        let command = self.flag("unknownCommandMeta").then_some(word);
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
            if command == "status" {
                self.shape_status(&mut data, &mut meta, ci);
            }
            let document = json!({"ok": true, "data": data, "meta": meta});
            self.print_document(&document);
            if command == "status" && self.flag("probeTwoDocuments") {
                self.print_document(&document);
            }
            if command == "status" {
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

    /// The probe's answer: a `next` entry and a page with a cursor, and the knobs that break them or make them depend on the environment.
    fn shape_status(&self, data: &mut Value, meta: &mut Map<String, Value>, ci: bool) {
        if !self.flag("noNextNoPage") {
            let tool = self.name();
            let mut next = vec![
                json!({"command": format!("{tool} status --json"), "argv": ["status", "--json"], "reason": "read the status again"}),
            ];
            if self.flag("nextEmptyArgv") {
                next[0] = json!({"command": tool.clone(), "argv": []});
            }
            if self.flag("nextPlaceholder") {
                next[0] = json!({"command": format!("{tool} touch <name> --json"), "argv": ["touch", "<name>", "--json"]});
            }
            if self.flag("nextWithoutJson") {
                next[0] = json!({"command": format!("{tool} status"), "argv": ["status"]});
            }
            if self.flag("nextCommandMismatch") {
                next[0] = json!({"command": format!("{tool} status --json"), "argv": ["status", "--quiet", "--json"]});
            }
            if !self.flag("pageNoCursorEntry") {
                next.push(json!({"command": format!("{tool} status --json --cursor c1"), "argv": ["status", "--json", "--cursor", "c1"], "reason": "the next page"}));
            }
            meta.insert("next".into(), Value::Array(next));
            let page = if self.flag("pageBadShape") {
                json!({"returned": -1, "nextCursor": "c1"})
            } else {
                json!({"returned": 1, "nextCursor": "c1"})
            };
            meta.insert("page".into(), page);
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
        if self.flag("promptOnStdin") && std::io::stdin().is_terminal() {
            let mut line = String::new();
            std::io::stdin()
                .lock()
                .read_line(&mut line)
                .unwrap_or_default();
        }
    }

    /// `status` when the environment changes its result: fails with the registered `STATE_ODD` and the same data.
    fn status_failure_in_ci(&self) -> ExitCode {
        let mut error = Map::new();
        error.insert("code".into(), json!("STATE_ODD"));
        error.insert("message".into(), json!("odd state"));
        error.insert("retryable".into(), json!(false));
        let document = json!({"ok": false, "error": error, "data": {"items": 0, "state": "ok"}, "meta": self.meta(Some("status"))});
        self.print_document(&document);
        ExitCode::from(1)
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

fn is_unknown_option(arg: &str) -> bool {
    arg.starts_with("--")
        && arg != "--"
        && arg != "--json"
        && arg != "--version"
        && arg != "--errors"
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
    let mut rest = args.iter();
    let mut errors = false;
    for arg in rest.by_ref() {
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
            "--errors" => errors = true,
            "--version" if words.is_empty() => return version(tool),
            option if is_unknown_option(option) => {
                return tool.unknown_option(
                    option,
                    words
                        .first()
                        .map(String::as_str)
                        .filter(|word| tool.commands().contains(word)),
                )
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
        return tool.unknown_command(command);
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
        "touch" => {
            std::fs::write("touched.txt", "x").unwrap_or_default();
            tool.success("touch", json!({"touched": true}))
        }
        "schema" if errors => schema_errors(tool),
        "schema" => tool.fail(
            "usage",
            "USAGE",
            "schema takes --errors here",
            Some("schema"),
            &[("field", json!("--errors"))],
            2,
        ),
        _ if tool.json && tool.flag("envChangesResult") && std::env::var_os("CI").is_some() => {
            tool.status_failure_in_ci()
        }
        _ => tool.success("status", json!({"items": 0, "state": "ok"})),
    }
}

/// `schema --errors --json`: the registry, with one row's exit changed when the registry is seeded to drift.
fn schema_errors(tool: &Tool) -> ExitCode {
    let rows: Vec<Value> = REGISTRY
        .iter()
        .map(|(code, exit)| {
            let exit = if tool.flag("schemaErrorsDrift") && *code == "STATE_ODD" {
                2
            } else {
                *exit
            };
            json!({"code": code, "exit": exit, "retryable": false, "meaning": "m", "fix": "f"})
        })
        .collect();
    tool.success("schema", json!({"kind": "errors", "errors": rows}))
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
