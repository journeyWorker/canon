//! 0.18 #29: `canon report` reads the sqlite hot tier. `canon init`
//! routes `task` to a sqlite `hot` rung, and before 0.18 the report read
//! only the git ledger and local parquet, so every `Task` column of a
//! fresh repo's report was empty. These tests drive the actually-built
//! `canon` binary through the loop a fresh repo runs.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).env("CANON_ACTOR", "canon").output().expect("spawn canon binary")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn duckdb_available() -> bool {
    Command::new("duckdb").arg("--version").output().is_ok()
}

fn ok(output: Output, what: &str) -> Output {
    assert!(output.status.success(), "{what}: stdout: {} stderr: {}", stdout(&output), stderr(&output));
    output
}

/// A fresh `canon init` repo with one subject, one change `add-login`
/// carrying two tasks, its brief approved.
fn fresh_repo_with_tasks() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap().success());
    ok(run_canon(&["init", "--no-agents-md", "--no-policy"], repo), "init");
    ok(run_canon(&["subject", "new", "auth", "--domain", "dev", "--title", "Auth"], repo), "subject new");
    ok(run_canon(&["change", "new", "add-login", "--subject", "auth", "--title", "Add login"], repo), "change new");
    let tasks = repo.join(".canon/changes/add-login/tasks.md");
    let mut text = fs::read_to_string(&tasks).unwrap();
    text.push_str("- [ ] 1 Write the login scenarios\n- [ ] 2 Implement login\n");
    fs::write(&tasks, text).unwrap();
    let brief = repo.join(".canon/changes/add-login/brief.md");
    let mut text = fs::read_to_string(&brief).unwrap();
    text.push_str("\nApproved: journey 2026-10-12 — \"go\"\n");
    fs::write(&brief, text).unwrap();
    ok(run_canon(&["ingest", "plans"], repo), "ingest plans");
    dir
}

/// report.generation.10: in a fresh `canon init` repo — `task` routed
/// to the sqlite hot tier — the trust matrix carries each task's title
/// and status, the flipped one `done`, and nothing is named as not read
/// directly.
#[test]
fn report_generation_10_task_columns_fill_from_the_sqlite_hot_tier() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = fresh_repo_with_tasks();
    let repo = dir.path();
    let canon_yaml = fs::read_to_string(repo.join("canon.yaml")).unwrap();
    assert!(canon_yaml.contains("backend: sqlite") && canon_yaml.contains("task: hot"), "init's default routing: {canon_yaml}");

    ok(
        run_canon(&["evidence", "add", "--task", "add-login#1", "--kind", "test-run", "--ref", "cargo test login", "--role", "implementer", "--summary", "login scenarios written"], repo),
        "evidence add",
    );
    ok(run_canon(&["gate", "task", "add-login#1"], repo), "gate task");
    assert!(repo.join(".canon/hot.db").is_file(), "tasks live in the sqlite hot tier");
    assert!(!repo.join(".canon/ledger/kind=task").exists(), "no task reached the git ledger, so the report can only have read sqlite");

    let report = ok(run_canon(&["report"], repo), "report");
    assert!(!stderr(&report).contains("WARN"), "{}", stderr(&report));
    let content = fs::read_to_string(repo.join(".canon/REPORT.md")).unwrap();
    assert!(content.contains("| add-login | add-login#1 | Write the login scenarios | done | ✓ | ✓ | canon | 1 |"), "{content}");
    assert!(content.contains("| add-login | add-login#2 | Implement login | open | · | · | — | 0 |"), "{content}");
    assert!(!content.contains("## Kinds not read directly"), "{content}");

    let check = run_canon(&["report", "--check"], repo);
    assert_eq!(check.status.code(), Some(0), "the hot tier reads the same twice: {}", stderr(&check));
}

/// report.generation.11: a hot db that does not exist yet is no rows —
/// the report succeeds and does not create it; a hot db that is not a
/// sqlite database fails the report naming the file, rather than
/// rendering a report that silently drops its tasks.
#[test]
fn report_generation_11_a_missing_hot_db_is_no_rows_and_an_unreadable_one_fails_loud() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    fs::write(
        repo.join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\nrouting:\n  task: hot\n  change: local\n",
    )
    .unwrap();

    let report = ok(run_canon(&["report"], repo), "report over a missing hot db");
    assert!(!stderr(&report).contains("WARN"), "{}", stderr(&report));
    assert!(!repo.join(".canon/hot.db").exists(), "reading never creates the hot db");

    fs::write(repo.join(".canon/hot.db"), "not a sqlite database, just text\n".repeat(64)).unwrap();
    let report = run_canon(&["report"], repo);
    assert_eq!(report.status.code(), Some(1), "stdout: {} stderr: {}", stdout(&report), stderr(&report));
    let err = stderr(&report);
    assert!(err.contains("reading the sqlite hot tier at") && err.contains(".canon/hot.db"), "{err}");
}
