//! The commands the L2 safety and effects rows exercise (approaches/checker.md 6): `purge` (a destructive command with a
//! preview and a commit), `deploy start` and `deploy status` (asynchronous work with a controllable worker and operation
//! keys), `seal`, `stamp` and `log` (the idempotency values `once`, `content-keyed` and `appends`) and `peek` (a read command
//! that appends one line to a log, which its `effectNote` declares).
//!
//! Their state is files in the working directory, so the checker sees every effect in its snapshots. Every violation is
//! a knob of the configuration; each names the rule of the contract it breaks where it acts.

use std::io::Write;
use std::process::ExitCode;

use serde_json::{json, Map, Value};

use super::doc::Model;
use super::parse::{parse, Parsed};
use super::run::refuse_failure;
use super::spec::{Arg, Command, Rule};
use crate::Tool;

/// The commands, groups aside, in the order of the tree.
pub fn commands(pair_code: bool) -> Vec<Command> {
    vec![purge(pair_code), deploy_start(), deploy_status(), seal(), stamp(), log(), peek()]
}

fn purge(pair_code: bool) -> Command {
    let mut command = Command::new("purge", "Remove the log files of one kind: the bare run previews the removal set and its planDigest, `--commit --plan-digest <DIGEST>` deletes it")
        .writes("replay-safe")
        .options(vec![
            Arg::option("kind", "enum", "The kind of log to purge").value("KIND").required().possible(&["run", "session"]),
            Arg::option("before", "string", "Purge the logs before this date (YYYY-MM-DD)").value("DATE").required().domain(json!({"pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}$"})),
            Arg::option("commit", "switch", "Carry out the previewed plan").flag_only(),
            Arg::option("plan-digest", "string", "The digest of the plan to carry out").value("DIGEST").flag_only(),
        ])
        .rule(with_code(Rule::requires("commit", &[], &["plan-digest"]), pair_code))
        .rule(with_code(Rule::requires("plan-digest", &[], &["commit"]), pair_code))
        .errors(&["PAIR_REQUIRED", "PLAN_STALE", "PURGE_FAILED", "USAGE"])
        .output(
            "PurgeReport",
            json!({
                "type": "object",
                "required": ["kind", "before", "committed"],
                "properties": {
                    "kind": {"type": "string"},
                    "before": {"type": "string"},
                    "planDigest": {"type": "string"},
                    "effects": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["action", "path"],
                            "properties": {"action": {"enum": ["delete", "append"]}, "path": {"type": "string"}, "detail": {"type": "string"}},
                            "additionalProperties": false
                        }
                    },
                    "committed": {"type": "boolean"}
                },
                "additionalProperties": false
            }),
        )
        .examples(&["{tool} purge --kind session --before 2030-01-01 --json"]);
    command.effect = "destructive";
    command.effect_note = Some("a committed run appends one line to .purge-audit.log when it deleted something; a preview writes nothing");
    command
}

/// The relation of `--commit` and `--plan-digest` states its own code when the tool registers one (spec 10.8.1).
fn with_code(rule: Rule, pair_code: bool) -> Rule {
    if pair_code {
        rule.code("PAIR_REQUIRED")
    } else {
        rule
    }
}

fn deploy_start() -> Command {
    let mut command = Command::new("deploy start", "Start deploying to an environment; the deployment runs on a worker and is followed with deploy status")
        .writes("replay-safe")
        .operand(Arg::operand("environment", "string", "The environment to deploy to").value("ENVIRONMENT").required().domain(json!({"pattern": "^[a-z][a-z0-9-]*$"})))
        .options(vec![Arg::option("operation", "string", "A key naming this request: a repeat with it starts nothing").value("KEY").domain(json!({"pattern": "^[a-z0-9][a-z0-9-]{0,39}$"}))])
        .errors(&["DEPLOY_FAILED", "OPERATION_REUSED", "USAGE"])
        .output(
            "DeployStartReport",
            json!({
                "type": "object",
                "required": ["deployment", "environment", "state"],
                "properties": {"deployment": {"type": "string"}, "environment": {"type": "string"}, "state": {"type": "string"}},
                "additionalProperties": false
            }),
        )
        .examples(&["{tool} deploy start staging --json"]);
    command.is_async = true;
    command.effect_note = Some("records one deployment, then queues it on the deploy worker; a repeat with the same --operation and arguments returns the deployment it started");
    command
}

fn deploy_status() -> Command {
    Command::new("deploy status", "Show a deployment's state, or wait until it ends")
        .operand(Arg::operand("deployment", "string", "The deployment, as deploy start reported it").value("DEPLOYMENT").required())
        .options(vec![
            Arg::option("wait", "switch", "Wait until the deployment ends"),
            Arg::option("timeout", "string", "How long --wait waits (90s, 10m)").value("DURATION").default(json!("10m")).domain(json!({"maxLength": 6})),
        ])
        .errors(&["DEPLOY_FAILED", "DEPLOYMENT_NOT_FOUND", "USAGE", "WAIT_TIMEOUT"])
        .output(
            "DeployStatusReport",
            json!({
                "type": "object",
                "required": ["deployment", "state"],
                "properties": {"deployment": {"type": "string"}, "state": {"type": "string"}},
                "additionalProperties": false
            }),
        )
        .examples(&["{tool} deploy status d-1 --wait --json"])
}

fn seal() -> Command {
    Command::new("seal", "Seal the directory: it can be sealed once")
        .writes("once")
        .options(Vec::new())
        .errors(&["ALREADY_SEALED", "USAGE"])
        .output(
            "SealReport",
            json!({"type": "object", "required": ["sealed"], "properties": {"sealed": {"type": "boolean"}}, "additionalProperties": false}),
        )
        .examples(&["{tool} seal --json"])
}

fn stamp() -> Command {
    Command::new("stamp", "Keep a stamp for a text: the same text is kept once")
        .writes("content-keyed")
        .operand(Arg::operand("text", "string", "The text to keep").value("TEXT").required().domain(json!({"maxLength": 40})))
        .options(vec![Arg::option("dry-run", "switch", "Print what would be kept and keep nothing")])
        .errors(&["USAGE"])
        .output(
            "StampReport",
            json!({
                "type": "object",
                "required": ["stamp", "status"],
                "properties": {"stamp": {"type": "string"}, "status": {"enum": ["written", "unchanged", "planned"]}},
                "additionalProperties": false
            }),
        )
        .examples(&["{tool} stamp hello --json"])
}

fn log() -> Command {
    Command::new("log", "Append a line to the log")
        .writes("appends")
        .operand(Arg::operand("text", "string", "The line").value("TEXT").required().domain(json!({"maxLength": 40})))
        .options(Vec::new())
        .errors(&["USAGE"])
        .output(
            "LogReport",
            json!({"type": "object", "required": ["lines"], "properties": {"lines": {"type": "integer", "minimum": 0}}, "additionalProperties": false}),
        )
        .examples(&["{tool} log hello --json"])
}

fn peek() -> Command {
    let mut command = Command::new("peek", "Read the log, and note the read in the access log")
        .options(Vec::new())
        .errors(&["USAGE"])
        .output(
            "PeekReport",
            json!({"type": "object", "required": ["lines"], "properties": {"lines": {"type": "integer", "minimum": 0}}, "additionalProperties": false}),
        )
        .examples(&["{tool} peek --json"]);
    command.effect_note = Some("appends one line to .peek-access.log");
    command
}

/// The group `deploy`'s summary.
pub const DEPLOY_SUMMARY: &str = "Deploy and follow deployments";

/// A hash of `text` as 64 hexadecimal digits: stable, and a function of the text alone.
fn digest(text: &str) -> String {
    let mut out = String::from("sha256:");
    for round in 0..4u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325 ^ (round.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        out.push_str(&format!("{hash:016x}"));
    }
    out
}

/// Prints a failure with its own `meta.next` and retryability (a refusal of the plan, a wait that timed out).
fn fail_with(tool: &Tool, code: &str, message: &str, command: &str, extras: &[(&str, Value)], how: (u8, bool, Option<Value>)) -> ExitCode {
    let (exit, retryable, next) = how;
    let mut error = Map::new();
    error.insert("code".into(), json!(code));
    error.insert("message".into(), json!(message));
    error.insert("retryable".into(), json!(retryable));
    for (key, value) in extras {
        error.insert((*key).to_string(), value.clone());
    }
    let mut meta = tool.meta(Some(command));
    if let Some(next) = next {
        meta.insert("next".into(), json!([next]));
    }
    let document = json!({"ok": false, "error": error, "meta": meta});
    if tool.json {
        tool.print_document(&document);
    } else {
        eprintln!("{}: {code}: {message}", tool.name());
    }
    ExitCode::from(exit)
}

fn next_entry(tool: &Tool, argv: &[String], reason: &str) -> Value {
    json!({"command": format!("{} {}", tool.name(), argv.join(" ")), "argv": argv, "reason": reason})
}

/// Answers the commands of this module; `None` for any other.
pub fn handle(tool: &Tool, args: &[String], words: &[String]) -> Option<ExitCode> {
    match words.first().map(String::as_str)? {
        "purge" => Some(run(tool, "purge", args, purge_run)),
        "seal" => Some(run(tool, "seal", args, seal_run)),
        "stamp" => Some(run(tool, "stamp", args, stamp_run)),
        "log" => Some(run(tool, "log", args, log_run)),
        "peek" => Some(run(tool, "peek", args, peek_run)),
        "deploy" => Some(deploy(tool, args, words)),
        _ => None,
    }
}

fn run(tool: &Tool, id: &'static str, args: &[String], handler: fn(&Tool, &[String], &Parsed) -> ExitCode) -> ExitCode {
    let model = Model::of(tool);
    let Some(command) = model.command(id) else {
        return tool.unknown_command(id, &[], None);
    };
    let skip_rules = id == "purge" && tool.flag("halfAccepted");
    match parse(tool, command, args, skip_rules) {
        Err(failure) => refuse_failure(tool, id, failure),
        Ok(parsed) => {
            if let Some(refused) = tool.seeded_refusal(id) {
                return refused;
            }
            handler(tool, args, &parsed)
        }
    }
}

// ---- purge ----

/// The files a purge of `kind` would delete: `logs/<kind>-*.log`, with their sizes.
fn purge_set(kind: &str) -> Vec<(String, u64)> {
    let mut files: Vec<(String, u64)> = std::fs::read_dir("logs")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with(&format!("{kind}-")).then(|| (format!("logs/{name}"), entry.metadata().map_or(0, |meta| meta.len())))
        })
        .collect();
    files.sort();
    files
}

fn plan_digest(kind: &str, before: &str, files: &[(String, u64)]) -> String {
    let listing: Vec<String> = files.iter().map(|(name, size)| format!("{name}:{size}")).collect();
    digest(&format!("{kind}|{before}|{}", listing.join(",")))
}

fn purge_run(tool: &Tool, _args: &[String], parsed: &Parsed) -> ExitCode {
    let kind = parsed.text("kind").unwrap_or_default().to_string();
    let before = parsed.text("before").unwrap_or_default().to_string();
    let files = purge_set(&kind);
    let digest = plan_digest(&kind, &before, &files);
    let effects: Vec<Value> = files.iter().map(|(path, size)| json!({"action": "delete", "path": path, "detail": format!("{size} bytes")})).collect();
    let preview_argv: Vec<String> = ["purge", "--kind", &kind, "--before", &before, "--json"].iter().map(ToString::to_string).collect();
    let mut commit_argv: Vec<String> = ["purge"].iter().map(ToString::to_string).collect();
    if tool.flag("previewNextInput") && std::env::var_os("FAKE_INPUT_FROM_STDIN").is_some() {
        commit_argv.extend(["--input", "-"].map(String::from));
    } else {
        commit_argv.extend(["--kind", &kind, "--before", &before].map(String::from));
    }
    if tool.flag("digestOneWord") {
        // The form of spec 9.12.1: every valued option as `--name=value`.
        commit_argv.extend(["--commit".to_string(), format!("--plan-digest={digest}"), "--json".to_string()]);
    } else {
        commit_argv.extend(["--commit", "--plan-digest", &digest, "--json"].map(String::from));
    }
    let data = |committed: bool| json!({"kind": kind, "before": before, "planDigest": digest, "effects": effects, "committed": committed});
    if !parsed.has("commit") {
        if tool.flag("previewFails") && std::env::var_os("FAKE_INPUT_FROM_STDIN").is_none() {
            return fail_with(tool, "PURGE_FAILED", "the plan could not be made", "purge", &[], (1, false, None));
        }
        if tool.flag("previewWrites") {
            std::fs::write(".purge-preview-cache", "written by a preview").unwrap_or_default();
        }
        let mut extra = Map::new();
        if !tool.flag("previewNoNext") {
            extra.insert("next".into(), json!([next_entry(tool, &commit_argv, "carry out exactly this plan")]));
        }
        let mut shown = data(false);
        if tool.flag("previewNoDigest") {
            shown.as_object_mut().map(|object| object.remove("planDigest"));
        }
        if tool.flag("previewNoEffects") {
            shown.as_object_mut().map(|object| object.remove("effects"));
        }
        if tool.flag("previewCommitted") {
            shown["committed"] = json!(true);
        }
        return tool.success_with("purge", shown, extra);
    }
    let given = parsed.text("plan-digest");
    if let Some(given) = given {
        if given != digest && !tool.flag("staleAccepted") {
            let next = (!tool.flag("staleNoNext")).then(|| next_entry(tool, &preview_argv, "plan again"));
            return fail_with(tool, "PLAN_STALE", "the state changed since the plan was made", "purge", &[("field", json!("--plan-digest"))], (2, false, next));
        }
    }
    if !tool.flag("commitNoEffect") {
        for (path, _) in &files {
            std::fs::remove_file(path).unwrap_or_default();
        }
        if !files.is_empty() {
            let mut audit = std::fs::OpenOptions::new().create(true).append(true).open(".purge-audit.log").unwrap_or_else(|_| std::process::exit(70));
            writeln!(audit, "purged {} {}", kind, files.len()).unwrap_or_default();
        }
    }
    let mut shown = data(!tool.flag("commitNotMarked"));
    if tool.flag("commitNotMarked") {
        shown["committed"] = json!(false);
    }
    if tool.flag("commitDigestChanges") {
        shown["planDigest"] = json!(plan_digest(&kind, &before, &purge_set(&kind)));
    }
    tool.success("purge", shown)
}

// ---- seal, stamp, log, peek ----

fn seal_run(tool: &Tool, _args: &[String], _parsed: &Parsed) -> ExitCode {
    let sealed = std::path::Path::new(".sealed").exists();
    if sealed && !tool.flag("onceTwice") {
        return tool.refuse("ALREADY_SEALED", "the directory is sealed already", "seal", &[]);
    }
    if !sealed {
        std::fs::write(".sealed", "sealed").unwrap_or_default();
    }
    tool.success("seal", json!({"sealed": true}))
}

fn stamp_run(tool: &Tool, _args: &[String], parsed: &Parsed) -> ExitCode {
    let text = parsed.text("text").unwrap_or_default();
    let key = digest(text).replace("sha256:", "");
    let path = std::path::Path::new("stamps").join(format!("{}.txt", &key[..16]));
    let kept = path.exists();
    // Seeded: every run appends an audit line, a named side effect that is not a new effect of the stamp.
    if tool.flag("stampAudit") && !parsed.has("dry-run") {
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(".stamp-audit.log").unwrap_or_else(|_| std::process::exit(70));
        writeln!(file, "stamped").unwrap_or_default();
    }
    // Seeded: every run also makes a directory of its own that holds nothing (`bare-0`, `bare-1`, ...): a directory is no effect.
    if tool.flag("stampBareDir") {
        let made = std::fs::read_dir(".").map_or(0, |entries| entries.flatten().filter(|entry| entry.file_name().to_string_lossy().starts_with("bare-")).count());
        std::fs::create_dir(format!("bare-{made}")).unwrap_or_default();
    }
    if parsed.has("dry-run") {
        if tool.flag("dryRunWrites") {
            std::fs::write("dry-run-cache.txt", "written by a dry run").unwrap_or_default();
        }
        return tool.success("stamp", json!({"stamp": text, "status": "planned"}));
    }
    if !kept || tool.flag("stampDuplicates") {
        std::fs::create_dir_all("stamps").unwrap_or_default();
        let target = if kept { std::path::Path::new("stamps").join(format!("{}-again.txt", &key[..16])) } else { path };
        std::fs::write(target, text).unwrap_or_default();
        return tool.success("stamp", json!({"stamp": text, "status": "written"}));
    }
    tool.success("stamp", json!({"stamp": text, "status": "unchanged"}))
}

fn log_run(tool: &Tool, _args: &[String], parsed: &Parsed) -> ExitCode {
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open("log.txt").unwrap_or_else(|_| std::process::exit(70));
    writeln!(file, "{}", parsed.text("text").unwrap_or_default()).unwrap_or_default();
    let lines = std::fs::read_to_string("log.txt").map_or(0, |text| text.lines().count());
    tool.success("log", json!({"lines": lines}))
}

fn peek_run(tool: &Tool, _args: &[String], _parsed: &Parsed) -> ExitCode {
    // Seeded: the access line goes to a directory the read creates (`.audit/access.log`), as canon's `.canon/audit/access.jsonl` does.
    let path = if tool.flag("peekAuditDir") {
        std::fs::create_dir_all(".audit").unwrap_or_else(|_| std::process::exit(70));
        ".audit/access.log"
    } else {
        ".peek-access.log"
    };
    // Seeded: the read also makes a directory that holds nothing: a directory is no effect.
    if tool.flag("peekBareDir") {
        std::fs::create_dir_all(".peek-empty").unwrap_or_else(|_| std::process::exit(70));
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap_or_else(|_| std::process::exit(70));
    writeln!(file, "peeked").unwrap_or_default();
    if tool.flag("peekWritesOther") {
        std::fs::write("peek-cache.txt", "written by a read").unwrap_or_default();
    }
    let lines = std::fs::read_to_string("log.txt").map_or(0, |text| text.lines().count());
    tool.success("peek", json!({"lines": lines}))
}

// ---- deploy ----

fn deploy(tool: &Tool, args: &[String], words: &[String]) -> ExitCode {
    let verbs = ["start", "status"];
    let Some(verb) = words.get(1).map(String::as_str) else {
        return tool.fail("group", "COMMAND_REQUIRED", "deploy needs a command", Some("deploy"), &[("expected", json!(verbs))], 2);
    };
    if !verbs.contains(&verb) {
        return tool.unknown_command(verb, &verbs, Some("deploy"));
    }
    let id = format!("deploy {verb}");
    let model = Model::of(tool);
    let Some(command) = model.command(&id) else {
        return tool.unknown_command(verb, &verbs, Some("deploy"));
    };
    match parse(tool, command, args, false) {
        Err(failure) => refuse_failure(tool, &id, failure),
        Ok(parsed) => {
            if let Some(refused) = tool.seeded_refusal(&id) {
                return refused;
            }
            if verb == "start" {
                deploy_start_run(tool, &parsed)
            } else {
                deploy_status_run(tool, &parsed)
            }
        }
    }
}

const DEPLOY_DIR: &str = ".deploy";

/// A small number made of `text`: the same text, the same number.
fn hash_of(text: &str) -> u32 {
    text.bytes().fold(7u32, |sum, byte| sum.wrapping_mul(31).wrapping_add(u32::from(byte)))
}

fn worker_start(deployment: &str) {
    std::fs::create_dir_all(DEPLOY_DIR).unwrap_or_default();
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(format!("{DEPLOY_DIR}/worker.log")).unwrap_or_else(|_| std::process::exit(70));
    writeln!(log, "start {deployment}").unwrap_or_default();
}

fn wait_argv(deployment: &str) -> Vec<String> {
    ["deploy", "status", deployment, "--wait", "--json"].map(String::from).to_vec()
}

fn accepted(tool: &Tool, deployment: &str, environment: &str) -> ExitCode {
    let state = if tool.flag("startClaimsDone") { "done" } else { "accepted" };
    let mut extra = Map::new();
    if !tool.flag("startNoWait") {
        let followed = if tool.flag("startIdentifierMismatch") { deployment.to_uppercase() } else { deployment.to_string() };
        let argv = if tool.flag("startNextWrites") {
            // Seeded: next[0] starts more work instead of reading the outcome.
            ["stamp", followed.as_str(), "--json"].map(String::from).to_vec()
        } else if tool.flag("startStatusOnly") {
            // A correct tool whose next[0] only reads the status: no `--wait`, so the outcomes are not driven.
            ["deploy", "status", followed.as_str(), "--json"].map(String::from).to_vec()
        } else {
            wait_argv(&followed)
        };
        extra.insert("next".into(), json!([next_entry(tool, &argv, "wait for the deployment to end")]));
    }
    tool.success_with("deploy start", json!({"deployment": deployment, "environment": environment, "state": state}), extra)
}

fn deploy_start_run(tool: &Tool, parsed: &Parsed) -> ExitCode {
    // The seeded violation: a start that fails when the worker is controlled (the fixture's `async` runs set it).
    if tool.flag("startFails") && std::env::var_os(tool.text("workerEnv", "FAKE_WORKER")).is_some() {
        return fail_with(tool, "DEPLOY_FAILED", "the deployment could not be queued", "deploy start", &[], (1, false, None));
    }
    let environment = parsed.text("environment").unwrap_or_default().to_string();
    std::fs::create_dir_all(DEPLOY_DIR).unwrap_or_default();
    let Some(key) = parsed.text("operation") else {
        // Without a key the request is its payload: the same environment names the same deployment, which is started once.
        let deployment = format!("d-{}", 1000 + hash_of(&environment) % 9000);
        let record = format!("{DEPLOY_DIR}/{deployment}.json");
        if !std::path::Path::new(&record).exists() {
            std::fs::write(&record, json!({"environment": environment}).to_string()).unwrap_or_default();
            worker_start(&deployment);
        }
        return accepted(tool, &deployment, &environment);
    };
    let claim = format!("{DEPLOY_DIR}/op-{key}");
    let deployment = format!("d-{}", 1000 + hash_of(key) % 9000);
    let body = format!("{environment}\n{deployment}\n");
    let created = if tool.flag("operationRace") {
        // The seeded violation: look, then act, with a gap another request can fall into.
        let exists = std::path::Path::new(&claim).exists();
        std::thread::sleep(std::time::Duration::from_millis(250));
        if !exists {
            std::fs::write(&claim, &body).unwrap_or_default();
        }
        !exists
    } else {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&claim) {
            Ok(mut file) => {
                file.write_all(body.as_bytes()).unwrap_or_default();
                true
            }
            Err(_) => false,
        }
    };
    if created {
        std::fs::write(format!("{DEPLOY_DIR}/{deployment}.json"), json!({"environment": environment}).to_string()).unwrap_or_default();
        worker_start(&deployment);
        return accepted(tool, &deployment, &environment);
    }
    // A repeat: wait for the reservation to be written, then compare the payload.
    let mut held = String::new();
    for _ in 0..60 {
        held = std::fs::read_to_string(&claim).unwrap_or_default();
        if held.lines().count() >= 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let mut lines = held.lines();
    let (first_environment, first_deployment) = (lines.next().unwrap_or_default().to_string(), lines.next().unwrap_or_default().to_string());
    if first_environment != environment && !tool.flag("operationReusedAccepted") {
        let hint = format!("{} deploy status {first_deployment} --json", tool.name());
        return fail_with(
            tool,
            "OPERATION_REUSED",
            "that --operation key names another request",
            "deploy start",
            &[("field", json!(if tool.flag("reusedFieldWrong") { "ENVIRONMENT" } else { "--operation" })), ("hint", json!(hint))],
            (2, false, None),
        );
    }
    if tool.flag("operationRepeatRefused") {
        return fail_with(tool, "OPERATION_REUSED", "that --operation key is taken", "deploy start", &[("field", json!("--operation"))], (2, false, None));
    }
    if tool.flag("operationRestarts") {
        worker_start(&first_deployment);
    }
    if tool.flag("operationNewId") {
        return accepted(tool, "d-9999", &first_environment);
    }
    accepted(tool, &first_deployment, &first_environment)
}

fn deploy_status_run(tool: &Tool, parsed: &Parsed) -> ExitCode {
    let asked = parsed.text("deployment").unwrap_or_default().to_string();
    let deployment = asked.to_lowercase();
    if !std::path::Path::new(&format!("{DEPLOY_DIR}/{deployment}.json")).exists() {
        return tool.refuse("DEPLOYMENT_NOT_FOUND", "there is no such deployment", "deploy status", &[("field", json!("DEPLOYMENT"))]);
    }
    if !parsed.has("wait") {
        return tool.success("deploy status", json!({"deployment": deployment, "state": "accepted"}));
    }
    let worker = tool.text("workerEnv", "FAKE_WORKER");
    let outcome = std::env::var(&worker).unwrap_or_else(|_| "done".to_string());
    match outcome.as_str() {
        "failed" if tool.flag("failedExits0") => tool.success("deploy status", json!({"deployment": deployment, "state": "failed"})),
        "failed" => fail_with(tool, "DEPLOY_FAILED", "the deployment failed", "deploy status", &[], (1, false, None)),
        "timeout" if tool.flag("timeoutExits0") => tool.success("deploy status", json!({"deployment": deployment, "state": "accepted"})),
        "timeout" => {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let mut argv = wait_argv(&asked);
            if let Some(timeout) = parsed.text("timeout") {
                argv.splice(argv.len() - 1..argv.len() - 1, ["--timeout".to_string(), timeout.to_string()]);
            }
            let next = (!tool.flag("timeoutNoNext")).then(|| next_entry(tool, &argv, "wait again"));
            let retryable = tool.config.get("retryableCodes").and_then(Value::as_array).is_some_and(|codes| codes.iter().any(|code| code == "WAIT_TIMEOUT"));
            fail_with(tool, "WAIT_TIMEOUT", "the deployment did not end in time", "deploy status", &[], (1, retryable, next))
        }
        _ if tool.flag("waitDoneExits1") => fail_with(tool, "DEPLOY_FAILED", "the deployment failed", "deploy status", &[], (1, false, None)),
        _ => tool.success("deploy status", json!({"deployment": deployment, "state": "done"})),
    }
}
