//! What the L1 part of the fake tool does when it is run: `--help`, `schema`, and the `member` commands.

use std::process::ExitCode;

use serde_json::{json, Map, Value};

use super::doc::{command_doc, group_doc, patched, tree_doc, Model};
use super::help;
use super::parse::{parse, Failure, Parsed};
use super::project;
use crate::Tool;

/// The words of a path: the arguments before `--` that are no option, `--repo` taking its value with it.
pub fn path_words(args: &[String]) -> Vec<String> {
    let mut words = Vec::new();
    let mut rest = args.iter().take_while(|arg| arg.as_str() != "--");
    while let Some(arg) = rest.next() {
        if arg == "--repo" {
            rest.next();
        } else if !arg.starts_with('-') {
            words.push(arg.clone());
        }
    }
    words
}

/// Answers the invocations the L1 part owns: `--help`, `schema` and `member`; `None` leaves the rest to the
/// L0 part of the tool.
pub fn handle(tool: &Tool, args: &[String]) -> Option<ExitCode> {
    let before: Vec<String> = args
        .iter()
        .take_while(|arg| arg.as_str() != "--")
        .cloned()
        .collect();
    let words = path_words(args);
    if before.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Some(help_for(tool, &words));
    }
    if tool.flag("l2") {
        if let Some(code) = super::safety::handle(tool, args, &words).or_else(|| super::guides::handle(tool, args, &words)) {
            return Some(code);
        }
    }
    match words.first().map(String::as_str) {
        Some("member") => Some(member(tool, args, &words)),
        Some("serve") if tool.config.contains_key("serveMode") => Some(serve_command(tool, args)),
        Some("record") if tool.flag("l2") => Some(record(tool, args, &words)),
        Some("schema") if !tool.flag("noSchema") => Some(schema(tool, args)),
        _ => None,
    }
}

/// `serve` of the seeded `notRun` runs: its arguments are parsed by the model, then it serves (`Tool::serve`).
fn serve_command(tool: &Tool, args: &[String]) -> ExitCode {
    let model = Model::of(tool);
    let Some(command) = model.command("serve") else {
        return tool.unknown_command("serve", &[], None);
    };
    match parse(tool, command, args, false) {
        Err(failure) => refuse_failure(tool, "serve", failure),
        Ok(_) => crate::serve(tool),
    }
}

/// The help of the deepest node the words name; the root's when they name none.
fn help_for(tool: &Tool, words: &[String]) -> ExitCode {
    let model = Model::of(tool);
    let mut current = String::new();
    for word in words {
        let candidate = if current.is_empty() {
            word.clone()
        } else {
            format!("{current} {word}")
        };
        if model.command(&candidate).is_some() {
            current = candidate;
            break;
        }
        if model.group(&candidate).is_some() {
            current = candidate;
        } else {
            break;
        }
    }
    if tool.flag("helpWrites") {
        std::fs::write("help-cache.txt", "written by --help").unwrap_or_default();
    }
    let text = if let Some(command) = model.command(&current) {
        let doc = patched(
            tool,
            &current,
            command_doc(&tool.name(), command),
        );
        help::command_help(tool, &doc)
    } else {
        help::group_help(tool, &model, &current)
    };
    print!("{text}");
    ExitCode::SUCCESS
}

/// A refusal of the model's own rules, in the shape the configuration asks for.
pub fn refuse_failure(tool: &Tool, command: &str, failure: Failure) -> ExitCode {
    match failure {
        Failure::Unknown(option) => tool.unknown_option(&option, Some(command)),
        Failure::Refuse {
            code,
            message,
            field,
            expected,
            source,
        } => {
            let mut extras: Vec<(&str, Value)> = Vec::new();
            if source == "domain" && tool.flag("domainOmitsField") {
                extras.push((
                    "hint",
                    json!(format!("{} schema {command} --json", tool.name())),
                ));
            } else if let Some(field) = field {
                extras.push(("field", json!(field)));
            }
            if let Some(expected) = expected {
                extras.push(("expected", json!(expected)));
            }
            tool.refuse(&code, &message, command, &extras)
        }
    }
}

// ---- schema ----

fn schema(tool: &Tool, args: &[String]) -> ExitCode {
    let model = Model::of(tool);
    let Some(command) = model.command("schema") else {
        return tool.unknown_command("schema", &[], None);
    };
    // A configuration cannot say that the tool needs the configuration: the run in an empty directory has none to
    // read. The seeded violations of `no-state` are the executable's name (a copy of this tool under another name).
    let alias = std::env::args().next().unwrap_or_default();
    let alias = std::path::Path::new(&alias)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if alias.contains("needs-config") && std::env::var_os("FAKE_ATIP_CONFIG").is_none() {
        return tool.refuse(
            "USAGE",
            "schema needs the configuration file",
            "schema",
            &[("field", json!("--repo"))],
        );
    }
    // The seeded violation of `writes-nothing`: a cache file, written when the answer is given and not by a refusal.
    let writes = tool.flag("schemaWrites") || alias.contains("writes");
    let cache = || {
        if writes {
            std::fs::write("schema-cache.txt", "written by schema").unwrap_or_default();
        }
    };
    let mut parsed = parse(tool, command, args, false);
    if let Err(Failure::Refuse {
        source: "constraint",
        field,
        ..
    }) = &parsed
    {
        // The seeded violations: two of the registry forms, or a path and a registry form, are answered. The
        // argument at fault is the first one given, in the order of the rule: the path comes first.
        let skip = if field.as_deref() == Some("PATH") {
            // Only `schema <group> --errors` is answered: `schema --errors x` keeps its refusal.
            tool.flag("schemaGroupErrorsAnswers")
                && parse(tool, command, args, true).is_ok_and(|lenient| {
                    lenient
                        .text("path")
                        .and_then(|word| word.split_whitespace().next().map(ToString::to_string))
                        .is_some_and(|word| model.group(&word).is_some())
                })
        } else {
            tool.flag("schemaOverlapAnswers")
        };
        if skip {
            parsed = parse(tool, command, args, true);
        }
    }
    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(failure) => return refuse_failure(tool, "schema", failure),
    };
    let words: Vec<String> = parsed
        .values("path")
        .iter()
        .filter_map(Value::as_str)
        .flat_map(str::split_whitespace)
        .map(ToString::to_string)
        .collect();
    if let Some(query) = parsed.text("search") {
        cache();
        let tree = patched(tool, "<tree>", super::guides::search_tree(tool, &model, query));
        return tool.success("schema", tree);
    }
    let registry: Vec<(String, Option<String>)> = ["errors", "error", "warnings", "warning"]
        .iter()
        .filter(|name| parsed.has(name))
        .map(|name| {
            (
                format!("--{name}"),
                parsed.text(name).map(ToString::to_string),
            )
        })
        .collect();
    if !registry.is_empty() {
        if registry.iter().all(|(name, _)| name == "--errors" || name == "--warnings") {
            cache();
        }
        return tool.schema(&registry);
    }
    if !words.is_empty() {
        let quoted = parsed
            .values("path")
            .iter()
            .filter_map(Value::as_str)
            .any(|word| word.contains(char::is_whitespace));
        return schema_path(tool, &model, &words, quoted, &cache);
    }
    // The L0 tests' older seeding of a tree of one or two entries: the unchanged answer of the L0 fake.
    if tool.config.get("schemaTree").is_some() {
        return tool.schema(&[]);
    }
    if tool.config.get("treeAnswer").and_then(Value::as_str) == Some("usage") {
        return tool.refuse(
            "USAGE",
            "schema takes --errors here",
            "schema",
            &[("field", json!("--errors"))],
        );
    }
    let tree = patched(tool, "<tree>", tree_doc(&model, tool.flag("treeGroupsLast"), &tool.text("treeLevel", if tool.flag("l2") { "L2" } else { "L1" })));
    cache();
    tool.success("schema", tree)
}

fn schema_path(tool: &Tool, model: &Model, words: &[String], quoted: bool, cache: &dyn Fn()) -> ExitCode {
    let mut current = String::new();
    for (index, word) in words.iter().enumerate() {
        let candidate = if current.is_empty() {
            word.clone()
        } else {
            format!("{current} {word}")
        };
        let known = model.command(&candidate).is_some() || model.group(&candidate).is_some();
        if !known || tool.lists("schemaUnknown", &candidate) {
            let expected: Vec<String> = model
                .children(&current)
                .iter()
                .map(|(id, _)| id.rsplit(' ').next().unwrap_or_default().to_string())
                .collect();
            let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
            if let Some(code) = tool.config.get("schemaUnknownCode").and_then(Value::as_str) {
                return tool.fail(
                    "usage",
                    code,
                    &format!("{word} is not a command"),
                    Some("schema"),
                    &[("field", json!("command")), ("expected", json!(expected))],
                    2,
                );
            }
            // Seeded: the refusal of an unknown path names another field than `command` (spec 10.1.2).
            if tool.flag("schemaUnknownField") {
                return tool.fail(
                    "usage",
                    "UNKNOWN_COMMAND",
                    &format!("{word} is not a command"),
                    Some("schema"),
                    &[("field", json!("PATH")), ("expected", json!(expected))],
                    2,
                );
            }
            return tool.unknown_command(word, &expected, Some("schema"));
        }
        current = candidate;
        if model.command(&current).is_some() && index + 1 < words.len() {
            return tool.refuse(
                "USAGE",
                &format!("{current} has no subcommand {}", words[index + 1]),
                "schema",
                &[("field", json!("ARGUMENT"))],
            );
        }
    }
    let mut doc = if let Some(command) = model.command(&current) {
        let mut doc = command_doc(&tool.name(), command);
        // Seeded: a tool whose documents carry no example (so no read command has an invocation to change into a pair).
        if tool.flag("noExamples") || (tool.flag("examplesOnlyServe") && current != "serve") {
            doc["examples"] = json!([]);
        }
        doc
    } else {
        group_doc(model, &current)
    };
    if quoted && tool.flag("quotedPathDiffers") {
        doc["summary"] = json!("another summary");
    }
    cache();
    tool.success("schema", patched(tool, &current, doc))
}

// ---- member ----

fn member(tool: &Tool, args: &[String], words: &[String]) -> ExitCode {
    let model = Model::of(tool);
    let verbs = ["list", "search", "tag"];
    match words.get(1).map(String::as_str) {
        None => {
            if tool.config.get("groupRuns").and_then(Value::as_str) == Some("member") {
                return tool.success("member", json!({"member": true}));
            }
            let mut expected = verbs.to_vec();
            if tool.flag("groupExpectedWrong") {
                expected.push("nothing");
            }
            tool.fail(
                "group",
                "COMMAND_REQUIRED",
                "member needs a command",
                Some("member"),
                &[("expected", json!(expected))],
                2,
            )
        }
        Some(verb) if verbs.contains(&verb) => {
            let id = format!("member {verb}");
            let Some(command) = model.command(&id) else {
                return tool.unknown_command(verb, &verbs, Some("member"));
            };
            match parse(tool, command, args, false) {
                Err(failure) => refuse_failure(tool, &id, failure),
                Ok(parsed) => {
                    if let Some(refused) = tool.seeded_refusal(&id) {
                        return refused;
                    }
                    match verb {
                        "list" => list(tool, args, &parsed),
                        "search" => search(tool, &parsed),
                        _ => tag(tool, &parsed),
                    }
                }
            }
        }
        Some(word) => tool.unknown_command(word, &verbs, Some("member")),
    }
}

/// One member of the directory.
struct Member {
    id: i64,
    name: &'static str,
    team: &'static str,
    age: i64,
    nickname: Option<&'static str>,
    profile: Option<(&'static str, &'static str)>,
    tags: &'static [&'static str],
    owners: &'static [(&'static str, &'static str)],
    attributes: &'static [(&'static str, &'static str)],
}

/// The directory: five members, four of them in the team `core`.
const MEMBERS: [Member; 5] = [
    Member {
        id: 1,
        name: "Cy",
        team: "core",
        age: 41,
        nickname: Some("cee"),
        profile: Some(("Cy Park", "cy@example.test")),
        tags: &["lead", "core"],
        owners: &[("Bo", "bo@example.test"), ("Di", "di@example.test")],
        attributes: &[("region", "eu"), ("http.status-code", "200")],
    },
    Member {
        id: 2,
        name: "Ada",
        team: "core",
        age: 29,
        nickname: Some("ace"),
        profile: None,
        tags: &[],
        owners: &[],
        attributes: &[],
    },
    Member {
        id: 3,
        name: "Di",
        team: "core",
        age: 35,
        nickname: Some("dee"),
        profile: Some(("Di Kim", "di@example.test")),
        tags: &["core"],
        owners: &[("Ada", "ada@example.test")],
        attributes: &[("region", "us")],
    },
    Member {
        id: 4,
        name: "Bo",
        team: "core",
        age: 52,
        nickname: None,
        profile: Some(("Bo Lee", "bo@example.test")),
        tags: &["ops"],
        owners: &[],
        attributes: &[("region", "eu")],
    },
    Member {
        id: 5,
        name: "Ed",
        team: "ops",
        age: 23,
        nickname: Some("ted"),
        profile: Some(("Ed Ito", "ed@example.test")),
        tags: &["ops", "lead"],
        owners: &[("Cy", "cy@example.test")],
        attributes: &[("region", "ap")],
    },
];

impl Member {
    fn record(&self) -> Value {
        let mut record = Map::new();
        record.insert("id".into(), json!(self.id));
        record.insert("name".into(), json!(self.name));
        if let Some(nickname) = self.nickname {
            record.insert("nickname".into(), json!(nickname));
        }
        record.insert(
            "profile".into(),
            self.profile.map_or(Value::Null, |(name, email)| {
                json!({"name": name, "email": email, "timezone": "UTC"})
            }),
        );
        record.insert("tags".into(), json!(self.tags));
        record.insert(
            "owners".into(),
            json!(self
                .owners
                .iter()
                .map(|(name, email)| json!({"name": name, "email": email}))
                .collect::<Vec<_>>()),
        );
        let attributes: Map<String, Value> = self
            .attributes
            .iter()
            .map(|(key, value)| ((*key).to_string(), json!(value)))
            .collect();
        record.insert("attributes".into(), Value::Object(attributes));
        Value::Object(record)
    }
}

/// The hash a cursor carries of the arguments it is bound to (FNV-1a, stable across runs).
fn binding_hash(binding: &str) -> u32 {
    binding.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

fn encode_cursor(offset: usize, binding: &str) -> String {
    format!("m1.{offset}.{:08x}", binding_hash(binding))
}

/// The offset a cursor stands for, or the refusal for it.
fn decode_cursor(tool: &Tool, cursor: &str, binding: &str, total: usize) -> Result<usize, ExitCode> {
    let parts: Vec<&str> = cursor.split('.').collect();
    let offset = match parts.as_slice() {
        ["m1", offset, hash] if u32::from_str_radix(hash, 16).is_ok() => {
            offset.parse::<usize>().ok().map(|offset| (offset, *hash))
        }
        _ => None,
    };
    let refuse = |message: &str, code: &str| {
        tool.refuse(
            code,
            message,
            "member list",
            &[("field", json!("--cursor")), ("received", json!(cursor))],
        )
    };
    let Some((offset, hash)) = offset.filter(|(offset, _)| *offset <= total) else {
        return Err(refuse("that is not a cursor of this command", "CURSOR_INVALID"));
    };
    if hash != format!("{:08x}", binding_hash(binding)) && !tool.flag("cursorIgnoresFilter") {
        let code = if tool.flag("cursorFilterRefusedAsUsage") {
            "USAGE"
        } else {
            "CURSOR_INVALID"
        };
        return Err(refuse("that cursor was made with other arguments", code));
    }
    Ok(offset)
}

/// `argv` of the next page: the invocation again, with the cursor of the next page.
fn next_page_argv(args: &[String], cursor: &str, document: &[String], keep_input: bool) -> Vec<String> {
    let mut argv = Vec::new();
    let mut words = args.iter().take_while(|arg| arg.as_str() != "--");
    while let Some(word) = words.next() {
        if word == "--json" {
            continue;
        }
        if word == "--cursor" || (word == "--input" && !keep_input) {
            words.next();
            continue;
        }
        if word.starts_with("--cursor=") || (word.starts_with("--input=") && !keep_input) {
            continue;
        }
        argv.push(word.clone());
    }
    // What an `--input` document gave is written out as options: the next page needs no document.
    argv.extend(document.iter().cloned());
    argv.push("--cursor".to_string());
    argv.push(cursor.to_string());
    argv.push("--json".to_string());
    argv
}

/// The display string of an invocation: the tool name and the words, quoted as 6.8 asks.
fn display(tool: &Tool, argv: &[String]) -> String {
    std::iter::once(tool.name())
        .chain(argv.iter().map(|word| atip::quote(word)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The paths a `--fields` value may select: the declared ones and, in 1.1, the keys of a declared map.
fn selectable(tool: &Tool, declared: &[String], maps: &[&str], selected: &str) -> bool {
    if declared.iter().any(|path| path == selected) {
        return true;
    }
    let Some(steps) = project::parse_path(selected) else {
        return false;
    };
    if !tool.is_1_1() || steps.len() < 2 || steps.last().is_some_and(|step| step.array) {
        return false;
    }
    let parent = project::render(&steps[..steps.len() - 1]);
    maps.contains(&parent.as_str())
}

/// The kept paths and their trie: the selection of `--fields` plus the `keep` paths, or the refusal.
fn projection(tool: &Tool, value: &str, doc: &Value) -> Result<(Vec<String>, Vec<String>), ExitCode> {
    let fields = &doc["outputs"][0]["fields"];
    let strings = |value: &Value| -> Vec<String> {
        value
            .as_array()
            .map(|items| items.iter().filter_map(Value::as_str).map(ToString::to_string).collect())
            .unwrap_or_default()
    };
    let declared = strings(&fields["paths"]);
    let keep = strings(&fields["keep"]);
    let mut selection: Vec<String> = Vec::new();
    for path in project::split_list(value) {
        if !selectable(tool, &declared, &["attributes"], &path) {
            if tool.flag("fieldsAcceptsUnknown") || (tool.flag("fieldsUnknownAcceptedInText") && !tool.json) {
                continue;
            }
            return Err(tool.refuse(
                "USAGE",
                &format!("{path} is not a field of the members"),
                "member list",
                &[
                    ("field", json!("--fields")),
                    ("expected", json!(declared)),
                    ("received", json!(path)),
                ],
            ));
        }
        if !selection.contains(&path) {
            selection.push(path);
        }
    }
    Ok((selection, keep))
}

fn list(tool: &Tool, args: &[String], parsed: &Parsed) -> ExitCode {
    let model = Model::of(tool);
    let Some(command) = model.command("member list") else {
        return ExitCode::from(70);
    };
    let doc = command_doc(&tool.name(), command);
    if tool.flag("ghostTeamUsage") && parsed.text("team") == Some("ghost") {
        // The seeded violation: a refusal the model does not declare, for a value its documents accept.
        return tool.refuse("USAGE", "there is no such team", "member list", &[("field", json!("--team"))]);
    }
    if tool.flag("inputRefused") && parsed.has("input") {
        return tool.refuse("INPUT_INVALID", "the document is not read", "member list", &[("field", json!("--input"))]);
    }
    let env_team = std::env::var("FAKE_TEAM").ok().filter(|_| tool.is_1_2());
    let team_text = parsed.text("team").map(ToString::to_string).or_else(|| env_team.clone());
    let team = team_text.as_deref();
    let sort = parsed.text("sort").unwrap_or("name");
    if tool.flag("fieldsNeedsJson") && !tool.json && parsed.has("fields") {
        // The seeded violation of 1.2: `--fields` is refused without `--json`.
        return tool.refuse("USAGE", "--fields needs --json", "member list", &[("field", json!("--fields"))]);
    }
    let mut members: Vec<&Member> = MEMBERS
        .iter()
        .filter(|member| team.is_none_or(|team| member.team == team))
        .collect();
    if sort == "name" {
        members.sort_by_key(|member| member.name);
    } else {
        members.sort_by_key(|member| member.id);
    }
    // A cursor is bound to the arguments the invocation gave (a default written out is another invocation) and, for
    // an option it left out, to the value its environment variable has (spec 6.9, 1.2); before 1.2, to the effective values.
    let binding = if tool.is_1_2() && !tool.flag("cursorBindsEffective") {
        let env = if tool.flag("cursorIgnoresEnv") || parsed.has("team") { String::new() } else { env_team.clone().unwrap_or_default() };
        let kinds: Vec<&str> = parsed.values("kind").iter().filter_map(Value::as_str).collect();
        format!("team={};sort={};env={env};kind={}", parsed.text("team").unwrap_or_default(), parsed.text("sort").unwrap_or_default(), kinds.join(","))
    } else {
        format!("team={};sort={sort}", team.unwrap_or_default())
    };
    if tool.flag("cursorRefusesAll") && parsed.has("all") && parsed.has("cursor") {
        // The seeded violation of 1.2: a cursor made under `--limit` is refused when `--all` replaces it.
        return tool.refuse("CURSOR_INVALID", "that cursor was made with --limit", "member list", &[("field", json!("--cursor"))]);
    }
    let offset = match parsed.text("cursor") {
        None => 0,
        Some(cursor) => match decode_cursor(tool, cursor, &binding, members.len()) {
            Ok(offset) => offset,
            Err(refusal) => return refusal,
        },
    };
    let remaining = members.len() - offset;
    let size = if parsed.has("all") {
        remaining
    } else {
        let limit = usize::try_from(parsed.int("limit").unwrap_or(50)).unwrap_or(usize::MAX);
        let limit = if tool.flag("limitIgnored") { limit.saturating_add(1) } else { limit };
        limit.min(remaining)
    };
    let page: Vec<Value> = members[offset..offset + size].iter().map(|member| member.record()).collect();
    let more = offset + size < members.len();
    let mut meta = Map::new();
    let mut page_meta = json!({"returned": page.len(), "total": members.len()});
    if more {
        let next_offset = if tool.flag("cursorRepeats") { offset } else { offset + size };
        let cursor = encode_cursor(next_offset, &binding);
        page_meta["nextCursor"] = json!(cursor);
        // The arguments an `--input` document gave, as options (and, seeded, a default written out beside them).
        let mut document: Vec<String> = Vec::new();
        if parsed.has("input") {
            for name in ["team", "sort"] {
                let written = args.iter().any(|word| word == &format!("--{name}") || word.starts_with(&format!("--{name}=")));
                if let (Some(value), false) = (parsed.text(name), written) {
                    document.extend([format!("--{name}"), value.to_string()]);
                }
            }
            if tool.flag("nextCarriesDefault") && !document.iter().any(|word| word == "--sort") {
                document.extend(["--sort".to_string(), "name".to_string()]);
            }
        }
        let argv = next_page_argv(args, &cursor, &document, tool.flag("nextKeepsInput"));
        meta.insert(
            "next".into(),
            json!([{"command": display(tool, &argv), "argv": argv, "reason": "the next page of members"}]),
        );
    }
    meta.insert("page".into(), page_meta);
    let mut records = page;
    if let Some(value) = parsed.text("fields") {
        let (selection, keep) = match projection(tool, value, &doc) {
            Ok(projection) => projection,
            Err(refusal) => return refusal,
        };
        let kept: Vec<Vec<project::Step>> = selection
            .iter()
            .chain(keep.iter())
            .filter_map(|path| project::parse_path(path))
            .collect();
        let trie = project::trie(&kept);
        let originals = records.clone();
        records = originals
            .iter()
            .map(|record| {
                let mut projected = project::project(record, &trie);
                seed_projection(tool, &selection, record, &mut projected);
                projected
            })
            .collect();
        let mut listed = selection;
        if !tool.flag("fieldsMetaOmitsKeep") {
            listed.extend(keep);
        }
        meta.insert("fields".into(), json!(listed));
    }
    tool.success_with("member list", json!({"members": records}), meta)
}

/// The seeded violations of a projection: what it leaves, drops or breaks.
fn seed_projection(tool: &Tool, selection: &[String], record: &Value, projected: &mut Value) {
    if tool.flag("fieldsKeepsUnselected") {
        if let (Some(profile), Some(original)) = (projected.get_mut("profile"), record.get("profile")) {
            if profile.is_object() {
                *profile = original.clone();
            }
        }
    }
    if tool.flag("fieldsDropsSelected") && selection.iter().any(|path| path == "nickname") {
        if let Some(members) = projected.as_object_mut() {
            members.remove("nickname");
        }
    }
    if tool.flag("fieldsBreaksSchema") && selection.iter().any(|path| path == "tags") {
        if let Some(members) = projected.as_object_mut() {
            members.insert("tags".into(), json!("not a list"));
        }
    }
}

fn search(tool: &Tool, parsed: &Parsed) -> ExitCode {
    let query = parsed.text("query").map(str::to_lowercase);
    let tags: Vec<&str> = parsed.values("tag").iter().filter_map(Value::as_str).collect();
    let filter_team = parsed
        .values("filter")
        .first()
        .and_then(|filter| filter.get("team"))
        .and_then(Value::as_str);
    if filter_team == Some("ghost") && tool.flag("ghostFilterCrash") {
        // The seeded violation: the run ends with exit 1 and prints no envelope at all.
        eprintln!("fake: the team ghost crashed the search");
        return ExitCode::from(1);
    }
    let min_score = parsed.int("min-score").unwrap_or(0);
    let mut matches: Vec<(i64, &Member)> = MEMBERS
        .iter()
        .enumerate()
        .map(|(rank, member)| (100 - 10 * i64::try_from(rank).unwrap_or(0), member))
        .filter(|(score, member)| {
            *score >= min_score
                && query.as_ref().is_none_or(|query| {
                    let name = member.name.to_lowercase();
                    if parsed.has("exact") {
                        name == *query
                    } else if parsed.has("prefix") {
                        name.starts_with(query.as_str())
                    } else {
                        name.contains(query.as_str())
                    }
                })
                && tags.iter().all(|tag| member.tags.contains(tag))
                && filter_team.is_none_or(|team| member.team == team)
        })
        .collect();
    match parsed.text("sort").unwrap_or("name") {
        "score" => matches.sort_by_key(|(score, _)| -score),
        "age" => matches.sort_by_key(|(_, member)| member.age),
        _ => matches.sort_by_key(|(_, member)| member.name),
    }
    let mut meta = Map::new();
    let data = if parsed.has("total") {
        meta.insert("output".into(), json!("MemberTotalReport"));
        json!({"count": matches.len()})
    } else {
        meta.insert("output".into(), json!("MemberSearchReport"));
        json!({
            "count": matches.len(),
            "matches": matches
                .iter()
                .map(|(score, member)| json!({"id": member.id, "name": member.name, "score": score}))
                .collect::<Vec<_>>(),
        })
    };
    tool.success_with("member search", data, meta)
}

/// A refusal of an item of a command: what [`Tool::refuse`] prints, or what a failed item of a batch reports.
pub struct ItemError {
    code: &'static str,
    message: String,
    extras: Vec<(&'static str, Value)>,
}

fn tag(tool: &Tool, parsed: &Parsed) -> ExitCode {
    match tag_item(tool, parsed) {
        Ok(data) => {
            let mut meta = Map::new();
            if tool.flag("l2") {
                meta.insert("output".into(), json!("MemberTagReport"));
            }
            tool.success_with("member tag", data, meta)
        }
        Err(error) => tool.refuse(error.code, &error.message, "member tag", &error.extras),
    }
}

/// One `member tag`: the data it reports, or the refusal.
fn tag_item(tool: &Tool, parsed: &Parsed) -> Result<Value, ItemError> {
    let id = parsed.int("member").unwrap_or_default();
    if !MEMBERS.iter().any(|member| member.id == id) {
        // The hint is a runnable command line; the seeded violation names a command the tool does not have.
        let listing = if tool.flag("hintNoCommand") { "member lst" } else { "member list" };
        return Err(ItemError {
            code: "MEMBER_NOT_FOUND",
            message: format!("there is no member {id}"),
            extras: vec![
                ("field", json!("MEMBER")),
                ("received", json!(id.to_string())),
                ("hint", json!(format!("{} {listing} --json", tool.name()))),
            ],
        });
    }
    let path = std::path::Path::new("tags").join(format!("{id}.json"));
    let before: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({"member": id, "tags": []}));
    let mut tags: Vec<String> = before["tags"]
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).map(ToString::to_string).collect())
        .unwrap_or_default();
    tags.extend(parsed.values("tag").iter().filter_map(Value::as_str).map(ToString::to_string));
    tags.sort();
    tags.dedup();
    let mut after = json!({"member": id, "tags": tags});
    let note = parsed
        .text("note")
        .map(ToString::to_string)
        .or_else(|| {
            before["note"]
                .as_str()
                .filter(|_| !parsed.has("clear-note"))
                .map(ToString::to_string)
        });
    if let Some(note) = note {
        after["note"] = json!(note);
    }
    let status = if after == before { "unchanged" } else { "written" };
    if status == "written" {
        std::fs::create_dir_all("tags").unwrap_or_default();
        std::fs::write(&path, after.to_string()).unwrap_or_default();
    }
    after["status"] = json!(status);
    Ok(after)
}

// ---- record ----

fn record(tool: &Tool, args: &[String], words: &[String]) -> ExitCode {
    let model = Model::of(tool);
    match words.get(1).map(String::as_str) {
        None => tool.fail("group", "COMMAND_REQUIRED", "record needs a command", Some("record"), &[("expected", json!(["put"]))], 2),
        Some("put") => {
            let Some(command) = model.command("record put") else {
                return tool.unknown_command("put", &["put"], Some("record"));
            };
            match parse(tool, command, args, false) {
                Err(failure) => refuse_failure(tool, "record put", failure),
                Ok(parsed) => {
                    if let Some(refused) = tool.seeded_refusal("record put") {
                        return refused;
                    }
                    put(tool, &parsed)
                }
            }
        }
        Some(word) => tool.unknown_command(word, &["put"], Some("record")),
    }
}

/// `record put <ID> --round <N> ...`: writes `records/<ID>.json`; the same record again is `unchanged`.
fn put(tool: &Tool, parsed: &Parsed) -> ExitCode {
    let id = parsed.int("id").unwrap_or_default();
    // Seeded: a label that starts like an option, an inline document or a file is looked up in the state and refused by a precondition (as a revision
    // that is not in the repository is). With `dashPreconditionNamed` the refusal names the argument and the value it received.
    if tool.flag("dashPrecondition") {
        let both = tool.flag("dashPreconditionBoth");
        let mut given = parsed.text("label").map(|label| ("/label", label)).into_iter().chain(parsed.text("note").filter(|_| both).map(|note| ("/note", note)));
        if let Some((pointer, text)) = given.find(|(_, text)| text.starts_with(['-', '{', '['])) {
            let field = tool.text("dashPreconditionField", pointer);
            let extras: Vec<(&str, Value)> = if tool.flag("dashPreconditionNamed") { vec![("field", json!(field)), ("received", json!(text))] } else { Vec::new() };
            let code = tool.text("dashPreconditionCode", "ALREADY_SEALED");
            return tool.fail("usage", &code, "the value names something that is not there", Some("record put"), &extras, 2);
        }
    }
    let mut record = json!({"id": id, "round": parsed.int("round").unwrap_or_default()});
    for (key, value) in [("label", parsed.text("label").map(|text| json!(text))), ("weight", parsed.values("weight").first().cloned()), ("score", parsed.values("score").first().cloned()), ("meta", parsed.values("meta").first().cloned())] {
        if let Some(value) = value {
            record[key] = value;
        }
    }
    if let Some(note) = parsed.text("note").filter(|_| !parsed.has("clear-note")) {
        record["note"] = json!(note);
    }
    let path = std::path::Path::new("records").join(format!("{id}.json"));
    let before: Option<Value> = std::fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str(&text).ok());
    let status = if before.as_ref() == Some(&record) { "unchanged" } else { "written" };
    if status == "written" {
        std::fs::create_dir_all("records").unwrap_or_default();
        std::fs::write(&path, record.to_string()).unwrap_or_default();
    }
    record["status"] = json!(status);
    if tool.flag("replayStatusWritten") && status == "unchanged" {
        // The seeded violation: a repeat is reported as new work, with a result that differs from the first run's.
        record["status"] = json!("written");
        record["label"] = json!(format!("run-{}", std::process::id()));
    }
    if tool.flag("dataFormDiffers") && std::env::var_os("FAKE_FROM_DOCUMENT").is_some() {
        // Seeded: the document form prints another label than the argv form, in `data` alone: the same exit, `ok` and code, and a value that is the same
        // on every run of the argv form (so it is not a varying path).
        record["label"] = json!(format!("{}-from-document", record["label"].as_str().unwrap_or("")));
    }
    if tool.flag("dataStamp") {
        // Seeded: a value that differs on every run, as a timestamp does (the label carries the nanoseconds of the run).
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_nanos());
        record["label"] = json!(format!("{}@{nanos}", record["label"].as_str().unwrap_or("")));
    }
    tool.success_with("record put", record, Map::new())
}

// ---- batches ----

/// Executes a batch whose preflight passed (spec 11.11): the items one at a time, in order, stopping at the first that fails.
pub fn execute_batch(tool: &Tool, id: &'static str, items: Vec<Parsed>) -> ExitCode {
    let mut report: Vec<Value> = Vec::new();
    let mut failed: Option<(usize, ItemError)> = None;
    for (index, parsed) in items.iter().enumerate() {
        if failed.is_some() {
            if tool.flag("batchStatusWrong") {
                // The seeded violation: an item that did not run is reported as written: a valid item, the wrong status.
                report.push(json!({"index": index, "status": "unchanged", "data": {}}));
            } else {
                report.push(json!({"index": index, "status": "skipped"}));
            }
            continue;
        }
        let result = if parsed.has("__late__") {
            Err(ItemError { code: "USAGE", message: "the item is not valid".to_string(), extras: vec![("field", json!(format!("/{index}")))] })
        } else {
            match id {
                "member tag" => tag_item(tool, parsed),
                _ => Err(ItemError { code: "USAGE", message: format!("{id} takes no batch"), extras: Vec::new() }),
            }
        };
        match result {
            Ok(data) => {
                let status = data["status"].as_str().unwrap_or("written").to_string();
                report.push(json!({"index": index, "status": status, "data": data}));
            }
            Err(error) => {
                let object = json!({"code": error.code, "message": error.message, "retryable": tool.flag("batchRetryableWrong"), "field": format!("/{index}/operands/0")});
                report.push(json!({"index": index, "status": "failed", "error": object}));
                failed = Some((index, error));
            }
        }
    }
    let mut meta = tool.meta(Some(id));
    let failing = failed.is_some() || tool.flag("batchFailsOnSuccess");
    if !(tool.flag("batchNoOutput") || tool.flag(if failing { "batchNoOutputOnFailure" } else { "batchNoOutputOnSuccess" })) {
        let output = if id == "member tag" { "MemberTagBatchReport" } else { "BatchReport" };
        meta.insert("output".into(), json!(output));
    }
    if tool.flag("batchItemsShort") {
        report.retain(|item| item["status"] != "skipped");
    }
    let failed = failed.or_else(|| {
        // The seeded violation: a batch whose items all ran is reported as stopped at its last item.
        tool.flag("batchFailsOnSuccess").then(|| (items.len().saturating_sub(1), ItemError { code: "INTERNAL", message: "the batch stopped".to_string(), extras: Vec::new() }))
    });
    let Some((index, error)) = failed else {
        if tool.flag("batchOkSingleReport") {
            return tool.success(id, json!({"items": report.len()}));
        }
        if tool.flag("batchOkItemsShort") {
            report.pop();
        }
        if tool.flag("batchOkStatusOdd") {
            if let Some(item) = report.first_mut() {
                *item = json!({"index": 0, "status": "skipped"});
            }
        }
        if tool.flag("batchOkReportBadShape") {
            if let Some(item) = report.first_mut() {
                item["note"] = json!("extra");
            }
        }
        tool.print_document(&json!({"ok": true, "data": {"items": report}, "meta": meta}));
        return ExitCode::SUCCESS;
    };
    if index == 0 && tool.flag("batchFirstFailPlain") {
        return tool.refuse(error.code, &error.message, id, &error.extras);
    }
    let field = if tool.flag("batchWrongField") { "--input".to_string() } else { format!("/{index}") };
    if tool.flag("batchReportBadShape") {
        if let Some(item) = report.first_mut() {
            item["note"] = json!("extra");
        }
    }
    let mut document = json!({
        "ok": false,
        "error": {"code": "BATCH_INCOMPLETE", "message": format!("item {index} of {} failed ({})", items.len(), error.code), "retryable": false, "field": field},
        "data": {"items": report},
        "meta": meta,
    });
    if tool.flag("batchNoData") {
        document.as_object_mut().map(|object| object.remove("data"));
    }
    tool.print_document(&document);
    ExitCode::from(1)
}
