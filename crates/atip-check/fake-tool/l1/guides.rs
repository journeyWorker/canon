//! The entries an agent uses a tool through, besides its commands (approaches/checker.md 6, L2): `skills show` (the guides built
//! into the binary), `build` (a stream: NDJSON lines in JSON mode) and `schema --search`.
//!
//! Each violation is a knob of the configuration; each names the rule of the contract it breaks where it acts.

use std::process::ExitCode;

use serde_json::{json, Map, Value};

use super::doc::{tree_doc, Model};
use super::parse::{parse, Parsed};
use super::run::refuse_failure;
use super::spec::{Arg, Command};
use crate::Tool;

/// The names of the guides of this build.
const GUIDES: [&str; 2] = ["overview", "safety"];

pub const SKILLS_SUMMARY: &str = "Read the guides built into the tool";

/// The commands of this module.
pub fn commands() -> Vec<Command> {
    vec![skills_show(), build()]
}

fn skills_show() -> Command {
    Command::new("skills show", "Print a guide: the default one, or the one NAME names")
        .operand(Arg::operand("name", "string", "The guide to print").value("NAME"))
        .options(vec![Arg::option("full", "switch", "Include the long material")])
        .errors(&["USAGE"])
        .output(
            "GuideReport",
            json!({
                "type": "object",
                "required": ["name", "guides"],
                "properties": {
                    "name": {"type": "string"},
                    "text": {"type": "string"},
                    "guides": {"type": "array", "items": {"type": "string"}},
                    "full": {"type": "boolean"}
                }
            }),
        )
        .examples(&["{tool} skills show --json", "{tool} skills show safety --json"])
}

fn build() -> Command {
    let mut command = Command::new("build", "Build the targets, one event for each step")
        .options(vec![
            Arg::option("target", "string", "What to build").value("TARGET").default(json!("all")).domain(json!({"maxLength": 20})),
            Arg::option("quiet", "switch", "Print the result only"),
        ])
        .errors(&["BUILD_FAILED", "USAGE"])
        .examples(&["{tool} build --json", "{tool} build --quiet --json"]);
    command = command.output_stream(
        "BuildResult",
        "BuildEvent",
        None,
        json!({"type": "object", "required": ["artifacts"], "properties": {"artifacts": {"type": "integer", "minimum": 0}}, "additionalProperties": false}),
        json!({
            "type": "object",
            "required": ["step", "status"],
            "properties": {"step": {"type": "string"}, "status": {"enum": ["started", "done"]}},
            "additionalProperties": false
        }),
    );
    command.output_stream(
        "BuildQuietResult",
        "NoEvent",
        Some("--quiet"),
        json!({"type": "object", "required": ["artifacts", "quiet"], "properties": {"artifacts": {"type": "integer", "minimum": 0}, "quiet": {"type": "boolean"}}, "additionalProperties": false}),
        json!(false),
    )
}

/// The text of a guide: Markdown with fenced command lines. `{tool}` is the tool's name.
fn guide_text(tool: &Tool, name: &str) -> String {
    let body = match name {
        "overview" => concat!(
            "# Using {tool}\n\n",
            "Read the state, list the members and tag one.\n\n",
            "```bash\n",
            "{tool} status --json\n",
            "{tool} member list --team <TEAM> --sort <SORT> --limit 2 --json\n",
            "{tool} member tag <MEMBER> --tag lead --note \"a note\" --json\n",
            "{tool} member search <QUERY> --tag lead \\\n",
            "  --exact --json\n",
            "{tool} schema member list --json | head -c 200\n",
            "```\n\n",
            "Anything else on a line is prose: {tool} is not run from it.\n",
        ),
        _ => concat!(
            "# Destructive commands\n\n",
            "A destructive command previews first, and `--commit` carries the plan out.\n\n",
            "```bash\n",
            "{tool} purge --kind <KIND> --before 2030-01-01 --json\n",
            "{tool} deploy start staging --operation <KEY> --json\n",
            "```\n",
        ),
    };
    let mut text = body.replace("{tool}", &tool.name());
    if tool.flag("guideBadOption") && name == "overview" {
        text.push_str(&format!("\n```bash\n{} status --bogus-option --json\n```\n", tool.name()));
    }
    text
}

/// Answers the commands of this module; `None` for any other.
pub fn handle(tool: &Tool, args: &[String], words: &[String]) -> Option<ExitCode> {
    match words.first().map(String::as_str)? {
        "skills" => Some(skills(tool, args, words)),
        "build" => Some(build_run(tool, args)),
        _ => None,
    }
}

fn skills(tool: &Tool, args: &[String], words: &[String]) -> ExitCode {
    let Some(verb) = words.get(1).map(String::as_str) else {
        return tool.fail("group", "COMMAND_REQUIRED", "skills needs a command", Some("skills"), &[("expected", json!(["show"]))], 2);
    };
    if verb != "show" {
        return tool.unknown_command(verb, &["show"], Some("skills"));
    }
    let model = Model::of(tool);
    let Some(command) = model.command("skills show") else {
        return tool.unknown_command(verb, &["show"], Some("skills"));
    };
    let parsed = match parse(tool, command, args, false) {
        Ok(parsed) => parsed,
        Err(failure) => return refuse_failure(tool, "skills show", failure),
    };
    skills_answer(tool, &parsed)
}

fn skills_answer(tool: &Tool, parsed: &Parsed) -> ExitCode {
    let named = parsed.text("name");
    let name = named.unwrap_or(GUIDES[0]);
    if !GUIDES.contains(&name) {
        let mut extras: Vec<(&str, Value)> = vec![("field", json!("NAME")), ("received", json!(name))];
        if !tool.flag("skillsUnknownNoExpected") {
            let names: Vec<&str> = GUIDES.iter().copied().filter(|guide| !(tool.flag("skillsDefaultNotListed") && *guide == "overview")).collect();
            extras.push(("expected", json!(names)));
        }
        if tool.flag("skillsUnknownOk") {
            return tool.success("skills show", json!({"name": GUIDES[0], "text": guide_text(tool, GUIDES[0]), "guides": GUIDES}));
        }
        return tool.refuse("USAGE", &format!("{name} is not a guide"), "skills show", &extras);
    }
    if tool.flag("skillsNeedsState") && !std::path::Path::new("state.txt").exists() {
        return tool.refuse("USAGE", "skills show needs the state of the project", "skills show", &[("field", json!("--repo"))]);
    }
    if tool.flag("skillsPutsFile") {
        std::fs::write("guide-cache.txt", "written by skills show").unwrap_or_default();
    }
    let mut guides: Vec<&str> = GUIDES.to_vec();
    if tool.flag("skillsMissingName") {
        guides.retain(|guide| *guide != "safety");
    }
    if tool.flag("skillsDefaultNotListed") {
        guides.retain(|guide| *guide != "overview");
    }
    if tool.flag("skillsGuidesDiffer") && named.is_some() {
        guides.retain(|guide| *guide == name);
    }
    let shown = if tool.flag("skillsWrongName") && named.is_some() { GUIDES[0] } else { name };
    let mut data = json!({"name": shown, "text": guide_text(tool, name), "guides": guides});
    if parsed.has("full") {
        data["full"] = json!(true);
    }
    if tool.flag("skillsBadSchema") && named.is_none() {
        data["markdown"] = json!(guide_text(tool, name));
    }
    if tool.flag("skillsGuideBadSchema") && named.is_some() {
        data.as_object_mut().map(|object| object.remove("text"));
    }
    tool.success("skills show", data)
}

// ---- build: a stream ----

fn stream_meta(tool: &Tool, output: &str) -> Map<String, Value> {
    let mut meta = tool.meta(Some("build"));
    meta.insert("output".into(), json!(output));
    meta
}

fn build_run(tool: &Tool, args: &[String]) -> ExitCode {
    let model = Model::of(tool);
    let Some(command) = model.command("build") else {
        return tool.unknown_command("build", &[], None);
    };
    let parsed = match parse(tool, command, args, false) {
        Ok(parsed) => parsed,
        Err(failure) => {
            if tool.flag("streamRefusalHasStart") && tool.json && args.iter().any(|arg| arg == "--nosuch-xyz-option") {
                // The seeded violation: a usage error after a start line, as a stream ending in a failure: the refusal of a stream is one envelope.
                let _ = failure;
                print_start(tool, "BuildResult", false);
                let error = json!({"code": "USAGE", "message": "--nosuch-xyz-option is not an option", "field": "--nosuch-xyz-option", "retryable": false});
                tool.print_document(&json!({"type": "end", "result": {"ok": false, "error": error, "meta": stream_meta(tool, "BuildResult")}}));
                return ExitCode::from(2);
            }
            return refuse_failure(tool, "build", failure);
        }
    };
    if let Some(refused) = tool.seeded_refusal("build") {
        return refused;
    }
    let quiet = parsed.has("quiet");
    let broken = parsed.text("target") == Some("broken");
    let output = if quiet { "BuildQuietResult" } else { "BuildResult" };
    if !tool.json {
        println!("build: {} steps", if quiet { 0 } else { 2 });
        return ExitCode::SUCCESS;
    }
    if tool.flag("streamPrintsEnvelope") {
        // The seeded violation: a command that declares a stream prints one report.
        let data = if quiet { json!({"artifacts": 2, "quiet": true}) } else { json!({"artifacts": 2}) };
        return tool.success_with("build", data, stream_meta(tool, output));
    }
    let mut start = json!({"type": "start", "contract": "atip/1", "tool": tool.name(), "toolVersion": "1.2.3", "command": "build", "schemaVersion": "1.0"});
    if tool.is_1_1() {
        start["contractVersion"] = json!(tool.minor());
    }
    if !tool.flag("streamNoOutputOnStart") {
        start["output"] = json!(output);
    }
    if tool.flag("streamStartVersionMismatch") {
        start["schemaVersion"] = json!("2.0");
    }
    tool.print_document(&start);
    if !quiet {
        for (step, status) in [("compile", "started"), ("compile", "done")] {
            let status = if tool.flag("streamBadEvent") { "nosuch" } else { status };
            tool.print_document(&json!({"type": "event", "data": {"step": step, "status": status}}));
        }
    }
    if tool.flag("streamNoEnd") {
        return ExitCode::SUCCESS;
    }
    let mut meta = stream_meta(tool, output);
    if broken {
        // A step that fails after the stream began: the end result is the failure, and the exit is its registered one.
        let retryable = tool.flag("streamEndRetryableWrong");
        let error = json!({"code": "BUILD_FAILED", "message": "step link failed", "retryable": retryable});
        tool.print_document(&json!({"type": "end", "result": {"ok": false, "error": error, "meta": meta}}));
        return ExitCode::from(if tool.flag("streamBrokenExit0") { 0 } else { 2 });
    }
    if tool.flag("streamEndOutputMismatch") {
        meta.insert("output".into(), json!(if quiet { "BuildResult" } else { "BuildQuietResult" }));
    }
    let mut data = if quiet { json!({"artifacts": 2, "quiet": true}) } else { json!({"artifacts": 2}) };
    if tool.flag("streamEndBadData") {
        data = json!({"artifacts": "two"});
    }
    tool.print_document(&json!({"type": "end", "result": {"ok": true, "data": data, "meta": meta}}));
    ExitCode::SUCCESS
}

fn print_start(tool: &Tool, output: &str, wrong_version: bool) {
    let mut start = json!({"type": "start", "contract": "atip/1", "tool": tool.name(), "toolVersion": "1.2.3", "command": "build", "schemaVersion": if wrong_version { "2.0" } else { "1.0" }, "output": output});
    if tool.is_1_1() {
        start["contractVersion"] = json!(tool.minor());
    }
    tool.print_document(&start);
}

// ---- schema --search ----

/// The words of `text`, lowercase, a trailing `s` dropped.
fn stems(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.len() > 2)
        .map(|word| word.to_lowercase().trim_end_matches('s').to_string())
        .collect()
}

/// The tree that answers `query`: the commands that share words with it, the best first, and the query itself.
pub fn search_tree(tool: &Tool, model: &Model, query: &str) -> Value {
    let wanted = stems(query);
    let mut entries: Vec<(usize, Value)> = model
        .all_ids()
        .into_iter()
        .filter_map(|(id, is_group)| {
            let entry = model.entry(&id, is_group);
            let text = format!("{id} {}", entry["summary"].as_str().unwrap_or_default());
            let words = stems(&text);
            let score = wanted.iter().filter(|word| words.contains(word)).count();
            (score > 0).then_some((score, entry))
        })
        .collect();
    entries.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    if tool.flag("searchRanksLast") {
        // The seeded violation: every command answers, the best last.
        let answered: Vec<String> = entries.iter().filter_map(|(_, entry)| entry["command"].as_str().map(ToString::to_string)).collect();
        let mut rest: Vec<(usize, Value)> = model
            .all_ids()
            .into_iter()
            .filter(|(id, _)| !answered.contains(id))
            .map(|(id, is_group)| (0, model.entry(&id, is_group)))
            .collect();
        entries.reverse();
        rest.append(&mut entries);
        entries = rest;
    }
    let mut tree = tree_doc(model, false, "L2");
    tree["commands"] = Value::Array(entries.into_iter().map(|(_, entry)| entry).collect());
    if !tool.flag("searchNoQuery") {
        tree["query"] = json!(query);
    }
    if tool.flag("searchNotTree") {
        tree["kind"] = json!("group");
    }
    tree
}
