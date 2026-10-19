//! One name per concept (0.24 D1-D3, #85) through the built binary.
//!
//! The contract: a concept has one flag spelling across every command
//! (`--<noun>-id`), a `--kind` value names a record kind without the
//! `_record` suffix, and the 0.23 spellings keep running for one release,
//! each use warning once on stderr and, with `--json`, in `meta.deprecated`.
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
/// arguments only, so a hidden deprecated alias is not part of the surface).
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

// ── the deprecated spellings ─────────────────────────────────────────────

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

fn deprecation_lines(out: &Output) -> Vec<String> {
    stderr(out).lines().filter(|l| l.contains("is deprecated")).map(str::to_string).collect()
}

/// context.capability.16: a deprecated flag still runs, warns once on stderr and
/// is named in `meta.deprecated`; the canonical spelling does neither.
#[test]
fn context_capability_16_a_deprecated_flag_spelling_still_runs_and_warns_once() {
    let dir = repo_with_change();
    let repo = dir.path();

    let old = canon_in(repo, &["change", "new", "add-search", "--subject", "demo", "--title", "Add search", "--json"]);
    assert!(old.status.success(), "{}", stderr(&old));
    assert_eq!(deprecation_lines(&old), vec!["canon change new: --subject is deprecated — use --subject-id (removed in 0.25.0)".to_string()]);
    let body: Value = serde_json::from_slice(&old.stdout).expect("one JSON document");
    assert_eq!(body["meta"]["deprecated"], serde_json::json!([{ "used": "--subject", "use": "--subject-id", "removedIn": "0.25.0" }]), "{body}");
    assert!(repo.join(".canon/changes/add-search/brief.md").is_file(), "the old spelling ran the command");

    let new = canon_in(repo, &["change", "new", "add-cart", "--subject-id", "demo", "--title", "Add cart", "--json"]);
    assert!(deprecation_lines(&new).is_empty(), "{}", stderr(&new));
    assert!(serde_json::from_slice::<Value>(&new.stdout).unwrap()["meta"].get("deprecated").is_none());

    // The `=` form is the same flag; a global argument of `brief` covers its subcommands; a
    // command whose JSON is not an envelope carries the same entry in its top-level `meta`.
    let equals = canon_in(repo, &["brief", "check", "--change=add-login", "--json"]);
    assert_eq!(deprecation_lines(&equals), vec!["canon brief check: --change is deprecated — use --change-id (removed in 0.25.0)".to_string()]);
    let report: Value = serde_json::from_slice(&equals.stdout).expect("one JSON document");
    assert_eq!(report["change"], "add-login", "the old spelling checked the named change: {report}");
    assert_eq!(report["meta"]["deprecated"][0]["use"], "--change-id", "{report}");
    let before_command = canon_in(repo, &["brief", "--change", "add-login", "check"]);
    assert_eq!(deprecation_lines(&before_command).len(), 1, "{}", stderr(&before_command));

    // Two deprecated spellings in one call: one line and one entry each.
    let both = canon_in(repo, &["scenario", "new", "demo.login.01", "--title", "A sign-in", "--subject", "demo", "--actor", "alice", "--json"]);
    assert!(both.status.success(), "{}", stderr(&both));
    assert_eq!(deprecation_lines(&both).len(), 2, "{}", stderr(&both));
    let body: Value = serde_json::from_slice(&both.stdout).unwrap();
    assert_eq!(body["meta"]["deprecated"].as_array().unwrap().len(), 2, "{body}");
}

/// context.capability.17: a deprecated `--kind` value still runs and names the
/// value to use.
#[test]
fn context_capability_17_a_deprecated_kind_value_still_runs_and_warns_with_the_value_to_use() {
    let dir = repo_with_change();
    let repo = dir.path();

    for (command, old, new) in [("query", "review_record", "review"), ("export", "evidence_record", "evidence"), ("query", "finding_record", "finding")] {
        let out = canon_in(repo, &[command, "--kind", old, "--json"]);
        assert!(out.status.success(), "{command} {old}: {}", stderr(&out));
        assert_eq!(deprecation_lines(&out), vec![format!("canon {command}: --kind {old} is deprecated — use --kind {new} (removed in 0.25.0)")], "{command} {old}");
        let body: Value = serde_json::from_slice(&out.stdout).expect("one JSON document");
        assert_eq!(body["meta"]["deprecated"], serde_json::json!([{ "used": format!("--kind {old}"), "use": format!("--kind {new}"), "removedIn": "0.25.0" }]), "{body}");
    }
    let equals = canon_in(repo, &["purge", "--kind=session_record", "--before", "2020-01-01T00:00:00Z", "--plan", "--json"]);
    assert!(equals.status.success(), "{}", stderr(&equals));
    assert_eq!(deprecation_lines(&equals).len(), 1, "{}", stderr(&equals));

    let canonical = canon_in(repo, &["query", "--kind", "evidence", "--json"]);
    assert!(canonical.status.success() && deprecation_lines(&canonical).is_empty(), "{}", stderr(&canonical));
    assert!(serde_json::from_slice::<Value>(&canonical.stdout).unwrap()["meta"].get("deprecated").is_none());

    let unknown = canon_in(repo, &["query", "--kind", "nonsense", "--json"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(deprecation_lines(&unknown).is_empty());
}

/// context.capability.18: help hides the old spellings; schema lists them.
#[test]
fn context_capability_18_help_hides_the_deprecated_spellings_and_schema_lists_them() {
    let dir = tempfile::tempdir().unwrap();
    for (args, canonical) in [(&["brief", "approve", "--help"][..], "--change-id"), (&["change", "new", "--help"], "--subject-id"), (&["query", "--help"], "--kind <KIND>"), (&["scenario", "new", "--help"], "--subject-id")] {
        let help = stdout(&canon_in(dir.path(), args));
        assert!(help.contains(canonical), "{args:?} help shows the canonical `{canonical}`:\n{help}");
        for old in ["--change ", "--change\n", "--subject ", "--subject\n", "--project ", "--actor ", "--run "] {
            assert!(!help.contains(old), "{args:?} help shows the deprecated `{old:?}`:\n{help}");
        }
        assert!(!help.contains("_record"), "{args:?} help shows a _record kind:\n{help}");
        if args[0] == "query" {
            assert!(help.contains("evidence") && help.contains("review"), "query help lists the kinds without the suffix:\n{help}");
        }
    }

    let options = |command: &[&str]| -> Vec<Value> {
        let mut args = vec!["schema"];
        args.extend(command);
        args.push("--json");
        schema_json(&args)["options"].as_array().unwrap().clone()
    };
    let option = |options: &[Value], name: &str| options.iter().find(|o| o["name"] == name).unwrap_or_else(|| panic!("no --{name}")).clone();

    let brief_approve = options(&["brief", "approve"]);
    assert_eq!(option(&brief_approve, "change-id")["deprecatedAliases"], serde_json::json!(["--change"]));
    assert!(option(&brief_approve, "by").get("deprecatedAliases").is_none(), "only renamed options carry the field");
    let scenario_new = options(&["scenario", "new"]);
    for (new, old) in [("subject-id", "--subject"), ("change-id", "--change"), ("project-id", "--project"), ("actor-id", "--actor")] {
        assert_eq!(option(&scenario_new, new)["deprecatedAliases"], serde_json::json!([old]), "{new}");
    }
    assert_eq!(option(&options(&["dispatch", "end"]), "run-id")["deprecatedAliases"], serde_json::json!(["--run"]));

    for command in ["query", "export", "purge"] {
        let kind = option(&options(&[command]), "kind");
        let values = kind["deprecatedValues"].as_array().unwrap();
        assert_eq!(values.len(), 14, "{command}");
        assert!(values.contains(&serde_json::json!({ "value": "evidence_record", "use": "evidence" })), "{command}: {values:?}");
        assert!(values.contains(&serde_json::json!({ "value": "review_record", "use": "review" })), "{command}: {values:?}");
        let possible: Vec<&str> = kind["possibleValues"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert!(possible.contains(&"evidence") && possible.iter().all(|v| !v.ends_with("_record")), "{command}: {possible:?}");
    }
    assert!(option(&options(&["change", "new"]), "kind").get("deprecatedValues").is_none(), "change new's --kind is not a record kind");
}

/// context.capability.19: only a token clap reads as a flag counts.
#[test]
fn context_capability_19_a_token_after_the_separator_or_an_option_value_is_never_a_deprecated_spelling() {
    let dir = repo_with_change();
    let repo = dir.path();

    // A value that is spelled like a deprecated flag.
    let value = canon_in(repo, &["change", "new", "add-search", "--subject-id", "demo", "--title=--subject", "--json"]);
    assert!(value.status.success(), "{}", stderr(&value));
    assert!(deprecation_lines(&value).is_empty(), "{}", stderr(&value));
    let brief = std::fs::read_to_string(repo.join(".canon/changes/add-search/brief.md")).unwrap();
    assert!(brief.contains("--subject"), "the value reached the brief untouched: {brief}");

    // Everything after `--` is positional.
    let after = canon_in(repo, &["schema", "--json", "--", "--change"]);
    assert!(deprecation_lines(&after).is_empty(), "{}", stderr(&after));

    // `--change=<slug>` with an equals sign is a flag, read as the deprecated spelling like `--change <slug>`.
    let equals = canon_in(repo, &["brief", "check", "--change=add-login", "--json"]);
    let spaced = canon_in(repo, &["brief", "check", "--change", "add-login", "--json"]);
    let canonical = canon_in(repo, &["brief", "check", "--change-id", "add-login", "--json"]);
    let expected = "canon brief check: --change is deprecated — use --change-id (removed in 0.25.0)".to_string();
    assert_eq!(deprecation_lines(&equals), vec![expected.clone()], "{}", stderr(&equals));
    assert_eq!(deprecation_lines(&spaced), vec![expected], "{}", stderr(&spaced));
    assert!(deprecation_lines(&canonical).is_empty(), "{}", stderr(&canonical));
    assert_eq!(equals.status.code(), canonical.status.code());
}

/// context.capability.20: a usage error names the canonical spelling.
#[test]
fn context_capability_20_a_usage_error_names_the_canonical_spelling() {
    let dir = repo_with_change();
    let out = canon_in(dir.path(), &["brief", "check", "--change", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    let body: Value = serde_json::from_slice(&out.stdout).expect("one JSON document");
    assert_eq!(body["error"]["code"], "USAGE", "{body}");
    let text = body.to_string();
    assert!(text.contains("--change-id") && !text.contains("'--change'"), "{text}");
    assert!(!body["error"]["hint"].as_str().unwrap().is_empty(), "{body}");
}

/// context.capability.21: a `scenario new --from` line may still use the old key names.
#[test]
fn context_capability_21_a_scenario_batch_line_may_still_carry_the_deprecated_key_names() {
    let dir = repo_with_change();
    let repo = dir.path();
    let lines = [
        r#"{"id":"demo.login.01","title":"One","subject":"demo","project":"root"}"#,
        r#"{"id":"demo.login.02","title":"Two","subject":"demo","case":"failure"}"#,
        r#"{"id":"demo.login.03","title":"Three","subject-id":"demo","project-id":"root"}"#,
    ];
    let file = repo.join("batch.ndjson");
    std::fs::write(&file, lines.join("\n")).unwrap();
    let out = canon_in(repo, &["scenario", "new", "--from", "batch.ndjson", "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let mut seen = deprecation_lines(&out);
    seen.sort();
    assert_eq!(
        seen,
        vec![
            "canon scenario new: --project is deprecated — use --project-id (removed in 0.25.0)".to_string(),
            "canon scenario new: --subject is deprecated — use --subject-id (removed in 0.25.0)".to_string(),
        ]
    );
    let body: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["meta"]["deprecated"].as_array().unwrap().len(), 2, "{body}");
    let feature = std::fs::read_to_string(repo.join(".canon/specs/kind=feature/area=demo/login.feature")).unwrap();
    assert_eq!(feature.matches("@subject:demo").count(), 3, "{feature}");
}
