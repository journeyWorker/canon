//! `--help --json` and `--version --json` (`canon-0-36-help-json`, scenarios `atip.helpjson.*`), against the built binary:
//! both are refused with `USAGE` (`field` `--help` / `--version`) and a hint that points at `schema` (or at `canon --version`),
//! after the path is resolved, so an unknown word still answers `UNKNOWN_COMMAND`. Without `--json` the text is as it was.
//!
//! Tests are named by scenario id (`atip_helpjson_01_…` proves `atip.helpjson.01`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

fn canon(dir: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-helpjson").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().expect("canon runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The one JSON document on stdout, with nothing on stderr.
fn json_of(output: &Output) -> Value {
    assert!(output.stderr.is_empty(), "stderr is empty in JSON mode: {}", stderr(output));
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}", stdout(output)))
}

/// A git repo with canon's config and a change with one finding, to read nothing from and write nothing to.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    let run = |program: &str, args: &[&str]| {
        let out = Command::new(program).current_dir(path).args(args).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-helpjson").output().expect("spawn");
        assert!(out.status.success(), "{program} {args:?}: {}{}", stdout(&out), stderr(&out));
    };
    run("git", &["init", "-q"]);
    run(env!("CARGO_BIN_EXE_canon"), &["init", "--no-harness"]);
    run("git", &["add", "-A"]);
    run("git", &["-c", "user.name=canon", "-c", "user.email=canon@example.invalid", "commit", "-q", "-m", "fixture"]);
    run(env!("CARGO_BIN_EXE_canon"), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    run(env!("CARGO_BIN_EXE_canon"), &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    run(env!("CARGO_BIN_EXE_canon"), &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "bob", "--summary", "Naming nit"]);
    dir
}

/// Every file of `root` by path and bytes, `.git` left out.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            if rel == ".git" {
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

/// What a refusal says: `(exit, code, field, hint)`, with `meta.command` (absent at the root).
fn refusal(output: &Output) -> (Option<i32>, String, String, String, Option<String>) {
    let body = json_of(output);
    assert_eq!(body["ok"], false, "{body}");
    assert!(body.get("data").is_none(), "a refusal carries no data: {body}");
    let error = &body["error"];
    (output.status.code(), error["code"].as_str().unwrap_or_default().to_string(), error["field"].as_str().unwrap_or_default().to_string(), error["hint"].as_str().unwrap_or_default().to_string(), body["meta"]["command"].as_str().map(str::to_string))
}

/// The refusal of `--help --json`: exit 2, `USAGE`, `field` `--help`, and `hint` as given; `meta.command` is the resolved path.
fn assert_help_refused(dir: &Path, args: &[&str], hint: &str, command: Option<&str>) {
    let (exit, code, field, got_hint, got_command) = refusal(&canon(dir, args));
    assert_eq!((exit, code.as_str(), field.as_str(), got_hint.as_str(), got_command.as_deref()), (Some(2), "USAGE", "--help", hint, command), "{args:?}");
}

fn assert_version_refused(dir: &Path, args: &[&str], command: Option<&str>) {
    let (exit, code, field, hint, got_command) = refusal(&canon(dir, args));
    assert_eq!((exit, code.as_str(), field.as_str(), hint.as_str(), got_command.as_deref()), (Some(2), "USAGE", "--version", "canon --version", command), "{args:?}");
}

/// The answer `UNKNOWN_COMMAND` names the word and the words that would have parsed at its place.
fn assert_unknown_command(dir: &Path, args: &[&str], word: &str, expected_has: &str) {
    let out = canon(dir, args);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(2), "{args:?}: {body}");
    let error = &body["error"];
    assert_eq!(error["code"], "UNKNOWN_COMMAND", "{args:?}: {body}");
    assert_eq!(error["field"], "command", "{args:?}");
    assert_eq!(error["received"], word, "{args:?}");
    assert!(error["expected"].as_array().is_some_and(|words| words.iter().any(|w| w == expected_has)), "{args:?}: expected holds `{expected_has}`: {body}");
}

#[test]
fn atip_helpjson_01_help_with_json_on_a_command_is_refused_and_points_at_its_schema_call() {
    let dir = repo();
    for args in [&["finding", "close", "--json", "-h"][..], &["--json", "finding", "close", "-h"], &["--as", "reviewer", "finding", "close", "--help", "--json"]] {
        assert_help_refused(dir.path(), args, "canon schema finding close --json", Some("finding close"));
    }
}

#[test]
fn atip_helpjson_02_the_root_and_a_group_are_refused_with_the_schema_call_of_their_path() {
    let dir = repo();
    for args in [&["-h", "--json"][..], &["--json", "--help"]] {
        assert_help_refused(dir.path(), args, "canon schema --json", None);
    }
    assert_help_refused(dir.path(), &["--json", "finding", "-h"], "canon schema finding --json", Some("finding"));
    // A leaf that takes no required argument resolves the same way.
    assert_help_refused(dir.path(), &["status", "--help", "--json"], "canon schema status --json", Some("status"));
}

#[test]
fn atip_helpjson_03_a_word_after_the_end_of_options_is_no_request_and_text_help_is_text() {
    let dir = repo();
    // `--` ends the options: `--help` is the query, so this is a search, not a refusal.
    let out = canon(dir.path(), &["search", "--json", "--", "--help"]);
    let body = json_of(&out);
    assert_eq!((out.status.code(), &body["ok"], &body["data"]["query"]), (Some(0), &Value::Bool(true), &Value::from("--help")), "{body}");
    // Without --json the help is text and the exit is 0, with every spelling.
    for args in [&["finding", "close", "-h"][..], &["--help"], &["finding", "--help"]] {
        let out = canon(dir.path(), args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        assert!(serde_json::from_slice::<Value>(&out.stdout).is_err(), "{args:?} prints text, not a JSON document");
        assert!(out.stderr.is_empty(), "{args:?}: {}", stderr(&out));
    }
}

#[test]
fn atip_helpjson_04_an_unknown_word_answers_unknown_command_before_the_help_request() {
    let dir = repo();
    assert_unknown_command(dir.path(), &["--help", "nosuch", "--json"], "nosuch", "finding");
    assert_unknown_command(dir.path(), &["--json", "-h", "finding", "nosuch"], "nosuch", "close");
    assert_unknown_command(dir.path(), &["--help", "finding", "nosuch", "--json"], "nosuch", "close");
}

#[test]
fn atip_helpjson_05_version_with_json_is_refused_and_points_at_the_version_line() {
    let dir = repo();
    assert_version_refused(dir.path(), &["--json", "--version"], None);
    assert_version_refused(dir.path(), &["finding", "--version", "--json"], Some("finding"));
    assert_version_refused(dir.path(), &["finding", "close", "-V", "--json"], Some("finding close"));
}

#[test]
fn atip_helpjson_06_an_unknown_word_answers_before_the_version_request_and_text_version_is_one_line() {
    let dir = repo();
    assert_unknown_command(dir.path(), &["finding", "nosuch", "-V", "--json"], "nosuch", "close");
    let version = json_of(&canon(dir.path(), &["status", "--json"]))["meta"]["toolVersion"].as_str().unwrap().to_string();
    let out = canon(dir.path(), &["-V"]);
    assert_eq!((out.status.code(), stdout(&out)), (Some(0), format!("canon {version}\n")));
    assert!(out.stderr.is_empty(), "-V: {}", stderr(&out));
}

#[test]
fn atip_helpjson_07_the_refusal_reads_nothing_and_writes_nothing() {
    let outside = TempDir::new().unwrap();
    let inside = repo();
    for args in [&["finding", "close", "--help", "--json"][..], &["--version", "--json"], &["--help", "--json"]] {
        let before = tree(inside.path());
        let in_repo = canon(inside.path(), args);
        let out_of_repo = canon(outside.path(), args);
        assert_eq!(tree(inside.path()), before, "{args:?} left the repository as it was");
        assert!(tree(outside.path()).is_empty(), "{args:?} wrote nothing in a directory with no repository");
        let (inside_answer, outside_answer) = (json_of(&in_repo), json_of(&out_of_repo));
        assert_ne!(outside_answer["error"]["code"], "CONFIG_MISSING", "{args:?}: {outside_answer}");
        assert_eq!(inside_answer, outside_answer, "{args:?}: the same answer in and out of a repository");
    }
}

#[test]
fn atip_helpjson_08_text_help_keeps_its_first_line_and_the_help_command_stays_text() {
    let dir = repo();
    for args in [&["--help"][..], &["finding", "--help"], &["finding", "close", "--help"], &["schema", "-h"], &["help", "finding"], &["help", "finding", "--json"], &["--json", "help", "finding"]] {
        let out = canon(dir.path(), args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        let text = stdout(&out);
        assert!(text.contains("canon schema --json") && text.contains("canon skills show"), "{args:?}");
    }
}
