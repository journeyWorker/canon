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

/// 0.14 acceptance rerun G9: `canon gate task` re-imports the flipped
/// change, and the re-imported change version used to carry
/// `subject_id: null`, superseding the adoption `change new` wrote. The
/// latest change still names its subject after the flip, and after a
/// full `canon ingest plans`.
#[test]
fn a_task_flip_keeps_the_changes_adopted_subject() {
    let dir = inited_repo_with_subject();
    let repo = dir.path();
    let created = run(repo, &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]);
    assert!(created.status.success(), "{}", stderr(&created));
    let tasks_path = repo.join("openspec/changes/add-login/tasks.md");
    let mut tasks = std::fs::read_to_string(&tasks_path).unwrap();
    tasks.push_str("\n- [ ] 1 Write the failure-path scenarios\n");
    std::fs::write(&tasks_path, tasks).unwrap();
    assert!(run(repo, &["ingest", "plans"]).status.success());

    let evidence = run(repo, &["evidence", "add", "--task", "add-login#1", "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl"]);
    assert!(evidence.status.success(), "{}", stderr(&evidence));
    assert!(run(repo, &["gate", "promote"]).status.success());
    let flip = run(repo, &["gate", "task", "add-login#1"]);
    assert!(flip.status.success(), "{}{}", stdout(&flip), stderr(&flip));
    assert!(!stderr(&flip).contains("WARN"), "the re-import is clean: {}", stderr(&flip));

    // `canon query --kind change` lists every version; the latest by
    // `at` is the one every folded reader (gate, status) sees.
    let latest = |repo: &Path| {
        query(repo, "change")
            .into_iter()
            .filter(|c| c["change_id"] == "add-login")
            .max_by_key(|c| chrono::DateTime::parse_from_rfc3339(c["at"].as_str().unwrap()).unwrap())
            .expect("the change is in the store")
    };
    let change = latest(repo);
    assert_eq!(change["subject_id"], "auth", "the flip's re-import keeps the adoption: {change}");
    assert_eq!(change["status"], "completed", "the re-import did land: {change}");

    assert!(run(repo, &["ingest", "plans"]).status.success());
    assert_eq!(latest(repo)["subject_id"], "auth", "a full import keeps it too");
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

/// Make `dir` read-only so a ledger write into it fails. `false` when the
/// process ignores directory permissions (root): there is no failure to
/// force, and the caller skips.
#[cfg(unix)]
fn make_read_only(dir: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).unwrap();
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(dir.join("probe"), "").is_ok() {
        std::fs::remove_file(dir.join("probe")).unwrap();
        make_writable(dir);
        eprintln!("skipped: running with permissions that ignore a read-only directory");
        return false;
    }
    true
}

#[cfg(unix)]
fn make_writable(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Every record file in the git ledger, for "nothing was written" checks.
fn ledger_file_count(repo: &Path) -> usize {
    fn walk(dir: &Path) -> usize {
        std::fs::read_dir(dir).map(|entries| entries.filter_map(Result::ok).map(|e| if e.path().is_dir() { walk(&e.path()) } else { 1 }).sum()).unwrap_or(0)
    }
    walk(&repo.join(".canon/ledger"))
}

/// The first record write (the subject) fails: nothing has been written,
/// so the change dir is taken back and nothing remains.
#[cfg(unix)]
#[test]
fn change_new_leaves_nothing_behind_when_the_first_record_write_fails() {
    let dir = inited_repo_with_subject();
    let ledger_subject = dir.path().join(".canon/ledger/kind=subject");
    if !make_read_only(&ledger_subject) {
        return;
    }
    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]);
    make_writable(&ledger_subject);
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert!(stderr(&out).contains("failed to record the adoption") && stderr(&out).contains("nothing was written"), "{}", stderr(&out));
    assert!(stdout(&out).is_empty(), "nothing may be reported written: {}", stdout(&out));
    assert!(dir.path().join("openspec/changes").is_dir(), "the pre-existing changes dir is kept");
    assert_nothing_left(dir.path());
}

/// The second record write (the change) fails after the subject was
/// written: the subject record and the change dir stay, and the printed
/// repair completes the link and is a no-op when run again.
#[cfg(unix)]
#[test]
fn change_new_failing_between_record_writes_prints_a_repair_that_completes_the_link() {
    let dir = inited_repo_with_subject();
    let ledger_change = dir.path().join(".canon/ledger/kind=change");
    if !make_read_only(&ledger_change) {
        return;
    }
    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]);
    make_writable(&ledger_change);
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert!(
        stderr(&out).contains("subject `auth` now lists change `add-login`, but the change record could not be written")
            && stderr(&out).contains("complete the link with `canon ingest plans && canon subject adopt add-login --subject auth`"),
        "{}",
        stderr(&out)
    );
    assert!(dir.path().join("openspec/changes/add-login/proposal.md").is_file(), "the change dir stays for the repair to import");
    assert!(staging_leftovers(dir.path()).is_empty(), "{:?}", staging_leftovers(dir.path()));
    assert!(query(dir.path(), "change").is_empty());
    let subjects = query(dir.path(), "subject");
    assert!(subjects[0]["change_ids"].as_array().unwrap().iter().any(|c| c == "add-login"), "{subjects:?}");

    // The printed repair, run as printed.
    let ingest = run(dir.path(), &["ingest", "plans"]);
    assert!(ingest.status.success(), "{}", stderr(&ingest));
    let adopt = run(dir.path(), &["subject", "adopt", "add-login", "--subject", "auth"]);
    assert!(adopt.status.success(), "{}", stderr(&adopt));
    assert!(stdout(&adopt).contains("linked change `add-login` to subject `auth`"), "{}", stdout(&adopt));
    let changes = query(dir.path(), "change");
    assert!(changes.iter().any(|c| c["change_id"] == "add-login" && c["subject_id"] == "auth"), "{changes:?}");

    // Idempotent: a second run writes nothing.
    let before = ledger_file_count(dir.path());
    let again = run(dir.path(), &["subject", "adopt", "add-login", "--subject", "auth"]);
    assert!(again.status.success(), "{}", stderr(&again));
    assert!(stdout(&again).contains("already linked") && stdout(&again).contains("nothing written"), "{}", stdout(&again));
    assert_eq!(ledger_file_count(dir.path()), before, "a rerun on a linked pair must write no record");
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
