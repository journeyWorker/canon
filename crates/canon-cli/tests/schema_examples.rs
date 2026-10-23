//! `canon schema <command>` carries one `example` per leaf command (0.21 D5),
//! and every example runs: in a scratch repo with one subject, change,
//! scenario and finding, it exits 0 or refuses with a code its command
//! documents. The examples that cannot exit 0 there are named below with the
//! state they need, so an example that rots (or starts to work) fails here.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// Examples that do not run to success in the scratch repo, and the state they
/// need. Each must refuse with a code the command documents; one that exits 0
/// must leave this list.
const NEEDS_STATE: &[(&str, &str)] = &[
    ("adapter authorize", "a provider's adapter response file"),
    ("adapter validate", "a provider's adapter response file"),
    ("artifacts migrate", "a canon.yaml with an s3 tier named cold: the scratch repo has none, so the target is refused"),
    ("brief approve", "a brief with no blockers (the scaffolded one is a template to fill in)"),
    ("brief check", "a finished brief: the scaffolded one has blocker findings, which exit 2 by contract"),
    ("context-pack new", "a context manifest file"),
    ("context-pack show", "a context pack made by `context-pack new`"),
    ("context-pack verify", "a context pack made by `context-pack new`"),
    ("divergence accept", "a scenario whose latest evidence is divergent, attested by another actor and session"),
    ("evidence approval-payload", "a git commit to bind: --artifact-sha must name a commit of the repo"),
    ("dispatch end", "a run opened by `dispatch begin`, whose run id this one closes"),
    ("gate check", "a repo that passes the gate: the scratch repo has unattested scenarios, which exit 1 by contract"),
    ("gate task", "a plan corpus with the task row"),
    ("learn approve", "a strategy and its evaluation file"),
    ("learn promote", "a strategy in the strategy store"),
    ("learn rollback", "a strategy, and a human's signed approval"),
    ("plugin sync", "an installed plugin"),
    ("prompt add", "a prompt bundle manifest file"),
    ("prompt show", "a registered prompt bundle"),
];

/// Examples that are not run at all.
const SKIPPED: &[(&str, &str)] = &[("dashboard", "binds a port and serves until the process is stopped")];

fn canon(dir: &Path, home: &Path, args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env("CANON_ACTOR", "schema-examples").env("HOME", home).env("CANON_SKILLS_SOURCE", concat!(env!("CARGO_MANIFEST_DIR"), "/../../canon/skills")).output().expect("spawn canon binary")
}

fn ok(dir: &Path, home: &Path, args: &[&str]) {
    let out = canon(dir, home, &args.iter().map(|a| a.to_string()).collect::<Vec<_>>());
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A repo with the story every example tells: subject `demo`, change
/// `add-login`, scenario `auth.login.01` of spec root `root`, and finding 1.
fn fixture(home: &Path) -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, home, &["init", "--providers", "none"]);
    ok(repo, home, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, home, &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    ok(repo, home, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(repo, home, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(repo, home, &["inventory", "sync"]);
    ok(repo, home, &["finding", "add", "--change-id", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--reviewer", "bob", "--summary", "Naming nit"]);
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Splits a shell-like example into arguments: whitespace separates, double
/// quotes group.
fn words(line: &str) -> Vec<String> {
    let (mut out, mut current, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut current));
                    any = false;
                }
            }
            c => {
                current.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(current);
    }
    out
}

fn schema(dir: &Path, home: &Path, args: &[&str]) -> Value {
    let mut argv: Vec<String> = vec!["schema".into()];
    argv.extend(args.iter().map(|a| a.to_string()));
    argv.push("--json".into());
    let out = canon(dir, home, &argv);
    serde_json::from_slice::<Value>(&out.stdout).unwrap_or_else(|e| panic!("{argv:?}: {e}"))["data"].clone()
}

#[test]
fn every_leaf_has_an_example_and_it_runs_in_a_scratch_repo() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    let listing = schema(base.path(), home.path(), &[]);
    let leaves: Vec<String> = listing["commands"].as_array().unwrap().iter().map(|c| c["command"].as_str().unwrap().to_string()).collect();
    assert!(leaves.len() > 60, "{leaves:?}");
    for (name, _) in NEEDS_STATE.iter().chain(SKIPPED) {
        assert!(leaves.iter().any(|l| l == name), "`{name}` is not a command");
    }

    let mut problems = Vec::new();
    for leaf in &leaves {
        let path: Vec<&str> = leaf.split(' ').collect();
        let detail = schema(base.path(), home.path(), &path);
        let Some(example) = detail["example"].as_str() else {
            problems.push(format!("{leaf}: `canon schema {leaf}` has no example"));
            continue;
        };
        if !example.starts_with(&format!("canon {leaf}")) {
            problems.push(format!("{leaf}: the example does not invoke it: {example}"));
            continue;
        }
        if SKIPPED.iter().any(|(name, _)| name == leaf) {
            continue;
        }

        // Each example runs on its own copy, so none depends on another's writes.
        let scratch = TempDir::new().unwrap();
        let mut cwd = scratch.path().join("repo");
        copy_dir(base.path(), &cwd);
        match leaf.as_str() {
            "init" => {
                cwd = scratch.path().join("fresh");
                std::fs::create_dir_all(&cwd).unwrap();
            }
            _ => {}
        }
        let mut argv = words(example);
        argv.remove(0);
        argv.push("--json".into());
        let out = canon(&cwd, home.path(), &argv);
        let code = out.status.code();
        // D3: every leaf's `--json` stdout is one envelope naming the leaf, and stderr stays empty.
        match serde_json::from_slice::<Value>(&out.stdout) {
            Ok(document) => {
                if !document["ok"].is_boolean() {
                    problems.push(format!("{leaf}: the document has no boolean `ok`: {document}"));
                }
                if document["meta"]["command"].as_str() != Some(leaf.as_str()) {
                    problems.push(format!("{leaf}: meta.command is {}", document["meta"]["command"]));
                }
                if document["ok"].as_bool() == Some(true) && code != Some(0) || document["ok"].as_bool() == Some(false) && code == Some(0) {
                    problems.push(format!("{leaf}: ok={} but exit {code:?}", document["ok"]));
                }
            }
            Err(e) => problems.push(format!("{leaf}: stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout))),
        }
        if !out.stderr.is_empty() {
            problems.push(format!("{leaf}: stderr is not empty in JSON mode: {}", String::from_utf8_lossy(&out.stderr)));
        }
        let needs = NEEDS_STATE.iter().find(|(name, _)| name == leaf);
        match (code, needs) {
            (Some(0), None) => {}
            (Some(0), Some((_, state))) => problems.push(format!("{leaf}: now exits 0; drop it from NEEDS_STATE ({state})")),
            (Some(code), Some(_)) => {
                let body: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
                let error = body["error"]["code"].as_str().unwrap_or_default();
                let documented: Vec<i64> = detail["exits"].as_array().unwrap().iter().map(|e| e["code"].as_i64().unwrap()).collect();
                let codes: Vec<&str> = detail["errorCodes"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
                if error == "USAGE" || !codes.contains(&error) || !documented.contains(&i64::from(code)) {
                    problems.push(format!("{leaf}: exit {code} with `{error}` is not a documented refusal (exits {documented:?}, codes {codes:?}): {example}"));
                }
            }
            (code, _) => problems.push(format!("{leaf}: `{example}` exited {code:?}: {}", String::from_utf8_lossy(&out.stdout))),
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
