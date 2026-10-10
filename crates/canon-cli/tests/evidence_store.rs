//! 0.14 D4/D5 through the real binary: bound evidence stays provable
//! (the artifact store, `canon evidence vault`, the gate's re-hash), and
//! evidence is not shaped by tasks (scenario-only summaries, repeatable
//! report cases, aggregated task rows, task status in the record store,
//! and one task+scenario record answering both joins).

use std::path::Path;
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}", text(&out));
    out
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn blob(repo: &Path, bytes: &[u8]) -> std::path::PathBuf {
    repo.join(".canon/artifacts/sha256").join(sha(bytes))
}

/// `git init` + `canon init` + one scenario, synced. With `plan`, the
/// repo also gets an openspec plan source holding change `cats` with one
/// open task.
fn setup(plan: bool) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap().success());
    ok(repo, &["init", "--repo", "."]);
    if plan {
        let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
        let yaml = yaml.replace("plans:\n  sources: []", "plans:\n  sources:\n    - dialect: openspec\n      root: openspec/changes");
        assert!(yaml.contains("root: openspec/changes"), "the plan source must be configured:\n{yaml}");
        std::fs::write(repo.join("canon.yaml"), yaml).unwrap();
        let change = repo.join("openspec/changes/cats");
        std::fs::create_dir_all(&change).unwrap();
        std::fs::write(change.join("proposal.md"), "# cats\n\n## Why\n\nA cat-themed run.\n").unwrap();
        std::fs::write(change.join("tasks.md"), "## 1. Run\n\n- [ ] 1 Ship the run\n").unwrap();
    }
    dir
}

fn scenario(repo: &Path, id: &str, subject: Option<&str>) {
    let mut args = vec!["scenario", "new", id, "--title", "A run ends at zero health", "--case", "failure"];
    if let Some(subject) = subject {
        args.extend(["--subject", subject]);
    }
    ok(repo, &args);
    ok(repo, &["inventory", "sync"]);
}

fn attest(repo: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "test-run", "--ref", "npm run smoke", "--role", "implementer"];
    args.extend_from_slice(extra);
    let out = canon(repo, &args);
    if out.status.success() {
        ok(repo, &["gate", "promote"]);
    }
    out
}

fn committed_records(repo: &Path) -> Vec<serde_json::Value> {
    let dir = repo.join(".canon/ledger/kind=evidence_record");
    let mut paths: Vec<_> = std::fs::read_dir(dir).map(|d| d.filter_map(Result::ok).map(|e| e.path()).collect()).unwrap_or_default();
    paths.sort();
    paths.iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect()
}

/// Dogfood F9: a smoke script rewrote `reports/smoke.json` after it was
/// bound, and the gate stayed clean on bytes nobody could produce. Now
/// the bound bytes are stored, so a rewrite costs nothing; when the
/// stored blob is lost too, the record is `stale-evidence`.
#[test]
fn a_stored_blob_keeps_a_rewritten_report_provable_and_a_lost_one_is_stale_evidence() {
    let dir = setup(false);
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    std::fs::create_dir_all(repo.join("reports")).unwrap();
    let v1 = br#"{"at":"2026-10-10T14:00:00Z","checks":16}"#;
    std::fs::write(repo.join("reports/smoke.json"), v1).unwrap();

    let added = attest(repo, &["--artifact", "reports/smoke.json"]);
    assert!(added.status.success(), "{}", text(&added));
    assert!(text(&added).contains(".canon/artifacts/sha256/"), "the add names the store: {}", text(&added));
    assert_eq!(std::fs::read(blob(repo, v1)).unwrap(), v1, "the bound bytes are stored under their digest");

    // The smoke script runs again: the working tree changes, the gate
    // still proves the record from the store.
    std::fs::write(repo.join("reports/smoke.json"), br#"{"at":"2026-10-10T14:30:00Z","checks":16}"#).unwrap();
    let clean = canon(repo, &["gate", "check"]);
    assert!(clean.status.success(), "{}", text(&clean));
    assert!(!text(&clean).contains("stale-evidence") && !text(&clean).contains("unstored"), "{}", text(&clean));

    // The blob is lost as well: the bytes are gone.
    std::fs::remove_file(blob(repo, v1)).unwrap();
    let v3 = br#"{"at":"2026-10-10T15:00:00Z","checks":15}"#;
    std::fs::write(repo.join("reports/smoke.json"), v3).unwrap();
    let stale = canon(repo, &["gate", "check"]);
    assert_eq!(stale.status.code(), Some(1), "{}", text(&stale));
    let out = text(&stale);
    assert!(out.contains("stale-evidence game.run.01") && out.contains("`reports/smoke.json`"), "{out}");
    assert!(out.contains(&sha(v1)) && out.contains(&sha(v3)) && out.contains(": missing"), "names the path and both digests: {out}");
}

/// A record written before the store existed (0.11–0.13) is proven
/// from the working tree, with an advisory; `canon evidence vault`
/// stores its bytes and the advisory goes away.
#[test]
fn an_unstored_binding_is_an_advisory_until_canon_evidence_vault_stores_it() {
    let dir = setup(false);
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    std::fs::create_dir_all(repo.join("reports")).unwrap();
    let bytes = b"<testsuite><testcase name=\"game.run.01 ends the run\"/></testsuite>";
    std::fs::write(repo.join("reports/junit.xml"), bytes).unwrap();
    assert!(attest(repo, &["--report", "junit:reports/junit.xml"]).status.success());
    std::fs::remove_dir_all(repo.join(".canon/artifacts")).unwrap();

    let advisory = canon(repo, &["gate", "check"]);
    assert!(advisory.status.success(), "{}", text(&advisory));
    assert!(text(&advisory).contains("canon evidence vault") && text(&advisory).contains("unstored game.run.01 reports/junit.xml"), "{}", text(&advisory));

    let vault = ok(repo, &["evidence", "vault"]);
    assert!(text(&vault).contains("stored 1, already stored 0, could not store 0"), "{}", text(&vault));
    assert_eq!(std::fs::read(blob(repo, bytes)).unwrap(), bytes);
    let quiet = canon(repo, &["gate", "check"]);
    assert!(quiet.status.success() && !text(&quiet).contains("unstored"), "{}", text(&quiet));
    let again = ok(repo, &["evidence", "vault"]);
    assert!(text(&again).contains("stored 0, already stored 1"), "the vault is idempotent: {}", text(&again));
}

#[test]
fn evidence_add_refuses_a_bound_file_over_the_size_limit_and_names_the_override() {
    let dir = setup(false);
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    let big = vec![b'x'; 1024 * 1024 + 1];
    std::fs::write(repo.join("trace.bin"), &big).unwrap();

    let refused = attest(repo, &["--artifact", "trace.bin", "--max-artifact-mib", "1"]);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    assert!(text(&refused).contains("--max-artifact-mib"), "{}", text(&refused));
    assert!(committed_records(repo).is_empty() && !repo.join(".canon/artifacts").exists(), "a refused add writes nothing");

    let raised = attest(repo, &["--artifact", "trace.bin", "--max-artifact-mib", "2"]);
    assert!(raised.status.success(), "{}", text(&raised));
    assert!(blob(repo, &big).exists());
}

/// `canon init` must never ignore the store: the bytes are authored
/// evidence, committed with the ledger.
#[test]
fn canon_init_never_gitignores_the_artifact_store() {
    let dir = setup(false);
    let ignored = Command::new("git").args(["check-ignore", "-q", ".canon/artifacts/sha256/0000"]).current_dir(dir.path()).status().unwrap();
    assert_eq!(ignored.code(), Some(1), "`.canon/artifacts/` must be tracked");
}

/// Dogfood F8: a summary on scenario-only evidence was refused, so the
/// agent invented a scenario→task mapping to keep its notes.
#[test]
fn a_summary_is_kept_on_scenario_only_evidence_and_still_scanned_for_fabrication() {
    let dir = setup(false);
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    let added = attest(repo, &["--summary", "3 vitest cases and 1 smoke check passed"]);
    assert!(added.status.success(), "{}", text(&added));
    assert_eq!(committed_records(repo)[0]["evidence_note"]["summary"], "3 vitest cases and 1 smoke check passed");

    let fabricated = attest(repo, &["--summary", "TBD after the review"]);
    assert_eq!(fabricated.status.code(), Some(1), "{}", text(&fabricated));
    assert!(text(&fabricated).contains("fabricated-evidence game.run.01"), "{}", text(&fabricated));
}

/// Dogfood F15: one `--report-case` hid the other cases for the same
/// scenario. Every case carrying the id is bound, named cases are added,
/// and a failed one refuses `faithful`.
#[test]
fn every_report_case_carrying_the_scenario_id_is_bound_and_any_failure_refuses_faithful() {
    let dir = setup(false);
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    std::fs::create_dir_all(repo.join("reports")).unwrap();
    std::fs::write(
        repo.join("reports/junit.xml"),
        r#"<testsuite><testcase name="game.run.01: hp hits zero"/><testcase name="game.run.01: result screen shows"/><testcase name="hud_survives_restart"/></testsuite>"#,
    )
    .unwrap();
    let added = attest(repo, &["--report", "junit:reports/junit.xml", "--report-case", "hud_survives_restart"]);
    assert!(added.status.success(), "{}", text(&added));
    let cases: Vec<String> = committed_records(repo)[0]["attachments"].as_array().unwrap().iter().map(|a| a["case"].as_str().unwrap().to_string()).collect();
    assert_eq!(cases, ["game.run.01: hp hits zero", "game.run.01: result screen shows", "hud_survives_restart"]);

    std::fs::write(
        repo.join("reports/junit.xml"),
        r#"<testsuite><testcase name="game.run.01: hp hits zero"/><testcase name="game.run.01: result screen shows"><failure/></testcase><testcase name="hud_survives_restart"/></testsuite>"#,
    )
    .unwrap();
    let refused = attest(repo, &["--report", "junit:reports/junit.xml", "--report-case", "hud_survives_restart"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    assert!(text(&refused).contains("game.run.01: result screen shows` as failed"), "{}", text(&refused));
}

fn task_records(repo: &Path) -> serde_json::Value {
    let out = ok(repo, &["query", "--kind", "task", "--json"]);
    serde_json::from_slice(&out.stdout).unwrap()
}

/// Dogfood F8/F14: the flipped row named one of nine records, and the
/// record store still said the task was open.
#[test]
fn a_flipped_row_aggregates_every_record_and_the_task_store_agrees() {
    let dir = setup(true);
    let repo = dir.path();
    ok(repo, &["ingest", "plans"]);
    assert_eq!(task_records(repo)["records"][0]["status"], "open");

    for (verdict, summary) in [("faithful", "1 vitest case"), ("not-applicable", "9 vitest cases and 2 smoke checks")] {
        ok(repo, &["evidence", "add", "--task", "cats#1", "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--verdict", verdict, "--summary", summary]);
        ok(repo, &["gate", "promote"]);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    ok(repo, &["gate", "task", "cats#1"]);
    let tasks = std::fs::read_to_string(repo.join("openspec/changes/cats/tasks.md")).unwrap();
    assert!(
        tasks.contains("- [x] 1 Ship the run — ✅ 2 evidence records (1 faithful, 1 not-applicable); latest: 9 vitest cases and 2 smoke checks"),
        "{tasks}"
    );
    assert_eq!(task_records(repo)["records"][0]["status"], "done", "`canon query --kind task` agrees with the flipped checkbox");
}

/// Dogfood nyan-omp: every record carried a task AND a scenario key, so
/// the task key shadowed the scenario and `verifying → shipped` saw no
/// verdict. One attestation answers both joins.
#[test]
fn a_task_and_scenario_keyed_record_ships_its_scenario_and_still_flips_its_task() {
    let dir = setup(true);
    let repo = dir.path();
    ok(repo, &["subject", "new", "cat-run", "--domain", "game", "--title", "Cat run"]);
    scenario(repo, "game.run.01", Some("cat-run"));
    for state in ["specced", "building", "verifying"] {
        ok(repo, &["subject", "status", "cat-run", state]);
    }
    ok(repo, &["evidence", "add", "--task", "cats#1", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "test-run", "--ref", "npm test", "--role", "implementer"]);
    ok(repo, &["gate", "promote"]);

    let shipped = canon(repo, &["subject", "status", "cat-run", "shipped"]);
    assert!(shipped.status.success(), "the scenario cell has the record's faithful verdict: {}", text(&shipped));
    ok(repo, &["gate", "task", "cats#1"]);
    assert!(std::fs::read_to_string(repo.join("openspec/changes/cats/tasks.md")).unwrap().contains("- [x] 1 Ship the run"));
}
