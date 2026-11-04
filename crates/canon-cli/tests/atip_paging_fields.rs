//! Paging and field selection of atip/1 1.1 (`canon-0-34-atip-paging-fields`, scenarios `atip.paging.*`),
//! against the built binary: a cursor bound to the arguments of the call that issued it, its reasons, `--fields`
//! as the contract's projection (spec 6.9, 6.10, 7.2.3, 10.9, 10.13 rules 11 and 15), the declarations the
//! command documents publish, and the projected answers held to the published schemas.
//!
//! Tests are named by scenario id (`atip_paging_01_…` proves `atip.paging.01`).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

use atip::fields::{FieldPath, Rules};
use serde_json::{json, Value};
use tempfile::TempDir;

/// The rules of the minor canon declares (`canon_cli::json_out::DECLARED_MINOR`).
fn declared_rules() -> Rules {
    Rules::for_minor(canon_cli::json_out::DECLARED_MINOR).expect("canon declares a minor the atip crate knows")
}

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-paging").output().expect("spawn canon binary")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn ok(dir: &Path, args: &[&str]) -> Value {
    let out = canon(dir, args);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.stdout.is_empty() { Value::Null } else { serde_json::from_slice(&out.stdout).unwrap_or(Value::Null) }
}

/// A repo with three subjects, a change with three findings, a feature with three scenarios, two of them divergent. Committed.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    for subject in ["demo", "demo-two", "demo-three"] {
        ok(repo, &["subject", "new", subject, "--domain", "dev", "--title", &format!("Title {subject}")]);
    }
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    ok(repo, &["change", "new", "add-logout", "--subject", "demo-two", "--title", "Add logout"]);
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    for n in 1..=3 {
        ok(repo, &["scenario", "new", &format!("auth.login.0{n}"), "--title", &format!("Case {n}"), "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    }
    ok(repo, &["inventory", "sync"]);
    for args in [&["init", "-q"][..], &["add", "-A"], &["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "fixture"]] {
        assert!(Command::new("git").args(args).current_dir(repo).status().unwrap().success());
    }
    for n in 1..=3 {
        ok(repo, &["finding", "add", "--change", "add-login", "--round", "1", "--seq", &n.to_string(), "--severity", "should-fix", "--introduced-unknown", "--actor", "unit-1", "--summary", &format!("Finding {n}")]);
    }
    for n in 1..=2 {
        ok(repo, &["divergence", "add", &format!("auth.login.0{n}"), "--project", "root", "--actor", "alice", "--role", "reviewer"]);
    }
    dir
}

/// Every file under `dir` (but `.git`) with its bytes: what a refused call must leave as it found it.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(at).unwrap().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().display().to_string(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn argv_of(step: &Value) -> Vec<String> {
    step["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap().to_string()).collect()
}

fn run_argv(dir: &Path, argv: &[String]) -> Output {
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    canon(dir, &words)
}

/// The step of `meta.next` that is the next page: the call with `--cursor`.
fn next_page(document: &Value) -> Option<Vec<String>> {
    document["meta"]["next"].as_array()?.iter().map(argv_of).find(|argv| argv.iter().any(|word| word.starts_with("--cursor=")))
}

fn error_of(output: &Output) -> Value {
    assert_eq!(output.status.code(), Some(2), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    json(output)["error"].clone()
}

/// The first page of each paged command: `(name, call without paging options, how to read the records)`.
fn listings() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("query", vec!["query", "--kind", "subject", "--json"]),
        ("finding list", vec!["finding", "list", "--change", "add-login", "--json"]),
        ("divergence list", vec!["divergence", "list", "--json"]),
        ("export", vec!["export", "--kind", "subject", "--json"]),
    ]
}

/// The first page of `args` with `--limit 1` and the cursor it issued.
fn first_page(dir: &Path, args: &[&str]) -> (Value, String) {
    let mut call = args.to_vec();
    call.extend(["--limit", "1"]);
    let document = json(&canon(dir, &call));
    let cursor = document["meta"]["page"]["nextCursor"].as_str().unwrap_or_else(|| panic!("{args:?} has a second page: {document}")).to_string();
    (document, cursor)
}

/// The command document of `command` as canon declares it (the model: every member, `schemas` inside).
fn command_doc(_dir: &Path, command: &str) -> Value {
    canon_cli::spec::model()["commands"].as_array().unwrap().iter().find(|doc| doc["command"] == command).unwrap_or_else(|| panic!("`{command}` is a command of the model")).clone()
}

/// Where the members of `data` that `--fields` cuts are, per command: the `records` pointer `outputs[].fields` declares.
fn fields_output(doc: &Value) -> Value {
    doc["outputs"].as_array().unwrap().iter().find(|output| output.get("fields").is_some()).unwrap_or_else(|| panic!("no output declares fields: {}", doc["command"])).clone()
}

const SIX: [&str; 6] = ["finding list", "divergence list", "query", "export", "status", "context"];

#[test]
fn atip_paging_01_a_paged_listing_reports_its_page_and_the_next_call_reads_the_next_page() {
    let repo = repo();
    let mut seen = Vec::new();
    let mut call: Vec<String> = ["query", "--kind", "subject", "--limit", "1", "--json"].map(String::from).to_vec();
    for round in 0.. {
        let document = json(&run_argv(repo.path(), &call));
        assert_eq!(document["ok"], true, "{document}");
        assert_eq!(document["meta"]["page"]["returned"], 1);
        assert_eq!(document["meta"]["page"]["total"], 3);
        seen.push(document["data"]["records"][0]["subject_id"].as_str().unwrap().to_string());
        match next_page(&document) {
            Some(next) => {
                assert_eq!(document["meta"]["page"]["nextCursor"].as_str(), next.iter().find_map(|word| word.strip_prefix("--cursor=")), "`meta.next` names the cursor `meta.page` issued");
                assert_eq!(next[0], "query");
                assert!(next.iter().any(|word| word == "--json"), "{next:?}");
                call = next;
            }
            None => {
                assert_eq!(round, 2, "three records, one per page");
                assert!(document["meta"]["page"].get("nextCursor").is_none(), "the last page has no cursor: {document}");
                break;
            }
        }
    }
    seen.sort();
    assert_eq!(seen, ["demo", "demo-three", "demo-two"], "every record once");
    // The report keeps no paging member of its own: `meta.page` is the contract's.
    let first = json(&canon(repo.path(), &["query", "--kind", "subject", "--limit", "1", "--json"]));
    for member in ["page", "nextCursor", "returned", "hasMore", "meta"] {
        assert!(first["data"].get(member).is_none(), "data has no `{member}`: {first}");
    }
}

#[test]
fn atip_paging_02_a_page_size_below_one_and_a_limit_with_all_are_usage_naming_the_limit() {
    let repo = repo();
    for (name, args) in listings() {
        let mut zero = args.clone();
        zero.extend(["--limit", "0"]);
        let error = error_of(&canon(repo.path(), &zero));
        assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("USAGE"), Some("--limit")), "{name}: {error}");
        let mut both = args.clone();
        both.extend(["--limit", "1", "--all"]);
        let error = error_of(&canon(repo.path(), &both));
        assert_eq!(error["code"], "USAGE", "{name}: {error}");
        assert!(["--limit", "--all"].contains(&error["field"].as_str().unwrap()), "{name}: {error}");
    }
}

#[test]
fn atip_paging_03_a_cursor_given_other_arguments_is_refused_arguments_changed() {
    let repo = repo();
    let changed: Vec<(&str, Vec<&str>, Vec<&str>)> = vec![
        ("query --kind", vec!["query", "--kind", "subject", "--json"], vec!["query", "--kind", "change", "--json"]),
        ("query --since", vec!["query", "--kind", "subject", "--json"], vec!["query", "--kind", "subject", "--since", "2020-01-01", "--json"]),
        ("query --domain", vec!["query", "--kind", "subject", "--json"], vec!["query", "--kind", "subject", "--domain", "dev", "--json"]),
        ("finding list --change", vec!["finding", "list", "--change", "add-login", "--json"], vec!["finding", "list", "--change", "add-logout", "--json"]),
        ("finding list --open", vec!["finding", "list", "--change", "add-login", "--json"], vec!["finding", "list", "--change", "add-login", "--open", "--json"]),
        ("divergence list --as-of", vec!["divergence", "list", "--json"], vec!["divergence", "list", "--as-of", "2030-01-01", "--json"]),
        ("export --kind", vec!["export", "--kind", "subject", "--json"], vec!["export", "--kind", "change", "--json"]),
        ("export --before", vec!["export", "--kind", "subject", "--json"], vec!["export", "--kind", "subject", "--before", "2099-01-01", "--json"]),
        ("a flag", vec!["query", "--kind", "subject", "--json"], vec!["query", "--kind", "subject", "--history", "--json"]),
    ];
    for (what, issued, called) in changed {
        let (_, cursor) = first_page(repo.path(), &issued);
        let mut call = called.clone();
        call.extend(["--limit", "1", "--cursor", &cursor]);
        let before = snapshot(repo.path());
        let error = error_of(&canon(repo.path(), &call));
        assert_eq!((error["code"].as_str(), error["reason"].as_str(), error["field"].as_str()), (Some("CURSOR_INVALID"), Some("ARGUMENTS_CHANGED"), Some("--cursor")), "{what}: {error}");
        assert_eq!(snapshot(repo.path()), before, "{what}: a refusal writes nothing");
    }
    // The same arguments in another order and spelling are the same call.
    let (_, cursor) = first_page(repo.path(), &["query", "--kind", "subject", "--since", "2020-01-01", "--json"]);
    let same = canon(repo.path(), &["query", "--json", "--since=2020-01-01", "--kind=subject", "--limit", "1", "--cursor", &cursor]);
    assert!(same.status.success(), "{}", String::from_utf8_lossy(&same.stdout));
}

#[test]
fn atip_paging_04_a_cursor_of_another_command_or_no_cursor_is_foreign_a_gone_record_is_expired_and_the_reasons_are_registered() {
    let repo = repo();
    let (_, finding_cursor) = first_page(repo.path(), &["finding", "list", "--change", "add-login", "--json"]);
    let before = snapshot(repo.path());
    let foreign: Vec<(&str, Vec<&str>)> = vec![
        ("another command's cursor", vec!["query", "--kind", "subject", "--json", "--cursor", &finding_cursor]),
        ("an arbitrary string", vec!["query", "--kind", "subject", "--json", "--cursor", "not-a-cursor"]),
        ("hex that is not a cursor", vec!["finding", "list", "--change", "add-login", "--json", "--cursor", "0a0b"]),
        ("the empty string", vec!["divergence", "list", "--json", "--cursor", ""]),
        ("a 0.33 cursor, which has no binding", vec!["export", "--kind", "subject", "--json", "--cursor", "6578706f72743a7375626a656374" /* "export:subject" */]),
    ];
    for (what, args) in foreign {
        let error = error_of(&canon(repo.path(), &args));
        assert_eq!((error["code"].as_str(), error["reason"].as_str(), error["field"].as_str()), (Some("CURSOR_INVALID"), Some("FOREIGN"), Some("--cursor")), "{what}: {error}");
    }
    assert_eq!(snapshot(repo.path()), before, "a refusal writes nothing");

    // The record the cursor points after is gone: the same command, the same arguments, `EXPIRED`.
    let ledger = repo.path().join(".canon/ledger/kind=finding");
    let first = std::fs::read_dir(&ledger).unwrap().flatten().map(|entry| entry.path()).find(|path| path.file_name().unwrap().to_string_lossy().starts_with("add-login_1_1_")).expect("the first finding's file");
    std::fs::remove_file(first).unwrap();
    let before = snapshot(repo.path());
    let error = error_of(&canon(repo.path(), &["finding", "list", "--change", "add-login", "--json", "--limit", "1", "--cursor", &finding_cursor]));
    assert_eq!((error["code"].as_str(), error["reason"].as_str(), error["field"].as_str()), (Some("CURSOR_INVALID"), Some("EXPIRED"), Some("--cursor")), "{error}");
    assert_eq!(snapshot(repo.path()), before, "a refusal writes nothing");

    // Every reason is registered under the code, and nothing else is.
    let registry = json(&canon(repo.path(), &["schema", "--error", "CURSOR_INVALID", "--json"]));
    let reasons: Vec<&str> = registry["data"]["error"]["reasons"].as_array().unwrap().iter().map(|reason| reason["reason"].as_str().unwrap()).collect();
    assert_eq!(reasons, ["ARGUMENTS_CHANGED", "EXPIRED", "FOREIGN"]);
    for reason in registry["data"]["error"]["reasons"].as_array().unwrap() {
        assert!(reason["meaning"].as_str().is_some_and(|text| !text.is_empty()) && reason["fix"].as_str().is_some_and(|text| !text.is_empty()), "{reason}");
    }
    assert!(atip::registry_entry_problems(&registry["data"]["error"]).is_empty(), "the entry keeps the contract's schema");
}

#[test]
fn atip_paging_05_a_changed_page_size_or_field_selection_continues_at_the_same_place() {
    let repo = repo();
    let all = json(&canon(repo.path(), &["finding", "list", "--change", "add-login", "--json"]));
    let rows = all["data"]["findings"].as_array().unwrap().clone();
    let (_, cursor) = first_page(repo.path(), &["finding", "list", "--change", "add-login", "--json"]);
    let wider = json(&canon(repo.path(), &["finding", "list", "--change", "add-login", "--json", "--limit", "2", "--cursor", &cursor]));
    assert_eq!(wider["data"]["findings"], json!([rows[1], rows[2]]), "--limit changes the page, not the place");
    let rest = json(&canon(repo.path(), &["finding", "list", "--change", "add-login", "--json", "--all", "--cursor", &cursor]));
    assert_eq!(rest["data"]["findings"], json!([rows[1], rows[2]]), "--all reads on from the cursor");
    let projected = json(&canon(repo.path(), &["finding", "list", "--change", "add-login", "--json", "--limit", "1", "--fields", "summary", "--cursor", &cursor]));
    assert_eq!(projected["ok"], true, "{projected}");
    assert_eq!(projected["data"]["findings"][0], json!({ "round": 1, "seq": 2, "summary": "Finding 2" }), "--fields changes what is shown, not the place");
    // The next page of a call with --fields keeps --fields, and its cursor is the same kind of cursor.
    let next = next_page(&projected).expect("a third finding follows");
    assert!(next.iter().any(|word| word.starts_with("--fields=")), "{next:?}");
    let third = json(&run_argv(repo.path(), &next));
    assert_eq!(third["data"]["findings"], json!([{ "round": 1, "seq": 3, "summary": "Finding 3" }]));
    assert!(third["meta"]["page"].get("nextCursor").is_none());
}

#[test]
fn atip_paging_06_every_projected_output_declares_its_fields_and_the_declaration_keeps_the_contract() {
    let repo = repo();
    let cases: Vec<(String, Value, Value)> = SIX
        .iter()
        .flat_map(|command| {
            let doc = command_doc(repo.path(), command);
            let with_fields: Vec<Value> = doc["outputs"].as_array().unwrap().iter().filter(|output| output.get("fields").is_some()).cloned().collect();
            with_fields.into_iter().map(move |output| (command.to_string(), doc.clone(), output))
        })
        .collect();
    assert_eq!(cases.len(), 8, "six commands, export with three outputs");
    for (command, doc, output) in &cases {
        let command = command.as_str();
        let fields = &output["fields"];
        let strings = |key: &str| -> Vec<String> { fields.get(key).and_then(Value::as_array).map(|list| list.iter().map(|path| path.as_str().unwrap().to_string()).collect()).unwrap_or_default() };
        let (paths, keep, default) = (strings("paths"), strings("keep"), strings("default"));
        assert!(!paths.is_empty(), "{command}");
        assert!(default.iter().all(|path| paths.contains(path)), "{command}: default is a subset of paths");
        assert!(keep.iter().all(|path| !paths.contains(path)), "{command}: keep and paths are disjoint");
        for path in paths.iter().chain(&keep) {
            let parsed = FieldPath::parse(path).unwrap_or_else(|error| panic!("{command}: {error}"));
            assert_eq!(&parsed.to_string(), path, "{command}: one spelling");
            for step in parsed.steps() {
                assert_eq!(atip::fields::is_identifier(&step.name), !path.contains("[\""), "{command}: `{path}` writes a name in brackets exactly when it is not an identifier");
            }
        }
        // The pointer, the record schema and every path are held to spec 10.13 rule 11 by the shared checker.
        let schemas = doc["schemas"].as_object().unwrap();
        let problems = atip::fields::check_declaration(&atip::fields::schemas_document(schemas), &json!({ "$ref": output["schemaRef"] }), fields["records"].as_str().unwrap(), &paths, &keep, &default, declared_rules());
        assert_eq!(problems, Vec::<String>::new(), "{command}");
        let members = atip::fields::member_names(&atip::fields::schemas_document(schemas), &json!({ "$ref": output["schemaRef"] }), fields["records"].as_str().unwrap());
        for path in paths.iter().chain(&keep) {
            assert!(members.contains(path), "{command}: `{path}` is a member of the record schema");
        }
        let words: Vec<&str> = std::iter::once("schema").chain(command.split(' ')).chain(["--json"]).collect();
        assert_eq!(atip::schema_doc_problems(&json(&canon(repo.path(), &words))), Vec::<String>::new(), "{command}: the command document keeps the contract's schema");
    }
    // The records are where the report keeps them.
    let pointers: Vec<(String, String)> = SIX.iter().map(|command| (command.to_string(), fields_output(&command_doc(repo.path(), command))["fields"]["records"].as_str().unwrap().to_string())).collect();
    assert_eq!(pointers, [("finding list", "/findings"), ("divergence list", "/scenarios"), ("query", "/records"), ("export", "/records"), ("status", ""), ("context", "")].map(|(command, pointer)| (command.to_string(), pointer.to_string())));
    // `query`'s members are the snake_case keys of the records; a report's own members are camelCase.
    let query = fields_output(&command_doc(repo.path(), "query"));
    assert!(query["fields"]["paths"].as_array().unwrap().iter().any(|path| path == "subject_id"));
    let status = fields_output(&command_doc(repo.path(), "status"));
    assert!(status["fields"]["paths"].as_array().unwrap().iter().any(|path| path == "danglingReferences" || path == "statusVersion"), "{status}");
    assert_eq!(json!(status["fields"]["keep"]), json!(["unchanged", "waitingOn", "nextOmitted"]), "the members a report of a command that carries state may hold stay");
}

#[test]
fn atip_paging_07_a_declaration_or_schema_that_breaks_the_contract_is_found() {
    let repo = repo();
    let doc = command_doc(repo.path(), "finding list");
    let output = fields_output(&doc);
    let list = |key: &str| -> Vec<String> { output["fields"][key].as_array().into_iter().flatten().map(|path| path.as_str().unwrap().to_string()).collect() };
    let (paths, keep) = (list("paths"), list("keep"));
    let check = |schemas: &Value, paths: &[String], keep: &[String], default: &[String]| {
        let doc = json!({ "schemas": schemas });
        atip::fields::check_declaration(&doc, &json!({ "$ref": "#/schemas/FindingListReport" }), "/findings", paths, keep, default, declared_rules())
    };
    let clean = Value::Object(doc["schemas"].as_object().unwrap().clone());
    assert_eq!(check(&clean, &paths, &keep, &[]), Vec::<String>::new(), "the published declaration is clean");
    // Seeded: each breaks one rule of the contract alone and is found.
    let mut seeded: Vec<(&str, Vec<String>)> = Vec::new();
    for (what, pointer, keyword, value) in [
        ("a const on the root", "/FindingListReport", "const", json!({ "changeId": "x", "findings": [] })),
        ("an allOf on the root", "/FindingListReport", "allOf", json!([{ "required": ["findings"] }])),
        ("an enum on the record", "/FindingRow", "enum", json!([{ "round": 1 }])),
        ("a oneOf on the record", "/FindingRow", "oneOf", json!([{ "required": ["round"] }])),
        ("a minProperties on the record", "/FindingRow", "minProperties", json!(2)),
    ] {
        let mut schemas = clean.clone();
        schemas.pointer_mut(pointer).unwrap()[keyword] = value;
        seeded.push((what, check(&schemas, &paths, &keep, &[])));
    }
    let mut unique = clean.clone();
    unique["FindingListReport"]["properties"]["findings"]["uniqueItems"] = json!(true);
    seeded.push(("uniqueItems on the record array", check(&unique, &paths, &keep, &[])));
    let mut untyped = clean.clone();
    untyped["FindingRow"].as_object_mut().unwrap().remove("type");
    seeded.push(("an untyped record", check(&untyped, &paths, &keep, &[])));
    let mut boolean = clean.clone();
    boolean["FindingRow"] = json!(true);
    seeded.push(("a boolean record schema", check(&boolean, &paths, &keep, &[])));
    let mut with_nosuch = paths.clone();
    with_nosuch.push("nosuch".to_string());
    seeded.push(("a path the record schema lacks", check(&clean, &with_nosuch, &keep, &[])));
    let mut dotted = paths.clone();
    dotted.push("summary.text".to_string());
    seeded.push(("a path through a leaf", check(&clean, &dotted, &keep, &[])));
    seeded.push(("a default outside paths", check(&clean, &paths, &keep, &["nosuch".to_string()])));
    seeded.push(("keep overlapping paths", check(&clean, &paths, &["summary".to_string()], &[])));
    seeded.push(("an identifier in brackets", check(&clean, &["[\"summary\"]".to_string()], &[], &[])));
    seeded.push(("a malformed path", check(&clean, &["summary.".to_string()], &[], &[])));
    for (what, problems) in seeded {
        assert!(!problems.is_empty(), "{what} is found");
    }
    // A path through an array needs `[]`: a record that holds an array is declared with it.
    let nested = json!({ "R": { "type": "object", "required": ["rows"], "properties": { "rows": { "type": "array", "items": { "$ref": "#/schemas/Row" } } } }, "Row": { "type": "object", "properties": { "owners": { "type": "array", "items": { "$ref": "#/schemas/Owner" } } } }, "Owner": { "type": "object", "properties": { "email": { "type": "string" } } } });
    let run = |paths: &[&str]| {
        let paths: Vec<String> = paths.iter().map(ToString::to_string).collect();
        atip::fields::check_declaration(&json!({ "schemas": nested }), &json!({ "$ref": "#/schemas/R" }), "/rows", &paths, &[], &[], declared_rules())
    };
    assert_eq!(run(&["owners", "owners[].email"]), Vec::<String>::new());
    assert!(!run(&["owners.email"]).is_empty(), "an array is crossed only with []");
}

/// The members `value` has, at every depth, as dotted paths.
fn paths_in(value: &Value, at: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, member) in map {
                let path = if at.is_empty() { key.clone() } else { format!("{at}.{key}") };
                out.push(path.clone());
                paths_in(member, &path, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                paths_in(item, at, out);
            }
        }
        _ => {}
    }
}

#[test]
fn atip_paging_08_fields_keeps_the_selected_members_and_the_kept_ones_and_meta_fields_lists_them() {
    let repo = repo();
    let projected = json(&canon(repo.path(), &["query", "--kind", "subject", "--fields", "subject_id,status", "--json"]));
    let full = json(&canon(repo.path(), &["query", "--kind", "subject", "--json"]));
    assert_eq!(projected["meta"]["fields"], json!(["subject_id", "status"]), "the selected paths, and the kept ones (none for query): {projected}");
    let (rows, full_rows) = (projected["data"]["records"].as_array().unwrap(), full["data"]["records"].as_array().unwrap());
    assert_eq!(rows.len(), 3);
    for (row, full_row) in rows.iter().zip(full_rows) {
        let mut keys: Vec<&String> = row.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["status", "subject_id"], "two keys, and no unselected member at any depth: {row}");
        for path in ["subject_id", "status"] {
            assert_eq!(row[path], full_row[path], "a selected member is there whenever the unprojected call has it");
        }
    }
    // The other members of the report are not records and stay.
    assert_eq!(projected["data"]["count"], full["data"]["count"]);
    // A kept path stays whatever is selected, and is listed first in `meta.fields`.
    let findings = json(&canon(repo.path(), &["finding", "list", "--change", "add-login", "--fields", "summary", "--json"]));
    assert_eq!(findings["meta"]["fields"], json!(["round", "seq", "summary"]));
    let mut seen = Vec::new();
    paths_in(&findings["data"]["findings"], "", &mut seen);
    seen.sort();
    seen.dedup();
    assert_eq!(seen, ["round", "seq", "summary"]);
    // A path listed twice is kept once.
    let twice = json(&canon(repo.path(), &["finding", "list", "--change", "add-login", "--fields", "summary,summary,reviewer", "--json"]));
    assert_eq!(twice["meta"]["fields"], json!(["round", "seq", "summary", "reviewer"]));
    // `status` is its own record: the selected members only.
    let status = json(&canon(repo.path(), &["status", "--fields", "subjects,policy", "--json"]));
    let mut keys: Vec<&String> = status["data"].as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["policy", "subjects"]);
    assert_eq!(status["meta"]["fields"], json!(["unchanged", "waitingOn", "nextOmitted", "subjects", "policy"]));
    // Without `--fields` there is no `meta.fields`: the declaration has no default.
    assert!(full["meta"].get("fields").is_none());
}

/// One call of each output, with a `--fields` selection of members that exist for it.
fn selections() -> Vec<(&'static str, &'static str, Vec<&'static str>)> {
    vec![
        ("finding list", "FindingListReport", vec!["finding", "list", "--change", "add-login", "--fields", "summary,reviewer", "--json"]),
        ("divergence list", "DivergenceListReport", vec!["divergence", "list", "--fields", "state", "--json"]),
        ("query", "QueryReport", vec!["query", "--kind", "subject", "--fields", "subject_id,status", "--json"]),
        ("export", "ExportReport", vec!["export", "--kind", "subject", "--fields", "id,digest", "--json"]),
        ("export", "ExportWrittenReport", vec!["export", "--kind", "subject", "--output", "selected.json", "--fields", "output,kind", "--json"]),
        ("export", "ExportPreviewReport", vec!["export", "--kind", "subject", "--dry-run", "--fields", "kind", "--json"]),
        ("status", "StatusReport", vec!["status", "--fields", "subjects", "--json"]),
        ("context", "ContextReport", vec!["context", "--fields", "kinds,enums", "--json"]),
    ]
}

#[test]
fn atip_paging_09_a_projected_answer_passes_the_published_schema_with_required_reduced() {
    let repo = repo();
    for (command, report, args) in selections() {
        let doc = command_doc(repo.path(), command);
        let output = doc["outputs"].as_array().unwrap().iter().find(|output| output["schemaRef"] == format!("#/schemas/{report}")).unwrap_or_else(|| panic!("{report}")).clone();
        let records = output["fields"]["records"].as_str().unwrap();
        let schemas = doc["schemas"].as_object().unwrap();
        let schema_doc = atip::fields::schemas_document(schemas);
        let root = json!({ "$ref": output["schemaRef"] });
        let projected = json(&canon(repo.path(), &args));
        assert_eq!(projected["ok"], true, "{report}: {projected}");
        let kept: Vec<FieldPath> = projected["meta"]["fields"].as_array().unwrap().iter().map(|path| FieldPath::parse(path.as_str().unwrap()).unwrap()).collect();
        let problems = atip::fields::projected_problems(&schema_doc, &root, records, &kept, &projected["data"]);
        assert_eq!(problems, Vec::<String>::new(), "{report}: the projected data passes the schema with required reduced");
        // The same call without --fields passes the schema as it is: selecting every path is the whole answer.
        let at = args.iter().position(|word| *word == "--fields").unwrap();
        let plain: Vec<&str> = args.iter().enumerate().filter(|(index, _)| *index != at && *index != at + 1).map(|(_, word)| *word).collect();
        let full = json(&canon(repo.path(), &plain));
        let all_paths: Vec<FieldPath> = output["fields"]["paths"].as_array().unwrap().iter().chain(output["fields"]["keep"].as_array().into_iter().flatten()).map(|path| FieldPath::parse(path.as_str().unwrap()).unwrap()).collect();
        assert_eq!(atip::fields::projected_problems(&schema_doc, &root, records, &all_paths, &full["data"]), Vec::<String>::new(), "{report}: selecting everything is the whole answer");
        assert!(atip::envelope_problems(&projected).is_empty(), "{report}: {:?}", atip::envelope_problems(&projected));
    }
}

#[test]
fn atip_paging_10_bracketed_paths_and_array_paths_project_as_the_contract_shows() {
    let record = json!({
        "id": 1,
        "display-name": "Ada L.",
        "profile": { "name": "Ada", "email": "ada@example.com", "timezone": "UTC" },
        "owners": [ { "name": "Bo", "email": "bo@example.com" }, { "name": "Cy", "email": "cy@example.com" } ],
        "tags": ["x", "y"],
        "attributes": { "region": "eu", "http.status-code": 200, "": "blank" },
        "labels": [ { "region": "eu", "tier": "a" }, {} ],
        "settings": { "theme": "dark", "size": 3 }
    });
    let kept = |paths: &[&str]| -> Vec<FieldPath> { paths.iter().map(|path| FieldPath::parse(path).unwrap()).collect() };
    // spec 6.10: `owners[].email` descends into each element; `profile.name` keeps what leads to it.
    assert_eq!(atip::fields::project(&record, &kept(&["id", "profile.name", "owners[].email"])), json!({ "id": 1, "profile": { "name": "Ada" }, "owners": [ { "email": "bo@example.com" }, { "email": "cy@example.com" } ] }));
    // A whole array, and a parent with its child: the parent stays whole.
    assert_eq!(atip::fields::project(&record, &kept(&["tags", "profile", "profile.name"])), json!({ "tags": ["x", "y"], "profile": record["profile"] }));
    // Escaped keys (since 1.1): a bracketed key for a name that is not an identifier, and map keys of a declared map.
    assert_eq!(
        atip::fields::project(&record, &kept(&["id", "[\"display-name\"]", "attributes.region", "attributes[\"http.status-code\"]", "attributes[\"\"]", "labels[].region", "settings.theme"])),
        json!({ "id": 1, "display-name": "Ada L.", "attributes": { "region": "eu", "http.status-code": 200, "": "blank" }, "labels": [ { "region": "eu" }, {} ], "settings": { "theme": "dark" } })
    );
    assert_eq!(atip::fields::project(&record, &kept(&["attributes[\"http.status-code\"]"])), json!({ "attributes": { "http.status-code": 200 } }), "the keys that were not selected are left out, the empty one included");
    // The path grammar: one spelling, written back as read; an identifier is never bracketed.
    for text in ["a.b[].c", "[\"x.y\"].value", "attributes[\"q\\\"uote\"]", "attributes[\"back\\\\slash\"]"] {
        assert_eq!(FieldPath::parse(text).unwrap().to_string(), text);
    }
    for text in ["", "[]", "a[]b", "a[\"id\"]", "a[\"x\\u0041\"]", "a.", "owners[][]"] {
        assert!(FieldPath::parse(text).is_err(), "`{text}`");
    }
    // A comma inside a bracketed key does not separate paths.
    assert_eq!(atip::fields::split_list("a[\"x,y\"],b"), ["a[\"x,y\"]", "b"]);
}

#[test]
fn atip_paging_11_a_schema_that_cannot_hold_a_projection_and_a_meta_member_outside_the_contract_are_found() {
    // A record schema that carries an `enum` rejects a projection of a valid value: the projected check fails on it.
    let schemas = json!({ "R": { "type": "object", "required": ["rows"], "properties": { "rows": { "type": "array", "items": { "$ref": "#/schemas/Row" } } } }, "Row": { "type": "object", "required": ["a", "b"], "properties": { "a": { "type": "integer" }, "b": { "type": "integer" } }, "enum": [{ "a": 1, "b": 2 }] } });
    let doc = json!({ "schemas": schemas });
    let root = json!({ "$ref": "#/schemas/R" });
    let value = json!({ "rows": [{ "a": 1, "b": 2 }] });
    assert_eq!(atip::fields::projected_problems(&doc, &root, "/rows", &[FieldPath::parse("a").unwrap(), FieldPath::parse("b").unwrap()], &value), Vec::<String>::new(), "the whole record is valid");
    let projected = json!({ "rows": [{ "a": 1 }] });
    assert!(!atip::fields::projected_problems(&doc, &root, "/rows", &[FieldPath::parse("a").unwrap()], &projected).is_empty(), "the enum rejects the projection of a valid value");
    let paths = vec!["a".to_string(), "b".to_string()];
    assert!(!atip::fields::check_declaration(&doc, &root, "/rows", &paths, &[], &[], declared_rules()).is_empty(), "so a declaration over that schema is not projectable");
    // A key `meta` does not define fails the envelope validation, on a paged and projected answer too.
    let repo = repo();
    let mut document = json(&canon(repo.path(), &["query", "--kind", "subject", "--limit", "1", "--fields", "subject_id", "--json"]));
    assert!(atip::envelope_problems(&document).is_empty());
    document["meta"]["hasMore"] = json!(true);
    assert!(!atip::envelope_problems(&document).is_empty());
    let mut other = json(&canon(repo.path(), &["query", "--kind", "subject", "--limit", "1", "--fields", "subject_id", "--json"]));
    other["meta"]["fields"] = json!(["subject_id", 3]);
    assert!(!atip::envelope_problems(&other).is_empty(), "meta.fields holds field paths");
}

#[test]
fn atip_paging_12_a_path_the_declaration_does_not_hold_is_usage_with_the_selectable_paths() {
    let repo = repo();
    let bad: Vec<(&str, Vec<&str>, &str)> = vec![
        ("an unknown path", vec!["query", "--kind", "subject", "--json"], "nosuch"),
        ("camelCase of a snake_case key", vec!["query", "--kind", "subject", "--json"], "changeIds"),
        ("the empty value", vec!["query", "--kind", "subject", "--json"], ""),
        ("an array path on a scalar", vec!["query", "--kind", "subject", "--json"], "subject_id[]"),
        ("a trailing comma", vec!["query", "--kind", "subject", "--json"], "subject_id,"),
        ("a bracketed identifier", vec!["query", "--kind", "subject", "--json"], "[\"subject_id\"]"),
        ("an unclosed bracketed key", vec!["query", "--kind", "subject", "--json"], "a[\"b"),
        ("a kept path", vec!["finding", "list", "--change", "add-login", "--json"], "round"),
        ("snake_case of a report member", vec!["finding", "list", "--change", "add-login", "--json"], "resolution_sha"),
        ("snake_case of a report member", vec!["divergence", "list", "--json"], "scenario_id"),
        ("a path of another output", vec!["divergence", "list", "--json"], "summary"),
        ("snake_case of a report member", vec!["status", "--json"], "status_version"),
        ("snake_case of a report member", vec!["context", "--json"], "capability_version"),
        ("another case", vec!["export", "--kind", "subject", "--json"], "Digest"),
        ("a dotted path into a leaf", vec!["export", "--kind", "subject", "--json"], "digest.value"),
    ];
    for (what, args, value) in bad {
        let mut call = args.clone();
        call.extend(["--fields", value]);
        let before = snapshot(repo.path());
        let error = error_of(&canon(repo.path(), &call));
        assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("USAGE"), Some("--fields")), "{what}: {error}");
        let expected = error["expected"].as_array().unwrap_or_else(|| panic!("{what}: expected lists the selectable paths: {error}"));
        assert!(!expected.is_empty(), "{what}");
        let command = &args[..if args[0] == "finding" || args[0] == "divergence" { 2 } else { 1 }];
        let doc = command_doc(repo.path(), &command.join(" "));
        let declared = fields_output(&doc)["fields"]["paths"].clone();
        assert_eq!(json!(expected), declared, "{what}: `expected` is the declared paths");
        assert_eq!(snapshot(repo.path()), before, "{what}: a refusal writes nothing");
    }
    // The refusal says the same without --json, with no report on stdout.
    let out = canon(repo.path(), &["status", "--fields", "nosuch"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}

#[test]
fn atip_paging_13_export_is_three_outputs_and_fields_cuts_the_manifest_and_the_file() {
    let repo = repo();
    let doc = command_doc(repo.path(), "export");
    let outputs = doc["outputs"].as_array().unwrap();
    let names: Vec<&str> = outputs.iter().map(|output| output["schemaRef"].as_str().unwrap()).collect();
    assert_eq!(names, ["#/schemas/ExportReport", "#/schemas/ExportWrittenReport", "#/schemas/ExportPreviewReport", "#/schemas/BatchExportReport"]);
    assert!(outputs.iter().all(|output| output["when"].is_string()), "every output says when it is the one printed");
    assert_eq!(outputs.iter().filter(|output| output.get("fields").is_some()).count(), 3, "each of the three answers declares its own fields (the batch output has none)");
    assert_eq!(outputs[0]["fields"]["records"], "/records");
    assert_eq!((outputs[1]["fields"]["records"].as_str(), outputs[2]["fields"]["records"].as_str()), (Some(""), Some("")), "the written answer and the preview are their own record");
    assert_eq!(outputs[1]["fields"]["paths"], json!(["kind", "output", "recordCount"]));
    assert_eq!(outputs[2]["fields"]["paths"], json!(["dryRun", "kind", "output", "recordCount"]));
    assert_eq!(outputs[0]["schemaVersion"], "2.1", "the union became three outputs: a new major (2.0, and the 1 minor every write adds)");
    for kind in ["ExportReport", "ExportWrittenReport", "ExportPreviewReport"] {
        let schema = &doc["schemas"][kind];
        assert!(schema.get("anyOf").is_none() && schema.get("oneOf").is_none(), "{kind} is one shape");
    }
    // Each answer names its output.
    let manifest = json(&canon(repo.path(), &["export", "--kind", "subject", "--json"]));
    assert_eq!((manifest["meta"]["output"].as_str(), manifest["meta"]["schemaVersion"].as_str()), (Some("ExportReport"), Some("2.1")), "{manifest}");
    assert!(manifest["data"].get("meta").is_none(), "the page is meta.page, not a member of the report");
    let preview = json(&canon(repo.path(), &["export", "--kind", "subject", "--dry-run", "--json"]));
    assert_eq!(preview["meta"]["output"], "ExportPreviewReport", "{preview}");
    // `--fields` is read against the output the call selects: with --output the answer names the file, so its own members are the paths.
    let target = repo.path().join("out.json");
    let written = json(&canon(repo.path(), &["export", "--kind", "subject", "--output", target.to_str().unwrap(), "--fields", "kind", "--json"]));
    assert_eq!(written["meta"]["output"], "ExportWrittenReport", "{written}");
    assert_eq!(written["meta"]["fields"], json!(["unchanged", "waitingOn", "nextOmitted", "kind"]));
    assert_eq!(written["data"], json!({ "kind": "subject" }));
    let file: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    assert_eq!(file["records"].as_array().unwrap()[0].as_object().unwrap().len(), 4, "the file holds the whole manifest: it is not the output --fields was read against");
    assert_eq!(file["meta"]["total"], 3, "and the page it covers");
    let refused = error_of(&canon(repo.path(), &["export", "--kind", "subject", "--output", target.to_str().unwrap(), "--fields", "id", "--json"]));
    assert_eq!((refused["code"].as_str(), refused["field"].as_str()), (Some("USAGE"), Some("--fields")), "{refused}");
    assert_eq!(refused["expected"], json!(["kind", "output", "recordCount"]));
    let dry = json(&canon(repo.path(), &["export", "--kind", "subject", "--dry-run", "--fields", "recordCount", "--json"]));
    assert_eq!(dry["data"], json!({ "recordCount": 3 }), "{dry}");
    // Text mode prints, or writes, the manifest: `--fields` cuts its records.
    let text_target = repo.path().join("text.json");
    ok(repo.path(), &["export", "--kind", "subject", "--output", text_target.to_str().unwrap(), "--fields", "id"]);
    let file: Value = serde_json::from_slice(&std::fs::read(&text_target).unwrap()).unwrap();
    assert!(file["records"].as_array().unwrap().iter().all(|record| record.as_object().unwrap().keys().collect::<Vec<_>>() == ["id"]), "the file holds the projected records: {file}");
    assert_eq!(file["meta"]["total"], 3, "and the page it holds");
    // The envelope's manifest, projected, passes the published schema with required reduced.
    let projected = json(&canon(repo.path(), &["export", "--kind", "subject", "--fields", "id,digest", "--json"]));
    assert_eq!(projected["meta"]["fields"], json!(["id", "digest"]));
    assert!(projected["data"]["records"].as_array().unwrap().iter().all(|record| record.as_object().unwrap().len() == 2));
    assert_eq!(projected["data"]["contentDigests"].as_array().unwrap().len(), 3, "the manifest's digests do not depend on the projection");
}

#[test]
fn atip_paging_14_a_paged_or_projected_answer_carries_only_the_contracts_meta_members_and_no_paging_of_its_own() {
    let repo = repo();
    const META: &[&str] = &["schemaVersion", "tool", "toolVersion", "command", "contract", "contractVersion", "output", "context", "next", "warnings", "coverage", "page", "fields", "truncated", "x-canon"];
    for (name, args) in listings() {
        let mut call = args.clone();
        call.extend(["--limit", "1", "--fields", if name == "query" { "subject_id" } else if name == "export" { "id" } else if name == "finding list" { "summary" } else { "state" }]);
        let document = json(&canon(repo.path(), &call));
        assert_eq!(document["ok"], true, "{name}: {document}");
        let keys: Vec<&String> = document["meta"].as_object().unwrap().keys().collect();
        assert!(keys.iter().all(|key| META.contains(&key.as_str())), "{name}: {keys:?}");
        assert!(document["meta"]["page"].is_object() && document["meta"]["fields"].is_array() && document["meta"]["next"].is_array(), "{name}: {document}");
        assert!(atip::envelope_problems(&document).is_empty(), "{name}");
        for member in ["page", "nextCursor", "returned", "hasMore", "total", "meta"] {
            assert!(document["data"].get(member).is_none(), "{name}: data has no `{member}`");
        }
        // No answer is cut by size: there is no `meta.truncated`.
        assert!(document["meta"].get("truncated").is_none());
        // The report schema carries no paging member of its own either.
        let doc = command_doc(repo.path(), name);
        for schema in doc["schemas"].as_object().unwrap().values() {
            let properties = schema["properties"].as_object();
            for member in ["page", "nextCursor", "hasMore", "returned"] {
                assert!(properties.is_none_or(|properties| !properties.contains_key(member)), "{name}: {schema}");
            }
        }
    }
}

#[test]
fn atip_paging_15_the_camel_case_alias_of_a_snake_case_key_and_the_snake_case_alias_of_a_report_member_are_gone() {
    let repo = repo();
    // A record keeps its own key: `change_ids` selects, `changeIds` is refused; a report's member is camelCase and has no alias.
    let ok_call = canon(repo.path(), &["query", "--kind", "subject", "--fields", "change_ids", "--json"]);
    assert!(ok_call.status.success());
    let by_alias = canon(repo.path(), &["query", "--kind", "subject", "--fields", "changeIds", "--json"]);
    assert_eq!(error_of(&by_alias)["field"], "--fields");
    let spelled = canon(repo.path(), &["finding", "list", "--change", "add-login", "--fields", "resolutionSha", "--json"]);
    assert!(spelled.status.success());
    assert_eq!(error_of(&canon(repo.path(), &["finding", "list", "--change", "add-login", "--fields", "resolution_sha", "--json"]))["code"], "USAGE");
    let divergence = json(&canon(repo.path(), &["divergence", "list", "--fields", "state", "--json"]));
    assert_eq!(divergence["data"]["scenarios"][0], json!({ "projectId": "root", "scenarioId": "auth.login.01", "state": "open" }));
    assert_eq!(error_of(&canon(repo.path(), &["divergence", "list", "--fields", "project_id", "--json"]))["code"], "USAGE");
    // The help of each option says so too.
    for command in SIX {
        let doc = command_doc(repo.path(), command);
        let help = doc["options"].as_array().unwrap().iter().find(|option| option["name"] == "fields").unwrap()["help"].as_str().unwrap().to_string();
        assert!(!help.contains("camelCase and snake_case both match"), "{command}: {help}");
    }
}

#[test]
fn atip_paging_16_a_batch_takes_no_fields_and_fields_is_a_flag_a_document_cannot_set() {
    let repo = repo();
    let before = snapshot(repo.path());
    let (a, b) = (repo.path().join("a.json"), repo.path().join("b.json"));
    let (a, b) = (a.to_str().unwrap(), b.to_str().unwrap());
    // `--fields` is a flag: a `fields` key of a document is INPUT_INVALID / FLAG_ONLY_KEY, at plan time, whatever the element.
    let batch = format!(r#"[{{"kind":"subject","output":"{a}"}},{{"kind":"subject","output":"{b}","fields":"id"}}]"#);
    let refused = canon(repo.path(), &["export", "--json", "--input", &batch]);
    let error = error_of(&refused);
    assert_eq!((error["code"].as_str(), error["reason"].as_str(), error["field"].as_str()), (Some("INPUT_INVALID"), Some("FLAG_ONLY_KEY"), Some("/1/fields")), "{error}");
    assert_ne!(error["code"], "BATCH_INCOMPLETE");
    assert_eq!(snapshot(repo.path()), before, "nothing was written, the first element included: no file, no audit line");
    // A single document is refused the same way.
    let single = canon(repo.path(), &["export", "--json", "--input", r#"{"kind":"subject","fields":"id"}"#]);
    let error = error_of(&single);
    assert_eq!((error["code"].as_str(), error["reason"].as_str(), error["field"].as_str()), (Some("INPUT_INVALID"), Some("FLAG_ONLY_KEY"), Some("/fields")), "{error}");
    // `--fields` on the command line of a batch is read against the batch output, which declares no fields: USAGE.
    let plain = format!(r#"[{{"kind":"subject","output":"{a}"}},{{"kind":"subject","output":"{b}"}}]"#);
    let error = error_of(&canon(repo.path(), &["export", "--json", "--input", &plain, "--fields", "id"]));
    assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("USAGE"), Some("--fields")), "{error}");
    assert_eq!(snapshot(repo.path()), before, "a refusal at plan time writes nothing");
    // The same batch without `--fields` runs.
    let ran = canon(repo.path(), &["export", "--json", "--input", &plain]);
    assert!(ran.status.success(), "{}{}", String::from_utf8_lossy(&ran.stdout), String::from_utf8_lossy(&ran.stderr));
    // The command document says so: the batch output has no `fields`, the option is flag only and the fragment has no `fields` key.
    let doc = command_doc(repo.path(), "export");
    let batch_output = doc["outputs"].as_array().unwrap().iter().find(|output| output["batch"] == true).unwrap();
    assert!(batch_output.get("fields").is_none());
    let option = doc["options"].as_array().unwrap().iter().find(|option| option["name"] == "fields").unwrap();
    assert_eq!(option["flagOnly"], true);
    let fragment = doc["input"]["schemaRef"].as_str().unwrap().trim_start_matches("#/schemas/");
    assert!(doc["schemas"][fragment]["properties"].get("fields").is_none(), "the fragment holds no `fields`");
    for command in SIX {
        let doc = command_doc(repo.path(), command);
        let option = doc["options"].as_array().unwrap().iter().find(|option| option["name"] == "fields").unwrap();
        assert_eq!(option["flagOnly"], true, "{command}");
    }
}

/// The cursor named in a text-mode listing (`... next cursor: <hex>  (rerun ...`), wherever it was printed.
fn text_cursor(output: &Output) -> String {
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let after = text.split("next cursor: ").nth(1).unwrap_or_else(|| panic!("a next cursor in {text}"));
    after.split_whitespace().next().unwrap().to_string()
}

#[test]
fn atip_paging_17_the_output_mode_does_not_bind_a_cursor_an_explicit_default_does_and_text_mode_checks_fields() {
    let repo = repo();
    // Issued in text mode, used with --json; issued with --json, used in text mode.
    let text = canon(repo.path(), &["query", "--kind", "subject", "--limit", "1"]);
    assert!(text.status.success(), "{}", String::from_utf8_lossy(&text.stderr));
    let from_text = text_cursor(&text);
    let with_json = json(&canon(repo.path(), &["query", "--kind", "subject", "--limit", "1", "--cursor", &from_text, "--json"]));
    assert_eq!(with_json["ok"], true, "{with_json}");
    assert_eq!(with_json["meta"]["page"]["returned"], 1);
    let (_, from_json) = first_page(repo.path(), &["query", "--kind", "subject", "--json"]);
    let in_text = canon(repo.path(), &["query", "--kind", "subject", "--limit", "1", "--cursor", &from_json]);
    assert!(in_text.status.success(), "{}", String::from_utf8_lossy(&in_text.stderr));
    assert_eq!(from_text, from_json, "the same call has the same cursor in either mode");
    // An argument equal to its default still changes the call: `--repo .` is not the omitted `--repo`.
    let (_, cursor) = first_page(repo.path(), &["query", "--kind", "subject", "--json"]);
    let explicit = error_of(&canon(repo.path(), &["query", "--kind", "subject", "--repo", ".", "--limit", "1", "--cursor", &cursor, "--json"]));
    assert_eq!((explicit["code"].as_str(), explicit["reason"].as_str()), (Some("CURSOR_INVALID"), Some("ARGUMENTS_CHANGED")), "{explicit}");
    // `--fields` is read the same in text mode: an unknown path is USAGE, a declared one is accepted.
    let before = snapshot(repo.path());
    for args in [vec!["query", "--kind", "subject", "--fields", "nosuch"], vec!["finding", "list", "--change", "add-login", "--fields", "resolution_sha"], vec!["status", "--fields", "nosuch"]] {
        let out = canon(repo.path(), &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}: a refusal prints no report");
        assert!(String::from_utf8_lossy(&out.stderr).contains("--fields"), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    assert_eq!(snapshot(repo.path()), before);
    assert!(canon(repo.path(), &["query", "--kind", "subject", "--fields", "subject_id"]).status.success());
    assert!(canon(repo.path(), &["finding", "list", "--change", "add-login", "--fields", "summary"]).status.success());
}
