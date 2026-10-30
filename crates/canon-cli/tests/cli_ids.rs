//! 0.31 (`canon-0-31-cli-ids`): an option that takes an id is the bare noun, through the real binary. Every test is
//! named by the scenario it proves (`context.capability.47` to `.60`).
//!
//! The 0.30 → 0.31 parity of what the renamed options write is `tests/fixtures/cli-ids-030`: a line of each of the 24
//! commands that take an id, the output and stored records canon 0.30 gave for its spelling, and
//! [`context_capability_47_every_id_option_is_the_bare_noun_and_writes_the_record_the_old_spelling_wrote`] replaying it
//! with the bare spelling. The conformance corpus (`conformance/cases`) was re-blessed the same way.

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

/// The 0.30 → 0.31 parity fixture: the lines in the 0.30 spelling and what 0.30 printed and stored for them.
fn parity_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli-ids-030")
}

/// `word` in the 0.31 spelling: a `--<noun>-id` of the 14 nouns, alone or with `=value`, is `--<noun>`.
fn bare_spelling(word: &str) -> String {
    for noun in NOUNS {
        let old = format!("--{noun}-id");
        if let Some(rest) = word.strip_prefix(&old).filter(|rest| rest.is_empty() || rest.starts_with('=')) {
            return format!("--{noun}{rest}");
        }
    }
    word.to_string()
}

/// `text` with every `--<noun>-id` of the 14 nouns that stands as a whole option name written `--<noun>`.
fn bare_spellings_in(text: &str) -> String {
    let mut out = text.to_string();
    for noun in NOUNS {
        let (old, new) = (format!("--{noun}-id"), format!("--{noun}"));
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(at) = rest.find(&old) {
            let after = &rest[at + old.len()..];
            let whole = !after.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-');
            result.push_str(&rest[..at]);
            result.push_str(if whole { &new } else { &old });
            rest = after;
        }
        result.push_str(rest);
        out = result;
    }
    out
}

/// What a run of the parity lines printed and stored, normalized: the repo path, the version, times, ULIDs and digests
/// masked, and, for the 0.30 run, the old spellings in the output written the bare way (the one change it has).
fn parity_transcript(binary: &Path, old_spelling: bool) -> String {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let home = tempfile::tempdir().unwrap();
    let run = |args: &[String]| -> Output {
        Command::new(binary).args(args).env_clear().env("PATH", std::env::var_os("PATH").unwrap_or_default()).env("HOME", home.path()).env("LC_ALL", "C").env("TZ", "UTC").env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
    };
    let version = String::from_utf8(run(&["--version".to_string()]).stdout).unwrap().trim().trim_start_matches("canon ").to_string();
    let real = repo.canonicalize().unwrap();
    let normalize = |text: &str| -> String {
        let text = text.replace(&*real.to_string_lossy(), "<repo>").replace(&*repo.to_string_lossy(), "<repo>").replace(&version, "<version>");
        let text = if old_spelling { bare_spellings_in(&text) } else { text };
        // The blockers of the brief template `change new` writes are the brief-format change's (0.31 dropped a section
        // comment), and the fixture removes the brief right after, so the count is masked.
        let text = text.replace("7 blocker(s): write the brief", "<n> blocker(s): write the brief").replace("6 blocker(s): write the brief", "<n> blocker(s): write the brief");
        masked(&text)
    };
    git(repo, &["init", "-q", "-b", "main"]);
    let init = run(&["init", "--repo", ".", "--no-policy", "--providers", "none"].map(String::from));
    assert!(init.status.success(), "{}{}", stdout(&init), stderr(&init));
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "init"]);
    let mut out = String::new();
    let lines = std::fs::read_to_string(parity_dir().join("lines.txt")).unwrap();
    for line in lines.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let words = harness::words(line);
        if words[0] == "git" {
            let args: Vec<&str> = words[1..].iter().map(String::as_str).collect();
            git(repo, &args);
            continue;
        }
        if words[0] == "rm" {
            std::fs::remove_file(repo.join(&words[1])).unwrap();
            continue;
        }
        let args: Vec<String> = if old_spelling { words } else { words.iter().map(|w| bare_spelling(w)).collect() };
        let result = run(&args);
        out.push_str(&format!("$ canon {}\nexit {}\n", harness::words(line).iter().map(|w| bare_spelling(w)).collect::<Vec<_>>().join(" "), result.status.code().unwrap_or(-1)));
        out.push_str(&format!("--- stdout\n{}\n--- stderr\n{}\n", normalize(&stdout(&result)), normalize(&stderr(&result))));
    }
    // The stored records: every text file canon keeps under `.canon/`, by path. The briefs are the brief-format
    // change's (0.31 writes the template without comments), not this one's; the access log is no record.
    out.push_str("=== stored\n");
    let mut stored: Vec<(String, String)> = harness::tree(&repo.join(".canon"))
        .into_iter()
        .filter_map(|(path, bytes)| Some((path, String::from_utf8(bytes?).ok()?)))
        .filter(|(path, _)| !path.ends_with("brief.md") && !path.starts_with("audit"))
        .map(|(path, text)| (normalize(&path.to_string_lossy()), normalize(&text)))
        .collect();
    stored.sort();
    for (path, text) in stored {
        out.push_str(&format!("### {path}\n{text}\n"));
    }
    out
}

// ── F1 ───────────────────────────────────────────────────────────────────

/// context.capability.47: the 14 options are the bare nouns on the 70 places of 27 commands (0.32: `--actor` names the agent that relays an approval
/// on `brief approve`, `policy approve` and `migrate`), and a line of each of the
/// 24 commands exits, prints and stores what canon 0.30 did for the same line in its spelling.
#[test]
fn context_capability_47_every_id_option_is_the_bare_noun_and_writes_the_record_the_old_spelling_wrote() {
    let by_name = options_by_name();
    let mut places = 0;
    let mut commands = std::collections::BTreeSet::new();
    for (noun, count) in NOUNS.iter().zip([19, 15, 11, 6, 4, 3, 3, 3, 1, 1, 1, 1, 1, 1]) {
        let leaves = by_name.get(*noun).unwrap_or_else(|| panic!("no command takes --{noun}"));
        assert_eq!(leaves.len(), count, "--{noun} is on {leaves:?}");
        assert!(!by_name.contains_key(&format!("{noun}-id")), "--{noun}-id is still an option of {:?}", by_name.get(&format!("{noun}-id")));
        places += leaves.len();
        commands.extend(leaves.iter().cloned());
    }
    assert!(by_name.keys().all(|name| !name.ends_with("-id")), "{:?}", by_name.keys().filter(|n| n.ends_with("-id")).collect::<Vec<_>>());
    assert_eq!((places, commands.len()), (70, 27));

    // The parity replay. With CANON_030_BIN naming a canon 0.30 binary the fixture is blessed from it instead.
    let expected_path = parity_dir().join("expected.txt");
    if let Some(old) = std::env::var_os("CANON_030_BIN") {
        std::fs::write(&expected_path, parity_transcript(Path::new(&old), true)).unwrap();
    }
    let expected = std::fs::read_to_string(&expected_path).expect("the 0.30 transcript is checked in");
    let actual = parity_transcript(Path::new(env!("CARGO_BIN_EXE_canon")), false);
    if actual != expected {
        // Each step (`$ canon …`) and each stored file (`### …`) is one block; name every block that differs.
        let blocks = |text: &str| -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            for line in text.lines() {
                if line.starts_with("$ canon ") || line.starts_with("### ") || out.is_empty() {
                    out.push(String::new());
                }
                let last = out.last_mut().unwrap();
                last.push_str(line);
                last.push('\n');
            }
            out
        };
        let (want, got) = (blocks(&expected), blocks(&actual));
        let differing: Vec<String> = (0..want.len().max(got.len()))
            .filter(|i| want.get(*i) != got.get(*i))
            .map(|i| format!("--- 0.30\n{}--- 0.31\n{}", want.get(i).map_or("(none)\n", String::as_str), got.get(i).map_or("(none)\n", String::as_str)))
            .collect();
        panic!("the bare spelling does not do what 0.30 did in {} block(s):\n{}", differing.len(), differing.join("\n"));
    }
    for kind in ["evidence", "finding", "review", "scenario", "subject"] {
        assert!(expected.contains(&format!("kind={kind}/")), "the transcript stores a {kind} record");
    }
    assert!(expected.contains("\"scenario_id\"") && expected.contains("\"change_id\"") && expected.contains("\"session_id\""), "the stored records keep their id-suffixed keys");
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
    let task = canon(repo, &["dispatch", "begin", "--role", "implementer", "--task", "add-login#1", "--regime", "implementer/canon/search/abcdef", "--json"]);
    let said = stdout(&task);
    assert!(!said.contains("was removed"), "{said}");
    assert_eq!(task.status.code(), Some(2), "{said}{}", stderr(&task));
    assert_eq!(json(&task)["error"]["code"], "TASK_NOT_FOUND", "the line ran and the command checked the task: {said}");
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

    // A value that moved into an operand in 0.28 on the same line, before and after a renamed option: the error names
    // the first spelling typed, and the hint applies both, so it runs as printed.
    for (typed, first) in [
        (&["subject", "adopt", "--change-id", "add-login", "--subject-id", "demo", "--json"][..], "--change-id"),
        (&["subject", "unadopt", "--subject-id", "demo", "--change-id", "add-login", "--json"], "--subject-id"),
    ] {
        let before = harness::tree(repo);
        let body = usage_error(&canon(repo, typed));
        assert_eq!(body["error"]["field"], first, "{typed:?}: {body}");
        assert_eq!(body["error"]["message"], format!("{first} was removed in {}", if first == "--change-id" { "0.31.0" } else { "0.28.0" }), "{body}");
        let line = hint_line(&body);
        assert_eq!(line, [typed[0], typed[1], "demo", "--change", "add-login", "--json"], "{typed:?}: {body}");
        unchanged(repo, &before, "the refused line");
        let refs: Vec<&str> = line.iter().map(String::as_str).collect();
        ok(repo, &refs);
    }
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

/// The `canon …` command lines in one output: every backtick span that starts with `canon `, and in a JSON document the
/// `command` of every step and every `argv` array.
fn command_lines(text: &str) -> Vec<Vec<String>> {
    fn walk(value: &Value, strings: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
        match value {
            Value::String(s) => strings.push(s.clone()),
            Value::Array(items) => {
                for item in items {
                    walk(item, strings, out);
                }
            }
            Value::Object(map) => {
                for (key, v) in map {
                    match (key.as_str(), v) {
                        ("command", Value::String(s)) if s.starts_with("canon ") => out.push(harness::words(s)),
                        ("argv", Value::Array(words)) if !words.is_empty() => out.push(words.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()),
                        _ => walk(v, strings, out),
                    }
                }
            }
            _ => {}
        }
    }
    let mut strings = vec![text.to_string()];
    let mut out = Vec::new();
    if let Ok(doc) = serde_json::from_str::<Value>(text) {
        walk(&doc, &mut strings, &mut out);
    }
    for s in &strings {
        out.extend(s.split('`').skip(1).step_by(2).filter(|span| span.starts_with("canon ")).map(harness::words));
    }
    out
}

/// The example command lines of a `--help` text, read as a shell would: the lines of the `Examples:` block (it ends
/// at a blank line) that start with `canon `, a `\` continuation joined, a redirect, a pipe or a two-space comment
/// column cut off, and a `$(...)` substitution one word.
fn help_examples(text: &str) -> Vec<Vec<String>> {
    let Some((_, examples)) = text.split_once("Examples:") else { return Vec::new() };
    let mut joined: Vec<String> = Vec::new();
    let mut continued = false;
    for line in examples.lines().map(str::trim) {
        if line.is_empty() && !joined.is_empty() {
            break;
        }
        let (body, continues) = line.strip_suffix('\\').map_or((line, false), |body| (body.trim_end(), true));
        if continued {
            if let Some(last) = joined.last_mut() {
                last.push(' ');
                last.push_str(body);
            }
        } else if body.starts_with("canon ") {
            joined.push(body.to_string());
        } else {
            continue;
        }
        continued = continues;
    }
    joined
        .iter()
        .map(|line| {
            let line = [" > ", " | ", "  "].iter().fold(line.as_str(), |line, cut| line.split(cut).next().unwrap_or_default());
            let mut out = String::new();
            let mut rest = line;
            while let Some(at) = rest.find("$(") {
                out.push_str(&rest[..at]);
                out.push('x');
                rest = rest[at..].split_once(')').map_or("", |(_, after)| after);
            }
            out.push_str(rest);
            harness::words(&out)
        })
        .collect()
}

/// Parses `words` (`canon` first) against the clap tree: in full when it holds no placeholder; a line that only names
/// a command (`canon feature new`), or one with a `<placeholder>`, `[optional]` part or ellipsis, as a syntax line:
/// the command path resolves to a leaf and every `--option` is one of it.
fn parse_line(root: &clap::Command, words: &[String]) -> Result<(), String> {
    let mut built = root.clone();
    built.build();
    let mut cmd = &built;
    let mut path_len = 1;
    for word in &words[1..] {
        match cmd.find_subcommand(word) {
            Some(sub) => cmd = sub,
            None => break,
        }
        path_len += 1;
    }
    let placeholder = words.iter().any(|w| w.contains('<') || w.contains('[') || w.contains('…') || w.contains("..."));
    if !placeholder && path_len < words.len() {
        return match root.clone().try_get_matches_from(words) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == clap::error::ErrorKind::DisplayHelp => Ok(()),
            Err(e) => Err(e.to_string().lines().next().unwrap_or_default().to_string()),
        };
    }
    if cmd.has_subcommands() {
        return Err(format!("`{}` names no command", words.join(" ")));
    }
    for word in words.iter().filter(|w| w.starts_with("--")) {
        let name = word[2..].split('=').next().unwrap_or_default();
        if !cmd.get_arguments().any(|a| a.get_long() == Some(name)) {
            return Err(format!("`{}` has no option --{name}", cmd.get_name()));
        }
    }
    Ok(())
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
    // The steps, violations, remedies and refusals of a repo with a change and a scenario that lacks evidence, of one
    // under the starter policy (the gate reports that scenario), and of one with two spec roots.
    let dir = repo_with_change();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "demo.login.01", "--title", "One", "--subject", "demo", "--change", "add-login", "--flow", "F1", "--case", "happy"]);
    let gated = tempfile::tempdir().unwrap();
    git(gated.path(), &["init", "-q"]);
    ok(gated.path(), &["init", "--repo", ".", "--providers", "none"]);
    ok(gated.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(gated.path(), &["scenario", "new", "demo.login.01", "--title", "One", "--subject", "demo", "--case", "happy"]);
    let two = repo_with_roots(&["game", "ui"]);
    let generated: Vec<(&str, Vec<&str>)> = vec![
        ("repo", vec!["status", "--json"]),
        ("repo", vec!["status"]),
        ("repo", vec!["brief", "check", "add-login", "--json"]),
        ("repo", vec!["evidence", "add", "--scenario", "demo.login.01", "--class", "test-run", "--ref", "r", "--role", "implementer", "--json"]),
        ("repo", vec!["subject", "move", "demo", "--to", "shipped", "--json"]),
        ("repo", vec!["finding", "add", "--change-id", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor-id", "r", "--summary", "s", "--json"]),
        ("policy", vec!["gate", "check", "--json"]),
        ("policy", vec!["gate", "check"]),
        ("policy", vec!["status", "--json"]),
        ("two roots", vec!["status", "--json"]),
        ("two roots", vec!["gate", "check", "--json"]),
        ("two roots", vec!["feature", "new", "a.b", "--title", "T", "--json"]),
    ];
    for (which, args) in &generated {
        let at = match *which {
            "repo" => repo,
            "policy" => gated.path(),
            _ => two.path(),
        };
        let out = canon(at, args);
        texts.push((format!("{which}: {}", args.join(" ")), format!("{}{}", stdout(&out), stderr(&out))));
    }
    let mut parsed: BTreeMap<String, usize> = BTreeMap::new();
    let mut problems = Vec::new();
    for (what, text) in &texts {
        // A refusal of a typed old spelling names it; the command lines it prints spell none.
        if old_spellings(what).is_empty() {
            assert!(old_spellings(text).is_empty(), "{what} spells {:?}", old_spellings(text));
        }
        let lines: Vec<Vec<String>> = if what.ends_with("--help") {
            help_examples(text)
        } else {
            command_lines(text)
        };
        for words in lines {
            assert!(old_spellings(&words.join(" ")).is_empty(), "{what}: {words:?}");
            match parse_line(&root, &words) {
                Ok(()) => *parsed.entry(what.clone()).or_default() += 1,
                Err(e) => problems.push(format!("{what}: {e}\n    {}", words.join(" "))),
            }
        }
    }
    assert!(problems.is_empty(), "{} generated line(s) do not parse:\n{}", problems.len(), problems.join("\n"));
    // Each kind of output gave lines to parse: the next steps, the violations' remedies, the refusals' hints.
    for what in ["repo: status --json", "policy: gate check --json", "policy: gate check", "repo: evidence add", "repo: finding add", "two roots: feature new"] {
        assert!(parsed.iter().any(|(k, n)| k.starts_with(what) && *n > 0), "no command line parsed from `{what}`: {parsed:?}");
    }
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
