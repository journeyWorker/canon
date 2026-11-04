//! `canon-0-36-codes-refusals`, scenarios `atip.codesplit.01`-`.05`: the codes that said several refusals with one name say each with the code
//! of its condition (atip/1 spec 7.2.2), and a reason names what differs inside one condition. Against the built `canon` binary; the codes
//! of the other scenarios the split changed are asserted by the tests of those scenarios.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const REGIME: &str = "dev/canon/join-spine/9c93d024b1a2";

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).arg("--json").current_dir(dir).env_remove("CANON_ROLE").env_remove("CANON_ACTOR").output().expect("spawn canon")
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

fn ok(dir: &Path, args: &[&str]) -> Value {
    let out = run(dir, args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    body(&out)
}

/// Runs `args`, expecting exit 2, `error.code == code`, `error.reason == reason` and no `data`; returns the document.
fn refused(dir: &Path, args: &[&str], code: &str, reason: Option<&str>) -> Value {
    let out = run(dir, args);
    assert_eq!(out.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    let doc = body(&out);
    assert_eq!(doc["ok"], false, "{doc}");
    assert_eq!(doc["error"]["code"], code, "{args:?}: {doc}");
    assert_eq!(doc["error"].get("reason").and_then(Value::as_str), reason, "{args:?}: {doc}");
    assert!(doc.get("data").is_none(), "a refusal carries no data: {doc}");
    doc
}

fn tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// A git repo with one commit, set up by `canon init`.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(dir.path()).output().unwrap();
        assert!(out.status.success(), "git {args:?}");
    }
    ok(dir.path(), &["init", "--providers", "none"]);
    dir
}

/// atip.codesplit.01: no task or scenario is `EVIDENCE_REQUIRED`, an argument that forbids another is `EVIDENCE_ARGUMENT_CONFLICT`, an empty
/// value is `EVIDENCE_ARGUMENT_INVALID`; each exits 2 with no data and writes nothing.
#[test]
fn atip_codesplit_01_a_missing_subject_a_contradiction_and_an_empty_value_answer_with_their_own_condition() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let before = tree(d);
    refused(d, &["evidence", "add", "--class", "test-run", "--ref", "r", "--role", "implementer"], "EVIDENCE_REQUIRED", None);
    refused(d, &["evidence", "add", "--all-scenarios", "--scenario", "auth.login.01", "--report", "junit:r.xml", "--class", "test-run", "--ref", "r", "--role", "implementer"], "EVIDENCE_ARGUMENT_CONFLICT", None);
    refused(d, &["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", " ", "--role", "implementer"], "EVIDENCE_ARGUMENT_INVALID", None);
    refused(d, &["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "r", "--role", "implementer", "--command-result", "out.txt"], "EVIDENCE_ARGUMENT_REQUIRED", None);
    assert_eq!(tree(d), before, "a refusal writes nothing");
}

/// atip.codesplit.02: a file to bind that is outside the repository is `EVIDENCE_BINDING_DENIED`, a missing one `..._NOT_FOUND`, a directory
/// `..._INVALID`; the same for `review scenario` (`REVIEW_ARTIFACT_*`); a file that changed after it was bound is `..._STALE` (the unit tests of
/// `artifacts.rs`). Each refusal exits 2 and writes nothing.
#[test]
fn atip_codesplit_02_a_file_to_bind_is_refused_by_its_condition_and_nothing_is_written() {
    let dir = repo();
    let d = dir.path();
    let outside = TempDir::new().unwrap();
    std::fs::write(outside.path().join("trace.zip"), b"elsewhere").unwrap();
    std::fs::create_dir_all(d.join("reports/dir")).unwrap();
    std::fs::write(d.join("reports/ok.txt"), b"fine").unwrap();
    ok(d, &["scenario", "new", "auth.login.01", "--title", "A user signs in", "--case", "happy"]);
    ok(d, &["inventory", "sync"]);
    let before = tree(d);
    let base = ["evidence", "add", "--scenario", "auth.login.01", "--project", "root", "--class", "test-run", "--ref", "r", "--role", "implementer", "--artifact"];
    let with = |artifact: &str| -> Vec<String> { base.iter().map(ToString::to_string).chain([artifact.to_string()]).collect() };
    for (artifact, code) in [
        (outside.path().join("trace.zip").to_string_lossy().into_owned(), "EVIDENCE_BINDING_DENIED"),
        ("reports/none.png".to_string(), "EVIDENCE_BINDING_NOT_FOUND"),
        ("reports/dir".to_string(), "EVIDENCE_BINDING_INVALID"),
    ] {
        let args = with(&artifact);
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        refused(d, &words, code, None);
    }
    for (artifact, code) in [
        (outside.path().join("trace.zip").to_string_lossy().into_owned(), "REVIEW_ARTIFACT_DENIED"),
        ("reports/none.png".to_string(), "REVIEW_ARTIFACT_NOT_FOUND"),
        ("reports/dir".to_string(), "REVIEW_ARTIFACT_INVALID"),
    ] {
        refused(d, &["review", "scenario", "auth.login.01", "--project", "root", "--actor", "r", "--role", "reviewer", "--original-spec-ref", "HEAD", "--artifact", &artifact], code, None);
    }
    assert_eq!(tree(d), before, "a refused binding stores no bytes and writes no record");
}

/// atip.codesplit.03: a run that was already ended is `DISPATCH_CONFLICT` with the reason `ALREADY_ENDED`, the manifest keeps its bytes.
#[test]
fn atip_codesplit_03_ending_a_run_twice_is_a_conflict_with_the_reason_already_ended() {
    let dir = repo();
    let d = dir.path();
    let begun = ok(d, &["dispatch", "begin", "--role", "dev", "--regime", REGIME]);
    let run_id = begun["data"]["runId"].as_str().unwrap().to_string();
    ok(d, &["dispatch", "end", &run_id, "--run-outcome", "succeeded"]);
    let manifest = d.join(".canon/dispatch").join(format!("{run_id}.json"));
    let before = std::fs::read(&manifest).unwrap();
    refused(d, &["dispatch", "end", &run_id, "--run-outcome", "failed"], "DISPATCH_CONFLICT", Some("ALREADY_ENDED"));
    assert_eq!(std::fs::read(&manifest).unwrap(), before);
}

/// atip.codesplit.04: a malformed `skills:` or `specs:` section is `CONFIG_INVALID` with the reason `SKILLS` or `SPECS`, an unknown provider is
/// `SKILLS_ARGUMENT_INVALID`, a missing `--skills-source` is `SKILLS_SOURCE_NOT_FOUND`; each exits 2 and writes nothing.
#[test]
fn atip_codesplit_04_a_broken_section_and_a_bad_provider_answer_with_their_condition_and_reason() {
    let dir = repo();
    let d = dir.path();
    let yaml = std::fs::read_to_string(d.join("canon.yaml")).unwrap();
    refused(d, &["skills", "install", "--providers", "bogus"], "SKILLS_ARGUMENT_INVALID", None);
    refused(d, &["skills", "install", "--skills-source", "no-such-dir"], "SKILLS_SOURCE_NOT_FOUND", None);
    std::fs::write(d.join("canon.yaml"), format!("{yaml}\nskills: [claude]\n")).unwrap();
    let before = tree(d);
    refused(d, &["skills", "install"], "CONFIG_INVALID", Some("SKILLS"));
    assert_eq!(tree(d), before);
    std::fs::write(d.join("canon.yaml"), format!("{yaml}\nspecs:\n  bogus: 1\n")).unwrap();
    let before = tree(d);
    refused(d, &["inventory", "sync"], "CONFIG_INVALID", Some("SPECS"));
    assert_eq!(tree(d), before);
}

/// atip.codesplit.05: no live change with a brief is `BRIEF_REQUIRED`, a named change without one `BRIEF_NOT_FOUND`; after `canon change new`
/// the same call answers.
#[test]
fn atip_codesplit_05_no_brief_and_a_named_change_without_one_answer_with_their_own_code_and_the_fix_lets_the_call_through() {
    let dir = repo();
    let d = dir.path();
    refused(d, &["brief", "show"], "BRIEF_REQUIRED", None);
    refused(d, &["brief", "show", "no-such-change"], "BRIEF_NOT_FOUND", None);
    let created = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["subject", "new", "demo", "--domain", "dev", "--title", "Demo", "--json"])
        .current_dir(d)
        .env("CANON_ROLE", "planner")
        .env_remove("CANON_ACTOR")
        .output()
        .unwrap();
    assert_eq!(created.status.code(), Some(0), "{}", String::from_utf8_lossy(&created.stdout));
    let change = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["change", "new", "add-login", "--subject", "demo", "--title", "Add login", "--json"])
        .current_dir(d)
        .env("CANON_ROLE", "planner")
        .env_remove("CANON_ACTOR")
        .output()
        .unwrap();
    assert_eq!(change.status.code(), Some(0), "{}", String::from_utf8_lossy(&change.stdout));
    ok(d, &["brief", "show"]);
}
