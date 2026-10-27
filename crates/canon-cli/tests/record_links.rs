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
    args.extend(["--upstream-ref", "r", "--actor", "rev", "--role", "reviewer", "--json"]);
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
    let mut args = vec!["finding", "add", "--change", change, "--round", "1", "--seq", seq, "--severity", severity, "--actor", "rev", "--summary", "s", "--promote"];
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
    assert!(again["data"]["reviews"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["unchanged"] == true));

    std::fs::write(feature_path(repo), feature("Given a changed one")).unwrap();
    assert!(canon(&["inventory", "sync"], repo).status.success());
    let mixed = review_ids(&review(repo, &["cart.add.01", "cart.add.02", "cart.add.03"]));
    let id_of = |ids: &[(String, String)], scenario: &str| ids.iter().find(|(s, _)| s == scenario).unwrap().1.clone();
    assert_ne!(id_of(&mixed, "cart.add.01"), id_of(&first, "cart.add.01"), "the changed scenario is written with a new id");
    assert_eq!(id_of(&mixed, "cart.add.02"), id_of(&first, "cart.add.02"));
    assert_eq!(id_of(&mixed, "cart.add.03"), id_of(&first, "cart.add.03"));
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The 0.29 fixture (0.29-named record files, pre-0.23 reviews) where reviews 01 and 02 carry a
/// review id in their body and 04 has none; `canon migrate --commit` rewrites 02's pins.
#[test]
fn ledger_migrate_16_a_review_keeps_its_review_id_through_canon_migrate() {
    const ID_01: &str = "01J9ZXQ4M2K7V8R3T5W6Y0N1PB";
    const ID_02: &str = "01J9ZXQ4M2K7V8R3T5W6Y0N1PC";
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger-0.29"), repo);
    std::fs::rename(repo.join("gitignore"), repo.join(".gitignore")).unwrap();
    let area = repo.join(".canon/ledger/kind=review/area=auth");
    for (scenario, id) in [("auth.login.01", ID_01), ("auth.login.02", ID_02)] {
        let file = std::fs::read_dir(&area).unwrap().flatten().map(|e| e.path()).find(|p| p.file_name().unwrap().to_string_lossy().contains(scenario)).unwrap();
        let mut body: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        body["review_id"] = id.into();
        std::fs::write(&file, serde_json::to_vec_pretty(&body).unwrap()).unwrap();
    }
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture"]);

    let preview = json(&canon(&["migrate", "--json"], repo));
    let digest = preview["data"]["planDigest"].as_str().expect("a plan digest").to_string();
    let done = canon(&["migrate", "--commit", "--plan-digest", &digest, "--json"], repo);
    assert!(done.status.success(), "{}", text(&done));

    let reviews: Vec<Value> = ledger(repo, "review").into_iter().chain(archive(repo, "review")).map(|(_, v)| v).collect();
    let of = |scenario: &str| reviews.iter().find(|v| v["scenario_id"] == scenario).unwrap_or_else(|| panic!("review of {scenario}: {reviews:?}"));
    assert_eq!(of("auth.login.01")["review_id"], ID_01);
    assert_eq!(of("auth.login.02")["review_id"], ID_02);
    assert!(of("auth.login.02")["pins"]["checks"].is_array(), "review 02 was rewritten by the migrate: {}", of("auth.login.02"));
    assert!(of("auth.login.04").get("review_id").is_none(), "the review written before 0.30 still has none");
}

#[test]
fn finding_add_38_a_finding_added_with_a_review_id_points_at_that_review_and_a_query_shows_the_id() {
    let dir = fixture();
    let repo = dir.path();
    let id = review_ids(&review(repo, &["cart.add.01"]))[0].1.clone();
    assert!(add_finding(
        repo,
        "kept",
        "1",
        "blocker",
        &["--introduced-unknown", "--review", &id]
    )
    .status
    .success());
    assert!(add_finding(repo, "kept", "2", "note", &[]).status.success());
    let found = ledger(repo, "finding");
    assert_eq!(
        found
            .iter()
            .filter(|(_, v)| v["review_id"] == id.as_str())
            .count(),
        1
    );
    assert!(found
        .iter()
        .any(|(_, v)| v["seq"] == 2 && v.get("review_id").is_none()));
    let doc = json(&canon(&["query", "--kind", "finding", "--json"], repo));
    assert!(doc["data"]["records"].as_array().unwrap().iter().any(|r| r["review_id"] == id.as_str()), "{doc}");
}

#[test]
fn finding_add_39_a_review_id_that_no_review_has_is_refused_and_the_ledger_is_unchanged() {
    let dir = fixture();
    let repo = dir.path();
    review(repo, &["cart.add.01"]);
    let out = add_finding(repo, "kept", "1", "note", &["--review", ABSENT_ID]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out).contains("FINDING_REVIEW_NOT_FOUND") && text(&out).contains(ABSENT_ID), "{}", text(&out));
    assert!(ledger(repo, "finding").is_empty() && !repo.join(".canon/ledger/_staging/kind=finding").exists());
    let bad = add_finding(repo, "kept", "1", "note", &["--review", "nope"]);
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
    assert!(
        archive(repo, "review")
            .iter()
            .any(|(_, v)| v["review_id"] == old.as_str()),
        "the earlier review is archived"
    );
    assert!(!ledger(repo, "review")
        .iter()
        .any(|(_, v)| v["review_id"] == old.as_str()));
    let out = add_finding(repo, "kept", "1", "note", &["--review", &old]);
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn finding_close_88_closing_or_reopening_a_finding_copies_its_review_id_and_its_unknown_mark() {
    let dir = fixture();
    let repo = dir.path();
    let id = review_ids(&review(repo, &["cart.add.01"]))[0].1.clone();
    assert!(add_finding(
        repo,
        "kept",
        "1",
        "blocker",
        &["--introduced-unknown", "--review", &id]
    )
    .status
    .success());
    let close = |disposition: &str| {
        canon(
            &[
                "finding",
                "close",
                "kept",
                "--round",
                "1",
                "--seq",
                "1",
                "--disposition",
                disposition,
                "--promote",
            ],
            repo,
        )
    };
    assert!(close("fixed").status.success());
    assert!(close("open").status.success());
    let versions: Vec<Value> = ledger(repo, "finding").into_iter().chain(archive(repo, "finding")).map(|(_, v)| v).collect();
    assert_eq!(versions.len(), 3, "{versions:?}");
    assert!(versions.iter().all(|v| v["review_id"] == id.as_str() && v["introduced_unknown"] == true), "{versions:?}");
}

#[test]
fn finding_add_41_a_new_blocker_or_should_fix_with_no_introducing_commit_and_no_unknown_mark_is_refused(
) {
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
fn finding_add_42_a_new_defect_finding_is_recorded_with_an_introducing_commit_or_the_unknown_mark_and_a_note_needs_neither(
) {
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
    assert!(versions
        .iter()
        .all(|v| v.get("introduced_by").is_none() && v.get("introduced_unknown").is_none()));
}

#[test]
fn inventory_sync_53_a_scenario_index_record_holds_the_steps_in_source_order_with_inherited_ones_marked(
) {
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
    assert!(scenario_record(dir.path(), "cart.add.01")["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["keyword"] == "And"));

    // `scenario_digest` is what it was before steps were indexed: a 0.29 record (no `steps`) re-synced keeps its digest.
    let digests = |repo: &Path| -> Vec<(String, Value)> {
        ledger(repo, "scenario")
            .into_iter()
            .map(|(_, v)| {
                (
                    v["scenario_id"].as_str().unwrap().to_string(),
                    v["scenario_digest"].clone(),
                )
            })
            .collect()
    };
    let before = digests(dir.path());
    assert!(before.iter().all(|(_, d)| d.is_string()), "{before:?}");
    downgrade_index_to_0_29(dir.path());
    assert!(canon(&["inventory", "sync"], dir.path()).status.success());
    assert!(ledger(dir.path(), "scenario")
        .iter()
        .all(|(_, v)| v.get("steps").is_some()));
    let mut after = digests(dir.path());
    let mut before = before;
    before.sort_by(|a, b| a.0.cmp(&b.0));
    after.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(after, before, "the scenario digest changed with the steps");
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
    assert!(ledger(repo, "scenario")
        .iter()
        .all(|(_, v)| v.get("steps").is_none()));
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
    assert!(now["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["text"] == "an edited step"));
    assert!(
        archive(repo, "scenario")
            .iter()
            .any(|(_, v)| v["scenario_id"] == "cart.add.01"
                && v["steps"].to_string().contains("\"one\"")),
        "the old record is archived"
    );
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
    assert!(add_finding(repo, "gone-change", "1", "note", &[])
        .status
        .success());
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

/// Rewrite every record of `kind` in the ledger with `edit` applied to its body, as a new file named by the new body.
fn rewrite_ledger(repo: &Path, kind: &str, edit: impl Fn(&mut Value)) {
    use canon_model::RawRecord;
    use canon_store::git_tier::GitTier;
    use canon_store::tier::{RawWrite, Tier};
    for (path, mut body) in ledger(repo, kind) {
        std::fs::remove_file(path).unwrap();
        edit(&mut body);
        GitTier::new(repo.join(".canon/ledger"))
            .write(&RawWrite(RawRecord(body)))
            .unwrap();
    }
}

#[test]
fn context_status_84_status_counts_references_to_missing_sources_per_kind_and_gives_the_same_json_on_every_checkout_of_one_commit(
) {
    let dir = fixture();
    let repo = dir.path();
    std::fs::write(repo.join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  finding: local\n  review: local\n  evidence: local\n  subject: local\n").unwrap();
    // finding.change_id and finding.review_id: a finding of a change nobody committed, citing a review whose file is then gone.
    let review_id = review_ids(&review(repo, &["cart.add.01"]))[0].1.clone();
    assert!(add_finding(
        repo,
        "gone-change",
        "1",
        "note",
        &["--review", &review_id]
    )
    .status
    .success());
    assert!(add_finding(repo, "kept", "2", "note", &[]).status.success());
    rewrite_ledger(repo, "review", |body| {
        body["review_id"] = "01J9ZXQ4M2K7V8R3T5W6Y0N1PZ".into();
    });
    // evidence.task_id and evidence.scenario_id.
    let out = canon(
        &[
            "evidence",
            "add",
            "--scenario",
            "cart.add.01",
            "--class",
            "test-run",
            "--ref",
            "x",
            "--role",
            "implementer",
            "--actor",
            "a",
            "--promote",
        ],
        repo,
    );
    assert!(out.status.success(), "{}", text(&out));
    rewrite_ledger(repo, "evidence", |body| {
        body["task_id"] = "gone-task#1".into();
        body["scenario_id"] = "gone.scn.01".into();
    });
    // subject.change_ids.
    let out = canon(
        &[
            "subject",
            "new",
            "demo",
            "--domain",
            "d",
            "--title",
            "T",
            "--actor",
            "a",
        ],
        repo,
    );
    assert!(out.status.success(), "{}", text(&out));
    rewrite_ledger(repo, "subject", |body| {
        body["change_ids"] = serde_json::json!(["gone-change"]);
    });
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "records"]);

    let here = status_dangling(repo);
    assert_eq!(here["count"], 5, "{here}");
    for reference in [
        "finding.change_id",
        "finding.review_id",
        "evidence.task_id",
        "evidence.scenario_id",
        "subject.change_ids",
    ] {
        assert_eq!(here["byReference"][reference], 1, "{reference}: {here}");
    }
    let clone = tempfile::tempdir().unwrap();
    git(clone.path().parent().unwrap(), &["clone", "-q", repo.to_str().unwrap(), clone.path().to_str().unwrap()]);
    assert!(canon(&["inventory", "sync"], clone.path()).status.success());
    assert_eq!(status_dangling(clone.path()), here, "a fresh checkout of the commit reports the same");
    let human = text(&canon(&["status"], repo));
    assert!(human.contains("dangling references: 5"), "{human}");
}

#[test]
fn context_status_85_a_gitignored_derived_record_is_not_a_source_and_gate_check_is_unchanged() {
    let dir = fixture();
    let repo = dir.path();
    std::fs::write(repo.join(".gitignore"), ".canon/ledger/kind=change/\n").unwrap();
    std::fs::create_dir_all(repo.join(".canon/ledger/kind=change")).unwrap();
    std::fs::write(
        repo.join(".canon/ledger/kind=change/derived_00000000.json"),
        "{\"kind\":\"change\",\"change_id\":\"derived\"}",
    )
    .unwrap();
    // A review written before 0.23: it pins evidence record names that no longer exist.
    {
        use canon_model::RawRecord;
        use canon_store::git_tier::GitTier;
        use canon_store::tier::{RawWrite, Tier};
        let old = serde_json::json!({
            "actor": {"agent_id": "rev-old", "role": "reviewer"}, "at": "2026-01-01T00:00:00.000000Z", "kind": "review", "pin": "1111111111111111111111111111111111111111",
            "pins": {"evidence": [{"digest": "f778690a0216", "key": "cart.add.01"}], "scenario_digest": "b35933cccfd20c6521a722f5b422fb3ae5756c75042527c8474625fd6a2fbe51"},
            "project_id": "root", "provenance_ref": {"original_spec_ref": "specs/old.feature"}, "reviewer": "rev-old", "scenario_id": "cart.add.01", "schema": 1
        });
        GitTier::new(repo.join(".canon/ledger"))
            .write(&RawWrite(RawRecord(old)))
            .unwrap()
    };
    // The same finding, once pointing at a change that resolves and once at the one that has only a derived record.
    assert!(add_finding(repo, "kept", "1", "note", &[]).status.success());
    assert_eq!(status_dangling(repo)["count"], 0);
    let resolving = canon(&["gate", "check", "--json"], repo);
    rewrite_ledger(repo, "finding", |body| body["change_id"] = "derived".into());
    assert_eq!(status_dangling(repo)["byReference"]["finding.change_id"], 1);
    assert_eq!(
        status_dangling(repo)["count"],
        1,
        "the old review's pins are not checked"
    );
    assert_eq!(query_findings(repo)["data"]["dangling"]["count"], 1);
    let dangling = canon(&["gate", "check", "--json"], repo);
    assert!(
        !text(&dangling).contains("dangling"),
        "gate check does not read references"
    );
    assert_eq!(
        (dangling.status.code(), &dangling.stdout),
        (resolving.status.code(), &resolving.stdout),
        "gate check differs with a dangling reference"
    );
}

fn init_repo(yaml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), yaml).unwrap();
    dir
}

const ROUTED_HOT: &str = "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\nrouting:\n  session: hot\n  run: hot\n  event: hot\n  change: hot\n  task: hot\n  trajectory: hot\n";

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
fn authoring_init_102_init_check_says_so_when_no_hot_store_is_configured_and_names_a_hot_store_it_cannot_open(
) {
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
    let dir = init_repo(ROUTED_HOT);
    let home = tempfile::tempdir().unwrap();
    let transcripts = home.path().join(".claude/projects/-work-app");
    std::fs::create_dir_all(&transcripts).unwrap();
    let turn = |kind: &str, at: &str, message: Value| serde_json::json!({"type": kind, "sessionId": "s-15", "timestamp": at, "cwd": "/work/app", "message": message}).to_string();
    let lines = [
        turn("user", "2026-10-26T10:00:00Z", serde_json::json!({"role": "user", "content": "go"})),
        turn("assistant", "2026-10-26T10:00:05Z", serde_json::json!({"role": "assistant", "model": "m", "content": [{"type": "text", "text": "done"}]})),
    ];
    std::fs::write(transcripts.join("s-15.jsonl"), lines.join("\n") + "\n").unwrap();
    let home_arg = home.path().to_str().unwrap();

    let human = canon(&["ingest", "sessions", "--home", home_arg, "--all-workspaces"], dir.path());
    assert!(text(&human).contains("hot store: .canon/hot.db"), "{}", text(&human));
    let doc = json(&canon(&["ingest", "sessions", "--home", home_arg, "--all-workspaces", "--json"], dir.path()));
    assert_eq!(doc["data"]["hotStore"], ".canon/hot.db", "{doc}");
    let written = canon(&["query", "--kind", "session", "--json"], dir.path());
    assert!(json(&written)["data"]["records"].as_array().is_some_and(|r| !r.is_empty()), "the session was written: {}", text(&written));
}

#[test]
fn ingest_sessions_16_the_other_hot_writers_print_the_hot_store_location_too() {
    let dir = init_repo(ROUTED_HOT);
    let repo = dir.path();
    let plans = canon(&["ingest", "plans", "--json"], repo);
    assert_eq!(json(&plans)["data"]["hotStore"], ".canon/hot.db", "{}", text(&plans));
    let records = canon(&["ingest", "records"], repo);
    assert!(text(&records).contains("hot store: .canon/hot.db"), "{}", text(&records));
    let begin = canon(&["dispatch", "begin", "--role", "implementer", "--regime", "implementer/app/api/abcdef012345", "--actor", "a", "--json"], repo);
    assert!(text(&begin).contains(".canon/hot.db"), "{}", text(&begin));
}

#[test]
fn ingest_sessions_16b_subject_and_change_writes_to_the_hot_store_print_its_location_too() {
    let hot = "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\nrouting:\n  subject: hot\n  change: hot\n";
    let dir = init_repo(hot);
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    let new = canon(
        &[
            "subject",
            "new",
            "demo",
            "--domain",
            "d",
            "--title",
            "T",
            "--actor",
            "a",
        ],
        repo,
    );
    assert!(
        new.status.success() && text(&new).contains("hot store: .canon/hot.db"),
        "{}",
        text(&new)
    );
    let new_json = json(&canon(
        &[
            "subject",
            "new",
            "other",
            "--domain",
            "d",
            "--title",
            "T",
            "--actor",
            "a",
            "--json",
        ],
        repo,
    ));
    assert_eq!(new_json["meta"]["hotStore"], ".canon/hot.db", "{new_json}");
    let change = canon(
        &[
            "change",
            "new",
            "add-it",
            "--subject",
            "demo",
            "--title",
            "Add it",
            "--json",
        ],
        repo,
    );
    assert_eq!(
        json(&change)["data"]["hotStore"],
        ".canon/hot.db",
        "{}",
        text(&change)
    );
    let human = canon(
        &[
            "change",
            "new",
            "add-two",
            "--subject",
            "demo",
            "--title",
            "Add two",
        ],
        repo,
    );
    assert!(
        text(&human).contains("hot store: .canon/hot.db"),
        "{}",
        text(&human)
    );
    let adopt = canon(
        &["subject", "adopt", "other", "--change", "add-it"],
        repo,
    );
    assert!(
        text(&adopt).contains("hot store: .canon/hot.db"),
        "{}",
        text(&adopt)
    );
    let unadopt = canon(
        &[
            "subject",
            "unadopt",
            "other",
            "--change",
            "add-it",
            "--json",
        ],
        repo,
    );
    assert_eq!(
        json(&unadopt)["meta"]["hotStore"],
        ".canon/hot.db",
        "{}",
        text(&unadopt)
    );
    let moved = canon(&["subject", "move", "demo", "--to", "specced"], repo);
    assert!(
        text(&moved).contains("hot store: .canon/hot.db"),
        "{}",
        text(&moved)
    );
    // Nothing written, nothing said: a preview and an unchanged repeat.
    let preview = canon(
        &[
            "subject",
            "adopt",
            "other",
            "--change",
            "add-it",
            "--dry-run",
        ],
        repo,
    );
    assert!(!text(&preview).contains("hot store:"), "{}", text(&preview));
    let again = canon(
        &["subject", "unadopt", "other", "--change", "add-it"],
        repo,
    );
    assert!(!text(&again).contains("hot store:"), "{}", text(&again));
    // Routed to the ledger: no hot store line.
    let local = init_repo("tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\nrouting:\n  subject: local\n");
    let out = canon(
        &[
            "subject",
            "new",
            "demo",
            "--domain",
            "d",
            "--title",
            "T",
            "--actor",
            "a",
        ],
        local.path(),
    );
    assert!(
        out.status.success() && !text(&out).contains("hot store:"),
        "{}",
        text(&out)
    );
}

/// Every `canon finding add` command written by hand in the skill guides (the generated blocks are checked by `docs_generated`)
/// that raises a blocker or should-fix names the introducing commit, or says it is unknown: `finding add` refuses it otherwise.
#[test]
fn hand_written_finding_add_blocker_and_should_fix_examples_name_the_introducing_commit() {
    fn markdown(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                markdown(&path, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![root.join("README.md")];
    markdown(&root.join("canon/skills"), &mut files);
    let (mut seen, mut problems) = (0, Vec::new());
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let mut lines = text.lines().enumerate();
        while let Some((number, line)) = lines.next() {
            let Some(start) = line
                .find("canon finding add ")
                .filter(|_| line.trim_start().starts_with("canon finding add "))
            else {
                continue;
            };
            let mut command = line[start..].to_string();
            while command.ends_with('\\') {
                let Some((_, next)) = lines.next() else { break };
                command.pop();
                command.push_str(next.trim());
            }
            seen += 1;
            // Flags: a severity of blocker or should-fix; a template (`blocker|should-fix|note`) is not an example.
            let words: Vec<&str> = command.split_whitespace().collect();
            let flagged = words
                .windows(2)
                .any(|w| w[0] == "--severity" && matches!(w[1], "blocker" | "should-fix"));
            // `--input` documents: every object with such a severity.
            let documents = command.split("{\"").skip(1).filter(|object| {
                object.contains("\"severity\":\"blocker\"")
                    || object.contains("\"severity\":\"should-fix\"")
            });
            let unnamed_documents = documents
                .filter(|object| {
                    !object.contains("\"introducedRev\"")
                        && !object.contains("\"introducedUnknown\"")
                })
                .count();
            let unnamed_flags = flagged
                && !command.contains("--introduced-rev")
                && !command.contains("--introduced-unknown");
            if unnamed_flags || unnamed_documents > 0 {
                problems.push(format!("{}:{}: {command}", file.display(), number + 1));
            }
        }
    }
    assert!(seen >= 5, "found only {seen} `canon finding add` examples");
    assert!(problems.is_empty(), "these examples raise a blocker or should-fix without --introduced-rev or --introduced-unknown:\n{}", problems.join("\n"));
}

/// ingest.sessions.16: the hot store line names only a write that went
/// there. With only `change` routed hot, `subject new` writes the subject to
/// the ledger and prints no hot store line; `subject adopt` rewrites the
/// change in the hot store and prints it.
#[test]
fn ingest_sessions_16c_a_subject_write_routed_to_the_ledger_prints_no_hot_store_line() {
    let routing = "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\nrouting:\n  subject: local\n  change: hot\n";
    let dir = init_repo(routing);
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    let new = canon(&["subject", "new", "demo", "--domain", "d", "--title", "T", "--actor", "a"], repo);
    assert!(new.status.success(), "{}", text(&new));
    assert!(!text(&new).contains("hot store:"), "{}", text(&new));
    let new_json = json(&canon(&["subject", "new", "other", "--domain", "d", "--title", "T", "--actor", "a", "--json"], repo));
    assert!(new_json["meta"].get("hotStore").is_none(), "{new_json}");
}
