//! Batches of `--input` follow spec 11.8 and 11.11 (`canon-0-35-batch-pointers`, scenarios `atip.batchpointers.*`),
//! against the built binary: a preflight refusal is the item's own code with a `/<index>/…` pointer and no `data`, a
//! failure while an item runs is `BATCH_INCOMPLETE` at `/<index>` (index 0 too), preflight reads no state, and a failed
//! item has its own `error` and no `data`.
//!
//! Tests are named by scenario id (`atip_batchpointers_01_…` proves `atip.batchpointers.01`).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};
use tempfile::TempDir;

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-batchpointers").stdin(Stdio::null()).output().expect("spawn canon")
}

fn json_of(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn ok(dir: &Path, args: &[&str]) {
    let out = canon(dir, args);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// A git repo holding the story of the examples: subject `demo` with change `add-login`, scenario `auth.login.01`, finding 1.
fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    let git = |args: &[&str]| {
        let out = Command::new("git").current_dir(repo).args(["-c", "user.name=canon", "-c", "user.email=canon@example.invalid"]).args(args).output().expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    ok(repo, &["init", "--providers", "none"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(repo, &["inventory", "sync"]);
    ok(repo, &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "bob", "--summary", "Naming nit"]);
    dir
}

/// Every file of the repo but `.git` and the hot tier's SQLite files, with its bytes.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            if rel == ".git" || rel.starts_with(".canon/hot.db") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn close(seq: u32, disposition: &str) -> Value {
    let mut item = json!({"operands": ["add-login"], "round": 1, "seq": seq, "disposition": disposition});
    if matches!(disposition, "rejected" | "deferred") {
        item["reason"] = json!("decided in review");
    }
    item
}

fn run_batch(repo: &Path, leaf: &[&str], items: &[Value]) -> (Option<i32>, Value) {
    let doc = Value::Array(items.to_vec()).to_string();
    let mut argv: Vec<&str> = leaf.to_vec();
    argv.extend(["--input", &doc, "--json"]);
    let out = canon(repo, &argv);
    (out.status.code(), json_of(&out))
}

fn statuses(body: &Value) -> Vec<&str> {
    body["data"]["items"].as_array().expect("a batch report").iter().map(|item| item["status"].as_str().unwrap()).collect()
}

#[test]
fn atip_batchpointers_01_a_preflight_refusal_is_the_items_code_at_its_index_with_no_data_and_nothing_ran() {
    let dir = fixture();
    let repo = dir.path();
    let before = tree(repo);
    let leaf = ["finding", "close"];
    let good = close(1, "deferred");
    // a type, an enumeration, an unknown key, a domain and a constraint: each refused whole, at index 1
    for (bad, code, field) in [
        (json!({"operands": ["add-login"], "round": "x", "seq": 1, "disposition": "open"}), "INPUT_INVALID", "/1/round"),
        (json!({"operands": ["add-login"], "round": 1, "seq": 1, "disposition": "nope"}), "INPUT_INVALID", "/1/disposition"),
        (json!({"operands": ["add-login"], "round": 1, "seq": 1, "disposition": "open", "bogus": true}), "INPUT_INVALID", "/1/bogus"),
        (json!({"operands": ["add-login"], "round": 0, "seq": 1, "disposition": "open"}), "FINDING_INVALID_ARGUMENT", "/1/round"),
        (json!({"operands": ["add-login"], "round": 1, "seq": 1, "disposition": "rejected"}), "", "/1"),
    ] {
        let (exit, body) = run_batch(repo, &leaf, &[good.clone(), bad.clone()]);
        assert_eq!(exit, Some(2), "{bad}: {body}");
        assert!(body.get("data").is_none(), "a preflight refusal has no data: {bad}: {body}");
        let found = body["error"]["field"].as_str().unwrap_or_default();
        assert!(found == field || found.starts_with(&format!("{field}/")) || (field == "/1" && found.starts_with("/1/")), "{bad}: field {found}, wanted {field}: {body}");
        if !code.is_empty() {
            assert_eq!(body["error"]["code"], code, "{bad}: {body}");
        }
        assert!(!body.to_string().contains("/items/"), "no pointer carries `/items/`: {body}");
        assert_eq!(tree(repo), before, "{bad}: the first item did not run");
    }
    // the same item at index 0 is `/0/…`, and the single document keeps `/round`
    let (exit, body) = run_batch(repo, &leaf, &[json!({"operands": ["add-login"], "round": "x", "seq": 1, "disposition": "open"})]);
    assert_eq!((exit, body["error"]["field"].as_str()), (Some(2), Some("/0/round")), "{body}");
    let out = canon(repo, &["finding", "close", "--input", r#"{"round":"x"}"#, "--json"]);
    assert_eq!(json_of(&out)["error"]["field"], "/round");
    // a conflict names the document pointer of the item, and an item that is no object is its index alone
    let doc = json!([good, {"operands": ["add-login"], "round": 2}]).to_string();
    let (exit, code, field) = {
        let out = canon(repo, &["finding", "close", "--round", "1", "--input", &doc, "--json"]);
        let body = json_of(&out);
        (out.status.code(), body["error"]["code"].as_str().unwrap_or_default().to_string(), body["error"]["field"].as_str().unwrap_or_default().to_string())
    };
    assert_eq!((exit, code.as_str(), field.as_str()), (Some(2), "INPUT_CONFLICT", "/1/round"));
    let (exit, body) = run_batch(repo, &leaf, &[good.clone(), json!(3)]);
    assert_eq!((exit, body["error"]["reason"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("NOT_OBJECT"), Some("/1")), "{body}");
    assert_eq!(tree(repo), before);
}

#[test]
fn atip_batchpointers_02_a_batch_of_good_items_is_exit_zero_with_one_report_per_item_in_order() {
    let dir = fixture();
    let repo = dir.path();
    ok(repo, &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "2", "--severity", "note", "--actor", "bob", "--summary", "Another nit"]);
    // two different findings, so that running the batch again changes nothing
    let items = [close(1, "deferred"), close(2, "rejected")];
    let (exit, body) = run_batch(repo, &["finding", "close"], &items);
    assert_eq!(exit, Some(0), "{body}");
    assert_eq!(body["ok"], true, "{body}");
    assert!(body.get("error").is_none(), "{body}");
    let report = body["data"]["items"].as_array().expect("a batch report");
    assert_eq!(report.iter().map(|item| item["index"].as_u64().unwrap()).collect::<Vec<_>>(), [0, 1], "{body}");
    assert_eq!(statuses(&body), ["written", "written"], "{body}");
    assert!(report.iter().all(|item| item["data"].is_object() && item.get("error").is_none()), "{body}");
    // run again: every item reports unchanged
    let (exit, body) = run_batch(repo, &["finding", "close"], &items);
    assert_eq!(exit, Some(0), "{body}");
    assert_eq!(statuses(&body), ["unchanged", "unchanged"], "{body}");
}

#[test]
fn atip_batchpointers_03_an_item_that_fails_when_it_runs_is_batch_incomplete_at_its_index() {
    let dir = fixture();
    let repo = dir.path();
    // item 1 names a finding no ledger has: it passes preflight and fails when it runs
    let (exit, body) = run_batch(repo, &["finding", "close"], &[close(1, "deferred"), close(9, "open"), close(1, "rejected")]);
    assert_eq!(exit, Some(1), "{body}");
    assert_eq!((body["ok"].as_bool(), body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(false), Some("BATCH_INCOMPLETE"), Some("/1")), "{body}");
    assert_eq!(statuses(&body), ["written", "failed", "skipped"], "{body}");
    assert_eq!(body["data"]["items"][1]["error"]["code"], "FINDING_NOT_FOUND", "{body}");
    assert_eq!(body["meta"]["command"], "finding close", "{body}");
    // a repeat reports the finished item unchanged and fails at the same place
    let (exit, body) = run_batch(repo, &["finding", "close"], &[close(1, "deferred"), close(9, "open"), close(1, "rejected")]);
    assert_eq!((exit, body["error"]["field"].as_str()), (Some(1), Some("/1")), "{body}");
    assert_eq!(statuses(&body), ["unchanged", "failed", "skipped"], "{body}");
}

#[test]
fn atip_batchpointers_04_the_first_item_failing_is_exit_one_at_index_zero_and_the_field_is_only_the_index() {
    let dir = fixture();
    let repo = dir.path();
    let (exit, body) = run_batch(repo, &["finding", "close"], &[close(9, "open"), close(1, "deferred")]);
    assert_eq!(exit, Some(1), "not exit 2 although the first item failed: {body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some("BATCH_INCOMPLETE"), Some("/0")), "{body}");
    assert_eq!(statuses(&body), ["failed", "skipped"], "{body}");

    // an item error that points deeper is deeper in the item's own `error` only
    let feature = ".canon/specs/kind=feature/area=auth/login.feature";
    let items = [
        json!({"operands": ["auth.login.02"], "title": "Two", "feature": feature}),
        json!({"operands": ["auth.login.01"], "title": "Again", "feature": feature}),
        json!({"operands": ["auth.login.03"], "title": "Three", "feature": feature}),
    ];
    let (exit, body) = run_batch(repo, &["scenario", "new"], &items);
    assert_eq!(exit, Some(1), "{body}");
    assert_eq!(body["error"]["field"], "/1", "{body}");
    assert_eq!(body["data"]["items"][1]["error"]["code"], "SCENARIO_TAG_EXISTS", "{body}");
    assert_eq!(body["data"]["items"][1]["error"]["field"], "/1/operands", "{body}");
    assert_eq!(statuses(&body), ["written", "failed", "skipped"], "{body}");
}

#[test]
fn atip_batchpointers_05_preflight_does_not_run_the_dry_run_so_a_missing_object_fails_the_item_not_the_batch() {
    let dir = fixture();
    let repo = dir.path();
    // `subject adopt` has `--dry-run`; take the change out of the subject first so that adopting it changes something
    ok(repo, &["subject", "unadopt", "demo", "--change", "add-login"]);
    let taken_out = tree(repo);
    let items = [json!({"operands": ["demo"], "change": "add-login"}), json!({"operands": ["demo"], "change": "no-such-change"})];
    let (exit, body) = run_batch(repo, &["subject", "adopt"], &items);
    assert_eq!(exit, Some(1), "was exit 2 when preflight ran each item's --dry-run: {body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some("BATCH_INCOMPLETE"), Some("/1")), "{body}");
    assert_eq!(statuses(&body), ["written", "failed"], "{body}");
    assert_ne!(tree(repo), taken_out, "the first item ran");
    // a fault that reads no state is still preflight: exit 2, no item ran
    ok(repo, &["subject", "unadopt", "demo", "--change", "add-login"]);
    let taken_out = tree(repo);
    let items = [json!({"operands": ["demo"], "change": "add-login"}), json!({"operands": ["demo"], "change": "add-login", "bogus": 1})];
    let (exit, body) = run_batch(repo, &["subject", "adopt"], &items);
    assert_eq!((exit, body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("INPUT_INVALID"), Some("/1/bogus")), "{body}");
    assert!(body.get("data").is_none(), "{body}");
    assert_eq!(tree(repo), taken_out, "no item ran");
}

#[test]
fn atip_batchpointers_06_an_item_that_needs_what_an_earlier_item_writes_passes_preflight_and_runs_in_order() {
    let dir = fixture();
    let repo = dir.path();
    // `proposed → specced → building` is the chain a subject moves along: the second item is valid only once the first has run, so no
    // state read before the first item can accept it, and preflight reads none
    let step = |to: &str| json!({"operands": ["demo"], "to": to});
    let (exit, body) = run_batch(repo, &["subject", "move"], &[step("specced"), step("building")]);
    assert_eq!(exit, Some(0), "{body}");
    assert_eq!(statuses(&body), ["written", "written"], "{body}");
    // the order is the order of the items: the same two the other way round fail at the first
    let dir = fixture();
    let repo = dir.path();
    let (exit, body) = run_batch(repo, &["subject", "move"], &[step("building"), step("specced")]);
    assert_eq!(exit, Some(1), "{body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some("BATCH_INCOMPLETE"), Some("/0")), "{body}");
    assert_eq!(statuses(&body), ["failed", "skipped"], "{body}");
    assert_eq!(body["data"]["items"][0]["error"]["code"], "SUBJECT_INVALID_TRANSITION", "{body}");
}

#[test]
fn atip_batchpointers_07_a_failed_item_has_its_error_and_no_data_and_a_skipped_item_has_neither() {
    let dir = fixture();
    let repo = dir.path();
    let (exit, body) = run_batch(repo, &["finding", "close"], &[close(1, "deferred"), close(9, "open"), close(1, "rejected"), close(1, "open")]);
    assert_eq!(exit, Some(1), "{body}");
    let items = body["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 4, "one entry per submitted item: {body}");
    assert!(items[0]["data"].is_object() && items[0].get("error").is_none(), "{body}");
    assert!(items[1].get("data").is_none(), "a failed item has no data: {body}");
    assert_eq!(items[1]["error"]["code"], "FINDING_NOT_FOUND", "{body}");
    assert_eq!(items[1]["error"]["retryable"], false, "the registry's retryable: {body}");
    for skipped in &items[2..] {
        assert_eq!(skipped["status"], "skipped", "{body}");
        assert!(skipped.get("data").is_none() && skipped.get("error").is_none(), "{body}");
    }
}

#[test]
fn atip_batchpointers_08_a_batch_with_an_item_that_fails_is_never_exit_zero_and_the_failed_and_later_items_report_no_success() {
    let dir = fixture();
    let repo = dir.path();
    // item 1 names a finding no ledger has: the batch has a failure, so it is not the success report of step 2
    let (exit, body) = run_batch(repo, &["finding", "close"], &[close(1, "deferred"), close(9, "open"), close(1, "rejected")]);
    assert_eq!(exit, Some(1), "{body}");
    assert_eq!((body["ok"].as_bool(), body["error"]["code"].as_str()), (Some(false), Some("BATCH_INCOMPLETE")), "{body}");
    let items = body["data"]["items"].as_array().expect("a batch report");
    assert_eq!(statuses(&body), ["written", "failed", "skipped"], "{body}");
    for item in &items[1..] {
        assert!(!matches!(item["status"].as_str(), Some("written" | "unchanged")), "the failed and the later items report no success: {body}");
        assert!(item.get("data").is_none(), "{body}");
    }
}

#[test]
fn atip_batchpointers_09_a_failed_item_points_at_what_to_fix_in_its_own_error_and_the_items_after_it_carry_nothing() {
    let dir = fixture();
    let repo = dir.path();
    let feature = ".canon/specs/kind=feature/area=auth/login.feature";
    let items = [
        json!({"operands": ["auth.login.02"], "title": "Two", "feature": feature}),
        json!({"operands": ["auth.login.01"], "title": "Again", "feature": feature}),
        json!({"operands": ["auth.login.03"], "title": "Three", "feature": feature}),
    ];
    let (exit, body) = run_batch(repo, &["scenario", "new"], &items);
    assert_eq!(exit, Some(1), "{body}");
    let report = body["data"]["items"].as_array().expect("a batch report");
    assert_eq!(report[1]["error"]["code"], "SCENARIO_TAG_EXISTS", "{body}");
    assert_eq!(report[1]["error"]["field"], "/1/operands", "the field of the item's own error names what to fix: {body}");
    assert!(report[1].get("data").is_none(), "a failed item has no data: {body}");
    assert_eq!(report[2]["status"], "skipped", "{body}");
    assert!(report[2].get("data").is_none() && report[2].get("error").is_none(), "a skipped item carries nothing: {body}");
}
