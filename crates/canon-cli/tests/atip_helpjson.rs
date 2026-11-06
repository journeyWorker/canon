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
    for args in [&["finding", "close", "--help", "--json"][..], &["finding", "close", "--json", "-h"], &["-h", "--json", "finding", "close"], &["--json", "finding", "close", "-h"], &["--as", "reviewer", "finding", "close", "--help", "--json"]] {
        assert_help_refused(dir.path(), args, "canon schema finding close --json", Some("finding close"));
    }
}

#[test]
fn atip_helpjson_02_the_root_and_a_group_are_refused_with_the_schema_call_of_their_path() {
    let dir = repo();
    for args in [&["--help", "--json"][..], &["-h", "--json"], &["--json", "--help"]] {
        assert_help_refused(dir.path(), args, "canon schema --json", None);
    }
    for args in [&["finding", "--help", "--json"][..], &["--json", "finding", "-h"]] {
        assert_help_refused(dir.path(), args, "canon schema finding --json", Some("finding"));
    }
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
    // `--json` after `--` is a value, not the switch: the call is a text-mode usage error about `--json`, never about `--help`.
    let out = canon(dir.path(), &["search", "--", "--help", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "text mode prints nothing on stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("field: --json") && !stderr(&out).contains("field: --help"), "{}", stderr(&out));
    // Without --json the help is text and the exit is 0, with every spelling.
    for args in [&["finding", "close", "--help"][..], &["finding", "close", "-h"], &["--help"], &["finding", "--help"]] {
        let out = canon(dir.path(), args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        assert!(serde_json::from_slice::<Value>(&out.stdout).is_err(), "{args:?} prints text, not a JSON document");
        assert!(out.stderr.is_empty(), "{args:?}: {}", stderr(&out));
    }
}

#[test]
fn atip_helpjson_04_an_unknown_word_answers_unknown_command_before_the_help_request() {
    let dir = repo();
    assert_unknown_command(dir.path(), &["nosuch", "--help", "--json"], "nosuch", "finding");
    assert_unknown_command(dir.path(), &["finding", "nosuch", "--help", "--json"], "nosuch", "close");
    assert_unknown_command(dir.path(), &["--help", "nosuch", "--json"], "nosuch", "finding");
    assert_unknown_command(dir.path(), &["--json", "-h", "finding", "nosuch"], "nosuch", "close");
    assert_unknown_command(dir.path(), &["--help", "finding", "nosuch", "--json"], "nosuch", "close");
}

#[test]
fn atip_helpjson_05_version_with_json_is_refused_and_points_at_the_version_line() {
    let dir = repo();
    for args in [&["--version", "--json"][..], &["-V", "--json"], &["--json", "--version"]] {
        assert_version_refused(dir.path(), args, None);
    }
    assert_version_refused(dir.path(), &["finding", "--version", "--json"], Some("finding"));
    assert_version_refused(dir.path(), &["finding", "close", "-V", "--json"], Some("finding close"));
    // The refusal does not print the version outside `meta.toolVersion`, which every answer carries.
    let out = canon(dir.path(), &["--version", "--json"]);
    let body = json_of(&out);
    let version = body["meta"]["toolVersion"].as_str().unwrap().to_string();
    assert_eq!(stdout(&out).matches(&version).count(), 1, "the version appears once, as meta.toolVersion: {body}");
}

#[test]
fn atip_helpjson_06_an_unknown_word_answers_before_the_version_request_and_text_version_is_one_line() {
    let dir = repo();
    assert_unknown_command(dir.path(), &["nosuch", "--version", "--json"], "nosuch", "finding");
    assert_unknown_command(dir.path(), &["finding", "nosuch", "-V", "--json"], "nosuch", "close");
    let version = json_of(&canon(dir.path(), &["status", "--json"]))["meta"]["toolVersion"].as_str().unwrap().to_string();
    for flag in ["--version", "-V"] {
        let out = canon(dir.path(), &[flag]);
        assert_eq!((out.status.code(), stdout(&out)), (Some(0), format!("canon {version}\n")), "{flag}");
        assert!(out.stderr.is_empty(), "{flag}: {}", stderr(&out));
    }
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
        assert_eq!(in_repo.status.code(), Some(2), "{args:?}");
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
        assert_eq!(text.lines().next(), Some(canon_cli::spec::HELP_POINTER), "{args:?}");
        assert!(text.contains("canon schema --json") && text.contains("canon skills show"), "{args:?}");
    }
}

/// The tracked text files that may name `--help` with `--json` on one line: the changelog tells the old behavior, the briefs and the
/// research notes record it, the specs and tests describe the refusal, and the ledger is data.
fn may_name_help_json(path: &str) -> bool {
    path == "CHANGELOG.md" || ["docs/research/", ".canon/", "crates/canon-cli/tests/", "conformance/"].iter().any(|prefix| path.starts_with(prefix))
}

/// Whether `line` recommends `--help` (or `-h`) with `--json` as a call: one quoted or backticked string (or a whole line of a code block) of
/// `canon … --help … --json`.
fn recommends_help_json(line: &str) -> bool {
    line.split(['`', '"', '\'']).any(|call| call.contains("canon ") && (call.contains("--help") || call.contains(" -h")) && call.contains("--json"))
}

#[test]
fn atip_helpjson_09_no_text_of_the_repository_recommends_help_with_json() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let listed = Command::new("git").current_dir(&root).args(["ls-files", "-z"]).output().expect("git ls-files");
    assert!(listed.status.success(), "{}", stderr(&listed));
    let mut seen = 0;
    let mut found = Vec::new();
    for path in String::from_utf8(listed.stdout).unwrap().split('\0').filter(|path| !path.is_empty() && !may_name_help_json(path)) {
        let Ok(text) = std::fs::read_to_string(root.join(path)) else { continue };
        seen += 1;
        for (number, line) in text.lines().enumerate() {
            // One call is one quoted or backticked string (or a whole line of a code block): `canon … --help … --json`.
            if recommends_help_json(line) {
                found.push(format!("{path}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    assert!(seen > 100, "the search read the repository's text files ({seen})");
    assert!(found.is_empty(), "these lines name `--help` with `--json`; the machine description is `canon schema`:\n{}", found.join("\n"));
}

/// What is wrong with the text help `text` a person gets from `--help` (the first line points at `canon schema --json` and `canon skills show`).
fn text_help_problems(text: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if text.lines().next() != Some(canon_cli::spec::HELP_POINTER) {
        problems.push("the first line does not point at schema and skills show".to_string());
    }
    if text.trim_start().starts_with('{') {
        problems.push("the text help is a JSON document".to_string());
    }
    problems
}

/// atip.helpjson.10 (the failure path of F4): a guide, script or page that recommends `--help --json` (or `-h`, in any order) as the machine
/// description is found by the repository guard; a text help or version that lost its pointer, became JSON or grew a second line is found too; the
/// text answers of the built binary pass both.
#[test]
fn atip_helpjson_10_a_guide_that_recommends_help_with_json_or_a_text_help_that_lost_its_pointer_is_found() {
    for recommended in [
        "Run `canon finding close --help --json` to see the options.",
        "    canon --json --help",
        "call \"canon -h finding --json\" first",
        "$ canon schema --json && canon search --help --json",
    ] {
        assert!(recommends_help_json(recommended), "a recommendation goes unseen: {recommended}");
    }
    for fine in ["Run `canon schema finding close --json` to see the options.", "`canon finding close --help` prints the text help", "canon --version prints one line", "pass --help --json to your own tool"] {
        assert!(!recommends_help_json(fine), "a line that recommends nothing is flagged: {fine}");
    }
    assert_eq!(text_help_problems("Usage: canon [OPTIONS]\n"), ["the first line does not point at schema and skills show"]);
    assert_eq!(text_help_problems("{\"ok\":false}"), ["the first line does not point at schema and skills show", "the text help is a JSON document"]);
    let dir = repo();
    for args in [&["--help"][..], &["-h"], &["finding", "--help"], &["finding", "close", "-h"]] {
        let out = canon(dir.path(), args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        assert_eq!(text_help_problems(&stdout(&out)), Vec::<String>::new(), "{args:?}");
    }
    // A request for text that names no command still fails, and the version line is exactly one line.
    let out = canon(dir.path(), &["nosuch", "--help"]);
    assert_ne!(out.status.code(), Some(0), "an unknown command asked for help is no help: {}", stdout(&out));
    for flag in ["--version", "-V"] {
        assert_eq!(stdout(&canon(dir.path(), &[flag])).lines().count(), 1, "{flag}");
    }
}
