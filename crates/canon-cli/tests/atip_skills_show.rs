//! `canon skills show` as atip/1 1.2 spec 14 has it (canon 0.35 `canon-0-35-skills-show`): `NAME` is optional and the default guide
//! lists every guide, `--json` is the contract's guide shape, an unknown name refuses with the guide names, the first line of every
//! `--help` points at `skills show` too, and every command line a guide shows parses (`guide_commands_parse`). Tests are named by scenario
//! id (`atip.skillsshow.NN`).

mod harness;
mod rule_support;

use std::collections::BTreeSet;
use std::path::Path;

use rule_support::{document_problems, run_empty_with, sample};
use serde_json::{json, Value};

fn canon_in(dir: &Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_clear().env("PATH", std::env::var_os("PATH").unwrap_or_default()).env("HOME", dir).output().expect("spawn canon")
}

fn json_of(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&output.stdout)))
}

fn guide_names() -> Vec<String> {
    canon_cli::guide::names().iter().map(ToString::to_string).collect()
}

/// Every file of a directory tree with its bytes, to compare a directory before and after a call.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    fn walk(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).expect("read a directory").flatten().collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.push((path.strip_prefix(root).expect("inside").display().to_string(), std::fs::read(&path).unwrap_or_default()));
            }
        }
    }
    walk(dir, dir, &mut files);
    files
}

// ----------------------------------------------------------------------------------------------------------------- F1

/// atip.skillsshow.01: `canon skills show --json` without a name answers the default guide, `canon`, with every guide name in `guides`, in
/// an empty directory.
#[test]
fn atip_skillsshow_01_the_default_guide_lists_every_guide() {
    let empty = tempfile::tempdir().expect("an empty directory");
    let output = canon_in(empty.path(), &["skills", "show", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stdout));
    let document = json_of(&output);
    assert_eq!(document["ok"], true);
    assert_eq!(document["data"]["name"], "canon");
    let guides: Vec<String> = document["data"]["guides"].as_array().expect("guides").iter().map(|name| name.as_str().expect("a name").to_string()).collect();
    assert_eq!(guides, guide_names());
    assert!(guides.len() > 20);
    assert_eq!(document["data"]["text"].as_str(), canon_cli::guide::guide("canon"));
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    // The default guide's own text names every guide (spec 14.2).
    let text = document["data"]["text"].as_str().expect("text");
    let unnamed: Vec<&String> = guides.iter().filter(|name| !text.contains(name.as_str())).collect();
    assert!(unnamed.is_empty(), "the default guide does not name {unnamed:?}");
}

/// atip.skillsshow.02: the text of the call without a name is the default guide's Markdown byte for byte, and neither form writes
/// anything, in an empty directory or in one that holds files.
#[test]
fn atip_skillsshow_02_a_call_without_a_name_prints_the_default_guide_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::create_dir_all(dir.path().join(".canon")).expect("state");
    std::fs::write(dir.path().join("canon.yaml"), "project: x\n").expect("a file");
    let before = snapshot(dir.path());
    let bare = canon_in(dir.path(), &["skills", "show"]);
    let named = canon_in(dir.path(), &["skills", "show", "canon"]);
    assert_eq!(bare.status.code(), Some(0));
    assert_eq!(bare.stdout, named.stdout);
    assert_eq!(bare.stdout, canon_cli::guide::guide("canon").expect("the core guide").as_bytes());
    for args in [&["skills", "show", "--json"][..], &["skills", "show", "--full", "--json"], &["skills", "show", "role-planner", "--json"], &["skills", "show", "nosuch-xyz", "--json"]] {
        canon_in(dir.path(), args);
    }
    assert_eq!(snapshot(dir.path()), before, "no call of `skills show` writes");
    // Without a repository at all, and without a role, it answers the same.
    let empty = tempfile::tempdir().expect("an empty directory");
    assert_eq!(canon_in(empty.path(), &["skills", "show"]).stdout, bare.stdout);
    assert!(snapshot(empty.path()).is_empty());
}

// ----------------------------------------------------------------------------------------------------------------- F2

/// atip.skillsshow.03: every guide's `--json` is `{name, text, guides, full?}` and passes the contract's guide schema, `name` is the name
/// asked for, `x-canon.version` is canon's, and `--full` says so.
#[test]
fn atip_skillsshow_03_every_guide_passes_the_contracts_guide_schema() {
    let empty = tempfile::tempdir().expect("an empty directory");
    let mut names = guide_names();
    names.push(String::new());
    for name in &names {
        let mut args = vec!["skills", "show"];
        if !name.is_empty() {
            args.push(name);
        }
        args.push("--json");
        let document = json_of(&canon_in(empty.path(), &args));
        assert_eq!(document["ok"], true, "{name}: {document}");
        let data = &document["data"];
        assert_eq!(atip::guide_problems(data), Vec::<String>::new(), "`{name}`");
        assert_eq!(data["name"], if name.is_empty() { "canon" } else { name.as_str() });
        assert_eq!(data["x-canon"]["version"], env!("CARGO_PKG_VERSION"), "{name}");
        assert_eq!(data["full"], false, "{name}");
        assert_eq!(atip::envelope_problems(&document), Vec::<String>::new(), "{name}");
        // The command document says the same: the artifact is `SkillsShowReport`, which is the contract's guide schema.
        assert_eq!(document["meta"]["command"], "skills show");
        assert_eq!(document["meta"]["schemaVersion"], "2.0");
    }
    let full = json_of(&canon_in(empty.path(), &["skills", "show", "canon", "--full", "--json"]));
    assert_eq!(full["data"]["full"], true);
    assert_eq!(atip::guide_problems(&full["data"]), Vec::<String>::new());
    assert!(full["data"]["text"].as_str().expect("text").len() > canon_cli::guide::guide("canon").expect("guide").len());
    // `--full` on a guide that is complete already changes nothing, and `full` says nothing was added.
    let other = json_of(&canon_in(empty.path(), &["skills", "show", "role-planner", "--full", "--json"]));
    assert_eq!(other["data"]["full"], false);
    assert_eq!(other["data"]["text"].as_str(), canon_cli::guide::guide("role-planner"));
}

/// atip.skillsshow.04: the old members are gone, the contract's schema refuses a changed document, and the command document refers to the
/// schema by its `$id`.
#[test]
fn atip_skillsshow_04_the_old_members_are_gone_and_the_schema_is_held() {
    let empty = tempfile::tempdir().expect("an empty directory");
    let document = json_of(&canon_in(empty.path(), &["skills", "show", "canon", "--json"]));
    let data = document["data"].as_object().expect("data");
    assert!(!data.contains_key("markdown") && !data.contains_key("version"), "{:?}", data.keys().collect::<Vec<_>>());
    let keys: BTreeSet<&str> = data.keys().map(String::as_str).collect();
    assert_eq!(keys, BTreeSet::from(["name", "text", "guides", "full", "x-canon"]));
    // A document of the old shape, and a changed one, break the schema.
    let old = json!({ "name": "canon", "version": "0.34.0", "markdown": "# canon" });
    assert!(!atip::guide_problems(&old).is_empty());
    let mut broken = document["data"].clone();
    broken["guides"] = json!([]);
    assert!(!atip::guide_problems(&broken).is_empty(), "a guide list may not be empty");
    broken["guides"] = json!(["canon", "canon"]);
    assert!(!atip::guide_problems(&broken).is_empty(), "a guide name appears once");
    let mut upper = document["data"].clone();
    upper["name"] = json!("Canon");
    assert!(!atip::guide_problems(&upper).is_empty(), "a name is lower-case and hyphens");
    let mut extra = document["data"].clone();
    extra["version"] = json!("0.35.0");
    assert!(!atip::guide_problems(&extra).is_empty(), "only `x-` members are open");
    // The schema of the report is the contract's, by `$id`; one changed character of the vendored copy is caught by the digest.
    let doc = json_of(&canon_in(empty.path(), &["schema", "skills", "show", "--json"]));
    assert_eq!(doc["data"]["schemas"]["SkillsShowReport"]["$ref"], "https://atip.dev/1/guide.schema.json");
    assert_eq!(doc["data"]["outputs"][0]["kind"], "artifact");
    assert_eq!(doc["data"]["outputs"][0]["schemaVersion"], "2.0");
    assert_eq!(doc["data"]["operands"][0]["required"], false);
    assert_eq!(doc["data"]["operands"][0]["defaultNote"], "the default guide, `canon`");
}

// ----------------------------------------------------------------------------------------------------------------- F3

/// atip.skillsshow.05: an unknown name, and a value outside the grammar of a name (an upper-case letter, a space), refuse with exit 2, the
/// registered `USAGE`, `field` `NAME` and `expected` the guide names; and a name that exists is not refused.
#[test]
fn atip_skillsshow_05_an_unknown_name_refuses_with_the_guide_names() {
    for value in ["nosuch-xyz", "Canon", "role implementer", "role_implementer", ""] {
        let outcome = run_empty_with(&["skills".to_string(), "show".to_string(), value.to_string()], &[]);
        assert_eq!(outcome.exit, 2, "`{value}`: {outcome:?}");
        assert_eq!(outcome.code.as_deref(), Some("USAGE"), "`{value}`");
        assert_eq!(outcome.field.as_deref(), Some("NAME"), "`{value}`");
        let document: Value = serde_json::from_str(&outcome.stdout).expect("a failure envelope");
        let expected: Vec<String> = document["error"]["expected"].as_array().expect("expected").iter().map(|name| name.as_str().expect("a name").to_string()).collect();
        assert_eq!(expected, guide_names(), "`{value}`");
        assert!(outcome.changed.is_empty());
    }
    let known = run_empty_with(&["skills".to_string(), "show".to_string(), "role-reviewer".to_string()], &[]);
    assert_eq!(known.exit, 0, "{known:?}");
}

// ----------------------------------------------------------------------------------------------------------------- F4

fn first_line(args: &[&str]) -> String {
    let dir = tempfile::tempdir().expect("a directory");
    String::from_utf8_lossy(&canon_in(dir.path(), args).stdout).lines().next().unwrap_or_default().to_string()
}

/// atip.skillsshow.06: the first line of the help of the root and of a group and a command names `schema` and `skills show`, and the
/// introduction stays below it.
#[test]
fn atip_skillsshow_06_the_first_line_of_every_help_points_at_schema_and_skills_show() {
    for args in [&["--help"][..], &["finding", "--help"], &["finding", "close", "--help"], &["skills", "show", "--help"], &["help", "finding"], &["schema", "-h"]] {
        let line = first_line(args);
        assert!(line.contains("schema") && line.contains("skills show"), "{args:?}: {line}");
        assert_eq!(line, canon_cli::spec::HELP_POINTER, "{args:?}");
    }
    let mut root = canon_cli::cli::command();
    root.build();
    for node in atip::project::nodes(&root) {
        let help = node.cmd.clone().render_long_help().to_string();
        let first = help.lines().next().unwrap_or_default();
        assert!(first.contains("schema") && first.contains("skills show"), "`{}`: {first}", node.command);
    }
    // A line that names only `schema`, as 0.34 had it, is not enough: the check is on the line's own words.
    let old = "Agents: \"canon schema --json\" describes every command as JSON.";
    assert!(!old.contains("skills show"));
}

/// Whether a first line of help points at both entrances, `schema` and `skills show`.
fn points_at_both(line: &str) -> bool {
    line.contains("schema") && line.contains("skills show")
}

/// atip.skillsshow.10 (the failure path of F4): a first line that names only `schema` (as 0.34 had it), only `skills show`, or neither is
/// not the pointer; and a help whose pointer line is cut is read from the line below it, which is not the pointer either.
#[test]
fn atip_skillsshow_10_a_first_line_that_names_one_entrance_or_none_is_not_the_pointer() {
    for line in ["Agents: \"canon schema --json\" describes every command as JSON.", "Agents: \"canon skills show\" prints the guides.", "", "Usage: canon [OPTIONS] <COMMAND>"] {
        assert!(!points_at_both(line), "{line:?}");
    }
    assert!(points_at_both(canon_cli::spec::HELP_POINTER), "{}", canon_cli::spec::HELP_POINTER);
    let mut root = canon_cli::cli::command();
    root.build();
    for node in atip::project::nodes(&root) {
        let help = node.cmd.clone().render_long_help().to_string();
        let mut lines = help.lines();
        assert!(points_at_both(lines.next().unwrap_or_default()), "`{}`: the first line is the pointer", node.command);
        let below = lines.find(|line| !line.trim().is_empty()).unwrap_or_default();
        assert!(!points_at_both(below), "`{}`: the line below the pointer is the introduction, not a second pointer: {below}", node.command);
    }
}

// ----------------------------------------------------------------------------------------------------------------- F5

/// A command line of a guide: the guide, the 1-based line of its first physical line, and the text with its continuations joined.
#[derive(Debug, Clone)]
struct GuideLine {
    guide: String,
    line: usize,
    text: String,
}

/// Every line of the fenced code blocks of `text` that starts with `canon`, `\` continuations joined.
fn command_lines(guide: &str, text: &str) -> Vec<GuideLine> {
    let mut lines = Vec::new();
    let mut fenced = false;
    let mut pending: Option<GuideLine> = None;
    for (index, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") {
            fenced = !fenced;
            pending = None;
            continue;
        }
        if !fenced {
            continue;
        }
        if let Some(mut open) = pending.take() {
            open.text.push(' ');
            let more = trimmed.strip_suffix('\\');
            open.text.push_str(more.unwrap_or(trimmed).trim());
            if more.is_some() {
                pending = Some(open);
            } else {
                lines.push(open);
            }
            continue;
        }
        if trimmed == "canon" || trimmed.starts_with("canon ") {
            let more = trimmed.strip_suffix('\\');
            let line = GuideLine { guide: guide.to_string(), line: index + 1, text: more.unwrap_or(trimmed).trim().to_string() };
            if more.is_some() {
                pending = Some(line);
            } else {
                lines.push(line);
            }
        }
    }
    lines
}

/// Whether `word` is a placeholder of spec 14.5, `<UPPER_CASE>`.
fn is_placeholder(word: &str) -> bool {
    word.strip_prefix('<').and_then(|rest| rest.strip_suffix('>')).is_some_and(|name| !name.is_empty() && name.starts_with(|c: char| c.is_ascii_uppercase()) && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
}

/// The line with each `<UPPER_CASE>` placeholder cut out, or `None` when something else of shell syntax is in it (`|`, `>`, `<` that is
/// no placeholder, `;`, `&`, `$(`, a backtick): those lines are outside spec 14.5.
fn outside_the_rule(text: &str) -> bool {
    let mut rest = text;
    let mut stripped = String::new();
    while let Some(at) = rest.find('<') {
        stripped.push_str(&rest[..at]);
        match rest[at..].find('>') {
            Some(end) if is_placeholder(&rest[at..=at + end]) => rest = &rest[at + end + 1..],
            _ => {
                stripped.push('<');
                rest = &rest[at + 1..];
            }
        }
    }
    stripped.push_str(rest);
    ['|', '>', '<', ';', '&', '`'].iter().any(|c| stripped.contains(*c)) || stripped.contains("$(")
}

/// The command document whose path is the longest prefix of `words` (the words after `canon`).
fn document_of(words: &[String]) -> Option<&'static Value> {
    rule_support::docs().into_iter().filter(|doc| doc["path"].as_array().is_some_and(|path| !path.is_empty() && path.iter().zip(words).all(|(a, b)| a.as_str() == Some(b.as_str())) && path.len() <= words.len())).max_by_key(|doc| doc["path"].as_array().map_or(0, Vec::len))
}

/// `words` with each `<UPPER_CASE>` placeholder replaced by a value its argument accepts. The argument is the option the word follows or
/// belongs to (`--name=<X>`), else the next operand.
fn substituted(doc: &Value, words: &[String]) -> Result<Vec<String>, String> {
    let path = doc["path"].as_array().map_or(0, Vec::len);
    let operands: Vec<&Value> = doc["operands"].as_array().into_iter().flatten().collect();
    let mut out: Vec<String> = Vec::new();
    let mut operand_at = 0;
    let mut at = path;
    out.extend(words[..path].iter().cloned());
    let mut only_operands = false;
    while at < words.len() {
        let word = &words[at];
        at += 1;
        if !only_operands && word == "--" {
            only_operands = true;
            out.push(word.clone());
            continue;
        }
        if !only_operands && word.starts_with("--") {
            let (name, inline) = match word[2..].split_once('=') {
                Some((name, value)) => (name.to_string(), Some(value.to_string())),
                None => (word[2..].to_string(), None),
            };
            let option = doc["options"].as_array().into_iter().flatten().find(|option| option["name"] == name.as_str());
            let Some(option) = option else {
                out.push(word.clone());
                continue;
            };
            if matches!(option["valueType"].as_str(), Some("switch" | "count")) {
                out.push(word.clone());
                continue;
            }
            let value = match inline {
                Some(value) => value,
                None => match words.get(at) {
                    Some(value) => {
                        at += 1;
                        value.clone()
                    }
                    None => {
                        out.push(word.clone());
                        continue;
                    }
                },
            };
            let value = if is_placeholder(&value) { sample(doc, &[], option) } else { value };
            if word.contains('=') {
                out.push(format!("--{name}={value}"));
            } else {
                out.push(word.clone());
                out.push(value);
            }
            continue;
        }
        let word = if is_placeholder(word) {
            let operand = operands.get(operand_at.min(operands.len().saturating_sub(1))).ok_or_else(|| format!("`{word}` fills no operand of `{}`", doc["command"]))?;
            sample(doc, &[], operand)
        } else {
            word.clone()
        };
        operand_at += 1;
        out.push(word);
    }
    Ok(out)
}

/// What is wrong with the command lines of `text`, one problem per line with the guide and its line number: the line does not parse by the
/// grammar of the command document (9.12, with the domains, `required` and the constraints) or by canon's own parser and the stateless stage.
fn guide_problems_of(guide: &str, text: &str) -> (Vec<String>, usize, usize) {
    let mut problems = Vec::new();
    let (mut checked, mut skipped) = (0, 0);
    let mut root = canon_cli::cli::command();
    root.build();
    for line in command_lines(guide, text) {
        let place = format!("{} line {}", line.guide, line.line);
        if outside_the_rule(&line.text) {
            skipped += 1;
            continue;
        }
        let words = atip::split_argv(&line.text);
        let Some(doc) = document_of(&words[1..]) else {
            problems.push(format!("{place}: `{}` names no command", line.text));
            continue;
        };
        let calls = match input_calls(doc, &words[1..]) {
            Some(Ok(calls)) => Ok(calls),
            Some(Err(problem)) => Err(problem),
            None => substituted(doc, &words[1..]).map(|call| vec![call]),
        };
        let calls = match calls {
            Ok(calls) if !calls.is_empty() => calls,
            Ok(_) => {
                problems.push(format!("{place}: `{}` has an empty `--input` document", line.text));
                continue;
            }
            Err(problem) => {
                problems.push(format!("{place}: {problem}"));
                continue;
            }
        };
        checked += 1;
        for call in calls {
            check_call(&mut problems, &root, doc, &place, &line.text, &call);
        }
    }
    (problems, checked, skipped)
}

/// The calls a `--input` line stands for: each element of its JSON document written out as the arguments it names (an option by its
/// camelCase name, `operands`), with the words of the line outside `--input`. `None` for a line without `--input`.
fn input_calls(doc: &Value, words: &[String]) -> Option<Result<Vec<Vec<String>>, String>> {
    let at = words.iter().position(|word| word == "--input")?;
    let rest: Vec<String> = words.iter().enumerate().filter(|(index, _)| *index != at && *index != at + 1).map(|(_, word)| word.clone()).collect();
    let Some(text) = words.get(at + 1) else { return Some(Err("`--input` has no value".to_string())) };
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => return Some(Err(format!("the `--input` document is not JSON: {error}"))),
    };
    let elements = match value {
        Value::Array(elements) => elements,
        other => vec![other],
    };
    let path = doc["path"].as_array().map_or(0, Vec::len);
    let mut calls = Vec::new();
    for element in elements {
        let Some(object) = element.as_object() else { return Some(Err("an `--input` element is not an object".to_string())) };
        let mut call: Vec<String> = rest[..path.min(rest.len())].to_vec();
        for operand in object.get("operands").and_then(Value::as_array).into_iter().flatten() {
            call.push(operand.as_str().map_or_else(|| operand.to_string(), str::to_string));
        }
        for (key, value) in object {
            if key == "operands" {
                continue;
            }
            let Some(option) = doc["options"].as_array().into_iter().flatten().find(|option| atip::project::camel_case(option["name"].as_str().unwrap_or_default()) == *key) else {
                return Some(Err(format!("`{key}` is no key of an `--input` document of `{}`", doc["command"])));
            };
            let name = option["name"].as_str().unwrap_or_default();
            let values: Vec<&Value> = match value {
                Value::Array(items) => items.iter().collect(),
                other => vec![other],
            };
            for value in values {
                if matches!(option["valueType"].as_str(), Some("switch" | "count")) {
                    if value == &Value::Bool(true) {
                        call.push(format!("--{name}"));
                    }
                } else {
                    call.push(format!("--{name}={}", value.as_str().map_or_else(|| value.to_string(), str::to_string)));
                }
            }
        }
        call.extend(rest[path.min(rest.len())..].iter().cloned());
        calls.push(call);
    }
    Some(Ok(calls))
}

/// What the document and canon's own parser say of one call of a command.
fn check_call(problems: &mut Vec<String>, root: &clap::Command, doc: &Value, place: &str, line: &str, call: &[String]) {
    for problem in document_problems(doc, call) {
        problems.push(format!("{place}: `{line}` breaks the document of `{}`: {problem}", doc["command"]));
    }
    // canon's own parser: the relaxed parse, the domain and constraint stage, then the real parse (`validators::parse`'s stages).
    let argv: Vec<String> = std::iter::once("canon".to_string()).chain(call.iter().cloned()).collect();
    let first_line = |error: &clap::Error| error.to_string().lines().next().unwrap_or_default().to_string();
    match canon_cli::arg_rules::relaxed::tree(canon_cli::cli::command()).try_get_matches_from(argv.clone()) {
        Err(error) if error.use_stderr() => problems.push(format!("{place}: `{line}` does not parse: {}", first_line(&error))),
        Err(_) => {}
        Ok(matches) => {
            if let Some((_, refusal)) = canon_cli::arg_rules::enforce::check(root, &matches) {
                problems.push(format!("{place}: `{line}` is refused: {}", refusal.message));
            } else if let Err(error) = root.clone().try_get_matches_from(argv) {
                if error.use_stderr() {
                    problems.push(format!("{place}: `{line}` does not parse: {}", first_line(&error)));
                }
            }
        }
    }
}

/// atip.skillsshow.07 (`guide_commands_parse`): every command line in the fenced code blocks of every guide parses, as `canon schema` says
/// it does and as canon's own parser does, once its placeholders are values.
#[test]
fn atip_skillsshow_07_guide_commands_parse() {
    let (mut problems, mut checked, mut skipped) = (Vec::new(), 0, 0);
    for name in guide_names() {
        let text = canon_cli::guide::guide(&name).expect("a guide");
        let (found, ok, out) = guide_problems_of(&name, text);
        problems.extend(found);
        checked += ok;
        skipped += out;
    }
    let full = canon_cli::guide::full();
    assert!(full.len() > 100_000);
    assert!(checked > 200, "{checked} command lines were checked ({skipped} are outside the rule)");
    assert!(problems.is_empty(), "{} lines of the guides do not parse ({checked} did):\n{}", problems.len(), problems.join("\n"));
}

/// atip.skillsshow.08: the test finds a line that is wrong: an option that is none, a missing required argument, a value outside its
/// domain, a command that is none; a line with a pipe and a line that is not in a code block are outside the rule.
#[test]
fn atip_skillsshow_08_a_wrong_line_in_a_guide_is_found() {
    let good = "```bash\ncanon finding add --change add-login --round 1 --seq 1 --severity note --actor alice --summary text\n```\n";
    let (problems, checked, _) = guide_problems_of("probe", good);
    assert_eq!((problems.len(), checked), (0, 1), "{problems:?}");
    for (what, wrong) in [
        ("an option that is none", good.replace("--change", "--chnage")),
        ("a required argument missing", good.replace("--change add-login ", "")),
        ("a value outside its domain", good.replace("--round 1", "--round 0")),
        ("a command that is none", good.replace("finding add", "finding nosuch")),
        ("an unknown value of a closed set", good.replace("--severity note", "--severity loud")),
    ] {
        let (problems, checked, _) = guide_problems_of("probe", &wrong);
        assert_eq!(checked, usize::from(what != "a command that is none"), "{what}");
        assert!(problems.iter().any(|problem| problem.starts_with("probe line 2:")), "{what}: {problems:?}");
    }
    // Outside the rule: shell syntax, a line outside a fence, a line that does not start with the tool name.
    let outside = "canon finding nosuch\n```bash\ncanon finding list | head\ncanon status > out.txt\ncanon finding nosuch <x>\nls canon finding nosuch\n```\n";
    let (problems, checked, skipped) = guide_problems_of("probe", outside);
    assert_eq!((problems.len(), checked, skipped), (0, 0, 3), "{problems:?}");
    // A placeholder is replaced by a valid value; one that is not `<UPPER_CASE>` is shell syntax.
    let placeholders = "```bash\ncanon finding add --change <CHANGE_ID> --round <N> --seq <N> --severity <SEVERITY> --actor <ACTOR_ID> --summary <TEXT>\n```\n";
    let (problems, checked, _) = guide_problems_of("probe", placeholders);
    assert_eq!((problems.len(), checked), (0, 1), "{problems:?}");
    let lower = "```bash\ncanon brief show <slug>\n```\n";
    assert_eq!(guide_problems_of("probe", lower).2, 1);
}

// ----------------------------------------------------------------------------------------------------------------- F6

/// The lines of `text` that read an old member of the output of `skills show` (`markdown`, a top-level `version`) within three lines of
/// the command's name: the 1-based line number and the line with `skills show`.
fn old_member_readers(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (at, line) in text.lines().enumerate() {
        if !line.contains("skills show") {
            continue;
        }
        let window: String = text.lines().skip(at).take(3).collect::<Vec<_>>().join(" ");
        if window.contains(".markdown") || window.contains("['markdown']") || window.contains("[\"markdown\"]") || window.contains("data.version") || window.contains(".data.version") || window.contains("{name, version, markdown}") {
            found.push((at + 1, line.to_string()));
        }
    }
    found
}

/// atip.skillsshow.09: nothing reads the old names (`markdown`, a top-level `version`) of the output of `skills show`: no guide, no
/// script, no hook and no page of the website names them as a member of it, and the installed skill is still the stub that points here.
#[test]
fn atip_skillsshow_09_no_consumer_reads_the_old_names() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["canon/skills", "scripts", "packages/website/src", ".github", "docs"] {
        collect(&root.join(dir), &mut files);
    }
    let mut problems = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        for (at, line) in old_member_readers(&text) {
            problems.push(format!("{} line {at}: reads an old member of the output: {line}", file.display()));
        }
    }
    assert!(!files.is_empty());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // The installed skill stays the stub 0.28 made it: it points at `skills show` and holds none of the output.
    let stub = canon_cli::guide::guide("canon").expect("guide");
    assert!(stub.contains("canon skills show"));
}

/// atip.skillsshow.11 (the failure path of F6): the scan finds a reader of `markdown` or of a top-level `version` of the output however it
/// is written, and passes a read of a member the output has now, so a reader that comes back fails `atip.skillsshow.09`.
#[test]
fn atip_skillsshow_11_the_scan_finds_a_reader_of_an_old_member_and_passes_a_current_one() {
    for text in [
        "canon skills show canon --json | jq -r .data.markdown",
        "canon skills show canon --json | jq .data.version",
        "const out = JSON.parse(run('canon skills show canon --json'));\nconst text = out['markdown'];",
        "const out = JSON.parse(run('canon skills show canon --json'));\nconst text = out[\"markdown\"];",
        "`canon skills show` prints {name, version, markdown}.",
    ] {
        assert_eq!(old_member_readers(text).len(), 1, "{text}");
    }
    for text in ["canon skills show canon --json | jq -r .data.text", "canon skills show canon --json | jq .data.guides", "the markdown of a guide is plain text", "canon schema --json | jq .data.version"] {
        assert!(old_member_readers(text).is_empty(), "{text}");
    }
    // A reader four lines below the command is out of the window, and the line of the command is what is reported.
    let far = "canon skills show canon --json\n\n\n\njq .data.markdown";
    assert!(old_member_readers(far).is_empty());
    assert_eq!(old_member_readers("a\ncanon skills show canon --json | jq .data.markdown"), [(2, "canon skills show canon --json | jq .data.markdown".to_string())]);
}

fn collect(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "node_modules" || name == "dist" || name == ".git") {
                continue;
            }
            collect(&path, files);
        } else if path.extension().is_some_and(|ext| matches!(ext.to_str(), Some("md" | "mdx" | "sh" | "py" | "ts" | "tsx" | "js" | "json" | "yml" | "yaml"))) {
            files.push(path);
        }
    }
}
