//! One name per concept (0.24 D1-D3, #85) through the built binary.
//!
//! The contract: a concept has one flag spelling across every command
//! (`--<noun>-id`), a `--kind` value names a record kind without the
//! `_record` suffix, and the 0.23 spellings 0.24 deprecated are refused (0.25 D6)
//! as usage errors whose hint names the spelling that replaced them.
//! The checker is a function over (command path, flags, `--kind` values), so
//! its own test can feed it a synthetic split and prove it would catch one.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

// ── the checker ──────────────────────────────────────────────────────────

/// One command as the checker sees it: its path, its visible flags (long
/// names without `--`) and the values of its `--kind` option, when it has a
/// closed set.
struct Surface {
    path: String,
    flags: Vec<String>,
    kind_values: Vec<String>,
}

/// Concepts whose flag is always `--<noun>-id` (the id of a record or a
/// configured entity), never `--<noun>`.
const CONCEPTS: &[&str] = &["change", "project", "subject", "actor", "scenario", "session", "run"];

/// Every way `commands` breaks the one-name rule: (a) two flags that differ
/// only by an `-id` suffix, (b) a concept spelled without `-id`, (c) a
/// `--kind` value that ends in `_record`.
fn one_name_problems(commands: &[Surface]) -> Vec<String> {
    let mut carriers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for command in commands {
        for flag in &command.flags {
            carriers.entry(flag.as_str()).or_default().push(command.path.as_str());
        }
    }
    let mut problems = Vec::new();
    for (flag, with_id) in &carriers {
        let Some(stem) = flag.strip_suffix("-id") else { continue };
        if let Some(without) = carriers.get(stem) {
            problems.push(format!("--{stem} ({}) and --{flag} ({}) name one concept two ways", without.join(", "), with_id.join(", ")));
        }
    }
    for (flag, paths) in &carriers {
        if CONCEPTS.contains(flag) {
            problems.push(format!("--{flag} ({}) is spelled without -id: use --{flag}-id", paths.join(", ")));
        }
    }
    for command in commands {
        for value in command.kind_values.iter().filter(|v| v.ends_with("_record")) {
            problems.push(format!("`{}` --kind value `{value}` ends in _record: use `{}`", command.path, value.trim_end_matches("_record")));
        }
    }
    problems
}

fn surface(path: &str, flags: &[&str], kind_values: &[&str]) -> Surface {
    Surface { path: path.to_string(), flags: flags.iter().map(|f| f.to_string()).collect(), kind_values: kind_values.iter().map(|v| v.to_string()).collect() }
}

/// context.capability.22: the checker reports a split, a bare concept and a
/// `_record` value, and stays quiet on a tree without them.
#[test]
fn context_capability_22_the_checker_reports_a_concept_spelled_two_ways() {
    let split = [surface("brief check", &["change", "json"], &[]), surface("finding list", &["change-id", "json"], &[])];
    let problems = one_name_problems(&split);
    assert!(problems.iter().any(|p| p.contains("--change (brief check) and --change-id (finding list) name one concept two ways")), "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("--change (brief check) is spelled without -id")), "{problems:?}");

    let bare = [surface("retrieve", &["subject"], &[])];
    assert_eq!(one_name_problems(&bare), vec!["--subject (retrieve) is spelled without -id: use --subject-id".to_string()]);

    let suffixed = [surface("query", &["json"], &["evidence", "review_record"])];
    assert_eq!(one_name_problems(&suffixed), vec!["`query` --kind value `review_record` ends in _record: use `review`".to_string()]);

    let clean = [surface("brief check", &["change-id", "json"], &[]), surface("query", &["change-id", "kind"], &["evidence", "review"]), surface("evidence add", &["kind"], &[])];
    assert!(one_name_problems(&clean).is_empty());

    // Flags that merely share a stem are not a split: `--scenario-ids-from` is not `--scenario-id`.
    let shared = [surface("review add", &["scenario-id", "scenario-ids-from", "evaluator-id", "evaluator-version"], &[])];
    assert!(one_name_problems(&shared).is_empty());
}

// ── the real tree ────────────────────────────────────────────────────────

fn canon_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").env_remove("CANON_ROLE").current_dir(dir).output().expect("spawn canon")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn schema_json(args: &[&str]) -> Value {
    let dir = tempfile::tempdir().unwrap();
    let out = canon_in(dir.path(), args);
    assert!(out.status.success(), "canon {args:?}: {}", stderr(&out));
    serde_json::from_slice::<Value>(&out.stdout).expect("stdout is JSON")["data"].clone()
}

/// Every command of the clap tree, read through `canon schema` (visible
/// arguments only).
fn command_tree() -> Vec<Surface> {
    let all = schema_json(&["schema", "--all", "--json"]);
    all["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let path = entry["command"].as_str().unwrap().to_string();
            let mut args = vec!["schema"];
            args.extend(path.split(' '));
            args.push("--json");
            let schema = schema_json(&args);
            let options = schema["options"].as_array().unwrap();
            let mut flags: Vec<String> = Vec::new();
            for option in options {
                flags.push(option["name"].as_str().unwrap().to_string());
                flags.extend(option["aliases"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().trim_start_matches('-').to_string()));
            }
            let kind_values = options
                .iter()
                .filter(|o| o["name"] == "kind")
                .flat_map(|o| o["possibleValues"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()))
                .collect();
            Surface { path, flags, kind_values }
        })
        .collect()
}

/// context.capability.14 and .15: `canon schema --all` shows no concept spelled
/// two ways, no concept without `-id`, and no `--kind` value ending in `_record`.
#[test]
fn context_capability_14_one_concept_has_one_flag_spelling_across_every_command() {
    let tree = command_tree();
    assert!(tree.len() > 60, "the walk covers the whole tree: {} commands", tree.len());
    let problems = one_name_problems(&tree);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn context_capability_15_a_kind_value_never_carries_the_record_suffix() {
    let tree = command_tree();
    for command in ["query", "export", "purge"] {
        let values = &tree.iter().find(|c| c.path == command).unwrap().kind_values;
        assert!(values.contains(&"evidence".to_string()) && values.contains(&"strategy_item".to_string()), "{command}: {values:?}");
        assert!(values.iter().all(|v| !v.ends_with("_record")), "{command}: {values:?}");
    }

    // The stored names do not move: the record's own `kind`, the ledger directory and the routing key.
    let dir = repo_with_change();
    let repo = dir.path();
    let out = canon_in(repo, &["evidence", "add", "--project-id", "root", "--scenario-id", "demo.login.01", "--kind", "test-run", "--ref", "npm test", "--role", "implementer"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    let ledger = repo.join(".canon/ledger/kind=evidence_record");
    let files: Vec<_> = std::fs::read_dir(&ledger).expect("the ledger directory keeps its stored name").flatten().collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let record: Value = serde_json::from_str(&std::fs::read_to_string(files[0].path()).unwrap()).unwrap();
    assert_eq!(record["kind"], "evidence_record", "{record}");
    assert!(std::fs::read_to_string(repo.join("canon.yaml")).unwrap().lines().any(|l| l.trim() == "evidence_record: local"), "the canon.yaml routing key keeps its stored name");
}

// ── the removed spellings ───────────────────────────────────────────────

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(repo).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A repo with one subject `demo` and one change `add-login` (brief skeleton).
fn repo_with_change() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    for args in [
        &["init", "--repo", ".", "--no-policy", "--providers", "none"][..],
        &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"],
        &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"],
    ] {
        let out = canon_in(dir.path(), args);
        assert!(out.status.success(), "canon {args:?}: {}{}", stdout(&out), stderr(&out));
    }
    dir
}

/// The usage error of a refused run: exit 2, code `USAGE`, one JSON document.
fn usage_error(out: &Output) -> Value {
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(out), stderr(out));
    let body: Value = serde_json::from_slice(&out.stdout).expect("one JSON document");
    assert_eq!(body["error"]["code"], "USAGE", "{body}");
    assert!(!body["error"]["hint"].as_str().unwrap_or_default().is_empty(), "{body}");
    body
}

/// context.capability.16: a removed flag is a usage error naming the flag that
/// replaced it; the canonical spelling runs and the envelope has no `deprecated`.
#[test]
fn context_capability_16_a_removed_flag_spelling_is_refused_with_the_flag_that_replaced_it() {
    let dir = repo_with_change();
    let repo = dir.path();

    let old = canon_in(repo, &["change", "new", "add-search", "--subject", "demo", "--title", "Add search", "--json"]);
    let body = usage_error(&old);
    assert_eq!(body["error"]["hint"], "--subject was renamed --subject-id in 0.24.0; use --subject-id", "{body}");
    assert_eq!(body["error"]["field"], "--subject", "{body}");
    assert!(!repo.join(".canon/changes/add-search").exists(), "a refused spelling runs nothing");

    let new = canon_in(repo, &["change", "new", "add-cart", "--subject-id", "demo", "--title", "Add cart", "--json"]);
    assert!(new.status.success(), "{}", stderr(&new));
    assert!(serde_json::from_slice::<Value>(&new.stdout).unwrap()["meta"].get("deprecated").is_none());

    // The `=` form, a global argument placed before the subcommand, and a flag missing its value.
    for args in [
        &["brief", "check", "--change=add-login", "--json"][..],
        &["brief", "check", "--change", "add-login", "--json"],
        &["brief", "--change", "add-login", "check", "--json"],
        &["brief", "check", "--change", "--json"],
    ] {
        let body = usage_error(&canon_in(repo, args));
        assert_eq!(body["error"]["hint"], "--change was renamed --change-id in 0.24.0; use --change-id", "{args:?}: {body}");
    }
    let canonical = canon_in(repo, &["brief", "check", "--change-id", "add-login", "--json"]);
    let report: Value = serde_json::from_slice(&canonical.stdout).expect("one JSON document");
    assert_eq!(report["data"]["change"], "add-login", "{report}");
    assert!(report["meta"].get("deprecated").is_none(), "{report}");

    // Every removed flag, on a command that renamed it.
    for (args, old, new) in [
        (&["scenario", "new", "demo.login.01", "--title", "A", "--actor", "alice", "--json"][..], "--actor", "--actor-id"),
        (&["scenario", "new", "demo.login.01", "--title", "A", "--project", "root", "--json"], "--project", "--project-id"),
        (&["dispatch", "end", "--run", "r1", "--json"], "--run", "--run-id"),
    ] {
        let body = usage_error(&canon_in(repo, args));
        assert_eq!(body["error"]["hint"], format!("{old} was renamed {new} in 0.24.0; use {new}"), "{args:?}: {body}");
    }

    // Without --json the refusal is still exit 2, on stderr, with the hint.
    let human = canon_in(repo, &["brief", "check", "--change", "add-login"]);
    assert_eq!(human.status.code(), Some(2));
    assert!(stderr(&human).contains("use --change-id"), "{}", stderr(&human));
}

/// context.capability.17: a removed `--kind` value is a usage error naming the
/// value that replaced it.
#[test]
fn context_capability_17_a_removed_kind_value_is_refused_with_the_value_that_replaced_it() {
    let dir = repo_with_change();
    let repo = dir.path();

    for (args, old, new) in [
        (&["query", "--kind", "review_record", "--json"][..], "review_record", "review"),
        (&["export", "--kind", "evidence_record", "--json"], "evidence_record", "evidence"),
        (&["query", "--kind=finding_record", "--json"], "finding_record", "finding"),
        (&["purge", "--kind=session_record", "--before", "2020-01-01T00:00:00Z", "--plan", "--json"], "session_record", "session"),
    ] {
        let body = usage_error(&canon_in(repo, args));
        assert_eq!(body["error"]["hint"], format!("--kind {old} was renamed --kind {new} in 0.24.0; use --kind {new}"), "{args:?}: {body}");
    }

    let canonical = canon_in(repo, &["query", "--kind", "evidence", "--json"]);
    assert!(canonical.status.success(), "{}", stderr(&canonical));
    assert!(serde_json::from_slice::<Value>(&canonical.stdout).unwrap()["meta"].get("deprecated").is_none());

    // An unknown value that was never a spelling gets no rename hint.
    let unknown = usage_error(&canon_in(repo, &["query", "--kind", "nonsense", "--json"]));
    assert!(!unknown.to_string().contains("renamed"), "{unknown}");
}

/// context.capability.18: help and schema show only the one name.
#[test]
fn context_capability_18_help_and_schema_show_only_the_one_name() {
    let dir = tempfile::tempdir().unwrap();
    for (args, canonical) in [(&["brief", "approve", "--help"][..], "--change-id"), (&["change", "new", "--help"], "--subject-id"), (&["query", "--help"], "--kind <KIND>"), (&["scenario", "new", "--help"], "--subject-id")] {
        let help = stdout(&canon_in(dir.path(), args));
        assert!(help.contains(canonical), "{args:?} help shows the canonical `{canonical}`:\n{help}");
        for old in ["--change ", "--change\n", "--subject ", "--subject\n", "--project ", "--actor ", "--run "] {
            assert!(!help.contains(old), "{args:?} help shows the removed `{old:?}`:\n{help}");
        }
        assert!(!help.contains("_record"), "{args:?} help shows a _record kind:\n{help}");
        if args[0] == "query" {
            assert!(help.contains("evidence") && help.contains("review"), "query help lists the kinds without the suffix:\n{help}");
        }
    }

    for command in [&["brief", "approve"][..], &["scenario", "new"], &["dispatch", "end"], &["query"], &["export"], &["purge"]] {
        let mut args = vec!["schema"];
        args.extend(command);
        args.push("--json");
        let schema = schema_json(&args);
        for option in schema["options"].as_array().unwrap() {
            assert!(option.get("deprecatedAliases").is_none() && option.get("deprecatedValues").is_none(), "{command:?}: {option}");
            assert!(option["aliases"].as_array().unwrap().is_empty(), "{command:?}: no alias keeps an old spelling: {option}");
        }
    }
    let kind = schema_json(&["schema", "query", "--json"])["options"].as_array().unwrap().iter().find(|o| o["name"] == "kind").unwrap().clone();
    let possible: Vec<&str> = kind["possibleValues"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert!(possible.contains(&"evidence") && possible.iter().all(|v| !v.ends_with("_record")), "{possible:?}");
}

/// context.capability.19: only a token clap reads as a flag is a removed spelling.
#[test]
fn context_capability_19_a_token_after_the_separator_or_an_option_value_is_never_a_removed_spelling() {
    let dir = repo_with_change();
    let repo = dir.path();

    // A value that is spelled like a removed flag.
    let value = canon_in(repo, &["change", "new", "add-search", "--subject-id", "demo", "--title=--subject", "--json"]);
    assert!(value.status.success(), "{}", stderr(&value));
    let brief = std::fs::read_to_string(repo.join(".canon/changes/add-search/brief.md")).unwrap();
    assert!(brief.contains("--subject"), "the value reached the brief untouched: {brief}");

    // Lines clap refuses for another reason (`--bogus`), so the removed-spelling
    // scan runs: a removed spelling after `--` or as an option's value is not
    // reported.
    for args in [
        &["brief", "check", "--bogus", "--json", "--", "--change"][..],
        &["brief", "check", "--bogus", "--change-id", "--change", "--json"],
        &["brief", "check", "--bogus", "--change-id=--change", "--json"],
    ] {
        let refused = usage_error(&canon_in(repo, args));
        let hint = refused["error"]["hint"].as_str().unwrap_or_default().to_string();
        assert!(!refused.to_string().contains("was removed") && !hint.contains("--change "), "{args:?}: {refused}");
    }
    // The same line with the spelling as a real flag is reported.
    let flagged = usage_error(&canon_in(repo, &["brief", "check", "--bogus", "--change", "x", "--json"]));
    assert!(flagged["error"]["hint"].as_str().unwrap().contains("use --change-id"), "{flagged}");

    // As a flag it is refused, with a space or an equals sign.
    for args in [&["brief", "check", "--change=add-login", "--json"][..], &["brief", "check", "--change", "add-login", "--json"]] {
        usage_error(&canon_in(repo, args));
    }
}

/// context.capability.20: a removed spelling is refused only on the commands
/// that renamed it.
#[test]
fn context_capability_20_a_removed_spelling_is_refused_only_on_the_commands_that_renamed_it() {
    let dir = repo_with_change();
    let other = usage_error(&canon_in(dir.path(), &["status", "--subject", "demo", "--json"]));
    assert!(!other.to_string().contains("renamed"), "{other}");
    let renamed = usage_error(&canon_in(dir.path(), &["brief", "check", "--change", "--json"]));
    assert!(renamed["error"]["hint"].as_str().unwrap().contains("use --change-id"), "{renamed}");
}

/// context.capability.21: a `scenario new --from` line with a removed key refuses
/// the batch and names the key to use.
#[test]
fn context_capability_21_a_scenario_batch_line_with_a_removed_key_name_refuses_the_batch() {
    let dir = repo_with_change();
    let repo = dir.path();
    let file = repo.join("batch.ndjson");
    for (key, new) in [("subject", "subject-id"), ("project", "project-id"), ("change", "change-id")] {
        let lines = [r#"{"id":"demo.login.01","title":"One","subject-id":"demo"}"#.to_string(), format!(r#"{{"id":"demo.login.02","title":"Two","{key}":"{}"}}"#, match key { "subject" => "demo", "project" => "root", _ => "add-login" })];
        std::fs::write(&file, lines.join("\n")).unwrap();
        let out = canon_in(repo, &["scenario", "new", "--from", "batch.ndjson", "--json"]);
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        let body: Value = serde_json::from_slice(&out.stdout).expect("one JSON document");
        assert_eq!(body["error"]["code"], "SCAFFOLD_INVALID_ARGUMENT", "{body}");
        let item = &body["data"]["items"][0];
        assert_eq!(item["line"], 2, "{body}");
        assert_eq!(item["error"]["field"], key, "{body}");
        assert_eq!(item["error"]["hint"], format!("`{key}` was renamed `{new}` in 0.24.0; use `{new}`"), "{body}");
        assert!(!repo.join(".canon/specs/kind=feature/area=demo/login.feature").exists(), "a refused batch writes nothing");
    }

    let lines = [
        r#"{"id":"demo.login.01","title":"One","subject-id":"demo","project-id":"root"}"#,
        r#"{"id":"demo.login.02","title":"Two","subject-id":"demo","case":"failure","flow":"F1","change-id":"add-login"}"#,
    ];
    std::fs::write(&file, lines.join("\n")).unwrap();
    let brief_path = repo.join(".canon/changes/add-login/brief.md");
    let brief = std::fs::read_to_string(&brief_path).unwrap().replace("## User flow\n", "## User flow\n\n1. F1 The visitor submits credentials → lands on the catalog.\n");
    std::fs::write(&brief_path, brief).unwrap();
    let out = canon_in(repo, &["scenario", "new", "--from", "batch.ndjson", "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let feature = std::fs::read_to_string(repo.join(".canon/specs/kind=feature/area=demo/login.feature")).unwrap();
    assert_eq!(feature.matches("@subject:demo").count(), 2, "{feature}");
}
