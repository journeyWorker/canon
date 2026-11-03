//! A fake tool whose behaviour is a JSON configuration: the test subject of `atip check`'s own tests.
//!
//! The configuration file is named by `FAKE_ATIP_CONFIG` (the fixture's `env` passes it). With no configuration
//! the tool follows atip/1 1.1 L0 to the letter (`"claims": "1.0"` makes it a 1.0 tool). Each member below breaks
//! the one rule its name says; `tests/support/mod.rs` maps every condition of every L0 check to the configuration
//! that breaks it.
//!
//! Commands: `init` (writes `state.txt`; `initLog` appends a line to a file), `status` (the probe: prints a small
//! report with a `next` entry, a warning and a page; `--cursor c1` is the second page), `items` (the same without
//! the probe's role), `touch` (needs `--name` or `--input '{"name": ...}'`; an `--input` that is no JSON is refused
//! with a reason), `reject` (a `USAGE` that names no argument), `team` (a group with the command `add`, which
//! writes `team.txt`), `member` (a group with the L1 commands `list`, `search` and `tag`, see `l1/`), and `schema`
//! (the tree, groups and command documents, `--errors`, `--error`, `--warnings`, `--warning`). Everything else is
//! unknown.
//!
//! The L1 part (`l1/`) is one model of the commands from which the printed documents, `--help` and the enforcement
//! are derived. The members of the configuration that seed an L1 violation are described where they act; the
//! generic one is `docPatch`: a map from a document key (`<tree>`, a command or group id, `--errors`,
//! `--error <CODE>`, `--warnings`, `--warning <CODE>`) to JSON-patch operations applied to the printed `data`.

use std::io::{BufRead, IsTerminal, Read, Write};
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{json, Map, Value};

mod l1;

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

    /// The minor the tool claims (`"claims"`: `1.0`, `1.1` the default, or `1.2`). A copy of the tool whose name has `claims12` in it claims
    /// 1.2 by default, so that the runs that get no configuration (`L1.no-state` runs in an empty directory) say what the configured ones say.
    fn minor(&self) -> String {
        let alias = std::env::args().next().unwrap_or_default();
        let default = if alias.contains("claims12") { "1.2" } else { "1.1" };
        self.text("claims", default)
    }

    /// Whether the tool claims 1.1 or later.
    fn is_1_1(&self) -> bool {
        self.minor() != "1.0"
    }

    /// Whether the tool claims 1.2.
    fn is_1_2(&self) -> bool {
        self.minor() == "1.2"
    }

    fn commands(&self) -> Vec<&'static str> {
        let mut commands = vec!["init", "items", "member", "reject", "status", "team", "touch"];
        if self.flag("l2") {
            commands.retain(|command| *command != "touch");
            commands.extend(["build", "deploy", "log", "peek", "purge", "record", "seal", "skills", "stamp"]);
        }
        if !self.flag("noSchema") {
            commands.push("schema");
        }
        if self.config.contains_key("serveMode") {
            commands.push("serve");
        }
        commands.sort_unstable();
        commands
    }

    /// The code the tool answers for `code`: with `usageRenamed` the tool has no `USAGE` and answers `usageReplacement` (default `OPTION_INVALID`).
    fn usage_name(&self) -> String {
        self.text("usageReplacement", "OPTION_INVALID")
    }

    fn renamed<'a>(&self, code: &'a str) -> std::borrow::Cow<'a, str> {
        if code == "USAGE" && self.flag("usageRenamed") {
            std::borrow::Cow::Owned(self.usage_name())
        } else {
            std::borrow::Cow::Borrowed(code)
        }
    }

    fn registered_exit(code: &str) -> u8 {
        REGISTRY
            .iter()
            .find(|(name, _, _)| *name == code)
            .map_or(2, |(_, exit, _)| u8::try_from(*exit).unwrap_or(2))
    }

    /// The seeded refusal of the command `id` (`refuseWith`: `{code, commands, unlessEnv?}`): the command is refused with `code` when its
    /// environment does not hold the variable `unlessEnv` (always, when the knob names none). A refusal of state or environment, not of the arguments.
    fn seeded_refusal(&self, id: &str) -> Option<ExitCode> {
        let knob = self.config.get("refuseWith")?;
        let listed = knob.get("commands").and_then(Value::as_array)?.iter().any(|command| command == id);
        let given = knob.get("unlessEnv").and_then(Value::as_str).is_some_and(|name| std::env::var_os(name).is_some());
        if !listed || given {
            return None;
        }
        let code = knob.get("code").and_then(Value::as_str).unwrap_or("NEEDS_TOKEN");
        Some(self.refuse(code, "the backend needs a credential", id, &[]))
    }

    fn meta(&self, command: Option<&str>) -> Map<String, Value> {
        let mut meta = Map::new();
        let schema_answer = command == Some("schema");
        let version = if schema_answer {
            self.text(
                "schemaAnswerVersion",
                &if self.is_1_1() { self.minor() } else { "1.0".to_string() },
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
                json!(self.text("contractVersionSays", &self.minor())),
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
        let usage = code == "USAGE";
        let code = &*self.renamed(code);
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
            // Seeded (`usageNoField`): a refusal that is a usage problem does not say which argument.
            if *key == "field" && usage && self.flag("usageNoField") {
                continue;
            }
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
        // Seeded: INPUT_INVALID exits 1 where the registry says 2 (the code and the envelope are right).
        let exit = if code == "INPUT_INVALID" && self.flag("inputInvalidExit1") { 1 } else { Self::registered_exit(code) };
        self.fail("usage", code, message, Some(command), extras, exit)
    }

    fn success(&self, command: &str, data: Value) -> ExitCode {
        self.success_with(command, data, Map::new())
    }

    /// A success whose `meta` also carries `extra` (the page, the next steps, the fields, the output), with the
    /// seeded patches of `dataPatch` and `metaPatch` for the command applied last.
    fn success_with(&self, command: &str, data: Value, extra: Map<String, Value>) -> ExitCode {
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
            meta.extend(extra);
            if command == "status" || command == "items" {
                self.shape_status(command, &mut data, &mut meta, ci);
            }
            l1::seed_answer(self, command, &mut data, &mut meta);
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
            // The L1 tool pages in `member list` only: its probe has no `--cursor` (and no `--limit`, which a page
            // needs to be bound to anything), so its answers carry no page and no next-page entry.
            let pages = !self.flag("l1Registry");
            if pages && !self.flag("pageNoCursorEntry") {
                next.push(json!({"command": format!("{tool} {command} --json --cursor c1"), "argv": [command, "--json", "--cursor", "c1"], "reason": "the next page"}));
            }
            if self.flag("nextBogusCommand") {
                next.push(json!({"command": format!("{tool} nosuch-xyz --json"), "argv": ["nosuch-xyz", "--json"]}));
            }
            if self.flag("nextUsage") {
                next.push(json!({"command": format!("{tool} member list --limit=0 --json"), "argv": ["member", "list", "--limit=0", "--json"]}));
            }
            meta.insert("next".into(), Value::Array(next));
            let page = if self.flag("pageBadShape") {
                json!({"returned": -1, "nextCursor": "c1"})
            } else {
                json!({"returned": 1, "nextCursor": "c1"})
            };
            if pages {
                meta.insert("page".into(), page);
            }
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
        // `data.kind` names the document from 1.0 on (spec 10.1.4); only the warning kinds are since 1.1.
        let with_kind = |kind: &str, mut data: Value| {
            data["kind"] = json!(kind);
            data
        };
        if has("--errors") {
            let rows = self.error_rows();
            let kind = self.text("errorsKind", "errors");
            let mut data = json!({"errors": rows});
            data["kind"] = json!(kind);
            return self.success("schema", l1::patched(self, "--errors", data));
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
                return self.success("schema", l1::patched(self, &format!("--error {code}"), with_kind("error", json!({"error": row}))));
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
            let unknown_code = self.text("schemaErrorUnknownCode", "USAGE");
            let unknown_field = self.text("schemaErrorUnknownField", "--error");
            return self.refuse(
                &unknown_code,
                "not an error code",
                "schema",
                &[("field", json!(unknown_field)), ("received", json!(code))],
            );
        }
        if is_1_1 && has("--warnings") {
            let rows = self.warning_rows();
            let kind = self.text("warningsKind", "warnings");
            return self.success("schema", l1::patched(self, "--warnings", json!({"kind": kind, "warnings": rows})));
        }
        if is_1_1 && has("--warning") {
            let code = value("--warning").unwrap_or_default();
            let mut rows = self.warning_rows();
            if self.flag("warningAnswerDiffers") {
                rows[0]["meaning"] = json!("another meaning");
            }
            if let Some(row) = rows.into_iter().find(|row| row["code"] == code.as_str()) {
                return self.success("schema", l1::patched(self, &format!("--warning {code}"), with_kind("warning", json!({"warning": row}))));
            }
            return self.refuse(
                "USAGE",
                "not a warning code",
                "schema",
                &[("field", json!(self.text("schemaWarningUnknownField", "--warning"))), ("received", json!(code))],
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

    /// The registry rows `schema --errors --json` prints: the L0 ones, with the L1 ones added when the tool is
    /// configured with `l1Registry`, one row's exit changed when the registry is seeded to drift (`schemaErrorsDrift`,
    /// `reservedUnknownCommandExit`, `reservedExit`: a map from a code to its exit).
    fn error_rows(&self) -> Vec<Value> {
        let tool = self.name();
        let mut table: Vec<(String, i64, &[&str], String, String)> = REGISTRY
            .iter()
            .map(|(code, exit, reasons)| ((*code).to_string(), *exit, *reasons, "m".to_string(), "f".to_string()))
            .collect();
        if self.flag("l1Registry") {
            table.extend(l1::EXTRA_ERRORS.iter().map(|(code, exit, meaning, fix)| {
                ((*code).to_string(), *exit, &[][..], (*meaning).to_string(), fix.replace("{tool}", &tool))
            }));
        }
        // `stoppedMidway` registers the code `STOPPED_MIDWAY` (spec 8.3.2): exit 1 and not retryable (`reservedExit` and
        // `stoppedMidwayRetryable` seed it wrong; below 1.2 it is a code of the tool's own).
        if self.flag("stoppedMidway") {
            let exit = if self.flag("stoppedMidwayExit2") { 2 } else { 1 };
            table.push(("STOPPED_MIDWAY".to_string(), exit, &[][..], "m".to_string(), "f".to_string()));
        }
        if self.flag("l2") {
            const INPUT_INVALID: &[&str] = &["BATCH_UNSUPPORTED", "EMPTY", "FLAG_ONLY_KEY", "INVALID_VALUE", "NOT_JSON", "NOT_OBJECT", "STDIN_TTY", "UNKNOWN_KEY", "UNREADABLE"];
            const TOO_LARGE: &[&str] = &["TOO_MANY_BYTES", "TOO_MANY_ITEMS"];
            for row in &mut table {
                match row.0.as_str() {
                    "INPUT_INVALID" => row.2 = INPUT_INVALID,
                    "INPUT_TOO_LARGE" => row.2 = TOO_LARGE,
                    _ => {}
                }
            }
        }
        for row in self.config.get("extraCodes").and_then(Value::as_array).into_iter().flatten() {
            table.push((
                row["code"].as_str().unwrap_or_default().to_string(),
                row["exit"].as_i64().unwrap_or(2),
                &[][..],
                "m".to_string(),
                "f".to_string(),
            ));
        }
        if self.flag("usageRenamed") {
            let replacement = self.usage_name();
            for row in table.iter_mut().filter(|row| row.0 == "USAGE") {
                row.0 = replacement.clone();
            }
        }
        table.sort_by(|left, right| left.0.cmp(&right.0));
        table
            .into_iter()
            .map(|(code, exit, reasons, meaning, fix)| {
                let exit = if self.flag("schemaErrorsDrift") && code == "STATE_ODD" {
                    2
                } else if code == "UNKNOWN_COMMAND" {
                    self.number("reservedUnknownCommandExit").map_or(exit, |exit| i64::try_from(exit).unwrap_or(2))
                } else {
                    self.config
                        .get("reservedExit")
                        .and_then(|rows| rows.get(&code))
                        .and_then(Value::as_i64)
                        .unwrap_or(exit)
                };
                let retryable = (code == "STOPPED_MIDWAY" && self.flag("stoppedMidwayRetryable"))
                    || self.config.get("retryableCodes").and_then(Value::as_array).is_some_and(|codes| codes.iter().any(|retryable| retryable == &code));
                let mut row = json!({"code": code, "exit": exit, "retryable": retryable, "meaning": meaning, "fix": fix});
                if self.is_1_1() && !reasons.is_empty() {
                    row["reasons"] = json!(reasons
                        .iter()
                        .map(|reason| json!({"reason": reason, "meaning": "m", "fix": "f"}))
                        .collect::<Vec<_>>());
                }
                row
            })
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
        if self.flag("dualWarning") {
            rows.push(json!({"code": "STATE_ODD", "meaning": "another meaning", "fix": "f"}));
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
    let mut config: Map<String, Value> = std::env::var_os("FAKE_ATIP_CONFIG")
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    // An executable named `...claims12...` is a tool that claims 1.2 when no configuration says otherwise (the run in an empty directory has no environment).
    if config.is_empty() {
        // The name of the executable alone: the directory it is in says nothing.
        let name = std::env::args().next().and_then(|path| std::path::Path::new(&path).file_name().map(|file| file.to_string_lossy().into_owned())).unwrap_or_default();
        if name.contains("claims12") {
            config.insert("claims".to_string(), json!("1.2"));
        }
        // An L2 tool answers `skills show` in an empty directory with a minimal environment, which has no configuration to read.
        if name.contains("-l2") {
            config.insert("l2".to_string(), json!(true));
            config.insert("l1Registry".to_string(), json!(true));
            // The seeded violations of what that run answers are named in the executable's name, after `-l2`: `...-l2-skillswrites`.
            const KNOBS: [&str; 11] = [
                "skillsPutsFile", "skillsNeedsState", "skillsBadSchema", "skillsDefaultNotListed", "skillsGuideBadSchema", "skillsWrongName", "skillsGuidesDiffer", "skillsUnknownOk",
                "skillsUnknownNoExpected", "skillsMissingName", "guideBadOption",
            ];
            let file = name.to_lowercase();
            for knob in KNOBS {
                if file.split('-').any(|token| token == knob.to_lowercase()) {
                    config.insert(knob.to_string(), json!(true));
                }
            }
        }
    }
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
    if let Some(log) = tool.config.get("invocationLog").and_then(Value::as_str) {
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
            let _ = std::io::Write::write_all(&mut file, format!("{}\n", args.join(" ")).as_bytes());
        }
    }
    if let Some(command) = tool.config.get("hangCommand").and_then(Value::as_str) {
        let words: Vec<&str> = args.iter().take_while(|arg| arg.as_str() != "--").filter(|arg| !arg.starts_with('-')).map(String::as_str).collect();
        let asks_help = args.iter().any(|arg| arg == "--help" || arg == "-h");
        if !asks_help && words.join(" ").starts_with(command) {
            std::thread::sleep(Duration::from_secs(600));
        }
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
        "--help",
        "--limit",
        "--all",
        "--fields",
        "--team",
        "--sort",
        "--tag",
        "--filter",
        "--min-score",
        "--age",
        "--step",
        "--since",
        "--exact",
        "--prefix",
        "--quiet",
        "--verbose",
        "--total",
        "--note",
        "--clear-note",
        "--round",
        "--label",
        "--weight",
        "--score",
        "--meta",
    ];
    arg.starts_with("--") && arg != "--" && !known.contains(&arg)
}

/// The command `serve` of the seeded `notRun` runs (`serveMode`): it prints one envelope and then serves until it is signalled.
/// `exitBefore` ends without a document, `exitAfter` ends after it, `silent` prints none and keeps running, `ignoreTerm` ignores SIGTERM;
/// `dataPatch` and `metaPatch` of `serve` seed a bad document, and `invocationLog` counts the runs.
#[expect(
    unsafe_code,
    reason = "libc::signal is an unsafe FFI call; the seeded violation is a server that ignores SIGTERM"
)]
fn serve(tool: &Tool) -> ExitCode {
    let mode = tool.text("serveMode", "serve");
    if mode == "ignoreTerm" {
        // SAFETY: sets the disposition of SIGTERM of this process to ignore; nothing else touches signals.
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
        }
    }
    match mode.as_str() {
        "exitBefore" => return ExitCode::SUCCESS,
        // Refused at the start with exit 2 and the code `serveRefuseCode` (a precondition when it is `CURSOR_INVALID`, a usage error as `USAGE`).
        "refuseStart" => {
            let code = tool.text("serveRefuseCode", "CURSOR_INVALID");
            let field = tool.text("serveRefuseField", "");
            let extras: Vec<(&str, Value)> = if field.is_empty() { Vec::new() } else { vec![("field", json!(field))] };
            // `serveRefuseExit` seeds the exit of the refusal (default 2): with 1 it is not a refusal of the kind the contract knows.
            let exit = tool.number("serveRefuseExit").map_or(2, |exit| u8::try_from(exit).unwrap_or(2));
            return tool.fail("usage", &code, "serve cannot start here", Some("serve"), &extras, exit);
        }
        "silent" => {}
        _ => {
            tool.success("serve", json!({"serving": true}));
        }
    }
    if mode == "exitAfter" {
        return ExitCode::SUCCESS;
    }
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
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
    // An L2 tool reads `--input` first (spec 11.7): a refusal, a batch, or the argv that says what the document and argv say.
    let synthesized: Vec<String>;
    let args: &[String] = if tool.flag("l2") {
        match l1::prepare_input(tool, args) {
            l1::Prepared::Unchanged => args,
            l1::Prepared::Done(code) => return code,
            l1::Prepared::Argv(words) => {
                // What a handler that answers differently for a document needs to know: the arguments came from `--input`.
                std::env::set_var("FAKE_FROM_DOCUMENT", "1");
                synthesized = words;
                &synthesized
            }
            l1::Prepared::Batch(id, items) => return l1::execute_batch(tool, id, items),
        }
    } else {
        args
    };
    if let Some(code) = l1::handle(tool, args) {
        return code;
    }
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
                    .find(|(name, _)| *name == option && !(*name == "--cursor" && (tool.flag("noPaging") || tool.flag("l1Registry"))));
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
    if let Some(refused) = tool.seeded_refusal(command) {
        return refused;
    }
    if command == "serve" {
        return serve(tool);
    }
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
            if tool.flag("l2") {
                // The logs `purge` removes.
                std::fs::create_dir_all("logs").unwrap_or_default();
                for name in ["run-a", "session-a", "session-b"] {
                    std::fs::write(format!("logs/{name}.log"), format!("{name}\n")).unwrap_or_default();
                }
            }
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
            &[("expected", json!(team_verbs(tool)))],
            2,
        ),
        Some("roster") if tool.flag("deepTree") => match words.get(2).map(String::as_str) {
            None => tool.fail(
                "group",
                "COMMAND_REQUIRED",
                "team roster needs a command",
                Some("team roster"),
                &[("expected", json!(["list"]))],
                2,
            ),
            Some("list") => tool.success("team roster list", json!({"roster": []})),
            Some(word) => tool.unknown_command(word, &["list"], Some("team roster")),
        },
        Some("add") if tool.flag("teamAddNoEnvelope") && tool.json => {
            println!("{}", json!({"ok": true, "data": {}}));
            ExitCode::SUCCESS
        }
        // A stream command (13.5): a start line, an event, an end line holding the envelope.
        Some("add") if tool.flag("teamAddStream") && tool.json => {
            let version = tool.text("streamStartContractVersion", &tool.minor());
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
        Some("add") => {
            std::fs::write("team.txt", "added").unwrap_or_default();
            tool.success("team add", json!({"added": true}))
        }
        Some(word) => tool.unknown_command(word, &team_verbs(tool), Some("team")),
    }
}

/// The words of the group `team`.
fn team_verbs(tool: &Tool) -> Vec<&'static str> {
    let mut verbs = vec!["add"];
    if tool.flag("deepTree") {
        verbs.push("roster");
    }
    if tool.flag("groupExpectedWrong") {
        verbs.push("nothing");
    }
    verbs
}

/// `touch --name <NAME>` or `touch --input '{"name": <NAME>}'`, or a batch: `--input '[{"name": ...}, ...]'` (spec
/// 11.11; at most `MAX_BATCH_ITEMS` documents, each merged with the argv arguments). An `--input` that is no JSON is
/// refused with a reason, a document with another key with `UNKNOWN_KEY`, a larger one with `INPUT_TOO_LARGE`, and a
/// name that differs from `--name` with `INPUT_CONFLICT`; a batch is refused whole, before its first item runs.
fn touch(tool: &Tool, name: &Option<String>, input: &Option<String>) -> ExitCode {
    let mut name = name.clone();
    if let Some(text) = input {
        let refuse_input = |code: &str, reason: Option<&str>, message: &str, field: &str| {
            let mut extras: Vec<(&str, Value)> = vec![("field", json!(field))];
            let situation = reason;
            let reason = if tool.flag("unregisteredReason") && code == "INPUT_INVALID" {
                Some("NOT_A_REASON")
            } else {
                reason.filter(|_| tool.is_1_1() || tool.flag("reasonIn10"))
            };
            if let Some(reason) = reason {
                extras.push(("reason", json!(reason)));
            }
            if tool.flag("proseHintStartsWithTool") && situation == Some("NOT_JSON") {
                extras.push(("hint", json!(format!("{} can't read this document; quote it in single quotes.", tool.name()))));
            }
            tool.refuse(code, message, "touch", &extras)
        };
        if text.len() > l1::MAX_INPUT_BYTES {
            return refuse_input("INPUT_TOO_LARGE", None, "--input is larger than the command accepts", "--input");
        }
        let Ok(parsed) = serde_json::from_str::<Value>(text) else {
            return refuse_input("INPUT_INVALID", Some("NOT_JSON"), "--input is not JSON", "--input");
        };
        let batch = parsed.is_array();
        let documents = match parsed {
            Value::Array(items) => items,
            other => vec![other],
        };
        if batch && documents.is_empty() {
            return refuse_input("INPUT_INVALID", None, "a batch holds at least one document", "--input");
        }
        if documents.len() > l1::MAX_BATCH_ITEMS {
            return refuse_input("INPUT_TOO_LARGE", None, "the batch holds more documents than the command accepts", "--input");
        }
        let mut names: Vec<Option<String>> = Vec::new();
        for (index, item) in documents.iter().enumerate() {
            let prefix = if batch { format!("/{index}") } else { String::new() };
            let Some(document) = item.as_object() else {
                let field = if batch { prefix.clone() } else { "--input".to_string() };
                return refuse_input("INPUT_INVALID", Some("NOT_JSON"), "--input is not a JSON object", &field);
            };
            if let Some(key) = document.keys().find(|key| key.as_str() != "name") {
                return refuse_input("INPUT_INVALID", Some("UNKNOWN_KEY"), "the document has a key the command does not take", &format!("{prefix}/{key}"));
            }
            let mut item_name = name.clone();
            match document.get("name") {
                Some(Value::String(from_document)) => {
                    if name.as_ref().is_some_and(|from_argv| from_argv != from_document) {
                        return refuse_input("INPUT_CONFLICT", None, "--name and the document differ", &format!("{prefix}/name"));
                    }
                    item_name = Some(from_document.clone());
                }
                Some(_) => return refuse_input("INPUT_INVALID", None, "name is not a string", &format!("{prefix}/name")),
                None => {}
            }
            names.push(item_name);
        }
        if batch {
            if let Some(index) = names.iter().position(Option::is_none) {
                return tool.refuse("USAGE", "--name is required", "touch", &[("field", json!(format!("/{index}/name")))]);
            }
            let mut items = Vec::new();
            for index in 0..names.len() {
                let status = if std::fs::read_to_string("touched.txt").is_ok_and(|text| text == "x") { "unchanged" } else { "written" };
                std::fs::write("touched.txt", "x").unwrap_or_default();
                items.push(json!({"index": index, "status": status, "data": {"touched": true}}));
            }
            let mut meta = Map::new();
            meta.insert("output".into(), json!("TouchBatchReport"));
            return tool.success_with("touch", json!({"items": items}), meta);
        }
        name = names.pop().flatten();
    }
    if name.is_none() {
        return tool.name_missing();
    }
    std::fs::write("touched.txt", "x").unwrap_or_default();
    let mut meta = Map::new();
    meta.insert("output".into(), json!("TouchReport"));
    tool.success_with("touch", json!({"touched": true}), meta)
}

fn version(tool: &Tool) -> ExitCode {
    if tool.flag("versionWrites") {
        std::fs::write("version-cache.txt", "written by --version").unwrap_or_default();
    }
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
