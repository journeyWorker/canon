//! Slice A of the 0.28 self-corpus update: behaviour tests, through the real
//! `canon` binary, for the evidence, finding, review and gate scenarios that
//! no earlier test proves. Exit codes, JSON fields, records written and trees
//! left unchanged; no help text and no wording.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};
use tempfile::TempDir;

const FEATURE: &str = ".canon/specs/kind=feature/area=auth/login.feature";
const CHANGE: &str = "add-login";

struct Fixture {
    dir: TempDir,
    home: TempDir,
}

impl Fixture {
    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn run(&self, args: &[&str]) -> Output {
        run(self.path(), self.home.path(), args)
    }

    fn ok(&self, args: &[&str]) -> Output {
        let out = self.run(args);
        assert!(out.status.success(), "canon {args:?}: {}{}", text(&out.stdout), text(&out.stderr));
        out
    }
}

fn run(repo: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(args)
        .current_dir(repo)
        .env("CANON_ACTOR", "spec028-a")
        .env("HOME", home)
        .env_remove("CANON_ROLE")
        .output()
        .expect("spawn canon")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn json_of(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}{}", text(&out.stdout), text(&out.stderr)))
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["-c", "user.name=canon", "-c", "user.email=canon@example.invalid"])
        .args(args)
        .output()
        .expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", text(&out.stderr));
    text(&out.stdout).trim().to_string()
}

/// A git repo with one commit, an initialized canon and two indexed scenarios
/// that no subject owns, so no brief gates their evidence.
fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let f = Fixture { dir, home };
    git(f.path(), &["init", "-q", "-b", "main"]);
    f.ok(&["init", "--providers", "none", "--no-agents-md"]);
    std::fs::write(f.path().join("lib.rs"), "pub fn login() {}\n").unwrap();
    git(f.path(), &["add", "-A"]);
    git(f.path(), &["commit", "-q", "-m", "setup"]);
    f.ok(&["feature", "new", "auth.login", "--title", "Login"]);
    for (id, title) in [("auth.login.01", "Sign in"), ("auth.login.02", "Sign out")] {
        f.ok(&["scenario", "new", id, "--title", title, "--feature", FEATURE]);
    }
    f.ok(&["inventory", "sync"]);
    git(f.path(), &["add", "-A"]);
    git(f.path(), &["commit", "-q", "-m", "scenarios"]);
    f
}

/// Every file of the repo by relative path (not `.git`, not the hot tier's
/// SQLite pages, which are never records).
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

fn files_under(dir: PathBuf) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir];
    while let Some(next) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The committed ledger records of one kind.
fn committed(repo: &Path, kind: &str) -> Vec<Value> {
    files_under(repo.join(".canon/ledger").join(format!("kind={kind}")))
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap())
        .collect()
}

/// The reference of each committed evidence record, ordered by scenario id. The ledger keeps the
/// newest record of a scenario, so this is one entry per attested scenario.
fn refs(repo: &Path) -> Vec<String> {
    let mut rows: Vec<(String, String)> = committed(repo, "evidence")
        .iter()
        .map(|r| (r["scenario_id"].as_str().unwrap().to_string(), r["evidence"]["ref"].as_str().unwrap().to_string()))
        .collect();
    rows.sort();
    rows.into_iter().map(|(_, reference)| reference).collect()
}

fn evidence_doc(scenario: &str, reference: &str) -> Value {
    json!({"scenario": scenario, "class": "test-run", "ref": reference, "role": "implementer"})
}

fn finding_doc(seq: u32, severity: &str) -> Value {
    json!({"change": CHANGE, "round": 1, "seq": seq, "severity": severity, "introducedUnknown": true, "actor": "reviewer-2", "summary": format!("finding {seq}")})
}

fn review_doc(scenarios: &[&str]) -> Value {
    json!({"operands": scenarios, "actor": "reviewer-2", "role": "reviewer", "originalSpecRef": "docs/prd.md"})
}

fn statuses(body: &Value) -> Vec<String> {
    body["data"]["items"].as_array().unwrap_or_else(|| panic!("a batch report: {body}")).iter().map(|i| i["status"].as_str().unwrap().to_string()).collect()
}

// ------------------------------------------------------------- evidence add

#[test]
fn evidence_add_106_an_object_is_one_report_and_an_array_or_ndjson_is_a_batch() {
    let f = fixture();
    let r = f.path();

    let one = f.ok(&["evidence", "add", "--input", &evidence_doc("auth.login.01", "npm test").to_string(), "--json"]);
    let body = json_of(&one);
    assert_eq!(body["meta"]["command"], "evidence add");
    assert!(body["data"]["records"].is_array() && body["data"].get("items").is_none(), "an object returns the ordinary report: {body}");
    assert_eq!(committed(r, "evidence").len(), 1);

    let array = json!([evidence_doc("auth.login.02", "npm test"), evidence_doc("auth.login.01", "npm test again")]);
    let batch = f.ok(&["evidence", "add", "--input", &array.to_string(), "--json"]);
    let body = json_of(&batch);
    assert_eq!(body["meta"]["command"], "evidence add", "the batch keeps the leaf id");
    assert_eq!(statuses(&body), ["written", "written"]);
    assert_eq!(body["data"]["items"][0]["index"], 0);
    assert_eq!(body["data"]["items"][1]["index"], 1);
    assert_eq!(body["data"]["items"][0]["data"]["records"][0]["subject"], "auth.login.02");
    assert_eq!(refs(r), ["npm test again", "npm test"], "one record per element, newest per scenario");

    // One element in an array is still a batch; NDJSON from a file is a batch.
    let single = f.ok(&["evidence", "add", "--input", &json!([evidence_doc("auth.login.02", "a third run")]).to_string(), "--json"]);
    assert_eq!(statuses(&json_of(&single)), ["written"]);
    assert_eq!(refs(r), ["npm test again", "a third run"]);
    std::fs::write(r.join("batch.ndjson"), format!("# the suite\n\n{}\n{}\n", evidence_doc("auth.login.01", "run a"), evidence_doc("auth.login.02", "run b"))).unwrap();
    let nd = f.ok(&["evidence", "add", "--input", "batch.ndjson", "--json"]);
    assert_eq!(statuses(&json_of(&nd)), ["written", "written"]);
    assert_eq!(refs(r), ["run a", "run b"]);
}

#[test]
fn evidence_add_107_one_invalid_element_writes_nothing_and_points_at_it() {
    let f = fixture();
    let r = f.path();
    let before = tree(r);

    let bad_value = json!([evidence_doc("auth.login.01", "npm test"), {"scenario": "auth.login.02", "class": "test-run", "ref": "npm test", "role": "implementer", "verdict": "great"}]);
    let out = f.run(&["evidence", "add", "--input", &bad_value.to_string(), "--json"]);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(2), "{body}");
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["field"], "/items/1/verdict", "{body}");
    assert_eq!(tree(r), before, "the valid first element was not written either");

    let missing_key = json!([evidence_doc("auth.login.01", "npm test"), {"scenario": "auth.login.02", "ref": "npm test", "role": "implementer"}]);
    let out = f.run(&["evidence", "add", "--input", &missing_key.to_string(), "--json"]);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(2), "{body}");
    assert_eq!(body["error"]["field"], "/items/1/class", "{body}");
    assert_eq!(tree(r), before);
}

#[test]
fn evidence_add_108_a_repeat_writes_nothing_and_is_reported_unchanged() {
    let f = fixture();
    let r = f.path();
    let args = ["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--json"];
    let first = f.ok(&args);
    assert!(json_of(&first)["data"].get("unchanged").is_none() && json_of(&first)["meta"].get("unchanged").is_none(), "the first add writes");
    let before = tree(r);

    let again = f.ok(&args);
    assert_eq!(json_of(&again)["data"]["unchanged"], true);
    assert!(json_of(&again)["meta"].get("unchanged").is_none(), "unchanged is the report's, not meta's");
    assert_eq!(tree(r), before, "a repeat writes nothing");
    assert_eq!(committed(r, "evidence").len(), 1);

    // The same holds for a batch: every element that already landed is unchanged and the exit is 0.
    let batch = json!([evidence_doc("auth.login.01", "npm test"), evidence_doc("auth.login.02", "npm test")]).to_string();
    let first = f.ok(&["evidence", "add", "--input", &batch, "--json"]);
    assert_eq!(statuses(&json_of(&first)), ["unchanged", "written"]);
    let before = tree(r);
    let again = f.ok(&["evidence", "add", "--input", &batch, "--json"]);
    assert_eq!(statuses(&json_of(&again)), ["unchanged", "unchanged"]);
    assert_eq!(tree(r), before);
}

#[test]
fn evidence_add_113_an_explicit_faithful_verdict_over_a_skipped_or_failed_case_is_refused() {
    let f = fixture();
    let r = f.path();
    std::fs::write(r.join("skipped.xml"), r#"<testsuite><testcase name="auth.login.01 signs in"/><testcase name="auth.login.01 flaky"><skipped/></testcase></testsuite>"#).unwrap();
    std::fs::write(r.join("failed.xml"), r#"<testsuite><testcase name="auth.login.01 signs in"/><testcase name="auth.login.01 breaks"><failure/></testcase></testsuite>"#).unwrap();
    let add = |report: &str, verdict: Option<&str>| {
        let mut args = vec!["evidence", "add", "--scenario", "auth.login.01", "--report", report, "--role", "implementer", "--json"];
        if let Some(verdict) = verdict {
            args.extend(["--verdict", verdict]);
        }
        f.run(&args)
    };
    let verdict = || committed(r, "evidence")[0]["verdict"].as_str().unwrap().to_string();

    for report in ["junit:skipped.xml", "junit:failed.xml"] {
        let before = tree(r);
        let out = add(report, Some("faithful"));
        assert_eq!(out.status.code(), Some(1), "{report}: {}{}", text(&out.stdout), text(&out.stderr));
        assert_eq!(tree(r), before, "{report}: a refusal writes nothing");
    }
    let out = add("junit:skipped.xml", None);
    assert!(out.status.success(), "{}{}", text(&out.stdout), text(&out.stderr));
    assert_eq!(verdict(), "not_applicable");
    let out = add("junit:failed.xml", None);
    assert!(out.status.success(), "{}{}", text(&out.stdout), text(&out.stderr));
    assert_eq!(verdict(), "divergent");
}

// ------------------------------------------------------------- finding add

#[test]
fn finding_add_34_an_object_is_one_report_and_an_array_is_a_batch() {
    let f = fixture();
    let r = f.path();

    let one = f.ok(&["finding", "add", "--input", &finding_doc(1, "note").to_string(), "--json"]);
    let body = json_of(&one);
    assert_eq!(body["meta"]["command"], "finding add");
    assert_eq!(body["data"]["changeId"], CHANGE);
    assert!(body["data"].get("items").is_none(), "an object returns the ordinary report: {body}");

    let batch = f.ok(&["finding", "add", "--input", &json!([finding_doc(2, "should-fix"), finding_doc(3, "blocker")]).to_string(), "--json"]);
    let body = json_of(&batch);
    assert_eq!(statuses(&body), ["written", "written"]);
    assert_eq!(body["data"]["items"][1]["data"]["seq"], 3);
    let mut seqs: Vec<u64> = committed(r, "finding").iter().map(|f| f["seq"].as_u64().unwrap()).collect();
    seqs.sort();
    assert_eq!(seqs, [1, 2, 3]);
}

#[test]
fn finding_add_09_a_finding_is_a_recorded_observation_never_checked_against_the_code() {
    let f = fixture();
    let r = f.path();
    f.ok(&[
        "finding", "add", "--change", CHANGE, "--round", "2", "--seq", "3", "--severity", "blocker", "--introduced-unknown", "--actor", "reviewer-2", "--summary", "the retry loop never ends", "--file-ref",
        "no/such/file.rs:1-2",
    ]);
    let findings = committed(r, "finding");
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding["summary"], "the retry loop never ends");
    assert_eq!(finding["file_ref"], "no/such/file.rs:1-2", "a path that does not exist is recorded as given");
    assert_eq!((finding["change_id"].as_str(), finding["round"].as_u64(), finding["seq"].as_u64()), (Some(CHANGE), Some(2), Some(3)));
    assert_eq!((finding["severity"].as_str(), finding["disposition"].as_str()), (Some("blocker"), Some("open")));
    assert_eq!(finding["reviewer"], "reviewer-2");
    assert_eq!(finding["actor"]["agent_id"], "reviewer-2");
    assert!(finding.get("introduced_by").is_none(), "no introducing commit is guessed: {finding}");
}

#[test]
fn finding_add_35_one_invalid_element_writes_nothing_and_points_at_it() {
    let f = fixture();
    let r = f.path();
    let before = tree(r);

    let doc = json!([finding_doc(1, "note"), finding_doc(2, "urgent")]).to_string();
    let out = f.run(&["finding", "add", "--input", &doc, "--json"]);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(2), "{body}");
    assert_eq!(body["error"]["field"], "/items/1/severity", "{body}");
    assert_eq!(tree(r), before, "the valid first element was not written either");

    let mut no_summary = finding_doc(2, "note");
    no_summary.as_object_mut().unwrap().remove("summary");
    let out = f.run(&["finding", "add", "--input", &json!([finding_doc(1, "note"), no_summary]).to_string(), "--json"]);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(2), "{body}");
    assert_eq!(body["error"]["field"], "/items/1/summary", "{body}");
    assert_eq!(tree(r), before);
}

#[test]
fn finding_add_06_an_annotated_tag_resolves_to_the_commit_it_names() {
    let f = fixture();
    let r = f.path();
    let commit = git(r, &["rev-parse", "HEAD"]);
    git(r, &["tag", "-a", "v1", "-m", "release"]);
    let tag_object = git(r, &["rev-parse", "v1"]);
    assert_ne!(tag_object, commit, "the premise: an annotated tag has an object id of its own");

    for (seq, rev) in [(1, "v1"), (2, tag_object.as_str())] {
        let out = f.run(&[
            "finding", "add", "--change", CHANGE, "--round", "1", "--seq", &seq.to_string(), "--severity", "note", "--actor", "reviewer-2", "--summary", "tagged", "--rev", rev,
            "--introduced-rev", rev, "--json",
        ]);
        assert!(out.status.success(), "{rev}: {}{}", text(&out.stdout), text(&out.stderr));
    }
    let findings = committed(r, "finding");
    assert_eq!(findings.len(), 2);
    for finding in findings {
        assert_eq!(finding["reviewed_sha"], commit.as_str(), "{finding}");
        assert_eq!(finding["introduced_by"], commit.as_str(), "never the tag object's own id: {finding}");
    }
}

#[test]
fn finding_add_31_a_short_revision_that_names_no_commit_is_refused_and_nothing_is_staged() {
    let f = fixture();
    let r = f.path();
    let before = tree(r);
    for flag in ["--rev", "--introduced-rev", "--resolution-rev"] {
        let mut args = vec!["finding", "add", "--change", CHANGE, "--round", "1", "--seq", "1", "--severity", "note", "--actor", "reviewer-2", "--summary", "s", "--json"];
        if flag == "--resolution-rev" {
            args.extend(["--disposition", "fixed"]);
        }
        args.extend([flag, "deadbee"]);
        let out = f.run(&args);
        assert_eq!(out.status.code(), Some(2), "{flag}: {}{}", text(&out.stdout), text(&out.stderr));
        let body = json_of(&out);
        assert_eq!(body["error"]["code"], "NO_COMMIT", "{flag}: {body}");
        assert!(body["error"]["message"].as_str().unwrap().contains("deadbee"), "the refusal names the value: {body}");
    }
    assert_eq!(tree(r), before, "a refused add stages and commits nothing");
}

#[test]
fn finding_close_08_a_fixed_close_records_head_by_default_and_refuses_a_revision_it_cannot_hold() {
    let f = fixture();
    let r = f.path();
    let head = git(r, &["rev-parse", "HEAD"]);
    for seq in 1..=3u32 {
        f.ok(&["finding", "add", "--change", CHANGE, "--round", "1", "--seq", &seq.to_string(), "--severity", "note", "--actor", "reviewer-2", "--summary", "s"]);
    }

    let before = tree(r);
    let unheld = f.run(&["finding", "close", CHANGE, "--round", "1", "--seq", "1", "--disposition", "fixed", "--resolution-rev", &"b".repeat(40), "--json"]);
    assert_eq!(unheld.status.code(), Some(2), "{}", text(&unheld.stdout));
    assert_eq!(json_of(&unheld)["error"]["code"], "NO_COMMIT");
    let wrong_disposition = f.run(&["finding", "close", CHANGE, "--round", "1", "--seq", "1", "--disposition", "rejected", "--reason", "by design", "--resolution-rev", "HEAD", "--json"]);
    assert_eq!(wrong_disposition.status.code(), Some(2), "{}", text(&wrong_disposition.stdout));
    assert_eq!(tree(r), before, "a refused close writes nothing");

    f.ok(&["finding", "close", CHANGE, "--round", "1", "--seq", "1", "--disposition", "fixed"]);
    let closed = committed(r, "finding").into_iter().filter(|f| f["seq"] == 1 && f["disposition"] == "fixed").collect::<Vec<_>>();
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0]["resolution_sha"], head.as_str(), "no --resolution-rev records HEAD: {}", closed[0]);
    assert!(closed[0]["resolution_code_tree"].is_string(), "{}", closed[0]);
}

// ------------------------------------------------------------- review scenario

#[test]
fn review_scenario_93_an_object_is_one_report_and_an_array_is_a_batch() {
    let f = fixture();
    let r = f.path();

    // An object naming two operands writes one review per operand and returns the ordinary report.
    let one = f.ok(&["review", "scenario", "--input", &review_doc(&["auth.login.01", "auth.login.02"]).to_string(), "--json"]);
    let body = json_of(&one);
    assert_eq!(body["meta"]["command"], "review scenario");
    assert_eq!(body["data"]["reviews"].as_array().unwrap().len(), 2, "{body}");
    assert!(body["data"].get("items").is_none(), "an object returns the ordinary report: {body}");
    let mut reviewed: Vec<String> = committed(r, "review").iter().map(|review| review["scenario_id"].as_str().unwrap().to_string()).collect();
    reviewed.sort();
    assert_eq!(reviewed, ["auth.login.01", "auth.login.02"], "one review per operand");

    // An array is a batch of such documents: one item per element, in order, each written.
    let doc = |scenarios: &[&str], reviewer: &str| json!({"operands": scenarios, "actor": reviewer, "role": "reviewer", "originalSpecRef": "docs/prd.md"});
    let two = f.ok(&["review", "scenario", "--input", &json!([doc(&["auth.login.02"], "reviewer-3"), doc(&["auth.login.01", "auth.login.02"], "reviewer-4")]).to_string(), "--json"]);
    let body = json_of(&two);
    assert_eq!(statuses(&body), ["written", "written"]);
    assert_eq!(body["meta"]["command"], "review scenario");
    let items = body["data"]["items"].as_array().unwrap();
    assert_eq!(items.iter().map(|item| item["index"].as_u64().unwrap()).collect::<Vec<_>>(), [0, 1], "in order: {body}");
    let written: Vec<Vec<&str>> = items.iter().map(|item| item["data"]["reviews"].as_array().unwrap().iter().map(|review| review["location"].as_str().unwrap()).collect()).collect();
    assert_eq!(written[0].len(), 1, "the first element reviews its one operand: {body}");
    assert!(written[0][0].contains("auth.login.02"), "{body}");
    assert_eq!(written[1].len(), 2, "the second element reviews each of its operands: {body}");
    assert!(written[1][0].contains("auth.login.01") && written[1][1].contains("auth.login.02"), "{body}");
    assert_eq!(committed(r, "review").len(), 5);
}

#[test]
fn review_scenario_94_one_invalid_or_unindexed_element_writes_nothing() {
    let f = fixture();
    let r = f.path();
    let before = tree(r);

    let bad_outcome = json!([review_doc(&["auth.login.01"]), {"operands": ["auth.login.02"], "actor": "reviewer-2", "role": "reviewer", "originalSpecRef": "docs/prd.md", "reviewOutcome": "approved"}]);
    let out = f.run(&["review", "scenario", "--input", &bad_outcome.to_string(), "--json"]);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(2), "{body}");
    assert_eq!(body["error"]["field"], "/items/1/reviewOutcome", "{body}");
    assert_eq!(tree(r), before, "the valid first element was not written either");

    // A scenario no record carries is refused for the whole call.
    let unindexed = f.run(&["review", "scenario", "--input", &review_doc(&["auth.login.01", "auth.login.77"]).to_string(), "--json"]);
    assert_eq!(unindexed.status.code(), Some(2), "{}", text(&unindexed.stdout));
    assert_eq!(json_of(&unindexed)["error"]["code"], "REVIEW_SCENARIO_NOT_INDEXED");
    assert!(committed(r, "review").is_empty(), "no review is written for any of them");
}

#[test]
fn review_scenario_95_a_repeat_writes_nothing_and_is_reported_unchanged() {
    let f = fixture();
    let r = f.path();
    let doc = review_doc(&["auth.login.01"]).to_string();
    let first = f.ok(&["review", "scenario", "--input", &doc, "--json"]);
    assert!(json_of(&first)["data"].get("unchanged").is_none() && json_of(&first)["meta"].get("unchanged").is_none(), "the first review writes");
    let before = tree(r);

    let again = f.ok(&["review", "scenario", "--input", &doc, "--json"]);
    let body = json_of(&again);
    assert_eq!(body["data"]["unchanged"], true, "{body}");
    assert!(body["meta"].get("unchanged").is_none(), "unchanged is the report's, not meta's: {body}");
    assert_eq!(tree(r), before, "a repeat writes nothing");
    assert_eq!(committed(r, "review").len(), 1);

    let batch = json!([review_doc(&["auth.login.01"])]).to_string();
    let again = f.ok(&["review", "scenario", "--input", &batch, "--json"]);
    assert_eq!(statuses(&json_of(&again)), ["unchanged"]);
    assert_eq!(tree(r), before);
}

// ------------------------------------------------------------- gate check

#[test]
fn gate_check_102_nothing_examined_is_unverifiable_until_something_is() {
    let dir = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let r = dir.path();
    git(r, &["init", "-q", "-b", "main"]);
    let init = run(r, home.path(), &["init", "--providers", "none", "--no-agents-md", "--no-policy"]);
    assert!(init.status.success(), "{}{}", text(&init.stdout), text(&init.stderr));
    git(r, &["add", "-A"]);
    git(r, &["commit", "-q", "-m", "setup"]);

    let empty = run(r, home.path(), &["gate", "check", "--json"]);
    let body = json_of(&empty);
    assert_eq!(empty.status.code(), Some(1), "{body}");
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["code"], "UNVERIFIABLE", "{body}");
    assert_eq!(body["data"]["unverifiable"], true, "{body}");
    assert_eq!(body["data"]["clean"], false, "{body}");
    assert_eq!(body["meta"]["coverage"]["scenarios"], 0, "{body}");

    for args in [vec!["feature", "new", "auth.login", "--title", "Login"], vec!["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", FEATURE], vec!["inventory", "sync"]] {
        let out = run(r, home.path(), &args);
        assert!(out.status.success(), "{args:?}: {}{}", text(&out.stdout), text(&out.stderr));
    }
    let examined = run(r, home.path(), &["gate", "check", "--json"]);
    let body = json_of(&examined);
    assert_eq!(examined.status.code(), Some(0), "{body}");
    assert_eq!(body["data"]["clean"], true, "{body}");
    assert!(body["data"].get("unverifiable").is_none_or(|v| v.is_null() || v == false), "{body}");
    assert_eq!(body["meta"]["coverage"]["scenarios"], 1, "{body}");
}
