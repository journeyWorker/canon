//! `--repo` discovery and the `CONFIG_MISSING` refusal (#109).
//!
//! `--repo` absent means the nearest ancestor of the working directory that
//! holds a `canon.yaml`; any value, `.` included, is that directory as given. A
//! command that needs canon state and finds no `canon.yaml` refuses with
//! `CONFIG_MISSING` (exit 2) instead of grading an empty repo as clean.
//!
//! These tests drive the built binary only, so the same compiled test can be
//! pointed at an older `canon` to show it fails there.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn canon(dir: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(args)
        .current_dir(dir)
        .env("CANON_ACTOR", "repo-discovery")
        .env("HOME", home)
        .env("CANON_SKILLS_SOURCE", concat!(env!("CARGO_MANIFEST_DIR"), "/../../canon/skills"))
        .output()
        .expect("spawn canon binary")
}

fn ok(dir: &Path, home: &Path, args: &[&str]) {
    let out = canon(dir, home, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The `error` object of a `--json` refusal.
fn error_of(out: &Output) -> Value {
    serde_json::from_slice::<Value>(&out.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}", stdout(out)))["error"].clone()
}

/// A canon repo with one subject, so `status` has something to report.
fn repo(home: &Path) -> TempDir {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), home, &["init", "--providers", "none"]);
    ok(dir.path(), home, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    dir
}

#[test]
fn gate_check_in_a_directory_without_canon_yaml_refuses_instead_of_reporting_clean() {
    let home = TempDir::new().unwrap();
    let empty = TempDir::new().unwrap();

    let out = canon(empty.path(), home.path(), &["gate", "check", "--json"]);

    assert_eq!(out.status.code(), Some(2), "stdout: {} stderr: {}", stdout(&out), stderr(&out));
    let error = error_of(&out);
    assert_eq!(error["code"], "CONFIG_MISSING", "{error}");
    assert_eq!(error["field"], "--repo", "{error}");
    let hint = error["hint"].as_str().unwrap();
    assert!(hint.contains("canon init"), "the hint names the next step: {hint}");
    assert!(hint.contains(empty.path().file_name().unwrap().to_str().unwrap()), "the hint names the directory searched: {hint}");
    assert!(!stdout(&out).contains("clean"), "a refusal is not a clean gate: {}", stdout(&out));
}

#[test]
fn gate_check_with_a_repo_that_has_no_canon_yaml_refuses_even_inside_a_canon_repo() {
    let home = TempDir::new().unwrap();
    let root = repo(home.path());
    let sub = root.path().join("subdir-without-config");
    std::fs::create_dir_all(&sub).unwrap();

    // An explicit directory is that directory as given: no walk up to the root's canon.yaml.
    let out = canon(root.path(), home.path(), &["gate", "check", "--repo", sub.to_str().unwrap(), "--json"]);

    assert_eq!(out.status.code(), Some(2), "stdout: {} stderr: {}", stdout(&out), stderr(&out));
    let error = error_of(&out);
    assert_eq!(error["code"], "CONFIG_MISSING", "{error}");
    assert_eq!(error["field"], "--repo", "{error}");
    assert!(error["hint"].as_str().unwrap().contains("subdir-without-config"), "{error}");

    let human = canon(root.path(), home.path(), &["gate", "check", "--repo", sub.to_str().unwrap()]);
    assert_eq!(human.status.code(), Some(2));
    assert!(stderr(&human).contains("CONFIG_MISSING"), "{}", stderr(&human));
}

#[test]
fn a_nested_directory_of_a_canon_repo_finds_the_root_without_repo() {
    let home = TempDir::new().unwrap();
    let root = repo(home.path());
    let nested = root.path().join("packages").join("app");
    std::fs::create_dir_all(&nested).unwrap();

    let out = canon(&nested, home.path(), &["status", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let body: Value = serde_json::from_slice(&out.stdout).unwrap();
    let subjects = body["subjects"].to_string();
    assert!(subjects.contains("demo"), "a nested directory reads the root's ledger: {subjects}");

    // A given `--repo` is that directory as given, never searched upward from (D5).
    let given = canon(&nested, home.path(), &["status", "--repo", ".", "--json"]);
    assert_eq!(given.status.code(), Some(2), "{}", stderr(&given));
    assert_eq!(error_of(&given)["code"], "CONFIG_MISSING");

    let gate = canon(&nested, home.path(), &["gate", "check"]);
    assert_ne!(gate.status.code(), Some(2), "the gate runs on the root's canon.yaml: {}", stderr(&gate));
}

#[test]
fn every_command_that_needs_canon_state_refuses_without_a_canon_yaml() {
    let home = TempDir::new().unwrap();
    let empty = TempDir::new().unwrap();
    let needing_config: &[&[&str]] = &[
        &["status"],
        &["query", "--kind", "subject"],
        &["finding", "list", "--change-id", "add-login"],
        &["divergence", "status"],
        &["brief", "check"],
        &["policy", "show"],
        &["dispatch", "diff"],
        &["artifacts", "verify"],
        &["subject", "new", "x", "--domain", "dev", "--title", "X"],
        &["inventory", "sync"],
        &["report"],
        &["ingest", "sessions"],
        &["tier", "age"],
        &["ledger", "compact"],
        &["format"],
        &["gate", "check"],
    ];
    for args in needing_config {
        let mut argv = args.to_vec();
        argv.push("--json");
        let out = canon(empty.path(), home.path(), &argv);
        assert_eq!(out.status.code(), Some(2), "{args:?}: stdout {} stderr {}", stdout(&out), stderr(&out));
        assert_eq!(error_of(&out)["code"], "CONFIG_MISSING", "{args:?}");
    }
    assert!(std::fs::read_dir(empty.path()).unwrap().next().is_none(), "a refused command writes nothing");
}

#[test]
fn commands_that_run_without_a_canon_yaml_still_do() {
    let home = TempDir::new().unwrap();
    let empty = TempDir::new().unwrap();
    for args in [&["schema", "gate", "check"][..], &["context", "--json"][..], &["retrieve", "--role", "reviewer", "--domain", "dev"][..], &["regime-key", "--role", "reviewer", "--repo-name", "r", "--area", "a", "--hash", "abcdef012345"][..], &["--help"][..]] {
        let out = canon(empty.path(), home.path(), args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
    }
    let init = canon(empty.path(), home.path(), &["init", "--providers", "none"]);
    assert_eq!(init.status.code(), Some(0), "init creates the canon.yaml the others look for: {}", stderr(&init));
    assert!(empty.path().join("canon.yaml").is_file());
}

#[test]
fn format_never_reports_a_missing_path_or_a_missing_config_as_clean() {
    let home = TempDir::new().unwrap();
    let empty = TempDir::new().unwrap();

    let missing = canon(empty.path(), home.path(), &["format", "no-such-dir", "--json"]);
    assert_eq!(missing.status.code(), Some(2), "stdout: {} stderr: {}", stdout(&missing), stderr(&missing));
    assert_eq!(error_of(&missing)["code"], "REPO_UNREADABLE");

    let absolute = canon(empty.path(), home.path(), &["format", "/no/such/corpus/anywhere", "--json"]);
    assert_eq!(absolute.status.code(), Some(2));

    let no_config = canon(empty.path(), home.path(), &["format", ".", "--repo", empty.path().to_str().unwrap(), "--json"]);
    assert_eq!(no_config.status.code(), Some(2), "stdout: {} stderr: {}", stdout(&no_config), stderr(&no_config));
    assert_eq!(error_of(&no_config)["code"], "CONFIG_MISSING");

    // A corpus that exists is still checked without a canon.yaml: `format` validates the directory it is handed.
    // One with no file in it examined nothing, so it is `unverifiable` (exit 1), never clean.
    std::fs::create_dir(empty.path().join("spec")).unwrap();
    let present = canon(empty.path(), home.path(), &["format", "spec", "--json"]);
    assert_eq!(present.status.code(), Some(1), "{}", stderr(&present));
    assert_eq!(error_of(&present)["code"], "UNVERIFIABLE");
}
