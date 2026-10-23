//! T9: outputs no other test of the suite reached with `--json`, each run once with its
//! document held to the spec's schemas: the reads that need state (an adapter response, a
//! context pack, a prompt bundle), the human-only acceptance of a divergence, and the
//! failure envelopes of leaves whose refusals no other test provokes.

mod harness;

use std::path::{Path, PathBuf};

use harness::*;
use serde_json::Value;
use tempfile::TempDir;

fn run(repo: &Path, home: &Path, args: &[&str]) -> (i32, Value) {
    let mut argv = strs(args);
    argv.push("--json".into());
    let out = canon_in(repo, home, &argv, &[]);
    assert!(out.stderr.is_empty(), "stderr in JSON mode: {}", String::from_utf8_lossy(&out.stderr));
    let document = document(&out);
    let problems = validate_document(&document);
    assert!(problems.is_empty(), "canon {args:?}: {problems:?}\n{document}");
    (out.status.code().unwrap(), document)
}

fn repo_copy() -> (TempDir, TempDir, PathBuf) {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    let (scratch, repo) = copy_of(base.path());
    (home, scratch, repo)
}

#[test]
fn an_adapter_response_is_validated_and_refused_authorization_with_its_report() {
    let (home, _scratch, repo) = repo_copy();
    std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/adapter/claude.json"), repo.join("response.json")).unwrap();
    let (code, valid) = run(&repo, home.path(), &["adapter", "validate", "--response", "response.json"]);
    assert_eq!((code, &valid["ok"]), (0, &Value::Bool(true)));
    let (code, refused) = run(&repo, home.path(), &["adapter", "authorize", "--response", "response.json"]);
    assert_eq!(code, 1, "no policy: default denial");
    assert_eq!(refused["data"]["executionAuthorized"], false, "the report stays in data on exit 1");
}

#[test]
fn a_context_pack_and_a_prompt_bundle_are_shown_and_verified_after_they_are_added() {
    let (home, _scratch, repo) = repo_copy();
    std::fs::write(repo.join("doc.md"), "a document\n").unwrap();
    std::fs::write(repo.join("manifest.json"), r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "manifest"]);
    let (code, added) = run(&repo, home.path(), &["context-pack", "add", "--manifest", "manifest.json"]);
    assert_eq!(code, 0);
    let id = added["data"]["id"].as_str().expect("the pack's id").to_string();
    assert_eq!(run(&repo, home.path(), &["context-pack", "show", &id]).0, 0);
    assert_eq!(run(&repo, home.path(), &["context-pack", "verify", &id]).0, 0);

    assert_eq!(run(&repo, home.path(), &["prompt", "add", "reviewer@v1", "--manifest", "manifest.json"]).0, 0);
    let (code, shown) = run(&repo, home.path(), &["prompt", "show", "reviewer@v1"]);
    assert_eq!((code, shown["data"]["name"].as_str()), (0, Some("reviewer")));
}

#[test]
fn a_reviewer_accepts_a_divergent_scenario_attested_by_another_actor() {
    let (home, _scratch, repo) = repo_copy();
    let feature = ".canon/specs/kind=feature/area=auth/login.feature";
    assert!(feature.len() > 1);
    let (code, _) = run(
        &repo,
        home.path(),
        &["evidence", "add", "--project-id", "root", "--scenario-id", "auth.login.02", "--class", "test-run", "--ref", "npm test", "--verdict", "divergent", "--summary", "1 failed", "--actor-id", "impl", "--session-id", "s1", "--role", "implementer"],
    );
    assert_eq!(code, 0);
    let (code, accepted) = run(
        &repo,
        home.path(),
        &["divergence", "accept", "auth.login.02", "--project-id", "root", "--reason", "the engine drops the event", "--tracking", "https://example.com/engine/issues/1", "--actor-id", "rev", "--role", "reviewer", "--session-id", "s9"],
    );
    assert_eq!((code, &accepted["ok"]), (0, &Value::Bool(true)), "{accepted}");
}

#[test]
fn leaves_whose_refusals_no_other_test_provokes_answer_with_the_failure_envelope() {
    let (home, _scratch, repo) = repo_copy();
    for (args, leaf, code) in [
        (vec!["completion", "no-such-shell"], "completion", "USAGE"),
        (vec!["selftest", "--suite", "no-such-suite"], "selftest", "USAGE"),
        (vec!["demo", "init", "--repo", "canon.yaml"], "demo init", "WRITE_FAILED"),
        (vec!["subject", "show", "no-such-subject"], "subject show", "SUBJECT_NOT_FOUND"),
    ] {
        let (exit, body) = run(&repo, home.path(), &args);
        assert_eq!(exit, 2, "{args:?}");
        assert_eq!((body["meta"]["command"].as_str(), body["error"]["code"].as_str()), (Some(leaf), Some(code)), "{body}");
    }
}

/// Every leaf that takes `--input` answers a one-element batch with its batch report (D7), in a copy of the fixture.
#[test]
fn every_batch_leaf_answers_a_one_element_batch_with_its_batch_report() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    let mut batches = 0;
    for node in leaves() {
        let Some(input) = node["inputs"].as_array().unwrap().iter().find(|i| i["batch"] == true) else { continue };
        let path = path_of(node);
        let (_scratch, mut repo) = copy_of(base.path());
        let _fresh;
        if path == "init" || path == "demo init" {
            _fresh = TempDir::new().unwrap();
            repo = _fresh.path().to_path_buf();
        }
        let document_text = serde_json::to_string(&Value::Array(vec![input["example"].clone()])).unwrap();
        let mut argv = path.split(' ').map(str::to_string).collect::<Vec<_>>();
        argv.extend(["--input".to_string(), document_text, "--json".to_string()]);
        let out = canon_in(&repo, home.path(), &argv, &[]);
        let body = document(&out);
        let problems = validate_document(&body);
        assert!(problems.is_empty(), "canon {path} --input [..]: {problems:?}\n{body}");
        if body["data"]["items"].is_array() {
            batches += 1;
        }
    }
    assert!(batches > 20, "batch reports were answered ({batches})");
}
