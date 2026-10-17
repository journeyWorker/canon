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

fn ok(repo: &Path, args: &[&str]) -> Value {
    json_of(&canon(repo, args, None), &format!("{args:?}"))
}

/// A complete brief with the one flow step `F1`.
fn brief() -> String {
    "# Add login\n\n\
<!-- canon:section intent role=context -->\n## Intent\n\nAdd login\n\nVisitors lose their cart when they switch devices.\n\n\
<!-- canon:section outcome role=normative -->\n## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
<!-- canon:section flow role=normative -->\n## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n\n\
<!-- canon:section detail role=normative -->\n## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n\
<!-- canon:section acceptance role=normative -->\n## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie.\n\n\
<!-- canon:section questions role=question -->\n## Questions and assumptions\n\nNone.\n\n\
<!-- canon:section out-of-scope role=context -->\n## Out of scope\n\nSocial sign-in.\n"
        .to_string()
}

/// `init` (strict policy: evidence, a failure case and an independent
/// review), subject `demo`, and change `add-login` with a clean brief.
fn repo_with_change() -> (TempDir, Value) {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none", "--json"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo", "--json"]);
    let created = ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login", "--json"]);
    std::fs::write(repo.join(".canon/changes/add-login/brief.md"), brief()).unwrap();
    (dir, created)
}

/// `status --json`'s first step about `subject`.
fn status_step(repo: &Path, subject: &str) -> Value {
    let report = json_of(&canon(repo, &["status", "--json"], None), "status");
    report["next"].as_array().unwrap().iter().find(|step| step["why"].as_str().unwrap_or("").starts_with(subject)).cloned().unwrap_or_else(|| panic!("status names no step for {subject}: {}", report["next"]))
}

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
    // subject's step is the planner's, not the human's (0.22 D4).
    let steps = meta_next(&created);
    assert_eq!(steps[0]["actor"], "agent");
    assert_eq!(steps[0]["role"], "planner");
    assert_eq!(steps[0]["command"], "canon brief check --change add-login");
    assert!(steps[0]["why"].as_str().unwrap().contains("blocker(s): write the brief"), "{}", steps[0]);

    // Once the brief is written clean, the one human stop is the subject's only step.
    let step = status_step(repo, "demo");
    assert_eq!(step["actor"], "human");
    assert!(step["role"].is_null());

    let approved = ok(repo, &["brief", "approve", "--by", "Ana", "--note", "Go.", "--json"]);
    assert_eq!(meta_next(&approved)[0], status_step(repo, "demo"), "brief approve");
    assert_eq!(meta_next(&approved)[0]["actor"], "agent");

    let scenario = ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--case", "happy", "--flow", "F1", "--json"]);
    assert_eq!(meta_next(&scenario)[0], status_step(repo, "demo"), "scenario new");

    let moved = ok(repo, &["subject", "status", "demo", "--to", "specced", "--json"]);
    assert_eq!(meta_next(&moved)[0], status_step(repo, "demo"), "subject status");

    ok(repo, &["subject", "status", "demo", "--to", "building", "--json"]);
    let evidence = ok(
        repo,
        &["evidence", "add", "--scenario-id", "auth.login.01", "--project-id", "root", "--kind", "test-run", "--role", "implementer", "--actor-id", "agent-a", "--session-id", "s1", "--verdict", "faithful", "--ref", "cargo test", "--promote", "--json"],
    );
    assert_eq!(meta_next(&evidence)[0], status_step(repo, "demo"), "evidence add");

    let review = ok(
        repo,
        &["review", "add", "--project-id", "root", "--scenario-id", "auth.login.01", "--reviewer", "rev", "--actor-id", "rev", "--session-id", "s2", "--role", "reviewer", "--pin", "0123456789abcdef0123456789abcdef01234567", "--original-spec-ref", "login.feature", "--json"],
    );
    assert_eq!(meta_next(&review)[0], status_step(repo, "demo"), "review add");
}

#[test]
fn a_staged_write_names_the_promote_that_makes_it_count() {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    ok(repo, &["brief", "approve", "--by", "Ana", "--note", "Go.", "--json"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--case", "happy", "--flow", "F1", "--json"]);
    let staged = ok(
        repo,
        &["evidence", "add", "--scenario-id", "auth.login.01", "--project-id", "root", "--kind", "test-run", "--role", "implementer", "--actor-id", "agent-a", "--session-id", "s1", "--verdict", "faithful", "--ref", "cargo test", "--no-promote", "--json"],
    );
    let steps = meta_next(&staged);
    assert_eq!(steps.len(), 1, "{steps:?}");
    assert_eq!(steps[0]["command"], "canon gate promote");
}

#[test]
fn human_mode_ends_with_one_next_line_and_a_count_of_the_rest() {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    let out = canon(repo, &["brief", "approve", "--by", "Ana", "--note", "Go."], None);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    let last = text.lines().last().unwrap();
    assert!(last.starts_with("next: canon scenario new "), "{text}");
    assert!(last.contains(" — "), "{last}");
}

#[test]
fn one_scenario_goes_from_its_brief_to_shipped_on_meta_next_alone() {
    let (dir, created) = repo_with_change();
    let repo = dir.path();
    // The human answers; everything after that is the next step each write names.
    let mut last = ok(repo, &["brief", "approve", "--by", "Ana", "--note", "Go.", "--json"]);
    let _ = created;
    let mut ran = vec!["canon brief approve --by Ana --note Go.".to_string()];
    for _ in 0..24 {
        let steps = meta_next(&last);
        let Some(step) = steps.first() else { break };
        assert_eq!(step["actor"], "agent", "{step}");
        let command = step["command"].as_str().unwrap();
        let mut args = words(command);
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
        args.push("--json".to_string());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        last = ok(repo, &refs);
    }
    println!("{}", ran.join("\n"));
    let status = json_of(&canon(repo, &["status", "--json"], None), "status");
    let demo = status["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "demo").unwrap();
    assert_eq!(demo["status"], "shipped", "ran:\n{}\n{status}", ran.join("\n"));
    assert!(meta_next(&last).is_empty(), "a shipped subject has no step: {last}");
}

/// Strict repo where scenario 01 is evidenced and awaits review while 02
/// has no evidence: a reviewer step and an implementer step are open.
fn two_roles_waiting() -> TempDir {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    ok(repo, &["brief", "approve", "--by", "Ana", "--note", "Go.", "--json"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--case", "happy", "--flow", "F1", "--json"]);
    ok(repo, &["scenario", "new", "auth.login.02", "--title", "Wrong password", "--subject", "demo", "--case", "failure", "--flow", "F1", "--json"]);
    ok(repo, &["subject", "status", "demo", "--to", "specced", "--json"]);
    ok(repo, &["subject", "status", "demo", "--to", "building", "--json"]);
    ok(
        repo,
        &["evidence", "add", "--scenario-id", "auth.login.01", "--project-id", "root", "--kind", "test-run", "--role", "implementer", "--actor-id", "agent-a", "--session-id", "s1", "--verdict", "faithful", "--ref", "cargo test", "--promote", "--json"],
    );
    dir
}

#[test]
fn a_declared_role_keeps_its_steps_and_counts_the_rest() {
    let dir = two_roles_waiting();
    let repo = dir.path();

    let unset = json_of(&canon(repo, &["status", "--json"], None), "status");
    assert!(unset.get("waitingOn").is_none(), "no role, no counts");
    assert!(unset["next"].as_array().unwrap().iter().all(|s| s.get("role").is_some()), "every step carries its role");
    let roles: Vec<&str> = unset["next"].as_array().unwrap().iter().filter_map(|s| s["role"].as_str()).collect();
    assert!(roles.contains(&"reviewer") && roles.contains(&"implementer"), "{roles:?}");

    let reviewer = json_of(&canon(repo, &["status", "--json"], Some("reviewer")), "status as reviewer");
    let steps = reviewer["next"].as_array().unwrap();
    assert!(!steps.is_empty() && steps.iter().all(|s| s["role"] == "reviewer"), "{steps:?}");
    assert!(steps[0]["command"].as_str().unwrap().starts_with("canon review add"), "{steps:?}");
    assert_eq!(reviewer["waitingOn"]["implementer"], 1, "{reviewer}");

    let text = canon(repo, &["status"], Some("reviewer"));
    assert!(String::from_utf8_lossy(&text.stdout).contains("waiting on implementer: 1"));

    // A write under a role gets the same filter in `meta.next`.
    let wrote = json_of(&canon(repo, &["scenario", "new", "auth.login.03", "--title", "Edge", "--subject", "demo", "--case", "edge", "--flow", "F1", "--json"], Some("reviewer")), "scenario new as reviewer");
    let steps = meta_next(&wrote);
    assert!(!steps.is_empty() && steps.iter().all(|s| s["role"] == "reviewer"), "{steps:?}");
    assert_eq!(wrote["meta"]["waitingOn"]["implementer"], 1, "{wrote}");
}

/// [`brief`] with a second flow step, `F2`.
fn brief_with_two_flows() -> String {
    brief()
        .replace("lands on the catalog. ==**Only a valid sign-in opens a session.**==\n\n", "lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor signs out → the session ends. ==**A signed-out cart stays saved.**==\n\n")
        .replace("The password is never logged.**==\n\n", "The password is never logged.**==\n\n### F2 Sign out\n\nThe server ends the session. ==**The cookie is cleared.**==\n\n")
        .replace("set a session cookie.\n\n", "set a session cookie.\n- F2: signing out clears the cookie.\n\n")
}

/// The step `status` names for `demo` that carries `--flow <flow>`.
fn flow_step_of(repo: &Path, flow: &str) -> Option<Value> {
    let report = json_of(&canon(repo, &["status", "--json"], None), "status");
    report["next"].as_array().unwrap().iter().find(|step| step["command"].as_str().unwrap_or("").contains(&format!("--flow {flow} "))).cloned()
}

#[test]
fn a_flow_step_with_only_a_happy_scenario_is_asked_for_its_failure_case_with_its_flow() {
    let (dir, _) = repo_with_change();
    let repo = dir.path();
    std::fs::write(repo.join(".canon/changes/add-login/brief.md"), brief_with_two_flows()).unwrap();
    ok(repo, &["brief", "approve", "--by", "Ana", "--note", "Go.", "--json"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--subject", "demo", "--case", "happy", "--flow", "F1", "--json"]);
    ok(repo, &["scenario", "new", "auth.login.02", "--title", "Wrong password", "--subject", "demo", "--case", "failure", "--flow", "F1", "--json"]);

    // F2 has no scenario yet: the step is the happy case. Scenarios are the
    // implementer's (the planner reference writes none).
    let none = flow_step_of(repo, "F2").expect("a step for F2");
    let command = none["command"].as_str().unwrap();
    assert!(command.starts_with("canon scenario new auth.login.03 --subject demo --case happy --flow F2 --title "), "{none}");
    assert_eq!(none["role"], "implementer", "{none}");

    // F2 gains only its happy scenario: the same step now asks for the failure case.
    let wrote = ok(repo, &["scenario", "new", "auth.login.03", "--title", "Sign out", "--subject", "demo", "--case", "happy", "--flow", "F2", "--json"]);
    for step in [flow_step_of(repo, "F2").expect("status names F2's failure case"), meta_next(&wrote).into_iter().find(|s| s["command"].as_str().unwrap().contains("--flow F2 ")).expect("meta.next names it too")] {
        let command = step["command"].as_str().unwrap();
        assert!(command.starts_with("canon scenario new auth.login.04 --subject demo --case failure --flow F2 --title "), "{step}");
        assert_eq!(step["role"], "implementer", "{step}");
    }

    // Once F2 has a failure scenario, no step asks for it again.
    ok(repo, &["scenario", "new", "auth.login.04", "--title", "Sign out twice", "--subject", "demo", "--case", "failure", "--flow", "F2", "--json"]);
    assert!(flow_step_of(repo, "F2").is_none(), "F2 is covered");
}
