//! `canon-0-36-codes-exit`, scenarios `atip.codeexit.01`-`.12` (`.12` is in `grammar_t5_errors.rs`): a refusal that starts nothing answers exit 2 with
//! its code and no `data`; several causes are `error.details[]`; the check commands' negative answers are exit 1 `*_FAILED` with their result
//! in `data`; the commands that cannot start answer exit 2. Against the built `canon` binary.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn canon(dir: &Path, args: &[&str], role: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).arg("--json").current_dir(dir).env_remove("CANON_ROLE").env_remove("CANON_ACTOR");
    if let Some(role) = role {
        command.env("CANON_ROLE", role);
    }
    command.output().expect("spawn canon")
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

fn ok(dir: &Path, args: &[&str], role: Option<&str>) -> Value {
    let out = canon(dir, args, role);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    body(&out)
}

fn git_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(dir.path()).output().unwrap();
        assert!(out.status.success(), "git {args:?}");
    }
    dir
}

fn brief_path(repo: &Path) -> std::path::PathBuf {
    repo.join(".canon/changes/add-login/brief.md")
}

/// A complete English PRD brief with flow steps `F1`, `F2`; `extra` is
/// appended (waivers, an approval line).
fn en_brief(extra: &str) -> String {
    format!(
        "# Add login\n\n\
## Intent\n\nAdd login\n\nVisitors lose their cart when they switch devices.\n\n\
## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor submits a wrong password → sees an error → can retry.\n\n\
## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n### F2 Failed submit\n\nFive failures lock the form for thirty seconds.\n\n\
## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie.\n- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==\n\n\
## Questions and assumptions\n\nNone.\n\n\
## Out of scope\n\nSocial sign-in.\n\n{extra}"
    )
}

/// A repo with subject `demo`, change `add-login` with a clean brief (not yet approved) and one task.
fn repo_with_clean_brief() -> TempDir {
    let dir = git_repo();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"], None);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"], Some("planner"));
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"], Some("planner"));
    std::fs::write(brief_path(repo), en_brief("")).unwrap();
    let tasks = repo.join(".canon/changes/add-login/tasks.md");
    let mut text = std::fs::read_to_string(&tasks).unwrap_or_else(|_| "# Tasks: Add login\n".to_string());
    text.push_str("\n- [ ] 1 Write the failure-path scenarios\n");
    std::fs::write(&tasks, text).unwrap();
    ok(repo, &["ingest", "plans"], None);
    dir
}

const ATTEST: [&str; 10] = ["evidence", "add", "--task", "add-login#1", "--class", "test-run", "--ref", "npm test", "--role", "implementer"];

/// Every file of `dir` (the work tree and `.canon/`, not `.git`) with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().display().to_string(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// Runs `args` and checks a refusal: exit 2, `error.code`, no `data`, nothing retried; returns the document.
fn refused(dir: &Path, role: Option<&str>, args: &[&str], code: &str, retryable: bool) -> Value {
    let before = snapshot(dir);
    let out = canon(dir, args, role);
    let b = body(&out);
    assert_eq!(out.status.code(), Some(2), "{args:?}: {b}");
    assert_eq!((b["ok"].clone(), b["error"]["code"].as_str(), b["error"]["retryable"].as_bool()), (Value::Bool(false), Some(code), Some(retryable)), "{args:?}: {b}");
    assert!(b.get("data").is_none(), "a refusal carries no data: {b}");
    assert_eq!(snapshot(dir), before, "{args:?}: the refusal wrote nothing");
    b
}

/// atip.codeexit.02: an approved brief takes the same call; a brief edited after its approval is `BRIEF_APPROVAL_STALE`, and a re-approval opens it.
#[test]
fn atip_codeexit_02_an_approved_brief_takes_the_call_and_an_edit_after_the_approval_is_stale() {
    let dir = repo_with_clean_brief();
    let repo = dir.path();
    ok(repo, &["brief", "approve", "--approver", "Ana", "--note", "Go."], None);
    let mut attest = ATTEST.to_vec();
    attest.extend(["--actor", "impl"]);
    ok(repo, &attest, Some("implementer"));
    let text = std::fs::read_to_string(brief_path(repo)).unwrap();
    std::fs::write(brief_path(repo), text.replace("Social sign-in.", "Social sign-in, later.")).unwrap();
    refused(repo, Some("implementer"), &attest, "BRIEF_APPROVAL_STALE", false);
}

/// atip.codeexit.03: a `gate promote` that meets two refused records is exit 2, the first record's code, two `details` that each carry their
/// own code, and no `data`; the records stay staged.
#[test]
fn atip_codeexit_03_gate_promote_with_two_refused_records_is_exit_2_with_two_details_and_no_data() {
    let dir = git_repo();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"], None);
    for seq in ["1", "2"] {
        ok(repo, &["finding", "add", "--change", "c1", "--round", "1", "--seq", seq, "--severity", "note", "--actor", "a1", "--summary", "first", "--promote"], Some("reviewer"));
    }
    let ledger = repo.join(".canon/ledger/kind=finding");
    let mut staged = Vec::new();
    for entry in std::fs::read_dir(&ledger).unwrap() {
        let path = entry.unwrap().path();
        let mut record: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        record["summary"] = Value::String(format!("{} changed", record["summary"].as_str().unwrap()));
        let name = path.file_name().unwrap().to_str().unwrap().rsplit_once('_').unwrap().0.to_string();
        let target = repo.join(".canon/ledger/_staging/kind=finding").join(format!("{name}_ffffffff.json"));
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
        staged.push(target);
    }
    assert_eq!(staged.len(), 2);
    let before = snapshot(repo);
    let out = canon(repo, &["gate", "promote"], None);
    let b = body(&out);
    assert_eq!(out.status.code(), Some(2), "{b}");
    assert!(b.get("data").is_none(), "{b}");
    let details = b["error"]["details"].as_array().unwrap_or_else(|| panic!("two causes are two details: {b}"));
    assert_eq!(details.len(), 2, "{b}");
    assert!(details.iter().all(|d| d["code"] == "PROMOTION_INVALID"), "{b}");
    assert_eq!(b["error"]["code"], details[0]["code"], "the error is the first cause: {b}");
    assert_eq!(snapshot(repo), before, "the records stay staged and nothing was promoted");
}

/// atip.codeexit.04: a `subject move` the gate blocks changes no status; one cause is the error itself, with no `details`.
#[test]
fn atip_codeexit_04_a_subject_move_the_gate_blocks_is_exit_2_with_the_cause_as_the_error() {
    let dir = git_repo();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"], None);
    ok(repo, &["subject", "new", "demo", "--domain", "d", "--title", "T"], Some("planner"));
    for to in ["specced", "building", "verifying"] {
        ok(repo, &["subject", "move", "demo", "--to", to], Some("planner"));
    }
    let b = refused(repo, Some("planner"), &["subject", "move", "demo", "--to", "shipped"], "SUBJECT_APPROVAL_REQUIRED", false);
    assert!(b["error"].get("details").is_none(), "one cause is the error itself: {b}");
}

fn adapter_fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/adapter/claude.json")
}

/// Runs `args` and checks a check command's negative answer: exit 1, `error.code`, the result in `data`.
fn failed(dir: &Path, args: &[&str], code: &str) -> Value {
    let out = canon(dir, args, None);
    let b = body(&out);
    assert_eq!(out.status.code(), Some(1), "{args:?}: {b}");
    assert_eq!((b["ok"].clone(), b["error"]["code"].as_str(), b["error"]["retryable"].as_bool()), (Value::Bool(false), Some(code), Some(false)), "{args:?}: {b}");
    assert!(b["data"].is_object(), "the result stays in data: {b}");
    b
}

/// atip.codeexit.06: the negative answer of a check is exit 1, a `*_FAILED` code and the result in `data`.
#[test]
fn atip_codeexit_06_a_check_command_that_answers_no_is_exit_1_with_its_failed_code_and_the_result_in_data() {
    let dir = git_repo();
    let d = dir.path();
    ok(d, &["init", "--providers", "none"], None);
    std::fs::write(d.join("bad.json"), r#"{"nope":1}"#).unwrap();
    let b = failed(d, &["adapter", "validate", "--response", "bad.json"], "ADAPTER_VALIDATION_FAILED");
    assert_eq!(b["data"]["valid"], false, "{b}");
    let b = failed(d, &["adapter", "authorize", "--response", adapter_fixture().to_str().unwrap()], "ADAPTER_AUTHORIZATION_FAILED");
    assert_eq!(b["data"]["executionAuthorized"], false, "{b}");
    let b = failed(d, &["report", "check"], "REPORT_CHECK_FAILED");
    assert_eq!((b["error"]["reason"].as_str(), b["data"]["state"].as_str()), (Some("STALE"), Some("missing")), "{b}");

    // A pack changed after it was created: `verify` answers no; the commands that read the pack refuse it.
    std::fs::write(d.join("doc.md"), "hi\n").unwrap();
    std::fs::write(d.join("m.json"), r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#).unwrap();
    for args in [&["add", "-A"][..], &["commit", "-q", "-m", "m"]] {
        assert!(Command::new("git").args(args).current_dir(d).status().unwrap().success());
    }
    let id = ok(d, &["context-pack", "add", "--manifest", "m.json"], None)["data"]["id"].as_str().unwrap().to_string();
    let object = std::fs::read_dir(d.join(".canon/context-packs/objects")).unwrap().next().unwrap().unwrap().path();
    std::fs::write(&object, [std::fs::read(&object).unwrap(), b"X".to_vec()].concat()).unwrap();
    let b = failed(d, &["context-pack", "verify", &id], "CONTEXT_PACK_VERIFY_FAILED");
    assert_eq!((b["data"]["id"].as_str(), b["data"]["verified"].as_bool()), (Some(id.as_str()), Some(false)), "{b}");
    refused(d, None, &["context-pack", "show", &id], "CONTEXT_PACK_CORRUPT", false);
}

/// atip.codeexit.07: the same commands that cannot start (no input file, no canon.yaml) are exit 2 and not `FAILED`.
#[test]
fn atip_codeexit_07_a_check_command_that_cannot_start_is_exit_2_and_not_failed() {
    let dir = git_repo();
    let d = dir.path();
    ok(d, &["init", "--providers", "none"], None);
    let b = refused(d, None, &["adapter", "validate", "--response", "no-such-response.json"], "ADAPTER_FILE_NOT_FOUND", false);
    assert!(!b["error"]["code"].as_str().unwrap().ends_with("_FAILED"), "{b}");
    let out = canon(d, &["report", "check", "--repo", "/no/such/repo"], None);
    assert_eq!(out.status.code(), Some(2), "{}", body(&out));
    assert_eq!(body(&out)["error"]["code"], "CONFIG_REQUIRED");
    let out = canon(d, &["context-pack", "verify", "sha256:0000000000000000000000000000000000000000000000000000000000000000"], None);
    assert_eq!((out.status.code(), body(&out)["error"]["code"].as_str().map(str::to_string)), (Some(2), Some("CONTEXT_PACK_NOT_FOUND".to_string())));
}

/// atip.codeexit.08: four of the five commands that cannot start answer exit 2 and their new code with nothing written
/// (`DISPATCH_RUN_UNAVAILABLE` is proved by `cli_error` on the registry and by `spec030_dispatch_demo`).
#[test]
fn atip_codeexit_08_a_command_that_cannot_start_is_exit_2_with_its_code_and_writes_nothing() {
    // CONTEXT_GIT_REQUIRED: a pack records the commit, and git cannot be run (no `git` on the PATH).
    let plain = git_repo();
    let p = plain.path();
    ok(p, &["init", "--providers", "none"], None);
    std::fs::write(p.join("doc.md"), "hi\n").unwrap();
    std::fs::write(p.join("m.json"), r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#).unwrap();
    let before = snapshot(p);
    let no_git = TempDir::new().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["context-pack", "add", "--manifest", "m.json", "--json"]).current_dir(p).env("PATH", no_git.path()).env_remove("CANON_ROLE").output().unwrap();
    let b = body(&out);
    assert_eq!((out.status.code(), b["error"]["code"].as_str(), b["error"]["retryable"].as_bool()), (Some(2), Some("CONTEXT_GIT_REQUIRED"), Some(false)), "{b}");
    assert!(b.get("data").is_none(), "{b}");
    assert_eq!(snapshot(p), before, "the refusal wrote nothing");

    // INGEST_SOURCE_INVALID: `root:` one level above the `changes` directory.
    let dir = git_repo();
    let d = dir.path();
    std::fs::write(d.join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: openspec\n").unwrap();
    std::fs::create_dir_all(d.join("openspec/changes/widget")).unwrap();
    std::fs::write(d.join("openspec/changes/widget/proposal.md"), "## Why\n\nWidgets.\n").unwrap();
    std::fs::write(d.join("openspec/changes/widget/tasks.md"), "- [ ] 1.1 wire it\n").unwrap();
    refused(d, None, &["ingest", "plans"], "INGEST_SOURCE_INVALID", false);

    // DASHBOARD_PORT_UNAVAILABLE (retryable) and DASHBOARD_UNSUPPORTED: a taken port, and a repo with no dashboard app.
    let taken = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = taken.local_addr().unwrap().port().to_string();
    let package = d.join("packages/dashboard");
    std::fs::create_dir_all(package.join("dist")).unwrap();
    std::fs::write(package.join("package.json"), r#"{"name":"@acme/dashboard"}"#).unwrap();
    std::fs::write(package.join("dist/index.html"), "<html>someone else's app</html>").unwrap();
    refused(d, None, &["dashboard", "--port", "0"], "DASHBOARD_UNSUPPORTED", false);
    let live = TempDir::new().unwrap();
    let l = live.path();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        assert!(Command::new("git").args(args).current_dir(l).status().unwrap().success());
    }
    ok(l, &["init", "--providers", "none"], None);
    let app = l.join("packages/dashboard");
    std::fs::create_dir_all(app.join("dist/snapshot")).unwrap();
    std::fs::write(app.join("package.json"), r#"{"name":"@canon-dev/dashboard"}"#).unwrap();
    for page in ["index.html", "briefs.html"] {
        std::fs::write(app.join("dist").join(page), format!("<html>canon dashboard {page}</html>")).unwrap();
    }
    std::fs::write(app.join("dist/snapshot/manifest.json"), r#"{"demo":true}"#).unwrap();
    let b = refused(l, None, &["dashboard", "--port", &port], "DASHBOARD_PORT_UNAVAILABLE", true);
    assert!(b["error"]["message"].as_str().unwrap().contains(&port), "{b}");
}

/// atip.codeexit.09: the retry values of the five commands that cannot start are the table's: the port and the closing run are retryable.
#[test]
fn atip_codeexit_09_the_retry_values_of_the_commands_that_cannot_start_are_the_tables() {
    for (code, retryable) in [("DASHBOARD_PORT_UNAVAILABLE", true), ("DISPATCH_RUN_UNAVAILABLE", true), ("DASHBOARD_UNSUPPORTED", false), ("CONTEXT_GIT_REQUIRED", false), ("INGEST_SOURCE_INVALID", false)] {
        let info = canon_cli::cli_error::all_codes().iter().find(|info| info.code == code).unwrap_or_else(|| panic!("{code}"));
        assert_eq!((info.exit, info.retryable), (2, retryable), "{code}");
    }
}

/// The table of the brief: the old code, its new codes, and the new exit of each.
const EXIT_TABLE: &[(&str, &[&str], u8)] = &[
    ("ADAPTER_EXECUTION_NOT_AUTHORIZED", &["ADAPTER_AUTHORIZATION_FAILED"], 1),
    ("ADAPTER_RESPONSE_INVALID", &["ADAPTER_VALIDATION_FAILED"], 1),
    ("BRIEF_CHANGED_SINCE_APPROVAL", &["BRIEF_APPROVAL_STALE"], 2),
    ("BRIEF_UNAPPROVED", &["BRIEF_APPROVAL_REQUIRED"], 2),
    ("CONTEXT_GIT_UNAVAILABLE", &["CONTEXT_GIT_REQUIRED"], 2),
    ("CONTEXT_OBJECT_CONFLICT", &["CONTEXT_OBJECT_CONFLICT"], 2),
    ("CONTEXT_PACK_TAMPERED", &["CONTEXT_PACK_VERIFY_FAILED", "CONTEXT_PACK_CORRUPT"], 0),
    ("DASHBOARD_NOT_BUILT", &["DASHBOARD_UNSUPPORTED"], 2),
    ("DASHBOARD_PORT_IN_USE", &["DASHBOARD_PORT_UNAVAILABLE"], 2),
    ("DISPATCH_END_IN_PROGRESS", &["DISPATCH_RUN_UNAVAILABLE"], 2),
    ("DIVERGENCE_REFUSED", &["DIVERGENCE_INVALID"], 2),
    ("EVIDENCE_CONTRACT_MISMATCH", &["EVIDENCE_CONTRACT_CONFLICT"], 2),
    ("EVIDENCE_FABRICATED", &["EVIDENCE_NOTE_INVALID"], 2),
    ("EVIDENCE_REPORT_CONTRADICTION", &["EVIDENCE_REPORT_CONFLICT"], 2),
    ("GATE_TASK_BLOCKED", &["TASK_EVIDENCE_REQUIRED"], 2),
    ("GATE_TASK_EVIDENCE_INVALID", &["TASK_EVIDENCE_INVALID"], 2),
    ("INGEST_SOURCE_UNUSABLE", &["INGEST_SOURCE_INVALID"], 2),
    ("LEARN_EVALUATION_REJECTED", &["STRATEGY_EVALUATION_INVALID"], 2),
    ("LEARN_PROMOTION_BLOCKED", &["STRATEGY_PROMOTION_INVALID"], 2),
    ("POLICY_CHANGED_SINCE_APPROVAL", &["POLICY_APPROVAL_STALE"], 2),
    ("PROMOTE_REFUSED", &["PROMOTION_INVALID", "EVIDENCE_STALE"], 2),
    ("REPORT_STALE", &["REPORT_CHECK_FAILED"], 1),
    ("SUBJECT_GATE_VIOLATION", &["SUBJECT_APPROVAL_REQUIRED", "EVIDENCE_STALE", "SUBJECT_INVALID", "LAYOUT_MIGRATION_REQUIRED"], 2),
];

/// The rows of the CHANGELOG's exit table: `(old code, new code, exit before, exit after)`, read from the rows whose third cell starts with `<n> → <m>`
/// (or `1 or 2 → 2`).
fn changelog_exit_rows(changelog: &str) -> Vec<(String, String, u8, u8)> {
    let start = changelog.find("\n## Unreleased").expect("an Unreleased section");
    let rest = &changelog[start + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |n| n + 3);
    rest[..end]
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let ["", old, new, exit, ..] = cells.as_slice() else { return None };
            let code = |cell: &str| cell.strip_prefix('`').and_then(|c| c.strip_suffix('`')).map(str::to_string);
            let (before, after) = exit.split_once(" → ")?;
            let before = u8::try_from(before.chars().next()?.to_digit(10)?).ok()?;
            let after = u8::try_from(after.chars().next()?.to_digit(10)?).ok()?;
            Some((code(old)?, code(new)?, before, after))
        })
        .collect()
}

/// atip.codeexit.10: every row of the CHANGELOG's exit table is the registry's: each new code exists with the exit the row ends in, an old code
/// that is not also a new one is gone, the rows hold every code of the brief's table, and the headline counts the rows that moved from 1 to 2.
#[test]
fn atip_codeexit_10_every_row_of_the_exit_table_is_the_registrys() {
    let all = canon_cli::cli_error::all_codes();
    let changelog = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).unwrap();
    let rows = changelog_exit_rows(&changelog);
    assert!(rows.len() > 25, "the exit table was found: {} rows", rows.len());
    for (old, new, _, after) in &rows {
        let info = all.iter().find(|info| info.code == new).unwrap_or_else(|| panic!("the row `{old}` → `{new}` names a code the registry lacks"));
        assert_eq!(info.exit, *after, "`{new}` (row of `{old}`) exits {} in the registry, the row says {after}", info.exit);
        if old != new {
            assert!(!all.iter().any(|info| info.code == old), "`{old}` is still registered");
        }
    }
    // Every old code of the brief's table has a row.
    for (old, _, _) in EXIT_TABLE {
        assert!(rows.iter().any(|(o, ..)| o == old), "the CHANGELOG has no row for `{old}`");
    }
    // The headline counts the old codes whose exit moved from 1 to 2.
    let mut moved: Vec<&str> = rows.iter().filter(|(_, _, before, after)| *before == 1 && *after == 2).map(|(old, ..)| old.as_str()).collect();
    moved.sort_unstable();
    moved.dedup();
    assert!(changelog.contains(&format!("**{} codes changed from exit 1 to exit 2", moved.len())), "the headline says {} codes: {moved:?}", moved.len());
    assert_eq!(all.iter().find(|i| i.code == "CONTEXT_PACK_VERIFY_FAILED").unwrap().exit, 1);
    assert_eq!(all.iter().find(|i| i.code == "CONTEXT_PACK_CORRUPT").unwrap().exit, 2);
    assert!(canon_cli::cli_error::all_warnings().iter().any(|w| w.code == "POLICY_CHANGED_SINCE_APPROVAL"), "the warning keeps its name");
}

/// atip.codeexit.10 (failure case): a row whose exit disagrees with the registry, or whose count is wrong, is found.
#[test]
fn atip_codeexit_10_a_row_that_disagrees_with_the_registry_is_found() {
    let table = "\n## Unreleased\n\n| `BRIEF_UNAPPROVED` | `BRIEF_APPROVAL_REQUIRED` | 1 → 1 |\n| `NO_COMMIT` | `COMMIT_REQUIRED` | 1 → 2 (a note) |\n\n## 0.35.0\n";
    let rows = changelog_exit_rows(table);
    assert_eq!(rows, [("BRIEF_UNAPPROVED".to_string(), "BRIEF_APPROVAL_REQUIRED".to_string(), 1, 1), ("NO_COMMIT".to_string(), "COMMIT_REQUIRED".to_string(), 1, 2)]);
    let registry = canon_cli::cli_error::all_codes().iter().find(|i| i.code == "BRIEF_APPROVAL_REQUIRED").unwrap().exit;
    assert_ne!(registry, rows[0].3, "the first row says exit 1 where the registry says {registry}: the test above fails on it");
}

/// atip.codeexit.11: the branches on a canon exit that canon ships (the pre-commit hook, the npm launcher, the eval scripts) treat every non-zero
/// exit alike, so a refusal that moved from 1 to 2 changes none of them.
#[test]
fn atip_codeexit_11_no_shipped_branch_depends_on_exit_1_of_a_changed_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let hook = std::fs::read_to_string(root.join("crates/canon-gate/scripts/canon-gate-pre-commit.sh")).unwrap();
    assert!(hook.lines().any(|l| l.trim() == "canon gate check") && !branches_on_exit_1(&hook), "the hook blocks on any non-zero exit of `canon gate check`, which is unchanged");
    let launcher = std::fs::read_to_string(root.join("packages/cli/src/index.ts")).unwrap();
    assert!(!branches_on_exit_1(&launcher), "the launcher passes the exit through");
}

/// atip.codeexit.11 (failure case): a branch on exit `1` is found by the scan that guards the audit.
#[test]
fn atip_codeexit_11_a_branch_on_exit_1_is_found_by_the_scan() {
    assert!(branches_on_exit_1("canon gate check\nif [ $? -eq 1 ]; then echo refused; fi\n"));
    assert!(branches_on_exit_1("if (result.status === 1) refuse();"));
    assert!(branches_on_exit_1("if returncode == 1:"));
    assert!(!branches_on_exit_1("canon gate check || exit 1\nif [ $? -ne 0 ]; then exit 2; fi\n"));
}

/// Whether `text` compares an exit status with `1`.
fn branches_on_exit_1(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        let mentions_exit = line.contains("$?") || line.contains("status") || line.contains("returncode") || line.contains("exitCode") || line.contains("exit_code");
        mentions_exit && ["-eq 1", "== 1", "=== 1"].iter().any(|op| line.contains(op))
    })
}

/// atip.codeexit.01: no exit-2 answer carries `data`: the one place that prints an error with data refuses an exit-2 error in a debug build
/// (the build every test in this directory runs), so a refusal that grows a `data` member fails the test that meets it.
#[cfg(debug_assertions)]
#[test]
fn atip_codeexit_01_an_exit_2_error_cannot_be_emitted_with_data() {
    use canon_cli::cli_error::{CliError, CliErrorExt as _};
    let refusal = CliError::refused("ACTOR_DENIED", "x");
    assert_eq!(refusal.exit, 2);
    let result = std::panic::catch_unwind(|| refusal.emit_with_data("canon x", false, &canon_cli::adapter::ValidationFailure { valid: false, problem: "x".into() }));
    assert!(result.is_err(), "an exit-2 error with data is refused");
    let failure = CliError::failed("GATE_CHECK_FAILED", "x");
    assert_eq!(failure.exit, 1);
    assert_eq!(failure.emit_with_data("canon x", false, &canon_cli::adapter::ValidationFailure { valid: false, problem: "x".into() }), 1, "an exit-1 failure keeps its data");
}

/// atip.codeexit.04 (two causes): a `subject move` the gate blocks for two causes is exit 2 with no `data`; `error` is the first cause and
/// `details` lists both, each with its own code (a missing approval and an open blocker are different conditions); the status is unchanged.
#[test]
fn atip_codeexit_04_a_subject_move_blocked_for_two_causes_lists_both_in_details() {
    let dir = git_repo();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"], None);
    ok(repo, &["subject", "new", "demo", "--domain", "d", "--title", "T"], Some("planner"));
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "L"], Some("planner"));
    for to in ["specced", "building", "verifying"] {
        ok(repo, &["subject", "move", "demo", "--to", to], Some("planner"));
    }
    ok(repo, &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "blocker", "--introduced-unknown", "--actor", "r1", "--summary", "blocks", "--promote"], Some("reviewer"));
    let b = refused(repo, Some("planner"), &["subject", "move", "demo", "--to", "shipped"], "SUBJECT_APPROVAL_REQUIRED", false);
    let details = b["error"]["details"].as_array().unwrap_or_else(|| panic!("two causes are two details: {b}"));
    assert!(details.len() >= 2, "{b}");
    assert_eq!(details[0]["code"], b["error"]["code"], "the error is the first cause: {b}");
    let codes: Vec<&str> = details.iter().filter_map(|d| d["code"].as_str()).collect();
    assert!(codes.contains(&"SUBJECT_APPROVAL_REQUIRED") && codes.contains(&"SUBJECT_INVALID"), "each cause has its own code: {codes:?}");
}

/// atip.codeexit.08 (the hot rung): a refused `ingest plans` leaves the tree byte-identical even when canon.yaml has a sqlite hot rung, whose file
/// a store would create as it opens: the sources are judged before any store is opened.
#[test]
fn atip_codeexit_08_a_refused_ingest_does_not_create_the_hot_store() {
    let dir = git_repo();
    let d = dir.path();
    std::fs::write(d.join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\nrouting:\n  task: hot\n  change: local\nplans:\n  sources:\n    - dialect: openspec\n      root: openspec\n").unwrap();
    std::fs::create_dir_all(d.join("openspec/changes/widget")).unwrap();
    std::fs::write(d.join("openspec/changes/widget/proposal.md"), "## Why\n\nWidgets.\n").unwrap();
    std::fs::write(d.join("openspec/changes/widget/tasks.md"), "- [ ] 1.1 wire it\n").unwrap();
    refused(d, None, &["ingest", "plans"], "INGEST_SOURCE_INVALID", false);
    assert!(!d.join(".canon/hot.db").exists(), "no hot store was created");
}
