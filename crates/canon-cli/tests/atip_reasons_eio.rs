//! `canon-0-36-code-reasons`, the items carried over from the reviews of `canon-0-36-codes-refusals` and `-stored` (scenarios `atip.reasons.10-14`):
//!
//! - a read the system fails for a repeat to pass is the subject's `_UNAVAILABLE` (exit 2, retryable), and this is shown at every call site that
//!   answers one, through the real binary, with the debug-build seam `CANON_TEST_EIO=<path glob>` (`canon_store::eio`): the reads of the paths the
//!   glob matches fail with `EIO`;
//! - a failure of a read before any work began is that code, and `_FAILED` / `_INCOMPLETE` only a failure after work started (`canon query`,
//!   the compaction a `migrate` previews);
//! - every code a command can emit is in `canon schema <command>`;
//! - a Postgres DSN that is set and cannot be reached is `RETENTION_TIERS_UNAVAILABLE`, not `RETENTION_TIERS_REQUIRED`.
//!
//! The seam exists in a debug build only: a test that needs it is skipped by `cargo test --release`.

#![cfg(debug_assertions)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use canon_cli::cli_error::{all_codes, code_info};
use serde_json::Value;
use tempfile::TempDir;

// ── The binary ──

fn run(dir: &Path, glob: Option<&str>, envs: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).arg("--json").current_dir(dir).env_remove("CANON_ROLE").env_remove("CANON_ACTOR").env_remove("CANON_TEST_EIO").env_remove("CANON_PG_DSN");
    if let Some(glob) = glob {
        command.env("CANON_TEST_EIO", glob);
    }
    for (name, value) in envs {
        command.env(name, value);
    }
    command.output().expect("spawn canon")
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn ok(dir: &Path, args: &[&str]) -> Value {
    let out = run(dir, None, &[], args);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    body(&out)
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com"]).args(args).current_dir(dir).status().unwrap().success(), "git {args:?}");
}

/// Whether `canon schema <command> --json` lists `code` among the codes the command can emit.
fn schema_lists(command: &[&str], code: &str) -> bool {
    let mut args = vec!["schema"];
    args.extend_from_slice(command);
    args.push("--json");
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(&args).output().expect("spawn canon");
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    body(&out)["data"]["errors"].as_array().unwrap_or_else(|| panic!("{args:?} has no errors")).iter().any(|listed| listed == code)
}

/// The answer of a call that the system failed in a way a repeat can pass: a refusal (exit 2, no `data`), the code, the `retryable` of its registry
/// row (true), the operating-system error in its message, and a code that `canon schema <command>` lists for the command.
fn unavailable(out: &Output, code: &str, command: &[&str]) -> Value {
    let document = body(out);
    let row = code_info(code).unwrap_or_else(|| panic!("{code} is not registered"));
    assert!(row.retryable && row.exit == 2, "{code}: a read a repeat can pass is exit 2 and retryable");
    assert_eq!(out.status.code(), Some(2), "{code}: {document}");
    assert_eq!(document["ok"], false, "{document}");
    assert_eq!(document["error"]["code"], code, "{document}");
    assert_eq!(document["error"]["retryable"], true, "{document}");
    assert!(document.get("data").is_none(), "{code}: a refusal carries no data: {document}");
    assert!(document["error"]["message"].as_str().is_some_and(|m| m.contains("Input/output error")), "{code}: the message names the system's error: {document}");
    assert!(document["error"]["hint"].as_str().is_some_and(|h| !h.is_empty()), "{code}: the answer says what to do: {document}");
    assert!(schema_lists(command, code), "canon schema {command:?} does not list {code}");
    document
}

// ── The repositories ──

const BRIEF: &str = "# Add login\n\n\
## Intent\n\nAdd login\n\nVisitors lose their cart when they switch devices.\n\n\
## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor submits a wrong password → sees an error → can retry.\n\n\
## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n### F2 Failed submit\n\nFive failures lock the form for thirty seconds.\n\n\
## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie.\n- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==\n\n\
## Questions and assumptions\n\nNone.\n\n\
## Out of scope\n\nSocial sign-in.\n";

/// A repository with a subject, a change whose brief has flow steps, a feature with a scenario, one committed evidence record and a bound
/// artifact, committed.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    fs::write(repo.join(".canon/changes/add-login/brief.md"), BRIEF).unwrap();
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(repo, &["inventory", "sync"]);
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture"]);
    fs::write(repo.join("art.txt"), "hello\n").unwrap();
    ok(repo, &["evidence", "add", "--scenario", "auth.login.01", "--project", "root", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--artifact", "art.txt", "--promote", "--override-reason", "fixture"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "evidence"]);
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

// ── atip.reasons.10: the kind of an error is read before its raw number ──

/// atip.reasons.10 (the unit test is `read_cause::tests::atip_reasons_10_…`): the integration proof is that a call the seam fails says `EIO`
/// on a unix system and is read as the cause a repeat can pass, which `unavailable` checks at every site below.
#[test]
fn atip_reasons_10_the_seam_fails_a_read_with_eio_and_the_cause_is_the_one_a_repeat_can_pass() {
    let dir = repo();
    let d = dir.path();
    let out = run(d, Some("*/canon.yaml"), &[], &["gate", "check"]);
    let document = unavailable(&out, "CONFIG_UNAVAILABLE", &["gate", "check"]);
    assert!(document["error"]["message"].as_str().unwrap().contains("os error 5"), "{document}");
    // The same call with no seam, and with a glob that matches nothing, reads canon.yaml.
    for glob in [None, Some("*/no-such-file")] {
        let out = run(d, glob, &[], &["gate", "check"]);
        assert_ne!(body(&out)["error"]["code"], "CONFIG_UNAVAILABLE");
    }
}

// ── atip.reasons.11: a failure before any work is the `_UNAVAILABLE` code ──

/// atip.reasons.11: a ledger read the system fails is `REPO_UNAVAILABLE` (exit 2, retryable) for `canon query`, with or without `--plugin`, and not
/// `QUERY_FAILED` (exit 1); the same call with no seam answers.
#[test]
fn atip_reasons_11_a_query_whose_ledger_read_fails_for_a_repeat_to_pass_is_repo_unavailable_not_query_failed() {
    let dir = repo();
    let d = dir.path();
    let answered = run(d, None, &[], &["query", "--kind", "evidence"]);
    assert_eq!(answered.status.code(), Some(0), "{}", String::from_utf8_lossy(&answered.stdout));
    let out = run(d, Some("*/.canon/ledger*"), &[], &["query", "--kind", "evidence"]);
    unavailable(&out, "REPO_UNAVAILABLE", &["query"]);
    // The registry has QUERY_FAILED as exit 1 still, for a failure that is not a read a repeat can pass.
    assert_eq!(code_info("QUERY_FAILED").map(|row| row.exit), Some(1));
    // Nothing the call read changed.
    assert_eq!(body(&run(d, None, &[], &["query", "--kind", "evidence"]))["data"]["count"], 1);
}

/// atip.reasons.11: the same for `canon migrate`: a canon.yaml the system fails to read is `CONFIG_UNAVAILABLE` (it was `CONFIG_REQUIRED`, "does
/// not exist"), a file of the plan is `MIGRATION_UNAVAILABLE` and not `MIGRATION_INVALID`; `CONFIG_REQUIRED` stays for a canon.yaml that is not there.
#[test]
fn atip_reasons_11_a_migrate_whose_reads_fail_for_a_repeat_to_pass_is_unavailable_and_a_missing_canon_yaml_is_still_required() {
    let dir = repo();
    let d = dir.path();
    unavailable(&run(d, Some("*/canon.yaml"), &[], &["migrate", "--as", "orchestrator"]), "CONFIG_UNAVAILABLE", &["migrate"]);
    let bare = TempDir::new().unwrap();
    let missing = body(&run(bare.path(), None, &[], &["migrate", "--as", "orchestrator"]));
    assert_eq!(missing["error"]["code"], "CONFIG_REQUIRED", "{missing}");
}

// ── atip.reasons.12: every `_UNAVAILABLE` call site, through the real binary ──

/// atip.reasons.12: the call sites that read a file or a directory, one by one: each answers its subject's `_UNAVAILABLE` for an `EIO`.
#[test]
fn atip_reasons_12_every_call_site_that_reads_answers_its_subjects_unavailable_code_for_an_eio() {
    let dir = repo();
    let d = dir.path();

    // CONFIG: canon.yaml, read by every command that loads the repository's settings.
    unavailable(&run(d, Some("*/canon.yaml"), &[], &["divergence", "list"]), "CONFIG_UNAVAILABLE", &["divergence", "list"]);
    unavailable(&run(d, Some("*/canon.yaml"), &[], &["gate", "check"]), "CONFIG_UNAVAILABLE", &["gate", "check"]);

    // REPO: the ledger.
    unavailable(&run(d, Some("*/.canon/ledger*"), &[], &["query", "--kind", "evidence"]), "REPO_UNAVAILABLE", &["query"]);

    // BRIEF: brief.md.
    for command in [&["brief", "show", "add-login", "--body"][..], &["brief", "check", "add-login"]] {
        unavailable(&run(d, Some("*/.canon/changes/add-login/brief.md"), &[], command), "BRIEF_UNAVAILABLE", &command[..2]);
    }

    // SPEC_ROOT: the spec root a building change's flow steps are matched against (`brief check` of an approved brief reads it).
    let building = TempDir::new().unwrap();
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases/subject-ship-allowed/repo"), building.path());
    let judged = run(building.path(), None, &[], &["brief", "check", "c-checkout"]);
    assert_eq!(body(&judged)["data"]["phase"], "building", "the brief is judged against its flow scenarios: {}", String::from_utf8_lossy(&judged.stdout));
    unavailable(&run(building.path(), Some("*/specs"), &[], &["brief", "check", "c-checkout"]), "SPEC_ROOT_UNAVAILABLE", &["brief", "check"]);

    // ADAPTER_FILE: the response file the caller names.
    fs::write(d.join("resp.json"), "{}").unwrap();
    unavailable(&run(d, Some("*/resp.json"), &[], &["adapter", "validate", "--response", "resp.json"]), "ADAPTER_FILE_UNAVAILABLE", &["adapter", "validate"]);

    // EVIDENCE_BINDING and REVIEW_ARTIFACT: a file the caller binds.
    unavailable(
        &run(d, Some("*/art.txt"), &[], &["evidence", "add", "--scenario", "auth.login.01", "--project", "root", "--class", "test-run", "--ref", "x", "--role", "implementer", "--artifact", "art.txt"]),
        "EVIDENCE_BINDING_UNAVAILABLE",
        &["evidence", "add"],
    );
    unavailable(
        &run(d, Some("*/art.txt"), &[], &["review", "scenario", "auth.login.01", "--project", "root", "--actor", "rv", "--role", "reviewer", "--original-spec-ref", "docs/prd.md", "--artifact", "art.txt"]),
        "REVIEW_ARTIFACT_UNAVAILABLE",
        &["review", "scenario"],
    );

    // HOOKS_FILE: the settings file `gate install-hooks` reads before it writes.
    unavailable(&run(d, Some("*/.claude/settings.json"), &[], &["gate", "install-hooks"]), "HOOKS_FILE_UNAVAILABLE", &["gate", "install-hooks"]);

    // POLICY: the policy file `policy approve` hashes.
    unavailable(&run(d, Some("*/.canon/policy.yaml"), &[], &["policy", "approve", "--approver", "alice"]), "POLICY_UNAVAILABLE", &["policy", "approve"]);

    // RETENTION_TIERS: the canon.yaml a purge or an export reads its tiers from.
    unavailable(&run(d, Some("*/canon.yaml"), &[], &["export", "--kind", "session", "--output", "out.json"]), "RETENTION_TIERS_UNAVAILABLE", &["export"]);
    unavailable(&run(d, Some("*/canon.yaml"), &[], &["purge", "--kind", "session", "--before", "2020-01-01T00:00:00Z"]), "RETENTION_TIERS_UNAVAILABLE", &["purge"]);
}

/// atip.reasons.12: `POLICY_APPROVAL_UNAVAILABLE`: the approval of the policy, read by the transition that ships (the gate fails closed on it).
#[test]
fn atip_reasons_12_a_policy_approval_the_system_fails_to_read_refuses_the_ship_as_unavailable() {
    let dir = repo();
    let d = dir.path();
    for to in ["specced", "building", "verifying"] {
        ok(d, &["subject", "move", "demo", "--to", to]);
    }
    let out = run(d, Some("*/.canon/policy.approval.json"), &[], &["subject", "move", "demo", "--to", "shipped"]);
    unavailable(&out, "POLICY_APPROVAL_UNAVAILABLE", &["subject", "move"]);
    let moved = body(&run(d, None, &[], &["subject", "show", "demo"]));
    assert_eq!(moved["data"]["status"], "verifying", "the refused ship moved nothing: {moved}");
}

/// atip.reasons.12: the artifact store: the directory is read before a blob moves (`ARTIFACTS_STORE_UNAVAILABLE`), and so is each blob.
#[test]
fn atip_reasons_12_the_local_artifact_store_the_system_fails_to_read_is_artifacts_store_unavailable() {
    let dir = repo();
    let d = dir.path();
    let yaml = fs::read_to_string(d.join("canon.yaml")).unwrap();
    let with_cold = yaml.replace("  # cold:\n  #   backend: s3\n  #   bucket_env: CANON_R2_BUCKET\n  #   prefix: \"canon/\"\n", "  cold:\n    backend: s3\n    bucket_env: CANON_R2_BUCKET\n    prefix: \"canon/\"\n");
    assert_ne!(yaml, with_cold, "init's canon.yaml carries the commented cold tier");
    fs::write(d.join("canon.yaml"), with_cold).unwrap();
    let store = [("CANON_R2_BUCKET", "b"), ("CANON_S3_ENDPOINT", "http://127.0.0.1:9"), ("CANON_S3_ACCESS_KEY", "a"), ("CANON_S3_SECRET_KEY", "b")];
    for glob in ["*/.canon/artifacts/sha256", "*/.canon/artifacts/sha256/*"] {
        let out = run(d, Some(glob), &store, &["artifacts", "move", "--tier", "cold"]);
        let document = unavailable(&out, "ARTIFACTS_STORE_UNAVAILABLE", &["artifacts", "move"]);
        assert!(document["error"].get("reason").is_none(), "ARTIFACTS_STORE_UNAVAILABLE has one situation and no reason: {document}");
    }
}

/// atip.reasons.12: the dispatch manifest of a run (`DISPATCH_MANIFEST_UNAVAILABLE`), the plan corpus `--task` is resolved against
/// (`PLAN_CORPUS_UNAVAILABLE`), and a run whose end another process holds (`DISPATCH_RUN_UNAVAILABLE`, a lock and not a read).
#[test]
fn atip_reasons_12_the_dispatch_manifest_the_plan_corpus_and_the_run_lock_are_unavailable_for_a_repeat() {
    let dir = repo();
    let d = dir.path();
    let regime = "dev/canon/join-spine/9c93d024b1a2";
    let begun = ok(d, &["dispatch", "begin", "--regime", regime, "--role", "dev"]);
    let run_id = begun["data"]["runId"].as_str().unwrap().to_string();

    unavailable(&run(d, Some("*/.canon/dispatch/*"), &[], &["dispatch", "end", &run_id, "--run-outcome", "succeeded"]), "DISPATCH_MANIFEST_UNAVAILABLE", &["dispatch", "end"]);

    // The plan corpus: canon.yaml's `plans:` section is read for every `--task`.
    for command in [
        &["dispatch", "begin", "--regime", regime, "--role", "dev", "--task", "add-login#1"][..],
        &["evidence", "add", "--task", "add-login#1", "--class", "test-run", "--ref", "x", "--role", "implementer"],
        &["evidence", "approval-payload", "--task", "add-login#1", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z"],
        &["task", "done", "add-login#1"],
        &["ingest", "plans"],
    ] {
        let leaf: Vec<&str> = command.iter().take(2).copied().collect();
        unavailable(&run(d, Some("*/canon.yaml"), &[], command), "PLAN_CORPUS_UNAVAILABLE", &leaf);
    }

    // A run another process is closing: the lock sidecar exists; nothing is read, and the answer is retryable all the same.
    let manifest = d.join(".canon/dispatch").join(format!("{run_id}.json"));
    fs::write(manifest.with_extension("json.lock"), "").unwrap();
    let out = run(d, None, &[], &["dispatch", "end", &run_id, "--run-outcome", "succeeded"]);
    let document = body(&out);
    assert_eq!((out.status.code(), document["error"]["code"].as_str(), document["error"]["retryable"].as_bool()), (Some(2), Some("DISPATCH_RUN_UNAVAILABLE"), Some(true)), "{document}");
    assert!(schema_lists(&["dispatch", "end"], "DISPATCH_RUN_UNAVAILABLE"));
}

/// atip.reasons.12: every reader of the plan corpus answers the same for a canon.yaml it cannot read, by the cause: `PLAN_CORPUS_UNAVAILABLE` for an
/// `EIO`, `CONFIG_DENIED` for a read the system refuses, `CONFIG_INVALID` (reason `CANON_YAML`) for a directory or text that is not UTF-8, and never
/// "no plan sources" (`PLAN_SOURCES_REQUIRED`), `TASK_NOT_FOUND` or `INGEST_FAILED`; a canon.yaml that is not there is still the first-run state.
#[test]
fn atip_reasons_12_every_reader_of_the_plan_corpus_answers_by_the_cause_of_an_unreadable_canon_yaml() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = repo();
    let d = dir.path();
    let regime = "dev/canon/join-spine/9c93d024b1a2";
    let readers: [(&[&str], &[&str]); 5] = [
        (&["dispatch", "begin", "--regime", regime, "--role", "dev", "--task", "add-login#1"], &["dispatch", "begin"]),
        (&["evidence", "add", "--task", "add-login#1", "--class", "test-run", "--ref", "x", "--role", "implementer"], &["evidence", "add"]),
        (&["evidence", "approval-payload", "--task", "add-login#1", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z"], &["evidence", "approval-payload"]),
        (&["task", "done", "add-login#1"], &["task", "done"]),
        (&["ingest", "plans"], &["ingest", "plans"]),
    ];
    let canon_yaml = d.join("canon.yaml");
    let original = fs::read(&canon_yaml).unwrap();
    let refusal = |out: &Output, code: &str, reason: Option<&str>, leaf: &[&str]| {
        let document = body(out);
        let row = code_info(code).unwrap();
        assert_eq!((out.status.code(), document["error"]["code"].as_str(), document["error"].get("reason").and_then(Value::as_str)), (Some(2), Some(code), reason), "{leaf:?}: {document}");
        assert_eq!(document["error"]["retryable"].as_bool(), Some(row.retryable), "{leaf:?}: {document}");
        assert!(document.get("data").is_none(), "a refusal carries no data: {document}");
        assert!(schema_lists(leaf, code), "canon schema {leaf:?} does not list {code}");
    };
    for (command, leaf) in readers {
        // Not UTF-8 text, and a directory where the file belongs: the file is no canon.yaml, whatever the command.
        fs::write(&canon_yaml, b"\xff\xfe not utf-8").unwrap();
        refusal(&run(d, None, &[], command), "CONFIG_INVALID", Some("CANON_YAML"), leaf);
        fs::remove_file(&canon_yaml).unwrap();
        fs::create_dir(&canon_yaml).unwrap();
        refusal(&run(d, None, &[], command), "CONFIG_INVALID", Some("CANON_YAML"), leaf);
        fs::remove_dir(&canon_yaml).unwrap();
        fs::write(&canon_yaml, &original).unwrap();
        // A read the system refuses (`evidence add` and `evidence approval-payload` take the code tree of the repository first, which git cannot take
        // with an unreadable file: `COMMIT_REQUIRED`, before the plan corpus is read).
        if leaf[0] != "evidence" {
            fs::set_permissions(&canon_yaml, fs::Permissions::from_mode(0o000)).unwrap();
            let refused = run(d, None, &[], command);
            fs::set_permissions(&canon_yaml, fs::Permissions::from_mode(0o644)).unwrap();
            if fs::read(&canon_yaml).is_ok() && fs::File::open(&canon_yaml).is_ok() && refused.status.code() == Some(2) {
                // The test runs as a user the mode binds (root is not bound by it, and reads the file).
                refusal(&refused, "CONFIG_DENIED", None, leaf);
            }
        }
        // An EIO: the same refusal for all five, retryable.
        unavailable(&run(d, Some("*/canon.yaml"), &[], command), "PLAN_CORPUS_UNAVAILABLE", leaf);
    }
    // `task done` for a task the corpus does not hold is still `TASK_NOT_FOUND`, and `ingest plans` with canon.yaml readable is still the import.
    let absent = body(&run(d, None, &[], &["task", "done", "add-login#9"]));
    assert_eq!(absent["error"]["code"], "TASK_NOT_FOUND", "{absent}");
}

/// atip.reasons.12: `dispatch diff` reads the records of the plan tasks and the runs and degrades: a read the system fails for a repeat to pass is the
/// warning `DISPATCH_DIFF_RECORDS_UNAVAILABLE`, naming the kind, and the diff still answers (exit 0).
#[test]
fn atip_reasons_12_dispatch_diff_names_a_records_read_the_system_failed_as_its_unavailable_warning() {
    let dir = repo();
    let d = dir.path();
    let yaml = fs::read_to_string(d.join("canon.yaml")).unwrap().replace("  task: hot", "  task: local").replace("  run: hot", "  run: local");
    fs::write(d.join("canon.yaml"), yaml).unwrap();
    fs::create_dir_all(d.join(".canon/ledger/kind=task")).unwrap();
    fs::write(d.join(".canon/ledger/kind=task/x.json"), "{}").unwrap();
    let out = run(d, Some("*/.canon/ledger/kind=task/*"), &[], &["dispatch", "diff"]);
    let document = body(&out);
    assert_eq!(out.status.code(), Some(0), "{document}");
    let warnings = document["meta"]["warnings"].as_array().unwrap_or_else(|| panic!("{document}"));
    let warning = warnings.iter().find(|w| w["code"] == "DISPATCH_DIFF_RECORDS_UNAVAILABLE").unwrap_or_else(|| panic!("{document}"));
    assert!(warning["message"].as_str().unwrap().contains("`task` records unavailable"), "{warning}");
    assert!(warning["message"].as_str().unwrap().contains("Input/output error"), "{warning}");
}

/// atip.reasons.12: `MIGRATION_UNAVAILABLE`: the files the plan reads (a record in the old layout) and the files the preview copies into its scratch
/// directory (every file of the ledger, so a file the plan does not read is read only there).
#[test]
fn atip_reasons_12_the_files_a_migrate_plan_and_its_preview_read_are_migration_unavailable_for_an_eio() {
    let dir = repo();
    let d = dir.path();
    // A record in the 0.29 layout: `kind=evidence_record/<key>__<12 hex>.json`, its body's kind `evidence_record`.
    let ledger = d.join(".canon/ledger/kind=evidence");
    let record = fs::read_dir(&ledger).unwrap().flatten().next().unwrap().path();
    let legacy = d.join(".canon/ledger/kind=evidence_record");
    fs::create_dir_all(&legacy).unwrap();
    let text = fs::read_to_string(&record).unwrap().replace("\"kind\": \"evidence\"", "\"kind\": \"evidence_record\"");
    fs::write(legacy.join("root_scenario_auth.login.01__93c486cc1234.json"), text).unwrap();
    fs::remove_file(&record).unwrap();
    git(d, &["add", "-A"]);
    git(d, &["commit", "-q", "-m", "legacy"]);

    // The plan reads the record to rename it.
    unavailable(&run(d, Some("*/kind=evidence_record/*"), &[], &["migrate", "--as", "orchestrator"]), "MIGRATION_UNAVAILABLE", &["migrate"]);
    // The preview copies every file of the ledger; a file the plan does not read is read there only.
    fs::write(d.join(".canon/ledger/notes.txt"), "kept\n").unwrap();
    unavailable(&run(d, Some("*/.canon/ledger/notes.txt"), &[], &["migrate", "--as", "orchestrator"]), "MIGRATION_UNAVAILABLE", &["migrate"]);
    // With no seam the plan is made (a preview, nothing written).
    let planned = run(d, None, &[], &["migrate", "--as", "orchestrator"]);
    assert_eq!(planned.status.code(), Some(0), "{}", String::from_utf8_lossy(&planned.stdout));
}

/// atip.reasons.12: `STRATEGY_SIGNATURE_UNAVAILABLE`: the detached signature of a promotion and of a rollback.
#[test]
fn atip_reasons_12_a_detached_signature_the_system_fails_to_read_is_strategy_signature_unavailable() {
    use canon_learn::{ParquetStrategyStore, PromotionApproval, PromotionEvaluation, PromotionEvaluationFields, StrategyId, StrategyItem, StrategyItemFields, StrategyStore, TrajectoryId};
    use canon_model::ids::{regime_key, RegimeKey, RoleId};
    let dir = repo();
    let d = dir.path();
    let regime = RegimeKey::parse(regime_key("dev", "canon", "join-spine", "9c93d024b1a2")).unwrap();
    let store = ParquetStrategyStore::open(d.join(".canon").join("learn").join("strategies"));
    let candidate = StrategyItem::new(StrategyItemFields {
        id: StrategyId::new(),
        regime_key: regime,
        role: RoleId::parse("dev").unwrap(),
        title: "review guidance".into(),
        description: "one-liner".into(),
        content: "sign me".into(),
        source_trajectory_ids: vec![TrajectoryId::new()],
        recorded_at: chrono::Utc::now(),
    });
    store.append(&candidate).expect("seed strategy");
    let id = candidate.id.to_string();
    let evaluation = PromotionEvaluation::new(
        &candidate,
        PromotionEvaluationFields {
            candidate_version: "candidate-v2".into(),
            baseline_result_digests: vec!["baseline-panel-0".to_string()],
            candidate_result_digests: vec!["candidate-panel-0".to_string()],
            corpus_version: "corpus-v1".into(),
            eval_version: "paired-eval-v1".into(),
            context_digest: "context-digest-v1".into(),
            policy_digest: "policy-digest-v1".into(),
            model_digest: "model-digest-v1".into(),
            tool_digest: "tool-digest-v1".into(),
            paired_metrics: [("pairs".to_string(), 1.0)].into_iter().collect(),
            passed: true,
            decision: "promote".into(),
        },
    );
    let approval = PromotionApproval::new(candidate.id, evaluation.digest(), "alice", "human", true, chrono::Utc::now());
    fs::write(d.join("evaluation.json"), serde_json::to_vec_pretty(&evaluation).unwrap()).unwrap();
    fs::write(d.join("approval.json"), serde_json::to_vec_pretty(&approval).unwrap()).unwrap();
    fs::write(d.join("approval.sig"), "-----BEGIN SSH SIGNATURE-----\n").unwrap();

    unavailable(
        &run(d, Some("*/approval.sig"), &[], &["strategy", "promote", &id, "--evaluation", "evaluation.json", "--approval", "approval.json", "--signature-file", "approval.sig"]),
        "STRATEGY_SIGNATURE_UNAVAILABLE",
        &["strategy", "promote"],
    );
    unavailable(
        &run(d, Some("*/approval.sig"), &[], &["strategy", "rollback", &id, "--reason", "r", "--actor", "a", "--signature-file", "approval.sig", "--approval-at", "2030-01-01T00:00:00Z"]),
        "STRATEGY_SIGNATURE_UNAVAILABLE",
        &["strategy", "rollback"],
    );
}

/// atip.reasons.12: every `_UNAVAILABLE` code of the registry is driven above, or is no read: a lock, a port or a warning of a snapshot. A code
/// of the registry that is neither is a call site with no proof.
#[test]
fn atip_reasons_12_every_unavailable_code_is_driven_by_a_test_here_or_is_no_read() {
    const DRIVEN: &[&str] = &[
        "ADAPTER_FILE_UNAVAILABLE", "ARTIFACTS_STORE_UNAVAILABLE", "BRIEF_UNAVAILABLE", "CONFIG_UNAVAILABLE", "DISPATCH_DIFF_RECORDS_UNAVAILABLE", "DISPATCH_MANIFEST_UNAVAILABLE", "DISPATCH_RUN_UNAVAILABLE",
        "EVIDENCE_BINDING_UNAVAILABLE", "HOOKS_FILE_UNAVAILABLE", "MIGRATION_UNAVAILABLE", "PLAN_CORPUS_UNAVAILABLE", "POLICY_APPROVAL_UNAVAILABLE", "POLICY_UNAVAILABLE", "REPO_UNAVAILABLE",
        "RETENTION_TIERS_UNAVAILABLE", "REVIEW_ARTIFACT_UNAVAILABLE", "SPEC_ROOT_UNAVAILABLE", "STRATEGY_SIGNATURE_UNAVAILABLE", "TIER_UNAVAILABLE",
    ];
    // Not an operating-system read: another process holds a lock (`REPO_LOCK_UNAVAILABLE`, `INVENTORY_INDEX_UNAVAILABLE`), the port is taken
    // (`DASHBOARD_PORT_UNAVAILABLE`), or the dashboard's snapshot warning (`DASHBOARD_SNAPSHOT_UNAVAILABLE`), each proved by the tests of its command.
    const NOT_A_READ: &[&str] = &["DASHBOARD_PORT_UNAVAILABLE", "DASHBOARD_SNAPSHOT_UNAVAILABLE", "INVENTORY_INDEX_UNAVAILABLE", "REPO_LOCK_UNAVAILABLE"];
    let registry: Vec<&str> = all_codes().iter().map(|info| info.code).chain(canon_cli::cli_error::all_warnings().iter().map(|info| info.code)).filter(|code| code.ends_with("_UNAVAILABLE")).collect();
    let unproved: Vec<&&str> = registry.iter().filter(|code| !DRIVEN.contains(code) && !NOT_A_READ.contains(code)).collect();
    assert!(unproved.is_empty(), "`_UNAVAILABLE` codes with no test that drives them: {unproved:?}");
    for code in DRIVEN.iter().chain(NOT_A_READ) {
        assert!(registry.contains(code), "{code} is listed here and is no longer registered");
    }
}

// ── atip.reasons.13: every code a command can emit is listed ──

/// atip.reasons.13: `canon schema evidence approval-payload` and `canon schema evidence add` list `TASK_NOT_FOUND` and `PLAN_SOURCES_REQUIRED` (and the
/// three `PLAN_CORPUS_*` codes), the codes `--task` answers through `DispatchError::to_cli_error`, and each command answers them.
#[test]
fn atip_reasons_13_every_code_the_plan_corpus_admission_of_task_answers_is_listed_for_both_commands() {
    let dir = repo();
    let d = dir.path();
    let approval = ["evidence", "approval-payload", "--task", "add-login#9", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z"];
    let add = ["evidence", "add", "--task", "add-login#9", "--class", "test-run", "--ref", "x", "--role", "implementer"];
    for (command, leaf) in [(&approval[..], &["evidence", "approval-payload"][..]), (&add[..], &["evidence", "add"][..])] {
        let out = run(d, None, &[], command);
        let document = body(&out);
        assert_eq!((out.status.code(), document["error"]["code"].as_str()), (Some(2), Some("TASK_NOT_FOUND")), "{leaf:?}: {document}");
        for code in ["TASK_NOT_FOUND", "PLAN_SOURCES_REQUIRED", "PLAN_CORPUS_DENIED", "PLAN_CORPUS_INVALID", "PLAN_CORPUS_UNAVAILABLE"] {
            assert!(schema_lists(leaf, code), "canon schema {leaf:?} does not list {code}");
        }
    }
    // A repository with no plan source at all: `PLAN_SOURCES_REQUIRED`.
    fs::remove_dir_all(d.join(".canon/changes")).unwrap();
    for (command, leaf) in [(&approval[..], &["evidence", "approval-payload"][..]), (&add[..], &["evidence", "add"][..])] {
        let out = run(d, None, &[], command);
        let document = body(&out);
        assert_eq!((out.status.code(), document["error"]["code"].as_str()), (Some(2), Some("PLAN_SOURCES_REQUIRED")), "{leaf:?}: {document}");
    }
}

// ── atip.reasons.14: a Postgres DSN that is set and cannot be reached ──

/// atip.reasons.14: a tier whose DSN is set and cannot be reached is `RETENTION_TIERS_UNAVAILABLE` (retryable); one whose DSN is unset is
/// `RETENTION_TIERS_REQUIRED` (not): the settings are not there, and waiting does not set them.
#[test]
fn atip_reasons_14_a_postgres_dsn_that_is_set_and_unreachable_is_unavailable_and_an_unset_one_is_required() {
    let dir = repo();
    let d = dir.path();
    let yaml = fs::read_to_string(d.join("canon.yaml")).unwrap();
    let postgres = yaml.replace("  hot:\n    backend: sqlite\n    path: .canon/hot.db\n", "  hot:\n    backend: postgres\n    dsn_env: CANON_PG_DSN\n    schema: canon_v1\n");
    assert_ne!(yaml, postgres, "init's canon.yaml has the sqlite hot tier");
    fs::write(d.join("canon.yaml"), postgres).unwrap();
    git(d, &["add", "-A"]);
    git(d, &["commit", "-q", "-m", "postgres"]);

    let unset = run(d, None, &[], &["export", "--kind", "session", "--output", "out.json"]);
    let document = body(&unset);
    assert_eq!((unset.status.code(), document["error"]["code"].as_str(), document["error"]["retryable"].as_bool()), (Some(2), Some("RETENTION_TIERS_REQUIRED"), Some(false)), "{document}");

    // A host name that cannot resolve (`.invalid` is reserved for it): refused at once, with no wait for a pool to time out.
    let dsn = [("CANON_PG_DSN", "postgres://canon:canon@no-such-host.invalid:5432/canon")];
    let unreachable = run(d, None, &dsn, &["export", "--kind", "session", "--output", "out.json"]);
    let document = body(&unreachable);
    assert_eq!((unreachable.status.code(), document["error"]["code"].as_str(), document["error"]["retryable"].as_bool()), (Some(2), Some("RETENTION_TIERS_UNAVAILABLE"), Some(true)), "{document}");
    assert!(document.get("data").is_none(), "a refusal carries no data: {document}");
    assert!(!d.join("out.json").exists(), "nothing was written");
    assert!(schema_lists(&["export"], "RETENTION_TIERS_UNAVAILABLE") && schema_lists(&["export"], "RETENTION_TIERS_REQUIRED"));

    // `query` and `tier age` read through the same tiers: a tier that did not answer is `*_UNAVAILABLE` for them too, and its message says the tier "did not
    // answer" (the DSN is set), not that it is "not attached (no live DSN)".
    let query = run(d, None, &dsn, &["query", "--kind", "session"]);
    let document = unavailable_tier(&query, "REPO_UNAVAILABLE", &["query"]);
    assert!(!document["error"]["message"].as_str().unwrap().contains("no live DSN"), "{document}");
    let age = run(d, None, &dsn, &["tier", "age"]);
    unavailable_tier(&age, "TIER_UNAVAILABLE", &["tier", "age"]);
    // With the DSN unset they say "not attached": no refusal a repeat can pass.
    let unset_query = body(&run(d, None, &[], &["query", "--kind", "session"]));
    assert_eq!(unset_query["error"]["code"], "QUERY_FAILED", "{unset_query}");
    assert!(unset_query["error"]["message"].as_str().unwrap().contains("is not attached"), "{unset_query}");

    // A DSN that is set and empty or malformed is a value the caller wrote, which no wait fixes: `RETENTION_TIERS_INVALID` for purge and export (not
    // retryable), `CONFIG_INVALID` (reason `TIERS`) for `query` and `tier age`. Only a well-formed DSN is tried at all.
    for bad in ["", "not a dsn", "postgres://no-port-and-a-space here/x"] {
        let dsn = [("CANON_PG_DSN", bad)];
        for command in [&["export", "--kind", "session", "--output", "out.json"][..], &["purge", "--kind", "session", "--before", "2020-01-01T00:00:00Z"]] {
            let out = run(d, None, &dsn, command);
            let document = body(&out);
            assert_eq!((out.status.code(), document["error"]["code"].as_str(), document["error"]["retryable"].as_bool()), (Some(2), Some("RETENTION_TIERS_INVALID"), Some(false)), "{bad:?} {command:?}: {document}");
            assert!(document["error"]["message"].as_str().unwrap().contains("DSN"), "{document}");
            assert!(document.get("data").is_none() && !d.join("out.json").exists(), "a refusal changes nothing: {document}");
        }
        for command in [&["query", "--kind", "session"][..], &["tier", "age"]] {
            let out = run(d, None, &dsn, command);
            let document = body(&out);
            assert_eq!((out.status.code(), document["error"]["code"].as_str(), document["error"]["reason"].as_str(), document["error"]["retryable"].as_bool()), (Some(2), Some("CONFIG_INVALID"), Some("TIERS"), Some(false)), "{bad:?} {command:?}: {document}");
        }
    }
    assert!(schema_lists(&["export"], "RETENTION_TIERS_INVALID") && schema_lists(&["purge"], "RETENTION_TIERS_INVALID"));
}

/// [`unavailable`] for a tier that did not answer: the message is the tier's own, not the system's `EIO`.
fn unavailable_tier(out: &Output, code: &str, command: &[&str]) -> Value {
    let document = body(out);
    let row = code_info(code).unwrap_or_else(|| panic!("{code} is not registered"));
    assert!(row.retryable && row.exit == 2, "{code}");
    assert_eq!((out.status.code(), document["error"]["code"].as_str(), document["error"]["retryable"].as_bool()), (Some(2), Some(code), Some(true)), "{document}");
    assert!(document.get("data").is_none(), "a refusal carries no data: {document}");
    assert!(document["error"]["message"].as_str().is_some_and(|m| m.contains("did not answer")), "{document}");
    assert!(schema_lists(command, code), "canon schema {command:?} does not list {code}");
    document
}
