//! 0.16 D2 + D4 #8 through the real binary: staged writes promote on
//! write (`--promote`, `write: { promote: auto }`, `--no-promote`), the
//! readers of the scenario index refresh it when a `.feature` file
//! changed, and `.canon/audit/` stays out of git.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?} failed:\n{}{}", stdout(&out), stderr(&out));
    out
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(repo)
        .output()
        .expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `git init` + `canon init --no-policy`: a repo scaffolded by this
/// build, so `canon.yaml` carries `write: { promote: auto }`.
fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    ok(dir.path(), &["init", "--repo", ".", "--no-policy"]);
    dir
}

/// A repo whose `canon.yaml` predates 0.16: no `write:` key, so writes
/// stay staged (`manual`).
fn manual_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let yaml = std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap();
    let start = yaml.find("write:\n").expect("init writes a write: section");
    std::fs::write(dir.path().join("canon.yaml"), &yaml[..start]).unwrap();
    dir
}

fn files_under(dir: PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files
}

fn staged(repo: &Path) -> usize {
    files_under(repo.join(".canon/ledger/_staging")).len() + files_under(repo.join(".canon/ledger/_staging_divergence")).len()
}

fn committed(repo: &Path, kind: &str) -> usize {
    files_under(repo.join(format!(".canon/ledger/kind={kind}"))).len()
}

fn scenario_records(repo: &Path) -> Vec<serde_json::Value> {
    files_under(repo.join(".canon/ledger/kind=scenario")).iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect()
}

fn evidence(id: &str) -> Vec<&str> {
    vec!["evidence", "add", "--scenario-id", id, "--project-id", "root", "--kind", "test-run", "--ref", "cargo test", "--role", "implementer", "--actor-id", "impl"]
}

fn with<'a>(mut args: Vec<&'a str>, extra: &[&'a str]) -> Vec<&'a str> {
    args.extend_from_slice(extra);
    args
}

const PROMOTED: &str = "— staged and promoted in this run; nothing to promote";
const STAGED: &str = "— run `canon gate promote` to commit it";

// ── promote on write ──

/// evidence.add.60: `--promote` stages and promotes in one run in a repo
/// with no `write:` key; `canon gate promote` afterwards has nothing to
/// do and still exits 0.
#[test]
fn promote_flag_commits_in_the_same_run_and_a_later_gate_promote_is_a_no_op() {
    let dir = manual_repo();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);

    let out = ok(repo, &with(evidence("game.run.01"), &["--promote"]));
    let text = stdout(&out);
    assert!(text.lines().any(|l| l.contains(PROMOTED)), "the success line names the promotion:\n{text}");
    assert!(text.contains("canon gate promote: promoted evidence_record=1"), "the promote report is printed:\n{text}");
    assert_eq!(staged(repo), 0);
    assert_eq!(committed(repo, "evidence_record"), 1);

    let again = ok(repo, &["gate", "promote"]);
    assert!(stdout(&again).contains("nothing staged"), "{}", stdout(&again));
    assert_eq!(committed(repo, "evidence_record"), 1, "a second promote commits nothing");
}

/// evidence.add.61: an absent `write:` key keeps today's behavior — the
/// write stays staged until `canon gate promote`.
#[test]
fn without_a_write_key_a_staged_write_stays_staged() {
    let dir = manual_repo();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);

    let out = ok(repo, &evidence("game.run.01"));
    assert!(stdout(&out).lines().any(|l| l.contains(STAGED)), "{}", stdout(&out));
    assert_eq!((staged(repo), committed(repo, "evidence_record")), (1, 0));
    ok(repo, &["gate", "promote"]);
    assert_eq!((staged(repo), committed(repo, "evidence_record")), (0, 1));
}

/// evidence.add.62: under `write: { promote: auto }` (what `canon init`
/// writes) every staged write promotes itself; `--no-promote` keeps one
/// write staged for `canon gate promote`.
#[test]
fn auto_promotes_every_staged_write_and_no_promote_keeps_one_staged() {
    let dir = init_repo();
    let repo = dir.path();
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    assert!(yaml.contains("write:\n") && yaml.contains("  promote: auto\n"), "init writes auto:\n{yaml}");
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);

    ok(repo, &evidence("game.run.01"));
    ok(repo, &["finding", "add", "--change-id", "c-run", "--round", "1", "--seq", "1", "--severity", "note", "--reviewer", "r", "--summary", "s"]);
    ok(repo, &["finding", "close", "--change-id", "c-run", "--round", "1", "--seq", "1", "--disposition", "rejected"]);
    let sha = "a".repeat(40);
    ok(repo, &["divergence", "stage", "--project-id", "root", "--scenario-id", "game.run.01", "--sha", &sha, "--reviewer", "r", "--role", "reviewer"]);
    assert_eq!(staged(repo), 0, "nothing waits for a promote");
    assert_eq!((committed(repo, "evidence_record"), committed(repo, "finding"), committed(repo, "divergence")), (1, 2, 1));

    let out = ok(repo, &with(evidence("game.run.01"), &["--no-promote"]));
    assert!(stdout(&out).lines().any(|l| l.contains(STAGED)), "{}", stdout(&out));
    assert_eq!(staged(repo), 1);
    ok(repo, &["gate", "promote"]);
    assert_eq!((staged(repo), committed(repo, "evidence_record")), (0, 2));
}

/// evidence.add.63 (@case:failure): a malformed `write:` section is a
/// usage error that stages nothing, and `--promote` with `--no-promote`
/// is refused by the parser.
#[test]
fn a_malformed_write_section_or_both_flags_are_refused_before_anything_is_staged() {
    let dir = manual_repo();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    std::fs::write(repo.join("canon.yaml"), format!("{yaml}write:\n  promote: always\n")).unwrap();

    let out = canon(repo, &evidence("game.run.01"));
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("canon.yaml `write:` section is malformed"), "{}", stderr(&out));
    let out = canon(repo, &["finding", "add", "--change-id", "c-run", "--round", "1", "--seq", "1", "--severity", "note", "--reviewer", "r", "--summary", "s"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert_eq!(staged(repo), 0);

    let check = canon(repo, &["init", "--check-config", "--repo", "."]);
    assert_eq!(check.status.code(), Some(1));
    assert!(stdout(&check).contains("[FAIL] write:"), "{}", stdout(&check));

    std::fs::write(repo.join("canon.yaml"), &yaml).unwrap();
    let out = canon(repo, &with(evidence("game.run.01"), &["--promote", "--no-promote"]));
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert_eq!(staged(repo), 0);
}

// ── stale index refresh ──

/// inventory.sync.40: a reader refreshes the scenario index from a new
/// `.feature` file, says so in one line, and is silent once it is fresh.
#[test]
fn a_reader_refreshes_a_stale_index_once_and_names_the_files() {
    let dir = init_repo();
    let repo = dir.path();
    ok(repo, &["subject", "new", "runs", "--domain", "dev", "--title", "Runs", "--actor-id", "lead"]);
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure", "--subject", "runs"]);
    assert!(scenario_records(repo).is_empty(), "scenario new writes no index record");

    let out = ok(repo, &["status", "--json"]);
    let lines: Vec<String> = stderr(&out).lines().map(str::to_string).collect();
    assert_eq!(lines.len(), 1, "one line says what was refreshed: {lines:?}");
    assert!(lines[0].starts_with("canon status: refreshed the scenario index from 1 changed feature file(s) (specs/features/"), "{lines:?}");
    assert!(lines[0].ends_with(": 1 record(s) written"), "{lines:?}");
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout stays JSON");
    let runs = report["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "runs").unwrap();
    assert_eq!(runs["scenarios"], 1, "status counts the scenario it just indexed: {report}");

    let quiet = ok(repo, &["gate", "check"]);
    assert!(!stderr(&quiet).contains("scenario index"), "a fresh index is not refreshed again: {}", stderr(&quiet));
    assert_eq!(scenario_records(repo).len(), 1);
}

/// gate.check.80: an edited `.feature` file is re-indexed by the
/// next reader — only that file — with the record `canon inventory sync`
/// would write.
#[test]
fn an_edited_feature_file_is_reindexed_by_the_next_reader_and_only_that_file() {
    let dir = init_repo();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);
    ok(repo, &["scenario", "new", "game.shop.01", "--title", "A shop opens", "--case", "failure"]);
    ok(repo, &["inventory", "sync"]);
    assert_eq!(scenario_records(repo).len(), 2);

    let feature = files_under(repo.join("specs/features")).into_iter().find(|p| p.to_string_lossy().contains("run")).expect("the run feature file");
    let text = std::fs::read_to_string(&feature).unwrap();
    std::fs::write(&feature, text.replace("A run ends", "A run ends at zero health")).unwrap();

    let out = ok(repo, &["gate", "check"]);
    let err = stderr(&out);
    assert!(err.contains("canon gate check: refreshed the scenario index from 1 changed feature file(s)") && err.contains(": 1 record(s) written"), "{err}");
    let records = scenario_records(repo);
    assert_eq!(records.len(), 3, "one new record for the edited scenario, none for the untouched one");
    assert!(records.iter().any(|r| r["title"] == "A run ends at zero health"), "{records:?}");

    let sync = ok(repo, &["inventory", "sync"]);
    assert!(stdout(&sync).contains("0 record(s) written"), "the refresh wrote what sync would: {}", stdout(&sync));
}

/// inventory.sync.41: `review add` and `subject status` see a scenario
/// that was only written, never synced — no `inventory sync` step.
#[test]
fn review_add_and_subject_status_need_no_sync_step() {
    let dir = init_repo();
    let repo = dir.path();
    ok(repo, &["subject", "new", "runs", "--domain", "dev", "--title", "Runs", "--actor-id", "lead"]);
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure", "--subject", "runs"]);
    let review = ok(repo, &[
        "review", "add", "--project-id", "root", "--scenario-id", "game.run.01", "--reviewer", "rev", "--pin", "p", "--original-spec-ref", "specs/x.feature",
        "--role", "reviewer", "--actor-id", "rev",
    ]);
    assert!(stderr(&review).contains("canon review add: refreshed the scenario index"), "{}", stderr(&review));

    let feature = files_under(repo.join("specs/features")).pop().unwrap();
    let text = std::fs::read_to_string(&feature).unwrap();
    std::fs::write(&feature, text.replace("A run ends", "A run ends cleanly")).unwrap();
    let out = ok(repo, &["subject", "status", "runs", "specced", "--actor-id", "lead"]);
    assert!(stderr(&out).contains("canon subject status: refreshed the scenario index"), "{}", stderr(&out));
}

/// inventory.sync.42 (@case:failure): a root with format violations is
/// named, not refreshed, and the command still runs; a directory without
/// `canon.yaml` is never written to.
#[test]
fn a_root_that_cannot_sync_is_named_and_a_dir_without_canon_yaml_is_never_written() {
    let dir = init_repo();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);
    let feature = files_under(repo.join("specs/features")).pop().unwrap();
    let text = std::fs::read_to_string(&feature).unwrap();
    // Drop the provenance comment `canon format` requires on the header.
    let broken: String = text.lines().filter(|l| !l.trim_start().starts_with("# canon:")).map(|l| format!("{l}\n")).collect();
    assert_ne!(broken, text, "the fixture must actually lose its provenance comments");
    std::fs::write(&feature, broken).unwrap();

    let out = canon(repo, &["status"]);
    let err = stderr(&out);
    assert!(err.contains("canon status: found the scenario index stale for 1 changed feature file(s)"), "{err}");
    assert!(err.contains("NOT refreshed: root `root`") && err.contains("`canon inventory sync` lists them"), "{err}");
    assert!(scenario_records(repo).is_empty(), "an aborted root writes nothing");

    let bare = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(bare.path().join("specs/features")).unwrap();
    std::fs::copy(&feature, bare.path().join("specs/features/run.feature")).unwrap();
    let _ = canon(bare.path(), &["gate", "check", "--repo", "."]);
    let _ = canon(bare.path(), &["status", "--repo", "."]);
    assert!(!bare.path().join(".canon").exists(), "no canon.yaml, no refresh");
}

// ── #8: the audit log stays out of git ──

/// authoring.init.70: `canon init` ignores `.canon/audit/` (#8), so a
/// fresh init followed by a query leaves `git status` clean.
#[test]
fn a_fresh_init_then_a_query_leaves_git_status_clean() {
    let dir = init_repo();
    let repo = dir.path();
    let gitignore = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert!(gitignore.lines().any(|l| l.trim() == ".canon/audit/"), "{gitignore}");
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "init"]);

    ok(repo, &["query", "--kind", "subject"]);
    assert!(repo.join(".canon/audit/access.jsonl").is_file(), "the query was audited");
    assert_eq!(git(repo, &["status", "--porcelain"]), "", "the audit log must not show up as untracked");
}
