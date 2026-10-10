//! Integration tests for `canon change new <slug> --subject <id> --title
//! <t>` (0.14 D2), against the actually-built `canon` binary in a repo set
//! up by `canon init` alone: no hand edit of `canon.yaml` anywhere.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).output().expect("spawn canon binary")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

/// A `canon init` repo with one subject, `auth`.
fn inited_repo_with_subject() -> TempDir {
    let dir = TempDir::new().unwrap();
    let init = run(dir.path(), &["init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let subject = run(dir.path(), &["subject", "new", "auth", "--domain", "dev", "--title", "Auth"]);
    assert!(subject.status.success(), "{}", stderr(&subject));
    dir
}

fn query(repo: &Path, kind: &str) -> Vec<Value> {
    let out = run(repo, &["query", "--kind", kind, "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let payload: Value = serde_json::from_str(&stdout(&out)).unwrap();
    payload["records"].as_array().unwrap().clone()
}

#[test]
fn change_new_scaffolds_imports_and_adopts_with_no_config_edit() {
    let dir = inited_repo_with_subject();
    let canon_yaml = std::fs::read(dir.path().join("canon.yaml")).unwrap();

    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]);
    assert!(out.status.success(), "stdout: {}\nstderr: {}", stdout(&out), stderr(&out));
    assert!(stdout(&out).contains("openspec/changes/add-login/proposal.md"), "{}", stdout(&out));
    assert_eq!(std::fs::read(dir.path().join("canon.yaml")).unwrap(), canon_yaml, "canon.yaml must not change");

    let proposal = std::fs::read_to_string(dir.path().join("openspec/changes/add-login/proposal.md")).unwrap();
    assert!(proposal.contains("## Why\n\nAdd login\n"), "{proposal}");
    assert!(dir.path().join("openspec/changes/add-login/tasks.md").is_file());

    let changes = query(dir.path(), "change");
    assert!(
        changes.iter().any(|c| c["change_id"] == "add-login" && c["subject_id"] == "auth" && c["summary"] == "Add login"),
        "the change must be imported and carry subject_id=auth: {changes:?}"
    );
    let subjects = query(dir.path(), "subject");
    assert_eq!(subjects.len(), 1);
    assert!(subjects[0]["change_ids"].as_array().unwrap().iter().any(|c| c == "add-login"), "{subjects:?}");

    // A task row added to the scaffolded tasks.md imports through the
    // default plans source.
    let tasks_path = dir.path().join("openspec/changes/add-login/tasks.md");
    let mut tasks = std::fs::read_to_string(&tasks_path).unwrap();
    tasks.push_str("\n- [ ] 1 Write the failure-path scenarios\n");
    std::fs::write(&tasks_path, tasks).unwrap();
    let ingest = run(dir.path(), &["ingest", "plans"]);
    assert!(ingest.status.success(), "{}", stderr(&ingest));
    let task_rows = query(dir.path(), "task");
    assert!(task_rows.iter().any(|t| t["task_id"] == "add-login#1"), "{task_rows:?}");
}

#[test]
fn change_new_refuses_an_unknown_subject_and_writes_nothing() {
    let dir = inited_repo_with_subject();
    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "no-such-subject", "--title", "Add login"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("subject `no-such-subject` does not exist"), "{}", stderr(&out));
    assert!(!dir.path().join("openspec/changes/add-login").exists());
    assert!(query(dir.path(), "change").is_empty());
}

#[test]
fn change_new_refuses_an_existing_slug_and_leaves_it_untouched() {
    let dir = inited_repo_with_subject();
    assert!(run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]).status.success());
    let proposal_path = dir.path().join("openspec/changes/add-login/proposal.md");
    let before = std::fs::read(&proposal_path).unwrap();

    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Something else"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("already exists"), "{}", stderr(&out));
    assert_eq!(std::fs::read(&proposal_path).unwrap(), before);

    // A change dir with no record yet (written by hand, not imported) is
    // refused too.
    std::fs::create_dir_all(dir.path().join("openspec/changes/by-hand")).unwrap();
    let out = run(dir.path(), &["change", "new", "by-hand", "--subject", "auth", "--title", "By hand"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("openspec/changes/by-hand` already exists"), "{}", stderr(&out));
}

#[test]
fn change_new_refuses_without_an_openspec_plans_source() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  subject: local\n  change: local\nplans:\n  sources: []\n",
    )
    .unwrap();
    assert!(run(dir.path(), &["subject", "new", "auth", "--domain", "dev", "--title", "Auth"]).status.success());
    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("no `openspec` plans source"), "{}", stderr(&out));
    assert!(!dir.path().join("openspec").exists());
}
