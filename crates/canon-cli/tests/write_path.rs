//! 0.16 D2 + D4 #8 through the real binary: staged writes promote on
//! write (`--promote`, `write: { promote: auto }`, `--no-promote`), the
//! readers of the scenario index refresh it when a `.feature` file
//! changed, and `.canon/audit/` stays out of git.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A write command's stdout without its closing `next:` line (0.21 D2).
fn without_next(text: &str) -> String {
    text.lines().filter(|line| !line.starts_with("next: ")).map(|line| format!("{line}\n")).collect()
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
    files_under(repo.join(".canon/ledger/_staging")).len()
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
    assert_eq!(without_next(&text).lines().count(), 1, "one success line:\n{text}");
    assert!(text.starts_with("canon evidence add: wrote kind=evidence_record/game.run.01__") && text.contains(PROMOTED), "the success line names the committed record and the promotion:\n{text}");
    assert!(!text.contains("canon gate promote:"), "a promote on write reports its own record, not a batch tally:\n{text}");
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
    ok(repo, &["divergence", "add", "--project-id", "root", "--scenario-id", "game.run.01", "--sha", &sha, "--reviewer", "r", "--role", "reviewer"]);
    assert_eq!(staged(repo), 0, "nothing waits for a promote");
    assert_eq!((committed(repo, "evidence_record"), committed(repo, "finding"), committed(repo, "divergence")), (1, 2, 1));

    // A second, different attestation (an identical one is a repeat and writes nothing).
    let out = ok(repo, &with(evidence("game.run.01"), &["--summary", "attested again", "--no-promote"]));
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

/// evidence.add.65: a promote on write commits only the record its own
/// run staged. A record another author staged with `--no-promote` — an
/// attestation or a divergence candidate — stays staged until a full
/// `canon gate promote` commits it.
#[test]
fn a_promote_on_write_commits_only_its_own_record() {
    let dir = init_repo();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);

    ok(repo, &[
        "evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "test-run", "--ref", "cargo test other", "--role", "reviewer",
        "--actor-id", "other", "--no-promote",
    ]);
    assert_eq!(staged(repo), 1);
    let out = ok(repo, &evidence("game.run.01"));
    let text = stdout(&out);
    assert_eq!(without_next(&text).lines().count(), 1, "one success line, for this run's own record:\n{text}");
    assert!(text.starts_with("canon evidence add: wrote kind=evidence_record/") && text.contains(PROMOTED), "{text}");
    assert_eq!((staged(repo), committed(repo, "evidence_record")), (1, 1), "the other author's record is still staged");

    let sha = "a".repeat(40);
    let stage = |actor: &str, extra: &[&str]| {
        let mut args = vec!["divergence", "add", "--project-id", "root", "--scenario-id", "game.run.01", "--sha", &sha, "--reviewer", "r", "--role", "reviewer", "--actor-id", actor];
        args.extend_from_slice(extra);
        ok(repo, &args)
    };
    stage("other", &["--no-promote"]);
    assert_eq!(staged(repo), 2);
    let out = stage("mine", &[]);
    assert!(stdout(&out).lines().any(|l| l.contains(PROMOTED)), "{}", stdout(&out));
    assert_eq!((staged(repo), committed(repo, "divergence")), (2, 1), "the other reviewer's candidate is still staged");

    let all = ok(repo, &["gate", "promote"]);
    assert!(stdout(&all).contains("canon gate promote: promoted evidence_record=1"), "{}", stdout(&all));
    ok(repo, &["divergence", "promote"]);
    assert_eq!((staged(repo), committed(repo, "evidence_record"), committed(repo, "divergence")), (0, 2, 2));
}

/// evidence.add.67: under `write: { promote: auto }` every staged write
/// prints ONE success line naming the record's committed file, never the
/// staged file promotion moved (dogfood eval 3 H2); with `--no-promote`
/// the line names the staged file and the promote that commits it:
/// `canon gate promote`.
#[test]
fn evidence_add_67_a_promoted_write_names_only_its_committed_record() {
    let dir = init_repo();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);
    let sha = "a".repeat(40);
    let writes: [(&str, Vec<&str>); 4] = [
        ("canon evidence add", evidence("game.run.01")),
        ("canon finding add", vec!["finding", "add", "--change-id", "c-run", "--round", "1", "--seq", "1", "--severity", "note", "--reviewer", "r", "--summary", "s"]),
        ("canon finding close", vec!["finding", "close", "--change-id", "c-run", "--round", "1", "--seq", "1", "--disposition", "rejected"]),
        ("canon divergence add", vec!["divergence", "add", "--project-id", "root", "--scenario-id", "game.run.01", "--sha", &sha, "--reviewer", "r", "--role", "reviewer"]),
    ];
    for (command, args) in writes {
        let text = stdout(&ok(repo, &args));
        let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with("canon finding add: note") && !l.starts_with("next: ")).collect();
        assert_eq!(lines.len(), 1, "{command}: one success line:\n{text}");
        let path = lines[0].strip_prefix(&format!("{command}: wrote ")).unwrap_or_else(|| panic!("{command} names what it wrote: {text}"));
        let path = path.split([' ']).next().unwrap();
        assert!(repo.join(".canon/ledger").join(path).is_file(), "{command}: {path} is the committed file");
        assert!(lines[0].contains(PROMOTED), "{text}");
        assert!(!text.contains("_staging") && !text.lines().any(|l| l.starts_with("promoted ")), "{command}: no staged path, no separate promote line:\n{text}");
    }
    assert_eq!(staged(repo), 0);

    let kept: [(&str, Vec<&str>, &str); 3] = [
        ("canon evidence add", with(evidence("game.run.01"), &["--summary", "attested again", "--no-promote"]), STAGED),
        ("canon finding add", vec!["finding", "add", "--change-id", "c-run", "--round", "1", "--seq", "2", "--severity", "note", "--reviewer", "r", "--summary", "s", "--no-promote"], STAGED),
        ("canon divergence add", vec!["divergence", "add", "--project-id", "root", "--scenario-id", "game.run.01", "--sha", &sha, "--reviewer", "r", "--role", "reviewer", "--no-promote"], STAGED),
    ];
    for (command, args, suffix) in kept {
        let text = stdout(&ok(repo, &args));
        let line = text.lines().next().unwrap_or_default();
        let path = line.strip_prefix(&format!("{command}: staged ")).and_then(|rest| rest.split(' ').next()).unwrap_or_else(|| panic!("{command} names the staged file: {text}"));
        let staged_file = if Path::new(path).is_absolute() { PathBuf::from(path) } else { repo.join(".canon/ledger/_staging").join(path) };
        assert!(staged_file.is_file(), "{command}: {path} is the staged file");
        assert!(line.contains(suffix), "{command} directs to its promote: {text}");
    }
    assert_eq!(staged(repo), 3);
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
    assert!(lines[0].starts_with("canon status: refreshed the scenario index from 1 changed feature file(s) (.canon/specs/kind=feature/"), "{lines:?}");
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

    let feature = files_under(repo.join(".canon/specs/kind=feature")).into_iter().find(|p| p.to_string_lossy().contains("run")).expect("the run feature file");
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

    let feature = files_under(repo.join(".canon/specs/kind=feature")).pop().unwrap();
    let text = std::fs::read_to_string(&feature).unwrap();
    std::fs::write(&feature, text.replace("A run ends", "A run ends cleanly")).unwrap();
    let out = ok(repo, &["subject", "status", "runs", "--to", "specced", "--actor-id", "lead"]);
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
    let feature = files_under(repo.join(".canon/specs/kind=feature")).pop().unwrap();
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
    std::fs::create_dir_all(bare.path().join(".canon/specs/kind=feature")).unwrap();
    std::fs::copy(&feature, bare.path().join(".canon/specs/kind=feature/run.feature")).unwrap();
    let _ = canon(bare.path(), &["gate", "check", "--repo", "."]);
    let _ = canon(bare.path(), &["status", "--repo", "."]);
    assert!(!bare.path().join(".canon/ledger").exists(), "no canon.yaml, no refresh");
}

/// Every scenario index record of `repo` without its `at` stamp, sorted:
/// what two runs that wrote the same records at different instants
/// share.
fn scenario_records_without_at(repo: &Path) -> Vec<String> {
    let mut records: Vec<String> = scenario_records(repo)
        .into_iter()
        .map(|mut record| {
            record.as_object_mut().unwrap().remove("at");
            record.to_string()
        })
        .collect();
    records.sort();
    records
}

/// A repo with two scenarios in two `.feature` files, indexed, whose
/// `game.shop` file is then deleted; `spec_coverage` grades every
/// scenario for evidence.
fn repo_with_a_deleted_scenario() -> tempfile::TempDir {
    let dir = init_repo();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_evidence: true\n").unwrap();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);
    ok(repo, &["scenario", "new", "game.shop.01", "--title", "A shop opens", "--case", "failure"]);
    ok(repo, &["inventory", "sync"]);
    let shop = files_under(repo.join(".canon/specs/kind=feature")).into_iter().find(|p| p.to_string_lossy().contains("shop")).expect("the shop feature file");
    std::fs::remove_file(shop).unwrap();
    dir
}

/// inventory.sync.43: a scenario whose `.feature` file is deleted is a
/// change to the index like any other. A reader's refresh and a full
/// `canon inventory sync` both mark it removed with the same record; the
/// gate stops grading it and `canon query` stops listing it; adding it
/// back brings it back.
#[test]
fn a_deleted_scenario_is_removed_the_same_way_by_a_refresh_and_by_a_full_sync() {
    // One fixture, copied: two separately scaffolded repos carry
    // provenance timestamps that differ across a second boundary, and so
    // different source digests, which made the comparison below flaky.
    let refreshed = repo_with_a_deleted_scenario();
    let synced = tempfile::tempdir().unwrap();
    let copied = Command::new("cp").arg("-R").arg(refreshed.path().join(".")).arg(synced.path()).status().unwrap();
    assert!(copied.success());

    let err = stderr(&canon(refreshed.path(), &["gate", "check"]));
    assert!(
        err.contains("canon gate check: refreshed the scenario index from 1 scenario(s) no longer in any feature file (game.shop.01): 1 record(s) written"),
        "{err}"
    );
    let sync = ok(synced.path(), &["inventory", "sync"]);
    assert!(stdout(&sync).contains("1 record(s) written (1 marking scenario(s) no longer in any feature file as removed)"), "{}", stdout(&sync));
    assert_eq!(scenario_records_without_at(refreshed.path()), scenario_records_without_at(synced.path()), "the refresh writes exactly what a full sync writes");
    assert!(scenario_records(synced.path()).iter().any(|r| r["scenario_id"] == "game.shop.01" && r["removed"] == true), "{:?}", scenario_records(synced.path()));

    for repo in [refreshed.path(), synced.path()] {
        assert!(stdout(&ok(repo, &["inventory", "sync"])).contains("0 record(s) written"), "nothing is left for a sync to do");
        let graded = stdout(&canon(repo, &["gate", "check"]));
        assert!(graded.contains("game.run.01") && !graded.contains("game.shop"), "the gate grades only the scenario still specified:\n{graded}");
        let listed = stdout(&ok(repo, &["query", "--kind", "scenario", "--json"]));
        assert!(listed.contains("game.run.01") && !listed.contains("game.shop.01"), "{listed}");
    }

    ok(refreshed.path(), &["scenario", "new", "game.shop.01", "--title", "A shop opens", "--case", "failure"]);
    let graded = stdout(&canon(refreshed.path(), &["gate", "check"]));
    assert!(graded.contains("game.shop.01"), "a re-added scenario is graded again:\n{graded}");
}

/// inventory.sync.44 (@case:failure): a root that refuses to sync marks
/// nothing removed either. Neither a reader's refresh nor a full sync
/// writes a tombstone for a root with format violations, so a half-edited
/// corpus never empties the index.
#[test]
fn a_root_that_cannot_sync_marks_no_scenario_removed() {
    let dir = repo_with_a_deleted_scenario();
    let repo = dir.path();
    let run = files_under(repo.join(".canon/specs/kind=feature")).pop().expect("the run feature file");
    let text = std::fs::read_to_string(&run).unwrap();
    std::fs::write(&run, text.lines().filter(|l| !l.trim_start().starts_with("# canon:")).map(|l| format!("{l}\n")).collect::<String>()).unwrap();

    let err = stderr(&canon(repo, &["status"]));
    assert!(err.contains("canon status: found the scenario index stale for 1 changed feature file(s)"), "{err}");
    assert!(err.contains("and 1 scenario(s) no longer in any feature file (game.shop.01): 0 record(s) written; NOT refreshed: root `root`"), "{err}");
    let sync = canon(repo, &["inventory", "sync"]);
    assert_eq!(sync.status.code(), Some(1), "{}", stdout(&sync));
    let records = scenario_records(repo);
    assert_eq!(records.len(), 2, "no tombstone and no new generation: {records:?}");
    assert!(records.iter().all(|r| r.get("removed").is_none()), "{records:?}");
}

/// inventory.sync.47 (@case:failure): a root the readers' refresh cannot
/// index fails closed through one decision
/// (`canon_cli::inventory::unrefreshable_roots`): `canon gate check`
/// exits 1 with an `uncovered-cell` naming the root, the `verifying →
/// shipped` guard refuses with the same violation, and `canon status`'s
/// only step is the sync. Restoring the file lifts all three.
#[test]
fn inventory_sync_47_an_unrefreshable_root_fails_the_gate_the_ship_guard_and_status() {
    let dir = init_repo();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_evidence: true\n").unwrap();
    ok(repo, &["subject", "new", "demo", "--domain", "game", "--title", "Demo", "--actor-id", "lead"]);
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--subject", "demo", "--case", "failure"]);
    ok(repo, &evidence("game.run.01"));
    for state in ["specced", "building", "verifying"] {
        ok(repo, &["subject", "status", "demo", "--to", state, "--actor-id", "lead"]);
    }
    let feature = files_under(repo.join(".canon/specs/kind=feature")).pop().unwrap();
    let clean = std::fs::read_to_string(&feature).unwrap();
    let mut lines: Vec<&str> = clean.lines().collect();
    let provenance = lines.iter().rposition(|l| l.contains("# canon:")).unwrap();
    lines.remove(provenance);
    std::fs::write(&feature, lines.join("\n").replace("A run ends", "A run ends at zero health") + "\n").unwrap();

    let violation = "uncovered-cell root — the scenario index for spec root `root` is stale and could not be refreshed (1 format violation(s), 0 sync error(s)), so its changed .feature files are not graded; `canon inventory sync` lists the problems to fix";
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}{}", stdout(&gate), stderr(&gate));
    assert!(stdout(&gate).contains(violation), "{}", stdout(&gate));

    let ship = canon(repo, &["subject", "status", "demo", "--to", "shipped", "--actor-id", "lead"]);
    assert_eq!(ship.status.code(), Some(1), "{}", stderr(&ship));
    assert!(stderr(&ship).contains("uncovered-cell root — verifying → shipped: the scenario index for spec root `root` is stale"), "{}", stderr(&ship));
    let report: serde_json::Value = serde_json::from_slice(&ok(repo, &["status", "--json"]).stdout).unwrap();
    assert_eq!(report["subjects"][0]["status"], "verifying", "a refused ship leaves the subject: {report}");
    let next = report["next"].as_array().unwrap();
    assert_eq!(next.len(), 1, "the sync is the only step: {report}");
    assert_eq!(next[0]["command"], "canon inventory sync");
    assert!(next[0]["why"].as_str().unwrap().contains("spec root(s) `root`"), "{report}");

    std::fs::write(&feature, &clean).unwrap();
    let gate = canon(repo, &["gate", "check"]);
    assert!(!stdout(&gate).contains("could not be refreshed"), "{}", stdout(&gate));
    ok(repo, &["subject", "status", "demo", "--to", "shipped", "--actor-id", "lead"]);
}

/// inventory.sync.45: `canon inventory sync` and every reader's refresh
/// take the repo's scenario-index lock, and a writer that waited checks
/// staleness again before writing. Deterministic: the test holds the
/// lock itself, starts two syncs and two refreshing readers against one
/// stale index, sees that none of them writes while it holds the lock,
/// releases it — and the index ends with one record per scenario,
/// written by exactly one of the four.
#[test]
fn concurrent_syncs_and_refreshes_write_each_index_record_once() {
    let dir = init_repo();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);
    ok(repo, &["scenario", "new", "game.shop.01", "--title", "A shop opens", "--case", "failure"]);

    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(repo.join(".canon/scenario-index.lock")).unwrap();
    lock.lock().unwrap();
    let spawn = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn canon")
    };
    let children = [spawn(&["inventory", "sync"]), spawn(&["inventory", "sync"]), spawn(&["status"]), spawn(&["gate", "check"])];
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(scenario_records(repo).is_empty(), "nothing writes the index while another process holds the lock");
    lock.unlock().unwrap();
    drop(lock);

    let outputs: Vec<Output> = children.into_iter().map(|child| child.wait_with_output().unwrap()).collect();
    for sync in &outputs[..2] {
        assert!(sync.status.success(), "{}{}", stdout(sync), stderr(sync));
    }
    let writers = outputs.iter().filter(|out| stdout(out).contains("2 record(s) written") || stderr(out).contains("refreshed the scenario index")).count();
    assert_eq!(writers, 1, "exactly one writer indexed the change: {:?}", outputs.iter().map(|o| (stdout(o), stderr(o))).collect::<Vec<_>>());
    assert_eq!(scenario_records(repo).len(), 2, "one record per scenario: {:?}", scenario_records(repo));
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

/// authoring.init.71: `canon init` also ignores the derived scenario
/// index, so a reader's refresh after a committed `.feature` edit leaves
/// `git status` clean too.
#[test]
fn the_scenario_index_a_reader_rebuilds_stays_out_of_git() {
    let dir = init_repo();
    let repo = dir.path();
    let gitignore = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert!(gitignore.lines().any(|l| l.trim() == ".canon/ledger/kind=scenario/"), "{gitignore}");
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "spec"]);

    let out = ok(repo, &["status"]);
    assert!(stderr(&out).contains("refreshed the scenario index"), "{}", stderr(&out));
    assert_eq!(scenario_records(repo).len(), 1);
    ok(repo, &["query", "--kind", "subject"]);
    assert_eq!(git(repo, &["status", "--porcelain"]), "", "neither the rebuilt index nor the audit log shows up as untracked");
}

/// authoring.init.72: `canon init` ignores the scenario-index lock file,
/// so the lock a reader's refresh leaves behind never shows up in `git
/// status`.
#[test]
fn the_scenario_index_lock_stays_out_of_git() {
    let dir = init_repo();
    let repo = dir.path();
    let gitignore = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert!(gitignore.lines().any(|l| l.trim() == ".canon/scenario-index.lock"), "{gitignore}");
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "spec"]);

    ok(repo, &["status"]);
    assert!(repo.join(".canon/scenario-index.lock").is_file(), "the refresh took the lock");
    assert_eq!(git(repo, &["status", "--porcelain"]), "", "the lock file must not show up as untracked");
}
