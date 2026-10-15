//! Integration tests for `canon change new <slug> --subject <id> --title
//! <t>` (0.14 D2, 0.17 D2), against the actually-built `canon` binary in a
//! repo set up by `canon init` alone: no hand edit of `canon.yaml`
//! anywhere, and no `plans:` entry, since `.canon/changes` is canon's own
//! plan source.

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

/// A write command's stdout without its closing `next:` line (0.21 D2).
fn without_next(text: &str) -> String {
    text.lines().filter(|line| !line.starts_with("next: ")).map(|line| format!("{line}\n")).collect()
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
    assert!(stdout(&out).contains(".canon/changes/add-login/brief.md"), "{}", stdout(&out));
    assert_eq!(std::fs::read(dir.path().join("canon.yaml")).unwrap(), canon_yaml, "canon.yaml must not change");
    assert!(!dir.path().join("openspec").exists(), "canon writes no openspec directory");

    let brief = std::fs::read_to_string(dir.path().join(".canon/changes/add-login/brief.md")).unwrap();
    assert!(brief.contains("<!-- canon:section intent role=context -->\n## Intent\n\nAdd login\n") && brief.contains("<!-- canon:section out-of-scope role=context -->"), "{brief}");
    assert!(!dir.path().join(".canon/changes/add-login/tasks.md").exists(), "0.19 D4: change new writes the brief only");
    assert!(stdout(&out).contains("brief.md") && !stdout(&out).contains("tasks.md"), "{}", stdout(&out));

    let changes = query(dir.path(), "change");
    assert!(
        changes.iter().any(|c| c["change_id"] == "add-login" && c["subject_ids"] == serde_json::json!(["auth"]) && c["summary"] == "Add login"),
        "the change must be imported and carry subject_ids=[auth]: {changes:?}"
    );
    assert!(
        changes.iter().all(|c| c["actor"]["agent_id"] == "canon-plan-import-canon"),
        "a change canon wrote carries canon's own provenance: {changes:?}"
    );
    let subjects = query(dir.path(), "subject");
    assert_eq!(subjects.len(), 1);
    assert!(subjects[0]["change_ids"].as_array().unwrap().iter().any(|c| c == "add-login"), "{subjects:?}");

    // A tasks.md written by hand is still read through canon's own plan
    // source, with no `plans:` entry.
    let tasks_path = dir.path().join(".canon/changes/add-login/tasks.md");
    std::fs::write(&tasks_path, "# Tasks\n\n- [ ] 1 Write the failure-path scenarios\n").unwrap();
    let ingest = run(dir.path(), &["ingest", "plans"]);
    assert!(ingest.status.success(), "{}", stderr(&ingest));
    let task_rows = query(dir.path(), "task");
    assert!(task_rows.iter().any(|t| t["task_id"] == "add-login#1"), "{task_rows:?}");
}

/// 0.14 acceptance rerun G9: `canon gate task` re-imports the flipped
/// change, and the re-imported change version used to carry no subject,
/// superseding the adoption `change new` wrote. The latest change still
/// names its subject after the flip, and after a full `canon ingest
/// plans`.
#[test]
fn a_task_flip_keeps_the_changes_adopted_subject() {
    let dir = inited_repo_with_subject();
    let repo = dir.path();
    let created = run(repo, &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]);
    assert!(created.status.success(), "{}", stderr(&created));
    let tasks_path = repo.join(".canon/changes/add-login/tasks.md");
    std::fs::write(&tasks_path, "# Tasks\n\n- [ ] 1 Write the failure-path scenarios\n").unwrap();
    assert!(run(repo, &["ingest", "plans"]).status.success());
    // The human approved the brief (0.16 M7: nothing is attested before).
    let brief_path = repo.join(".canon/changes/add-login/brief.md");
    let brief = std::fs::read_to_string(&brief_path).unwrap();
    std::fs::write(&brief_path, format!("{brief}\nApproved: Journey 2026-10-12 — \"go\"\n")).unwrap();

    let evidence = run(repo, &["evidence", "add", "--task", "add-login#1", "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl"]);
    assert!(evidence.status.success(), "{}", stderr(&evidence));
    assert!(run(repo, &["gate", "promote"]).status.success());
    let flip = run(repo, &["gate", "task", "add-login#1"]);
    assert!(flip.status.success(), "{}{}", stdout(&flip), stderr(&flip));
    assert!(!stderr(&flip).contains("WARN"), "the re-import is clean: {}", stderr(&flip));
    assert!(std::fs::read_to_string(&tasks_path).unwrap().contains("- [x] 1 Write the failure-path scenarios"), "canon's own tasks.md row flips");

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
    assert_eq!(change["subject_ids"], serde_json::json!(["auth"]), "the flip's re-import keeps the adoption: {change}");
    assert_eq!(change["status"], "completed", "the re-import did land: {change}");

    assert!(run(repo, &["ingest", "plans"]).status.success());
    assert_eq!(latest(repo)["subject_ids"], serde_json::json!(["auth"]), "a full import keeps it too");
}

#[test]
fn change_new_refuses_an_unknown_subject_and_writes_nothing() {
    let dir = inited_repo_with_subject();
    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "no-such-subject", "--title", "Add login"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("subject `no-such-subject` does not exist"), "{}", stderr(&out));
    assert!(!dir.path().join(".canon/changes/add-login").exists());
    assert!(query(dir.path(), "change").is_empty());
}

#[test]
fn change_new_refuses_an_existing_slug_and_leaves_it_untouched() {
    let dir = inited_repo_with_subject();
    assert!(run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]).status.success());
    let brief_path = dir.path().join(".canon/changes/add-login/brief.md");
    let before = std::fs::read(&brief_path).unwrap();

    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Something else"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("already exists"), "{}", stderr(&out));
    assert_eq!(std::fs::read(&brief_path).unwrap(), before);

    // A change dir with no record yet (written by hand, not imported) is
    // refused too, and its brief is left byte for byte.
    let by_hand = dir.path().join(".canon/changes/by-hand");
    std::fs::create_dir_all(&by_hand).unwrap();
    std::fs::write(by_hand.join("brief.md"), "# By hand\n\n## Why\n\nWritten by a person.\n").unwrap();
    let out = run(dir.path(), &["change", "new", "by-hand", "--subject", "auth", "--title", "By hand"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains(".canon/changes/by-hand` already exists"), "{}", stderr(&out));
    assert_eq!(std::fs::read_to_string(by_hand.join("brief.md")).unwrap(), "# By hand\n\n## Why\n\nWritten by a person.\n");
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
    assert!(!repo.join(".canon/changes/add-login").exists(), "the change dir must be removed");
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
        "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  subject: local\n  change: local\n",
    )
    .unwrap();
    assert!(run(dir.path(), &["subject", "new", "auth", "--domain", "dev", "--title", "Auth"]).status.success());
    // `.canon/changes` is a file, so `.canon/changes/<slug>` cannot be created.
    std::fs::write(dir.path().join(".canon/changes"), "not a directory\n").unwrap();

    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]);
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert!(stderr(&out).contains("failed to create `.canon/changes/add-login`"), "{}", stderr(&out));
    assert!(stdout(&out).is_empty(), "nothing may be reported written: {}", stdout(&out));
    assert_eq!(std::fs::read_to_string(dir.path().join(".canon/changes")).unwrap(), "not a directory\n");
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
    assert!(!dir.path().join(".canon/changes").exists(), "the changes dir this run created is removed again");
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
    assert!(dir.path().join(".canon/changes/add-login/brief.md").is_file(), "the change dir stays for the repair to import");
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
    assert!(changes.iter().any(|c| c["change_id"] == "add-login" && c["subject_ids"] == serde_json::json!(["auth"])), "{changes:?}");

    // Idempotent: a second run writes nothing.
    let before = ledger_file_count(dir.path());
    let again = run(dir.path(), &["subject", "adopt", "add-login", "--subject", "auth"]);
    assert!(again.status.success(), "{}", stderr(&again));
    assert!(stdout(&again).contains("already linked") && stdout(&again).contains("nothing written"), "{}", stdout(&again));
    assert_eq!(ledger_file_count(dir.path()), before, "a rerun on a linked pair must write no record");
}

/// 0.17 D2: an external openspec source configured beside canon's own
/// is never written into: `change new` writes `.canon/changes`, the
/// external change dir stays byte for byte, and the two import side by
/// side, each with its own provenance.
#[test]
fn change_new_writes_canon_changes_beside_an_external_openspec_source_it_never_touches() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  subject: local\n  change: local\n  task: local\nplans:\n  sources:\n    - { dialect: openspec, root: . }\n",
    )
    .unwrap();
    let external = dir.path().join("openspec/changes/ext-plan");
    std::fs::create_dir_all(&external).unwrap();
    std::fs::write(external.join("proposal.md"), "# Ext\n\n## Why\n\nA human's plan.\n").unwrap();
    let before = std::fs::read(external.join("proposal.md")).unwrap();
    assert!(run(dir.path(), &["subject", "new", "auth", "--domain", "dev", "--title", "Auth"]).status.success());

    let out = run(dir.path(), &["change", "new", "add-login", "--subject", "auth", "--title", "Add login"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(dir.path().join(".canon/changes/add-login/brief.md").is_file());
    assert!(!dir.path().join("openspec/changes/add-login").exists(), "nothing is written under the external source");
    assert_eq!(std::fs::read(external.join("proposal.md")).unwrap(), before);

    assert!(run(dir.path(), &["ingest", "plans"]).status.success());
    let changes = query(dir.path(), "change");
    let actor = |id: &str| changes.iter().find(|c| c["change_id"] == id).map(|c| c["actor"]["agent_id"].clone());
    assert_eq!(actor("add-login"), Some(Value::from("canon-plan-import-canon")), "{changes:?}");
    assert_eq!(actor("ext-plan"), Some(Value::from("canon-plan-import-openspec")), "{changes:?}");
}

/// subject.lifecycle.33 (0.18 D4, issue #28's repro): `change new
/// refactor-core --subject perception`, then `subject adopt refactor-core
/// --subject editing`. The second adopt adds the subject and says the
/// change now belongs to both; both subjects list the change, the change
/// lists both, status counts its open blocker against both, and that
/// blocker refuses each subject's move into review scope.
#[test]
fn subject_lifecycle_33_a_change_adopted_by_a_second_subject_belongs_to_both_and_its_blocker_gates_both() {
    use canon_model::{Actor, ChangeId, Envelope, Finding, FindingSeverity, RecordKind, RoleId};
    use canon_store::tier::Tier;

    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    assert!(run(repo, &["init"]).status.success());
    for id in ["perception", "editing"] {
        let out = run(repo, &["subject", "new", id, "--domain", "dev", "--title", id]);
        assert!(out.status.success(), "{}", stderr(&out));
    }
    let created = run(repo, &["change", "new", "refactor-core", "--subject", "perception", "--title", "Refactor core"]);
    assert!(created.status.success(), "{}", stderr(&created));

    let adopt = run(repo, &["subject", "adopt", "refactor-core", "--subject", "editing"]);
    assert!(adopt.status.success(), "{}", stderr(&adopt));
    assert_eq!(
        without_next(&stdout(&adopt)),
        "canon subject adopt: linked change `refactor-core` to subject `editing` as well; the change now belongs to `perception` and `editing` — written directly; nothing to promote\n"
    );

    let subjects = query(repo, "subject");
    for id in ["perception", "editing"] {
        let subject = subjects.iter().find(|s| s["subject_id"] == id).unwrap();
        assert_eq!(subject["change_ids"], serde_json::json!(["refactor-core"]), "{id} lists the change: {subject}");
    }
    let rows: Vec<Value> = query(repo, "change").into_iter().filter(|c| c["change_id"] == "refactor-core").collect();
    assert_eq!(rows.len(), 1, "`canon query --kind change` folds the versions to the current one: {rows:?}");
    assert_eq!(rows[0]["subject_ids"], serde_json::json!(["perception", "editing"]), "the change lists both, never re-pointed: {rows:?}");

    // An open blocker on the shared change.
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_evidence: true\n  require_review: {}\n").unwrap();
    let envelope = Envelope::new(1, RecordKind::Finding, chrono::Utc::now(), Actor::new("reviewer-2", RoleId::parse("reviewer").unwrap()));
    let finding = Finding::new(envelope, ChangeId::parse("refactor-core").unwrap(), 1, 1, FindingSeverity::Blocker, "reviewer-2", "core leaks a handle");
    canon_store::git_tier::GitTier::new(repo.join(".canon/ledger")).write(&finding).unwrap();

    let status: Value = serde_json::from_str(&stdout(&run(repo, &["status", "--json"]))).unwrap();
    for id in ["perception", "editing"] {
        let summary = status["subjects"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap();
        assert_eq!(summary["openBlockers"], 1, "{id}: {summary}");
    }
    for id in ["perception", "editing"] {
        for state in ["specced", "building"] {
            assert!(run(repo, &["subject", "status", id, state]).status.success());
        }
        let out = run(repo, &["subject", "status", id, "verifying"]);
        assert_eq!(out.status.code(), Some(1), "{id}: {}", stderr(&out));
        assert!(
            stderr(&out).contains("open-blocker refactor-core#1.1 — building → verifying: open blocker finding by `reviewer-2`"),
            "{id}: {}",
            stderr(&out)
        );
        assert!(stderr(&out).contains("ran open-blocker — 1 adopted change(s) checked"), "{id}: {}", stderr(&out));
    }
}
