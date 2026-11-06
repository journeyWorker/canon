//! The 1.3 rules of `--input` (`canon-0-36-input-1-3`, scenarios `atip.input13.*`), against the built binary: step 3 reads argv
//! alone so a document may supply a required argument, a batch runs its items in order and the code of the item that failed says
//! whether it had an effect, an integer outside ±(2^53-1) is refused however it is spelled, the next page is the `meta.next`
//! arguments with only `--limit` (`--all`) and `--fields` changed, `[]` is refused (a repeatable option's none is the key omitted), and
//! several problems answer with the first `details` entry as `field`.
//!
//! Tests are named by scenario id (`atip_input13_01_…` proves `atip.input13.01`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};
use tempfile::TempDir;

fn canon_in(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = hermetic::canon();
    command.args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-input13").stdin(Stdio::null());
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("spawn canon")
}

fn canon(dir: &Path, args: &[&str]) -> Output {
    canon_in(dir, args, &[])
}

fn json_of(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn ok(dir: &Path, args: &[&str]) {
    let out = canon(dir, args);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// A git repo with subject `demo`, change `add-login` and three findings (round 1, seq 1-3), all open. Committed before the findings.
fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    let git = |args: &[&str]| {
        let out = Command::new("git").current_dir(repo).args(["-c", "user.name=canon", "-c", "user.email=canon@example.invalid"]).args(args).output().expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    ok(repo, &["init", "--providers", "none"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    for seq in ["1", "2", "3"] {
        ok(repo, &["finding", "add", "--change", "add-login", "--round", "1", "--seq", seq, "--severity", "note", "--actor", "bob", "--summary", &format!("Nit {seq}")]);
    }
    dir
}

/// Every file of the repo but `.git` and the hot tier's SQLite files, with its bytes.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            if rel == ".git" || rel.starts_with(".canon/hot.db") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// `(exit, code, reason, field)` of a refusal, and the whole answer.
type Refusal = (Option<i32>, String, Option<String>, Option<String>, Value);

fn refusal(output: &Output) -> Refusal {
    let body = json_of(output);
    let error = &body["error"];
    (output.status.code(), error["code"].as_str().unwrap_or_default().to_string(), error["reason"].as_str().map(str::to_string), error["field"].as_str().map(str::to_string), body)
}

/// The `(seq, disposition)` of every finding of `add-login`, as `finding list` reports them.
fn dispositions(repo: &Path) -> Vec<(u64, String)> {
    let body = json_of(&canon(repo, &["finding", "list", "--change", "add-login", "--all", "--json"]));
    body["data"]["findings"].as_array().expect("findings").iter().map(|finding| (finding["seq"].as_u64().unwrap(), finding["disposition"].as_str().unwrap().to_string())).collect()
}

fn close(seq: u64, disposition: &str) -> Value {
    let mut item = json!({"operands": ["add-login"], "round": 1, "seq": seq, "disposition": disposition});
    if matches!(disposition, "rejected" | "deferred") {
        item["reason"] = json!("decided in review");
    }
    item
}

fn run_batch(repo: &Path, leaf: &[&str], items: &[Value]) -> (Option<i32>, Value) {
    let doc = Value::Array(items.to_vec()).to_string();
    let mut argv: Vec<&str> = leaf.to_vec();
    argv.extend(["--input", &doc, "--json"]);
    let out = canon(repo, &argv);
    (out.status.code(), json_of(&out))
}

fn statuses(body: &Value) -> Vec<&str> {
    body["data"]["items"].as_array().expect("a batch report").iter().map(|item| item["status"].as_str().unwrap()).collect()
}

/// The exit of `code` in the registry `canon schema --errors` prints.
fn registry_exit(repo: &Path, code: &str) -> u64 {
    let body = json_of(&canon(repo, &["schema", "--errors", "--json"]));
    body["data"]["errors"].as_array().expect("the registry").iter().find(|row| row["code"] == code).unwrap_or_else(|| panic!("{code} is registered")).get("exit").and_then(Value::as_u64).expect("an exit")
}

// ------------------------------------------------------------------------------------------------ F1: step 3

#[test]
fn atip_input13_01_a_document_supplies_the_required_options_and_argv_gives_the_operand() {
    let dir = fixture();
    let repo = dir.path();
    // The required `--round`, `--seq` and `--disposition` are the document's; argv holds the operand only.
    let out = canon(repo, &["finding", "close", "add-login", "--input", r#"{"round":1,"seq":1,"disposition":"deferred","reason":"decided in review"}"#, "--json"]);
    let body = json_of(&out);
    assert_eq!((out.status.code(), body["ok"].as_bool()), (Some(0), Some(true)), "{body}");
    assert_eq!(dispositions(repo), [(1, "deferred".to_string()), (2, "open".to_string()), (3, "open".to_string())]);
    // The operand may be the document's too, and argv's own explicit value that equals the document's merges (`01` is the number 1).
    let out = canon(repo, &["finding", "close", "--round", "01", "--input", r#"{"operands":["add-login"],"round":1,"seq":2,"disposition":"rejected","reason":"decided in review"}"#, "--json"]);
    let body = json_of(&out);
    assert_eq!((out.status.code(), body["ok"].as_bool()), (Some(0), Some(true)), "{body}");
    assert_eq!(dispositions(repo)[1], (2, "rejected".to_string()));
}

#[test]
fn atip_input13_02_an_argument_that_differs_from_the_document_is_input_conflict_at_the_key() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    let document = r#"{"round":1,"seq":1,"disposition":"deferred","reason":"decided in review"}"#;
    // The contract names a value at fault in a document by its pointer (10.8), and the checker's L2.input-conflict reads the document pointer.
    let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "add-login", "--round", "2", "--input", document, "--json"]));
    assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "INPUT_CONFLICT", Some("/round")), "{body}");
    let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "other-change", "--input", &json!({"operands": ["add-login"], "round": 1, "seq": 1, "disposition":"fixed"}).to_string(), "--json"]));
    assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "INPUT_CONFLICT", Some("/operands")), "{body}");
    assert_eq!(tree(repo), before, "a refused call writes nothing");
}

#[test]
fn atip_input13_03_a_fault_of_argv_alone_answers_before_the_document_and_a_required_argument_neither_gives_is_missing() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    let document = r#"{"round":1,"seq":1,"disposition":"deferred","reason":"decided in review"}"#;
    // Argv is read alone at step 3: an unknown option or a malformed value is refused with its usual code, though the document is fine.
    let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "add-login", "--bogus", "--input", document, "--json"]));
    assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "USAGE", Some("--bogus")), "{body}");
    let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "add-login", "--disposition", "nope", "--input", document, "--json"]));
    assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "USAGE", Some("--disposition")), "{body}");
    // What neither gives is judged after the merge, with the code a call without a document has: `--seq` is missing.
    let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "add-login", "--input", r#"{"round":1,"disposition":"fixed"}"#, "--json"]));
    assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "USAGE", Some("/seq")), "{body}");
    // The merged call is checked as one call: a value the document gave breaks the rule an argv value would break.
    let (exit, code, _, _, body) = refusal(&canon(repo, &["finding", "close", "add-login", "--input", r#"{"round":0,"seq":1,"disposition":"fixed"}"#, "--json"]));
    let (_, plain_code, _, _, _) = refusal(&canon(repo, &["finding", "close", "add-login", "--round", "0", "--seq", "1", "--disposition", "fixed", "--json"]));
    assert_eq!((exit, code.as_str()), (Some(2), plain_code.as_str()), "{body}");
    assert_eq!(tree(repo), before, "a refused call writes nothing");
}

// ------------------------------------------------------------------------------------------------ F2: batch

#[test]
fn atip_input13_04_a_batch_checks_every_item_before_it_runs_one() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    // The last item has the wrong type: the two before it are not run, the answer is exit 2 at `/2/…` with no data.
    let (exit, body) = run_batch(repo, &["finding", "close"], &[close(1, "deferred"), close(2, "rejected"), json!({"operands": ["add-login"], "round": "x", "seq": 3, "disposition":"fixed"})]);
    assert_eq!(exit, Some(2), "{body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some("INPUT_INVALID"), Some("/2/round")), "{body}");
    assert!(body.get("data").is_none(), "a preflight refusal has no data: {body}");
    assert_eq!(tree(repo), before, "no item ran");
    assert_eq!(dispositions(repo), [(1, "open".to_string()), (2, "open".to_string()), (3, "open".to_string())]);
    // A fault that needs no state in the last item (a conflict with argv) is the same: nothing ran.
    let doc = Value::Array(vec![close(1, "deferred"), json!({"operands": ["add-login"], "round": 2, "seq": 2, "disposition":"fixed"})]).to_string();
    let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "--round", "1", "--input", &doc, "--json"]));
    assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "INPUT_CONFLICT", Some("/1/round")), "{body}");
    assert_eq!(tree(repo), before, "no item ran");
}

#[test]
fn atip_input13_05_the_item_that_fails_when_it_runs_stops_the_batch_and_its_code_tells_whether_it_had_an_effect() {
    let dir = fixture();
    let repo = dir.path();
    // Item 1 names a finding no ledger has: it passes the checks that read no state and fails when it runs.
    let (exit, body) = run_batch(repo, &["finding", "close"], &[close(1, "deferred"), close(9, "fixed"), close(2, "rejected")]);
    assert_eq!(exit, Some(1), "{body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some("BATCH_INCOMPLETE"), Some("/1")), "{body}");
    assert_eq!(statuses(&body), ["written", "failed", "skipped"], "{body}");
    let items = body["data"]["items"].as_array().unwrap();
    assert!(items[1].get("data").is_none() && items[2].get("data").is_none() && items[2].get("error").is_none(), "{body}");
    // The exit of the failed item's code is the answer to "did it have an effect": 2 is a refusal, nothing of that item was written.
    let failed = items[1]["error"]["code"].as_str().expect("the failed item's own code");
    assert_eq!(registry_exit(repo, failed), 2, "{failed} is a refusal in the registry");
    assert_eq!(dispositions(repo), [(1, "deferred".to_string()), (2, "open".to_string()), (3, "open".to_string())], "item 0 ran, item 1 had no effect, item 2 was skipped");
}

#[test]
fn atip_input13_06_every_item_is_one_entry_of_the_report_in_the_order_submitted() {
    let dir = fixture();
    let repo = dir.path();
    let submitted = [close(1, "deferred"), close(2, "rejected"), close(3, "fixed")];
    let (exit, body) = run_batch(repo, &["finding", "close"], &submitted);
    assert_eq!(exit, Some(0), "{body}");
    let items = body["data"]["items"].as_array().unwrap();
    assert_eq!(items.iter().map(|item| item["index"].as_u64().unwrap()).collect::<Vec<_>>(), [0, 1, 2], "{body}");
    assert_eq!(items.iter().map(|item| item["data"]["seq"].as_u64().unwrap()).collect::<Vec<_>>(), [1, 2, 3], "one entry per item, in the order it was submitted: {body}");
    // The same batch with the third item out of order of the ledger fails at its index and still lists all four.
    let (exit, body) = run_batch(repo, &["finding", "close"], &[close(3, "deferred"), close(1, "deferred"), close(7, "fixed"), close(2, "fixed")]);
    assert_eq!(exit, Some(1), "{body}");
    assert_eq!(statuses(&body), ["written", "unchanged", "failed", "skipped"], "{body}");
    assert_eq!(body["error"]["field"], "/2", "{body}");
}

// ------------------------------------------------------------------------------------------------ F3: integers out of range

const MAX_SAFE: &str = "9007199254740991";

#[test]
fn atip_input13_07_the_largest_safe_integer_passes_the_integer_check_and_the_next_two_do_not_in_either_spelling() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    let outside = |body: &Value| body["error"]["message"].as_str().is_some_and(|message| message.contains("2^53") || message.contains("outside the integers"));
    // The largest value is no integer fault: it reaches the argument's own domain (`--round` is at most 4294967295).
    for (what, document) in [("number", format!(r#"{{"change":"add-login","round":{MAX_SAFE},"seq":1,"severity":"note","actor":"bob","summary":"m"}}"#)), ("string", format!(r#"{{"change":"add-login","round":"{MAX_SAFE}","seq":1,"severity":"note","actor":"bob","summary":"m"}}"#))] {
        let (exit, code, reason, field, body) = refusal(&canon(repo, &["finding", "add", "--input", &document, "--json"]));
        assert_eq!(exit, Some(2), "{what}: {body}");
        assert!(!outside(&body) && reason.as_deref() != Some("INVALID_VALUE"), "{what}: the largest safe integer is not an integer fault: {body}");
        assert_eq!((code.as_str(), field.as_deref()), ("USAGE", Some("/round")), "{what}: it is judged by the domain: {body}");
    }
    // The two after it are refused as numbers and as strings, at the value's pointer, with the value as written (no rounding).
    for value in ["9007199254740992", "9007199254740993", "-9007199254740992"] {
        for (what, written) in [("number", value.to_string()), ("string", format!("\"{value}\""))] {
            let document = format!(r#"{{"change":"add-login","round":{written},"seq":1,"severity":"note","actor":"bob","summary":"m"}}"#);
            let (exit, code, reason, field, body) = refusal(&canon(repo, &["finding", "add", "--input", &document, "--json"]));
            assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("INVALID_VALUE"), Some("/round")), "{value} as a {what}: {body}");
            assert_eq!(body["error"]["received"], value, "{value} as a {what}: the value as written: {body}");
        }
    }
    // In a batch the pointer carries the index.
    let (exit, body) = run_batch(repo, &["finding", "close"], &[close(1, "fixed"), json!({"operands": ["add-login"], "round": 1, "seq": 9007199254740993_u64, "disposition":"fixed"})]);
    assert_eq!((exit, body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("INPUT_INVALID"), Some("/1/seq")), "{body}");
    assert_eq!(tree(repo), before, "every refusal left the repository as it was");
}

#[test]
fn atip_input13_08_argv_refuses_the_same_integers_as_the_same_spelling_in_a_document() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    for value in ["9007199254740992", "9007199254740993", "-9007199254740993"] {
        let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "add", "--change", "add-login", "--round", value, "--seq", "1", "--severity", "note", "--actor", "bob", "--summary", "m", "--json"]));
        assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "USAGE", Some("--round")), "{value}: {body}");
        assert_eq!(body["error"]["received"], value, "{value}: {body}");
        assert!(body["error"]["message"].as_str().unwrap().contains("9007199254740991"), "{value}: the refusal names the range: {body}");
    }
    // The largest safe integer is not refused as out of range on argv either: it reaches the domain.
    let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "add", "--change", "add-login", "--round", MAX_SAFE, "--seq", "1", "--severity", "note", "--actor", "bob", "--summary", "m", "--json"]));
    assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "USAGE", Some("--round")), "{body}");
    assert!(!body["error"]["message"].as_str().unwrap().contains("beyond"), "{body}");
    assert_eq!(tree(repo), before, "every refusal left the repository as it was");
}

// ------------------------------------------------------------------------------------------------ F4: the next page

/// The first page of `finding list` (`--limit 1`) and the argv `meta.next[0]` gives for the next.
fn first_page(repo: &Path, extra: &[&str], env: &[(&str, &str)]) -> Vec<String> {
    let mut args = vec!["finding", "list", "--change", "add-login", "--limit", "1", "--json"];
    args.extend(extra);
    let document = json_of(&canon_in(repo, &args, env));
    assert_eq!(document["ok"], true, "{document}");
    document["meta"]["next"][0]["argv"].as_array().expect("a next page").iter().map(|word| word.as_str().unwrap().to_string()).collect()
}

fn run_argv(repo: &Path, argv: &[String], env: &[(&str, &str)]) -> Output {
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    canon_in(repo, &words, env)
}

fn without(argv: &[String], prefix: &str) -> Vec<String> {
    argv.iter().filter(|word| !word.starts_with(prefix)).cloned().collect()
}

fn seqs(output: &Output) -> Vec<u64> {
    let body = json_of(output);
    assert_eq!(body["ok"], true, "{body}");
    body["data"]["findings"].as_array().unwrap().iter().map(|finding| finding["seq"].as_u64().unwrap()).collect()
}

#[test]
fn atip_input13_09_the_next_page_is_the_next_arguments_as_given_and_only_the_page_size_and_the_projection_change() {
    let dir = fixture();
    let repo = dir.path();
    let next = first_page(repo, &[], &[]);
    assert_eq!(&next[..3], ["finding", "list", "--change=add-login"], "{next:?}");
    // As given: the second finding.
    assert_eq!(seqs(&run_argv(repo, &next, &[])), [2]);
    // Another page size, the rest of the listing with `--all`, and a projection: each still continues from the same place.
    let mut wider = without(&next, "--limit=");
    wider.push("--limit=2".to_string());
    assert_eq!(seqs(&run_argv(repo, &wider, &[])), [2, 3]);
    let mut all = without(&next, "--limit=");
    all.push("--all".to_string());
    assert_eq!(seqs(&run_argv(repo, &all, &[])), [2, 3]);
    let mut projected = next.clone();
    projected.push("--fields=summary".to_string());
    let body = json_of(&run_argv(repo, &projected, &[]));
    let kept = body["data"]["findings"][0].clone();
    assert_eq!((body["ok"].as_bool(), kept["seq"].clone(), kept["summary"].clone(), kept.get("severity").is_some()), (Some(true), json!(2), json!("Nit 2"), false), "{body}");
}

#[test]
fn atip_input13_10_a_changed_filter_an_explicit_default_or_a_changed_environment_is_cursor_invalid_at_the_cursor() {
    let dir = fixture();
    let repo = dir.path();
    let next = first_page(repo, &[], &[]);
    let before = tree(repo);
    let refused = |what: &str, argv: Vec<String>, env: &[(&str, &str)]| {
        let (exit, code, reason, field, body) = refusal(&run_argv(repo, &argv, env));
        assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "CURSOR_INVALID", Some("ARGUMENTS_CHANGED"), Some("--cursor")), "{what}: {body}");
    };
    // The filter changed.
    let mut filtered = next.clone();
    filtered.push("--open".to_string());
    refused("--open added", filtered, &[]);
    refused("another change", next.iter().map(|word| word.replace("--change=add-login", "--change=add-logout")).collect(), &[]);
    // An explicit default is an argument the cursor was not issued for: canon's choice (the contract leaves it to the tool) is to refuse it.
    let mut explicit = next.clone();
    explicit.push("--repo=.".to_string());
    refused("--repo given with its default", explicit, &[]);
    // The environment the cursor was issued under.
    refused("CANON_ROLE set", next.clone(), &[("CANON_ROLE", "reviewer")]);
    let issued_under_role = first_page(repo, &[], &[("CANON_ROLE", "reviewer")]);
    refused("CANON_ROLE dropped", issued_under_role.clone(), &[]);
    refused("CANON_ROLE changed", issued_under_role.clone(), &[("CANON_ROLE", "planner")]);
    assert_eq!(seqs(&run_argv(repo, &issued_under_role, &[("CANON_ROLE", "reviewer")])), [2], "the same environment continues");
    assert_eq!(tree(repo), before, "a refused cursor writes nothing");
}

// ------------------------------------------------------------------------------------------------ F5: []

/// `camelCase` of an option's long name (`surface-ref` is `surfaceRef`).
fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for ch in name.chars() {
        if ch == '-' {
            up = true;
        } else if up {
            out.extend(ch.to_uppercase());
            up = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Every leaf that takes `--input`, as the words of its command and the properties of its input schema (`name`, `repeatable`).
type InputLeaf = (Vec<String>, Vec<(String, bool)>);

fn leaves_with_input(_repo: &Path) -> Vec<InputLeaf> {
    let mut out = Vec::new();
    for doc in canon_cli::spec::model()["commands"].as_array().unwrap() {
        let words: Vec<String> = doc["path"].as_array().unwrap().iter().map(|word| word.as_str().unwrap().to_string()).collect();
        if !doc["options"].as_array().unwrap().iter().any(|option| option["name"] == "input") {
            continue;
        }
        let schemas = doc["schemas"].as_object().unwrap();
        let input = schemas.iter().find(|(name, _)| name.ends_with("Input") && !name.starts_with("Batch")).map_or_else(|| panic!("{words:?} has an input schema"), |(_, schema)| schema);
        let properties = input["properties"].as_object().unwrap().iter().map(|(name, schema)| (name.clone(), schema["type"] == "array")).collect();
        out.push((words, properties));
    }
    out
}

#[test]
fn atip_input13_11_an_empty_array_for_a_repeatable_option_is_input_invalid_at_the_key_and_writes_nothing() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    // The brief's call: `evidence add` with `artifact: []`.
    let (exit, code, reason, field, body) = refusal(&canon(repo, &["evidence", "add", "--input", r#"{"artifact":[]}"#, "--json"]));
    assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("INVALID_VALUE"), Some("/artifact")), "{body}");
    assert!(body["error"]["hint"].as_str().unwrap().contains("omitting the key"), "none is written by omitting the key: {body}");
    // Every repeatable option of every leaf that takes a document says the same, at its own key; in a batch the pointer has the index.
    let mut checked = 0;
    for (words, properties) in leaves_with_input(repo) {
        for (key, repeatable) in properties.iter().filter(|(key, repeatable)| *repeatable && key != "operands") {
            let _ = repeatable;
            let mut argv: Vec<&str> = words.iter().map(String::as_str).collect();
            let document = json!({ key.clone(): [] }).to_string();
            argv.extend(["--input", &document, "--json"]);
            let (exit, code, reason, field, body) = refusal(&canon(repo, &argv));
            assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("INVALID_VALUE"), Some(format!("/{key}").as_str())), "{words:?} {key}: {body}");
            assert_eq!(key, &camel(key), "the key is the camelCase spelling");
            checked += 1;
        }
    }
    assert!(checked >= 11, "the repeatable options of the leaves were all tried ({checked})");
    let (exit, body) = run_batch(repo, &["evidence", "add"], &[json!({"scenario": "x.y.01", "artifact": []})]);
    assert_eq!((exit, body["error"]["code"].as_str(), body["error"]["reason"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("INPUT_INVALID"), Some("INVALID_VALUE"), Some("/0/artifact")), "{body}");
    assert_eq!(tree(repo), before, "every refusal wrote nothing");
    // The key left out is the call as it was: the document without `artifact` is not refused for it.
    let (_, _, _, field, body) = refusal(&canon(repo, &["evidence", "add", "--input", r#"{"scenario":"auth.login.01"}"#, "--json"]));
    assert_ne!(field.as_deref(), Some("/artifact"), "{body}");
}

#[test]
fn atip_input13_12_an_empty_array_for_an_option_that_is_not_repeatable_is_refused_too() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    let mut checked = 0;
    for (words, properties) in leaves_with_input(repo) {
        // One key of each leaf that is no array and no flag-only: its value must not be an array.
        let Some((key, _)) = properties.iter().find(|(key, repeatable)| !*repeatable && key != "operands") else { continue };
        let mut argv: Vec<&str> = words.iter().map(String::as_str).collect();
        let document = json!({ key.clone(): [] }).to_string();
        argv.extend(["--input", &document, "--json"]);
        let (exit, code, reason, field, body) = refusal(&canon(repo, &argv));
        assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("INVALID_VALUE"), Some(format!("/{key}").as_str())), "{words:?} {key}: {body}");
        checked += 1;
    }
    assert!(checked >= 40, "a key of every leaf that takes a document was tried ({checked})");
    // The operands are refused the same way (they were before).
    let (exit, code, reason, field, body) = refusal(&canon(repo, &["finding", "close", "--input", r#"{"operands":[],"round":1,"seq":1,"disposition":"fixed"}"#, "--json"]));
    assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("INVALID_VALUE"), Some("/operands")), "{body}");
    assert_eq!(tree(repo), before, "every refusal wrote nothing");
}

// ------------------------------------------------------------------------------------------------ F6: several problems

#[test]
fn atip_input13_13_with_several_problems_field_is_the_first_details_entry_and_one_problem_has_no_details() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    for (document, order) in [(r#"{"roundd":1,"seq":"x","severity":"nope"}"#, ["/roundd", "/seq", "/severity"]), (r#"{"seq":"x","severity":"nope","roundd":1}"#, ["/seq", "/severity", "/roundd"])] {
        let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "add", "--input", document, "--json"]));
        assert_eq!((exit, code.as_str()), (Some(2), "INPUT_INVALID"), "{body}");
        let details: Vec<&str> = body["error"]["details"].as_array().expect("details").iter().map(|detail| detail["field"].as_str().unwrap()).collect();
        assert_eq!(details, order, "every problem, in the order the document met them: {body}");
        assert_eq!(field.as_deref(), Some(details[0]), "field is the first details entry: {body}");
    }
    // In a batch the same rule holds with the index in every pointer.
    // The document is written as text: a `json!` object would sort its keys.
    let document = r#"[{"change":"add-login","round":1,"seq":9,"severity":"note","actor":"bob","summary":"m"},{"seq":"x","roundd":1}]"#;
    let out = canon(repo, &["finding", "add", "--input", document, "--json"]);
    let (exit, body) = (out.status.code(), json_of(&out));
    assert_eq!(exit, Some(2), "{body}");
    let details: Vec<&str> = body["error"]["details"].as_array().expect("details").iter().map(|detail| detail["field"].as_str().unwrap()).collect();
    assert_eq!((details, body["error"]["field"].as_str()), (vec!["/1/seq", "/1/roundd"], Some("/1/seq")), "{body}");
    // One problem: no `details`.
    let (_, _, _, field, body) = refusal(&canon(repo, &["finding", "add", "--input", r#"{"seq":"x"}"#, "--json"]));
    assert_eq!(field.as_deref(), Some("/seq"), "{body}");
    assert!(body["error"].get("details").is_none(), "one problem has no details: {body}");
    assert_eq!(tree(repo), before);
}
