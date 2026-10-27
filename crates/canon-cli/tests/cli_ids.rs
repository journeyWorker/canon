//! 0.31 (`canon-0-31-cli-ids`): an option that takes an id is the bare noun, through the real binary. Every test is
//! named by the scenario it proves (`context.capability.47` to `.60`).
//!
//! The 0.30 → 0.31 parity of what the renamed options write is the conformance corpus: `conformance/cases` was re-blessed
//! with the new spellings and every expected file differs from the 0.30 one in nothing but the spelling.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

mod harness;

/// The 14 options that took an id in 0.30, with the places they were on.
const NOUNS: [&str; 14] = ["actor", "session", "project", "change", "run", "task", "subject", "scenario", "skill", "parent-run", "evaluator", "review", "plugin", "contradicting-trajectory"];

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").env_remove("CANON_ROLE").current_dir(repo).output().expect("spawn canon")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("one JSON document ({e}): {}{}", stdout(out), stderr(out)))
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}{}", stdout(&out), stderr(&out));
    out
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(repo).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// Every `--<noun>-id` of the 14 nouns in `text`, as spelled.
fn old_spellings(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(pos) = text[at..].find("--") {
        let start = at + pos;
        let before_ok = start == 0 || !(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'-');
        let end = text[start + 2..].find(|c: char| !(c.is_ascii_lowercase() || c == '-')).map_or(text.len(), |n| start + 2 + n);
        let word = &text[start..end];
        if before_ok && word.strip_suffix("-id").is_some_and(|stem| NOUNS.contains(&stem.trim_start_matches("--"))) {
            found.push(word.to_string());
        }
        at = (start + 2).min(text.len());
    }
    found
}

/// A git repo with one spec root `root`, a subject `demo` and a change `add-login` whose brief has the flow F1.
fn repo_with_change() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    ok(dir.path(), &["init", "--repo", ".", "--no-policy", "--providers", "none"]);
    ok(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(dir.path(), &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    let brief_path = dir.path().join(".canon/changes/add-login/brief.md");
    let brief = std::fs::read_to_string(&brief_path).unwrap().replace("## User flow\n", "## User flow\n\n1. F1 The visitor submits credentials → lands on the catalog.\n");
    std::fs::write(&brief_path, brief).unwrap();
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "x"]);
    dir
}

const FEATURE_REL: &str = "kind=feature/area=cart/add.feature";
const FEATURE: &str = "Feature: cart add
  # canon: {\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}

  # canon: {\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}
  @cart.add.01
  Scenario: Behavior cart.add.01
    Given a cart
    When the shopper acts
    Then the cart holds the item
";

/// A git repo whose canon.yaml configures `roots`, each holding `cart.add.01`, indexed, with one commit.
fn repo_with_roots(roots: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let mut yaml = String::from("tiers:\n  local:\n    backend: git\n    root: .canon/ledger\nspecs:\n  roots:\n");
    for id in roots {
        yaml.push_str(&format!("    - id: {id}\n      root: specs-{id}\n"));
        let feature = repo.join(format!("specs-{id}")).join(FEATURE_REL);
        std::fs::create_dir_all(feature.parent().unwrap()).unwrap();
        std::fs::write(feature, FEATURE).unwrap();
    }
    yaml.push_str("routing:\n  scenario: local\n  evidence: local\n  review: local\n  divergence: local\n");
    std::fs::write(repo.join("canon.yaml"), yaml).unwrap();
    git(repo, &["init", "-q"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "seed"]);
    ok(repo, &["inventory", "sync"]);
    dir
}

/// The command line a hint names between backticks, as argv without `canon`.
fn hint_line(body: &Value) -> Vec<String> {
    let hint = body["error"]["hint"].as_str().unwrap_or_default();
    let line = hint.split('`').nth(1).unwrap_or_else(|| panic!("no command line in the hint: {hint}"));
    let line = line.strip_prefix("canon ").unwrap_or(line);
    harness::words(line)
}

fn usage_error(out: &Output) -> Value {
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(out), stderr(out));
    let body = json(out);
    assert_eq!(body["error"]["code"], "USAGE", "{body}");
    body
}

fn unchanged(repo: &Path, before: &BTreeMap<PathBuf, Option<Vec<u8>>>, what: &str) {
    let changed = harness::changed(before, &harness::tree(repo));
    let changed: Vec<_> = changed.iter().filter(|p| !p.to_string_lossy().contains(".canon/audit")).collect();
    assert!(changed.is_empty(), "{what} wrote {changed:?}");
}

/// The id options of the spec, by name, with the leaves that carry each.
fn options_by_name() -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for leaf in harness::leaves() {
        for option in harness::options(leaf) {
            if let Some(name) = option["name"].as_str() {
                out.entry(name.to_string()).or_default().push(harness::path_of(leaf));
            }
        }
    }
    out
}

/// Masks times, ULIDs and digests, the values that differ between two runs of one fixture.
fn masked(text: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        let suffix = if word.ends_with(".json") { ".json" } else { "" };
        let bare = word.strip_suffix(suffix).unwrap_or(word).to_string();
        let word_text = std::mem::replace(word, bare);
        let digits = word.len();
        let timestamp = digits >= 20 && word.as_bytes()[4] == b'-' && word.contains('T') && word.ends_with('Z');
        let ulid = digits == 26 && word.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        let digest = (digits >= 40 || digits == 8) && word.chars().all(|c| c.is_ascii_hexdigit()) && word.chars().any(|c| c.is_ascii_digit());
        out.push_str(&if timestamp {
            format!("<time>{suffix}")
        } else if ulid {
            format!("<ulid>{suffix}")
        } else if digest {
            format!("<digest>{suffix}")
        } else {
            word_text
        });
        word.clear();
    };
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | ':' | '.') {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

fn ledger_records(repo: &Path, kind: &str) -> Vec<Value> {
    let dir = repo.join(format!(".canon/ledger/kind={kind}"));
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    files.sort();
    files.iter().map(|f| serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap()).collect()
}

// ── F1 ───────────────────────────────────────────────────────────────────

/// context.capability.47: the 14 options are the bare nouns on the 67 places of 24 commands, and a record written
/// through them keeps the `*_id` keys it had.
#[test]
fn context_capability_47_every_id_option_is_the_bare_noun_and_writes_the_record_the_old_spelling_wrote() {
    let by_name = options_by_name();
    let mut places = 0;
    let mut commands = std::collections::BTreeSet::new();
    for (noun, count) in NOUNS.iter().zip([16, 15, 11, 6, 4, 3, 3, 3, 1, 1, 1, 1, 1, 1]) {
        let leaves = by_name.get(*noun).unwrap_or_else(|| panic!("no command takes --{noun}"));
        assert_eq!(leaves.len(), count, "--{noun} is on {leaves:?}");
        assert!(!by_name.contains_key(&format!("{noun}-id")), "--{noun}-id is still an option of {:?}", by_name.get(&format!("{noun}-id")));
        places += leaves.len();
        commands.extend(leaves.iter().cloned());
    }
    assert!(by_name.keys().all(|name| !name.ends_with("-id")), "{:?}", by_name.keys().filter(|n| n.ends_with("-id")).collect::<Vec<_>>());
    assert_eq!((places, commands.len()), (67, 24));

    let dir = repo_with_change();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "demo.login.01", "--title", "One", "--subject", "demo", "--actor", "impl-1", "--session", "s-1", "--project", "root"]);
    ok(repo, &["inventory", "sync"]);
    let feature = std::fs::read_to_string(repo.join(".canon/specs/kind=feature/area=demo/login.feature")).unwrap();
    assert!(feature.contains(r#""agent_id":"impl-1""#) && feature.contains(r#""session_id":"s-1""#), "{feature}");
    let attested = repo_with_roots(&["root"]);
    let report = json(&ok(attested.path(), &["evidence", "add", "--project", "root", "--scenario", "cart.add.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor", "impl-1", "--session", "s-1", "--json"]));
    assert_eq!(report["ok"], true, "{report}");
    ok(attested.path(), &["gate", "promote"]);
    let evidence = &ledger_records(attested.path(), "evidence")[0];
    assert_eq!(evidence["scenario_id"], "cart.add.01", "{evidence}");
    assert_eq!(evidence["project_id"], "root", "{evidence}");
    assert_eq!(evidence["actor"]["agent_id"], "impl-1", "{evidence}");
    assert_eq!(evidence["actor"]["session_id"], "s-1", "{evidence}");
    ok(repo, &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--introduced-unknown", "--actor", "rev-1", "--session", "s-2", "--scenario", "demo.login.01", "--summary", "Token is logged"]);
    let finding = &ledger_records(repo, "finding")[0];
    assert_eq!(finding["change_id"], "add-login", "{finding}");
    assert_eq!(finding["scenario_ids"], serde_json::json!(["demo.login.01"]), "{finding}");
}

/// context.capability.48: the id-suffixed spelling on the same line exits 2 and writes nothing.
#[test]
fn context_capability_48_the_old_change_spelling_on_the_same_line_exits_2_and_the_ledger_is_unchanged() {
    let dir = repo_with_change();
    let repo = dir.path();
    let args = ["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--introduced-unknown", "--actor", "rev-1", "--summary", "s", "--json"];
    let before = harness::tree(repo);
    let old: Vec<&str> = args.iter().map(|a| if *a == "--change" { "--change-id" } else { *a }).collect();
    let body = usage_error(&canon(repo, &old));
    assert_eq!(body["error"]["field"], "--change-id", "{body}");
    assert_eq!(body["error"]["message"], "--change-id was removed in 0.31.0", "{body}");
    assert_eq!(hint_line(&body), args.to_vec(), "{body}");
    unchanged(repo, &before, "the refused line");
    ok(repo, &args);
    assert_eq!(ledger_records(repo, "finding").len(), 1);
}

// ── F2 ───────────────────────────────────────────────────────────────────

/// context.capability.49: each row of the table, on each command it names, in the space, equals and missing-value forms.
#[test]
fn context_capability_49_every_removed_id_spelling_is_a_usage_error_naming_the_bare_spelling() {
    let dir = repo_with_change();
    let repo = dir.path();
    let before = harness::tree(repo);
    let mut exercised = 0;
    for row in canon_cli::removed_028::ROWS.iter().filter(|row| row.since == canon_cli::removed_028::V31) {
        for scope in row.scope {
            let leaf: Vec<&str> = scope.split(' ').collect();
            let joined = format!("{}=value", row.old);
            for (form, tail) in [("space", vec![row.old, "value"]), ("equals", vec![joined.as_str()]), ("missing value", vec![row.old])] {
                let mut args = leaf.clone();
                args.extend(tail);
                args.push("--json");
                let body = usage_error(&canon(repo, &args));
                assert_eq!(body["error"]["field"], row.old, "{scope} {form}: {body}");
                assert_eq!(body["error"]["expected"], serde_json::json!([row.new]), "{scope} {form}: {body}");
                assert_eq!(body["error"]["message"], format!("{} was removed in 0.31.0", row.old), "{scope} {form}: {body}");
                let line = hint_line(&body);
                assert!(line.iter().any(|w| w.split('=').next() == Some(row.new)), "{scope} {form}: the hint line names {}: {line:?}", row.new);
                assert!(old_spellings(&line.join(" ")).is_empty(), "{scope} {form}: {line:?}");
                exercised += 1;
            }
        }
    }
    assert_eq!(exercised, 67 * 3);
    unchanged(repo, &before, "a refused spelling");
}

/// context.capability.50: not an option, not removed; a command that never had it gets the ordinary error; 0.28's `--task` runs again.
#[test]
fn context_capability_50_a_token_after_the_separator_or_an_option_value_is_never_read_as_an_old_id_spelling() {
    let dir = repo_with_change();
    let repo = dir.path();
    for args in [
        &["scenario", "new", "demo.login.01", "--title", "A", "--bogus", "--json", "--", "--change-id"][..],
        &["scenario", "new", "demo.login.01", "--title", "A", "--bogus", "--lane", "--change-id", "--json"],
        &["scenario", "new", "demo.login.01", "--title", "A", "--bogus", "--lane=--actor-id", "--json"],
    ] {
        let refused = usage_error(&canon(repo, args));
        assert!(!refused.to_string().contains("was removed"), "{args:?}: {refused}");
    }
    // A command that never had the spelling: the ordinary unknown-argument error, no removal hint.
    let never = usage_error(&canon(repo, &["status", "--actor-id", "x", "--json"]));
    assert!(!never.to_string().contains("was removed"), "{never}");
    assert_eq!(never["error"]["field"], "--actor-id", "{never}");
    // The spelling 0.28 removed is valid again: the line reaches the command, which checks the task itself.
    let task = canon(repo, &["dispatch", "begin", "--task", "add-login#1", "--regime", "implementer/canon/search/abcdef", "--json"]);
    let said = stdout(&task);
    assert!(!said.contains("was removed"), "{said}");
}

/// context.capability.51: several old spellings are refused once; the hint line runs as printed.
#[test]
fn context_capability_51_a_line_with_several_old_id_spellings_is_refused_once_and_its_hint_line_runs() {
    let dir = repo_with_change();
    let repo = dir.path();
    let typed = ["scenario", "new", "demo.login.01", "--title", "One", "--subject-id", "demo", "--change-id=add-login", "--flow", "F1", "--case", "happy", "--actor-id", "impl-1", "--session-id", "s-1", "--project-id", "root", "--json"];
    let before = harness::tree(repo);
    let body = usage_error(&canon(repo, &typed));
    assert_eq!(body["error"]["field"], "--subject-id", "the message names the first spelling in the line: {body}");
    let line = hint_line(&body);
    assert!(old_spellings(&line.join(" ")).is_empty(), "{line:?}");
    assert!(line.contains(&"--change=add-login".to_string()), "{line:?}");
    unchanged(repo, &before, "the refused line");
    let refs: Vec<&str> = line.iter().map(String::as_str).collect();
    ok(repo, &refs);
    let feature = std::fs::read_to_string(repo.join(".canon/specs/kind=feature/area=demo/login.feature")).unwrap();
    assert!(feature.contains("@flow:add-login.F1") && feature.contains("@case:happy"), "{feature}");

    // A value that moved into an operand in 0.28 on the same line.
    let adopt = usage_error(&canon(repo, &["subject", "adopt", "--subject-id", "demo", "--change-id", "add-login", "--json"]));
    assert_eq!(adopt["error"]["field"], "--subject-id", "{adopt}");
    let line = hint_line(&adopt);
    assert_eq!(line[..3], ["subject", "adopt", "demo"], "{line:?}");
    assert!(old_spellings(&line.join(" ")).is_empty(), "{line:?}");
    let refs: Vec<&str> = line.iter().map(String::as_str).collect();
    ok(repo, &refs);
}

// ── F3 ───────────────────────────────────────────────────────────────────

/// context.capability.52: the document with the bare keys and the flags write the same bytes.
#[test]
fn context_capability_52_an_input_document_keyed_by_the_bare_option_names_writes_what_the_flags_write() {
    let flags = repo_with_change();
    let document = repo_with_change();
    let (a, b) = (flags.path(), document.path());
    let scenario_flags = ["scenario", "new", "demo.login.01", "--title", "One", "--subject", "demo", "--case", "happy", "--flow", "F1", "--change", "add-login", "--actor", "impl-1", "--session", "s-1", "--json"];
    let scenario_doc = r#"{"operands":["demo.login.01"],"title":"One","subject":"demo","case":"happy","flow":"F1","change":"add-login","actor":"impl-1","session":"s-1"}"#;
    let out_a = ok(a, &scenario_flags);
    let out_b = ok(b, &["scenario", "new", "--input", scenario_doc, "--json"]);
    let norm = |out: &Output, repo: &Path| masked(&stdout(out).replace(&*repo.to_string_lossy(), "<repo>"));
    assert_eq!(norm(&out_a, a), norm(&out_b, b));
    let read = |repo: &Path| masked(&std::fs::read_to_string(repo.join(".canon/specs/kind=feature/area=demo/login.feature")).unwrap());
    assert_eq!(read(a), read(b));

    let (flags, document) = (repo_with_roots(&["root"]), repo_with_roots(&["root"]));
    let (a, b) = (flags.path(), document.path());
    let evidence_flags = ["evidence", "add", "--project", "root", "--scenario", "cart.add.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor", "impl-1", "--session", "s-1", "--json"];
    let evidence_doc = r#"{"project":"root","scenario":"cart.add.01","class":"test-run","ref":"npm test","role":"implementer","actor":"impl-1","session":"s-1"}"#;
    let out_a = ok(a, &evidence_flags);
    let out_b = ok(b, &["evidence", "add", "--input", evidence_doc, "--json"]);
    assert_eq!(norm(&out_a, a), norm(&out_b, b));
    ok(a, &["gate", "promote"]);
    ok(b, &["gate", "promote"]);
    let record = |repo: &Path| masked(&serde_json::to_string(&ledger_records(repo, "evidence")[0]).unwrap());
    assert_eq!(record(a), record(b));
    let keys: Vec<String> = ledger_records(a, "evidence")[0].as_object().unwrap().keys().cloned().collect();
    assert!(keys.contains(&"scenario_id".to_string()) && keys.contains(&"project_id".to_string()), "{keys:?}");
    let data_keys: Vec<String> = json(&out_a)["data"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
    assert!(data_keys.iter().all(|k| !NOUNS.contains(&k.as_str()) || k == "actor"), "the report JSON keeps the keys it had: {data_keys:?}");
}

/// context.capability.53: a second element with an old key refuses the whole batch.
#[test]
fn context_capability_53_a_batch_whose_second_element_keeps_an_old_id_key_is_refused_and_writes_nothing() {
    let dir = repo_with_roots(&["root"]);
    let repo = dir.path();
    let before = harness::tree(repo);
    let doc = r#"[{"scenario":"cart.add.01","project":"root","class":"test-run","ref":"npm test","role":"implementer"},{"scenarioId":"cart.add.01","project":"root","class":"test-run","ref":"npm test again","role":"implementer"}]"#;
    let out = canon(repo, &["evidence", "add", "--input", doc, "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(&out), stderr(&out));
    let body = json(&out);
    assert_eq!(body["error"]["code"], "INPUT_UNKNOWN_FIELD", "{body}");
    assert_eq!(body["error"]["field"], "/items/1/scenarioId", "{body}");
    assert!(body["error"]["expected"].as_array().unwrap().iter().any(|k| k == "scenario"), "{body}");
    assert_eq!(body["error"]["suggestions"][0], "scenario", "{body}");
    assert!(body["error"]["hint"].as_str().unwrap().contains("use \"scenario\""), "{body}");
    unchanged(repo, &before, "a refused batch, its first element included");
    // The same batch with the bare key writes both.
    let fixed = doc.replace("scenarioId", "scenario");
    let out = ok(repo, &["evidence", "add", "--input", &fixed, "--json"]);
    assert_eq!(json(&out)["ok"], true);
}

// ── F4 ───────────────────────────────────────────────────────────────────

/// context.capability.54: two spec roots and no --project.
#[test]
fn context_capability_54_with_two_spec_roots_a_missing_project_is_refused_in_the_new_spelling() {
    let dir = repo_with_roots(&["game", "ui"]);
    let repo = dir.path();
    let before = harness::tree(repo);
    let out = canon(repo, &["feature", "new", "cart.extra", "--title", "Extra", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(&out), stderr(&out));
    let body = json(&out);
    assert_eq!(body["error"]["code"], "SPEC_ROOT_UNRESOLVED", "{body}");
    let message = body["error"]["message"].as_str().unwrap();
    let hint = body["error"]["hint"].as_str().unwrap();
    assert!(message.contains("--project <id>"), "{message}");
    assert!(hint.contains("--project <id>"), "the placeholder stays: {hint}");
    assert!(old_spellings(&format!("{message} {hint}")).is_empty(), "{message} {hint}");
    unchanged(repo, &before, "a refused feature new");
}

/// context.capability.55: an unknown root and an unknown review.
#[test]
fn context_capability_55_an_unknown_project_root_and_an_unknown_finding_review_are_refused_naming_the_new_options() {
    let dir = repo_with_roots(&["root"]);
    let repo = dir.path();
    let before = harness::tree(repo);
    let out = canon(repo, &["evidence", "add", "--project", "nope", "--scenario", "cart.add.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(&out), stderr(&out));
    let body = json(&out);
    assert_eq!(body["error"]["code"], "UNKNOWN_SPEC_ROOT", "{body}");
    assert_eq!(body["error"]["field"], "--project", "{body}");
    assert!(body["error"]["hint"].as_str().unwrap().contains("`--project root`"), "{body}");
    unchanged(repo, &before, "a refused evidence add");
    ok(repo, &["evidence", "add", "--project", "root", "--scenario", "cart.add.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--json"]);

    let dir = repo_with_change();
    let repo = dir.path();
    let before = harness::tree(repo);
    let out = canon(repo, &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--introduced-unknown", "--actor", "rev", "--summary", "s", "--review", "01ARZ3NDEKTSV4RRFFQ69G5FAV", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(&out), stderr(&out));
    let body = json(&out);
    assert_eq!(body["error"]["field"], "--review", "{body}");
    assert!(old_spellings(&body.to_string()).is_empty(), "{body}");
    unchanged(repo, &before, "a refused finding add");
}

/// context.capability.56: what canon prints as a command spells no old option, and parses.
#[test]
fn context_capability_56_every_command_line_canon_generates_uses_the_bare_spelling_and_parses() {
    let root = canon_cli::cli::command();
    // The examples of the spec and the help of every leaf.
    let mut texts: Vec<(String, String)> = Vec::new();
    for leaf in harness::leaves() {
        let argv = harness::example_argv(leaf);
        assert!(old_spellings(&argv.join(" ")).is_empty(), "{argv:?}");
        if let Err(e) = root.clone().try_get_matches_from(std::iter::once("canon".to_string()).chain(argv.clone())) {
            panic!("the example `{}` does not parse: {e}", argv.join(" "));
        }
        let path = harness::path_of(leaf);
        let dir = tempfile::tempdir().unwrap();
        let mut args: Vec<&str> = path.split(' ').collect();
        args.push("--help");
        texts.push((format!("{path} --help"), stdout(&canon(dir.path(), &args))));
    }
    // The steps, violations and refusals of a repo.
    let dir = repo_with_change();
    let repo = dir.path();
    for args in [&["status", "--json"][..], &["status"], &["gate", "check", "--json"], &["brief", "check", "add-login", "--json"]] {
        texts.push((args.join(" "), format!("{}{}", stdout(&canon(repo, args)), stderr(&canon(repo, args)))));
    }
    let two = repo_with_roots(&["game", "ui"]);
    for args in [&["status", "--json"][..], &["gate", "check", "--json"], &["feature", "new", "a.b", "--title", "T", "--json"]] {
        texts.push((format!("two roots: {}", args.join(" ")), stdout(&canon(two.path(), args))));
    }
    for (what, text) in &texts {
        assert!(old_spellings(text).is_empty(), "{what} spells {:?}", old_spellings(text));
    }
    // Every `meta.next` argv without a placeholder parses.
    let status = json(&canon(repo, &["status", "--json"]));
    let mut parsed = 0;
    for step in status["meta"]["next"].as_array().into_iter().flatten() {
        let argv: Vec<String> = step["argv"].as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_string()).collect();
        if argv.iter().any(|w| w.contains('<')) {
            continue;
        }
        if let Err(e) = root.clone().try_get_matches_from(argv.clone()) {
            panic!("`{}` does not parse: {e}", argv.join(" "));
        }
        parsed += 1;
    }
    let _ = parsed;
}

// ── F5 ───────────────────────────────────────────────────────────────────

/// context.capability.57: no option of the schema ends in -id; the skills and the website spell none.
#[test]
fn context_capability_57_the_schema_shows_no_option_ending_in_id_and_the_docs_spell_none() {
    for leaf in harness::leaves() {
        for option in harness::options(leaf) {
            let name = option["name"].as_str().unwrap();
            assert!(!name.ends_with("-id"), "{} has --{name}", harness::path_of(leaf));
            assert!(option["aliases"].as_array().is_none_or(Vec::is_empty), "{}: {option}", harness::path_of(leaf));
        }
    }
    let root = harness::repo_root();
    let mut problems = Vec::new();
    for (dir, ext) in [("canon/skills", "md"), ("packages/website/src/content/docs", "mdx"), ("docs/design", "tsv"), ("docs/design", "md")] {
        let mut files = Vec::new();
        walk(&root.join(dir), ext, &mut files);
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            for (n, line) in text.lines().enumerate() {
                // A line that says what an old spelling became names it on purpose.
                if line.contains("was removed") || line.contains("0.27 spelling") || line.contains("0.30 spelling") || line.contains("(0.27)") {
                    continue;
                }
                let old = old_spellings(line);
                if !old.is_empty() && !is_history(&file, line) {
                    problems.push(format!("{}:{}: {old:?}", file.strip_prefix(&root).unwrap().display(), n + 1));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{} line(s) spell an old id option:\n{}", problems.len(), problems.join("\n"));
    let guide = std::fs::read_to_string(root.join("canon/skills/SKILL.src.md")).unwrap();
    assert!(guide.contains("An option that takes an id is the bare noun"), "the guide states the rule once");
    assert!(!guide.contains("`--<noun>-id`"), "the old rule is gone");
}

/// The design doc spells 0.27 on purpose in its migration tables; only those lines are history.
fn is_history(file: &Path, line: &str) -> bool {
    file.ends_with("cli-commands-0.28.md") && line.starts_with('|') && line.split('|').nth(1).is_some_and(|first| !old_spellings(first).is_empty()) && line.split('|').nth(2).is_some_and(|second| old_spellings(second).is_empty())
}

fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "node_modules" || n == "dist" || n == ".astro") {
                continue;
            }
            walk(&path, ext, out);
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
}

/// context.capability.59: the corpus of this repo and of the platformer example name no old option, and `canon format` passes.
#[test]
fn context_capability_59_the_spec_corpus_names_no_id_option_spelling_and_format_passes() {
    let root = harness::repo_root();
    for corpus in [".canon/specs", "examples/platformer/specs"] {
        let mut files = Vec::new();
        walk(&root.join(corpus), "feature", &mut files);
        assert!(!files.is_empty(), "{corpus} holds features");
        for file in &files {
            let old = old_spellings(&std::fs::read_to_string(file).unwrap());
            assert!(old.is_empty(), "{} spells {old:?}", file.display());
        }
        let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["format", &root.join(corpus).to_string_lossy()]).env_remove("CANON_ROLE").current_dir(&root).output().expect("spawn canon");
        assert!(out.status.success(), "canon format {corpus}: {}{}", stdout(&out), stderr(&out));
    }
}

// ── context.capability.60 ────────────────────────────────────────────────

const BASE: &str = r#"Feature: cart add
  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @cart.add.01
  Scenario: Adding an item puts it in the cart
    When `canon evidence add --scenario-id` records the evidence
    Then the cart holds that item
"#;

const FEATURE_PATH: &str = ".canon/specs/kind=feature/area=cart/add.feature";

fn write_feature(repo: &Path, text: &str) {
    let path = repo.join(FEATURE_PATH);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    ok(repo, &["inventory", "sync"]);
}

fn gate(repo: &Path, exit: i32) -> String {
    let out = canon(repo, &["gate", "check"]);
    assert_eq!(out.status.code(), Some(exit), "{}{}", stdout(&out), stderr(&out));
    format!("{}{}", stdout(&out), stderr(&out))
}

/// context.capability.60: a rewritten step text makes the scenario's review stale until it is reviewed again.
#[test]
fn context_capability_60_a_rewritten_scenario_without_a_new_review_is_reported_by_gate_check_as_stale() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "x"]);
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_review:\n    scope: []\n").unwrap();
    write_feature(repo, BASE);
    let evidence = ["evidence", "add", "--scenario", "cart.add.01", "--project", "root", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor", "impl-agent"];
    ok(repo, &evidence);
    ok(repo, &["gate", "promote"]);
    let review = ["review", "scenario", "cart.add.01", "--project", "root", "--original-spec-ref", FEATURE_PATH, "--actor", "reviewer-2", "--role", "reviewer", "--session", "sess-review"];
    ok(repo, &review);
    gate(repo, 0);

    // The step text is rewritten from the old option to the bare one.
    write_feature(repo, &BASE.replace("`canon evidence add --scenario-id` records the evidence", "`canon evidence add --scenario` records the evidence"));
    let stale = gate(repo, 1);
    assert!(stale.contains("unreviewed-promotion cart.add.01 — stale review by `reviewer-2`: its spec changed"), "{stale}");

    // New evidence and a review from another session make it clean.
    ok(repo, &["evidence", "add", "--scenario", "cart.add.01", "--project", "root", "--class", "test-run", "--ref", "npm test again", "--role", "implementer", "--actor", "impl-agent"]);
    ok(repo, &["gate", "promote"]);
    ok(repo, &["review", "scenario", "cart.add.01", "--project", "root", "--original-spec-ref", FEATURE_PATH, "--actor", "reviewer-3", "--role", "reviewer", "--session", "sess-review-2"]);
    gate(repo, 0);
}
