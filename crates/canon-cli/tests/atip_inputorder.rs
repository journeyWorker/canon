//! The order of spec 11.7 for `--input` (`canon-0-35-input-order`, scenarios `atip.inputorder.*`), against the
//! built binary: framing without comments, every problem of a document in its key order, enumerations as
//! `INPUT_INVALID`, the command line's own parse failure before the conflict check, integers compared by the
//! number they denote, and an environment variable that never touches a key the document gives.
//!
//! Tests are named by scenario id (`atip_inputorder_01_…` proves `atip.inputorder.01`).

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};
use tempfile::TempDir;

fn canon_with(dir: &Path, args: &[&str], stdin: Option<&str>, env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-inputorder").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("spawn canon");
    let mut pipe = child.stdin.take().expect("stdin");
    if let Some(text) = stdin {
        pipe.write_all(text.as_bytes()).expect("write stdin");
    }
    drop(pipe);
    child.wait_with_output().expect("canon runs")
}

fn canon(dir: &Path, args: &[&str]) -> Output {
    canon_with(dir, args, None, &[])
}

fn json_of(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn ok(dir: &Path, args: &[&str]) {
    let out = canon(dir, args);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// `(exit, code, reason, field)` of a refusal and the whole answer.
fn refusal(output: &Output) -> (Option<i32>, String, Option<String>, Option<String>, Value) {
    let body = json_of(output);
    let error = &body["error"];
    (output.status.code(), error["code"].as_str().unwrap_or_default().to_string(), error["reason"].as_str().map(str::to_string), error["field"].as_str().map(str::to_string), body)
}

/// A git repo holding the story of the examples: scenario `auth.login.01`, change `add-login`, finding 1.
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
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(repo, &["inventory", "sync"]);
    ok(repo, &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "bob", "--summary", "Naming nit"]);
    dir
}

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

const SUBJECT: &str = r#"{"operands":["NAME"],"domain":"dev","title":"T"}"#;

fn subject_doc(name: &str) -> String {
    SUBJECT.replace("NAME", name)
}

#[test]
fn atip_inputorder_01_the_non_empty_lines_of_an_ndjson_file_are_all_documents() {
    let dir = fixture();
    let repo = dir.path();
    std::fs::write(repo.join("batch.ndjson"), format!("\n{}\n\n\n{}\n  \n{}\n", subject_doc("one"), subject_doc("two"), subject_doc("three"))).unwrap();
    let out = canon(repo, &["subject", "new", "--input", "batch.ndjson", "--json"]);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(0), "{body}");
    let items = body["data"]["items"].as_array().expect("a batch report");
    assert_eq!(items.iter().map(|item| item["status"].as_str().unwrap()).collect::<Vec<_>>(), ["written", "written", "written"], "{body}");
    assert_eq!(items.iter().map(|item| item["data"]["subject_id"].as_str().unwrap()).collect::<Vec<_>>(), ["one", "two", "three"], "{body}");
}

#[test]
fn atip_inputorder_02_a_hash_line_is_not_json_and_the_other_framing_refusals_stay() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    let line = subject_doc("x");
    for (name, content) in [("first.ndjson", format!("# subjects\n{line}\n")), ("middle.ndjson", format!("{line}\n# between\n{line}\n")), ("only.ndjson", "# nothing but a comment\n".to_string())] {
        std::fs::write(repo.join(name), content).unwrap();
        let out = canon(repo, &["subject", "new", "--input", name, "--json"]);
        let (exit, code, reason, field, body) = refusal(&out);
        assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("NOT_JSON"), Some("--input")), "{name}: {body}");
    }
    let stdin = canon_with(repo, &["subject", "new", "--input", "-", "--json"], Some(&format!("{line}\n# comment\n")), &[]);
    let (exit, code, reason, field, body) = refusal(&stdin);
    assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("NOT_JSON"), Some("--input")), "stdin: {body}");

    std::fs::write(repo.join("number.json"), "5\n").unwrap();
    std::fs::write(repo.join("blank.ndjson"), "\n  \n").unwrap();
    std::fs::write(repo.join("big.ndjson"), format!("{}\n", "x".repeat(8 * 1024 * 1024 + 1))).unwrap();
    for (what, args, expected) in [
        ("a bare number", vec!["--input", "number.json"], ("INPUT_INVALID", Some("NOT_OBJECT"))),
        ("a file that is not there", vec!["--input", "missing.ndjson"], ("INPUT_INVALID", Some("UNREADABLE"))),
        ("a source with no line", vec!["--input", "blank.ndjson"], ("INPUT_INVALID", Some("EMPTY"))),
        ("a source over the byte limit", vec!["--input", "big.ndjson"], ("INPUT_TOO_LARGE", Some("TOO_MANY_BYTES"))),
    ] {
        let mut argv = vec!["subject", "new"];
        argv.extend(args);
        argv.push("--json");
        let (exit, code, reason, _, body) = refusal(&canon(repo, &argv));
        assert_eq!((exit, code.as_str(), reason.as_deref()), (Some(2), expected.0, expected.1), "{what}: {body}");
    }
    for name in ["number.json", "blank.ndjson", "big.ndjson", "first.ndjson", "middle.ndjson", "only.ndjson"] {
        std::fs::remove_file(repo.join(name)).unwrap();
    }
    assert_eq!(tree(repo).into_iter().filter(|(path, _)| !path.ends_with(".ndjson") && !path.ends_with(".json")).collect::<BTreeMap<_, _>>(), before.into_iter().filter(|(path, _)| !path.ends_with(".ndjson") && !path.ends_with(".json")).collect::<BTreeMap<_, _>>(), "a refused document writes nothing");
}

#[test]
fn atip_inputorder_03_every_problem_of_a_document_is_reported_in_its_key_order() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    let out = canon(repo, &["finding", "close", "--input", r#"{"roundd":1,"seq":"x"}"#, "--json"]);
    let (exit, code, reason, field, body) = refusal(&out);
    assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("UNKNOWN_KEY"), Some("/roundd")), "{body}");
    let details = body["error"]["details"].as_array().expect("several problems carry details");
    assert_eq!(details.iter().map(|d| (d["field"].as_str().unwrap(), d["reason"].as_str().unwrap(), d["code"].as_str().unwrap())).collect::<Vec<_>>(), [("/roundd", "UNKNOWN_KEY", "INPUT_INVALID"), ("/seq", "INVALID_VALUE", "INPUT_INVALID")], "{body}");
    assert!(details.iter().all(|d| d["message"].as_str().is_some_and(|m| !m.is_empty()) && d.get("retryable").is_none() && d.get("details").is_none()), "{body}");

    // the other order of the same keys is the other order of the problems, and the first one is the error's
    let out = canon(repo, &["finding", "close", "--input", r#"{"seq":"x","roundd":1}"#, "--json"]);
    let (_, code, reason, field, body) = refusal(&out);
    assert_eq!((code.as_str(), reason.as_deref(), field.as_deref()), ("INPUT_INVALID", Some("INVALID_VALUE"), Some("/seq")), "{body}");
    assert_eq!(body["error"]["details"].as_array().unwrap().iter().map(|d| d["field"].as_str().unwrap()).collect::<Vec<_>>(), ["/seq", "/roundd"], "{body}");

    // not alphabetical: `zeta` before `alpha` stays in front
    let out = canon(repo, &["finding", "close", "--input", r#"{"zeta":1,"alpha":2,"round":1}"#, "--json"]);
    let body = json_of(&out);
    assert_eq!(body["error"]["details"].as_array().unwrap().iter().map(|d| d["field"].as_str().unwrap()).collect::<Vec<_>>(), ["/zeta", "/alpha"], "{body}");

    // in a batch too, each item in its own prefix
    let out = canon(repo, &["finding", "close", "--input", r#"[{"round":1,"bogus":1,"seq":"x"}]"#, "--json"]);
    let body = json_of(&out);
    assert_eq!(body["error"]["field"], "/items/0/bogus", "{body}");
    assert_eq!(body["error"]["details"].as_array().unwrap().iter().map(|d| d["field"].as_str().unwrap()).collect::<Vec<_>>(), ["/items/0/bogus", "/items/0/seq"], "{body}");
    assert_eq!(tree(repo), before);

    // the text form lists them too
    let out = canon(repo, &["finding", "close", "--input", r#"{"roundd":1,"seq":"x"}"#]);
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("problem: /roundd") && text.contains("problem: /seq"), "{text}");
}

#[test]
fn atip_inputorder_04_one_problem_has_no_details_and_a_right_document_is_not_refused() {
    let dir = fixture();
    let repo = dir.path();
    let out = canon(repo, &["finding", "close", "--input", r#"{"roundd":1}"#, "--json"]);
    let body = json_of(&out);
    assert_eq!(body["error"]["code"], "INPUT_INVALID", "{body}");
    assert!(body["error"].get("details").is_none(), "one problem is the error alone: {body}");

    // every key right: no document problem (the invocation then runs)
    let out = canon(repo, &["finding", "close", "--input", r#"{"operands":["add-login"],"round":1,"seq":1,"disposition":"open"}"#, "--json"]);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(0), "{body}");

    // a null, an unknown key and a flag-only key: three problems in the document's order
    let out = canon(repo, &["finding", "close", "--input", r#"{"round":null,"bogus":1,"json":true,"seq":1}"#, "--json"]);
    let body = json_of(&out);
    let details = body["error"]["details"].as_array().expect("three problems");
    assert_eq!(details.iter().map(|d| (d["field"].as_str().unwrap(), d["reason"].as_str().unwrap())).collect::<Vec<_>>(), [("/round", "INVALID_VALUE"), ("/bogus", "UNKNOWN_KEY"), ("/json", "FLAG_ONLY_KEY")], "{body}");
    assert_eq!(body["error"]["field"], "/round", "{body}");

    // an operand array with a bad element and an empty one
    let out = canon(repo, &["finding", "close", "--input", r#"{"operands":[]}"#, "--json"]);
    let (exit, code, reason, field, body) = refusal(&out);
    assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("INVALID_VALUE"), Some("/operands")), "{body}");
}

#[test]
fn atip_inputorder_05_a_value_outside_an_enumeration_is_invalid_input_and_on_argv_is_usage() {
    let dir = fixture();
    let repo = dir.path();
    let out = canon(repo, &["finding", "close", "--input", r#"{"disposition":"nope"}"#, "--json"]);
    let (exit, code, reason, field, body) = refusal(&out);
    assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("INVALID_VALUE"), Some("/disposition")), "{body}");
    let expected: Vec<&str> = body["error"]["expected"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(expected, ["open", "fixed", "rejected", "deferred"], "{body}");

    let out = canon(repo, &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "nope", "--json"]);
    let (exit, code, _, field, body) = refusal(&out);
    assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "USAGE", Some("--disposition")), "{body}");

    // the value of another type than a string, and in a batch
    let out = canon(repo, &["finding", "close", "--input", r#"{"disposition":3}"#, "--json"]);
    let (_, code, reason, field, body) = refusal(&out);
    assert_eq!((code.as_str(), reason.as_deref(), field.as_deref()), ("INPUT_INVALID", Some("INVALID_VALUE"), Some("/disposition")), "{body}");
    // preflight takes the elements in order: element 0 is whole, element 1 holds the problem
    let out = canon(repo, &["finding", "close", "--input", r#"[{"operands":["add-login"],"round":1,"seq":1,"disposition":"open"},{"disposition":"nope"}]"#, "--json"]);
    let (exit, code, reason, field, body) = refusal(&out);
    assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("INVALID_VALUE"), Some("/items/1/disposition")), "{body}");
}

#[test]
fn atip_inputorder_06_the_command_line_that_does_not_parse_answers_before_the_conflict_check() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    for (argv, field) in [
        (vec!["finding", "close", "add-login", "--round", "abc", "--input", r#"{"round":1}"#, "--json"], "--round"),
        (vec!["finding", "close", "add-login", "--disposition", "bogus", "--input", r#"{"disposition":"open"}"#, "--json"], "--disposition"),
        (vec!["finding", "close", "add-login", "--round", "abc", "--input", r#"[{"round":1}]"#, "--json"], "--round"),
    ] {
        let (exit, code, _, found, body) = refusal(&canon(repo, &argv));
        assert_eq!((exit, code.as_str(), found.as_deref()), (Some(2), "USAGE", Some(field)), "{argv:?}: {body}");
    }
    // an unknown option answers first as well, and the earliest argv position answers among several
    let (_, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "--bogus", "--round", "abc", "--input", r#"{"round":1}"#, "--json"]));
    assert_eq!((code.as_str(), field.as_deref()), ("USAGE", Some("--bogus")), "{body}");
    let (_, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "--seq", "abc", "--round", "abc", "--input", r#"{"round":1}"#, "--json"]));
    assert_eq!((code.as_str(), field.as_deref()), ("USAGE", Some("--seq")), "{body}");
    // a document problem is step 2: it answers before the argv's
    let (_, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "--round", "abc", "--input", r#"{"seq":"x"}"#, "--json"]));
    assert_eq!((code.as_str(), field.as_deref()), ("INPUT_INVALID", Some("/seq")), "{body}");
    assert_eq!(tree(repo), before);
}

#[test]
fn atip_inputorder_07_a_value_given_two_ways_that_differ_is_a_conflict_at_the_document_pointer() {
    let dir = fixture();
    let repo = dir.path();
    let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "open", "--input", r#"{"round":2}"#, "--json"]));
    assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "INPUT_CONFLICT", Some("/round")), "{body}");
    let (_, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "open", "--input", r#"{"disposition":"fixed"}"#, "--json"]));
    assert_eq!((code.as_str(), field.as_deref()), ("INPUT_CONFLICT", Some("/disposition")), "{body}");
    let (_, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "open", "--input", r#"[{"round":2}]"#, "--json"]));
    assert_eq!((code.as_str(), field.as_deref()), ("INPUT_CONFLICT", Some("/items/0/round")), "{body}");
    let (_, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "--seq", "1", "--disposition", "open", "--input", r#"{"operands":["other"],"round":1}"#, "--json"]));
    assert_ne!(code, "INPUT_CONFLICT", "{body}");
    let _ = field;

    // the same value on both sides, the argv alone and the document alone are not a conflict
    let out = canon(repo, &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "open", "--input", r#"{"round":1,"disposition":"open"}"#, "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", json_of(&out));
    let out = canon(repo, &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "open", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", json_of(&out));
    let out = canon(repo, &["finding", "close", "--input", &json!({"operands": ["add-login"], "round": 1, "seq": 1, "disposition": "open"}).to_string(), "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", json_of(&out));
}

#[test]
fn atip_inputorder_08_integers_are_compared_by_the_number_they_denote() {
    let dir = fixture();
    let repo = dir.path();
    // argv 1 against 1, "1" and "01"-free spellings; `+1` on argv is still accepted and equals 1
    for (argv_round, doc) in [("1", r#"{"round":1}"#), ("1", r#"{"round":"1"}"#), ("+1", r#"{"round":1}"#), ("+1", r#"{"round":"1"}"#)] {
        let out = canon(repo, &["finding", "close", "add-login", "--round", argv_round, "--seq", "1", "--disposition", "open", "--input", doc, "--json"]);
        assert_eq!(out.status.code(), Some(0), "--round {argv_round} with {doc}: {}", json_of(&out));
    }
    // zero, which `divergence add --round` takes: -0, "-0", 0 and "0" are one value
    for doc in [r#"{"round":-0}"#, r#"{"round":"-0"}"#, r#"{"round":0}"#, r#"{"round":"0"}"#] {
        let out = canon(repo, &["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--round", "0", "--input", doc, "--json"]);
        assert_eq!(out.status.code(), Some(0), "--round 0 with {doc}: {}", json_of(&out));
        let out = canon(repo, &["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--round", "+0", "--input", doc, "--json"]);
        assert_eq!(out.status.code(), Some(0), "--round +0 with {doc}: {}", json_of(&out));
    }
    // a different integer is still a conflict, however it is written
    let (_, code, _, field, body) = refusal(&canon(repo, &["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--round", "1", "--input", r#"{"round":"2"}"#, "--json"]));
    assert_eq!((code.as_str(), field.as_deref()), ("INPUT_CONFLICT", Some("/round")), "{body}");
    let (_, code, _, field, body) = refusal(&canon(repo, &["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--round", "0", "--input", r#"{"round":-1}"#, "--json"]));
    assert_eq!((code.as_str(), field.as_deref()), ("INPUT_CONFLICT", Some("/round")), "{body}");
}

#[test]
fn atip_inputorder_09_an_integer_spelling_the_contract_does_not_know_is_invalid_input() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    for round in [r#""+1""#, r#""0x1""#, r#""1e3""#, r#"" 1""#, r#""1 ""#, r#""01""#, r#""1.0""#, "1.0", "1e3", "9007199254740992", r#""9007199254740992""#, "-9007199254740992", "true", "[1]"] {
        let doc = format!(r#"{{"round":{round}}}"#);
        let (exit, code, reason, field, body) = refusal(&canon(repo, &["finding", "close", "--input", &doc, "--json"]));
        assert_eq!((exit, code.as_str(), reason.as_deref(), field.as_deref()), (Some(2), "INPUT_INVALID", Some("INVALID_VALUE"), Some("/round")), "{doc}: {body}");
    }
    // the largest exact integer is a number of the document; the command's own domain (step 6) then speaks, not the input
    let (_, code, _, field, body) = refusal(&canon(repo, &["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--input", r#"{"round":9007199254740991}"#, "--json"]));
    assert_eq!((code.as_str(), field.as_deref()), ("USAGE", Some("/round")), "{body}");
    let (exit, code, _, field, body) = refusal(&canon(repo, &["finding", "close", "add-login", "--round", "9007199254740992", "--seq", "1", "--disposition", "open", "--json"]));
    assert_eq!((exit, code.as_str(), field.as_deref()), (Some(2), "USAGE", Some("--round")), "{body}");
    // a number is not a string
    let (_, code, reason, field, body) = refusal(&canon(repo, &["subject", "new", "--input", r#"{"operands":["a"],"domain":"dev","title":5}"#, "--json"]));
    assert_eq!((code.as_str(), reason.as_deref(), field.as_deref()), ("INPUT_INVALID", Some("INVALID_VALUE"), Some("/title")), "{body}");
    assert_eq!(tree(repo), before);
}

#[test]
fn atip_inputorder_10_the_environment_never_touches_a_key_the_document_gives() {
    let dir = fixture();
    let repo = dir.path();
    let feature = ".canon/specs/kind=feature/area=auth/login.feature";
    let with_actor = json!({"operands": ["auth.login.02"], "title": "Two", "feature": feature, "actor": "alice"}).to_string();
    let out = canon_with(repo, &["scenario", "new", "--input", &with_actor, "--json"], None, &[("CANON_ACTOR", "bob")]);
    assert_eq!(out.status.code(), Some(0), "{}", json_of(&out));
    let without_actor = json!({"operands": ["auth.login.03"], "title": "Three", "feature": feature}).to_string();
    let out = canon_with(repo, &["scenario", "new", "--input", &without_actor, "--json"], None, &[("CANON_ACTOR", "bob")]);
    assert_eq!(out.status.code(), Some(0), "{}", json_of(&out));
    let text = std::fs::read_to_string(repo.join(feature)).unwrap();
    let provenance = |tag: &str| {
        let at = text.find(&format!("@{tag}")).unwrap_or_else(|| panic!("{tag} is in the feature:\n{text}"));
        text[..at].lines().rev().find(|line| line.contains("# canon:")).unwrap().to_string()
    };
    assert!(provenance("auth.login.02").contains(r#""agent_id":"alice""#), "the document's actor: {}", provenance("auth.login.02"));
    assert!(provenance("auth.login.03").contains(r#""agent_id":"bob""#), "the environment fills what the document lacks: {}", provenance("auth.login.03"));
    // the same without a document, for the contrast: the environment is the actor
    let out = canon_with(repo, &["scenario", "new", "auth.login.04", "--title", "Four", "--feature", feature, "--json"], None, &[("CANON_ACTOR", "carol")]);
    assert_eq!(out.status.code(), Some(0), "{}", json_of(&out));
    // and the environment does not conflict with the argv's explicit value either
    let out = canon_with(repo, &["scenario", "new", "auth.login.05", "--title", "Five", "--feature", feature, "--actor", "dave", "--input", r#"{"actor":"dave"}"#, "--json"], None, &[("CANON_ACTOR", "carol")]);
    assert_eq!(out.status.code(), Some(0), "{}", json_of(&out));
}
