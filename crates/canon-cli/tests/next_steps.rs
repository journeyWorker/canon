//! `meta.next` after a successful write (0.21 D2) and role filtering of
//! `status` and `meta.next` (0.21 D3), against the built binary.
//!
//! The step a write names is `canon status`'s own step for the subject it
//! touched, so each write is checked against `status --json` at that moment,
//! and one scenario is carried from its brief to `shipped` by running only
//! what `meta.next` says.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// Makes `repo` a git work tree with one commit, which every revision-bound
/// write needs.
fn git_seed(repo: &Path) {
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(repo).output().expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

fn canon(repo: &Path, args: &[&str], role: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).current_dir(repo).env("CANON_ACTOR", "agent-a").env_remove("CANON_ROLE");
    if let Some(role) = role {
        command.env("CANON_ROLE", role);
    }
    command.output().expect("spawn canon binary")
}

fn json_of(out: &Output, what: &str) -> Value {
    assert!(out.status.success(), "{what}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{what}: stdout is not one JSON value ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

/// `status --json` as one report: `data` (with its `waitingOn` and
/// `nextOmitted`) plus the `meta.next` it carries, `[]` when absent. A step
/// that holds a `<placeholder>` is not an entry, so see [`status_text`].
fn status_report(repo: &Path, role: Option<&str>) -> Value {
    let doc = json_of(&canon(repo, &["status", "--json"], role), "status");
    assert_eq!(doc["ok"], true, "{doc}");
    let mut report = doc["data"].clone();
    report["next"] = meta_next_of(&doc);
    report
}

/// A response's `meta.next`, `[]` when the member is absent (never `[]` on the wire).
fn meta_next_of(doc: &Value) -> Value {
    doc["meta"].get("next").cloned().unwrap_or_else(|| serde_json::json!([]))
}

/// The steps of `canon status` as its human form lists them (`# why` over the
/// command; `human:` before the human's step), under `role`. Each is
/// `{actor, command, why}`; the role is not printed.
fn status_text(repo: &Path, role: Option<&str>) -> Vec<Value> {
    let out = canon(repo, &["status"], role);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let Some((_, block)) = text.split_once("\nnext:\n") else { return Vec::new() };
    let mut steps = Vec::new();
    let mut why = None;
    for line in block.lines() {
        if let Some(reason) = line.strip_prefix("  # ") {
            why = Some(reason.to_string());
        } else if let (Some(command), Some(reason)) = (line.strip_prefix("  "), why.take()) {
            let (actor, command) = command.strip_prefix("human: ").map_or(("agent", command), |rest| ("human", rest));
            steps.push(serde_json::json!({"actor": actor, "command": command, "why": reason}));
        }
    }
    steps
}

/// The last `next: <command> — <why>` line of a write's human output.
fn next_line(text: &str) -> Option<(String, String)> {
    let line = text.lines().last()?.strip_prefix("next: ")?;
    let (command, why) = line.split_once(" — ")?;
    let why = why.rsplit_once(" (+").filter(|(_, count)| count.ends_with(" more)")).map_or(why, |(why, _)| why);
    Some((command.to_string(), why.to_string()))
}

/// What `canon status` names first for `subject`, as a write's human `next:` line prints it.
fn status_first(repo: &Path, subject: &str) -> (String, String) {
    let steps = status_text(repo, None);
    let step = steps.iter().find(|step| step["why"].as_str().unwrap().starts_with(subject)).unwrap_or_else(|| panic!("status names no step for {subject}: {steps:?}"));
    let command = step["command"].as_str().unwrap();
    (if step["actor"] == "human" { format!("human: {command}") } else { command.to_string() }, step["why"].as_str().unwrap().to_string())
}

/// A write run in human mode: its output, which must end in the `next:` line `status` names for `subject`.
fn text_write_names_status_step(repo: &Path, args: &[&str], subject: &str, what: &str) {
    let out = canon(repo, args, None);
    assert!(out.status.success(), "{what}: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert_eq!(next_line(&text), Some(status_first(repo, subject)), "{what}: {text}");
}

fn ok(repo: &Path, args: &[&str]) -> Value {
    json_of(&canon(repo, args, None), &format!("{args:?}"))
}

/// A complete brief with the one flow step `F1`.
fn brief() -> String {
    "# Add login\n\n\
## Intent\n\nAdd login\n\nVisitors lose their cart when they switch devices.\n\n\
## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n\n\
## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n\
## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie. ==**F1 is the happy path.**==\n\n\
## Questions and assumptions\n\nNone.\n\n\
## Out of scope\n\nSocial sign-in.\n"
        .to_string()
}

/// `init` (strict policy: evidence, a failure case and an independent
/// review), subject `demo`, and change `add-login` with a clean brief.
fn repo_with_change() -> (TempDir, Value) {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    git_seed(repo);
    ok(repo, &["init", "--providers", "none", "--json"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo", "--json"]);
    let created = ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login", "--json"]);
    std::fs::write(repo.join(".canon/changes/add-login/brief.md"), brief()).unwrap();
    (dir, created)
}

/// What `status --json` lists as `meta.next` for `subject`: the entries whose reason starts with it.
fn status_entries(repo: &Path, subject: &str) -> Vec<Value> {
    status_report(repo, None)["next"].as_array().unwrap().iter().filter(|step| step["reason"].as_str().unwrap_or("").starts_with(subject)).cloned().collect()
}

/// A response's `meta.next` entries, which must be there.
fn meta_next(out: &Value) -> Vec<Value> {
    out["meta"]["next"].as_array().unwrap_or_else(|| panic!("no meta.next in {out}")).clone()
}

/// Splits `command` on spaces, keeping double-quoted text together, and
/// fills the placeholders an agent fills.
fn words(command: &str) -> Vec<String> {
    let (mut out, mut word, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in command.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            ' ' if !quoted => {
                if any || !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
                any = false;
            }
            _ => word.push(c),
        }
    }
    if any || !word.is_empty() {
        out.push(word);
    }
    out.into_iter()
        .map(|w| match w.as_str() {
            "<unit>" => "agent-a".to_string(),
            "<session>" => "s1".to_string(),
            "<review-session>" => "s2".to_string(),
            "<reviewer>" => "rev".to_string(),
            "<sha>" => "0123456789abcdef0123456789abcdef01234567".to_string(),
            "<test command>" => "cargo test".to_string(),
            "<feature file>" => ".canon/specs/kind=feature/area=auth/login.feature".to_string(),
            "<behavior>" | "<what happens on this path>" => "behaves".to_string(),
            w if w.starts_with("<area>.<surface>.") => w.replace("<area>.<surface>", "auth.login"),
            _ => w,
        })
        .collect()
}

#[test]
fn writes_name_the_step_status_names_for_the_subject_they_touched() {
    let (dir, created) = repo_with_change();
    let repo = dir.path();

    // `change new`: the brief is still a skeleton with blockers, so the
    // subject's step is the planner's, not the human's (0.22 D4). It is a
    // runnable command, so it is a `meta.next` entry: `argv` without the
    // executable and with `--json`, `reason`, and who takes it in `x-canon`.
    let steps = meta_next(&created);
    assert_eq!(steps[0]["x-canon"], serde_json::json!({"actor": "agent", "role": "planner"}), "{}", steps[0]);
    assert_eq!(steps[0]["command"], "canon brief check add-login --json");
    assert_eq!(steps[0]["argv"], serde_json::json!(["brief", "check", "add-login", "--json"]));
    assert!(steps[0]["reason"].as_str().unwrap().contains("blocker(s): write the brief"), "{}", steps[0]);
    assert!(steps[0].get("why").is_none() && steps[0].get("actor").is_none() && steps[0].get("role").is_none(), "{}", steps[0]);

    // Once the brief is written clean, the one human stop is the subject's only step:
    // not a canon command, so not a `meta.next` entry; the human form lists it
    // and a declared role counts it as the human's.
    let human = status_text(repo, None);
    assert_eq!(human.len(), 1, "{human:?}");
    assert_eq!(human[0]["actor"], "human");
    assert!(human[0]["command"].as_str().unwrap().starts_with("show the brief in "), "{human:?}");
    assert!(status_entries(repo, "demo").is_empty());
    assert!(json_of(&canon(repo, &["status", "--json"], None), "status")["meta"].get("next").is_none(), "no entry, no member");
    assert_eq!(status_report(repo, Some("planner"))["waitingOn"], serde_json::json!({"human": 1}));

    let approved = ok(repo, &["brief", "approve", "--approver", "Ana", "--note", "Go.", "--json"]);
    assert_eq!(meta_next_of(&approved), meta_next_of(&json_of(&canon(repo, &["status", "--json"], None), "status")), "brief approve");
    assert_eq!(status_first(repo, "demo").0.split(' ').take(3).collect::<Vec<_>>(), ["canon", "scenario", "new"]);
    assert!(approved["meta"].get("next").is_none(), "the scenario step holds placeholders, so it is not an entry: {approved}");

    // Every later write ends its human output with the step `status` names.
    let args = ["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--case", "happy", "--flow", "F1"];
    text_write_names_status_step(repo, &args, "demo", "scenario new");

    text_write_names_status_step(repo, &["subject", "move", "demo", "--to", "specced"], "demo", "subject status");

    ok(repo, &["subject", "move", "demo", "--to", "building", "--json"]);
    text_write_names_status_step(
        repo,
        &["evidence", "add", "--scenario", "auth.login.01", "--project", "root", "--class", "test-run", "--role", "implementer", "--actor", "agent-a", "--session", "s1", "--verdict", "faithful", "--ref", "cargo test", "--promote"],
        "demo",
        "evidence add",
    );

    text_write_names_status_step(
        repo,
        &["review", "scenario", "auth.login.01", "--project", "root", "--actor", "rev", "--session", "s2", "--role", "reviewer", "--original-spec-ref", "login.feature"],
        "demo",
        "review add",
    );
}

#[test]
fn a_staged_write_names_the_promote_that_makes_it_count() {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    ok(repo, &["brief", "approve", "--approver", "Ana", "--note", "Go.", "--json"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--case", "happy", "--flow", "F1", "--json"]);
    let staged = ok(
        repo,
        &["evidence", "add", "--scenario", "auth.login.01", "--project", "root", "--class", "test-run", "--role", "implementer", "--actor", "agent-a", "--session", "s1", "--verdict", "faithful", "--ref", "cargo test", "--no-promote", "--json"],
    );
    let steps = meta_next(&staged);
    assert_eq!(steps.len(), 1, "{steps:?}");
    assert_eq!(steps[0]["command"], "canon gate promote --json");
    assert_eq!(steps[0]["argv"], serde_json::json!(["gate", "promote", "--json"]));
    assert_eq!(steps[0]["x-canon"], serde_json::json!({"actor": "agent", "role": "implementer"}), "{}", steps[0]);
}

#[test]
fn human_mode_ends_with_one_next_line_and_a_count_of_the_rest() {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    let out = canon(repo, &["brief", "approve", "--approver", "Ana", "--note", "Go."], None);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    let last = text.lines().last().unwrap();
    assert!(last.starts_with("next: canon scenario new "), "{text}");
    assert!(last.contains(" — "), "{last}");
}

/// Runs `args` in human mode and returns its output.
fn text(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args, None);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// The steps hold `<placeholder>` words an agent fills, which `meta.next`
/// never carries (it lists runnable commands only), so the walk follows the
/// human form's `next:` line, which names the same step.
#[test]
fn one_scenario_goes_from_its_brief_to_shipped_on_meta_next_alone() {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    // The human answers; everything after that is the next step each write names.
    let mut last = text(repo, &["brief", "approve", "--approver", "Ana", "--note", "Go."]);
    let mut ran = vec!["canon brief approve --by Ana --note Go.".to_string()];
    for _ in 0..24 {
        let Some((command, why)) = next_line(&last) else { break };
        assert!(!command.starts_with("human:"), "a human step ends the walk: {command} — {why}");
        let mut args = words(&command);
        assert_eq!(args.remove(0), "canon", "{command}");
        // The record steps carry what only the run knows: the spec root and the flow.
        let mut args: Vec<String> = args.into_iter().map(|a| if a == "<root-id>" { "root".to_string() } else { a }).collect();
        if args.starts_with(&["scenario".to_string(), "new".to_string()]) && !args.iter().any(|a| a == "--flow") {
            args.extend(["--flow".to_string(), "F1".to_string()]);
        }
        if args[0] == "evidence" {
            args.push("--promote".to_string());
        }
        ran.push(format!("canon {}", args.join(" ")));
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        last = text(repo, &refs);
    }
    println!("{}", ran.join("\n"));
    let status = status_report(repo, None);
    let demo = status["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "demo").unwrap();
    assert_eq!(demo["status"], "shipped", "ran:\n{}\n{status}", ran.join("\n"));
    assert_eq!(next_line(&last), None, "a shipped subject has no step: {last}");
    assert_eq!(status["next"], serde_json::json!([]), "{status}");
    assert!(status_text(repo, None).is_empty());
}

/// Strict repo where scenario 01 is evidenced and awaits review while 02
/// has no evidence: a reviewer step and an implementer step are open.
fn two_roles_waiting() -> TempDir {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    ok(repo, &["brief", "approve", "--approver", "Ana", "--note", "Go.", "--json"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--case", "happy", "--flow", "F1", "--json"]);
    ok(repo, &["scenario", "new", "auth.login.02", "--title", "Wrong password", "--subject", "demo", "--case", "failure", "--flow", "F1", "--json"]);
    ok(repo, &["subject", "move", "demo", "--to", "specced", "--json"]);
    ok(repo, &["subject", "move", "demo", "--to", "building", "--json"]);
    ok(
        repo,
        &["evidence", "add", "--scenario", "auth.login.01", "--project", "root", "--class", "test-run", "--role", "implementer", "--actor", "agent-a", "--session", "s1", "--verdict", "faithful", "--ref", "cargo test", "--promote", "--json"],
    );
    dir
}

#[test]
fn a_declared_role_keeps_its_steps_and_counts_the_rest() {
    let dir = two_roles_waiting();
    let repo = dir.path();

    // Both open steps hold `<placeholder>` words, so no entry is in `meta.next`;
    // the human form lists them. No declared role, no counts.
    let unset = status_report(repo, None);
    assert!(unset.get("waitingOn").is_none(), "no role, no counts");
    assert_eq!(unset["next"], serde_json::json!([]), "{unset}");
    let commands: Vec<String> = status_text(repo, None).iter().map(|s| s["command"].as_str().unwrap().to_string()).collect();
    assert!(commands.iter().any(|c| c.starts_with("canon review scenario")) && commands.iter().any(|c| c.starts_with("canon evidence add")), "{commands:?}");

    let reviewer = status_report(repo, Some("reviewer"));
    let steps = status_text(repo, Some("reviewer"));
    assert!(!steps.is_empty() && steps.iter().all(|s| s["command"].as_str().unwrap().starts_with("canon review scenario")), "{steps:?}");
    assert_eq!(reviewer["waitingOn"], serde_json::json!({"implementer": 1}), "{reviewer}");
    let implementer = status_report(repo, Some("implementer"));
    let steps = status_text(repo, Some("implementer"));
    assert!(!steps.is_empty() && steps.iter().all(|s| s["command"].as_str().unwrap().starts_with("canon evidence add")), "{steps:?}");
    assert_eq!(implementer["waitingOn"], serde_json::json!({"reviewer": 1}), "{implementer}");

    let text = canon(repo, &["status"], Some("reviewer"));
    assert!(String::from_utf8_lossy(&text.stdout).contains("waiting on implementer: 1"));

    // A write under a role gets the same filter: the count is in the report's `data`
    // (never in `meta`), the human form ends with the role's `next:` line and the count.
    let wrote = json_of(&canon(repo, &["scenario", "new", "auth.login.03", "--title", "Edge", "--subject", "demo", "--case", "edge", "--flow", "F1", "--json"], Some("reviewer")), "scenario new as reviewer");
    assert_eq!(wrote["data"]["waitingOn"], serde_json::json!({"implementer": 1}), "{wrote}");
    assert!(wrote["meta"].get("waitingOn").is_none() && wrote["meta"].get("next").is_none(), "{wrote}");
    let out = canon(repo, &["scenario", "new", "auth.login.04", "--title", "Edge two", "--subject", "demo", "--case", "edge", "--flow", "F1"], Some("reviewer"));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[lines.len() - 2].starts_with("next: canon review scenario "), "{text}");
    assert_eq!(lines[lines.len() - 1], "waiting on implementer: 1", "{text}");
}

/// [`brief`] with a second flow step, `F2`.
fn brief_with_two_flows() -> String {
    brief()
        .replace("lands on the catalog. ==**Only a valid sign-in opens a session.**==\n\n", "lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor signs out → the session ends. ==**A signed-out cart stays saved.**==\n\n")
        .replace("The password is never logged.**==\n\n", "The password is never logged.**==\n\n### F2 Sign out\n\nThe server ends the session. ==**The cookie is cleared.**==\n\n")
        .replace("set a session cookie.\n\n", "set a session cookie.\n- F2: signing out clears the cookie.\n\n")
}

/// The step `status` names for `demo` that carries `--flow <flow>`, as its
/// human form lists it (the step holds `<placeholder>` words, so `meta.next`
/// has no entry for it), under `role` when given.
fn flow_step_of(repo: &Path, flow: &str, role: Option<&str>) -> Option<Value> {
    status_text(repo, role).into_iter().find(|step| step["command"].as_str().unwrap_or("").contains(&format!("--flow {flow} ")))
}

/// The roles that carry `step`: it appears in `status` under that role.
fn roles_of(repo: &Path, flow: &str) -> Vec<&'static str> {
    ["planner", "implementer", "reviewer", "orchestrator"].into_iter().filter(|role| flow_step_of(repo, flow, Some(role)).is_some()).collect()
}

#[test]
fn a_flow_step_with_only_a_happy_scenario_is_asked_for_its_failure_case_with_its_flow() {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    std::fs::write(repo.join(".canon/changes/add-login/brief.md"), brief_with_two_flows()).unwrap();
    ok(repo, &["brief", "approve", "--approver", "Ana", "--note", "Go.", "--json"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--case", "happy", "--flow", "F1", "--json"]);
    ok(repo, &["scenario", "new", "auth.login.02", "--title", "Wrong password", "--subject", "demo", "--case", "failure", "--flow", "F1", "--json"]);

    // F2 has no scenario yet: the step is the happy case. Scenarios are the
    // implementer's (the planner reference writes none).
    let none = flow_step_of(repo, "F2", None).expect("a step for F2");
    let command = none["command"].as_str().unwrap();
    assert!(command.starts_with("canon scenario new auth.login.03 --subject demo --case happy --flow F2 --title "), "{none}");
    assert_eq!(roles_of(repo, "F2"), ["implementer"], "{none}");
    // Placeholders keep the step out of `meta.next`.
    assert!(status_report(repo, None)["next"].as_array().unwrap().iter().all(|entry| !entry["command"].as_str().unwrap().contains("--flow F2 ")));

    // F2 gains only its happy scenario: the same step now asks for the failure case,
    // and the write's own `next:` line is the step status names first.
    text_write_names_status_step(repo, &["scenario", "new", "auth.login.03", "--title", "Sign out", "--subject", "demo", "--case", "happy", "--flow", "F2"], "demo", "scenario new");
    let step = flow_step_of(repo, "F2", None).expect("status names F2's failure case");
    let command = step["command"].as_str().unwrap();
    assert!(command.starts_with("canon scenario new auth.login.04 --subject demo --case failure --flow F2 --title "), "{step}");
    assert_eq!(roles_of(repo, "F2"), ["implementer"], "{step}");

    // Once F2 has a failure scenario, no step asks for it again.
    ok(repo, &["scenario", "new", "auth.login.04", "--title", "Sign out twice", "--subject", "demo", "--case", "failure", "--flow", "F2", "--json"]);
    assert!(flow_step_of(repo, "F2", None).is_none(), "F2 is covered");
}

/// The fixture brief with its acceptance key sentence removed: no blockers,
/// one `prose.key-sentence` advisory.
fn brief_with_an_advisory() -> String {
    brief().replace(" ==**F1 is the happy path.**==", "")
}

/// The steps `status` gives about `subject`, as its human form lists them
/// (`{actor, command, why}`; the human's step is not a `meta.next` entry).
fn status_steps(repo: &Path, subject: &str) -> Vec<Value> {
    status_text(repo, None).into_iter().filter(|step| step["why"].as_str().unwrap_or("").starts_with(subject)).collect()
}

/// context.status.81: a brief with no blockers and advisories left gets the
/// planner's polish step first, naming the count and the rule ids, then the
/// human stop; a write's `next:` line says the same.
#[test]
fn context_status_81_the_polish_step_comes_before_the_human_stop() {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    std::fs::write(repo.join(".canon/changes/add-login/brief.md"), brief_with_an_advisory()).unwrap();

    let steps = status_steps(repo, "demo");
    assert_eq!(steps.len(), 2, "{steps:?}");
    assert_eq!(steps[0]["actor"], "agent", "{}", steps[0]);
    let command = steps[0]["command"].as_str().unwrap();
    assert!(command.contains("canon skills show brief-polish") && command.contains("`canon brief check add-login`"), "{command}");
    let why = steps[0]["why"].as_str().unwrap();
    assert!(why.contains("1 advisory finding(s)") && why.contains("prose.key-sentence"), "{why}");
    assert_eq!(steps[1]["actor"], "human", "{}", steps[1]);
    // The planner's step is the planner's; the human's is counted for the human.
    assert_eq!(status_steps_under(repo, "planner"), [steps[0].clone()]);
    assert_eq!(status_report(repo, Some("planner"))["waitingOn"], serde_json::json!({"human": 1}));

    let out = canon(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--case", "happy", "--flow", "F1"], None);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert_eq!(next_line(&text), Some((command.to_string(), why.to_string())), "the write names the polish step first");
    assert!(text.lines().last().unwrap().ends_with(" (+1 more)"), "and counts the human stop: {text}");

    // Blockers still win: only the check step, no polish and no human step.
    std::fs::write(repo.join(".canon/changes/add-login/brief.md"), brief_with_an_advisory().replace("## Out of scope\n\nSocial sign-in.", "## Out of scope\n\nTBD")).unwrap();
    let blocked = status_steps(repo, "demo");
    assert_eq!(blocked.len(), 1, "{blocked:?}");
    assert_eq!(blocked[0]["command"], "canon brief check add-login");
    // That step is a runnable command: the JSON form carries it as an entry.
    let entries = status_entries(repo, "demo");
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0]["command"], "canon brief check add-login --json");
}

/// `status_steps` under a declared `role`.
fn status_steps_under(repo: &Path, role: &str) -> Vec<Value> {
    status_text(repo, Some(role)).into_iter().filter(|step| step["why"].as_str().unwrap_or("").starts_with("demo")).collect()
}

/// context.status.83: with no advisories the human stop is the only step,
/// and `canon brief approve` records an approval whatever advisories remain.
#[test]
fn context_status_83_no_advisories_keeps_the_human_stop_and_approve_ignores_advisories() {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    let clean = status_steps(repo, "demo");
    assert_eq!(clean.len(), 1, "{clean:?}");
    assert_eq!(clean[0]["actor"], "human");

    std::fs::write(repo.join(".canon/changes/add-login/brief.md"), brief_with_an_advisory()).unwrap();
    assert_eq!(status_steps(repo, "demo").len(), 2);
    let approved = ok(repo, &["brief", "approve", "--approver", "Ana", "--note", "Go.", "--json"]);
    assert_eq!(approved["data"]["policyApproved"], true, "{approved}");
    let after = status_steps(repo, "demo");
    assert!(after.iter().all(|s| s["actor"] == "agent" && !s["command"].as_str().unwrap().contains("brief-polish")), "an approved brief is not polished: {after:?}");
}
