//! `--repo` declared once (D5), the removed spellings it replaces, and the
//! D3 coverage rule: a check that examined nothing is `unverifiable`, exit 1.
//!
//! Drives the built binary only.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(args)
        .current_dir(dir)
        .env("CANON_ACTOR", "context-flags")
        .env("HOME", dir)
        .env("CANON_SKILLS_SOURCE", concat!(env!("CARGO_MANIFEST_DIR"), "/../../canon/skills"))
        .output()
        .expect("spawn canon binary")
}

fn ok(dir: &Path, args: &[&str]) {
    let out = canon(dir, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

fn strings(value: &Value) -> Vec<&str> {
    value.as_array().map(|items| items.iter().filter_map(Value::as_str).collect()).unwrap_or_default()
}

/// A repo `canon init` set up, with nothing authored in it.
fn fresh_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["init", "--providers", "none"]);
    dir
}

/// The lines of the repo's access audit, none when it does not exist.
fn audit_lines(repo: &Path) -> usize {
    std::fs::read_to_string(repo.join(".canon/audit/access.jsonl")).map(|text| text.lines().count()).unwrap_or(0)
}

#[test]
fn every_removed_spelling_is_a_usage_error_that_names_the_replacement() {
    let dir = TempDir::new().unwrap();
    let cases: &[(&[&str], &str)] = &[
        (&["skills", "check", "--target", "."], "--repo"),
        (&["skills", "install", "--target", "."], "--repo"),
        (&["skills", "doctor", "--target", "."], "--repo"),
        (&["ingest", "sessions", "--canon-yaml", "canon.yaml"], "--repo"),
        (&["query", "--kind", "change", "--canon-yaml", "canon.yaml"], "--repo"),
        (&["tier", "age", "--canon-yaml", "canon.yaml"], "--repo"),
        (&["regime-key", "--role", "reviewer", "--repo", "r", "--area", "a", "--hash", "abcdef012345"], "--repo-name"),
        (&["format", "--check", "spec"], "canon format"),
        (&["fmt", "spec"], "format"),
    ];
    for (args, replacement) in cases {
        let mut argv = args.to_vec();
        argv.push("--json");
        let out = canon(dir.path(), &argv);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
        let error = body(&out)["error"].clone();
        assert_eq!(error["code"], "USAGE", "{args:?}: {error}");
        assert!(strings(&error["expected"]).contains(replacement), "{args:?}: {error}");
        assert!(error["hint"].as_str().unwrap().contains(replacement), "{args:?}: {error}");
    }
}

#[test]
fn commands_from_a_subdirectory_find_the_repo_and_a_given_repo_is_used_as_is() {
    let repo = fresh_repo();
    ok(repo.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    let nested = repo.path().join("packages/app");
    std::fs::create_dir_all(&nested).unwrap();
    let elsewhere = TempDir::new().unwrap();

    for args in [&["status"][..], &["policy", "show"][..], &["query", "--kind", "subject"][..], &["gate", "check"][..], &["subject", "status", "demo"][..]] {
        let from_nested = canon(&nested, args);
        assert_ne!(from_nested.status.code(), Some(2), "{args:?} from a nested directory: {}", String::from_utf8_lossy(&from_nested.stderr));

        let mut named = args.to_vec();
        let root = repo.path().to_str().unwrap();
        named.extend(["--repo", root]);
        let from_elsewhere = canon(elsewhere.path(), &named);
        assert_ne!(from_elsewhere.status.code(), Some(2), "{named:?} from an unrelated directory: {}", String::from_utf8_lossy(&from_elsewhere.stderr));
        assert_eq!(from_nested.status.code(), from_elsewhere.status.code(), "{args:?}: discovery and --repo name the same repo");
    }

    // `--repo` names the directory itself: no walk upward from it.
    let given = canon(&nested, &["status", "--repo", nested.to_str().unwrap(), "--json"]);
    assert_eq!(given.status.code(), Some(2));
    assert_eq!(body(&given)["error"]["code"], "CONFIG_MISSING");
}

#[test]
fn adapter_validate_needs_no_canon_yaml_and_a_missing_response_is_a_refusal() {
    let empty = TempDir::new().unwrap();
    let out = canon(empty.path(), &["adapter", "validate", "--response", "no-such-response.json", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stdout));
    assert_ne!(body(&out)["error"]["code"], "CONFIG_MISSING", "adapter validate needs no canon state");
}

#[test]
fn gate_check_and_format_on_an_empty_corpus_are_unverifiable_never_clean() {
    let repo = fresh_repo();

    let gate = canon(repo.path(), &["gate", "check", "--json"]);
    assert_eq!(gate.status.code(), Some(1), "{}", String::from_utf8_lossy(&gate.stdout));
    let gate = body(&gate);
    assert_eq!(gate["data"]["unverifiable"], true, "{gate}");
    assert_eq!(gate["data"]["clean"], false, "{gate}");
    assert!(gate["meta"]["coverage"].is_object(), "{gate}");

    let format = canon(repo.path(), &["format", "--json"]);
    assert_eq!(format.status.code(), Some(1), "{}", String::from_utf8_lossy(&format.stdout));
    let format = body(&format);
    assert_eq!(format["data"]["unverifiable"], true, "{format}");
    assert_eq!(format["data"]["filesChecked"], 0, "{format}");
    assert!(format["meta"]["coverage"].is_object(), "{format}");

    // A directory with no file in it is the same.
    std::fs::create_dir(repo.path().join("empty-corpus")).unwrap();
    let empty_dir = canon(repo.path(), &["format", "empty-corpus", "--json"]);
    assert_eq!(empty_dir.status.code(), Some(1));
    assert_eq!(body(&empty_dir)["data"]["unverifiable"], true);

    // Not a directory: refused, not graded.
    let missing = canon(repo.path(), &["format", "no-such-dir", "--json"]);
    assert_eq!(missing.status.code(), Some(2));
}

#[test]
fn format_examines_every_configured_spec_root_and_reports_what_it_looked_at() {
    let repo = fresh_repo();
    ok(repo.path(), &["feature", "new", "cart.add", "--title", "Add to cart"]);
    ok(repo.path(), &["scenario", "new", "cart.add.01", "--title", "Adds an item", "--case", "happy"]);

    let out = canon(repo.path(), &["format", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let report = body(&out);
    assert_eq!(report["data"]["clean"], true, "{report}");
    assert!(report["data"]["filesChecked"].as_u64().unwrap() >= 1, "{report}");
    let roots = report["meta"]["coverage"]["specRoots"].as_array().expect("the examined roots are named");
    assert!(roots.iter().any(|root| root["examined"].as_u64().unwrap_or(0) >= 1), "{report}");

    // The same corpus by operand: relative to the working directory, and relative to `--repo`.
    let by_dir = canon(repo.path(), &["format", ".canon/specs"]);
    assert_eq!(by_dir.status.code(), Some(0));
    let elsewhere = TempDir::new().unwrap();
    let by_repo = canon(elsewhere.path(), &["format", ".canon/specs", "--repo", repo.path().to_str().unwrap()]);
    assert_eq!(by_repo.status.code(), Some(0), "{}", String::from_utf8_lossy(&by_repo.stderr));
}

#[test]
fn a_refused_read_or_export_writes_no_audit_line_and_a_served_one_does() {
    let repo = fresh_repo();
    assert_eq!(audit_lines(repo.path()), 0);

    // Refused before any read: a scope the kind does not take, a cursor that is not one.
    let refused = canon(repo.path(), &["query", "--kind", "evidence", "--domain", "dev", "--json"]);
    assert_eq!(refused.status.code(), Some(2));
    let bad_cursor = canon(repo.path(), &["export", "--kind", "change", "--cursor", "garbage", "--json"]);
    assert_eq!(bad_cursor.status.code(), Some(2), "{}", String::from_utf8_lossy(&bad_cursor.stdout));
    let stale_plan = canon(repo.path(), &["purge", "--kind", "session", "--before", "2020-01-01", "--confirm", "0000", "--json"]);
    assert_eq!(stale_plan.status.code(), Some(2), "{}", String::from_utf8_lossy(&stale_plan.stdout));
    assert_eq!(audit_lines(repo.path()), 0, "a refusal reads and deletes nothing, so it leaves no access line");

    // A query that reached the store writes one, even when it matched nothing.
    let served = canon(repo.path(), &["query", "--kind", "subject", "--json"]);
    assert_eq!(served.status.code(), Some(0), "{}", String::from_utf8_lossy(&served.stderr));
    assert_eq!(audit_lines(repo.path()), 1);

    let refused_again = canon(repo.path(), &["query", "--kind", "evidence", "--domain", "dev"]);
    assert_eq!(refused_again.status.code(), Some(2));
    assert_eq!(audit_lines(repo.path()), 1, "the refused run adds nothing");
}
