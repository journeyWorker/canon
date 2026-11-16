//! `canon change defer | drop | reopen` (0.42 `change-drop`), against the built `canon` binary. Tests are named by scenario id
//! (`change_drop_04_…` proves `change.drop.04`).

#[path = "support/hermetic.rs"]
mod hermetic;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration as StdDuration;

use canon_store::file_lock::FileLock;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const CHANGE: &str = "add-login";
const REASON: &str = "waiting on the vendor";

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").args(args).current_dir(repo).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn run_with(repo: &Path, role: Option<&str>, args: &[&str]) -> Output {
    let mut command = hermetic::canon();
    command.args(args).current_dir(repo);
    if let Some(role) = role {
        command.env("CANON_ROLE", role);
    }
    command.output().expect("spawn canon binary")
}

fn run(repo: &Path, args: &[&str]) -> Output {
    run_with(repo, None, args)
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = run(repo, args);
    assert!(out.status.success(), "{args:?}: {}", text(&out));
    out
}

/// `canon <args> --json` as `role`: the exit code and the one document.
fn call(repo: &Path, role: Option<&str>, args: &[&str]) -> (i32, Value) {
    let mut argv = args.to_vec();
    argv.push("--json");
    let out = run_with(repo, role, &argv);
    let body = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", text(&out)));
    (out.status.code().unwrap_or(-1), body)
}

fn brief() -> String {
    "# Change\n\n\
## Intent\n\nChange\n\nVisitors lose their cart when they switch devices.\n\n\
## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n\n\
## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n\
## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie. ==**F1 is the happy path.**==\n\n\
## Questions and assumptions\n\nNone.\n\n\
## Out of scope\n\nSocial sign-in.\n"
        .to_string()
}

fn brief_path(repo: &Path, change: &str) -> std::path::PathBuf {
    repo.join(format!(".canon/changes/{change}/brief.md"))
}

fn write_brief(repo: &Path, change: &str) {
    std::fs::write(brief_path(repo, change), brief()).unwrap();
}

fn approve(repo: &Path, change: &str, note: &str) -> Output {
    ok(repo, &["brief", "approve", change, "--approver", "Ana", "--note", note])
}

fn commit_all(repo: &Path) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "--allow-empty", "-m", "snapshot"]);
}

/// A git repo set up by `canon init`, with the subject `demo` and the change `add-login`.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.name", "t"]);
    git(repo, &["config", "user.email", "t@example.com"]);
    git(repo, &["commit", "-q", "--allow-empty", "-m", "x"]);
    ok(repo, &["init", "--no-harness", "--no-agents-md"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", CHANGE, "--subject", "demo", "--title", "Add login"]);
    dir
}

/// `change defer|drop <change> --reason <reason> --ref … --approver Ana`.
fn closing<'a>(leaf: &'a str, change: &'a str, reason: &'a str) -> Vec<&'a str> {
    vec!["change", leaf, change, "--reason", reason, "--ref", "chat 2026-11-16", "--approver", "Ana"]
}

fn reopening(change: &str) -> Vec<&str> {
    vec!["change", "reopen", change, "--reason", "the vendor answered", "--approver", "Ana"]
}

fn with<'a>(mut args: Vec<&'a str>, more: &[&'a str]) -> Vec<&'a str> {
    args.extend_from_slice(more);
    args
}

fn records_under(dir: &Path) -> Vec<Value> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "json") {
                found.push(serde_json::from_str::<Value>(&std::fs::read_to_string(&path).unwrap()).unwrap());
            }
        }
    }
    found.sort_by(|a, b| a["at"].as_str().cmp(&b["at"].as_str()));
    found
}

/// The `change_state` records the ledger holds now (the latest of each change), and the ones it superseded.
fn states(repo: &Path) -> Vec<Value> {
    records_under(&repo.join(".canon/ledger/kind=change_state"))
}

fn archived_states(repo: &Path) -> Vec<Value> {
    records_under(&repo.join(".canon/archive/kind=change_state"))
}

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                out.insert(path.strip_prefix(root).unwrap().display().to_string(), std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

fn code(body: &Value) -> (&str, &str) {
    (body["error"]["code"].as_str().unwrap_or_default(), body["error"]["reason"].as_str().unwrap_or_default())
}

fn warning_codes(body: &Value) -> Vec<String> {
    body["meta"]["warnings"].as_array().map(|w| w.iter().filter_map(|w| w["code"].as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// Writes one more version of subject `id` into the ledger, as `subject move` and `subject adopt` do: the newest version of it, with
/// the status, time and change list given, named by the digest of its content.
fn plant_subject(repo: &Path, id: &str, status: &str, at: DateTime<Utc>, changes: &[&str]) {
    let dir = repo.join(".canon/ledger/kind=subject");
    let mut newest: Option<Value> = None;
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let version: Value = serde_json::from_str(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
        if version["subject_id"] == id && newest.as_ref().is_none_or(|best| version["at"].as_str() > best["at"].as_str()) {
            newest = Some(version);
        }
    }
    let mut version = newest.expect("the subject has a version");
    version["status"] = json!(status);
    version["at"] = json!(at.to_rfc3339_opts(SecondsFormat::Micros, true));
    version["change_ids"] = json!(changes);
    let digest: String = Sha256::digest(serde_json::to_vec(&version).unwrap()).iter().take(4).map(|byte| format!("{byte:02x}")).collect();
    std::fs::write(dir.join(format!("{id}_{digest}.json")), serde_json::to_string_pretty(&version).unwrap()).unwrap();
}

/// change.drop.01: a human defers an approved change: one `change_state` record with the reason, the references, the approver and the
/// time, a report that names the subjects, and the two steps `status` and `gate check`.
#[test]
fn change_drop_01_a_human_defers_an_approved_change_and_the_decision_is_recorded() {
    let dir = repo();
    let repo = dir.path();
    write_brief(repo, CHANGE);
    approve(repo, CHANGE, "Go");
    let (exit, body) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!(exit, 0, "{body}");
    let data = &body["data"];
    assert_eq!((data["changeId"].as_str(), data["state"].as_str(), data["reason"].as_str(), data["approver"].as_str()), (Some(CHANGE), Some("deferred"), Some(REASON), Some("Ana")), "{body}");
    assert_eq!(data["refs"], json!(["chat 2026-11-16"]));
    assert_eq!(data["subjects"], json!(["demo"]));
    assert_eq!(data["recordsKept"], json!({"evidence": 0, "reviews": 0, "findings": 0}));
    assert!(data.get("relay").is_none() && data["unchanged"] != true, "{body}");
    assert!(DateTime::parse_from_rfc3339(data["at"].as_str().unwrap()).is_ok(), "{body}");
    let next = body["meta"]["next"].to_string();
    assert!(next.contains("status") && next.contains("gate") && next.contains("check"), "meta.next names `canon status` and `canon gate check`: {next}");

    let records = states(repo);
    assert_eq!(records.len(), 1, "{records:?}");
    let record = &records[0];
    assert_eq!((record["change_id"].as_str(), record["state"].as_str(), record["reason"].as_str(), record["approver"].as_str()), (Some(CHANGE), Some("deferred"), Some(REASON), Some("Ana")));
    assert_eq!(record["refs"], json!(["chat 2026-11-16"]));
    assert!(record.get("relay").is_none(), "{record}");
    assert!(std::fs::read_to_string(brief_path(repo, CHANGE)).unwrap().contains("Approved:"), "the brief and its approval stay");

    let human = repo_human();
    assert!(human.contains("recorded change `add-login` as deferred") && human.contains("next: canon status"), "{human}");
}

fn repo_human() -> String {
    let dir = repo();
    let out = ok(dir.path(), &closing("defer", CHANGE, REASON));
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// change.drop.02: an agent that relays the human's answer leaves its role, actor and tool on the record, and a repeat writes nothing.
#[test]
fn change_drop_02_a_relayed_defer_records_the_relay_and_a_repeat_is_unchanged() {
    let dir = repo();
    let repo = dir.path();
    let relayed = with(closing("defer", CHANGE, REASON), &["--relayed-via", "ask", "--actor", "impl-1"]);
    let (exit, first) = call(repo, Some("implementer"), &relayed);
    assert_eq!(exit, 0, "{first}");
    let relay = json!({"role": "implementer", "actor": "impl-1", "tool": "ask", "approver": "Ana", "answer": REASON});
    assert_eq!(first["data"]["relay"], relay, "{first}");
    let records = states(repo);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["relay"], relay, "{:?}", records[0]);

    let (exit, again) = call(repo, Some("implementer"), &relayed);
    assert_eq!(exit, 0, "{again}");
    assert_eq!(again["data"]["unchanged"], true, "{again}");
    assert_eq!(again["data"]["at"], first["data"]["at"], "the decision keeps its time");
    assert_eq!(states(repo), records, "nothing was written");
    assert!(archived_states(repo).is_empty());
    // The human's own repeat of the same values is a repeat too.
    let (exit, human) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!((exit, &human["data"]["unchanged"]), (0, &json!(true)), "{human}");
    assert_eq!(states(repo), records);
}

const CHORE: &str = "---\nkind: chore\n---\n\n# Bump the client\n\n## Intent\n\nBump the client\n\nThe client is two versions behind.\n\n## Category\n\ndependency\n\n## Invariants\n\n- Nothing changes. ==**No behavior changes.**==\n\n## Checks\n\n- The suite runs again. ==**The suite is the check.**==\n\n## Out of scope\n\nNone.\n";

/// change.drop.03: a change with a finding is deferred with `CHANGE_RECORDS_KEPT`, and the finding stays.
#[test]
fn change_drop_03_records_of_the_change_are_kept_and_counted_in_a_warning() {
    let dir = repo();
    let repo = dir.path();
    std::fs::write(brief_path(repo, CHANGE), CHORE).unwrap();
    let tasks = repo.join(".canon/changes/add-login/tasks.md");
    let mut plan = std::fs::read_to_string(&tasks).unwrap_or_else(|_| "# Tasks: Add login\n".to_string());
    plan.push_str("\n- [ ] 1 Write the scenarios\n");
    std::fs::write(&tasks, plan).unwrap();
    ok(repo, &["ingest", "plans"]);
    approve(repo, CHANGE, "Go");
    commit_all(repo);
    ok(repo, &["evidence", "add", "--task", "add-login#1", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor", "impl", "--promote"]);
    ok(repo, &["review", "change", CHANGE, "--actor", "reviewer-3", "--role", "reviewer", "--original-spec-ref", ".canon/changes/add-login/brief.md"]);
    ok(repo, &["finding", "add", "--change", CHANGE, "--round", "1", "--seq", "1", "--severity", "note", "--actor", "bob", "--summary", "Naming nit", "--promote"]);
    let kept = |repo: &Path| ["evidence", "review", "finding"].map(|kind| records_under(&repo.join(format!(".canon/ledger/kind={kind}"))));
    let before = kept(repo);
    assert_eq!(before.iter().map(Vec::len).collect::<Vec<_>>(), [1, 1, 1], "{before:?}");
    let (exit, body) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!(exit, 0, "{body}");
    assert!(warning_codes(&body).contains(&"CHANGE_RECORDS_KEPT".to_string()), "{body}");
    assert_eq!(body["data"]["recordsKept"], json!({"evidence": 1, "reviews": 1, "findings": 1}), "{body}");
    assert_eq!(kept(repo), before, "every record is as it was");
    for (kind, needle) in [("evidence", "add-login#1"), ("review", "add-login"), ("finding", "Naming nit")] {
        let (exit, listed) = call(repo, None, &["query", "--kind", kind]);
        assert!(exit == 0 && listed.to_string().contains(needle), "{kind} is still queried: {listed}");
    }
}

/// change.drop.04: the human-only, relay and argument refusals each write nothing.
#[test]
fn change_drop_04_an_agent_without_the_humans_answer_and_bad_relays_and_arguments_are_refused() {
    let dir = repo();
    let repo = dir.path();
    let plain = closing("defer", CHANGE, REASON);
    let (exit, body) = call(repo, Some("implementer"), &plain);
    assert_eq!((exit, code(&body).0), (2, "ACTOR_DENIED"), "{body}");
    assert!(body["error"]["hint"].as_str().unwrap().contains("--relayed-via"), "{body}");
    let (exit, body) = call(repo, Some("implementer"), &with(plain.clone(), &["--relayed-via", "ask"]));
    assert_eq!((exit, code(&body).0), (2, "USAGE"), "a relay needs --actor: {body}");
    let (exit, body) = call(repo, None, &with(plain.clone(), &["--relayed-via", "ask", "--actor", "impl-1"]));
    assert_eq!((exit, code(&body).0), (2, "USAGE"), "a relay needs a declared role: {body}");

    let (exit, body) = call(repo, None, &closing("defer", CHANGE, " "));
    assert_eq!((exit, code(&body).0, body["error"]["field"].as_str()), (2, "CHANGE_ARGUMENT_INVALID", Some("--reason")), "{body}");
    let (exit, body) = call(repo, None, &["change", "defer", CHANGE, "--reason", REASON, "--ref", " ", "--approver", "Ana"]);
    assert_eq!((exit, code(&body).0, body["error"]["field"].as_str()), (2, "CHANGE_ARGUMENT_INVALID", Some("--ref")), "{body}");
    let (exit, body) = call(repo, None, &["change", "defer", CHANGE, "--reason", REASON, "--ref", "chat", "--approver", " "]);
    assert_eq!((exit, code(&body).0, body["error"]["field"].as_str()), (2, "APPROVAL_INVALID", Some("--approver")), "{body}");
    let (exit, body) = call(repo, None, &["change", "defer", CHANGE, "--reason", REASON, "--approver", "Ana"]);
    assert_eq!((exit, code(&body).0), (2, "USAGE"), "--ref is required: {body}");
    assert!(states(repo).is_empty(), "no refusal wrote a record");

    // A canon.yaml that turns relaying off refuses a relayed call.
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    std::fs::write(repo.join("canon.yaml"), format!("{yaml}\napproval:\n  relay: deny\n")).unwrap();
    let (exit, body) = call(repo, Some("implementer"), &with(plain, &["--relayed-via", "ask", "--actor", "impl-1"]));
    assert_eq!((exit, code(&body).0), (2, "ACTOR_DENIED"), "{body}");
    assert!(states(repo).is_empty());
}

/// change.drop.05: an unknown change, and a different repeat on a closed one, are refused and write nothing.
#[test]
fn change_drop_05_an_unknown_change_and_a_different_repeat_on_a_closed_change_are_refused() {
    let dir = repo();
    let repo = dir.path();
    let (exit, body) = call(repo, None, &closing("defer", "no-such-change", REASON));
    assert_eq!((exit, code(&body).0), (2, "CHANGE_NOT_FOUND"), "{body}");
    assert!(body["error"]["hint"].as_str().unwrap().contains("ingest plans"), "the hint says to import the change first: {body}");
    assert!(states(repo).is_empty());

    let (exit, _) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!(exit, 0);
    let recorded = states(repo);
    let (exit, body) = call(repo, None, &closing("defer", CHANGE, "another reason"));
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "CLOSED")), "{body}");
    assert_eq!(states(repo), recorded, "the decision is as it was");
}

/// change.drop.06: a change that shipped is refused with `SHIPPED` (the first approval is what counts, whatever the later ones), one
/// adopted into a shipped subject or a draft at the ship is closed, and a history that cannot be read refuses.
#[test]
fn change_drop_06_a_shipped_change_is_refused_and_one_that_never_shipped_is_closed() {
    // Shipped, reopened, approved again.
    let dir = repo();
    let repo = dir.path();
    write_brief(repo, CHANGE);
    approve(repo, CHANGE, "Go");
    commit_all(repo);
    let now = Utc::now();
    plant_subject(repo, "demo", "shipped", now, &[CHANGE]);
    plant_subject(repo, "demo", "verifying", now + Duration::seconds(1), &[CHANGE]);
    approve(repo, CHANGE, "Go again");
    commit_all(repo);
    let (exit, body) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "SHIPPED")), "{body}");
    assert!(body["error"]["hint"].as_str().unwrap().contains("canon change new"), "the hint says to write a new change: {body}");
    assert!(states(repo).is_empty());
    let (exit, body) = call(repo, None, &closing("drop", CHANGE, REASON));
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "SHIPPED")), "{body}");

    // Adopted into a subject that had shipped already: no ship.
    let dir = repo_with_approved_change();
    let repo = dir.path();
    ok(repo, &["subject", "new", "other", "--domain", "dev", "--title", "Other"]);
    let now = Utc::now();
    plant_subject(repo, "other", "shipped", now, &[]);
    plant_subject(repo, "other", "shipped", now + Duration::seconds(1), &[CHANGE]);
    let (exit, body) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!(exit, 0, "{body}");

    // An unapproved draft when the subject shipped: no ship.
    let dir = crate::repo();
    let repo = dir.path();
    let now = Utc::now();
    plant_subject(repo, "demo", "proposed", now - Duration::hours(3), &[CHANGE]);
    plant_subject(repo, "demo", "shipped", now - Duration::hours(2), &[CHANGE]);
    write_brief(repo, CHANGE);
    approve(repo, CHANGE, "Go");
    commit_all(repo);
    let (exit, body) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!(exit, 0, "{body}");

    // A subject history that cannot be read refuses the same way.
    let dir = crate::repo();
    let repo = dir.path();
    let ledger = repo.join(".canon/ledger/kind=subject");
    let any = std::fs::read_dir(&ledger).unwrap().flatten().next().unwrap().path();
    std::fs::copy(&any, ledger.join("demo_00000000.json")).unwrap();
    let (exit, body) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "SHIPPED")), "{body}");
    assert!(states(repo).is_empty());

    // A change deferred while its subject shipped without it did not ship: a human drops it.
    let dir = repo_with_approved_change();
    let repo = dir.path();
    let (exit, body) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!(exit, 0, "{body}");
    plant_subject(repo, "demo", "shipped", Utc::now(), &[CHANGE]);
    let (exit, body) = call(repo, None, &closing("drop", CHANGE, "not building it"));
    assert_eq!((exit, body["data"]["state"].as_str()), (0, Some("dropped")), "a change the subject shipped without is not a shipped change: {body}");

    // The subject reopened, the change reopened, approved again and shipped with it: that move is a ship.
    let dir = repo_with_approved_change();
    let repo = dir.path();
    let (exit, body) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!(exit, 0, "{body}");
    plant_subject(repo, "demo", "shipped", Utc::now(), &[CHANGE]);
    plant_subject(repo, "demo", "verifying", Utc::now(), &[CHANGE]);
    let (exit, body) = call(repo, None, &reopening(CHANGE));
    assert_eq!(exit, 0, "{body}");
    approve(repo, CHANGE, "Go again");
    plant_subject(repo, "demo", "shipped", Utc::now(), &[CHANGE]);
    let (exit, body) = call(repo, None, &closing("drop", CHANGE, "not building it"));
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "SHIPPED")), "the move after the reopening shipped it: {body}");
    assert_eq!(states(repo).iter().map(|s| s["state"].as_str().unwrap()).collect::<Vec<_>>(), ["reopened"], "nothing was written");
}

fn repo_with_approved_change() -> TempDir {
    let dir = repo();
    write_brief(dir.path(), CHANGE);
    approve(dir.path(), CHANGE, "Go");
    commit_all(dir.path());
    dir
}

/// change.drop.07: an approved change, a deferred change and a draft are dropped, each recorded as dropped (the deferral is history), and the
/// report says a dropped change is not brought back, only a new change is.
#[test]
fn change_drop_07_a_human_drops_an_approved_change_a_deferred_change_and_a_draft() {
    let dir = repo_with_approved_change();
    let repo = dir.path();
    let said = text(&ok(repo, &closing("drop", CHANGE, "not building it")));
    assert!(said.contains("recorded change `add-login` as dropped"), "{said}");
    assert!(said.contains("a dropped change is not brought back, only a new change is"), "the report says it: {said}");
    assert!(said.contains("canon change new") && said.contains("`add-login`") && said.contains("`Source:`"), "and names the way: {said}");
    assert_eq!(states(repo)[0]["state"], "dropped");

    let dir = crate::repo();
    let repo = dir.path();
    let (exit, body) = call(repo, None, &closing("drop", CHANGE, "not building it"));
    assert_eq!((exit, body["data"]["state"].as_str()), (0, Some("dropped")), "{body}");
    assert_eq!(states(repo)[0]["state"], "dropped");

    let dir = crate::repo();
    let repo = dir.path();
    let (exit, _) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!(exit, 0);
    let (exit, body) = call(repo, None, &closing("drop", CHANGE, "not building it"));
    assert_eq!((exit, body["data"]["state"].as_str()), (0, Some("dropped")), "{body}");
    assert_eq!(states(repo).iter().map(|s| s["state"].as_str().unwrap()).collect::<Vec<_>>(), ["dropped"]);
    assert_eq!(archived_states(repo).iter().map(|s| s["state"].as_str().unwrap()).collect::<Vec<_>>(), ["deferred"], "the deferral is history");
}

/// change.drop.08: a dropped change takes no other state, and a dry run refuses with the same code and writes nothing.
#[test]
fn change_drop_08_a_dropped_change_takes_no_other_state_and_a_dry_run_writes_nothing() {
    let dir = repo();
    let repo = dir.path();
    let (exit, _) = call(repo, None, &closing("drop", CHANGE, "not building it"));
    assert_eq!(exit, 0);
    let recorded = states(repo);
    for args in [closing("defer", CHANGE, "not building it"), closing("drop", CHANGE, "another reason")] {
        let (exit, body) = call(repo, None, &args);
        assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "CLOSED")), "{body}");
        let (exit, body) = call(repo, None, &with(args, &["--dry-run"]));
        assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "CLOSED")), "a dry run refuses like the real call: {body}");
    }
    let (exit, body) = call(repo, None, &reopening(CHANGE));
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "DROPPED")), "{body}");
    let hint = body["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("canon change new") && hint.contains("Source"), "{hint}");
    assert_eq!(states(repo), recorded);
    // The same values again are a repeat.
    let (exit, body) = call(repo, None, &closing("drop", CHANGE, "not building it"));
    assert_eq!((exit, &body["data"]["unchanged"]), (0, &json!(true)), "{body}");
}

/// change.drop.18: a human reopens a deferred change; its old approval is void, and the same name and answer approve it again.
#[test]
fn change_drop_18_a_reopened_change_needs_a_new_approval_even_for_the_same_words() {
    let dir = repo();
    let repo = dir.path();
    write_brief(repo, CHANGE);
    approve(repo, CHANGE, "Go");
    // Control: the same approval again changes nothing.
    let (exit, again) = call(repo, None, &["brief", "approve", CHANGE, "--approver", "Ana", "--note", "Go"]);
    assert_eq!((exit, &again["data"]["unchanged"]), (0, &json!(true)), "{again}");
    let approved = std::fs::read_to_string(brief_path(repo, CHANGE)).unwrap();

    let (exit, _) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!(exit, 0);
    let (exit, body) = call(repo, None, &reopening(CHANGE));
    assert_eq!((exit, body["data"]["state"].as_str(), &body["data"]["subjects"]), (0, Some("reopened"), &json!(["demo"])), "{body}");
    assert_eq!(states(repo)[0]["state"], "reopened");

    let (_, shown) = call(repo, None, &["brief", "show", CHANGE]);
    assert_ne!(shown["data"]["state"], "approved", "the old approval is not an approval: {shown}");
    let status = text(&ok(repo, &["status"]));
    assert!(status.contains("stale"), "status says the approval is stale: {status}");

    let (exit, again) = call(repo, None, &["brief", "approve", CHANGE, "--approver", "Ana", "--note", "Go"]);
    assert_eq!((exit, &again["data"]["unchanged"]), (0, &Value::Null), "a new line is written: {again}");
    let renewed = std::fs::read_to_string(brief_path(repo, CHANGE)).unwrap();
    assert_ne!(renewed, approved, "the new line replaces the old one");
    let (_, shown) = call(repo, None, &["brief", "show", CHANGE]);
    assert_eq!(shown["data"]["state"], "approved", "{shown}");
}

/// change.drop.19: a change that was never closed or was dropped is not reopened; an agent needs the human's answer.
#[test]
fn change_drop_19_only_a_deferred_change_reopens_and_only_with_the_humans_answer() {
    let dir = repo();
    let repo = dir.path();
    let (exit, body) = call(repo, None, &reopening(CHANGE));
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "NOT_DEFERRED")), "{body}");
    let (exit, body) = call(repo, Some("implementer"), &reopening(CHANGE));
    assert_eq!((exit, code(&body).0), (2, "ACTOR_DENIED"), "{body}");
    let (exit, body) = call(repo, Some("implementer"), &with(reopening(CHANGE), &["--relayed-via", "ask"]));
    assert_eq!((exit, code(&body).0), (2, "USAGE"), "{body}");
    assert!(states(repo).is_empty());

    let (exit, _) = call(repo, None, &closing("drop", CHANGE, "not building it"));
    assert_eq!(exit, 0);
    let (exit, body) = call(repo, None, &reopening(CHANGE));
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "DROPPED")), "{body}");

    // A relayed reopening of a deferred change records the relay.
    let dir = crate::repo();
    let repo = dir.path();
    call(repo, None, &closing("defer", CHANGE, REASON));
    let (exit, body) = call(repo, Some("implementer"), &with(reopening(CHANGE), &["--relayed-via", "ask", "--actor", "impl-1"]));
    assert_eq!(exit, 0, "{body}");
    assert_eq!(states(repo)[0]["relay"]["actor"], "impl-1");

    // A repo that denies relaying refuses a relayed reopening, and the change stays deferred.
    let dir = crate::repo();
    let repo = dir.path();
    call(repo, None, &closing("defer", CHANGE, REASON));
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    std::fs::write(repo.join("canon.yaml"), format!("{yaml}\napproval:\n  relay: deny\n")).unwrap();
    let (exit, body) = call(repo, Some("implementer"), &with(reopening(CHANGE), &["--relayed-via", "ask", "--actor", "impl-1"]));
    assert_eq!((exit, code(&body).0), (2, "ACTOR_DENIED"), "{body}");
    assert_eq!(states(repo).iter().map(|s| s["state"].as_str().unwrap()).collect::<Vec<_>>(), ["deferred"], "nothing was written");
}

/// change.drop.20: `subject unadopt` keeps refusing an approved change, and its hint names the two decisions.
#[test]
fn change_drop_20_unadopt_keeps_refusing_and_its_hint_names_change_defer_and_change_drop() {
    let dir = repo();
    let repo = dir.path();
    write_brief(repo, CHANGE);
    approve(repo, CHANGE, "Go");
    let (exit, body) = call(repo, None, &["subject", "unadopt", "demo", "--change", CHANGE]);
    assert_eq!((exit, code(&body)), (2, ("SUBJECT_CONFLICT", "APPROVED_BRIEF")), "{body}");
    let hint = body["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("canon change defer") && hint.contains("canon change drop"), "{hint}");

    // A change with a flow-tagged scenario: unadopt refuses too, and its hint names the same two decisions.
    ok(repo, &["change", "new", "second", "--subject", "demo", "--title", "Second"]);
    write_brief(repo, "second");
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    let feature = ".canon/specs/kind=feature/area=auth/login.feature";
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--change", "second", "--flow", "F1", "--feature", feature]);
    ok(repo, &["inventory", "sync"]);
    let (exit, body) = call(repo, None, &["subject", "unadopt", "demo", "--change", "second"]);
    assert_eq!((exit, code(&body)), (2, ("SUBJECT_CONFLICT", "FLOW_TAGGED")), "{body}");
    let hint = body["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("canon change defer") && hint.contains("canon change drop"), "{hint}");
}

/// change.drop.21: a brief approval (one id or several), the dashboard's approval and a flow-tagged scenario for a closed change are
/// refused with `CLOSED`, and nothing is written.
#[test]
fn change_drop_21_writes_for_a_closed_change_are_refused() {
    let dir = repo();
    let repo = dir.path();
    ok(repo, &["change", "new", "second", "--subject", "demo", "--title", "Second"]);
    write_brief(repo, CHANGE);
    write_brief(repo, "second");
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    let (exit, _) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!(exit, 0);
    let before = tree(repo);

    let (exit, body) = call(repo, None, &["brief", "approve", CHANGE, "--approver", "Ana", "--note", "Go"]);
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "CLOSED")), "{body}");
    let (exit, body) = call(repo, None, &["brief", "approve", "second", CHANGE, "--approver", "Ana", "--note", "Go"]);
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "CLOSED")), "one closed id refuses them all: {body}");
    assert!(!std::fs::read_to_string(brief_path(repo, "second")).unwrap().contains("Approved:"), "the open change was not approved either");

    let feature = ".canon/specs/kind=feature/area=auth/login.feature";
    let (exit, body) = call(repo, None, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--change", CHANGE, "--flow", "F1", "--feature", feature]);
    assert_eq!((exit, code(&body)), (2, ("CHANGE_CONFLICT", "CLOSED")), "{body}");

    let sha = canon_cli::brief_approve::sha256_hex(&std::fs::read(brief_path(repo, CHANGE)).unwrap());
    let request = json!({"changeId": CHANGE, "sha256": sha, "by": "Ana", "note": "Go"}).to_string();
    let reply = canon_cli::dashboard_briefs::approve(repo, request.as_bytes());
    assert_eq!(reply.status, 409, "{:?}", reply.body);
    assert_eq!(reply.body["error"]["code"], "CHANGE_CONFLICT", "{:?}", reply.body);
    let listed = canon_cli::dashboard_briefs::list(repo).body;
    let ids: Vec<&str> = listed["data"]["briefs"].as_array().unwrap().iter().filter_map(|b| b["changeId"].as_str()).collect();
    assert!(!ids.contains(&CHANGE), "a closed change is not offered for approval: {ids:?}");
    assert_eq!(tree(repo), before, "nothing was written");
}

/// change.drop.29: the three leaves read `--input`, preview with `--dry-run`, wait for a held lock and repeat safely.
#[test]
fn change_drop_29_input_dry_run_held_lock_and_repeat() {
    let dir = repo();
    let repo = dir.path();
    // A dry run reports the decision and writes nothing.
    let before = tree(repo);
    let (exit, body) = call(repo, None, &with(closing("defer", CHANGE, REASON), &["--dry-run"]));
    assert_eq!((exit, &body["data"]["dryRun"], body["data"]["state"].as_str()), (0, &json!(true), Some("deferred")), "{body}");
    assert_eq!(tree(repo), before);
    assert!(states(repo).is_empty());

    // A held lock is a refusal that writes nothing.
    let held = FileLock::acquire(&repo.join(".canon/repo-write.lock"), StdDuration::ZERO).unwrap();
    let before = tree(repo);
    let mut command = hermetic::canon();
    command.args(closing("defer", CHANGE, REASON)).arg("--json").current_dir(repo).env("CANON_TEST_REPO_LOCK_WAIT_MS", "200");
    let out = command.output().unwrap();
    let body: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!((out.status.code(), code(&body).0), (Some(2), "REPO_LOCK_UNAVAILABLE"), "{body}");
    assert_eq!(tree(repo), before);
    drop(held);

    // One JSON document is read.
    let input = json!({"operands": [CHANGE], "reason": REASON, "ref": ["chat 2026-11-16"], "approver": "Ana"}).to_string();
    let (exit, body) = call(repo, None, &["change", "defer", "--input", &input]);
    assert_eq!((exit, body["data"]["state"].as_str()), (0, Some("deferred")), "{body}");
    // A repeat is safe, for each of the leaves that repeat.
    let (exit, body) = call(repo, None, &closing("defer", CHANGE, REASON));
    assert_eq!((exit, &body["data"]["unchanged"]), (0, &json!(true)), "{body}");
    assert_eq!(states(repo).len(), 1);
}
