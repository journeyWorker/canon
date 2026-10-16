//! 0.21 D1 + D3 through the real binary, as 0.22 D4 left them: the renamed
//! commands run under their new names only (the old ones are refused as
//! `USAGE` and `schema` lists none of them), `subject status <id> --to
//! <state>` is the only transition form, and a declared role (`CANON_ROLE` /
//! `--as`) narrows `schema` and defaults `--role` on `evidence add`.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::{json, Value};

fn canon_with(repo: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).env("CANON_ACTOR", "canon").env_remove("CANON_ROLE").current_dir(repo);
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().expect("spawn canon")
}

fn canon(repo: &Path, args: &[&str]) -> Output {
    canon_with(repo, args, &[])
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?} failed:\n{}{}", stdout(&out), stderr(&out));
    out
}

fn json_of(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}):\n{}\nstderr:\n{}", stdout(out), stderr(out)))
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(repo)
        .output()
        .expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A git repo that `canon init` has set up, with one scenario.
fn repo_with_scenario() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    ok(dir.path(), &["init", "--repo", ".", "--no-policy", "--providers", "none"]);
    ok(dir.path(), &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);
    dir
}

const MANIFEST: &str = r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#;

/// A committed repo holding a document and the manifest that selects it.
fn repo_with_manifest() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("doc.md"), "a document\n").unwrap();
    std::fs::write(dir.path().join("manifest.json"), MANIFEST).unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "seed"]);
    dir
}

// ── D1: the renamed commands (0.22 D4 removed the old names) ──

#[test]
fn context_pack_new_creates_a_pack() {
    let dir = repo_with_manifest();
    let new = ok(dir.path(), &["context-pack", "new", "--manifest", "manifest.json", "--json"]);
    assert!(json_of(&new)["data"]["id"].as_str().is_some_and(|id| id.starts_with("sha256:")), "{}", stdout(&new));
}

#[test]
fn prompt_add_adds_a_bundle() {
    let dir = repo_with_manifest();
    ok(dir.path(), &["prompt", "add", "--name", "reviewer", "--version", "v1", "--manifest", "manifest.json", "--json"]);
}

#[test]
fn divergence_add_stages_a_candidate() {
    let dir = repo_with_scenario();
    let sha = "a".repeat(40);
    ok(dir.path(), &["divergence", "add", "--project-id", "root", "--scenario-id", "game.run.01", "--sha", &sha, "--reviewer", "r", "--role", "reviewer", "--json"]);
}

#[test]
fn demo_add_records_the_demo_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let demo = dir.path().join("demo");
    ok(dir.path(), &["demo", "init", "--repo", demo.to_str().unwrap()]);
    ok(dir.path(), &["demo", "add", "--repo", demo.to_str().unwrap(), "--json"]);
}

/// Each form 0.21 deprecated, run with otherwise valid arguments, is now the
/// parser's ordinary `USAGE` refusal (exit 2) and writes nothing.
#[test]
fn the_removed_forms_are_refused_as_usage() {
    let dir = subject_repo();
    std::fs::write(dir.path().join("manifest.json"), MANIFEST).unwrap();
    let sha = "a".repeat(40);
    let removed: [&[&str]; 5] = [
        &["context-pack", "create", "--manifest", "manifest.json", "--json"],
        &["prompt", "register", "--name", "reviewer", "--version", "v1", "--manifest", "manifest.json", "--json"],
        &["divergence", "stage", "--project-id", "root", "--scenario-id", "game.run.01", "--sha", &sha, "--reviewer", "r", "--role", "reviewer", "--json"],
        &["demo", "attest", "--json"],
        &["subject", "status", "demo", "specced", "--json"],
    ];
    for args in removed {
        let out = canon(dir.path(), args);
        assert_eq!(out.status.code(), Some(2), "canon {args:?}:\n{}{}", stdout(&out), stderr(&out));
        assert_eq!(json_of(&out)["error"]["code"], "USAGE", "canon {args:?}: {}", stdout(&out));
    }
    let shown = ok(dir.path(), &["subject", "status", "demo"]);
    assert!(stdout(&shown).contains("(dev, proposed)"), "the two-positional form moved the subject: {}", stdout(&shown));
}

#[test]
fn the_old_forms_are_absent_from_help() {
    let dir = tempfile::tempdir().unwrap();
    for (group, old, new) in [("context-pack", "create", "new"), ("prompt", "register", "add"), ("divergence", "stage", "add"), ("demo", "attest", "add")] {
        let help = stdout(&canon(dir.path(), &[group, "--help"]));
        assert!(!help.lines().any(|l| l.trim_start().starts_with(old)), "{group} --help lists `{old}`:\n{help}");
        assert!(help.lines().any(|l| l.trim_start().starts_with(new)), "{group} --help lacks `{new}`:\n{help}");
    }
}

// ── D1: schema ──

fn listed(out: &Output) -> Vec<Value> {
    json_of(out)["data"]["commands"].as_array().expect("a command listing").clone()
}

fn names(commands: &[Value]) -> Vec<&str> {
    commands.iter().map(|c| c["command"].as_str().unwrap()).collect()
}

#[test]
fn schema_lists_no_removed_command_even_under_all() {
    let dir = tempfile::tempdir().unwrap();
    let all = listed(&canon(dir.path(), &["schema", "--all", "--json"]));
    for (old, new) in [("context-pack create", "context-pack new"), ("prompt register", "prompt add"), ("divergence stage", "divergence add"), ("demo attest", "demo add")] {
        assert!(!names(&all).contains(&old), "schema --all lists removed `{old}`");
        assert!(names(&all).contains(&new), "schema --all lacks `{new}`");
    }
    assert!(all.iter().all(|c| c.get("deprecated").is_none()));

    let one = canon(dir.path(), &["schema", "demo", "attest", "--json"]);
    assert_eq!(one.status.code(), Some(2), "{}", stdout(&one));
    assert_eq!(json_of(&one)["error"]["code"], "UNKNOWN_COMMAND", "{}", stdout(&one));
}

#[test]
fn subject_status_is_a_write_that_reads_without_to() {
    let dir = tempfile::tempdir().unwrap();
    let schema = json_of(&canon(dir.path(), &["schema", "subject", "status", "--json"]));
    assert_eq!(schema["data"]["effect"], "write");
    assert!(schema["data"]["effectNote"].as_str().unwrap().contains("without --to"), "{schema}");
    let options: Vec<&str> = schema["data"]["options"].as_array().unwrap().iter().map(|o| o["name"].as_str().unwrap()).collect();
    assert!(options.contains(&"to"), "{options:?}");
    let arguments: Vec<&str> = schema["data"]["arguments"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(arguments, ["id"], "the state is an option, not a positional");
}

// ── D1: subject status ──

fn subject_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["init", "--repo", ".", "--no-policy", "--providers", "none"]);
    ok(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    dir
}

#[test]
fn subject_status_to_transitions_and_without_to_shows() {
    let dir = subject_repo();
    let shown = ok(dir.path(), &["subject", "status", "demo", "--json"]);
    assert_eq!(json_of(&shown)["data"]["status"], "proposed", "{}", stdout(&shown));

    let moved = ok(dir.path(), &["subject", "status", "demo", "--to", "specced"]);
    assert!(stdout(&moved).contains("demo → specced"), "{}", stdout(&moved));

    let shown = ok(dir.path(), &["subject", "status", "demo"]);
    assert!(stdout(&shown).contains("(dev, specced)"), "{}", stdout(&shown));

    let reason_without_to = canon(dir.path(), &["subject", "status", "demo", "--reason", "why"]);
    assert_eq!(reason_without_to.status.code(), Some(2), "{}", stderr(&reason_without_to));
    assert!(stderr(&reason_without_to).contains("SUBJECT_INVALID_ARGUMENT"), "{}", stderr(&reason_without_to));
}

// ── D3: roles ──

fn schema_listing(env: &[(&str, &str)], args: &[&str]) -> (Output, Vec<Value>) {
    let dir = tempfile::tempdir().unwrap();
    let mut full = vec!["schema", "--json"];
    full.extend_from_slice(args);
    let out = canon_with(dir.path(), &full, env);
    let commands = if out.status.success() { listed(&out) } else { Vec::new() };
    (out, commands)
}

#[test]
fn canon_role_reviewer_lists_the_reviewers_commands_only() {
    let (out, commands) = schema_listing(&[("CANON_ROLE", "reviewer")], &[]);
    assert_eq!(json_of(&out)["data"]["role"], "reviewer");
    let listed = names(&commands);
    assert!(listed.contains(&"review add") && listed.contains(&"finding add") && listed.contains(&"brief check"), "{listed:?}");
    assert!(!listed.contains(&"evidence add") && !listed.contains(&"scenario new") && !listed.contains(&"gate promote"), "{listed:?}");
    assert!(commands.iter().all(|c| c["roles"].as_array().unwrap().contains(&json!("reviewer"))), "every listed command carries the role");

    let (_, everyone) = schema_listing(&[], &[]);
    assert!(everyone.len() > commands.len(), "no declared role lists every command");
    let (_, unfiltered) = schema_listing(&[], &["--all"]);
    let (_, all) = schema_listing(&[("CANON_ROLE", "reviewer")], &["--all"]);
    assert_eq!(names(&all), names(&unfiltered), "--all ignores the declared role");
}

#[test]
fn the_as_flag_wins_over_canon_role() {
    let (out, commands) = schema_listing(&[("CANON_ROLE", "reviewer")], &["--as", "implementer"]);
    assert_eq!(json_of(&out)["data"]["role"], "implementer");
    assert!(names(&commands).contains(&"evidence add") && !names(&commands).contains(&"review add"));
}

#[test]
fn every_command_declares_its_roles_and_humans_only_commands_declare_none() {
    let (_, all) = schema_listing(&[], &[]);
    let by_name = |name: &str| all.iter().find(|c| c["command"] == name).unwrap_or_else(|| panic!("no `{name}`"));
    assert_eq!(by_name("evidence add")["roles"], json!(["implementer"]));
    assert_eq!(by_name("review add")["roles"], json!(["reviewer"]));
    assert_eq!(by_name("brief approve")["roles"], json!([]));
    assert!(all.iter().all(|c| c["roles"].is_array()));
}

#[test]
fn an_unknown_canon_role_is_refused_with_unknown_role() {
    let dir = tempfile::tempdir().unwrap();
    let out = canon_with(dir.path(), &["schema", "--json"], &[("CANON_ROLE", "janitor")]);
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(&out), stderr(&out));
    assert_eq!(json_of(&out)["error"]["code"], "UNKNOWN_ROLE");
    assert!(stderr(&out).contains("UNKNOWN_ROLE") && stderr(&out).contains("janitor"), "{}", stderr(&out));

    let human = canon_with(dir.path(), &["status"], &[("CANON_ROLE", "janitor")]);
    assert_eq!(human.status.code(), Some(2));
    assert!(stderr(&human).contains("UNKNOWN_ROLE"), "{}", stderr(&human));

    let overridden = canon_with(dir.path(), &["schema", "--as", "planner", "--json"], &[("CANON_ROLE", "janitor")]);
    assert!(overridden.status.success(), "the flag wins, so the bad variable is never consulted: {}", stderr(&overridden));

    let bad_flag = canon(dir.path(), &["schema", "--as", "janitor"]);
    assert_eq!(bad_flag.status.code(), Some(2), "an unknown --as is a usage error");
}

fn evidence_without_role(repo: &Path, env: &[(&str, &str)]) -> Output {
    canon_with(repo, &["evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "test-run", "--ref", "cargo test", "--actor-id", "impl", "--json"], env)
}

#[test]
fn record_role_defaults_to_the_declared_implementer_or_reviewer() {
    let dir = repo_with_scenario();
    let repo = dir.path();

    let undeclared = evidence_without_role(repo, &[]);
    assert_eq!(undeclared.status.code(), Some(2), "{}", stderr(&undeclared));
    assert_eq!(json_of(&undeclared)["error"]["code"], "USAGE");
    assert!(stderr(&undeclared).contains("--role"), "{}", stderr(&undeclared));

    let planner = evidence_without_role(repo, &[("CANON_ROLE", "planner")]);
    assert_eq!(planner.status.code(), Some(2), "a planner records no evidence by default: {}", stderr(&planner));

    let implementer = evidence_without_role(repo, &[("CANON_ROLE", "implementer")]);
    assert!(implementer.status.success(), "{}{}", stdout(&implementer), stderr(&implementer));
    let records = std::fs::read_dir(repo.join(".canon/ledger/kind=evidence_record")).unwrap().count();
    assert_eq!(records, 1, "the record was committed");
    let record = std::fs::read_dir(repo.join(".canon/ledger/kind=evidence_record")).unwrap().next().unwrap().unwrap().path();
    let text = std::fs::read_to_string(record).unwrap();
    assert!(text.contains("\"implementer\""), "the record carries the declared role: {text}");

    let flag_wins = canon(repo, &["--as", "reviewer", "evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "test-run", "--ref", "cargo test 2", "--actor-id", "rev", "--role", "implementer"]);
    assert!(flag_wins.status.success(), "{}{}", stdout(&flag_wins), stderr(&flag_wins));
}
