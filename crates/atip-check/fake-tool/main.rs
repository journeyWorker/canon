//! A fake tool whose behaviour is a JSON configuration: the test subject of `atip check`'s own tests.
//!
//! The configuration file is named by `FAKE_ATIP_CONFIG` (the fixture's `env` passes it). With no
//! configuration the tool follows atip/1 L0 to the letter. Each member below breaks exactly one rule,
//! the one its name says; `crates/atip-check/tests/seeded_violations.rs` maps every L0 check id to
//! the member that makes that check fail, and approaches/checker.md 7.2 asks for exactly that.
//!
//! Commands: `init` (writes `state.txt`; `initLog` appends a line to that file), `status` (the probe: prints a small report), `touch`
//! (writes `touched.txt` and succeeds). Everything else is unknown.

use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{json, Map, Value};

const COMMANDS: [&str; 3] = ["init", "status", "touch"];

struct Tool {
    config: Map<String, Value>,
    json: bool,
}

impl Tool {
    fn flag(&self, name: &str) -> bool {
        self.config.get(name).and_then(Value::as_bool).unwrap_or(false)
    }

    fn text(&self, name: &str, default: &str) -> String {
        self.config.get(name).and_then(Value::as_str).unwrap_or(default).to_string()
    }

    fn name(&self) -> String {
        self.text("tool", "fake")
    }

    /// The code's registered exit; `unknownOptionExit` and `exitOutOfRange` are the seeded departures.
    fn exit_of(&self, code: &str) -> u8 {
        match code {
            "INTERNAL" => 1,
            _ => 2,
        }
    }

    fn meta(&self, command: Option<&str>) -> Value {
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
        Value::Object(meta)
    }

    fn print_document(&self, document: &Value) {
        let text = if self.flag("pretty") { serde_json::to_string_pretty(document) } else { serde_json::to_string(document) }.unwrap_or_default();
        println!("{text}");
    }

    /// Writes the files a seeded refusal leaves behind, then fails with the code's exit.
    fn refuse(&self, code: &str, message: &str, command: Option<&str>, extras: &[(&str, Value)], exit: u8) -> ExitCode {
        if self.flag("refusalWrites") {
            std::fs::write("refused.txt", "written by a refusal").unwrap_or_default();
        }
        if self.flag("refusalWritesHome") {
            if let Some(home) = std::env::var_os("HOME") {
                std::fs::write(std::path::Path::new(&home).join("telemetry.log"), "refused\n").unwrap_or_default();
            }
        }
        let retryable = self.flag("retryableMismatch") && code == "UNKNOWN_COMMAND";
        let mut error = Map::new();
        error.insert("code".into(), json!(code));
        error.insert("message".into(), json!(message));
        error.insert("retryable".into(), json!(retryable));
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
            self.print_document(&document);
            if self.flag("mirrorErrorOnStderr") {
                eprintln!("{document}");
            }
        } else {
            let stream_is_stdout = self.flag("textErrorOnStdout");
            let prefix = self.text("textErrorPrefix", &self.name());
            let line = format!("{prefix}: {code}: {message}");
            if stream_is_stdout {
                println!("{line}");
            } else {
                eprintln!("{line}");
            }
        }
        ExitCode::from(exit)
    }

    fn usage_for_option(&self, option: &str, command: Option<&str>) -> ExitCode {
        let exit = if self.flag("exitOutOfRange") { 3 } else { u8::try_from(self.config.get("unknownOptionExit").and_then(Value::as_u64).unwrap_or(2)).unwrap_or(2) };
        let mut extras: Vec<(&str, Value)> = Vec::new();
        if !self.flag("unknownOptionOmitsField") {
            extras.push(("field", json!(option)));
        }
        let command = if self.flag("unknownOptionOmitsCommand") { None } else { command };
        self.refuse("USAGE", &format!("{option} is not an option"), command, &extras, exit)
    }

    fn success(&self, command: &str, data: Value) -> ExitCode {
        if self.flag("successWrites") {
            std::fs::write("success-cache.txt", "written by a success").unwrap_or_default();
        }
        if self.flag("dirtyMarker") && command != "init" {
            // What a successful call leaves behind: an invocation that starts in the same directory fails.
            std::fs::write("dirty.txt", "x").unwrap_or_default();
        }
        if self.json {
            let document = json!({"ok": true, "data": data, "meta": self.meta(Some(command))});
            self.print_document(&document);
            if self.flag("probeTwoDocuments") && command == "status" {
                self.print_document(&document);
            }
        } else if self.flag("ciSwitchesText") && std::env::var_os("CI").is_some() {
            self.print_document(&json!({"ok": true, "data": data, "meta": self.meta(Some(command))}));
        } else {
            println!("{command}: ok");
        }
        ExitCode::SUCCESS
    }
}

fn main() -> ExitCode {
    let config: Map<String, Value> = std::env::var_os("FAKE_ATIP_CONFIG").and_then(|path| std::fs::read_to_string(path).ok()).and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut tool = Tool { config, json: false };
    tool.json = json_mode(&tool, &args);
    if let Some(seconds) = tool.config.get("hangOnVersion").and_then(Value::as_u64) {
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
    let before_dashdash = args.iter().take_while(|arg| arg.as_str() != "--");
    let args_before: Vec<&String> = before_dashdash.collect();
    let json = args_before.iter().any(|arg| arg.as_str() == "--json");
    // The seeded violation: an unknown option that comes after `--json` is reported as plain text.
    let json = json && !(tool.flag("unknownOptionAfterJsonIsText") && args_before.iter().skip_while(|arg| arg.as_str() != "--json").any(|arg| is_unknown_option(arg)));
    if tool.flag("outputFormatSwitchesJson") && std::env::var_os("OUTPUT_FORMAT").is_some() {
        return false;
    }
    if tool.flag("ttySwitchesJson") && std::io::stdout().is_terminal() {
        return false;
    }
    json
}

fn is_unknown_option(arg: &str) -> bool {
    arg.starts_with("--") && arg != "--" && arg != "--json" && arg != "--version"
}

fn run(tool: &Tool, args: &[String]) -> ExitCode {
    let mut words = Vec::new();
    let mut rest = args.iter();
    for arg in rest.by_ref() {
        match arg.as_str() {
            "--" => break,
            "--json" => {}
            "--version" if words.is_empty() => {
                return version(tool);
            }
            option if is_unknown_option(option) => return tool.usage_for_option(option, words.first().map(String::as_str).filter(|word| COMMANDS.contains(word))),
            word => words.push(word.to_string()),
        }
    }
    words.extend(rest.cloned());
    let Some(command) = words.first() else {
        let code = tool.text("bareCode", "COMMAND_REQUIRED");
        return tool.refuse(&code, "a command is required", None, &[("expected", json!(COMMANDS))], tool.exit_of(&code));
    };
    if !COMMANDS.contains(&command.as_str()) {
        return tool.refuse("UNKNOWN_COMMAND", &format!("{command} is not a command"), None, &[("expected", json!(COMMANDS)), ("received", json!(command)), ("field", json!("command"))], 2);
    }
    if let Some(extra) = words.get(1) {
        return tool.refuse("USAGE", &format!("{command} takes no operand, got {extra}"), Some(command), &[("field", json!("ARGUMENT"))], 2);
    }
    match command.as_str() {
        "init" => {
            std::fs::write("state.txt", "initialised").unwrap_or_default();
            if let Some(log) = tool.config.get("initLog").and_then(Value::as_str) {
                let mut file = std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap_or_else(|_| std::process::exit(70));
                std::io::Write::write_all(&mut file, b"init\n").unwrap_or_default();
            }
            tool.success("init", json!({"initialised": true}))
        }
        "touch" => {
            std::fs::write("touched.txt", "x").unwrap_or_default();
            tool.success("touch", json!({"touched": true}))
        }
        _ => tool.success("status", json!({"items": 0, "state": "ok"})),
    }
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
    ExitCode::SUCCESS
}
