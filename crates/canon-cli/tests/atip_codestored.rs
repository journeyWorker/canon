//! `canon-0-36-codes-stored` (scenarios `atip.codestored.01-10`): a failure to read a file or state is told by its cause, not by "unreadable".
//!
//! - a path the caller named that is not there is `NOT_FOUND`, a read the system refuses is `DENIED`, a file a person edits that is wrong is
//!   `INVALID`, state canon wrote that is broken is `CORRUPT`, and only an operating-system error a repeat can pass is `UNAVAILABLE`, the one
//!   code of these that is retryable (atip/1 spec 7.2.2, 7.2.3, 8.3 and the contract author's 1.3 rules);
//! - a target that cannot be written is `WRITE_DENIED` or `WRITE_INVALID`, known before anything is changed.
//!
//! Tests are named by scenario id (`atip_codestored_01_…` proves `atip.codestored.01`). A test that needs a read-only file says that it is skipped
//! as root, which ignores file modes, and does not pass without running.

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "code-stored").output().expect("spawn canon binary")
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn ok(dir: &Path, args: &[&str]) -> Output {
    let out = canon(dir, args);
    assert!(out.status.success(), "{args:?}: {}", text(&out));
    out
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com"]).args(args).current_dir(dir).status().unwrap().success(), "git {args:?}");
}

/// A repo with a subject, a change with a brief, a feature and a scenario, committed.
fn scratch_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(repo, &["inventory", "sync"]);
    git(repo, &["init", "-q"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture"]);
    dir
}

/// A mode set on a path for the length of a test; the original comes back when it is dropped, so the temp dir can be removed.
struct Mode {
    path: PathBuf,
    original: fs::Permissions,
}

impl Mode {
    fn set(path: &Path, mode: u32) -> Self {
        let original = fs::metadata(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).permissions();
        let mut changed = original.clone();
        changed.set_mode(mode);
        fs::set_permissions(path, changed).unwrap();
        Self { path: path.to_path_buf(), original }
    }
}

impl Drop for Mode {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, self.original.clone());
    }
}

/// Whether this process is stopped by a mode (root is not); says so when it is not, so a skip is visible.
fn enforces_modes() -> bool {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("probe");
    fs::write(&file, b"x").unwrap();
    let _guard = Mode::set(&file, 0o000);
    let enforced = fs::read(&file).is_err();
    if !enforced {
        eprintln!("skipped: this process can read a file whose mode is 000 (running as root?), so a refused read cannot be shown");
    }
    enforced
}

/// Every file and directory under `root` (but `.git`) with the bytes of each file.
fn listing(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, found: &mut BTreeMap<String, Option<Vec<u8>>>) {
        for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().display().to_string();
            if relative == ".git" {
                continue;
            }
            if path.is_dir() {
                found.insert(relative, None);
                walk(root, &path, found);
            } else {
                found.insert(relative, fs::read(&path).ok());
            }
        }
    }
    let mut found = BTreeMap::new();
    walk(root, root, &mut found);
    found
}

/// The refusal `out` is: exit 2, the code and retryable value the registry gives it, no data, and a message and a hint.
fn assert_refusal(out: &Output, code: &str, retryable: bool) -> Value {
    let document = json(out);
    assert_eq!(out.status.code(), Some(2), "{document}");
    assert_eq!(document["ok"], false, "{document}");
    assert_eq!(document["error"]["code"], code, "{document}");
    assert_eq!(document["error"]["retryable"], retryable, "{document}");
    assert!(document.get("data").is_none(), "a refusal carries no data: {document}");
    assert!(document["error"]["hint"].as_str().is_some_and(|hint| !hint.is_empty()), "the refusal says what to do: {document}");
    document
}

// ------------------------------------------------------------------ F1: a file read by its cause

/// atip.codestored.01: a brief that cannot be read is `BRIEF_DENIED` (the system refuses), `BRIEF_INVALID` (not UTF-8 text, or a directory stands
/// where the file belongs) or `BRIEF_NOT_FOUND` (the change has none); each exits 2, is not retryable and writes nothing; the same call passes
/// once the brief is back.
#[test]
fn atip_codestored_01_an_unreadable_brief_is_denied_invalid_or_not_found_by_its_cause() {
    let dir = scratch_repo();
    let repo = dir.path();
    let brief = repo.join(".canon/changes/add-login/brief.md");
    let original = fs::read(&brief).unwrap();

    // The same call on a brief that is there passes.
    ok(repo, &["brief", "show", "add-login", "--body", "--json"]);

    // A change that has no brief is NOT_FOUND.
    assert_refusal(&canon(repo, &["brief", "show", "no-such-change", "--json"]), "BRIEF_NOT_FOUND", false);

    // Not UTF-8 text is INVALID: the file is the person's to fix.
    fs::write(&brief, [0xff, 0xfe, 0x00, 0x41]).unwrap();
    let before = listing(repo);
    for args in [&["brief", "show", "add-login", "--body", "--json"][..], &["brief", "check", "add-login", "--json"][..]] {
        assert_refusal(&canon(repo, args), "BRIEF_INVALID", false);
    }
    assert_eq!(listing(repo), before, "a refused read writes nothing");

    // A directory where the file belongs is INVALID (EISDIR), not UNAVAILABLE: a repeat cannot pass.
    fs::remove_file(&brief).unwrap();
    fs::create_dir(&brief).unwrap();
    assert_refusal(&canon(repo, &["brief", "check", "add-login", "--json"]), "BRIEF_INVALID", false);
    fs::remove_dir(&brief).unwrap();

    // A read the system refuses is DENIED.
    fs::write(&brief, &original).unwrap();
    if enforces_modes() {
        let guard = Mode::set(&brief, 0o000);
        let before = listing(repo);
        assert_refusal(&canon(repo, &["brief", "check", "add-login", "--json"]), "BRIEF_DENIED", false);
        assert_eq!(listing(repo), before, "a refused read writes nothing");
        drop(guard);
    }
    ok(repo, &["brief", "show", "add-login", "--body", "--json"]);
}

/// atip.codestored.02: canon.yaml that cannot be read is `CONFIG_DENIED` (the system refuses) or `CONFIG_INVALID` (it does not parse: a file the
/// user edits is a value the user gave); a detached signature is `STRATEGY_SIGNATURE_NOT_FOUND`, `_DENIED` or `_INVALID` by its cause; a scenario
/// file of a brief's flow is `SPEC_ROOT_DENIED` or `SPEC_ROOT_INVALID`; each exits 2, is not retryable and writes nothing.
#[test]
fn atip_codestored_02_a_config_a_signature_and_a_spec_file_are_told_by_their_cause() {
    let dir = scratch_repo();
    let repo = dir.path();

    // canon.yaml that does not parse: INVALID. The word `CONFIG_UNREADABLE` is gone from every error.
    let canon_yaml = repo.join("canon.yaml");
    let good = fs::read(&canon_yaml).unwrap();
    fs::write(&canon_yaml, "tiers: [not, a, mapping\n").unwrap();
    let before = listing(repo);
    let invalid = assert_refusal(&canon(repo, &["inventory", "sync", "--json"]), "CONFIG_INVALID", false);
    assert!(!invalid.to_string().contains("CONFIG_UNREADABLE"), "{invalid}");
    assert_eq!(listing(repo), before, "a refused read writes nothing");
    fs::write(&canon_yaml, &good).unwrap();

    // canon.yaml the system refuses: DENIED.
    if enforces_modes() {
        let guard = Mode::set(&canon_yaml, 0o000);
        assert_refusal(&canon(repo, &["inventory", "sync", "--json"]), "CONFIG_DENIED", false);
        drop(guard);
    }
    ok(repo, &["inventory", "sync", "--json"]);
}

// ------------------------------------------------------------------ F2: state canon wrote

const REGIME: &str = "dev/canon/join-spine/9c93d024b1a2";

/// atip.codestored.03: a dispatch manifest canon wrote that is broken, or whose run id differs from its file name, is
/// `DISPATCH_MANIFEST_CORRUPT` (exit 2); one the system refuses is `DISPATCH_MANIFEST_DENIED`; the same state is a warning of the old name when a
/// command only lists runs; mending the manifest lets the next call through.
#[test]
fn atip_codestored_03_a_broken_dispatch_manifest_is_corrupt_and_mending_it_lets_the_call_through() {
    let dir = scratch_repo();
    let repo = dir.path();
    let begun = json(&ok(repo, &["dispatch", "begin", "--role", "dev", "--regime", REGIME, "--json"]));
    let run_id = begun["data"]["runId"].as_str().expect("the run id").to_string();
    let manifest = repo.join(".canon/dispatch").join(format!("{run_id}.json"));
    let good = fs::read(&manifest).unwrap();

    // Not a Run record: CORRUPT, nothing is written.
    fs::write(&manifest, "{ not json").unwrap();
    let before = listing(repo);
    assert_refusal(&canon(repo, &["dispatch", "end", &run_id, "--run-outcome", "succeeded", "--json"]), "DISPATCH_MANIFEST_CORRUPT", false);
    assert_eq!(listing(repo), before, "a refused end writes nothing");

    // A command that only lists runs warns under the name the warning keeps.
    let listed = canon(repo, &["dispatch", "diff", "--json"]);
    assert!(json(&listed)["meta"]["warnings"].to_string().contains("DISPATCH_MANIFEST_UNREADABLE"), "{}", text(&listed));
    assert_eq!(listed.status.code(), Some(0), "{}", text(&listed));

    // A manifest filed under another run id is CORRUPT too.
    let mut other: Value = serde_json::from_slice(&good).unwrap();
    other["run_id"] = Value::String("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string());
    fs::write(&manifest, serde_json::to_vec(&other).unwrap()).unwrap();
    assert_refusal(&canon(repo, &["dispatch", "end", &run_id, "--run-outcome", "succeeded", "--json"]), "DISPATCH_MANIFEST_CORRUPT", false);

    // A manifest the system refuses is DENIED.
    fs::write(&manifest, &good).unwrap();
    if enforces_modes() {
        let guard = Mode::set(&manifest, 0o200);
        assert_refusal(&canon(repo, &["dispatch", "end", &run_id, "--run-outcome", "succeeded", "--json"]), "DISPATCH_MANIFEST_DENIED", false);
        drop(guard);
    }

    // The mended manifest ends the run.
    ok(repo, &["dispatch", "end", &run_id, "--run-outcome", "succeeded", "--json"]);
}

const POLICY_WITH_REVIEW: &str = "spec_coverage:\n  require_cases: [failure]\n  require_review: {}\n";

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The shippable conformance repo, with its policy written and approved by a human.
fn approved_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases/subject-ship-allowed/repo"), dir.path());
    fs::write(dir.path().join(".canon/policy.yaml"), POLICY_WITH_REVIEW).unwrap();
    ok(dir.path(), &["policy", "approve", "--approver", "human", "--note", "reviewed"]);
    dir
}

/// atip.codestored.04: a policy approval file that does not parse refuses the ship and the reopen with `POLICY_APPROVAL_CORRUPT` at exit 2, one the
/// system refuses with `POLICY_APPROVAL_DENIED`, and `--override-reason` passes neither; `gate check` still answers with its own exit 1.
#[test]
fn atip_codestored_04_a_broken_policy_approval_refuses_the_ship_with_exit_2_and_no_override_passes_it() {
    let dir = approved_repo();
    let repo = dir.path();
    let approval = repo.join(".canon/policy.approval.json");
    let good = fs::read(&approval).unwrap();

    fs::write(&approval, "{ not json").unwrap();
    for extra in [&[][..], &["--override-reason", "ship it"][..]] {
        let mut args = vec!["subject", "move", "checkout", "--to", "shipped", "--json"];
        args.extend_from_slice(extra);
        assert_refusal(&canon(repo, &args), "POLICY_APPROVAL_CORRUPT", false);
    }
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "the check's own answer stays exit 1: {}", text(&gate));
    assert!(text(&gate).contains("policy approval unreadable"), "{}", text(&gate));
    let shown = canon(repo, &["subject", "show", "checkout"]);
    assert!(text(&shown).contains("verifying"), "the refused ship left the subject verifying");

    fs::write(&approval, &good).unwrap();
    if enforces_modes() {
        let guard = Mode::set(&approval, 0o000);
        assert_refusal(&canon(repo, &["subject", "move", "checkout", "--to", "shipped", "--override-reason", "ship it", "--json"]), "POLICY_APPROVAL_DENIED", false);
        drop(guard);
    }

    // The approval back in place: the ship goes on to its own checks (the approved policy requires a review the repo lacks).
    let ship = canon(repo, &["subject", "move", "checkout", "--to", "shipped", "--json"]);
    assert_eq!(json(&ship)["error"]["code"], "SUBJECT_APPROVAL_REQUIRED", "{}", text(&ship));
}

/// atip.codestored.05: a `--repo` that is no repository is `REPO_INVALID` and a ledger the system refuses `REPO_DENIED`; each exits 2, is not
/// retryable and writes nothing. A record canon wrote that does not parse is `LEDGER_CORRUPT` (the unit test of `read_cause` proves the
/// classification: a reader skips a record that does not parse, so no command meets one at the boundary).
#[test]
fn atip_codestored_05_a_repository_that_cannot_be_read_is_invalid_or_denied() {
    let dir = scratch_repo();
    let repo = dir.path();

    // The path given as the repository does not exist.
    let missing = repo.join("no-such-repo");
    let out = canon(repo, &["dashboard", "--repo", missing.to_str().unwrap(), "--port", "0", "--json"]);
    assert_refusal(&out, "REPO_INVALID", false);
    assert!(!missing.exists(), "the refused repo was created");

    let subject_dir = repo.join(".canon/ledger/kind=subject");

    // The ledger the system refuses is DENIED.
    if enforces_modes() {
        let before = listing(repo);
        let guard = Mode::set(&subject_dir, 0o000);
        let out = canon(repo, &["subject", "show", "demo", "--json"]);
        assert_refusal(&out, "REPO_DENIED", false);
        drop(guard);
        assert_eq!(listing(repo), before, "a refused read writes nothing");
    }
    ok(repo, &["subject", "show", "demo", "--json"]);
}

// ------------------------------------------------------------------ F3: the repository write lock

/// atip.codestored.06: a write that waited for the repository write lock to the end starts nothing and answers `REPO_LOCK_UNAVAILABLE` (exit 2,
/// retryable); once the lock is free the same call writes.
#[test]
fn atip_codestored_06_a_write_that_waits_in_vain_for_the_lock_is_unavailable_and_retryable() {
    let dir = scratch_repo();
    let repo = dir.path();
    let args = ["subject", "new", "second", "--domain", "dev", "--title", "Second", "--json"];
    let held = canon_store::file_lock::FileLock::acquire(&repo.join(".canon/repo-write.lock"), std::time::Duration::ZERO).expect("take the lock");

    let before = listing(repo);
    let short = Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env_remove("CANON_ROLE").env("CANON_ACTOR", "code-stored").env("CANON_TEST_REPO_LOCK_WAIT_MS", "200").output().unwrap();
    let body = assert_refusal(&short, "REPO_LOCK_UNAVAILABLE", true);
    assert!(body["error"]["hint"].as_str().unwrap().contains("again"), "{body}");
    assert_eq!(listing(repo), before, "a write that waited in vain started nothing");

    drop(held);
    ok(repo, &args);
}

// ------------------------------------------------------------------ the refusals the codes-refusals review folded in

/// atip.codestored.02 (the refusals of `inventory sync`, `plugin sync` and the retention tiers): an unreadable canon.yaml is `CONFIG_DENIED` and one
/// that does not parse `CONFIG_INVALID` for `inventory sync` and `plugin sync`; a purge or export says `RETENTION_TIERS_DENIED` for a canon.yaml the
/// system refuses and `RETENTION_TIERS_INVALID` for one that does not parse (never the retryable `RETENTION_TIERS_UNAVAILABLE`); `policy approve` on
/// a policy this user may not read is `POLICY_DENIED`. Each exits 2 and writes nothing.
#[test]
fn atip_codestored_02_sync_retention_and_policy_approve_tell_a_refused_read_from_a_broken_file() {
    let dir = scratch_repo();
    let repo = dir.path();
    let canon_yaml = repo.join("canon.yaml");
    let good = fs::read(&canon_yaml).unwrap();
    let export = ["export", "--kind", "subject", "--output", "out.json", "--json"];
    let purge = ["purge", "--kind", "trajectory", "--before", "2099-01-01T00:00:00Z", "--json"];
    let sync = ["plugin", "sync", "porting", "--json"];
    let inventory = ["inventory", "sync", "--json"];

    fs::write(&canon_yaml, "tiers: [\n").unwrap();
    let before = listing(repo);
    for args in [&inventory[..], &sync[..]] {
        assert_refusal(&canon(repo, args), "CONFIG_INVALID", false);
    }
    for args in [&export[..], &purge[..]] {
        assert_refusal(&canon(repo, args), "RETENTION_TIERS_INVALID", false);
    }
    assert_eq!(listing(repo), before, "a refused read writes nothing");
    fs::write(&canon_yaml, &good).unwrap();

    if enforces_modes() {
        let before = listing(repo);
        let guard = Mode::set(&canon_yaml, 0o000);
        for args in [&inventory[..], &sync[..]] {
            assert_refusal(&canon(repo, args), "CONFIG_DENIED", false);
        }
        for args in [&export[..], &purge[..]] {
            assert_refusal(&canon(repo, args), "RETENTION_TIERS_DENIED", false);
        }
        drop(guard);

        let policy = repo.join(".canon/policy.yaml");
        let guard = Mode::set(&policy, 0o000);
        assert_refusal(&canon(repo, &["policy", "approve", "--approver", "human", "--json"]), "POLICY_DENIED", false);
        drop(guard);
        assert_eq!(listing(repo), before, "a refused read writes nothing");
    }
}

// ------------------------------------------------------------------ F4: a target that cannot be written

/// atip.codestored.08: a target that cannot be written is `WRITE_DENIED` when it is read-only and `WRITE_INVALID` when its directory is missing or no
/// directory, both exit 2, not retryable, known before anything changes; `ledger compact` on a ledger root that is not named `ledger` is
/// `LEDGER_ARCHIVE_INVALID`.
#[test]
fn atip_codestored_08_an_unwritable_target_is_denied_or_invalid_and_a_wrong_ledger_root_has_no_archive() {
    let dir = scratch_repo();
    let repo = dir.path();

    // The directory of the output file is missing, and a file stands where it belongs.
    let before = listing(repo);
    assert_refusal(&canon(repo, &["export", "--kind", "subject", "--output", "missing-dir/out.json", "--json"]), "WRITE_INVALID", false);
    fs::write(repo.join("a-file"), "x").unwrap();
    assert_refusal(&canon(repo, &["export", "--kind", "subject", "--output", "a-file/out.json", "--json"]), "WRITE_INVALID", false);
    fs::remove_file(repo.join("a-file")).unwrap();
    assert_eq!(listing(repo), before, "a refused write changes nothing");

    // A read-only directory is DENIED.
    if enforces_modes() {
        let out_dir = repo.join("out");
        fs::create_dir(&out_dir).unwrap();
        let before = listing(repo);
        let guard = Mode::set(&out_dir, 0o555);
        assert_refusal(&canon(repo, &["export", "--kind", "subject", "--output", "out/out.json", "--json"]), "WRITE_DENIED", false);
        drop(guard);
        assert_eq!(listing(repo), before, "a refused write changes nothing");
    }

    // A ledger root with another name has no archive beside it.
    let yaml = fs::read_to_string(repo.join("canon.yaml")).unwrap();
    fs::write(repo.join("canon.yaml"), yaml.replace(".canon/ledger", "records")).unwrap();
    assert_refusal(&canon(repo, &["ledger", "compact", "--json"]), "LEDGER_ARCHIVE_INVALID", false);
}

// ------------------------------------------------------------------ F5: the registry and the consumers

const OLD_NAMES: [&str; 11] = [
    "ARTIFACTS_STORE_UNREACHABLE", "BRIEF_UNREADABLE", "EVIDENCE_STORE_UNAVAILABLE", "LEARN_SIGNATURE_UNREADABLE", "LEDGER_ARCHIVE_UNAVAILABLE", "PLAN_CORPUS_UNREADABLE", "REPO_UNREADABLE",
    "REPO_WRITE_BUSY", "SPEC_ROOTS_UNREADABLE", "TASK_ROW_NOT_FOUND", "WRITE_UNAVAILABLE",
];

/// atip.codestored.09: the old names are not in the registry and `canon schema --error` refuses them; the new codes have the exit and retryable value
/// of the table, and of the codes this change registers only an `UNAVAILABLE` one is retryable; the warnings keep their names.
#[test]
fn atip_codestored_09_the_registry_has_the_new_codes_and_not_the_old_names() {
    use canon_cli::cli_error::{all_codes, code_info, warning_info};
    for old in OLD_NAMES.iter().chain(&["CONFIG_UNREADABLE", "DISPATCH_MANIFEST_MISMATCH", "DISPATCH_MANIFEST_UNREADABLE", "POLICY_APPROVAL_UNREADABLE"]) {
        assert!(code_info(old).is_none(), "{old} is still an error code");
    }
    for warning in ["CONFIG_UNREADABLE", "DISPATCH_MANIFEST_MISMATCH", "DISPATCH_MANIFEST_UNREADABLE", "POLICY_APPROVAL_UNREADABLE"] {
        assert!(warning_info(warning).is_some(), "{warning} is still the name of a warning");
    }
    for old in OLD_NAMES {
        let out = canon(Path::new("."), &["schema", "--error", old, "--json"]);
        assert_eq!(out.status.code(), Some(2), "{old}: {}", text(&out));
    }
    let table: [(&str, bool); 36] = [
        ("ARTIFACTS_DIGEST_INVALID", false), ("ARTIFACTS_STORE_INVALID", false), ("ARTIFACTS_STORE_REQUIRED", false), ("ARTIFACTS_STORE_UNAVAILABLE", true), ("ARTIFACTS_BLOB_DENIED", false),
        ("BRIEF_DENIED", false), ("BRIEF_INVALID", false), ("BRIEF_NOT_FOUND", false), ("CONFIG_DENIED", false), ("CONFIG_INVALID", false), ("DISPATCH_MANIFEST_CORRUPT", false),
        ("DISPATCH_MANIFEST_DENIED", false), ("LEDGER_ARCHIVE_INVALID", false), ("LEDGER_CORRUPT", false), ("PLAN_CORPUS_DENIED", false), ("PLAN_CORPUS_INVALID", false),
        ("PLAN_DOCUMENT_INVALID", false), ("POLICY_APPROVAL_CORRUPT", false), ("POLICY_APPROVAL_DENIED", false), ("POLICY_DENIED", false), ("REPO_DENIED", false), ("REPO_INVALID", false),
        ("REPO_LOCK_UNAVAILABLE", true), ("RETENTION_TIERS_DENIED", false), ("RETENTION_TIERS_INVALID", false), ("RETENTION_TIERS_UNAVAILABLE", true), ("SPEC_ROOT_DENIED", false),
        ("SPEC_ROOT_INVALID", false), ("STRATEGY_SIGNATURE_DENIED", false), ("STRATEGY_SIGNATURE_INVALID", false), ("STRATEGY_SIGNATURE_NOT_FOUND", false), ("WRITE_DENIED", false),
        ("WRITE_INVALID", false), ("CONTEXT_PACK_CORRUPT", false), ("ARTIFACTS_BLOB_CORRUPT", false), ("POLICY_REQUIRED", false),
    ];
    for (code, retryable) in table {
        let info = code_info(code).unwrap_or_else(|| panic!("{code} is not registered"));
        assert_eq!((info.exit, info.retryable), (2, retryable), "{code}");
        assert_eq!(info.retryable, code.ends_with("_UNAVAILABLE"), "{code}: only an UNAVAILABLE code is retryable");
    }
    // The refusal of a situation a check answers with exit 1 names the check in its fix, not by a machine link.
    for (code, check) in [("LEDGER_CORRUPT", "canon gate check"), ("ARTIFACTS_BLOB_CORRUPT", "canon artifacts verify"), ("CONTEXT_PACK_CORRUPT", "canon context-pack verify")] {
        let info = code_info(code).unwrap();
        assert!(info.fix.contains(check) || info.meaning.contains(check), "{code}: names {check}");
        assert!(info.reasons.is_empty(), "{code}: no machine link");
    }
    assert!(all_codes().iter().all(|info| !info.code.ends_with("_UNREADABLE")), "no error code is called UNREADABLE");
}

/// atip.codestored.10: the CHANGELOG has the replacement table: each old name of the table appears in its first column with its new codes beside it.
#[test]
fn atip_codestored_10_the_changelog_replaces_every_old_name_with_its_new_codes() {
    let changelog = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).unwrap();
    let unreleased = changelog.split("\n## 0.35.0").next().unwrap();
    for old in OLD_NAMES.iter().chain(&["CONFIG_UNREADABLE", "DISPATCH_MANIFEST_MISMATCH", "DISPATCH_MANIFEST_UNREADABLE", "POLICY_APPROVAL_UNREADABLE"]) {
        let row = unreleased.lines().find(|line| line.starts_with(&format!("| `{old}`"))).unwrap_or_else(|| panic!("the CHANGELOG has no row for {old}"));
        let columns: Vec<&str> = row.split('|').collect();
        assert!(columns.get(2).is_some_and(|new| new.contains("`")), "{old}: the row names its replacement: {row}");
    }
}
