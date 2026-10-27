//! 0.31 `canon-0-31-search`: one test per scenario of the change (`search.index.*`, `search.query.*`,
//! `search.trace.*`, `search.graph.*`), through the real binary. Each test name starts with its scenario id
//! (`search.index.01` → `search_index_01_…`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};
use tempfile::TempDir;

// ── harness ──────────────────────────────────────────────────────────────

fn canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).env_remove("CANON_ROLE").env("CANON_ACTOR", "search-test").output().expect("spawn canon")
}

fn json_of(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "gc.auto=0"]).args(args).current_dir(dir).output().expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

const PROV: &str = "# canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}";
const LOCAL_YAML: &str = "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  change: local\n  subject: local\n  scenario: local\n  review: local\n  divergence: local\n  evidence: local\n  finding: local\nspecs:\n  roots:\n    - id: root\n      root: .canon/specs\nwrite:\n  promote: auto\n";
const IGNORE: &str = ".canon/index/\n.canon/hot.db*\n";

fn feature_text(scenarios: &str) -> String {
    format!("Feature: loop run\n  {PROV}\n\n  Background:\n    Given a backgroundword step\n\n{scenarios}")
}

fn two_scenarios() -> String {
    format!("  @loop.run.01 @flow:loop-change.F1\n  Scenario: One\n  {PROV}\n    Given one\n\n  @loop.run.02\n  Scenario: Two\n  {PROV}\n    Given two\n")
}

const BRIEF: &str = "# Loop change\n\n## Intent\n\nMake the loop visible.\n\n## Desired outcome\n\n- A reader sees the briefsectionword chain.\n\n## User flow\n\n1. F1 The reviewer runs the flowstepword trace.\n2. F2 The reviewer reads it.\n\n## Step detail\n\n### F1\n\nDetail one.\n\n### F2\n\nDetail two.\n\n## Acceptance\n\n- F1: a chain.\n\n## Questions and assumptions\n\n- none\n\n## Out of scope\n\n- none\n";

fn feature_path(repo: &Path) -> PathBuf {
    repo.join(".canon/specs/kind=feature/area=loop/run.feature")
}

/// A git repo with canon.yaml (`yaml`), the loop feature (`scenarios`), the change `loop-change` with a brief
/// and a change record, and one commit.
fn repo_with(yaml: &str, scenarios: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::write(repo.join("canon.yaml"), yaml).unwrap();
    std::fs::write(repo.join(".gitignore"), IGNORE).unwrap();
    std::fs::create_dir_all(feature_path(repo).parent().unwrap()).unwrap();
    std::fs::write(feature_path(repo), feature_text(scenarios)).unwrap();
    std::fs::create_dir_all(repo.join(".canon/changes/loop-change")).unwrap();
    std::fs::write(repo.join(".canon/changes/loop-change/brief.md"), BRIEF).unwrap();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "base"]);
    let out = canon(&["inventory", "sync"], repo);
    assert!(out.status.success(), "{}", text(&out));
    put(repo, json!({"actor": {"agent_id": "plan"}, "at": "2026-07-01T00:00:00Z", "change_id": "loop-change", "kind": "change", "schema": 2, "status": "proposed", "subject_ids": [], "summary": "Make the loop visible", "title": "loop-change"}));
    dir
}

fn repo() -> TempDir {
    repo_with(LOCAL_YAML, &two_scenarios())
}

/// Write one record straight into the ledger.
fn put(repo: &Path, body: Value) {
    use canon_store::tier::{RawWrite, Tier};
    let tier = canon_store::git_tier::GitTier::new(repo.join(".canon/ledger"));
    tier.write(&RawWrite(canon_model::RawRecord(body))).expect("write the record");
}

fn search_in(repo: &Path, args: &[&str]) -> Output {
    let mut argv = vec!["search"];
    argv.extend_from_slice(args);
    argv.push("--json");
    canon(&argv, repo)
}

fn data(output: &Output) -> Value {
    let doc = json_of(output);
    assert_eq!(doc["ok"], true, "{}", text(output));
    doc["data"].clone()
}

fn hit_ids(data: &Value) -> Vec<String> {
    data["hits"].as_array().unwrap().iter().map(|h| h["id"].as_str().unwrap().to_string()).collect()
}

fn neighbor_ids(data: &Value) -> Vec<String> {
    data["neighbors"].as_array().unwrap().iter().map(|h| h["id"].as_str().unwrap().to_string()).collect()
}

fn without_rebuilt(mut data: Value) -> Value {
    data["index"].as_object_mut().unwrap().remove("rebuilt");
    data
}

fn warnings(output: &Output) -> Vec<String> {
    json_of(output)["meta"]["warnings"].as_array().map(|w| w.iter().map(|s| s.as_str().unwrap().to_string()).collect()).unwrap_or_default()
}

fn finding(repo: &Path, round: &str, seq: &str, summary: &str, extra: &[&str]) -> Output {
    let mut args = vec!["finding", "add", "--change", "loop-change", "--round", round, "--seq", seq, "--severity", "should-fix", "--actor", "rev", "--summary", summary];
    if !extra.contains(&"--introduced-rev") {
        args.push("--introduced-unknown");
    }
    args.extend_from_slice(extra);
    let out = canon(&args, repo);
    assert!(out.status.success(), "{}", text(&out));
    out
}

fn review(repo: &Path, ids: &[&str], extra: &[&str]) -> String {
    let mut args = vec!["review", "scenario"];
    args.extend_from_slice(ids);
    args.extend(["--role", "reviewer", "--json"]);
    if !extra.contains(&"--upstream-ref") {
        args.extend(["--upstream-ref", "r"]);
    }
    if !extra.contains(&"--actor") {
        args.extend(["--actor", "rev"]);
    }
    args.extend_from_slice(extra);
    let out = canon(&args, repo);
    assert!(out.status.success(), "{}", text(&out));
    json_of(&out)["data"]["reviews"][0]["reviewId"].as_str().unwrap().to_string()
}

fn close_fixed(repo: &Path, round: &str, seq: &str, sha: &str) {
    let out = canon(&["finding", "close", "loop-change", "--round", round, "--seq", seq, "--disposition", "fixed", "--resolution-rev", sha], repo);
    assert!(out.status.success(), "{}", text(&out));
}

fn evidence(repo: &Path, scenario: &str, summary: &str, rev: &str) {
    let out = canon(&["evidence", "add", "--scenario", scenario, "--class", "test-run", "--ref", "cargo test", "--verdict", "faithful", "--summary", summary, "--role", "implementer", "--rev", rev], repo);
    assert!(out.status.success(), "{}", text(&out));
}

/// Commit a change to a file and return the new commit's sha.
fn commit(repo: &Path, what: &str) -> String {
    std::fs::write(repo.join("code.txt"), what).unwrap();
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", what]);
    git(repo, &["rev-parse", "HEAD"])
}

/// The review loop of search.trace.01: review R1 raised finding A about `loop.run.01`, fix commit S2 closed it,
/// review R2 pinned at S2 (R1 goes to the archive), evidence at S2 and again at S3 (the S2 record goes to the
/// archive).
struct Loop {
    dir: TempDir,
    s1: String,
    s2: String,
    s3: String,
}

fn the_loop() -> Loop {
    let dir = repo();
    let r = dir.path();
    let s1 = git(r, &["rev-parse", "HEAD"]);
    let r1 = review(r, &["loop.run.01"], &[]);
    finding(r, "1", "1", "Token is logged", &["--review", &r1, "--scenario", "loop.run.01"]);
    let s2 = commit(r, "fix");
    close_fixed(r, "1", "1", &s2);
    review(r, &["loop.run.01"], &["--rev", &s2]);
    evidence(r, "loop.run.01", "first run", &s2);
    let s3 = commit(r, "later");
    evidence(r, "loop.run.01", "second run", &s3);
    Loop { dir, s1, s2, s3 }
}

fn trace_in(repo: &Path, operand: &str) -> Output {
    canon(&["trace", operand, "--json"], repo)
}

fn chain(output: &Output) -> Vec<Value> {
    data(output)["chain"].as_array().unwrap().clone()
}

/// Every file under `dir`, with its content.
fn tree(dir: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.strip_prefix(root).is_ok_and(|rel| rel.starts_with(".git")) {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

// ── F1: the index ────────────────────────────────────────────────────────

/// search.index.01
#[test]
fn search_index_01_a_search_with_no_index_builds_it_and_writes_an_ignore_file_that_holds_only_a_star() {
    let dir = repo();
    let r = dir.path();
    finding(r, "1", "1", "Token is logged", &[]);
    assert!(!r.join(".canon/index").exists());

    let first = search_in(r, &["token"]);
    let doc = data(&first);
    assert!(r.join(".canon/index/search.db").is_file());
    assert_eq!(std::fs::read_to_string(r.join(".canon/index/.gitignore")).unwrap(), "*\n");
    assert_eq!(hit_ids(&doc), ["finding:loop-change_1_1"]);
    assert_eq!(doc["index"]["rebuilt"], true);

    let second = data(&search_in(r, &["token"]));
    assert_eq!(second["index"]["rebuilt"], false);
    assert_eq!(without_rebuilt(second), without_rebuilt(doc));
    assert!(git(r, &["status", "--porcelain"]).lines().all(|l| !l.contains(".canon/index")), "the index is never untracked-and-visible");
}

/// search.index.02
#[test]
fn search_index_02_a_finding_promoted_just_before_a_search_is_among_its_results() {
    let dir = repo();
    let r = dir.path();
    finding(r, "1", "1", "Token is logged", &[]);
    assert_eq!(hit_ids(&data(&search_in(r, &["logged"]))).len(), 1);

    finding(r, "1", "2", "Cache is logged twice", &[]);
    let doc = data(&search_in(r, &["logged"]));
    assert_eq!(hit_ids(&doc).len(), 2, "{doc}");
    assert!(hit_ids(&doc).contains(&"finding:loop-change_1_2".to_string()));
    assert_eq!(doc["index"]["rebuilt"], true);

    finding(r, "1", "3", "Queue is logged thrice", &["--no-promote"]);
    assert!(r.join(".canon/ledger/_staging").exists());
    let doc = data(&search_in(r, &["logged"]));
    assert_eq!(hit_ids(&doc).len(), 2, "a staged finding is not read");
}

/// search.index.03
#[test]
fn search_index_03_deleting_the_index_file_and_searching_again_gives_the_same_result_apart_from_the_rebuilt_field() {
    let l = the_loop();
    let r = l.dir.path();
    let ledger_before = tree(&r.join(".canon/ledger"));
    let archive_before = tree(&r.join(".canon/archive"));
    let search = || data(&search_in(r, &["logged", "--expand", "2"]));
    let traced = || data(&trace_in(r, "loop-change_1_1"));
    let (first_search, first_trace) = (search(), traced());
    assert_eq!(first_search["index"]["rebuilt"], true);

    for entry in std::fs::read_dir(r.join(".canon/index")).unwrap().flatten() {
        if entry.file_name().to_string_lossy().starts_with("search.db") {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }
    let (again_search, again_trace) = (search(), traced());
    assert_eq!(again_search["index"]["rebuilt"], true);
    assert_eq!(again_trace["index"]["rebuilt"], false, "the search just rebuilt it");
    assert_eq!(without_rebuilt(again_search), without_rebuilt(first_search));
    assert_eq!(without_rebuilt(again_trace), without_rebuilt(first_trace));
    assert_eq!(tree(&r.join(".canon/ledger")), ledger_before, "no record was written");
    assert_eq!(tree(&r.join(".canon/archive")), archive_before, "no record was written");
}

/// search.index.04
#[test]
fn search_index_04_a_corrupt_index_file_is_deleted_and_rebuilt_once_with_a_warning() {
    let dir = repo();
    let r = dir.path();
    finding(r, "1", "1", "Token is logged", &[]);
    let clean = data(&search_in(r, &["token"]));
    std::fs::write(r.join(".canon/index/search.db"), b"this is not a database, only bytes").unwrap();

    let out = search_in(r, &["token"]);
    assert!(out.status.success(), "{}", text(&out));
    let doc = data(&out);
    assert_eq!(doc["index"]["rebuilt"], true);
    assert_eq!(hit_ids(&doc), hit_ids(&clean));
    assert!(warnings(&out).iter().any(|w| w.contains(".canon/index/search.db") && w.contains("not a usable database")), "{:?}", warnings(&out));
    assert!(std::fs::read(r.join(".canon/index/search.db")).unwrap().starts_with(b"SQLite format 3"));
    let next = data(&search_in(r, &["token"]));
    assert_eq!(next["index"]["rebuilt"], false);
}

/// search.index.05
#[test]
fn search_index_05_an_index_directory_that_cannot_be_written_is_built_in_memory_with_a_warning_naming_the_path() {
    let dir = repo();
    let r = dir.path();
    finding(r, "1", "1", "Token is logged", &[]);
    let clean = data(&search_in(r, &["token"]));
    std::fs::remove_dir_all(r.join(".canon/index")).unwrap();
    std::fs::write(r.join(".canon/index"), b"in the way").unwrap();
    let before = tree(r);

    let out = search_in(r, &["token"]);
    assert!(out.status.success(), "{}", text(&out));
    let doc = data(&out);
    assert_eq!(hit_ids(&doc), hit_ids(&clean));
    assert_eq!(doc["index"]["inMemory"], true);
    assert_eq!(doc["index"]["rebuilt"], true);
    assert!(warnings(&out).iter().any(|w| w.contains(".canon/index") && w.contains("in memory")), "{:?}", warnings(&out));
    assert_eq!(tree(r), before, "nothing under the repo changed");
}

/// search.index.06
#[test]
fn search_index_06_a_record_input_that_cannot_be_read_exits_1_with_the_cause_and_writes_no_index() {
    let yaml = LOCAL_YAML.replace("  local: { backend: git, root: .canon/ledger }\n", "  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\n").replace("  finding: local\n", "  finding: local\n  task: hot\n");
    let dir = repo_with(&yaml, &two_scenarios());
    let r = dir.path();
    std::fs::write(r.join(".canon/hot.db"), b"not a sqlite database").unwrap();
    for args in [&["search", "token", "--json"][..], &["trace", "abcdef1", "--json"][..]] {
        let out = canon(args, r);
        assert_eq!(out.status.code(), Some(1), "{}", text(&out));
        let doc = json_of(&out);
        assert_eq!(doc["error"]["code"], "SEARCH_INDEX_FAILED");
        assert!(doc["error"]["message"].as_str().unwrap().contains("hot.db"), "{doc}");
        assert!(!r.join(".canon/index/search.db").exists());
    }
}

/// search.index.07
#[test]
fn search_index_07_a_full_rebuild_of_a_stale_index_over_2300_records_finishes_within_five_seconds() {
    let dir = repo();
    let r = dir.path();
    let sha = |n: usize| format!("{n:040x}");
    let actor = json!({"agent_id": "bulk", "role": "reviewer"});
    let mut count = 0;
    for n in 0..945 {
        put(r, json!({"actor": actor, "at": format!("2026-08-01T00:{:02}:{:02}Z", n / 60 % 60, n % 60), "kind": "review", "pin": sha(n), "project_id": "root", "provenance_ref": {"upstream_ref": "bulk"}, "reviewer": "bulk", "scenario_id": format!("bulk.review.{n:03}"), "schema": 1}));
        count += 1;
    }
    for n in 0..612 {
        put(r, json!({"actor": {"agent_id": "bulk"}, "at": "2026-08-01T00:00:00Z", "kind": "scenario", "project_id": "root", "scenario_digest": format!("{n:064x}"), "scenario_id": format!("bulk.scenario.{n:03}"), "schema": 1, "source_digest": format!("{:064x}", n + 1), "title": format!("Bulk scenario {n} reads its records")}));
        count += 1;
    }
    for n in 0..515 {
        put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "evidence": {"kind": "test-run", "ref": format!("cargo test bulk {n}")}, "evidence_note": {"summary": format!("bulk run {n} passed"), "command_result": "ok"}, "evidence_sha": sha(n), "kind": "evidence", "project_id": "root", "run_seq": n, "scenario_id": format!("bulk.scenario.{:03}", n % 612), "schema": 1, "verdict": "faithful"}));
        count += 1;
    }
    for n in 0..175 {
        put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "change_id": format!("bulk-change-{}", n % 19), "disposition": "open", "kind": "finding", "reviewer": "bulk", "round": 1, "schema": 2, "seq": n, "severity": "note", "summary": format!("Bulk finding {n} says the retry is slow")}));
        count += 1;
    }
    for n in 0..34 {
        put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "change_ids": [format!("bulk-change-{}", n % 19)], "domain": "dev", "kind": "subject", "owner_role": "implementer", "schema": 1, "status": "building", "subject_id": format!("bulk-subject-{n}"), "summary": "", "title": format!("Bulk subject {n}")}));
        count += 1;
    }
    for n in 0..19 {
        put(r, json!({"actor": {"agent_id": "plan"}, "at": "2026-08-01T00:00:00Z", "change_id": format!("bulk-change-{n}"), "kind": "change", "schema": 2, "status": "proposed", "subject_ids": [], "summary": format!("Bulk change {n}"), "title": format!("bulk-change-{n}")}));
        std::fs::create_dir_all(r.join(format!(".canon/changes/bulk-change-{n}"))).unwrap();
        std::fs::write(r.join(format!(".canon/changes/bulk-change-{n}/brief.md")), BRIEF).unwrap();
        count += 1;
    }
    assert_eq!(count, 2300);

    let built = data(&search_in(r, &["slow"]));
    assert_eq!(built["index"]["rebuilt"], true);
    put(r, json!({"actor": actor, "at": "2026-08-02T00:00:00Z", "change_id": "bulk-change-1", "disposition": "open", "kind": "finding", "reviewer": "bulk", "round": 2, "schema": 2, "seq": 1, "severity": "note", "summary": "A stale index is rebuilt in full"}));

    let started = std::time::Instant::now();
    let out = search_in(r, &["stale index"]);
    let elapsed = started.elapsed();
    let doc = data(&out);
    assert_eq!(doc["index"]["rebuilt"], true);
    assert_eq!(hit_ids(&doc), ["finding:bulk-change-1_2_1"]);
    assert!(doc["index"]["nodes"].as_u64().unwrap() >= 2301, "every record is a node: {}", doc["index"]);
    assert!(elapsed < std::time::Duration::from_secs(5), "the rebuild took {elapsed:?}");
    eprintln!("search.index.07: rebuilt {} nodes, {} edges in {elapsed:?}", doc["index"]["nodes"], doc["index"]["edges"]);
}

/// search.index.08
#[test]
fn search_index_08_records_in_postgres_or_an_object_store_and_staged_records_are_not_read_and_the_unread_stores_are_named() {
    let yaml = LOCAL_YAML.replace("  local: { backend: git, root: .canon/ledger }\n", "  local: { backend: git, root: .canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_SEARCH_TEST_NO_SUCH_DSN, schema: canon_v1 }\n  cold: { backend: s3, bucket_env: CANON_SEARCH_TEST_NO_SUCH_BUCKET, prefix: \"canon/\" }\n");
    let yaml = yaml.replace("  finding: local\n", "  finding: local\n  task: hot\n  trajectory: cold\n");
    let dir = repo_with(&yaml, &two_scenarios());
    let r = dir.path();
    finding(r, "1", "1", "Token is logged", &[]);
    finding(r, "1", "2", "Token is staged", &["--no-promote"]);
    let out = search_in(r, &["token"]);
    assert!(out.status.success(), "{}", text(&out));
    let doc = data(&out);
    assert_eq!(doc["index"]["unreadStores"], json!(["postgres", "s3"]));
    assert_eq!(hit_ids(&doc), ["finding:loop-change_1_1"]);
}

/// search.index.09
#[test]
fn search_index_09_the_read_leaves_name_the_cache_in_their_effect_note_init_ignores_it_and_until_then_they_warn() {
    let dir = repo();
    let r = dir.path();
    std::fs::write(r.join(".gitignore"), ".canon/hot.db*\n").unwrap();
    let home = tempfile::tempdir().unwrap();
    let with_home = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(r).env_remove("CANON_ROLE").env("HOME", home.path()).output().unwrap();

    let before = with_home(&["search", "loop", "--json"]);
    assert!(before.status.success(), "{}", text(&before));
    assert!(warnings(&before).iter().any(|w| w.contains(".gitignore") && w.contains(".canon/index/") && w.contains("canon init")), "{:?}", warnings(&before));
    let traced = with_home(&["trace", "abcdef1", "--json"]);
    assert!(warnings(&traced).iter().any(|w| w.contains("canon init")), "trace warns too: {:?}", warnings(&traced));

    let init = with_home(&["init", "--providers", "none", "--no-agents-md", "--no-policy"]);
    assert!(init.status.success(), "{}", text(&init));
    assert!(std::fs::read_to_string(r.join(".gitignore")).unwrap().lines().any(|l| l == ".canon/index/"));
    let after = with_home(&["search", "loop", "--json"]);
    assert!(warnings(&after).is_empty(), "{:?}", warnings(&after));

    for leaf in ["search", "trace"] {
        let schema = json_of(&with_home(&["schema", leaf, "--json"]))["data"].clone();
        assert_eq!(schema["effect"], "read");
        let note = schema["effectNote"].as_str().unwrap();
        assert!(note.contains(".canon/index/search.db") && note.contains(".canon/index/.gitignore"), "{note}");
    }
}

/// search.index.10
#[test]
fn search_index_10_searches_that_run_at_the_same_time_on_a_stale_index_all_succeed_and_leave_one_usable_index() {
    let dir = repo();
    let r = dir.path();
    finding(r, "1", "1", "Token is logged", &[]);
    let children: Vec<_> = (0..8).map(|_| Command::new(env!("CARGO_BIN_EXE_canon")).args(["search", "token", "--json"]).current_dir(r).env_remove("CANON_ROLE").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap()).collect();
    let outputs: Vec<Output> = children.into_iter().map(|c| c.wait_with_output().unwrap()).collect();
    for out in &outputs {
        assert!(out.status.success(), "{}", text(out));
        assert_eq!(hit_ids(&data(out)), ["finding:loop-change_1_1"]);
    }
    assert!(std::fs::read(r.join(".canon/index/search.db")).unwrap().starts_with(b"SQLite format 3"));
    let leftovers: Vec<_> = std::fs::read_dir(r.join(".canon/index")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".tmp")).collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    assert_eq!(data(&search_in(r, &["token"]))["index"]["rebuilt"], false);
}

/// search.index.11
#[test]
fn search_index_11_any_change_to_a_record_a_brief_a_feature_file_or_the_hot_store_makes_the_index_stale_and_the_next_run_rebuilds_before_reading() {
    let yaml = LOCAL_YAML.replace("  local: { backend: git, root: .canon/ledger }\n", "  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\n").replace("  finding: local\n", "  finding: local\n  task: hot\n");
    let dir = repo_with(&yaml, &two_scenarios());
    let r = dir.path();
    assert_eq!(data(&search_in(r, &["zzfeatureword"]))["index"]["rebuilt"], true);
    assert_eq!(data(&search_in(r, &["zzfeatureword"]))["index"]["rebuilt"], false, "no change, no rebuild");

    let mut feature = std::fs::read_to_string(feature_path(r)).unwrap();
    feature.push_str(&format!("\n  @loop.run.03\n  Scenario: Three has zzfeatureword\n  {PROV}\n    Given three\n"));
    std::fs::write(feature_path(r), feature).unwrap();
    let doc = data(&search_in(r, &["zzfeatureword"]));
    assert_eq!(doc["index"]["rebuilt"], true);
    assert_eq!(hit_ids(&doc), ["scenario:root_loop.run.03"]);

    std::fs::write(r.join(".canon/changes/loop-change/brief.md"), format!("{BRIEF}\nzzbriefword\n")).unwrap();
    let doc = data(&search_in(r, &["zzbriefword"]));
    assert_eq!(doc["index"]["rebuilt"], true);
    assert_eq!(doc["hits"].as_array().unwrap().len(), 1);

    let hot = canon_store::sqlite_tier::SqliteTier::connect(&r.join(".canon/hot.db")).unwrap();
    use canon_store::tier::{RawWrite, Tier};
    hot.write(&RawWrite(canon_model::RawRecord(json!({"actor": {"agent_id": "t"}, "at": "2026-08-01T00:00:00Z", "kind": "task", "schema": 1, "status": "open", "task_id": "loop-change#1", "title": "zzhotword task"})))).unwrap();
    drop(hot);
    let doc = data(&search_in(r, &["zzhotword"]));
    assert_eq!(doc["index"]["rebuilt"], true);
    assert_eq!(hit_ids(&doc), ["task:loop-change#1"]);

    finding(r, "1", "1", "A zzrecordword finding", &[]);
    let doc = data(&search_in(r, &["zzrecordword"]));
    assert_eq!(doc["index"]["rebuilt"], true);
    assert_eq!(data(&search_in(r, &["zzrecordword"]))["index"]["rebuilt"], false);
}

// ── F2: search ───────────────────────────────────────────────────────────

/// search.query.01
#[test]
fn search_query_01_a_keyword_search_returns_ranked_hits_with_their_neighbors_and_edges() {
    let dir = repo();
    let r = dir.path();
    let run = review(r, &["loop.run.01"], &[]);
    finding(r, "1", "1", "The request timeout is not retried", &["--review", &run, "--scenario", "loop.run.01"]);
    finding(r, "1", "2", "A timeout value hard coded in two places and never read from the settings file", &["--review", &run, "--scenario", "loop.run.01"]);
    finding(r, "1", "3", "Unrelated naming nit", &[]);

    let doc = data(&search_in(r, &["timeout"]));
    let hits = doc["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 2, "{doc}");
    for (n, hit) in hits.iter().enumerate() {
        for key in ["id", "kind", "rank", "score", "title", "snippet", "placeholder"] {
            assert!(hit.get(key).is_some(), "{key} in {hit}");
        }
        assert_eq!(hit["rank"], n + 1);
        assert_eq!(hit["kind"], "finding");
        assert_eq!(hit["placeholder"], false);
        assert!(hit["snippet"].as_str().unwrap().contains("timeout"));
    }
    assert!(hits[0]["score"].as_f64().unwrap() >= hits[1]["score"].as_f64().unwrap(), "best score first");
    assert!(hits[0]["score"].as_f64().unwrap() > 0.0);
    finding(r, "1", "4", "Identical retry wording", &[]);
    finding(r, "1", "5", "Identical retry wording", &[]);
    let tied = data(&search_in(r, &["Identical retry wording"]));
    let tied_hits = tied["hits"].as_array().unwrap();
    assert_eq!(tied_hits.len(), 2, "{tied}");
    assert_eq!(tied_hits[0]["score"], tied_hits[1]["score"], "equal scores");
    assert!(tied_hits[0]["id"].as_str().unwrap() < tied_hits[1]["id"].as_str().unwrap(), "equal scores order by id: {tied}");
    let neighbors = neighbor_ids(&doc);
    assert!(neighbors.contains(&"review:root_scenario_loop.run.01".to_string()), "{neighbors:?}");
    assert!(neighbors.contains(&"scenario:root_loop.run.01".to_string()), "{neighbors:?}");
    assert!(neighbors.contains(&"change:loop-change".to_string()), "{neighbors:?}");
    for n in doc["neighbors"].as_array().unwrap() {
        for key in ["id", "kind", "hops", "via", "title", "placeholder"] {
            assert!(n.get(key).is_some(), "{key} in {n}");
        }
        assert_eq!(n["hops"], 1);
    }
    let edges = doc["edges"].as_array().unwrap();
    assert!(edges.iter().any(|e| e["from"] == "finding:loop-change_1_1" && e["to"] == "change:loop-change" && e["rel"] == "change_id"), "{edges:?}");
    assert!(edges.iter().any(|e| e["from"] == "finding:loop-change_1_1" && e["to"] == "review:root_scenario_loop.run.01" && e["rel"] == "review_id"), "{edges:?}");
    assert_eq!(doc["counts"], json!({"matched": 2, "hits": 2, "neighbors": neighbors.len(), "edges": edges.len()}));
    assert!(doc["index"]["nodes"].as_u64().unwrap() > 0 && doc["index"]["edges"].as_u64().unwrap() > 0);

    let human = canon(&["search", "timeout"], r);
    assert!(human.status.success());
    assert!(String::from_utf8_lossy(&human.stdout).contains("finding:loop-change_1_1"));
}

/// search.query.02
#[test]
fn search_query_02_a_finding_summary_written_in_korean_is_found_by_a_two_character_word_and_by_a_word_of_three_or_more_characters() {
    let dir = repo();
    let r = dir.path();
    let run = review(r, &["loop.run.01"], &[]);
    finding(r, "1", "1", "재시도 로직이 타임아웃 값을 무시한다", &["--review", &run, "--scenario", "loop.run.01"]);
    finding(r, "1", "2", "다른 로직도 같은 문제가 있다", &[]);

    let long = data(&search_in(r, &["타임아웃"]));
    assert_eq!(hit_ids(&long), ["finding:loop-change_1_1"], "{long}");
    let neighbors = neighbor_ids(&long);
    assert!(neighbors.contains(&"review:root_scenario_loop.run.01".to_string()), "{neighbors:?}");
    assert!(neighbors.contains(&"scenario:root_loop.run.01".to_string()), "{neighbors:?}");
    assert!(neighbors.contains(&"change:loop-change".to_string()), "{neighbors:?}");

    let short = data(&search_in(r, &["로직"]));
    assert_eq!(hit_ids(&short), ["finding:loop-change_1_1", "finding:loop-change_1_2"], "a two-character query lists its hits by id");
    assert!(short["hits"].as_array().unwrap().iter().all(|h| h["score"] == 0.0));
    let three = data(&search_in(r, &["로직이"]));
    assert_eq!(hit_ids(&three), ["finding:loop-change_1_1"]);
}

/// search.query.03
#[test]
fn search_query_03_kind_keeps_only_seeds_of_those_kinds_limit_caps_the_hits_and_expand_sets_the_hops() {
    let dir = repo();
    let r = dir.path();
    let run = review(r, &["loop.run.01"], &[]);
    finding(r, "1", "1", "The loop run needs a retry", &["--review", &run, "--scenario", "loop.run.01"]);
    put(r, json!({"actor": {"agent_id": "p"}, "at": "2026-07-02T00:00:00Z", "change_ids": ["loop-change"], "domain": "dev", "kind": "subject", "owner_role": "implementer", "schema": 1, "status": "building", "subject_id": "loop-subject", "summary": "", "title": "Loop subject"}));

    let all = data(&search_in(r, &["loop"]));
    let kinds: std::collections::BTreeSet<String> = all["hits"].as_array().unwrap().iter().map(|h| h["kind"].as_str().unwrap().to_string()).collect();
    assert!(kinds.contains("finding") && kinds.contains("scenario") && kinds.contains("brief"), "{kinds:?}");

    let only = data(&search_in(r, &["loop", "--kind", "finding"]));
    assert_eq!(hit_ids(&only), ["finding:loop-change_1_1"]);
    assert!(!neighbor_ids(&only).is_empty(), "the neighbors are of any kind");
    let two = data(&search_in(r, &["loop", "--kind", "finding", "--kind", "scenario"]));
    assert!(two["hits"].as_array().unwrap().iter().all(|h| h["kind"] == "finding" || h["kind"] == "scenario"));
    assert!(two["hits"].as_array().unwrap().iter().any(|h| h["kind"] == "scenario"));

    let one = data(&search_in(r, &["loop", "--limit", "1"]));
    assert_eq!(one["hits"].as_array().unwrap().len(), 1);
    assert_eq!(one["counts"]["matched"], all["counts"]["matched"]);
    assert!(one["counts"]["matched"].as_u64().unwrap() > 1);

    let none = data(&search_in(r, &["loop", "--kind", "finding", "--expand", "0"]));
    assert!(none["neighbors"].as_array().unwrap().is_empty() && none["edges"].as_array().unwrap().is_empty());
    let one_hop = data(&search_in(r, &["retry", "--kind", "finding", "--expand", "1"]));
    let two_hops = data(&search_in(r, &["retry", "--kind", "finding", "--expand", "2"]));
    assert!(two_hops["neighbors"].as_array().unwrap().iter().any(|n| n["hops"] == 2), "{two_hops}");
    assert!(two_hops["neighbors"].as_array().unwrap().len() > one_hop["neighbors"].as_array().unwrap().len());
    assert!(one_hop["neighbors"].as_array().unwrap().iter().all(|n| n["hops"] == 1));
}

/// search.query.04
#[test]
fn search_query_04_an_empty_or_whitespace_query_exits_2_and_builds_nothing() {
    let dir = repo();
    let r = dir.path();
    for query in ["", "   "] {
        let out = canon(&["search", query, "--json"], r);
        assert_eq!(out.status.code(), Some(2), "{}", text(&out));
        assert_eq!(json_of(&out)["error"]["code"], "SEARCH_QUERY_EMPTY");
    }
    assert!(!r.join(".canon/index").exists());
}

/// search.query.05
#[test]
fn search_query_05_a_query_that_matches_nothing_exits_0_with_empty_hits() {
    let dir = repo();
    let r = dir.path();
    finding(r, "1", "1", "Token is logged", &[]);
    for query in ["qqqqnothing", "zz"] {
        let out = search_in(r, &[query]);
        assert_eq!(out.status.code(), Some(0), "{}", text(&out));
        let doc = data(&out);
        assert!(doc["hits"].as_array().unwrap().is_empty() && doc["neighbors"].as_array().unwrap().is_empty() && doc["edges"].as_array().unwrap().is_empty());
        assert_eq!(doc["counts"]["matched"], 0);
    }
}

/// search.query.06
#[test]
fn search_query_06_the_query_is_read_as_literal_text_quotes_and_and_are_not_syntax() {
    let dir = repo();
    let r = dir.path();
    finding(r, "1", "1", "\"retry\" AND timeout (v2) is wrong", &[]);
    finding(r, "1", "2", "retry the timeout", &[]);
    let exact = data(&search_in(r, &["\"retry\" AND timeout (v2)"]));
    assert_eq!(hit_ids(&exact), ["finding:loop-change_1_1"]);
    for query in ["\"", "x\"y\"z", "retry OR", "(v2", "NEAR(a b)", "a*"] {
        let out = search_in(r, &[query]);
        assert_eq!(out.status.code(), Some(0), "{query}: {}", text(&out));
    }
    let and = data(&search_in(r, &["retry AND nothing"]));
    assert!(and["hits"].as_array().unwrap().is_empty(), "AND is not an operator: {and}");
    let both = data(&search_in(r, &["retry the timeout"]));
    assert_eq!(hit_ids(&both), ["finding:loop-change_1_2"], "words in order are one phrase, not a set of terms");
}

/// search.query.07
#[test]
fn search_query_07_scenario_bodies_evidence_companions_and_brief_sections_are_searchable() {
    let scenarios = format!(
        "  @loop.run.01 @flow:loop-change.F1\n  Scenario: Doc and table\n  {PROV}\n    Given a docstringword text:\n      \"\"\"\n      the docstringword body\n      \"\"\"\n    When the table has\n      | col  | other |\n      | cellword | x |\n    Then it passes\n\n  Rule: Limits\n    Background:\n      Given a rulebgword limit\n\n    @loop.run.02\n    Scenario Outline: Outline\n    {PROV}\n      Given <n> of them\n\n      Examples:\n        | n |\n        | examplesword |\n"
    );
    let dir = repo_with(LOCAL_YAML, &scenarios);
    let r = dir.path();
    let out = canon(&["evidence", "add", "--scenario", "loop.run.01", "--class", "test-run", "--ref", "cargo test --test refword", "--verdict", "faithful", "--summary", "summaryword passed", "--command-result", "commandword 3 passed", "--role", "implementer"], r);
    assert!(out.status.success(), "{}", text(&out));

    let one = |word: &str, prefix: &str| {
        let doc = data(&search_in(r, &[word, "--expand", "0"]));
        let ids = hit_ids(&doc);
        assert!(ids.iter().any(|id| id.starts_with(prefix)), "{word}: {ids:?}");
        ids
    };
    assert!(one("backgroundword", "scenario:").contains(&"scenario:root_loop.run.01".to_string()));
    assert!(one("docstringword", "scenario:").contains(&"scenario:root_loop.run.01".to_string()));
    assert!(one("cellword", "scenario:").contains(&"scenario:root_loop.run.01".to_string()));
    let rule = one("rulebgword", "scenario:");
    assert_eq!(rule, ["scenario:root_loop.run.02"], "a Rule's Background is part of the scenarios inside it");
    assert_eq!(one("examplesword", "scenario:"), ["scenario:root_loop.run.02"]);
    assert_eq!(one("summaryword", "evidence:"), ["evidence:root_scenario_loop.run.01"]);
    assert_eq!(one("commandword", "evidence:"), ["evidence:root_scenario_loop.run.01"]);
    assert!(one("refword", "evidence:").len() == 1);
    assert_eq!(one("briefsectionword", "brief:"), ["brief:loop-change#outcome"]);
    assert_eq!(one("flowstepword", "brief:"), ["brief:loop-change#F1", "brief:loop-change#flow"], "the step and the section it is in");
}

/// search.query.08
#[test]
fn search_query_08_the_graph_joins_records_by_reference_fields_commits_and_flow_tags() {
    let dir = repo();
    let r = dir.path();
    let s1 = git(r, &["rev-parse", "HEAD"]);
    let run1 = review(r, &["loop.run.01"], &[]);
    finding(r, "1", "1", "alpha finding", &["--review", &run1, "--scenario", "loop.run.01"]);
    let s2 = commit(r, "fix");
    close_fixed(r, "1", "1", &s2);
    finding(r, "2", "1", "beta finding reviewed at the fix", &["--rev", &s2, "--scenario", "loop.run.01"]);
    finding(r, "2", "2", "gamma finding introduced by the fix", &["--introduced-rev", &s2, "--scenario", "loop.run.01"]);
    let run2 = {
        let out = canon(&["review", "scenario", "loop.run.01", "loop.run.02", "--upstream-ref", "r", "--actor", "rev", "--role", "reviewer", "--rev", &s2, "--json"], r);
        assert!(out.status.success(), "{}", text(&out));
        json_of(&out)["data"]["reviews"][0]["reviewId"].as_str().unwrap().to_string()
    };
    finding(r, "3", "1", "delta finding raised in the second run", &["--review", &run2, "--scenario", "loop.run.02", "--rev", &s2]);
    evidence(r, "loop.run.01", "evidence at the fix", &s2);
    let _ = s1;

    let doc = data(&search_in(r, &["finding", "--limit", "20", "--expand", "1"]));
    let edges = doc["edges"].as_array().unwrap();
    let has = |from: &str, to: &str, rel: &str| edges.iter().any(|e| e["from"] == from && e["to"] == to && e["rel"] == rel);
    assert!(has("finding:loop-change_1_1", "finding:loop-change_2_1", "re-review"), "{edges:?}");
    assert!(has("finding:loop-change_1_1", "finding:loop-change_2_2", "fix-of-fix"), "{edges:?}");
    assert!(has("finding:loop-change_1_1", "review:root_scenario_loop.run.01@", "fix-reviewed") || edges.iter().any(|e| e["from"] == "finding:loop-change_1_1" && e["rel"] == "fix-reviewed"), "{edges:?}");
    assert!(edges.iter().any(|e| e["from"] == "evidence:root_scenario_loop.run.01" && e["rel"] == "evidence-at-commit"), "{edges:?}");
    assert!(has("finding:loop-change_1_1", "scenario:root_loop.run.01", "scenario_ids"));
    assert!(has("finding:loop-change_1_1", "change:loop-change", "change_id"));
    let review_edges: Vec<&Value> = edges.iter().filter(|e| e["from"] == "finding:loop-change_3_1" && e["rel"] == "review_id").collect();
    assert_eq!(review_edges.len(), 2, "a finding's review id goes to every review that carries it: {edges:?}");

    let scenario = data(&search_in(r, &["One", "--kind", "scenario"]));
    assert!(scenario["edges"].as_array().unwrap().iter().any(|e| e["from"] == "scenario:root_loop.run.01" && e["to"] == "brief:loop-change#F1" && e["rel"] == "flow"), "{scenario}");
}

/// search.query.09
#[test]
fn search_query_09_only_the_current_version_of_a_record_can_be_a_hit() {
    let dir = repo();
    let r = dir.path();
    let s1 = git(r, &["rev-parse", "HEAD"]);
    review(r, &["loop.run.01"], &["--upstream-ref", "oldreviewerx"]);
    let s2 = commit(r, "next");
    review(r, &["loop.run.01"], &["--upstream-ref", "newreviewery", "--rev", &s2]);
    evidence(r, "loop.run.01", "firstversionword run", &s1);
    evidence(r, "loop.run.01", "secondversionword run", &s2);
    assert!(r.join(".canon/archive/kind=evidence").is_dir() && r.join(".canon/archive/kind=review").is_dir(), "the older versions are in the archive");

    for old in ["firstversionword", "oldreviewerx"] {
        assert!(data(&search_in(r, &[old]))["hits"].as_array().unwrap().is_empty(), "{old}");
    }
    assert_eq!(hit_ids(&data(&search_in(r, &["secondversionword"]))), ["evidence:root_scenario_loop.run.01"]);
}

// ── F3: trace ────────────────────────────────────────────────────────────

fn roles(chain: &[Value]) -> Vec<String> {
    chain.iter().map(|i| i["role"].as_str().unwrap().to_string()).collect()
}

/// search.trace.01
#[test]
fn search_trace_01_tracing_a_finding_lists_the_review_the_finding_its_fix_the_re_review_and_the_evidence_in_time_order_with_the_archived_versions() {
    let l = the_loop();
    let r = l.dir.path();
    let out = trace_in(r, "loop-change_1_1");
    assert!(out.status.success(), "{}", text(&out));
    let doc = data(&out);
    assert_eq!(doc["resolvedAs"], "finding");
    let items = chain(&out);
    assert_eq!(roles(&items), ["review", "finding", "fix", "review", "evidence"], "{doc}");
    let ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["review:root_scenario_loop.run.01", "finding:loop-change_1_1", "finding:loop-change_1_1", "review:root_scenario_loop.run.01", "evidence:root_scenario_loop.run.01"]);
    let kinds: Vec<&str> = items.iter().map(|i| i["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["review", "finding", "finding", "review", "evidence"]);
    let archived: Vec<bool> = items.iter().map(|i| i["archived"].as_bool().unwrap()).collect();
    assert_eq!(archived, [true, true, false, false, true], "R1, the finding's first version and the S2 evidence are the archive's");
    let shas: Vec<&str> = items.iter().map(|i| i["sha"].as_str().unwrap()).collect();
    assert_eq!(shas, [l.s1.as_str(), l.s1.as_str(), l.s2.as_str(), l.s2.as_str(), l.s2.as_str()]);
    assert!(!shas.contains(&l.s3.as_str()), "the evidence at S3 joins nothing");
    let times: Vec<&str> = items.iter().map(|i| i["at"].as_str().unwrap()).collect();
    assert!(times.windows(2).all(|w| w[0] <= w[1]), "{times:?}");
    for item in &items {
        for key in ["at", "id", "kind", "role", "sha", "edges", "version"] {
            assert!(item.get(key).is_some(), "{key} in {item}");
        }
    }
    let rels = |i: usize| -> Vec<String> { items[i]["edges"].as_array().unwrap().iter().map(|e| e["rel"].as_str().unwrap().to_string()).collect() };
    assert_eq!(rels(0), ["review_id"]);
    assert!(rels(2).contains(&"fix-reviewed".to_string()) && rels(2).contains(&"evidence-at-commit".to_string()), "{:?}", rels(2));
    assert_eq!(doc["counts"], json!({"items": 5, "archived": 3}));

    let human = canon(&["trace", "loop-change_1_1"], r);
    assert!(human.status.success() && String::from_utf8_lossy(&human.stdout).contains("(archived)"));
}

/// search.trace.02
#[test]
fn search_trace_02_tracing_the_fix_commit_gives_the_same_chain_as_tracing_the_finding() {
    let l = the_loop();
    let r = l.dir.path();
    let by_finding = chain(&trace_in(r, "loop-change_1_1"));
    for operand in [l.s2.as_str(), &l.s2[..7], &l.s2[..20]] {
        let out = trace_in(r, operand);
        assert!(out.status.success(), "{operand}: {}", text(&out));
        assert_eq!(data(&out)["resolvedAs"], "commit");
        assert_eq!(chain(&out), by_finding, "{operand}");
    }
}

/// search.trace.03
#[test]
fn search_trace_03_tracing_a_review_id_starts_from_every_review_the_run_wrote_and_a_26_character_operand_with_no_review_is_read_as_a_sha_prefix() {
    let dir = repo();
    let r = dir.path();
    let out = canon(&["review", "scenario", "loop.run.01", "loop.run.02", "--upstream-ref", "r", "--actor", "rev", "--role", "reviewer", "--json"], r);
    assert!(out.status.success(), "{}", text(&out));
    let run = json_of(&out)["data"]["reviews"][0]["reviewId"].as_str().unwrap().to_string();
    finding(r, "1", "1", "Raised in the run", &["--review", &run, "--scenario", "loop.run.01"]);

    let traced = trace_in(r, &run);
    assert!(traced.status.success(), "{}", text(&traced));
    assert_eq!(data(&traced)["resolvedAs"], "review");
    let items = chain(&traced);
    assert_eq!(roles(&items).iter().filter(|r| *r == "review").count(), 2, "both reviews of the run start the chain");
    assert!(items.iter().any(|i| i["id"] == "finding:loop-change_1_1"));
    let finding_item = items.iter().find(|i| i["role"] == "finding").unwrap();
    let to: Vec<&str> = finding_item["edges"].as_array().unwrap().iter().filter(|e| e["rel"] == "review_id").map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(to, ["review:root_scenario_loop.run.01"], "the finding keeps only the review that covers its scenario");

    let head = git(r, &["rev-parse", "HEAD"]);
    let reviews = trace_in(r, &head[..26]);
    assert!(reviews.status.success(), "{}", text(&reviews));
    assert_eq!(data(&reviews)["resolvedAs"], "commit", "a 26-character hex operand no review carries is a sha prefix");
}

/// search.trace.04
#[test]
fn search_trace_04_an_unknown_id_an_ambiguous_sha_prefix_and_an_unrecognized_shape_exit_2_and_name_the_three_accepted_shapes() {
    let dir = repo();
    let r = dir.path();
    let actor = json!({"agent_id": "t", "role": "implementer"});
    for (n, sha) in ["abcdef0123456789abcdef0123456789abcdef01", "abcdef0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"].iter().enumerate() {
        put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "evidence": {"kind": "test-run", "ref": "x"}, "evidence_sha": sha, "kind": "evidence", "project_id": "root", "run_seq": n, "scenario_id": format!("loop.run.0{}", n + 1), "schema": 1, "verdict": "faithful"}));
    }
    assert!(search_in(r, &["loop"]).status.success());
    let outside_index = |r: &Path| tree(r).into_iter().filter(|(path, _)| !path.starts_with(".canon/index")).collect::<Vec<_>>();
    let before = outside_index(r);
    for (operand, code) in [("deadbeefdead", "TRACE_NOT_FOUND"), ("abcdef0", "TRACE_NOT_FOUND"), ("01ARZ3NDEKTSV4RRFFQ69G5FAV", "TRACE_NOT_FOUND"), ("loop-change_9_9", "TRACE_NOT_FOUND"), ("not a shape!", "TRACE_OPERAND_INVALID"), ("abc", "TRACE_OPERAND_INVALID"), ("", "TRACE_OPERAND_INVALID")] {
        let out = trace_in(r, operand);
        assert_eq!(out.status.code(), Some(2), "{operand}: {}", text(&out));
        let doc = json_of(&out);
        assert_eq!(doc["error"]["code"], code, "{operand}: {doc}");
        let message = doc["error"]["message"].as_str().unwrap();
        assert!(message.contains("review id") && message.contains("commit sha") && message.contains("finding key"), "{operand}: {message}");
    }
    assert!(trace_in(r, "abcdef01").status.success(), "a prefix that fits one commit needs no more characters");
    assert_eq!(outside_index(r), before, "a trace writes nothing but the index");
}

/// search.trace.05
#[test]
fn search_trace_05_a_finding_recorded_without_a_scenario_has_no_evidence_in_its_chain() {
    let dir = repo();
    let r = dir.path();
    finding(r, "1", "1", "No scenario here", &[]);
    let s2 = commit(r, "fix");
    close_fixed(r, "1", "1", &s2);
    evidence(r, "loop.run.01", "evidence at the fix commit", &s2);
    let items = chain(&trace_in(r, "loop-change_1_1"));
    assert_eq!(roles(&items), ["finding", "fix"], "{items:?}");
}

/// search.trace.06
#[test]
fn search_trace_06_a_fix_reviewed_again_and_a_fix_of_a_fix_chain_through_every_round_in_time_order() {
    let dir = repo();
    let r = dir.path();
    finding(r, "1", "1", "Round one", &[]);
    let x = commit(r, "x");
    close_fixed(r, "1", "1", &x);
    finding(r, "2", "1", "Round two reviewed at X", &["--rev", &x]);
    let y = commit(r, "y");
    close_fixed(r, "2", "1", &y);
    finding(r, "3", "1", "Round three caused by Y", &["--introduced-rev", &y, "--rev", &x]);
    let out = trace_in(r, "loop-change_1_1");
    let items = chain(&out);
    let shown: Vec<String> = items.iter().map(|i| format!("{}/{}", i["id"].as_str().unwrap(), i["role"].as_str().unwrap())).collect();
    assert_eq!(shown, ["finding:loop-change_1_1/finding", "finding:loop-change_1_1/fix", "finding:loop-change_2_1/finding", "finding:loop-change_2_1/fix", "finding:loop-change_3_1/finding"], "{shown:?}");
    let rel = |i: usize, rel: &str| items[i]["edges"].as_array().unwrap().iter().any(|e| e["rel"] == rel);
    assert!(rel(1, "re-review"), "{:?}", items[1]);
    assert!(rel(3, "fix-of-fix"), "{:?}", items[3]);

    // Equal times are ordered by id: two records written with one `at`.
    let other = repo();
    let o = other.path();
    let at = "2026-08-01T00:00:00Z";
    let sha = "1".repeat(40);
    let base = |round: u32, extra: Value| {
        let mut body = json!({"actor": {"agent_id": "t", "role": "reviewer"}, "at": at, "change_id": "loop-change", "disposition": "open", "kind": "finding", "reviewer": "t", "round": round, "schema": 2, "seq": 1, "severity": "note", "summary": "tie"});
        body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        body
    };
    put(o, base(1, json!({"disposition": "fixed", "resolution_sha": sha})));
    put(o, base(2, json!({"reviewed_sha": sha})));
    let items = chain(&trace_in(o, "loop-change_1_1"));
    let ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["finding:loop-change_1_1", "finding:loop-change_1_1", "finding:loop-change_2_1"], "{items:?}");
}

// ── F4: placeholders ─────────────────────────────────────────────────────

fn ghost_findings(r: &Path) {
    let actor = json!({"agent_id": "t", "role": "reviewer"});
    for seq in 1..=2 {
        put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "change_id": "ghost-change", "disposition": "open", "kind": "finding", "reviewer": "t", "round": 1, "schema": 2, "seq": seq, "severity": "note", "summary": format!("A ghostword finding number {seq}")}));
    }
}

/// search.graph.01
#[test]
fn search_graph_01_a_finding_about_a_change_with_no_source_gets_a_marked_placeholder_node_and_no_error() {
    let dir = repo();
    let r = dir.path();
    ghost_findings(r);
    let out = search_in(r, &["ghostword"]);
    assert!(out.status.success(), "{}", text(&out));
    let doc = data(&out);
    assert_eq!(hit_ids(&doc).len(), 2);
    let placeholder = doc["neighbors"].as_array().unwrap().iter().find(|n| n["id"] == "change:ghost-change").unwrap_or_else(|| panic!("{doc}"));
    assert_eq!(placeholder["placeholder"], true);
    assert_eq!(placeholder["kind"], "change");
    assert_eq!(placeholder["refs"], 2, "the number of references that name it");
    assert!(doc["edges"].as_array().unwrap().iter().any(|e| e["to"] == "change:ghost-change" && e["rel"] == "change_id"));
    let by_name = data(&search_in(r, &["ghost-change"]));
    assert!(by_name["hits"].as_array().unwrap().iter().all(|h| h["placeholder"] == false && h["id"] != "change:ghost-change"), "no query finds a placeholder: {by_name}");
}

/// search.graph.02
#[test]
fn search_graph_02_tracing_a_placeholder_id_exits_2() {
    let dir = repo();
    let r = dir.path();
    ghost_findings(r);
    for operand in ["ghost-change", "change:ghost-change"] {
        let out = trace_in(r, operand);
        assert_eq!(out.status.code(), Some(2), "{operand}: {}", text(&out));
        let message = json_of(&out)["error"]["message"].as_str().unwrap().to_string();
        assert!(message.contains("review id") && message.contains("commit sha") && message.contains("finding key"), "{message}");
    }
}

/// search.graph.03
#[test]
fn search_graph_03_the_placeholder_ids_are_the_distinct_ids_canon_query_reports_as_having_no_source() {
    let dir = repo();
    let r = dir.path();
    let actor = json!({"agent_id": "t", "role": "reviewer"});
    put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "change_id": "ghost-change", "disposition": "open", "kind": "finding", "reviewer": "t", "review_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV", "round": 1, "schema": 2, "seq": 1, "severity": "note", "summary": "danglerword finding"}));
    put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "change_id": "ghost-change", "disposition": "open", "kind": "finding", "reviewer": "t", "round": 1, "schema": 2, "seq": 2, "severity": "note", "summary": "danglerword again"}));
    put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "evidence": {"kind": "test-run", "ref": "x"}, "evidence_note": {"summary": "danglerword evidence"}, "kind": "evidence", "project_id": "root", "run_seq": 1, "schema": 1, "task_id": "ghost-change#1", "verdict": "faithful"}));
    put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "evidence": {"kind": "test-run", "ref": "x"}, "evidence_note": {"summary": "danglerword evidence two"}, "kind": "evidence", "project_id": "root", "run_seq": 2, "scenario_id": "ghost.scenario.01", "schema": 1, "verdict": "faithful"}));
    // Change records are git-ignored here as in this repo, so the record below is no source.
    let ignore = std::fs::read_to_string(r.join(".gitignore")).unwrap();
    std::fs::write(r.join(".gitignore"), format!("{ignore}.canon/ledger/kind=change/\n")).unwrap();
    // A record for the id exists, but nothing committed holds it: query counts it as having no source.
    put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "change_id": "ghost-change", "kind": "change", "schema": 2, "status": "proposed", "subject_ids": [], "summary": "a derived change record", "title": "ghost-change"}));
    put(r, json!({"actor": actor, "at": "2026-08-01T00:00:00Z", "change_ids": ["ghost-change-two"], "domain": "dev", "kind": "subject", "owner_role": "implementer", "schema": 1, "status": "building", "subject_id": "ghost-subject", "summary": "", "title": "danglerword subject"}));

    let mut reported: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for kind in ["finding", "evidence", "subject"] {
        let out = canon(&["query", "--kind", kind, "--json"], r);
        assert!(out.status.success(), "{}", text(&out));
        for reference in json_of(&out)["data"]["dangling"]["references"].as_array().unwrap() {
            reported.insert(reference["id"].as_str().unwrap().to_string());
        }
    }
    assert_eq!(reported.len(), 5, "{reported:?}");

    let doc = data(&search_in(r, &["danglerword", "--limit", "50"]));
    let placeholders: std::collections::BTreeSet<String> = doc["neighbors"].as_array().unwrap().iter().filter(|n| n["placeholder"] == true).map(|n| n["id"].as_str().unwrap().split_once(':').unwrap().1.to_string()).collect();
    assert_eq!(placeholders, reported);
}
