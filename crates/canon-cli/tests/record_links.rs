//! 0.30 `canon-0-30-record-links`: one test per scenario of the change, through the real binary.
//! Each test name starts with its scenario id (`review.add.101` → `review_add_101_…`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).env_remove("CANON_ROLE").output().expect("spawn canon")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com"]).args(args).current_dir(dir).output().expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

const PROVENANCE: &str = "# canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}";

fn feature(first_step: &str) -> String {
    format!(
        "Feature: cart add\n  {PROVENANCE}\n\n  Background:\n    Given a signed-in shopper\n\n  @cart.add.01\n  Scenario: One\n  {PROVENANCE}\n    {first_step}\n    And a second step\n\n  @cart.add.02\n  Scenario: Two\n  {PROVENANCE}\n    Given two\n\n  Rule: Limits\n    Background:\n      Given a limit of 3\n\n    @cart.add.03\n    Scenario: Three\n    {PROVENANCE}\n      When three\n      Then it is refused\n      But nothing is lost\n"
    )
}

fn feature_path(repo: &Path) -> PathBuf {
    repo.join(".canon/specs/kind=feature/area=cart/add.feature")
}

/// A git repo with a cart feature (three scenarios), a changes directory `kept`, and the index synced.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::write(repo.join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  finding: local\n  review: local\n").unwrap();
    std::fs::create_dir_all(feature_path(repo).parent().unwrap()).unwrap();
    std::fs::write(feature_path(repo), feature("Given one")).unwrap();
    std::fs::create_dir_all(repo.join(".canon/changes/kept")).unwrap();
    std::fs::write(repo.join(".canon/changes/kept/brief.md"), "# Kept\n").unwrap();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "x"]);
    assert!(canon(&["inventory", "sync"], repo).status.success());
    dir
}

fn review(repo: &Path, ids: &[&str]) -> Value {
    let mut args = vec!["review", "scenario"];
    args.extend_from_slice(ids);
    args.extend(["--upstream-ref", "r", "--actor-id", "rev", "--role", "reviewer", "--json"]);
    let out = canon(&args, repo);
    assert!(out.status.success(), "{}", text(&out));
    json(&out)
}

fn review_ids(doc: &Value) -> Vec<(String, String)> {
    doc["data"]["reviews"].as_array().unwrap().iter().map(|r| (r["scenarioId"].as_str().unwrap().to_string(), r["reviewId"].as_str().unwrap().to_string())).collect()
}

fn records(dir: PathBuf) -> Vec<(PathBuf, Value)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut all: Vec<(PathBuf, Value)> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).map(|p| (p.clone(), serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap())).collect();
    all.sort_by(|a, b| a.0.cmp(&b.0));
    all
}

fn ledger(repo: &Path, kind: &str) -> Vec<(PathBuf, Value)> {
    records(repo.join(format!(".canon/ledger/kind={kind}")))
}

fn archive(repo: &Path, kind: &str) -> Vec<(PathBuf, Value)> {
    records(repo.join(format!(".canon/archive/kind={kind}")))
}

fn add_finding(repo: &Path, change: &str, seq: &str, severity: &str, extra: &[&str]) -> Output {
    let mut args = vec!["finding", "add", "--change-id", change, "--round", "1", "--seq", seq, "--severity", severity, "--actor-id", "rev", "--summary", "s", "--promote"];
    args.extend_from_slice(extra);
    canon(&args, repo)
}

fn scenario_record(repo: &Path, id: &str) -> Value {
    ledger(repo, "scenario").into_iter().map(|(_, v)| v).find(|v| v["scenario_id"] == id).expect("scenario record")
}

const ABSENT_ID: &str = "01J9ZXQ4M2K7V8R3T5W6Y0N1PA";

#[test]
fn review_add_101_one_review_run_gives_every_scenario_it_writes_the_same_review_id() {
    let dir = fixture();
    let doc = review(dir.path(), &["cart.add.01", "cart.add.02", "cart.add.03"]);
    let ids = review_ids(&doc);
    assert_eq!(ids.len(), 3);
    assert!(ids.iter().all(|(_, id)| id == &ids[0].1 && id.len() == 26), "{ids:?}");
    let stored = ledger(dir.path(), "review");
    assert_eq!(stored.len(), 3);
    for (path, body) in &stored {
        assert_eq!(body["review_id"], ids[0].1.as_str());
        assert_eq!(body["schema"], 2);
        assert!(!path.file_name().unwrap().to_string_lossy().to_lowercase().contains(&ids[0].1.to_lowercase()), "the id is not in the name");
    }
}

#[test]
fn review_add_102_running_the_same_review_again_writes_nothing_and_shows_the_earlier_review_ids() {
    let dir = fixture();
    let repo = dir.path();
    let first = review_ids(&review(repo, &["cart.add.01", "cart.add.02", "cart.add.03"]));
    let again = review(repo, &["cart.add.01", "cart.add.02", "cart.add.03"]);
    assert_eq!(ledger(repo, "review").len(), 3, "no new record");
    let mut shown = review_ids(&again);
    shown.sort();
    let mut expected = first.clone();
    expected.sort();
    assert_eq!(shown, expected);
    assert!(again["data"]["reviews"].as_array().unwrap().iter().all(|r| r["unchanged"] == true));

    std::fs::write(feature_path(repo), feature("Given a changed one")).unwrap();
    assert!(canon(&["inventory", "sync"], repo).status.success());
    let mixed = review_ids(&review(repo, &["cart.add.01", "cart.add.02", "cart.add.03"]));
    let id_of = |ids: &[(String, String)], scenario: &str| ids.iter().find(|(s, _)| s == scenario).unwrap().1.clone();
    assert_ne!(id_of(&mixed, "cart.add.01"), id_of(&first, "cart.add.01"), "the changed scenario is written with a new id");
    assert_eq!(id_of(&mixed, "cart.add.02"), id_of(&first, "cart.add.02"));
    assert_eq!(id_of(&mixed, "cart.add.03"), id_of(&first, "cart.add.03"));
}

#[test]
fn ledger_migrate_16_a_review_keeps_its_review_id_through_canon_migrate() {
    let dir = fixture();
    let repo = dir.path();
    let id = review_ids(&review(repo, &["cart.add.01"]))[0].1.clone();
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "reviewed"]);
    let _ = canon(&["migrate", "--json"], repo);
    let _ = canon(&["migrate", "--commit", "--json"], repo);
    let all: Vec<Value> = ledger(repo, "review").into_iter().chain(archive(repo, "review")).map(|(_, v)| v).collect();
    assert!(all.iter().any(|v| v["review_id"] == id.as_str()), "{all:?}");
}

#[test]
fn finding_add_38_a_finding_added_with_a_review_id_points_at_that_review_and_a_query_shows_the_id() {
    let dir = fixture();
    let repo = dir.path();
    let id = review_ids(&review(repo, &["cart.add.01"]))[0].1.clone();
    assert!(add_finding(repo, "kept", "1", "blocker", &["--introduced-unknown", "--review-id", &id]).status.success());
    assert!(add_finding(repo, "kept", "2", "note", &[]).status.success());
    let found = ledger(repo, "finding");
    assert_eq!(found.iter().filter(|(_, v)| v["review_id"] == id.as_str()).count(), 1);
    assert!(found.iter().any(|(_, v)| v["seq"] == 2 && v.get("review_id").is_none()));
    let doc = json(&canon(&["query", "--kind", "finding", "--json"], repo));
    assert!(doc["data"]["records"].as_array().unwrap().iter().any(|r| r["review_id"] == id.as_str()), "{doc}");
}

#[test]
fn finding_add_39_a_review_id_that_no_review_has_is_refused_and_the_ledger_is_unchanged() {
    let dir = fixture();
    let repo = dir.path();
    review(repo, &["cart.add.01"]);
    let out = add_finding(repo, "kept", "1", "note", &["--review-id", ABSENT_ID]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out).contains("FINDING_REVIEW_NOT_FOUND") && text(&out).contains(ABSENT_ID), "{}", text(&out));
    assert!(ledger(repo, "finding").is_empty() && !repo.join(".canon/ledger/_staging/kind=finding").exists());
    let bad = add_finding(repo, "kept", "1", "note", &["--review-id", "nope"]);
    assert_eq!(bad.status.code(), Some(2));
}

#[test]
fn finding_add_40_the_id_of_a_review_that_moved_to_the_archive_is_still_accepted() {
    let dir = fixture();
    let repo = dir.path();
    let old = review_ids(&review(repo, &["cart.add.01"]))[0].1.clone();
    std::fs::write(feature_path(repo), feature("Given a changed one")).unwrap();
    assert!(canon(&["inventory", "sync"], repo).status.success());
    review(repo, &["cart.add.01"]);
    assert!(archive(repo, "review").iter().any(|(_, v)| v["review_id"] == old.as_str()), "the earlier review is archived");
    assert!(!ledger(repo, "review").iter().any(|(_, v)| v["review_id"] == old.as_str()));
    let out = add_finding(repo, "kept", "1", "note", &["--review-id", &old]);
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn finding_close_88_closing_or_reopening_a_finding_copies_its_review_id_and_its_unknown_mark() {
    let dir = fixture();
    let repo = dir.path();
    let id = review_ids(&review(repo, &["cart.add.01"]))[0].1.clone();
    assert!(add_finding(repo, "kept", "1", "blocker", &["--introduced-unknown", "--review-id", &id]).status.success());
    let close = |disposition: &str| canon(&["finding", "close", "kept", "--round", "1", "--seq", "1", "--disposition", disposition, "--promote"], repo);
    assert!(close("fixed").status.success());
    assert!(close("open").status.success());
    let versions: Vec<Value> = ledger(repo, "finding").into_iter().chain(archive(repo, "finding")).map(|(_, v)| v).collect();
    assert_eq!(versions.len(), 3, "{versions:?}");
    assert!(versions.iter().all(|v| v["review_id"] == id.as_str() && v["introduced_unknown"] == true), "{versions:?}");
}

#[test]
fn finding_add_41_a_new_blocker_or_should_fix_with_no_introducing_commit_and_no_unknown_mark_is_refused() {
    let dir = fixture();
    let repo = dir.path();
    for (seq, severity) in [("1", "blocker"), ("2", "should-fix")] {
        let out = add_finding(repo, "kept", seq, severity, &[]);
        assert_eq!(out.status.code(), Some(2));
        let message = text(&out);
        assert!(message.contains("FINDING_INTRODUCED_REQUIRED") && message.contains("--introduced-rev") && message.contains("--introduced-unknown"), "{message}");
    }
    assert!(ledger(repo, "finding").is_empty() && !repo.join(".canon/ledger/_staging/kind=finding").exists());
}

#[test]
fn finding_add_42_a_new_defect_finding_is_recorded_with_an_introducing_commit_or_the_unknown_mark_and_a_note_needs_neither() {
    let dir = fixture();
    let repo = dir.path();
    assert!(add_finding(repo, "kept", "1", "blocker", &["--introduced-rev", "HEAD"]).status.success());
    assert!(add_finding(repo, "kept", "2", "should-fix", &["--introduced-unknown"]).status.success());
    assert!(add_finding(repo, "kept", "3", "note", &[]).status.success());
    let by_seq = |seq: u64| ledger(repo, "finding").into_iter().map(|(_, v)| v).find(|v| v["seq"] == seq).unwrap();
    let (blocker, should_fix, note) = (by_seq(1), by_seq(2), by_seq(3));
    assert!(blocker["introduced_by"].is_string() && blocker.get("introduced_unknown").is_none());
    assert!(should_fix["introduced_unknown"] == true && should_fix.get("introduced_by").is_none());
    assert!(note.get("introduced_by").is_none() && note.get("introduced_unknown").is_none());
}

#[test]
fn finding_add_43_an_introducing_commit_together_with_the_unknown_mark_is_refused() {
    let dir = fixture();
    let out = add_finding(dir.path(), "kept", "1", "blocker", &["--introduced-rev", "HEAD", "--introduced-unknown"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out).contains("FINDING_INVALID_ARGUMENT") && text(&out).contains("exclusive"), "{}", text(&out));
    assert!(ledger(dir.path(), "finding").is_empty());
}

#[test]
fn finding_close_89_a_blocker_written_before_0_30_with_neither_field_still_closes_and_reopens() {
    use canon_model::RawRecord;
    use canon_store::git_tier::GitTier;
    use canon_store::tier::{RawWrite, Tier};
    let dir = fixture();
    let repo = dir.path();
    let body = serde_json::json!({
        "schema": 1, "kind": "finding", "at": "2026-07-01T00:00:00Z", "actor": {"agent_id": "old", "role": "reviewer"},
        "change_id": "kept", "round": 1, "seq": 1, "severity": "blocker", "disposition": "open", "reviewer": "old", "summary": "from 0.29"
    });
    GitTier::new(repo.join(".canon/ledger")).write(&RawWrite(RawRecord(body))).unwrap();
    for disposition in ["fixed", "open"] {
        let out = canon(&["finding", "close", "kept", "--round", "1", "--seq", "1", "--disposition", disposition, "--promote"], repo);
        assert!(out.status.success(), "{}", text(&out));
    }
    let versions: Vec<Value> = ledger(repo, "finding").into_iter().chain(archive(repo, "finding")).map(|(_, v)| v).collect();
    assert_eq!(versions.len(), 3);
    assert!(versions.iter().all(|v| v.get("introduced_by").is_none() && v.get("introduced_unknown").is_none()));
}

#[test]
fn inventory_sync_53_a_scenario_index_record_holds_the_steps_in_source_order_with_inherited_ones_marked() {
    let dir = fixture();
    let steps = scenario_record(dir.path(), "cart.add.03")["steps"].clone();
    let shown: Vec<(String, String, Option<String>)> = steps.as_array().unwrap().iter().map(|s| (s["keyword"].as_str().unwrap().into(), s["text"].as_str().unwrap().into(), s["inherited"].as_str().map(str::to_string))).collect();
    let step = |k: &str, t: &str, i: Option<&str>| (k.to_string(), t.to_string(), i.map(str::to_string));
    assert_eq!(
        shown,
        vec![
            step("Given", "a signed-in shopper", Some("background")),
            step("Given", "a limit of 3", Some("rule")),
            step("When", "three", None),
            step("Then", "it is refused", None),
            step("But", "nothing is lost", None),
        ]
    );
    assert!(scenario_record(dir.path(), "cart.add.01")["steps"].as_array().unwrap().iter().any(|s| s["keyword"] == "And"));
}

/// Rewrite every scenario record of the ledger as 0.29 wrote it: no `steps`.
fn downgrade_index_to_0_29(repo: &Path) {
    use canon_model::RawRecord;
    use canon_store::git_tier::GitTier;
    use canon_store::tier::{RawWrite, Tier};
    let old = ledger(repo, "scenario");
    for (path, mut body) in old {
        std::fs::remove_file(path).unwrap();
        body.as_object_mut().unwrap().remove("steps");
        GitTier::new(repo.join(".canon/ledger")).write(&RawWrite(RawRecord(body))).unwrap();
    }
}

#[test]
fn inventory_sync_54_a_0_29_index_record_without_steps_is_rewritten_with_them_on_the_next_sync() {
    let dir = fixture();
    let repo = dir.path();
    downgrade_index_to_0_29(repo);
    assert!(ledger(repo, "scenario").iter().all(|(_, v)| v.get("steps").is_none()));
    let out = canon(&["inventory", "sync", "--json"], repo);
    assert!(out.status.success(), "{}", text(&out));
    assert!(ledger(repo, "scenario").iter().all(|(_, v)| v["steps"].as_array().is_some_and(|s| !s.is_empty())), "every scenario has steps now");
    let again = json(&canon(&["inventory", "sync", "--json"], repo));
    assert_eq!(again["meta"]["unchanged"], true, "a second sync writes nothing: {again}");
}

#[test]
fn inventory_sync_55_an_edited_step_replaces_the_record_and_the_old_record_goes_to_the_archive() {
    let dir = fixture();
    let repo = dir.path();
    std::fs::write(feature_path(repo), feature("Given an edited step")).unwrap();
    assert!(canon(&["inventory", "sync"], repo).status.success());
    let now = scenario_record(repo, "cart.add.01");
    assert!(now["steps"].as_array().unwrap().iter().any(|s| s["text"] == "an edited step"));
    assert!(archive(repo, "scenario").iter().any(|(_, v)| v["scenario_id"] == "cart.add.01" && v["steps"].to_string().contains("\"one\"")), "the old record is archived");
}

fn query_findings(repo: &Path) -> Value {
    let out = canon(&["query", "--kind", "finding", "--json"], repo);
    assert!(out.status.success(), "{}", text(&out));
    json(&out)
}

#[test]
fn query_read_83_a_query_marks_a_reference_to_a_source_that_is_not_committed_and_counts_them() {
    let dir = fixture();
    let repo = dir.path();
    assert!(add_finding(repo, "kept", "1", "note", &[]).status.success());
    assert!(add_finding(repo, "gone-change", "1", "note", &[]).status.success());
    let doc = query_findings(repo);
    let dangling = &doc["data"]["dangling"];
    assert_eq!(dangling["count"], 1, "{doc}");
    assert_eq!(dangling["byReference"]["finding.change_id"], 1);
    assert_eq!(dangling["references"][0]["id"], "gone-change");
    assert_eq!(dangling["references"][0]["target"], "change");
    let human = text(&canon(&["query", "--kind", "finding"], repo));
    assert!(human.contains("has no committed source") && human.contains("gone-change"), "{human}");
}

#[test]
fn query_read_84_a_query_whose_references_all_resolve_marks_nothing_and_counts_zero() {
    let dir = fixture();
    let repo = dir.path();
    assert!(add_finding(repo, "kept", "1", "note", &[]).status.success());
    let doc = query_findings(repo);
    assert_eq!(doc["data"]["dangling"]["count"], 0, "{doc}");
    assert_eq!(doc["data"]["dangling"]["references"].as_array().unwrap().len(), 0);
    assert!(!text(&canon(&["query", "--kind", "finding"], repo)).contains("no committed source"));
}

fn status_dangling(repo: &Path) -> Value {
    let out = canon(&["status", "--json"], repo);
    json(&out)["data"]["danglingReferences"].clone()
}

#[test]
fn context_status_84_status_counts_references_to_missing_sources_per_kind_and_gives_the_same_json_on_every_checkout_of_one_commit() {
    let dir = fixture();
    let repo = dir.path();
    assert!(add_finding(repo, "gone-change", "1", "note", &[]).status.success());
    assert!(add_finding(repo, "kept", "2", "note", &["--review-id", &review_ids(&review(repo, &["cart.add.01"]))[0].1]).status.success());
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "findings"]);
    let here = status_dangling(repo);
    assert_eq!(here["count"], 1, "{here}");
    assert_eq!(here["byReference"]["finding.change_id"], 1);
    let clone = tempfile::tempdir().unwrap();
    git(clone.path().parent().unwrap(), &["clone", "-q", repo.to_str().unwrap(), clone.path().to_str().unwrap()]);
    assert!(canon(&["inventory", "sync"], clone.path()).status.success());
    assert_eq!(status_dangling(clone.path()), here, "a fresh checkout of the commit reports the same");
    let human = text(&canon(&["status"], repo));
    assert!(human.contains("dangling references: 1"), "{human}");
}

#[test]
fn context_status_85_a_gitignored_derived_record_is_not_a_source_and_gate_check_is_unchanged() {
    let dir = fixture();
    let repo = dir.path();
    std::fs::write(repo.join(".gitignore"), ".canon/ledger/kind=change/\n").unwrap();
    std::fs::create_dir_all(repo.join(".canon/ledger/kind=change")).unwrap();
    std::fs::write(repo.join(".canon/ledger/kind=change/derived_00000000.json"), "{\"kind\":\"change\",\"change_id\":\"derived\"}").unwrap();
    assert!(add_finding(repo, "derived", "1", "note", &[]).status.success());
    assert_eq!(status_dangling(repo)["byReference"]["finding.change_id"], 1);
    assert_eq!(query_findings(repo)["data"]["dangling"]["count"], 1);
    let gate = canon(&["gate", "check", "--json"], repo);
    assert!(!text(&gate).contains("dangling"), "gate check does not read references");
}

fn init_repo(yaml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), yaml).unwrap();
    dir
}

const WITH_HOT: &str = "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\n";

#[test]
fn authoring_init_101_init_check_shows_where_the_hot_store_is() {
    let dir = init_repo(WITH_HOT);
    let out = canon(&["init", "--check", "--json"], dir.path());
    let shown = text(&out);
    assert!(shown.contains("hot-store: .canon/hot.db (sqlite file of this worktree"), "{shown}");
    let doc = json(&out);
    assert!(doc["data"]["sections"].as_array().unwrap().iter().any(|s| s["section"] == "hot-store" && s["detail"].as_str().unwrap().contains(".canon/hot.db")), "{doc}");
}

#[test]
fn authoring_init_102_init_check_says_so_when_no_hot_store_is_configured_and_names_a_hot_store_it_cannot_open() {
    let none = init_repo("tiers:\n  local: { backend: git, root: .canon/ledger }\n");
    assert!(text(&canon(&["init", "--check"], none.path())).contains("[not configured] hot-store"));
    let broken = init_repo(WITH_HOT);
    std::fs::create_dir_all(broken.path().join(".canon")).unwrap();
    std::fs::write(broken.path().join(".canon/hot.db"), "this is not a database").unwrap();
    let out = canon(&["init", "--check"], broken.path());
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("[FAIL] hot-store: cannot open .canon/hot.db"), "{}", text(&out));
}

#[test]
fn ingest_sessions_15_a_command_that_writes_hot_records_prints_the_hot_store_location() {
    let dir = init_repo(WITH_HOT);
    let home = tempfile::tempdir().unwrap();
    let human = canon(&["ingest", "sessions", "--home", home.path().to_str().unwrap()], dir.path());
    assert!(text(&human).contains("hot store: .canon/hot.db"), "{}", text(&human));
    let doc = json(&canon(&["ingest", "sessions", "--home", home.path().to_str().unwrap(), "--json"], dir.path()));
    assert_eq!(doc["data"]["hotStore"], ".canon/hot.db", "{doc}");
}
