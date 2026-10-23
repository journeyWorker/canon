//! T7 (0.28 D3): every leaf's `--json` output validates against its schema.
//!
//! In debug builds `json_out` already validates each document it prints, so the
//! whole suite is a check. This test does it from the outside, for each leaf's
//! spec example run in a scratch repo, against the schemas of the generated spec:
//! the envelope (`ok`, `data` or `error`, `meta`), then `data` against the report
//! the leaf declares, `meta.command` naming the leaf and `meta.schemaVersion` the
//! report's version, with stderr empty. The validator is also shown to reject a
//! document that breaks each rule, so a passing run is not vacuous.

mod harness;

use harness::*;
use serde_json::{json, Value};
use tempfile::TempDir;

/// Leaves whose example is not run, and why.
const SKIPPED: &[(&str, &str)] = &[("dashboard", "binds a port and serves until the process is stopped")];

#[test]
fn every_leafs_example_validates_against_its_report_schema() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    let (mut successes, mut failures) = (0, 0);
    let mut problems = Vec::new();
    for node in leaves() {
        let path = path_of(node);
        if SKIPPED.iter().any(|(leaf, _)| *leaf == path) {
            continue;
        }
        let scratch = TempDir::new().unwrap();
        let mut cwd = scratch.path().join("repo");
        copy_dir(base.path(), &cwd);
        if path == "init" {
            cwd = scratch.path().join("fresh");
            std::fs::create_dir_all(&cwd).unwrap();
        }
        let mut argv = example_argv(node);
        argv.push("--json".into());
        let out = canon_in(&cwd, home.path(), &argv, &[]);
        if !out.stderr.is_empty() {
            problems.push(format!("`{path}`: stderr is not empty in JSON mode: {}", String::from_utf8_lossy(&out.stderr)));
        }
        let document: Value = match serde_json::from_slice(&out.stdout) {
            Ok(document) => document,
            Err(e) => {
                problems.push(format!("`{path}`: stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)));
                continue;
            }
        };
        if document["meta"]["command"] != path.as_str() {
            problems.push(format!("`{path}`: meta.command is {}", document["meta"]["command"]));
        }
        if document["ok"] == true {
            successes += 1;
        } else {
            failures += 1;
        }
        problems.extend(validate_document(&document).into_iter().map(|p| format!("`{path}`: {p}")));
    }
    assert!(successes > 40 && failures > 5, "the examples ran ({successes} successes, {failures} refusals)");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_validator_rejects_a_document_that_breaks_a_rule() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    let good = document(&ok(base.path(), home.path(), &["status", "--json"]));
    assert_eq!(validate_document(&good), Vec::<String>::new());

    let mut wrong_type = good.clone();
    wrong_type["data"]["subjects"] = json!("not a list");
    assert!(!validate_document(&wrong_type).is_empty(), "a field of the wrong type");

    let mut missing = good.clone();
    missing["data"].as_object_mut().unwrap().clear();
    assert!(!validate_document(&missing).is_empty(), "a report without its required fields");

    let mut other_leaf = good.clone();
    other_leaf["meta"]["command"] = json!("gate check");
    assert!(!validate_document(&other_leaf).is_empty(), "data of another leaf's report");

    let mut no_meta = good.clone();
    no_meta.as_object_mut().unwrap().remove("meta");
    assert!(!validate_document(&no_meta).is_empty(), "an envelope without meta");

    let mut version = good.clone();
    version["meta"]["schemaVersion"] = json!("9.9");
    assert!(!validate_document(&version).is_empty(), "a version the report does not declare");

    let failure = json!({"ok": false, "error": {"code": "USAGE", "message": "m", "suggestions": [], "hint": "h", "retryable": false}, "meta": good["meta"]});
    assert!(validate_document(&failure).is_empty(), "a bare failure envelope is valid: {:?}", validate_document(&failure));
    let unspecified = json!({"ok": false, "meta": good["meta"]});
    assert!(!validate_document(&unspecified).is_empty(), "a failure without an error");
}
