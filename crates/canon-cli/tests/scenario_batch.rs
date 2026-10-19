//! `canon scenario new --from <file|->`: one NDJSON line per scenario, all
//! validated before anything is written, against the built `canon` binary.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

fn run(repo: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn canon binary");
    let mut pipe = child.stdin.take().unwrap();
    if let Some(text) = stdin {
        pipe.write_all(text.as_bytes()).unwrap();
    }
    drop(pipe);
    child.wait_with_output().unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn ok(repo: &Path, args: &[&str]) {
    let out = run(repo, args, None);
    assert!(out.status.success(), "{args:?}: {}", stderr(&out));
}

/// A `canon init` repo with subject `demo` and change `add-login`.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["init", "--providers", "none"]);
    ok(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(dir.path(), &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    dir
}

const THREE: &str = "# the login scenarios\n\n\
{\"id\":\"auth.login.01\",\"title\":\"Sign in\",\"subject\":\"demo\",\"case\":\"happy\"}\n\
{\"id\":\"auth.login.02\",\"title\":\"Wrong password\",\"subject\":\"demo\",\"lane\":\"behavior\",\"case\":\"failure\"}\n\
{\"id\":\"@auth.session.01\",\"title\":\"Session expires\"}\n";

fn features(repo: &Path) -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut stack = vec![repo.join(".canon/specs")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "feature") {
                found.push((path.strip_prefix(repo).unwrap().to_path_buf(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    found.sort();
    found
}

/// The provenance timestamps differ between runs; blank them.
fn without_at(text: &str) -> String {
    text.lines()
        .map(|l| match l.find("\"at\":\"") {
            Some(i) => format!("{}\"at\":\"\"{}", &l[..i], l[i + 6..].split_once('"').map_or("", |(_, rest)| rest)),
            None => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_batch_writes_what_three_single_calls_write() {
    let batch = repo();
    let out = run(batch.path(), &["scenario", "new", "--from", "-", "--json"], Some(THREE));
    assert!(out.status.success(), "{}", stderr(&out));
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    let items = json["data"]["items"].as_array().unwrap();
    assert_eq!(items.iter().map(|i| (i["line"].as_u64().unwrap(), i["id"].as_str().unwrap())).collect::<Vec<_>>(), vec![(3, "auth.login.01"), (4, "auth.login.02"), (5, "auth.session.01")]);
    assert!(items[0]["path"].as_str().unwrap().ends_with("auth/login.feature"), "{items:?}");
    assert!(json["meta"].get("next").is_some(), "one meta.next for the batch: {json}");

    let single = repo();
    ok(single.path(), &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject-id", "demo", "--case", "happy"]);
    ok(single.path(), &["scenario", "new", "auth.login.02", "--title", "Wrong password", "--subject-id", "demo", "--lane", "behavior", "--case", "failure"]);
    ok(single.path(), &["scenario", "new", "auth.session.01", "--title", "Session expires"]);

    let (a, b) = (features(batch.path()), features(single.path()));
    assert_eq!(a.len(), 2);
    assert_eq!(a.iter().map(|(p, t)| (p.clone(), without_at(t))).collect::<Vec<_>>(), b.iter().map(|(p, t)| (p.clone(), without_at(t))).collect::<Vec<_>>());
}

#[test]
fn a_file_source_works_like_stdin() {
    let dir = repo();
    std::fs::write(dir.path().join("batch.ndjson"), THREE).unwrap();
    let out = run(dir.path(), &["scenario", "new", "--from", "batch.ndjson"], None);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(features(dir.path()).len(), 2);
}

#[test]
fn one_bad_line_writes_nothing_and_names_that_line() {
    let dir = repo();
    let batch = "{\"id\":\"auth.login.01\",\"title\":\"Sign in\"}\n\
{\"id\":\"auth.login.02\",\"title\":\"Wrong password\",\"subject\":\"demo\",\"flow\":\"F9\"}\n\
{\"id\":\"auth.login.03\",\"title\":\"Locked out\"}\n";
    let out = run(dir.path(), &["scenario", "new", "--from", "-", "--json"], Some(batch));
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(features(dir.path()).is_empty(), "nothing may be written");
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["error"]["code"], "SCAFFOLD_INVALID_ARGUMENT");
    let items = json["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{json}");
    assert_eq!((items[0]["line"].as_u64(), items[0]["id"].as_str(), items[0]["error"]["code"].as_str(), items[0]["error"]["field"].as_str()), (Some(2), Some("auth.login.02"), Some("SCAFFOLD_INVALID_ARGUMENT"), Some("flow")));
}

#[test]
fn a_repeated_id_in_the_batch_is_refused() {
    let dir = repo();
    let batch = "{\"id\":\"auth.login.01\",\"title\":\"Sign in\"}\n{\"id\":\"auth.login.01\",\"title\":\"Sign in again\"}\n";
    let out = run(dir.path(), &["scenario", "new", "--from", "-", "--json"], Some(batch));
    assert_eq!(out.status.code(), Some(2));
    assert!(features(dir.path()).is_empty());
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    let items = json["data"]["items"].as_array().unwrap();
    assert_eq!((items.len(), items[0]["line"].as_u64(), items[0]["error"]["code"].as_str()), (1, Some(2), Some("SCENARIO_TAG_EXISTS")));
}

#[test]
fn an_existing_id_and_malformed_lines_are_each_reported_and_nothing_is_written() {
    let dir = repo();
    ok(dir.path(), &["scenario", "new", "auth.login.01", "--title", "Sign in"]);
    let before = features(dir.path());
    let batch = "{\"id\":\"auth.login.01\",\"title\":\"Again\"}\nnot json\n{\"id\":\"Bad Id\",\"title\":\"x\"}\n{\"id\":\"auth.login.05\",\"title\":\"x\",\"bogus\":1}\n{\"id\":\"auth.login.06\",\"title\":\"fine\"}\n";
    let out = run(dir.path(), &["scenario", "new", "--from", "-", "--json"], Some(batch));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(features(dir.path()), before);
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    let lines: Vec<u64> = json["data"]["items"].as_array().unwrap().iter().map(|i| i["line"].as_u64().unwrap()).collect();
    assert_eq!(lines, vec![1, 2, 3, 4]);
}

#[test]
fn from_excludes_the_single_form_and_an_empty_batch_is_refused() {
    let dir = repo();
    let out = run(dir.path(), &["scenario", "new", "auth.login.01", "--from", "-"], Some(THREE));
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    let out = run(dir.path(), &["scenario", "new", "--from", "-", "--title", "x"], Some(THREE));
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    let out = run(dir.path(), &["scenario", "new", "--from", "-"], Some("# only a comment\n\n"));
    assert_eq!(out.status.code(), Some(2));
    assert!(features(dir.path()).is_empty());
}
