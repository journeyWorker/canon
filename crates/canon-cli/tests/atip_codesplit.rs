//! `canon-0-36-codes-refusals`, scenarios `atip.codesplit.01`-`.07`: the codes that said several refusals with one name say each with the code
//! of its condition (atip/1 spec 7.2.2), and a reason names what differs inside one condition. Against the built `canon` binary; the codes
//! of the other scenarios the split changed are asserted by the tests of those scenarios.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use canon_store::file_lock::FileLock;

use serde_json::Value;
use tempfile::TempDir;

const REGIME: &str = "dev/canon/join-spine/9c93d024b1a2";

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).arg("--json").current_dir(dir).env_remove("CANON_ROLE").env_remove("CANON_ACTOR").output().expect("spawn canon")
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

fn ok(dir: &Path, args: &[&str]) -> Value {
    let out = run(dir, args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    body(&out)
}

/// Runs `args`, expecting exit 2, `error.code == code`, `error.reason == reason` and no `data`; returns the document.
fn refused(dir: &Path, args: &[&str], code: &str, reason: Option<&str>) -> Value {
    let out = run(dir, args);
    assert_eq!(out.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    let doc = body(&out);
    assert_eq!(doc["ok"], false, "{doc}");
    assert_eq!(doc["error"]["code"], code, "{args:?}: {doc}");
    assert_eq!(doc["error"].get("reason").and_then(Value::as_str), reason, "{args:?}: {doc}");
    assert!(doc.get("data").is_none(), "a refusal carries no data: {doc}");
    doc
}

fn tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// A git repo with one commit, set up by `canon init`.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(dir.path()).output().unwrap();
        assert!(out.status.success(), "git {args:?}");
    }
    ok(dir.path(), &["init", "--providers", "none"]);
    dir
}

/// `run` with extra environment variables.
fn run_env(dir: &Path, args: &[&str], env: &[(&str, &Path)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).arg("--json").current_dir(dir).env_remove("CANON_ROLE").env_remove("CANON_ACTOR");
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().expect("spawn canon")
}

/// A mode set on a path for the length of a test; the original comes back when it is dropped, so the temp dir can be removed. `None` when this
/// process ignores the mode (it runs as root), so a refused read cannot be shown: the caller says so and skips that leg.
struct Mode {
    path: PathBuf,
    original: std::fs::Permissions,
}

impl Mode {
    /// `path` at `mode`, when that refuses this process a read of it (a file) or a write into it (a directory).
    fn set(path: &Path, mode: u32) -> Option<Self> {
        use std::os::unix::fs::PermissionsExt as _;
        let original = std::fs::metadata(path).unwrap().permissions();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
        let held = Self { path: path.to_path_buf(), original };
        let refused = if path.is_dir() { std::fs::File::create(path.join(".mode-probe")).is_err() } else { std::fs::File::open(path).is_err() };
        if refused {
            return Some(held);
        }
        let _ = std::fs::remove_file(path.join(".mode-probe"));
        eprintln!("skipped: this process can still use `{}` at mode {mode:o} (running as root?), so a refusal cannot be shown", path.display());
        None
    }
}

impl Drop for Mode {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.path, self.original.clone());
    }
}

/// atip.codesplit.01: no task or scenario is `EVIDENCE_REQUIRED`, an argument that forbids another is `EVIDENCE_ARGUMENT_CONFLICT`, an empty
/// value is `EVIDENCE_ARGUMENT_INVALID`; each exits 2 with no data and writes nothing.
#[test]
fn atip_codesplit_01_a_missing_subject_a_contradiction_and_an_empty_value_answer_with_their_own_condition() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let before = tree(d);
    refused(d, &["evidence", "add", "--class", "test-run", "--ref", "r", "--role", "implementer"], "EVIDENCE_REQUIRED", None);
    refused(d, &["evidence", "add", "--all-scenarios", "--scenario", "auth.login.01", "--report", "junit:r.xml", "--class", "test-run", "--ref", "r", "--role", "implementer"], "EVIDENCE_ARGUMENT_CONFLICT", None);
    refused(d, &["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", " ", "--role", "implementer"], "EVIDENCE_ARGUMENT_INVALID", None);
    refused(d, &["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "r", "--role", "implementer", "--command-result", "out.txt"], "EVIDENCE_ARGUMENT_REQUIRED", None);
    assert_eq!(tree(d), before, "a refusal writes nothing");
}

/// atip.codesplit.01: `evidence approval-payload` and `evidence add` refuse the same situation with the same code: a `--task` that names no task in the plan
/// corpus is `TASK_NOT_FOUND` in both (never a catch-all code), with the hint of the task and not of the other command; `EVIDENCE_INVALID`, the
/// catch-all they shared, is no longer a code. A malformed `--surface-ref` is refused by the argument validator (`USAGE`) in both commands before
/// any state is read; a `--surface-ref` that gets past it is `EVIDENCE_ARGUMENT_INVALID` with the field `--surface-ref` (the unit test of
/// `evidence.rs`, `atip_codesplit_01_a_surface_ref_that_gets_past_the_validator_is_an_invalid_argument`).
#[test]
fn atip_codesplit_01_a_task_that_names_no_task_is_task_not_found_for_the_payload_and_the_attestation_alike() {
    let dir = repo();
    let d = dir.path();
    let planner = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(args).arg("--json").current_dir(d).env("CANON_ROLE", "planner").env_remove("CANON_ACTOR").output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    };
    planner(&["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    planner(&["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    let before = tree(d);
    let added = refused(d, &["evidence", "add", "--task", "add-login#9", "--class", "test-run", "--ref", "r", "--role", "implementer"], "TASK_NOT_FOUND", None);
    let payload = refused(d, &["evidence", "approval-payload", "--task", "add-login#9", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z"], "TASK_NOT_FOUND", None);
    assert_eq!(added["error"]["message"], payload["error"]["message"], "the same situation reads the same");
    let hint = payload["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("task ids look like") && !hint.contains("evidence add"), "the hint is about the task: {hint}");
    assert_eq!(tree(d), before, "a refusal writes nothing");
    assert!(canon_cli::cli_error::code_info("EVIDENCE_INVALID").is_none(), "the catch-all is not a code");

    // A malformed surface ref is the same refusal from both commands.
    for args in [
        &["evidence", "add", "--task", "add-login#1", "--class", "test-run", "--ref", "r", "--role", "implementer", "--surface-ref", "../x"][..],
        &["evidence", "approval-payload", "--task", "add-login#1", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z", "--surface-ref", "../x"][..],
    ] {
        let doc = refused(d, args, "USAGE", None);
        assert_eq!(doc["error"]["field"], "--surface-ref", "{doc}");
    }
}

/// atip.codesplit.02: a file to bind that is outside the repository is `EVIDENCE_BINDING_DENIED`, a missing one `..._NOT_FOUND`, a directory
/// `..._INVALID`; the same for `review scenario` (`REVIEW_ARTIFACT_*`); each refusal exits 2 and writes nothing. A file that changed while it was
/// read is `EVIDENCE_BINDING_STALE` (`REVIEW_ARTIFACT_STALE`): no real call can change a file between its two reads, so those two codes are proved by
/// the unit tests `atip_codesplit_02_a_bound_file_that_changed_is_evidence_binding_stale` (`artifacts.rs`) and
/// `atip_codesplit_02_a_bound_file_that_changed_is_review_artifact_stale` (`review.rs`), which drive the mapping with the store's own error.
#[test]
fn atip_codesplit_02_a_file_to_bind_is_refused_by_its_condition_and_nothing_is_written() {
    let dir = repo();
    let d = dir.path();
    let outside = TempDir::new().unwrap();
    std::fs::write(outside.path().join("trace.zip"), b"elsewhere").unwrap();
    std::fs::create_dir_all(d.join("reports/dir")).unwrap();
    std::fs::write(d.join("reports/ok.txt"), b"fine").unwrap();
    ok(d, &["scenario", "new", "auth.login.01", "--title", "A user signs in", "--case", "happy"]);
    ok(d, &["inventory", "sync"]);
    let before = tree(d);
    let base = ["evidence", "add", "--scenario", "auth.login.01", "--project", "root", "--class", "test-run", "--ref", "r", "--role", "implementer", "--artifact"];
    let with = |artifact: &str| -> Vec<String> { base.iter().map(ToString::to_string).chain([artifact.to_string()]).collect() };
    for (artifact, code) in [
        (outside.path().join("trace.zip").to_string_lossy().into_owned(), "EVIDENCE_BINDING_DENIED"),
        ("reports/none.png".to_string(), "EVIDENCE_BINDING_NOT_FOUND"),
        ("reports/dir".to_string(), "EVIDENCE_BINDING_INVALID"),
    ] {
        let args = with(&artifact);
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        refused(d, &words, code, None);
    }
    for (artifact, code) in [
        (outside.path().join("trace.zip").to_string_lossy().into_owned(), "REVIEW_ARTIFACT_DENIED"),
        ("reports/none.png".to_string(), "REVIEW_ARTIFACT_NOT_FOUND"),
        ("reports/dir".to_string(), "REVIEW_ARTIFACT_INVALID"),
    ] {
        refused(d, &["review", "scenario", "auth.login.01", "--project", "root", "--actor", "r", "--role", "reviewer", "--original-spec-ref", "HEAD", "--artifact", &artifact], code, None);
    }
    assert_eq!(tree(d), before, "a refused binding stores no bytes and writes no record");
}

/// atip.codesplit.03: a run that was already ended is `DISPATCH_CONFLICT` with the reason `ALREADY_ENDED`, the manifest keeps its bytes; a manifest
/// whose recorded status is not a run that can be closed (never dispatched live, or terminal without an end time) is the reason `NOT_RUNNING`.
#[test]
fn atip_codesplit_03_ending_a_run_twice_is_a_conflict_with_the_reason_already_ended() {
    let dir = repo();
    let d = dir.path();
    let begun = ok(d, &["dispatch", "begin", "--role", "dev", "--regime", REGIME]);
    let run_id = begun["data"]["runId"].as_str().unwrap().to_string();
    ok(d, &["dispatch", "end", &run_id, "--run-outcome", "succeeded"]);
    let manifest = d.join(".canon/dispatch").join(format!("{run_id}.json"));
    let before = std::fs::read(&manifest).unwrap();
    refused(d, &["dispatch", "end", &run_id, "--run-outcome", "failed"], "DISPATCH_CONFLICT", Some("ALREADY_ENDED"));
    assert_eq!(std::fs::read(&manifest).unwrap(), before);

    // A run recorded as pending, or as finished with no end time, cannot be closed: NOT_RUNNING, and the manifest keeps its bytes.
    let begun = ok(d, &["dispatch", "begin", "--role", "dev", "--regime", REGIME]);
    let second = begun["data"]["runId"].as_str().unwrap().to_string();
    let manifest = d.join(".canon/dispatch").join(format!("{second}.json"));
    let running = std::fs::read_to_string(&manifest).unwrap();
    assert!(running.contains("\"status\": \"running\""), "{running}");
    for status in ["pending", "succeeded"] {
        std::fs::write(&manifest, running.replace("\"status\": \"running\"", &format!("\"status\": \"{status}\""))).unwrap();
        let held = std::fs::read(&manifest).unwrap();
        let doc = refused(d, &["dispatch", "end", &second, "--run-outcome", "succeeded"], "DISPATCH_CONFLICT", Some("NOT_RUNNING"));
        assert!(doc["error"]["message"].as_str().unwrap().contains(status), "{doc}");
        assert_eq!(std::fs::read(&manifest).unwrap(), held, "a refused end leaves the manifest as it was");
    }
    std::fs::write(&manifest, &running).unwrap();
    ok(d, &["dispatch", "end", &second, "--run-outcome", "succeeded"]);
}

/// atip.codesplit.04: a malformed `skills:` or `specs:` section is `CONFIG_INVALID` with the reason `SKILLS` or `SPECS`, an unknown provider is
/// `SKILLS_ARGUMENT_INVALID`, a missing `--skills-source` is `SKILLS_SOURCE_NOT_FOUND`; each exits 2 and writes nothing.
#[test]
fn atip_codesplit_04_a_broken_section_and_a_bad_provider_answer_with_their_condition_and_reason() {
    let dir = repo();
    let d = dir.path();
    let yaml = std::fs::read_to_string(d.join("canon.yaml")).unwrap();
    refused(d, &["skills", "install", "--providers", "bogus"], "SKILLS_ARGUMENT_INVALID", None);
    refused(d, &["skills", "install", "--skills-source", "no-such-dir"], "SKILLS_SOURCE_NOT_FOUND", None);
    std::fs::write(d.join("canon.yaml"), format!("{yaml}\nskills: [claude]\n")).unwrap();
    let before = tree(d);
    refused(d, &["skills", "install"], "CONFIG_INVALID", Some("SKILLS"));
    assert_eq!(tree(d), before);
    std::fs::write(d.join("canon.yaml"), format!("{yaml}\nspecs:\n  bogus: 1\n")).unwrap();
    let before = tree(d);
    refused(d, &["inventory", "sync"], "CONFIG_INVALID", Some("SPECS"));
    refused(d, &["plugin", "sync", "some-plugin"], "CONFIG_INVALID", Some("SPECS"));
    assert_eq!(tree(d), before);

    // With the section mended, `plugin sync` goes on to its own check (the plugin is not installed), and it never waits for the scenario-index lock:
    // it takes none, so a lock another process holds is no refusal of it.
    std::fs::write(d.join("canon.yaml"), &yaml).unwrap();
    let held = FileLock::acquire(&d.join(".canon/scenario-index.lock"), Duration::ZERO).unwrap();
    let out = run_env(d, &["plugin", "sync", "some-plugin"], &[("CANON_TEST_INDEX_LOCK_WAIT_MS", Path::new("200"))]);
    let doc = body(&out);
    assert_eq!((out.status.code(), doc["error"]["code"].as_str()), (Some(2), Some("PLUGIN_REQUIRED")), "{doc}");
    drop(held);
}

/// atip.codesplit.05: no live change with a brief is `BRIEF_REQUIRED`, a named change without one `BRIEF_NOT_FOUND`; after `canon change new`
/// the same call answers.
#[test]
fn atip_codesplit_05_no_brief_and_a_named_change_without_one_answer_with_their_own_code_and_the_fix_lets_the_call_through() {
    let dir = repo();
    let d = dir.path();
    refused(d, &["brief", "show"], "BRIEF_REQUIRED", None);
    refused(d, &["brief", "show", "no-such-change"], "BRIEF_NOT_FOUND", None);
    let created = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["subject", "new", "demo", "--domain", "dev", "--title", "Demo", "--json"])
        .current_dir(d)
        .env("CANON_ROLE", "planner")
        .env_remove("CANON_ACTOR")
        .output()
        .unwrap();
    assert_eq!(created.status.code(), Some(0), "{}", String::from_utf8_lossy(&created.stdout));
    let change = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["change", "new", "add-login", "--subject", "demo", "--title", "Add login", "--json"])
        .current_dir(d)
        .env("CANON_ROLE", "planner")
        .env_remove("CANON_ACTOR")
        .output()
        .unwrap();
    assert_eq!(change.status.code(), Some(0), "{}", String::from_utf8_lossy(&change.stdout));
    ok(d, &["brief", "show"]);

    // A scenario no spec root declares is SCENARIO_INDEX_REQUIRED from `evidence approval-payload`, and the fix lets the same call through.
    let payload = ["evidence", "approval-payload", "--scenario", "auth.login.01", "--project", "root", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z"];
    let before = tree(d);
    let doc = refused(d, &payload, "SCENARIO_INDEX_REQUIRED", None);
    assert!(doc["error"]["hint"].as_str().unwrap().contains("canon inventory sync"), "{doc}");
    assert_eq!(tree(d), before, "a refusal writes nothing");
    ok(d, &["scenario", "new", "auth.login.01", "--title", "A user signs in", "--case", "happy"]);
    ok(d, &["inventory", "sync"]);
    assert!(ok(d, &payload)["data"]["payloadHex"].as_str().is_some_and(|hex| !hex.is_empty()), "the same call now gives the bytes to sign");
}

/// A copy of the 0.29 ledger fixture in a git repo with one commit: a repo `canon migrate` has a renaming to preview in.
fn old_repo() -> TempDir {
    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }
    }
    let dir = TempDir::new().unwrap();
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger-0.29"), dir.path());
    std::fs::rename(dir.path().join("gitignore"), dir.path().join(".gitignore")).unwrap();
    for args in [&["init", "-q"][..], &["add", "-A"], &["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "fixture"]] {
        let out = Command::new("git").args(args).current_dir(dir.path()).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    dir
}

/// atip.codesplit.07: a file or lock the system refuses is told by the code of its own subject, each exit 2 and nothing written: an adapter response
/// this user may not read is `ADAPTER_FILE_DENIED`; a hook file under `.claude` that it may not read is `HOOKS_FILE_DENIED`; a policy that is a directory is
/// `POLICY_INVALID`; a preview of `migrate` that cannot use the temp directory is `MIGRATION_DENIED`; an artifact tier that is not an object store is
/// `ARTIFACTS_TIER_INVALID`; a canon.yaml the system refuses or that does not parse is `RETENTION_TIERS_DENIED` or `RETENTION_TIERS_INVALID` for a purge or an
/// export (a backend that is down is `RETENTION_TIERS_UNAVAILABLE`, which only a unit test can show: `retention.rs`,
/// `atip_codestored_13_the_tiers_code_is_told_by_what_is_wrong`); a scenario index another process holds is `INVENTORY_INDEX_UNAVAILABLE`, retryable.
/// The refused-read legs are skipped when the test runs as root, which ignores file modes, and say so.
#[test]
fn atip_codesplit_07_a_file_or_lock_the_system_refuses_is_told_by_the_code_of_its_own_subject() {
    let dir = repo();
    let d = dir.path();

    // An adapter response this user may not read.
    let response = d.join("resp.json");
    std::fs::write(&response, "{}").unwrap();
    let before = tree(d);
    if let Some(guard) = Mode::set(&response, 0o000) {
        refused(d, &["adapter", "validate", "--response", "resp.json"], "ADAPTER_FILE_DENIED", None);
        refused(d, &["adapter", "authorize", "--response", "resp.json"], "ADAPTER_FILE_DENIED", None);
        drop(guard);
    }
    assert_eq!(tree(d), before, "a refused read writes nothing");

    // A hook file this user may not read: nothing is installed.
    std::fs::create_dir_all(d.join(".claude")).unwrap();
    let hooks = d.join(".claude/settings.json");
    std::fs::write(&hooks, "{}\n").unwrap();
    let before = tree(d);
    if let Some(guard) = Mode::set(&hooks, 0o000) {
        refused(d, &["gate", "install-hooks"], "HOOKS_FILE_DENIED", None);
        drop(guard);
    }
    assert_eq!(tree(d), before, "a refused install writes nothing");

    // A policy that is a directory.
    let policy = d.join(".canon/policy.yaml");
    let kept = std::fs::read(&policy).ok();
    if policy.exists() {
        std::fs::remove_file(&policy).unwrap();
    }
    std::fs::create_dir(&policy).unwrap();
    let before = tree(d);
    refused(d, &["policy", "approve", "--approver", "human"], "POLICY_INVALID", None);
    assert_eq!(tree(d), before, "a refused approval writes nothing");
    std::fs::remove_dir(&policy).unwrap();
    if let Some(bytes) = kept {
        std::fs::write(&policy, bytes).unwrap();
    }

    // An artifact tier that is not an object store.
    let before = tree(d);
    refused(d, &["artifacts", "move", "--tier", "hot"], "ARTIFACTS_TIER_INVALID", None);
    assert_eq!(tree(d), before, "a refused move writes nothing");

    // The tiers of a purge and an export: a canon.yaml that does not parse, and one the system refuses.
    let canon_yaml = d.join("canon.yaml");
    let good = std::fs::read_to_string(&canon_yaml).unwrap();
    let export = ["export", "--kind", "subject", "--output", "out.json"];
    let purge = ["purge", "--kind", "session", "--before", "2099-01-01T00:00:00Z"];
    std::fs::write(&canon_yaml, "tiers: [\n").unwrap();
    for args in [&export[..], &purge[..]] {
        refused(d, args, "RETENTION_TIERS_INVALID", None);
    }
    std::fs::write(&canon_yaml, &good).unwrap();
    if let Some(guard) = Mode::set(&canon_yaml, 0o000) {
        for args in [&export[..], &purge[..]] {
            refused(d, args, "RETENTION_TIERS_DENIED", None);
        }
        drop(guard);
    }
    assert!(!d.join("out.json").exists(), "a refused export writes nothing");

    // A scenario index another process holds: INVENTORY_INDEX_UNAVAILABLE, retryable, and the hint says to wait. The wait is cut to 200 ms by the debug seam.
    let held = FileLock::acquire(&d.join(".canon/scenario-index.lock"), Duration::ZERO).unwrap();
    let before = tree(d);
    let out = run_env(d, &["inventory", "sync"], &[("CANON_TEST_INDEX_LOCK_WAIT_MS", Path::new("200"))]);
    let doc = body(&out);
    assert_eq!((out.status.code(), doc["error"]["code"].as_str(), doc["error"]["retryable"].as_bool()), (Some(2), Some("INVENTORY_INDEX_UNAVAILABLE"), Some(true)), "{doc}");
    assert!(doc["error"]["hint"].as_str().unwrap().contains("wait"), "{doc}");
    assert!(doc.get("data").is_none(), "a refusal carries no data: {doc}");
    assert_eq!(tree(d), before, "a refused sync writes nothing");
    drop(held);
    ok(d, &["inventory", "sync"]);

    // A preview of `migrate` that cannot use the temp directory.
    let old = old_repo();
    let scratch = TempDir::new().unwrap();
    let before = tree(old.path());
    if let Some(guard) = Mode::set(scratch.path(), 0o500) {
        let out = run_env(old.path(), &["migrate"], &[("TMPDIR", scratch.path())]);
        let doc = body(&out);
        assert_eq!((out.status.code(), doc["error"]["code"].as_str(), doc["error"]["retryable"].as_bool()), (Some(2), Some("MIGRATION_DENIED"), Some(false)), "{doc}");
        drop(guard);
    }
    assert_eq!(tree(old.path()), before, "a refused preview writes nothing");
}
