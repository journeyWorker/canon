//! `canon-0-40-ci-evidence` through the real binary (F1-F6 of the brief): a batch recorded with `--from-ci` carries the CI run it came
//! from (`ci`: provider, run, attempt, address, repository, sha, and each report's digest and case counts), every `--report` batch
//! says how many cases each report held, and `spec_coverage.require_ci_run` accepts only a complete CI record of the code as it
//! stands now. F7 (the skill recipe, the CHANGELOG and the generated blocks) is `ci_evidence_docs.rs`.
//!
//! Each test is named by the id of the scenario it proves (`evidence.ci-run.NN`, `gate.ci-run.NN`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

/// What GitHub Actions sets, and what `--from-ci` reads.
const CI_VARIABLES: [&str; 8] = ["GITHUB_ACTIONS", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_SERVER_URL", "GITHUB_REPOSITORY", "GITHUB_SHA", "GITHUB_WORKFLOW", "GITHUB_JOB"];
const CI_SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const REPORT: &str = "target/ci/junit.xml";
const FEATURE: &str = ".canon/specs/kind=feature/area=shop/cart.feature";

// ── The binary ──

/// `canon <args>` in `repo` with exactly the GitHub variables of `env` (a real CI's own never leak in).
fn canon_env(repo: &Path, args: &[&str], env: &[(&str, String)]) -> Output {
    let mut command = hermetic::canon();
    command.args(args).env("CANON_ACTOR", "canon").current_dir(repo);
    for name in CI_VARIABLES {
        command.env_remove(name);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("spawn canon")
}

fn canon(repo: &Path, args: &[&str]) -> Output {
    canon_env(repo, args, &[])
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn text(output: &Output) -> String {
    format!("{}{}", stdout(output), String::from_utf8_lossy(&output.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}", text(&out));
    out
}

fn json_of(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", text(output)))
}

/// The GitHub variables of one run at `attempt`.
fn ci_env(attempt: &str) -> Vec<(&'static str, String)> {
    vec![
        ("GITHUB_ACTIONS", "true".into()),
        ("GITHUB_RUN_ID", "7001".into()),
        ("GITHUB_RUN_ATTEMPT", attempt.into()),
        ("GITHUB_SERVER_URL", "https://github.com".into()),
        ("GITHUB_REPOSITORY", "acme/shop".into()),
        ("GITHUB_SHA", CI_SHA.into()),
        ("GITHUB_WORKFLOW", "test".into()),
        ("GITHUB_JOB", "offline".into()),
    ]
}

// ── git and files ──

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(repo).output().expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn commit_all(repo: &Path) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "x"]);
}

fn write(repo: &Path, path: &str, text: &str) {
    let path = repo.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn files_under(root: &Path, needle: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, needle: &str, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, needle, out);
            } else if path.to_string_lossy().contains(needle) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, needle, &mut out);
    out.sort();
    out
}

/// Every file under `.canon` that names evidence (staged, committed, stored): what a refusal must leave as it was.
fn evidence_files(repo: &Path) -> Vec<PathBuf> {
    files_under(&repo.join(".canon"), "evidence")
}

/// The committed evidence records of `scenario` (every one when `None`), oldest first.
fn records(repo: &Path, scenario: Option<&str>) -> Vec<Value> {
    let mut all: Vec<Value> = files_under(&repo.join(".canon/ledger/kind=evidence"), ".json").into_iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect();
    all.retain(|r| scenario.is_none_or(|id| r["scenario_id"] == id));
    all.sort_by(|a, b| a["at"].as_str().cmp(&b["at"].as_str()));
    all
}

fn sha256_of(repo: &Path, path: &str) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(repo.join(path)).unwrap()))
}

fn head(repo: &Path) -> String {
    git(repo, &["rev-parse", "HEAD"])
}

// ── The shop repo: evidence.ci-run.* ──

fn shop_feature() -> String {
    let mut text = String::from("Feature: shop cart\n  # canon: {\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}\n");
    for n in 1..=6 {
        text.push_str(&format!(
            "\n  # canon: {{\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{{\"agent_id\":\"canon\"}}}}\n  @shop.cart.0{n}\n  Scenario: Cart behavior number {n}\n    When the shopper does thing {n}\n    Then the cart shows result {n}\n"
        ));
    }
    text
}

/// A committed repo whose `target/` is gitignored and whose spec root `root` holds `shop.cart.01`..`06`, indexed; its work tree is clean.
fn shop_repo() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    write(repo, ".gitignore", "target/\n");
    write(repo, "src/lib.rs", "pub fn cart() {}\n");
    commit_all(repo);
    write(repo, FEATURE, &shop_feature());
    ok(repo, &["inventory", "sync"]);
    dir
}

/// Five cases: three pass (two of scenarios, one with no scenario id), one fails, one is skipped.
const SUITE: &str = r#"<testsuite>
  <testcase classname="cart" name="shop.cart.01 adds"/>
  <testcase classname="cart" name="shop.cart.02 refuses"><failure/></testcase>
  <testcase classname="cart" name="shop.cart.02 counts"/>
  <testcase classname="cart" name="shop.cart.03 is not run"><skipped/></testcase>
  <testcase classname="util" name="helper works"/>
</testsuite>"#;

fn batch_args<'a>(reports: &[&'a str], extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["evidence", "add", "--project", "root", "--all-scenarios", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor", "ci-bot"];
    for report in reports {
        args.extend(["--report", report]);
    }
    args.extend_from_slice(extra);
    args
}

/// The batch over `junit:<path>` with `--from-ci` in the environment of run 7001 at `attempt`.
fn ci_batch(repo: &Path, path: &str, attempt: &str, extra: &[&str]) -> Output {
    let report = format!("junit:{path}");
    let mut args = batch_args(&[&report], &["--from-ci"]);
    args.extend_from_slice(extra);
    canon_env(repo, &args, &ci_env(attempt))
}

fn plain_batch(repo: &Path, path: &str, extra: &[&str]) -> Output {
    let report = format!("junit:{path}");
    canon(repo, &batch_args(&[&report], extra))
}

fn error_code(output: &Output) -> String {
    let body = json_of(output);
    assert_eq!(body["ok"], false, "{body}");
    body["error"]["code"].as_str().unwrap().to_string()
}

/// evidence.ci-run.01 (F1): every record of the batch carries the one `ci` block the environment and the report give.
#[test]
fn evidence_ci_run_01_every_record_of_a_from_ci_batch_carries_the_same_ci_block() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, REPORT, SUITE);

    let out = ci_batch(repo, REPORT, "1", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));

    let all = records(repo, None);
    let mut ids: Vec<&str> = all.iter().map(|r| r["scenario_id"].as_str().unwrap()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["shop.cart.01", "shop.cart.02", "shop.cart.03"], "one record per scenario that owns a case");
    let expected = json!({
        "provider": "github-actions",
        "run": "7001",
        "attempt": 1,
        "url": "https://github.com/acme/shop/actions/runs/7001/attempts/1",
        "repository": "acme/shop",
        "sha": CI_SHA,
        "workflow": "test",
        "job": "offline",
        "reports": [{"path": REPORT, "sha256": sha256_of(repo, REPORT), "format": "junit", "passed": 3, "failed": 1, "skipped": 1}],
    });
    for record in &all {
        assert_eq!(record["ci"], expected, "{record}");
        assert_eq!(record["evidence"]["ref"], "npm test", "the suite's command line, as given");
    }
    assert_ne!(all[0]["evidence_sha"], CI_SHA, "the CI's sha is its claim: evidence_sha stays the commit canon bound");
    assert_eq!(all[0]["evidence_sha"], head(repo).as_str());
    assert_eq!(all[0]["worktree_dirty"], false);

    // The workflow and the job are written only when the environment has them.
    let mut bare = ci_env("2");
    bare.retain(|(name, _)| !matches!(*name, "GITHUB_WORKFLOW" | "GITHUB_JOB"));
    let report = format!("junit:{REPORT}");
    let out = canon_env(repo, &batch_args(&[&report], &["--from-ci"]), &bare);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let latest = records(repo, Some("shop.cart.01")).pop().unwrap();
    assert!(latest["ci"].get("workflow").is_none() && latest["ci"].get("job").is_none(), "{latest}");
}

/// evidence.ci-run.02 (F1): a report no scenario owns a case of is an entry of `ci.reports`, and every case counts.
#[test]
fn evidence_ci_run_02_a_report_without_a_scenario_case_is_still_counted() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, "target/ci/a.xml", r#"<testsuite><testcase name="shop.cart.01 adds"/><testcase name="shop.cart.02 counts"/></testsuite>"#);
    write(
        repo,
        "target/ci/b.xml",
        r#"<testsuite><testcase name="helper adds"/><testcase name="helper removes"><failure/></testcase><testcase name="shop.cart.99 is not indexed"/><testcase name="helper waits"><skipped/></testcase></testsuite>"#,
    );

    let out = canon_env(repo, &batch_args(&["junit:target/ci/a.xml", "junit:target/ci/b.xml"], &["--from-ci"]), &ci_env("1"));
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));

    let all = records(repo, None);
    assert_eq!(all.len(), 2);
    for record in &all {
        let reports = record["ci"]["reports"].as_array().unwrap();
        assert_eq!(reports.len(), 2, "the report that matched no scenario is there too: {record}");
        assert_eq!((reports[0]["path"].as_str(), reports[0]["passed"].as_u64(), reports[0]["failed"].as_u64(), reports[0]["skipped"].as_u64()), (Some("target/ci/a.xml"), Some(2), Some(0), Some(0)));
        assert_eq!((reports[1]["path"].as_str(), reports[1]["passed"].as_u64(), reports[1]["failed"].as_u64(), reports[1]["skipped"].as_u64()), (Some("target/ci/b.xml"), Some(2), Some(1), Some(1)));
        assert_eq!(reports[1]["sha256"], sha256_of(repo, "target/ci/b.xml").as_str());
    }
}

/// evidence.ci-run.03 (F1): the same batch of the same attempt is `unchanged`; another attempt writes new records.
#[test]
fn evidence_ci_run_03_the_same_attempt_is_unchanged_and_another_attempt_is_new() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, REPORT, SUITE);
    assert_eq!(ci_batch(repo, REPORT, "1", &[]).status.code(), Some(0));
    let before = evidence_files(repo);
    assert_eq!(records(repo, None).len(), 3);

    let again = ci_batch(repo, REPORT, "1", &["--json"]);
    assert_eq!(again.status.code(), Some(0), "{}", text(&again));
    let body = json_of(&again);
    let entries = body["data"]["records"].as_array().unwrap();
    assert_eq!(entries.len(), 3, "{body}");
    assert!(entries.iter().all(|e| e["unchanged"] == true), "the same run and the same reports write nothing: {body}");
    assert_eq!(evidence_files(repo), before);

    let rerun = ci_batch(repo, REPORT, "2", &[]);
    assert_eq!(rerun.status.code(), Some(0), "{}", text(&rerun));
    assert!(!text(&rerun).contains("unchanged"), "another attempt is a new record: {}", text(&rerun));
    let after = records(repo, None);
    assert_eq!(after.len(), 3, "promotion keeps one current version per scenario");
    for record in &after {
        assert_eq!((record["ci"]["attempt"].as_u64(), record["ci"]["url"].as_str()), (Some(2), Some("https://github.com/acme/shop/actions/runs/7001/attempts/2")), "{record}");
    }
}

/// evidence.ci-run.04 (F1): a review holds the verdict and the checks, not the run: CI recording the same suite again leaves it current.
#[test]
fn evidence_ci_run_04_a_review_stays_current_when_ci_records_the_same_suite() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, ".canon/policy.yaml", "spec_coverage:\n  require_review:\n    scope: []\n");
    write(repo, REPORT, SUITE);
    let first = plain_batch(repo, REPORT, &[]);
    assert_eq!(first.status.code(), Some(0), "{}", text(&first));
    ok(repo, &["review", "scenario", "shop.cart.01", "--project", "root", "--actor", "reviewer-2", "--role", "reviewer", "--session", "sess-review", "--upstream-ref", "tests/cart.rs#shop_cart_01"]);
    commit_all(repo);
    let gate = || text(&canon(repo, &["gate", "check"]));
    assert!(!gate().contains("unreviewed-promotion shop.cart.01"), "the review is current: {}", gate());

    let ci = ci_batch(repo, REPORT, "1", &[]);
    assert_eq!(ci.status.code(), Some(0), "{}", text(&ci));
    assert!(records(repo, Some("shop.cart.01")).pop().unwrap().get("ci").is_some(), "the newest record is the CI's");
    assert!(!text(&ci).contains("stale"), "no stale-review notice for a record that says the same: {}", text(&ci));
    assert!(!gate().contains("unreviewed-promotion shop.cart.01"), "the review is still current: {}", gate());
}

/// evidence.ci-run.05 (F1, failure path): `--from-ci` is for the whole batch and names its command.
#[test]
fn evidence_ci_run_05_from_ci_needs_all_scenarios_and_an_explicit_ref() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, REPORT, SUITE);
    let before = evidence_files(repo);
    let report = format!("junit:{REPORT}");
    let one_scenario = ["evidence", "add", "--project", "root", "--scenario", "shop.cart.01", "--from-ci", "--ref", "npm test", "--report", &report, "--role", "implementer", "--json"];
    let no_ref = ["evidence", "add", "--project", "root", "--all-scenarios", "--from-ci", "--report", &report, "--role", "implementer", "--json"];
    for (what, args) in [("a single scenario", &one_scenario[..]), ("no --ref", &no_ref[..])] {
        let out = canon_env(repo, args, &ci_env("1"));
        assert_eq!(out.status.code(), Some(2), "{what}: {}", text(&out));
        assert_eq!(error_code(&out), "EVIDENCE_ARGUMENT_REQUIRED", "{what}: {}", text(&out));
        assert_eq!(evidence_files(repo), before, "{what}: nothing is written");
    }
}

/// evidence.ci-run.06 (F2, failure path): outside GitHub Actions the batch is refused, naming what is missing.
#[test]
fn evidence_ci_run_06_from_ci_outside_github_actions_is_ci_env_missing() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, REPORT, SUITE);
    let before = evidence_files(repo);
    let report = format!("junit:{REPORT}");

    let mut args = batch_args(&[&report], &["--from-ci", "--json"]);
    let none = canon_env(repo, &args, &[]);
    assert_eq!(none.status.code(), Some(2), "{}", text(&none));
    assert_eq!(error_code(&none), "CI_ENV_MISSING");
    assert!(json_of(&none)["error"]["message"].as_str().unwrap().contains("GITHUB_ACTIONS"), "{}", text(&none));

    let mut empty_run = ci_env("1");
    empty_run.iter_mut().find(|(name, _)| *name == "GITHUB_RUN_ID").unwrap().1.clear();
    let empty = canon_env(repo, &args, &empty_run);
    assert_eq!(empty.status.code(), Some(2), "{}", text(&empty));
    assert_eq!(error_code(&empty), "CI_ENV_MISSING");
    let message = json_of(&empty)["error"]["message"].as_str().unwrap().to_string();
    assert!(message.contains("GITHUB_RUN_ID") && !message.contains("GITHUB_ACTIONS"), "only the variable that is missing: {message}");

    let mut not_ci = ci_env("1");
    not_ci.iter_mut().find(|(name, _)| *name == "GITHUB_ACTIONS").unwrap().1 = "false".into();
    let other = canon_env(repo, &args, &not_ci);
    assert_eq!(error_code(&other), "CI_ENV_MISSING", "GITHUB_ACTIONS must be `true`");

    // Without --from-ci the same environment is no business of canon's: the record names no run.
    args.retain(|a| *a != "--from-ci" && *a != "--json");
    assert_eq!(evidence_files(repo), before, "every refusal wrote nothing");
    let plain = canon_env(repo, &args, &ci_env("1"));
    assert_eq!(plain.status.code(), Some(0), "{}", text(&plain));
    assert!(records(repo, None).iter().all(|r| r.get("ci").is_none()), "a CI shell does not make a person's record a CI record");
}

/// evidence.ci-run.07 (F2, failure path): a work tree that differs from HEAD is refused, and the gitignored report is the way out.
#[test]
fn evidence_ci_run_07_from_ci_on_a_dirty_work_tree_is_ci_worktree_dirty() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, "junit.xml", SUITE);
    let before = evidence_files(repo);

    let out = ci_batch(repo, "junit.xml", "1", &["--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert_eq!(error_code(&out), "CI_WORKTREE_DIRTY");
    let hint = json_of(&out)["error"]["hint"].as_str().unwrap().to_string();
    assert!(hint.contains("gitignored") && hint.contains(".canon/") && hint.contains("code.exclude"), "{hint}");
    assert_eq!(evidence_files(repo), before, "nothing is written");
}

/// evidence.ci-run.08 (F2): a report in a gitignored directory passes, and without `--from-ci` a dirty work tree is recorded as in 0.39.
#[test]
fn evidence_ci_run_08_a_gitignored_report_passes_and_a_plain_batch_still_records_a_dirty_tree() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, "junit.xml", SUITE);
    let plain = plain_batch(repo, "junit.xml", &[]);
    assert_eq!(plain.status.code(), Some(0), "{}", text(&plain));
    let written = records(repo, None);
    assert_eq!(written.len(), 3);
    assert!(written.iter().all(|r| r["worktree_dirty"] == true && r.get("ci").is_none()), "recorded as before: dirty, and no run named: {written:?}");

    std::fs::remove_file(repo.join("junit.xml")).unwrap();
    write(repo, REPORT, SUITE);
    let ci = ci_batch(repo, REPORT, "1", &[]);
    assert_eq!(ci.status.code(), Some(0), "{}", text(&ci));
    let latest = records(repo, Some("shop.cart.01")).pop().unwrap();
    assert_eq!((latest["worktree_dirty"].as_bool(), latest["ci"]["attempt"].as_u64()), (Some(false), Some(1)));
}

/// evidence.ci-run.09 (F2, failure path): the commit and the verdict of a CI record are not the caller's to name.
#[test]
fn evidence_ci_run_09_from_ci_refuses_rev_and_verdict() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, REPORT, SUITE);
    let before = evidence_files(repo);
    let sha = head(repo);
    for extra in [vec!["--rev", sha.as_str()], vec!["--verdict", "faithful"]] {
        let mut args = vec!["--json"];
        args.extend(extra.iter().copied());
        let out = ci_batch(repo, REPORT, "1", &args);
        assert_eq!(out.status.code(), Some(2), "{extra:?}: {}", text(&out));
        assert_eq!(error_code(&out), "EVIDENCE_ARGUMENT_CONFLICT", "{extra:?}");
        assert_eq!(evidence_files(repo), before, "{extra:?}: nothing is written");
    }
}

/// evidence.ci-run.10 (F3): each report of a batch is one line of counts and one entry of `data.reports[]`, `--from-ci` or not.
#[test]
fn evidence_ci_run_10_a_batch_says_how_many_cases_each_report_held() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, "reports/a.xml", SUITE);
    write(repo, "reports/b.xml", r#"<testsuite><testcase name="shop.cart.04 counts"/><testcase name="helper works"/></testsuite>"#);

    let out = canon(repo, &batch_args(&["junit:reports/a.xml", "junit:reports/b.xml"], &["--json"]));
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let reports = json_of(&out)["data"]["reports"].clone();
    assert_eq!(
        reports,
        json!([
            {"path": "reports/a.xml", "sha256": sha256_of(repo, "reports/a.xml"), "format": "junit", "passed": 3, "failed": 1, "skipped": 1},
            {"path": "reports/b.xml", "sha256": sha256_of(repo, "reports/b.xml"), "format": "junit", "passed": 2, "failed": 0, "skipped": 0},
        ])
    );
    assert!(!text(&out).contains(" passed, "), "in --json mode the counts are data, not lines: {}", text(&out));

    // The human form prints one line per report as well.
    let again = canon(repo, &batch_args(&["junit:reports/a.xml", "junit:reports/b.xml"], &[]));
    let lines: Vec<String> = text(&again).lines().filter(|l| l.contains(" passed, ")).map(str::to_string).collect();
    assert_eq!(lines, ["canon evidence add: reports/a.xml: 3 passed, 1 failed, 1 skipped", "canon evidence add: reports/b.xml: 2 passed, 0 failed, 0 skipped"]);
    // Nothing about the counts is recorded without --from-ci.
    assert!(records(repo, None).iter().all(|r| r.get("ci").is_none()));

    // A batch every record of which is unchanged says it too.
    let unchanged = json_of(&canon(repo, &batch_args(&["junit:reports/a.xml", "junit:reports/b.xml"], &["--json"])));
    assert_eq!(unchanged["data"]["reports"], reports);
}

/// evidence.ci-run.11 (F3, failure path): a report that cannot be parsed is refused as before: no counts, nothing written.
#[test]
fn evidence_ci_run_11_an_unparseable_report_is_refused_without_counts() {
    let dir = shop_repo();
    let repo = dir.path();
    write(repo, "reports/good.xml", SUITE);
    write(repo, "reports/bad.xml", "<testsuite><testcase name=\"shop.cart.01 adds\"");
    let before = evidence_files(repo);

    let out = canon(repo, &batch_args(&["junit:reports/good.xml", "junit:reports/bad.xml"], &[]));
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("reports/bad.xml is not a readable JUnit XML report"), "{}", text(&out));
    assert!(!text(&out).contains(" passed, "), "no counts for a refused batch: {}", text(&out));
    assert_eq!(evidence_files(repo), before, "nothing is written");
}

// ── The gate repo: gate.ci-run.* ──

const GATE_FEATURE_IDS: [&str; 2] = ["game.run.01", "game.run.02"];

/// A report of `n` passing cases: one for each scenario of the repo and `n - 2` with no scenario id.
fn suite_of(n: usize) -> String {
    let mut text = String::from("<testsuite>\n  <testcase name=\"game.run.01: health below zero keeps the run alive\"/>\n  <testcase name=\"game.run.02: a restart clears the score\"/>\n");
    for i in 0..n.saturating_sub(2) {
        text.push_str(&format!("  <testcase name=\"helper {i}\"/>\n"));
    }
    text.push_str("</testsuite>");
    text
}

/// A committed repo with the subject `run` at `verifying` owning `game.run.01` and `game.run.02`, under `policy` (none when empty).
fn gate_repo(policy: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    write(repo, ".gitignore", "target/\n");
    write(repo, "src/lib.rs", "pub fn run() {}\n");
    commit_all(repo);
    ok(repo, &["subject", "new", "run", "--domain", "dev", "--title", "A run", "--actor", "lead"]);
    for step in ["specced", "building", "verifying"] {
        ok(repo, &["subject", "move", "run", "--to", step, "--actor", "lead"]);
    }
    for id in GATE_FEATURE_IDS {
        ok(repo, &["scenario", "new", id, "--title", "A run ends at zero health", "--case", "failure", "--subject", "run"]);
    }
    ok(repo, &["inventory", "sync"]);
    if !policy.is_empty() {
        write(repo, ".canon/policy.yaml", policy);
    }
    write(repo, REPORT, &suite_of(2));
    dir
}

const REQUIRE: &str = "spec_coverage:\n  require_ci_run: {}\n";

/// The batch of `target/ci/junit.xml` as CI attempt `attempt`.
fn ci_record(repo: &Path, attempt: &str) {
    let report = format!("junit:{REPORT}");
    let out = canon_env(repo, &batch_args(&[&report], &["--from-ci"]), &ci_env(attempt));
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
}

/// A person's record of `scenario` (no report, no run named).
fn person_records(repo: &Path, scenario: &str) {
    ok(repo, &["evidence", "add", "--scenario", scenario, "--project", "root", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor", "impl-agent"]);
}

fn approve_brief(repo: &Path) {
    ok(repo, &["change", "new", "c-run", "--subject", "run", "--title", "The brief"]);
    write(repo, ".canon/changes/c-run/brief.md", "# The brief\n\n## Why\n\nThe brief.\n\nApproved: Journey 2026-10-12 — \"승인\"\n");
}

fn ship(repo: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["subject", "move", "run", "--to", "shipped", "--actor", "lead"];
    args.extend_from_slice(extra);
    canon(repo, &args)
}

fn reopen(repo: &Path) {
    ok(repo, &["subject", "move", "run", "--to", "verifying", "--reason", "the evidence is no longer the CI's", "--actor", "lead"]);
}

fn status(repo: &Path) -> Value {
    json_of(&ok(repo, &["status", "--json"]))["data"].clone()
}

fn red_of(repo: &Path) -> Vec<String> {
    let report = status(repo);
    let run = report["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "run").unwrap().clone();
    run.get("red").map(|red| red.as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect()).unwrap_or_default()
}

/// Rewrite the committed record of `scenario` that is newest: `change` edits its body, and the file is renamed by the digest of the new body.
fn rewrite_newest(repo: &Path, scenario: &str, change: impl FnOnce(&mut Value)) {
    let dir = repo.join(".canon/ledger/kind=evidence");
    let mut files: Vec<(Value, PathBuf)> = files_under(&dir, ".json").into_iter().map(|p| (serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap(), p)).collect();
    files.retain(|(body, _)| body["scenario_id"] == scenario);
    files.sort_by(|a, b| a.0["at"].as_str().cmp(&b.0["at"].as_str()));
    let (mut body, path) = files.pop().expect("a record of the scenario");
    change(&mut body);
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let stem = name.rsplit_once('_').unwrap().0;
    let renamed = dir.join(format!("{stem}_{}.json", canon_store::partition::content_digest(&body)));
    std::fs::remove_file(path).unwrap();
    std::fs::write(renamed, serde_json::to_string_pretty(&body).unwrap()).unwrap();
}

/// What `canon gate check` prints for `scenario`: the violation line, when there is one.
fn gate_line(repo: &Path, scenario: &str) -> Option<String> {
    let out = canon(repo, &["gate", "check"]);
    text(&out).lines().find(|l| l.trim_start().starts_with(['u', 's', 'f', 'm']) && l.contains(&format!(" {scenario} — "))).map(|l| l.trim().to_string())
}

/// Ship `run` on a CI record, then make the repo fail `reason` through `spoil`; `class` is the failure class `gate check`, the guard and `status` name.
fn assert_three_readers(policy: &str, reason: &str, class: &str, spoil: impl FnOnce(&Path)) {
    let dir = gate_repo(policy);
    let repo = dir.path();
    ci_record(repo, "1");
    approve_brief(repo);
    let shipped = ship(repo, &[]);
    assert_eq!(shipped.status.code(), Some(0), "a complete CI record of the code now ships: {}", text(&shipped));
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(0), "{}", text(&canon(repo, &["gate", "check"])));

    spoil(repo);

    let line = gate_line(repo, "game.run.01").unwrap_or_else(|| panic!("no violation line: {}", text(&canon(repo, &["gate", "check"]))));
    assert!(line.starts_with(class) && line.contains(&format!("spec_coverage.require_ci_run: {reason}")), "gate check: {line}");
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(1));
    let red = red_of(repo);
    assert!(red.iter().any(|r| r.contains(class) && r.contains("game.run.01")), "status is red with the same class: {red:?}");

    reopen(repo);
    let refused = ship(repo, &[]);
    assert_eq!(refused.status.code(), Some(2), "the guard refuses: {}", text(&refused));
    assert!(text(&refused).contains(&format!("{class} game.run.01 — verifying → shipped: spec_coverage.require_ci_run: {reason}")), "the guard says the same reason: {}", text(&refused));
}

/// gate.ci-run.01 (F4): a complete CI record of the code now passes `gate check` and the ship guard; a scenario with no evidence is the evidence requirement's.
#[test]
fn gate_ci_run_01_a_complete_ci_record_of_the_current_code_passes() {
    let dir = gate_repo(REQUIRE);
    let repo = dir.path();
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(0), "no evidence is no_ci_run's business only once a record exists");
    ci_record(repo, "1");
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "{}", text(&gate));
    assert!(!text(&gate).contains("require_ci_run"), "{}", text(&gate));
    approve_brief(repo);
    let shipped = ship(repo, &[]);
    assert_eq!(shipped.status.code(), Some(0), "{}", text(&shipped));
    assert!(red_of(repo).is_empty(), "a shipped subject on a CI record is green: {:?}", red_of(repo));
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(0), "the scope `[shipped]` keeps asking after the move");
}

/// gate.ci-run.02 (F4, failure path): a person's record newer than the CI's is `no_ci_run` until the CI records again.
#[test]
fn gate_ci_run_02_a_newer_record_of_a_person_is_no_ci_run() {
    assert_three_readers(REQUIRE, "no_ci_run", "uncovered-cell", |repo| person_records(repo, "game.run.01"));

    // The next CI record answers it.
    let dir = gate_repo(REQUIRE);
    let repo = dir.path();
    ci_record(repo, "1");
    person_records(repo, "game.run.01");
    assert!(gate_line(repo, "game.run.01").is_none(), "scope [shipped]: a verifying subject is not asked yet");
    approve_brief(repo);
    assert_eq!(ship(repo, &[]).status.code(), Some(2));
    ci_record(repo, "2");
    assert_eq!(ship(repo, &[]).status.code(), Some(0), "the CI's newer record clears it");
}

/// gate.ci-run.03 (F4, failure path): a CI record of other code is `other_code_tree`, a `stale-evidence`.
#[test]
fn gate_ci_run_03_a_ci_record_of_another_code_tree_is_other_code_tree() {
    assert_three_readers(REQUIRE, "other_code_tree", "stale-evidence", |repo| {
        write(repo, "src/lib.rs", "pub fn run() { /* changed */ }\n");
        commit_all(repo);
    });
}

/// gate.ci-run.04 (F4, failure path): a CI record that says its work tree was dirty is `dirty_worktree` (`--from-ci` never writes one; this is a record of another hand).
#[test]
fn gate_ci_run_04_a_ci_record_of_a_dirty_work_tree_is_dirty_worktree() {
    assert_three_readers(REQUIRE, "dirty_worktree", "uncovered-cell", |repo| rewrite_newest(repo, "game.run.01", |body| body["worktree_dirty"] = json!(true)));
}

/// gate.ci-run.05 (F4, failure path): `min_cases` is compared with `passed + failed` over the record's reports: 99 is short, 100 is whole.
#[test]
fn gate_ci_run_05_min_cases_decides_a_partial_run() {
    let policy = "spec_coverage:\n  require_ci_run:\n    min_cases: 100\n    scope: [verifying, shipped]\n";
    let dir = gate_repo(policy);
    let repo = dir.path();
    write(repo, REPORT, &suite_of(99));
    ci_record(repo, "1");
    let line = gate_line(repo, "game.run.01").expect("a violation");
    assert!(line.starts_with("uncovered-cell") && line.contains("spec_coverage.require_ci_run: short_run") && line.contains("ran 99 case(s) and min_cases is 100"), "{line}");
    approve_brief(repo);
    let refused = ship(repo, &[]);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    assert!(text(&refused).contains("verifying → shipped: spec_coverage.require_ci_run: short_run"), "{}", text(&refused));

    // 60 + 40 cases over two reports are 100.
    write(repo, "target/ci/a.xml", &suite_of(60));
    write(repo, "target/ci/b.xml", &format!("<testsuite>{}</testsuite>", (0..40).map(|i| format!("<testcase name=\"other {i}\"/>")).collect::<String>()));
    let out = canon_env(repo, &batch_args(&["junit:target/ci/a.xml", "junit:target/ci/b.xml"], &["--from-ci"]), &ci_env("2"));
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert!(gate_line(repo, "game.run.01").is_none(), "{}", text(&canon(repo, &["gate", "check"])));
    let shipped = ship(repo, &[]);
    assert_eq!(shipped.status.code(), Some(0), "{}", text(&shipped));
}

/// gate.ci-run.06 (F4, failure path): when git does not give the code tree now, a CI record is never passed as if it were current.
#[test]
fn gate_ci_run_06_an_unknown_code_tree_is_code_tree_unknown() {
    let dir = gate_repo(REQUIRE);
    let repo = dir.path();
    ci_record(repo, "1");
    approve_brief(repo);
    assert_eq!(ship(repo, &[]).status.code(), Some(0));
    assert!(gate_line(repo, "game.run.01").is_none());

    std::fs::rename(repo.join(".git"), repo.join(".git-away")).unwrap();
    let line = gate_line(repo, "game.run.01").unwrap_or_else(|| panic!("no violation: {}", text(&canon(repo, &["gate", "check"]))));
    assert!(line.starts_with("uncovered-cell") && line.contains("spec_coverage.require_ci_run: code_tree_unknown"), "{line}");
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(1));
    let red = red_of(repo);
    assert!(red.iter().any(|r| r.contains("uncovered-cell") && r.contains("game.run.01")), "{red:?}");
}

/// gate.ci-run.07 (F4, failure path): a malformed `require_ci_run` does not switch itself off: it makes `spec_coverage` unusable.
#[test]
fn gate_ci_run_07_a_malformed_policy_makes_spec_coverage_unusable() {
    for (yaml, detail) in [
        ("require_ci_run:\n    min_cases: 0\n", "`require_ci_run.min_cases` must be an integer of 1 or more, got 0"),
        ("require_ci_run:\n    min_cases: -3\n", "`require_ci_run.min_cases` must be an integer of 1 or more, got -3"),
        ("require_ci_run:\n    min_cases: 1.5\n", "`require_ci_run.min_cases` must be an integer of 1 or more, got 1.5"),
        ("require_ci_run:\n    min_cases:\n", "`require_ci_run.min_cases` must be an integer of 1 or more"),
        ("require_ci_run:\n    scope: [done]\n", "`done` is not a valid value for `require_ci_run.scope`"),
        ("require_ci_run:\n    sample: 3\n", "unknown field `sample`"),
        ("require_ci_run:\n", "`require_ci_run` must be a mapping"),
    ] {
        let dir = gate_repo(&format!("spec_coverage:\n  {yaml}"));
        let repo = dir.path();
        let gate = canon(repo, &["gate", "check"]);
        assert_eq!(gate.status.code(), Some(1), "{yaml}: {}", text(&gate));
        let out = text(&gate);
        assert!(out.contains("spec_coverage") && out.contains("unusable") && out.contains(detail), "{yaml}: {out}");
    }
    // A well-formed key is what it says, and `min_cases` with a scope is read.
    let dir = gate_repo("spec_coverage:\n  require_ci_run:\n    min_cases: 5\n    scope: [verifying]\n");
    assert_eq!(canon(dir.path(), &["gate", "check"]).status.code(), Some(0));
}

/// gate.ci-run.08 (F4, failure path): `--override-reason` waives the review checks and nothing else: not this one.
#[test]
fn gate_ci_run_08_an_override_reason_does_not_waive_the_ci_check() {
    let dir = gate_repo(REQUIRE);
    let repo = dir.path();
    ci_record(repo, "1");
    person_records(repo, "game.run.01");
    approve_brief(repo);
    let refused = ship(repo, &["--override-reason", "the owner says ship"]);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    let out = text(&refused);
    assert!(out.contains("verifying → shipped: spec_coverage.require_ci_run: no_ci_run"), "{out}");
    assert!(out.contains("--override-reason waives only the review checks"), "{out}");
    let subject = status(repo)["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "run").unwrap().clone();
    assert_eq!(subject["status"], "verifying", "the subject did not move: {subject}");
}

/// gate.ci-run.09 (F5): `context` and `status` say the requirement in one line, and the level is `custom`.
#[test]
fn gate_ci_run_09_context_and_status_name_the_requirement() {
    let policy = "spec_coverage:\n  require_evidence: true\n  require_ci_run:\n    min_cases: 100\n";
    let dir = gate_repo(policy);
    let repo = dir.path();
    let report = status(repo);
    assert_eq!(report["policy"]["spec_coverage"]["require_ci_run"], json!({"scope": ["shipped"], "min_cases": 100}), "{report}");
    assert_eq!(report["policy"]["level"], "custom");
    let human = text(&ok(repo, &["status"]));
    assert!(human.contains("\n  require_ci_run: scope=shipped min_cases=100\n"), "{human}");

    let context = json_of(&ok(repo, &["context", "--json"]));
    assert_eq!(context["data"]["policy"]["spec_coverage"]["require_ci_run"], json!({"scope": ["shipped"], "min_cases": 100}), "{context}");
    let outline = text(&ok(repo, &["context"]));
    assert!(outline.contains("require_ci_run=(scope=shipped min_cases=100)"), "{outline}");

    // `{}` is every default: the scope, no minimum.
    write(repo, ".canon/policy.yaml", "spec_coverage:\n  require_ci_run: {}\n");
    assert_eq!(status(repo)["policy"]["spec_coverage"]["require_ci_run"], json!({"scope": ["shipped"]}));
}

/// gate.ci-run.10 (F5, failure path): without the policy there is no line and no key.
#[test]
fn gate_ci_run_10_no_policy_no_require_ci_run_line() {
    for policy in ["", "spec_coverage:\n  require_evidence: true\n"] {
        let dir = gate_repo(policy);
        let repo = dir.path();
        let report = status(repo);
        assert!(report["policy"]["spec_coverage"].get("require_ci_run").is_none(), "{report}");
        assert!(!text(&ok(repo, &["status"])).contains("require_ci_run"));
        assert!(!text(&ok(repo, &["context"])).contains("require_ci_run"));
        assert!(json_of(&ok(repo, &["context", "--json"]))["data"]["policy"]["spec_coverage"].get("require_ci_run").is_none());
    }
}

/// gate.ci-run.11 (F6): without the policy a CI record changes no verdict, and a record written without `--from-ci` carries no `ci` key.
#[test]
fn gate_ci_run_11_without_the_policy_the_answers_are_those_of_0_39() {
    let dir = gate_repo("");
    let repo = dir.path();
    person_records(repo, "game.run.01");
    person_records(repo, "game.run.02");
    let keys: Vec<String> = records(repo, Some("game.run.01"))[0].as_object().unwrap().keys().cloned().collect();
    let known_in_0_39 = ["actor", "at", "code_tree", "evidence", "evidence_sha", "kind", "project_id", "run_id", "run_seq", "scenario_id", "schema", "staging_id", "verdict", "worktree_dirty"];
    assert!(keys.iter().all(|key| known_in_0_39.contains(&key.as_str())), "the keys of a record written without --from-ci are the ones 0.39 writes: {keys:?}");
    assert!(!keys.contains(&"ci".to_string()), "{keys:?}");
    approve_brief(repo);
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(0));
    let shipped = ship(repo, &[]);
    assert_eq!(shipped.status.code(), Some(0), "a person's records ship it, as before: {}", text(&shipped));

    ci_record(repo, "1");
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(0), "a CI record asks nothing more of a repo that did not ask");
    assert!(red_of(repo).is_empty());
}

/// gate.ci-run.12 (F6, failure path): a `ci` of the wrong shape makes the record unreadable, reported as any malformed record; a `ci` with one more key is read.
#[test]
fn gate_ci_run_12_a_malformed_ci_is_a_malformed_record_and_an_unknown_key_is_read() {
    let dir = gate_repo("");
    let repo = dir.path();
    ci_record(repo, "1");
    let read = |repo: &Path| json_of(&ok(repo, &["query", "--kind", "evidence", "--json"]))["data"].clone();
    assert_eq!(read(repo)["violations"], 0, "{}", read(repo));

    rewrite_newest(repo, "game.run.01", |body| body["ci"]["future"] = json!({"signature": "x"}));
    let extra = read(repo);
    assert_eq!((extra["violations"].as_u64(), extra["count"].as_u64()), (Some(0), Some(2)), "a key a later version adds is ignored: {extra}");
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(0));

    rewrite_newest(repo, "game.run.01", |body| body["ci"]["attempt"] = json!("two"));
    let broken = read(repo);
    assert_eq!(broken["violations"], 1, "{broken}");
    assert_eq!(broken["count"], 1, "the record is not read: {broken}");
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    assert!(text(&gate).contains("malformed-evidence"), "{}", text(&gate));
}
