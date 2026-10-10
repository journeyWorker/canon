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
    // refused too, and its proposal is left byte for byte.
    let by_hand = dir.path().join("openspec/changes/by-hand");
    std::fs::create_dir_all(&by_hand).unwrap();
    std::fs::write(by_hand.join("proposal.md"), "# By hand\n\n## Why\n\nWritten by a person.\n").unwrap();
    let out = run(dir.path(), &["change", "new", "by-hand", "--subject", "auth", "--title", "By hand"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("openspec/changes/by-hand` already exists"), "{}", stderr(&out));
    assert_eq!(std::fs::read_to_string(by_hand.join("proposal.md")).unwrap(), "# By hand\n\n## Why\n\nWritten by a person.\n");
    assert!(!by_hand.join("tasks.md").exists(), "a refused change new writes no file into the existing dir");
    assert!(!query(dir.path(), "change").iter().any(|c| c["change_id"] == "by-hand"), "a refused change new records nothing");
}

/// Entries `canon change new` leaves under `.canon/` (its staging dirs).
fn staging_leftovers(repo: &Path) -> Vec<String> {
    std::fs::read_dir(repo.join(".canon"))
        .map(|entries| entries.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("change-new-")).collect())
        .unwrap_or_default()
}

/// Nothing of `add-login` survives a failed `change new`: no change dir,
/// no staging dir, no change record, no link on the subject.
fn assert_nothing_left(repo: &Path) {
    assert!(!repo.join("openspec/changes/add-login").exists(), "the change dir must be removed");
    assert!(staging_leftovers(repo).is_empty(), "staging dirs left: {:?}", staging_leftovers(repo));
    assert!(query(repo, "change").is_empty(), "no change record may be written");
    let subjects = query(repo, "subject");
    assert!(subjects.iter().all(|s| s["change_ids"].as_array().is_none_or(|ids| ids.is_empty())), "{subjects:?}");
}

#[test]
fn change_new_leaves_nothing_behind_when_publishing_the_change_dir_fails() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  subject: local\n  change: local\nplans:\n  sources:\n    - { dialect: openspec, root: . }\n",
    )
    .unwrap();
    // `openspec` is a file, so `openspec/changes/<slug>` cannot be created.
    std::fs::write(dir.path().join("openspec"), "not a directory\n").unwrap();
    assert!(run(dir.path(), &["subject", "new", "auth", "--domain", "dev", "--title", "Auth"]).status.success());

    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]);
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert!(stderr(&out).contains("failed to create `openspec/changes/add-login`"), "{}", stderr(&out));
    assert!(stdout(&out).is_empty(), "nothing may be reported written: {}", stdout(&out));
    assert_eq!(std::fs::read_to_string(dir.path().join("openspec")).unwrap(), "not a directory\n");
    assert_nothing_left(dir.path());
}

/// The adopt write itself fails (the change ledger directory is read-only),
/// after the change dir was published: the dir is taken back and no record
/// or subject link remains.
#[cfg(unix)]
#[test]
fn change_new_leaves_nothing_behind_when_the_adopt_write_fails() {
    use std::os::unix::fs::PermissionsExt;

    let dir = inited_repo_with_subject();
    let ledger_change = dir.path().join(".canon/ledger/kind=change");
    std::fs::create_dir_all(&ledger_change).unwrap();
    std::fs::set_permissions(&ledger_change, std::fs::Permissions::from_mode(0o555)).unwrap();
    // Root ignores directory permissions; there is no failure to force.
    if std::fs::write(ledger_change.join("probe"), "").is_ok() {
        std::fs::remove_file(ledger_change.join("probe")).unwrap();
        std::fs::set_permissions(&ledger_change, std::fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipped: running with permissions that ignore a read-only directory");
        return;
    }

    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]);
    std::fs::set_permissions(&ledger_change, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert!(stderr(&out).contains("failed to record the change, nothing was kept"), "{}", stderr(&out));
    assert!(stdout(&out).is_empty(), "nothing may be reported written: {}", stdout(&out));
    assert!(dir.path().join("openspec/changes").is_dir(), "the pre-existing changes dir is kept");
    assert_nothing_left(dir.path());
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
