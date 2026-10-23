//! 0.28 grammar behaviours owned by the context-capability scenarios
//! (`context.capability.27` to `.37`), against the built `canon` binary:
//! the generated spec and its drift check (D2), the `--json` envelope, its
//! error object and its empty stderr (D3), the exit table (D4) and the removed
//! 0.27 spellings (D8).
//!
//! Each test names the scenario it proves. They assert exit codes, JSON fields
//! and the tree left behind, never help or message wording.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_cli::spec::{self, SPEC_PATH};
use serde_json::Value;
use tempfile::TempDir;

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env_remove("CANON_ACTOR").output().expect("spawn canon binary")
}

fn json_of(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").current_dir(dir).args(["-c", "user.name=canon", "-c", "user.email=canon@example.invalid"]).args(args).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A committed git repository with canon set up (light policy, no skills).
fn repo() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    let init = canon(dir.path(), &["init", "--no-agents-md", "--policy", "light", "--providers", "none"]);
    assert!(init.status.success(), "{}", stderr(&init));
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "fixture"]);
    dir
}

/// The same repository with one subject and one change whose brief is still the template.
fn repo_with_change() -> TempDir {
    let dir = repo();
    let subject = canon(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    assert!(subject.status.success(), "{}", stderr(&subject));
    let change = canon(dir.path(), &["change", "new", "add-x", "--subject-id", "demo", "--title", "X"]);
    assert!(change.status.success(), "{}", stderr(&change));
    dir
}

/// Every file under `root` (relative path, bytes), `.git` excluded.
fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                out.push((path.strip_prefix(root).expect("under root").to_path_buf(), std::fs::read(&path).expect("read")));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn spec_on_disk() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(SPEC_PATH)).expect("the checked-in spec")
}

/// The nodes of the spec as the drift check compares them: command → node.
fn nodes(doc: &Value) -> std::collections::BTreeMap<String, Value> {
    doc["commands"].as_array().expect("commands").iter().map(|n| (n["command"].as_str().expect("command").to_string(), n.clone())).collect()
}

// ── D2: the spec ────────────────────────────────────────────────────────────────────────

/// context.capability.27: `canon schema <command>` shows the node of the checked-in spec.
#[test]
fn context_capability_27_schema_nodes_are_the_nodes_of_the_checked_in_spec() {
    let doc: Value = serde_json::from_str(&spec_on_disk()).expect("the spec is JSON");
    let dir = tempfile::tempdir().expect("tempdir");
    let mut leaves = BTreeSet::new();
    for (command, node) in nodes(&doc) {
        if node["nodeKind"] != "leaf" {
            continue;
        }
        let mut args = vec!["schema"];
        args.extend(command.split(' '));
        args.push("--json");
        let out = canon(dir.path(), &args);
        assert!(out.status.success(), "{command}: {}", stderr(&out));
        assert_eq!(json_of(&out)["data"], node, "`canon schema {command}` and the checked-in spec disagree");
        leaves.insert(command);
    }
    assert!(leaves.len() > 60, "{} leaves", leaves.len());

    let listing = json_of(&canon(dir.path(), &["schema", "--json"]));
    let listed: BTreeSet<String> = listing["data"]["commands"].as_array().expect("commands").iter().map(|c| c["command"].as_str().expect("command").to_string()).collect();
    assert_eq!(listed, leaves, "the bare listing is every leaf of the spec and nothing else");
}

/// context.capability.28: the drift check is the byte comparison of the checked-in spec with the one the
/// CLI generates (`cli_spec.rs` runs the same comparison); it holds on the unmodified tree, and the spec
/// the CLI generates is the same text every time.
#[test]
fn context_capability_28_the_checked_in_spec_is_the_one_the_cli_generates() {
    let generated = spec::render();
    assert!(spec_on_disk() == generated, "{SPEC_PATH} is not the text the CLI generates");
    assert_eq!(spec::render(), generated, "two generations are the same text");
}

// ── D3: the envelope ────────────────────────────────────────────────────────────────────

fn sorted_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value.as_object().expect("an object").keys().map(String::as_str).collect();
    keys.sort_unstable();
    keys
}

/// context.capability.29: a success is `{ok: true, data, meta}` with the command, the schema
/// version and the canon version in `meta`.
#[test]
fn context_capability_29_a_success_is_ok_data_and_meta() {
    let dir = repo();
    for (args, command) in [(&["status", "--json"][..], "status"), (&["schema", "purge", "--json"], "schema"), (&["context", "--fields", "kinds", "--json"], "context"), (&["skills", "show", "canon", "--json"], "skills show")] {
        let out = canon(dir.path(), args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        let body = json_of(&out);
        assert_eq!(sorted_keys(&body), ["data", "meta", "ok"], "{args:?}: {body}");
        assert_eq!(body["ok"], true, "{args:?}");
        let meta = &body["meta"];
        assert_eq!(meta["command"], command, "{args:?}: {meta}");
        let version = meta["schemaVersion"].as_str().expect("schemaVersion");
        assert!(version.split('.').count() == 2 && version.split('.').all(|p| p.parse::<u32>().is_ok()), "{version}");
        assert_eq!(meta["canonVersion"], env!("CARGO_PKG_VERSION"));
        assert!(meta["context"].get("repo").is_some(), "{meta}");
    }
}

/// context.capability.30: a failure is `{ok: false, error, meta}`; the error carries a registered
/// code, a message, a hint and `retryable`, and a usage error names its `field`. A failing check
/// keeps its report in `data`.
#[test]
fn context_capability_30_a_failure_is_an_error_object_with_code_message_hint_and_retryable() {
    let dir = repo_with_change();

    let missing = canon(dir.path(), &["brief", "check", "no-such-change", "--json"]);
    assert_eq!(missing.status.code(), Some(2), "{}", stderr(&missing));
    let body = json_of(&missing);
    assert_eq!(body["ok"], false, "{body}");
    assert!(body.get("data").is_none(), "a refusal has no report: {body}");
    let error = &body["error"];
    assert_eq!(error["code"], "BRIEF_NOT_FOUND", "{body}");
    for key in ["message", "hint"] {
        assert!(error[key].as_str().is_some_and(|s| !s.is_empty()), "error.{key}: {body}");
    }
    assert_eq!(error["retryable"], false, "{body}");
    assert_eq!(body["meta"]["command"], "brief check");

    let usage = canon(dir.path(), &["evidence", "add", "--json"]);
    assert_eq!(usage.status.code(), Some(2));
    let body = json_of(&usage);
    assert_eq!((body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some("USAGE"), Some("--class")), "{body}");
    assert!(body["error"]["hint"].as_str().is_some_and(|h| h.contains("canon evidence add")), "{body}");
    assert!(body["error"]["message"].as_str().is_some_and(|m| !m.is_empty()), "{body}");
    assert!(body["error"]["retryable"].is_boolean(), "{body}");
    assert!(body.get("data").is_none(), "a usage error has no report: {body}");

    let blockers = canon(dir.path(), &["brief", "check", "add-x", "--json"]);
    assert_eq!(blockers.status.code(), Some(1));
    let body = json_of(&blockers);
    assert_eq!(body["ok"], false, "{body}");
    assert_eq!(body["error"]["code"], "BRIEF_HAS_BLOCKERS", "{body}");
    for key in ["message", "hint"] {
        assert!(body["error"][key].as_str().is_some_and(|s| !s.is_empty()), "error.{key}: {body}");
    }
    assert!(body["error"]["retryable"].is_boolean(), "{body}");
    assert!(body["data"]["counts"]["blockers"].as_u64().is_some_and(|n| n > 0), "the failing check keeps its report: {body}");

    // Every code a run printed is one `canon schema --error` explains.
    for code in ["BRIEF_NOT_FOUND", "USAGE", "BRIEF_HAS_BLOCKERS"] {
        let explained = canon(dir.path(), &["schema", "--error", code, "--json"]);
        assert!(explained.status.success(), "{code}: {}", stderr(&explained));
    }
}

/// context.capability.31: with `--json` a warning is `meta.warnings` and stderr stays empty; the
/// same run without `--json` says it on stderr and puts nothing in the document.
#[test]
fn context_capability_31_json_mode_carries_warnings_in_meta_and_leaves_stderr_empty() {
    let dir = repo();
    let finding = |seq: &str, json: bool| {
        let mut args = vec!["finding", "add", "--change-id", "c1", "--round", "1", "--seq", seq, "--severity", "note", "--actor-id", "x", "--summary", "s", "--role", "reviwer", "--no-promote"];
        if json {
            args.push("--json");
        }
        canon(dir.path(), &args)
    };

    let out = finding("1", true);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stderr(&out).is_empty(), "JSON mode writes nothing on stderr: {}", stderr(&out));
    let body = json_of(&out);
    let warnings = body["meta"]["warnings"].as_array().expect("meta.warnings");
    assert!(warnings.iter().any(|w| w.as_str().is_some_and(|w| w.contains("reviwer"))), "{body}");

    let human = finding("2", false);
    assert_eq!(human.status.code(), Some(0), "{}", stderr(&human));
    assert!(stderr(&human).contains("reviwer"), "the warning is on stderr without --json: {}", stderr(&human));
}

/// context.capability.32: a failure in JSON mode is one document on stdout and nothing on stderr,
/// whether clap, the removed-spelling table or the command itself refused it; `--json` is decided
/// from the argv before it is parsed.
#[test]
fn context_capability_32_a_json_failure_leaves_stderr_empty() {
    let repo = repo_with_change();
    let empty = tempfile::tempdir().expect("tempdir");
    let cases: [(&Path, &[&str], i32); 6] = [
        (repo.path(), &["status", "--no-such-flag", "--json"], 2),
        (repo.path(), &["brief", "--json"], 2),
        (repo.path(), &["--json", "no-such-command"], 2),
        (repo.path(), &["brief", "check", "no-such-change", "--json"], 2),
        (repo.path(), &["gate", "check", "--repo", "/nonexistent-canon-dir", "--json"], 2),
        (empty.path(), &["status", "--json"], 2),
    ];
    for (dir, args, code) in cases {
        let out = canon(dir, args);
        assert_eq!(out.status.code(), Some(code), "{args:?}: {}", stderr(&out));
        assert!(stderr(&out).is_empty(), "{args:?}: JSON mode writes nothing on stderr: {}", stderr(&out));
        let body = json_of(&out);
        assert_eq!(body["ok"], false, "{args:?}: {body}");
        assert!(body["error"]["code"].as_str().is_some(), "{args:?}: {body}");
    }
    let human = canon(empty.path(), &["status"]);
    assert_eq!(human.status.code(), Some(2));
    assert!(human.stdout.is_empty() && !stderr(&human).is_empty(), "without --json the refusal is on stderr");
}

// ── D4: the exit table ──────────────────────────────────────────────────────────────────

/// context.capability.33: 0 is ok, 1 is the command's finding (violations, blockers, nothing
/// examined), 2 is a refusal; the spec's table lists those three and no other.
#[test]
fn context_capability_33_exit_zero_is_ok_one_is_a_finding_and_two_is_a_refusal() {
    let dir = repo_with_change();

    assert_eq!(canon(dir.path(), &["status"]).status.code(), Some(0));

    let corpus = dir.path().join("loose/kind=feature/area=a");
    std::fs::create_dir_all(&corpus).expect("corpus dir");
    std::fs::write(corpus.join("b.feature"), "Feature: x\n\n  @a.b.01\n  Scenario: s\n    Given g\n").expect("feature");
    let format = canon(dir.path(), &["format", "loose", "--json"]);
    assert_eq!(format.status.code(), Some(1), "{}", stderr(&format));
    let body = json_of(&format);
    assert_eq!(body["ok"], false);
    assert!(body["data"]["violations"].as_array().is_some_and(|v| !v.is_empty()), "the finding is in data: {body}");

    let brief = canon(dir.path(), &["brief", "check", "add-x"]);
    assert_eq!(brief.status.code(), Some(1), "a brief with blockers is a finding, not a refusal");
    let brief = canon(dir.path(), &["brief", "check", "add-x", "--json"]);
    assert_eq!(brief.status.code(), Some(1));
    assert!(json_of(&brief)["data"]["counts"]["blockers"].as_u64().is_some_and(|n| n > 0), "the finding is in data: {}", stdout_of(&brief));

    let fresh = repo();
    let nothing = canon(fresh.path(), &["gate", "check", "--json"]);
    assert_eq!(nothing.status.code(), Some(1), "a check that examined nothing is not clean");
    assert_eq!(json_of(&nothing)["data"]["clean"], false);

    assert_eq!(canon(dir.path(), &["status", "--no-such-flag"]).status.code(), Some(2));
    assert_eq!(canon(dir.path(), &["brief", "check", "no-such-change"]).status.code(), Some(2));

    let doc: Value = serde_json::from_str(&spec_on_disk()).expect("spec");
    let codes: Vec<i64> = doc["exitTable"].as_array().expect("exitTable").iter().map(|r| r["code"].as_i64().expect("code")).collect();
    assert_eq!(codes, [0, 1, 2]);
}

/// Splits `canon <words>` the way a shell would, for the double-quoted values a hint carries.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(current);
    }
    out
}

/// context.capability.34: a usage error is exit 2 with a hint holding a working `canon …` call, which
/// parses as written (the hint's call followed by `--help` exits 0).
#[test]
fn context_capability_34_a_usage_error_is_exit_two_with_a_runnable_hint() {
    let dir = repo_with_change();
    for args in [&["subject", "move", "demo", "--json"][..], &["evidence", "add", "--json"], &["status", "--bogus", "--json"], &["scenario", "new", "--json"]] {
        let out = canon(dir.path(), args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
        let error = json_of(&out)["error"].clone();
        assert_eq!(error["code"], "USAGE", "{error}");
        let hint = error["hint"].as_str().unwrap_or_default();
        let call = hint.split("a working call: ").nth(1).and_then(|rest| rest.split(';').next()).unwrap_or_else(|| panic!("{args:?}: the hint holds a working call: {hint}"));
        let mut argv = words(call);
        assert_eq!(argv.remove(0), "canon", "{call}");
        argv.push("--help".to_string());
        let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
        let parsed = canon(dir.path(), &argv);
        assert_eq!(parsed.status.code(), Some(0), "the hint's call `{call}` parses: {}", stderr(&parsed));
    }
}

// ── D8: removed spellings ───────────────────────────────────────────────────────────────

/// The backticked `canon …` calls a hint names.
fn calls(hint: &str) -> Vec<String> {
    hint.split('`').skip(1).step_by(2).filter(|c| c.starts_with("canon ")).map(str::to_string).collect()
}

/// The refusal of a removed spelling: exit 2, USAGE, `field` the spelling, `expected` and `hint` the 0.28 one.
fn refused(dir: &Path, args: &[&str]) -> Value {
    let before = tree(dir);
    let out = canon(dir, args);
    assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
    assert!(stderr(&out).is_empty(), "{args:?}: {}", stderr(&out));
    let body = json_of(&out);
    assert_eq!(body["ok"], false, "{body}");
    assert_eq!(body["error"]["code"], "USAGE", "{body}");
    assert!(body["error"]["expected"].as_array().is_some_and(|e| !e.is_empty()), "{args:?}: expected lists the 0.28 spelling: {body}");
    assert_eq!(tree(dir), before, "{args:?}: a refused spelling runs nothing");
    body["error"].clone()
}

/// A command a hint names runs as written: it parses, so `--help` after it exits 0.
fn parses(dir: &Path, call: &str) {
    let mut argv = words(call);
    assert_eq!(argv.remove(0), "canon");
    argv.push("--help".to_string());
    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    let out = canon(dir, &argv);
    assert_eq!(out.status.code(), Some(0), "`{call}` is a 0.28 command: {}", stderr(&out));
}

/// context.capability.35: a removed leaf is exit 2 and its hint is the leaf that replaced it.
#[test]
fn context_capability_35_a_removed_leaf_is_exit_two_with_the_leaf_that_replaced_it() {
    let dir = repo_with_change();
    for (args, removed, replacement) in [
        (&["evidence", "vault", "--json"][..], "evidence vault", "canon artifacts store"),
        (&["gate", "task", "add-x#1", "--json"], "gate task", "canon task done"),
        (&["subject", "status", "demo", "--json"], "subject status", "canon subject show"),
        (&["learn", "promote", "--json"], "learn promote", "canon strategy promote"),
        (&["review", "add", "--scenario-id", "a.b.01", "--json"], "review add", "canon review scenario"),
        (&["regime-key", "--json"], "regime-key", "canon regime show"),
        (&["divergence", "status", "--json"], "divergence status", "canon divergence list"),
    ] {
        let error = refused(dir.path(), args);
        assert_eq!(error["field"], removed, "{args:?}: field is the removed spelling: {error}");
        let replacement_words = replacement.trim_start_matches("canon ");
        assert!(error["expected"].as_array().is_some_and(|e| e.iter().any(|v| v.as_str().is_some_and(|v| v == replacement_words || v.starts_with(&format!("{replacement_words} "))))), "{args:?}: expected holds the 0.28 spelling: {error}");
        let named = calls(error["hint"].as_str().unwrap_or_default());
        assert!(named.iter().any(|c| c == replacement || c.starts_with(&format!("{replacement} "))), "{args:?}: the hint names `{replacement}`: {error}");
        parses(dir.path(), replacement);
    }
}

/// context.capability.36: a removed option is exit 2 and its hint names the option that replaced it,
/// which the command takes.
#[test]
fn context_capability_36_a_removed_option_is_exit_two_with_the_option_that_replaced_it() {
    let dir = repo_with_change();
    for (args, removed, leaf, replacement) in [
        (&["format", "--check", "--json"][..], "--check", "format", "--check"),
        (&["query", "--canon-yaml", "x", "--kind", "evidence", "--json"], "--canon-yaml", "query", "--repo"),
        (&["retrieve", "-k", "3", "--for-role", "reviewer", "--domain", "dev", "--json"], "-k", "retrieve", "--limit"),
        (&["init", "--check-config", "--json"], "--check-config", "init", "--check"),
        (&["evidence", "add", "--kind", "audit", "--scenario-id", "a.b.01", "--json"], "--kind", "evidence add", "--class"),
        (&["tier", "age", "--dry-run", "--json"], "--dry-run", "tier age", "--commit"),
    ] {
        let error = refused(dir.path(), args);
        assert_eq!(error["field"], removed, "{args:?}: field is the removed option: {error}");
        if args[0] == "format" {
            // `format` lost the flag outright: the replacement is the bare command.
            assert!(calls(error["hint"].as_str().unwrap_or_default()).iter().any(|c| c == "canon format"), "{error}");
            parses(dir.path(), "canon format");
            continue;
        }
        let hint = error["hint"].as_str().unwrap_or_default();
        assert!(hint.contains(replacement), "{args:?}: the hint names {replacement}: {error}");
        let node = json_of(&canon(dir.path(), &[&["schema"][..], &leaf.split(' ').collect::<Vec<_>>(), &["--json"]].concat()));
        let options: Vec<&str> = node["data"]["options"].as_array().expect("options").iter().map(|o| o["name"].as_str().expect("name")).collect();
        assert!(options.contains(&replacement.trim_start_matches("--")), "`canon {leaf}` takes {replacement}: {options:?}");
    }
}

/// context.capability.37: a bare group that used to run is exit 2 and its hint is the leaf to run.
#[test]
fn context_capability_37_a_bare_group_is_exit_two_with_the_leaf_to_run() {
    let dir = repo_with_change();
    for (args, replacement) in [(&["brief", "--json"][..], "canon brief show"), (&["report", "--json"], "canon report build"), (&["artifacts", "--json"], "")] {
        let out = canon(dir.path(), args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(stderr(&out).is_empty(), "{args:?}: {}", stderr(&out));
        let error = json_of(&out)["error"].clone();
        assert_eq!(error["code"], "USAGE", "{error}");
        if replacement.is_empty() {
            // A group that never ran bare is the ordinary usage error: still exit 2, still a hint.
            assert!(error["hint"].as_str().is_some_and(|h| !h.is_empty()), "{error}");
            continue;
        }
        let named = calls(error["hint"].as_str().unwrap_or_default());
        assert!(named.iter().any(|c| c == replacement), "{args:?}: the hint names `{replacement}`: {error}");
        parses(dir.path(), replacement);
    }
}
