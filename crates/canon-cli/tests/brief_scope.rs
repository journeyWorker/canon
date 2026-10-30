//! Brief approval scoped to what a change adds (0.25 D1, #96), against the
//! built binary: a change adopted into a subject that is already building
//! or verifying freezes only the scenarios it names with `@flow:`, never
//! the scenarios an approved scope already covers; `canon subject unadopt`
//! takes an unapproved adoption back.

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

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(args)
        .current_dir(repo)
        .env_remove("CANON_ROLE")
        .output()
        .expect("spawn canon binary")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = run(repo, args);
    assert!(out.status.success(), "{args:?}: {}", stderr(&out));
    out
}

/// A complete English brief, so `canon brief approve` accepts it.
fn brief(extra: &str) -> String {
    format!(
        "# Change\n\n\
## Intent\n\nChange\n\nVisitors lose their cart when they switch devices.\n\n\
## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n\n\
## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n\
## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie. ==**F1 is the happy path.**==\n\n\
## Questions and assumptions\n\nNone.\n\n\
## Out of scope\n\nSocial sign-in.\n\n{extra}"
    )
}

fn brief_path(repo: &Path, change: &str) -> std::path::PathBuf {
    repo.join(format!(".canon/changes/{change}/brief.md"))
}

/// A repo whose subject `s` is `status` (reached through `specced` and
/// `building`), with no change yet.
fn repo_with_subject(status: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    git_seed(dir.path());
    let repo = dir.path();
    ok(
        repo,
        &[
            "init",
            "--providers",
            "none",
            "--no-agents-md",
            "--no-policy",
        ],
    );
    ok(
        repo,
        &["subject", "new", "s", "--domain", "dev", "--title", "S"],
    );
    ok(
        repo,
        &[
            "feature",
            "new",
            "shop.cart",
            "--title",
            "Cart",
            "--actor",
            "u",
        ],
    );
    for step in ["specced", "building", "verifying"] {
        if status == "proposed" {
            break;
        }
        ok(repo, &["subject", "move", "s", "--to", step]);
        if step == status {
            break;
        }
    }
    dir
}

/// Writes `change` with a complete brief, approved by the human when `approve`.
fn change(repo: &Path, change: &str, approve: bool) -> Output {
    let out = ok(
        repo,
        &[
            "change",
            "new",
            change,
            "--subject",
            "s",
            "--title",
            change,
        ],
    );
    std::fs::write(brief_path(repo, change), brief("")).unwrap();
    if approve {
        ok(
            repo,
            &[
                "brief",
                "approve",
                change,
                "--approver",
                "Ana",
                "--note",
                "Go",
            ],
        );
    }
    out
}

/// A scenario of `s`, tagged `@flow:<flow>` when given.
fn scenario(repo: &Path, id: &str, flow: Option<&str>) {
    ok(
        repo,
        &[
            "scenario",
            "new",
            id,
            "--title",
            id,
            "--subject",
            "s",
            "--case",
            "happy",
        ],
    );
    if let Some(flow) = flow {
        let path = repo.join(".canon/specs/kind=feature/area=shop/cart.feature");
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            text.replace(&format!("  @{id}\n"), &format!("  @flow:{flow}\n  @{id}\n")),
        )
        .unwrap();
    }
}

fn attest(repo: &Path, scenario: &str, reference: &str) -> Output {
    run(
        repo,
        &[
            "evidence",
            "add",
            "--project",
            "root",
            "--scenario",
            scenario,
            "--class",
            "test-run",
            "--ref",
            reference,
            "--role",
            "implementer",
            "--actor",
            "impl",
            "--promote",
        ],
    )
}

fn attest_task(repo: &Path, task: &str) -> Output {
    run(
        repo,
        &[
            "evidence",
            "add",
            "--task",
            task,
            "--class",
            "test-run",
            "--ref",
            "cargo test",
            "--role",
            "implementer",
            "--actor",
            "impl",
        ],
    )
}

/// evidence.add.100: a draft change adopted into a verifying subject
/// freezes only the scenarios tagged with it.
#[test]
fn evidence_add_100_a_draft_change_freezes_only_the_scenarios_it_is_tagged_on() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    change(repo, "a", true);
    let tasks = repo.join(".canon/changes/a/tasks.md");
    let mut text = std::fs::read_to_string(&tasks).unwrap_or_else(|_| "# Tasks: A\n".to_string());
    text.push_str("\n- [ ] 1 Build it\n");
    std::fs::write(&tasks, text).unwrap();
    ok(repo, &["ingest", "plans"]);
    scenario(repo, "shop.cart.01", None);
    scenario(repo, "shop.cart.02", Some("a.F1"));
    for id in ["shop.cart.01", "shop.cart.02"] {
        let out = attest(repo, id, "first run");
        assert!(out.status.success(), "{id}: {}", stderr(&out));
    }

    let adopted = change(repo, "b", false);
    let err = stderr(&adopted);
    assert!(
        err.contains("warning:")
            && err.contains("canon subject unadopt s --change b")
            && err.contains("@flow:b.*"),
        "{err}"
    );

    // The approved scope keeps accepting evidence, and so does A's task.
    for id in ["shop.cart.01", "shop.cart.02"] {
        let out = attest(repo, id, "second run");
        assert!(out.status.success(), "{id}: {}", stderr(&out));
    }
    let task = attest_task(repo, "a#1");
    assert!(task.status.success(), "{}", stderr(&task));

    // A scenario B adds waits for B's approval, naming B's brief.
    scenario(repo, "shop.cart.03", Some("b.F1"));
    let out = attest(repo, "shop.cart.03", "first run");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("BRIEF_UNAPPROVED")
            && stderr(&out).contains(".canon/changes/b/brief.md has no `Approved:` line"),
        "{}",
        stderr(&out)
    );
    // ... and the ship guard still asks for the newest adopted change's approval.
    let ship = run(repo, &["subject", "move", "s", "--to", "shipped"]);
    assert_eq!(ship.status.code(), Some(1), "{}", stderr(&ship));
    assert!(
        stderr(&ship).contains(".canon/changes/b/brief.md"),
        "{}",
        stderr(&ship)
    );

    // Approving B lifts the wait.
    ok(
        repo,
        &[
            "brief",
            "approve",
                "b",
            "--approver",
            "Ana",
            "--note",
            "Go",
        ],
    );
    let out = attest(repo, "shop.cart.03", "first run");
    assert!(out.status.success(), "{}", stderr(&out));
}

/// evidence.add.101: a subject whose only change is unapproved refuses
/// every scenario, and an approved brief edited since still refuses an
/// untagged one with the changed-since-approval code.
#[test]
fn evidence_add_101_an_unapproved_only_change_and_an_edited_approval_still_refuse() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    change(repo, "a", false);
    scenario(repo, "shop.cart.01", None);
    scenario(repo, "shop.cart.02", Some("a.F1"));
    for id in ["shop.cart.01", "shop.cart.02"] {
        let out = attest(repo, id, "run");
        assert_eq!(out.status.code(), Some(1), "{id}: {}", stderr(&out));
        assert!(
            stderr(&out).contains("BRIEF_UNAPPROVED")
                && stderr(&out).contains(".canon/changes/a/brief.md"),
            "{id}: {}",
            stderr(&out)
        );
    }

    ok(
        repo,
        &[
            "brief",
            "approve",
                "a",
            "--approver",
            "Ana",
            "--note",
            "Go",
        ],
    );
    assert!(attest(repo, "shop.cart.01", "run").status.success());
    std::fs::write(
        brief_path(repo, "a"),
        std::fs::read_to_string(brief_path(repo, "a"))
            .unwrap()
            .replace("Social sign-in.", "Social sign-in, later."),
    )
    .unwrap();
    change(repo, "b", false);
    let out = attest(repo, "shop.cart.01", "second run");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("BRIEF_CHANGED_SINCE_APPROVAL")
            && stderr(&out).contains(".canon/changes/a/brief.md"),
        "{}",
        stderr(&out)
    );
}

/// evidence.add.102: a subject built before briefs existed keeps
/// re-verifying its evidenced scenarios when it adopts its first, draft
/// change; a scenario never evidenced still waits.
#[test]
fn evidence_add_102_a_legacy_subject_keeps_re_verifying_its_evidenced_scenarios() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    scenario(repo, "shop.cart.01", None);
    scenario(repo, "shop.cart.02", None);
    let first = attest(repo, "shop.cart.01", "before briefs");
    assert!(first.status.success(), "{}", stderr(&first));

    let adopted = change(repo, "a", false);
    assert!(
        stderr(&adopted).contains("warning:"),
        "{}",
        stderr(&adopted)
    );
    let again = attest(repo, "shop.cart.01", "re-verified");
    assert!(again.status.success(), "{}", stderr(&again));
    let never = attest(repo, "shop.cart.02", "new");
    assert_eq!(never.status.code(), Some(1), "{}", stderr(&never));
    assert!(
        stderr(&never).contains("BRIEF_UNAPPROVED"),
        "{}",
        stderr(&never)
    );
}

fn warnings(out: &Output) -> Vec<String> {
    let json: Value = serde_json::from_str(&stdout(out)).unwrap();
    json["meta"]["warnings"]
        .as_array()
        .map(|w| w.iter().map(|w| w.as_str().unwrap().to_string()).collect())
        .unwrap_or_default()
}

/// subject.lifecycle.35: adopting an unapproved change into a building,
/// verifying or shipped subject warns once; nothing else does.
#[test]
fn subject_lifecycle_35_adopting_an_unapproved_change_warns_only_where_it_matters() {
    for status in ["proposed", "specced"] {
        let dir = repo_with_subject(status);
        let out = ok(
            dir.path(),
            &[
                "change",
                "new",
                "a",
                "--subject",
                "s",
                "--title",
                "A",
                "--json",
            ],
        );
        assert!(
            warnings(&out).is_empty() && !stderr(&out).contains("warning:"),
            "{status}: {}",
            stderr(&out)
        );
    }
    let dir = repo_with_subject("building");
    let repo = dir.path();
    let out = ok(
        repo,
        &[
            "change",
            "new",
            "a",
            "--subject",
            "s",
            "--title",
            "A",
            "--json",
        ],
    );
    let found = warnings(&out);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("canon subject unadopt s --change a")
            && found[0].contains("cannot ship"),
        "{found:?}"
    );
    assert!(stderr(&out).is_empty(), "JSON mode writes nothing to stderr: {}", stderr(&out));

    // `subject adopt` warns the same way; an approved brief does not.
    let other = repo_with_subject("verifying");
    let repo2 = other.path();
    ok(
        repo2,
        &[
            "subject", "new", "holding", "--domain", "dev", "--title", "Holding",
        ],
    );
    ok(
        repo2,
        &[
            "change",
            "new",
            "c",
            "--subject",
            "holding",
            "--title",
            "C",
        ],
    );
    let out = ok(
        repo2,
        &["subject", "adopt", "s", "--change", "c", "--json"],
    );
    assert_eq!(warnings(&out).len(), 1, "{}", stdout(&out));
    ok(repo2, &["subject", "unadopt", "s", "--change", "c"]);
    std::fs::write(brief_path(repo2, "c"), brief("")).unwrap();
    ok(
        repo2,
        &[
            "brief",
            "approve",
                "c",
            "--approver",
            "Ana",
            "--note",
            "Go",
        ],
    );
    let out = ok(
        repo2,
        &["subject", "adopt", "s", "--change", "c", "--json"],
    );
    assert!(warnings(&out).is_empty(), "{}", stdout(&out));
}

/// subject.lifecycle.36: unadopt removes the link on both sides, status
/// stops asking about the change for the subject, another subject can
/// adopt it, and an unlinked pair is a no-op.
#[test]
fn subject_lifecycle_36_unadopt_removes_the_link_on_both_sides() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    ok(
        repo,
        &[
            "subject", "new", "holding", "--domain", "dev", "--title", "Holding",
        ],
    );
    change(repo, "a", true);
    change(repo, "b", false);
    assert!(
        stdout(&run(repo, &["status"])).contains(".canon/changes/b/brief.md"),
        "{}",
        stdout(&run(repo, &["status"]))
    );

    let out = ok(
        repo,
        &["subject", "unadopt", "s", "--change", "b", "--json"],
    );
    let json: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(json["meta"]["command"], "subject unadopt");
    assert!(json["meta"]["next"].is_array(), "{json}");
    let records = |kind: &str| -> Vec<Value> {
        serde_json::from_str::<Value>(&stdout(&ok(repo, &["query", "--kind", kind, "--json"])))
            .unwrap()["data"]["records"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    };
    let subject = records("subject")
        .into_iter()
        .find(|r| r["subject_id"] == "s")
        .unwrap();
    assert_eq!(subject["change_ids"], serde_json::json!(["a"]), "{subject}");
    let change_b = records("change")
        .into_iter()
        .find(|r| r["change_id"] == "b")
        .unwrap();
    assert!(
        change_b["subject_ids"]
            .as_array()
            .is_none_or(std::vec::Vec::is_empty),
        "{change_b}"
    );
    let status = stdout(&run(repo, &["status"]));
    assert!(!status.contains("changes/b/brief.md"), "{status}");

    // The change stays: a holding subject adopts it.
    ok(repo, &["subject", "adopt", "holding", "--change", "b"]);
    // An unlinked pair is a no-op.
    let again = ok(repo, &["subject", "unadopt", "s", "--change", "b"]);
    assert!(
        stdout(&again).contains("not linked") && stdout(&again).contains("nothing written"),
        "{}",
        stdout(&again)
    );
}

/// subject.lifecycle.37: a change whose brief carries an `Approved:` line
/// is the subject's approved scope and is not unadopted.
#[test]
fn subject_lifecycle_37_unadopt_refuses_an_approved_change() {
    let dir = repo_with_subject("building");
    let repo = dir.path();
    change(repo, "a", true);
    let out = run(
        repo,
        &["subject", "unadopt", "s", "--change", "a", "--json"],
    );
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    let json: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(json["error"]["code"], "SUBJECT_UNADOPT_REFUSED", "{json}");
    assert!(
        json["error"]["hint"]
            .as_str()
            .is_some_and(|h| h.contains("approved scope")),
        "{json}"
    );
    let status = stdout(&run(repo, &["subject", "show", "s", "--json"]));
    assert!(status.contains("\"a\""), "the link stays: {status}");
}

/// evidence.add.100: a `@flow:` tag counts only for a change the
/// scenario's owning subject adopted. A tag naming a change that does not
/// exist, or one adopted only by another subject, is ignored, so the
/// scenario follows the untagged rule and is refused on a draft-only subject.
#[test]
fn evidence_add_100_a_tag_naming_a_change_the_subject_did_not_adopt_is_ignored() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    change(repo, "a", false);
    ok(
        repo,
        &["subject", "new", "other", "--domain", "dev", "--title", "Other"],
    );
    ok(
        repo,
        &["change", "new", "x", "--subject", "other", "--title", "X"],
    );
    std::fs::write(brief_path(repo, "x"), brief("")).unwrap();
    ok(
        repo,
        &["brief", "approve",
                "x", "--approver", "Ana", "--note", "Go"],
    );
    scenario(repo, "shop.cart.01", Some("zzz.F1"));
    scenario(repo, "shop.cart.02", Some("x.F1"));
    scenario(repo, "shop.cart.03", Some("a.F1"));
    for id in ["shop.cart.01", "shop.cart.02", "shop.cart.03"] {
        let out = attest(repo, id, "run");
        assert_eq!(out.status.code(), Some(1), "{id}: {}", stderr(&out));
        assert!(
            stderr(&out).contains("BRIEF_UNAPPROVED")
                && stderr(&out).contains(".canon/changes/a/brief.md"),
            "{id}: {}",
            stderr(&out)
        );
    }
}

/// evidence.add.101: an evidence record attested under `--override-reason`
/// never becomes the standing 'already evidenced' exemption: the next add
/// of that scenario is refused again without the override.
#[test]
fn evidence_add_101_an_override_attestation_does_not_exempt_the_scenario_later() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    change(repo, "a", false);
    scenario(repo, "shop.cart.01", None);
    let refused = attest(repo, "shop.cart.01", "run");
    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
    let waived = run(
        repo,
        &[
            "evidence", "add", "--project", "root", "--scenario", "shop.cart.01",
            "--class", "test-run", "--ref", "waived run", "--role", "implementer",
            "--actor", "impl", "--promote", "--override-reason", "urgent fix",
        ],
    );
    assert!(waived.status.success(), "{}", stderr(&waived));
    let again = attest(repo, "shop.cart.01", "run again");
    assert_eq!(again.status.code(), Some(1), "{}", stderr(&again));
    assert!(stderr(&again).contains("BRIEF_UNAPPROVED"), "{}", stderr(&again));
}

/// subject.lifecycle.37: a change a scenario of the subject is tagged
/// `@flow:<change>.*` for is not unadopted either: the scenario would fall
/// back to an older approval. The hint names the scenarios.
#[test]
fn subject_lifecycle_37_unadopt_refuses_a_change_a_scenario_is_tagged_for() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    change(repo, "b", false);
    scenario(repo, "shop.cart.01", Some("b.F1"));
    ok(repo, &["inventory", "sync"]);
    let out = run(
        repo,
        &["subject", "unadopt", "s", "--change", "b", "--json"],
    );
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    let json: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(json["error"]["code"], "SUBJECT_UNADOPT_REFUSED", "{json}");
    let text = json.to_string();
    assert!(
        text.contains("shop.cart.01") && text.contains("remove or retag"),
        "{json}"
    );
    assert!(stdout(&run(repo, &["subject", "show", "s", "--json"])).contains("\"b\""));

    // Retagged away, the unadopt goes through.
    let path = repo.join(".canon/specs/kind=feature/area=shop/cart.feature");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("  @flow:b.F1\n", "")).unwrap();
    ok(repo, &["inventory", "sync"]);
    ok(repo, &["subject", "unadopt", "s", "--change", "b"]);
}

/// subject.lifecycle.35: the shipped arm of the adoption warning.
#[test]
fn subject_lifecycle_35_adopting_an_unapproved_change_into_a_shipped_subject_warns() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    scenario(repo, "shop.cart.01", None);
    assert!(attest(repo, "shop.cart.01", "run").status.success());
    ok(repo, &["subject", "move", "s", "--to", "shipped"]);
    let out = ok(
        repo,
        &["change", "new", "a", "--subject", "s", "--title", "A", "--json"],
    );
    let found = warnings(&out);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("is shipped")
            && found[0].contains("can still be reopened and shipped for its approved scope")
            && found[0].contains("canon subject unadopt s --change a"),
        "{found:?}"
    );
}

/// subject.lifecycle.38: on a shipped subject a newest adopted draft change
/// no scenario is tagged for is pending: reopen, re-verification and ship
/// read the approved scope, and status still lists the draft. Tagging a
/// scenario for the draft brings its approval back into every rule.
#[test]
fn subject_lifecycle_38_a_pending_draft_does_not_freeze_a_shipped_subject() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    change(repo, "a", true);
    scenario(repo, "shop.cart.01", Some("a.F1"));
    scenario(repo, "shop.cart.03", Some("a.F1"));
    let path = repo.join(".canon/specs/kind=feature/area=shop/cart.feature");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("@case:happy\n  @flow:a.F1\n  @shop.cart.03", "@case:failure\n  @flow:a.F1\n  @shop.cart.03"),
    )
    .unwrap();
    for id in ["shop.cart.01", "shop.cart.03"] {
        let first = attest(repo, id, "first run");
        assert!(first.status.success(), "{id}: {}", stderr(&first));
    }
    let shipped = run(repo, &["subject", "move", "s", "--to", "shipped"]);
    assert!(shipped.status.success(), "{}", stderr(&shipped));

    let adopted = change(repo, "b", false);
    assert!(stderr(&adopted).contains("warning:"), "{}", stderr(&adopted));

    let reopen = run(
        repo,
        &["subject", "move", "s", "--to", "verifying", "--reason", "fix the cart"],
    );
    assert!(reopen.status.success(), "{}", stderr(&reopen));
    assert!(
        stderr(&reopen).contains("`b` is a pending draft (never approved, not a chore, no scenario tagged `@flow:b.*`): not part of this transition"),
        "{}",
        stderr(&reopen)
    );
    let again = attest(repo, "shop.cart.01", "second run");
    assert!(again.status.success(), "{}", stderr(&again));
    let ship = run(repo, &["subject", "move", "s", "--to", "shipped", "--json"]);
    assert!(ship.status.success(), "{}", stderr(&ship));
    let shipped: Value = serde_json::from_str(&stdout(&ship)).unwrap();
    assert!(
        shipped["meta"]["warnings"].to_string().contains("`b` is a pending draft"),
        "{shipped}"
    );

    // Status still shows the draft as awaiting the human, and as not yet
    // part of the subject's scope.
    let status: Value = serde_json::from_str(&stdout(&ok(repo, &["status", "--json"]))).unwrap();
    let text = status.to_string();
    assert!(text.contains("\"pendingDrafts\":[\"b\"]"), "{text}");
    let briefs = status["data"]["subjects"][0]["briefs"].to_string();
    assert!(!briefs.contains("\"change\":\"b\""), "a pending draft is shown, never judged: {briefs}");
    let human = stdout(&ok(repo, &["status"]));
    assert!(human.contains("`b` is a pending draft: not yet part of the subject's scope"), "{human}");
    // The guidance says what is refused: only what answers to the draft.
    assert!(!text.contains("refuses s's scenarios and tasks"), "{text}");
    assert!(!text.contains("build and attest nothing for s"), "{text}");

    // Once a scenario is tagged for the draft, shipping needs its approval.
    ok(
        repo,
        &["subject", "move", "s", "--to", "verifying", "--reason", "more"],
    );
    scenario(repo, "shop.cart.02", Some("b.F1"));
    let ship = run(repo, &["subject", "move", "s", "--to", "shipped"]);
    assert_eq!(ship.status.code(), Some(1), "{}", stderr(&ship));
    assert!(
        stderr(&ship).contains(".canon/changes/b/brief.md"),
        "{}",
        stderr(&ship)
    );
}

/// subject.lifecycle.38: a pending draft whose brief `canon brief check`
/// still has blockers for keeps `canon brief check <c>` as the
/// planner's next step; once the brief is clean the step asks the human.
/// Neither blocks the subject's own steps (here, its ship step).
#[test]
fn subject_lifecycle_38_a_pending_drafts_next_step_follows_its_brief_check() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    change(repo, "a", true);
    scenario(repo, "shop.cart.01", Some("a.F1"));
    scenario(repo, "shop.cart.02", Some("a.F1"));
    let path = repo.join(".canon/specs/kind=feature/area=shop/cart.feature");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("@case:happy\n  @flow:a.F1\n  @shop.cart.02", "@case:failure\n  @flow:a.F1\n  @shop.cart.02"),
    )
    .unwrap();
    for id in ["shop.cart.01", "shop.cart.02"] {
        assert!(attest(repo, id, "run").status.success());
    }
    // An unfilled brief skeleton: blockers.
    ok(repo, &["change", "new", "b", "--subject", "s", "--title", "B"]);

    let steps = |repo: &Path| -> Vec<Value> {
        let status: Value = serde_json::from_str(&stdout(&ok(repo, &["status", "--json"]))).unwrap();
        status["meta"]["next"].as_array().unwrap().clone()
    };
    let has = |steps: &[Value], prefix: &str| steps.iter().any(|s| s["command"].as_str().unwrap().starts_with(prefix));

    let blocked = steps(repo);
    assert!(has(&blocked, "canon brief check b"), "{blocked:?}");
    assert!(!has(&blocked, "show the pending draft's brief"), "{blocked:?}");
    assert!(has(&blocked, "canon subject move s --to shipped"), "{blocked:?}");

    std::fs::write(brief_path(repo, "b"), brief("")).unwrap();
    let clean = steps(repo);
    assert!(has(&clean, "show the pending draft's brief in .canon/changes/b/brief.md to the human and ask; s is not blocked meanwhile"), "{clean:?}");
    assert!(!has(&clean, "canon brief check"), "{clean:?}");
    assert!(has(&clean, "canon subject move s --to shipped"), "{clean:?}");
}

/// A subject that shipped with approved `a`, then adopted a draft `b`
/// scenario `shop.cart.03` is tagged for.
fn shipped_with_tagged_draft() -> TempDir {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    change(repo, "a", true);
    scenario(repo, "shop.cart.01", Some("a.F1"));
    scenario(repo, "shop.cart.02", Some("a.F1"));
    let path = repo.join(".canon/specs/kind=feature/area=shop/cart.feature");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("@case:happy\n  @flow:a.F1\n  @shop.cart.02", "@case:failure\n  @flow:a.F1\n  @shop.cart.02"),
    )
    .unwrap();
    for id in ["shop.cart.01", "shop.cart.02"] {
        assert!(attest(repo, id, "run").status.success());
    }
    ok(repo, &["subject", "move", "s", "--to", "shipped"]);
    change(repo, "b", false);
    scenario(repo, "shop.cart.03", Some("b.F1"));
    ok(repo, &["inventory", "sync"]);
    dir
}

/// subject.lifecycle.39: a tagged unapproved draft keeps blocking reopen
/// and ship under a newer untagged draft, and the output names every
/// change set aside.
#[test]
fn subject_lifecycle_39_a_tagged_draft_under_a_newer_untagged_draft_still_blocks() {
    let dir = shipped_with_tagged_draft();
    let repo = dir.path();
    let reopen = |repo: &Path| {
        run(repo, &["subject", "move", "s", "--to", "verifying", "--reason", "fix"])
    };
    let control = reopen(repo);
    assert_eq!(control.status.code(), Some(1), "{}", stderr(&control));
    assert!(stderr(&control).contains(".canon/changes/b/brief.md"), "{}", stderr(&control));

    // A newer untagged draft c changes nothing: the scope still ends at b.
    change(repo, "c", false);
    let out = reopen(repo);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains(".canon/changes/b/brief.md"), "{}", stderr(&out));
    assert!(!stderr(&out).contains(".canon/changes/c/brief.md"), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("`c` is a pending draft (never approved, not a chore, no scenario tagged `@flow:c.*`): not part of this transition"),
        "a refused reopen names what it set aside: {}",
        stderr(&out)
    );
    let json = run(repo, &["subject", "move", "s", "--to", "verifying", "--reason", "fix", "--json"]);
    assert_eq!(json.status.code(), Some(1));
    assert!(stdout(&json).contains("`c` is a pending draft"), "{}", stdout(&json));

    // Status does not advise the reopen the guard refuses.
    let text = stdout(&ok(repo, &["status"]));
    assert!(text.contains("reopening is refused until the brief above is approved"), "{text}");
    assert!(!text.contains("reopen with `canon subject move s"), "{text}");
}

/// subject.lifecycle.39: ship, from verifying, is refused naming b under
/// the same setup; every pending draft set aside is named on a transition
/// that goes through.
#[test]
fn subject_lifecycle_39_ship_names_the_tagged_draft_and_every_change_set_aside() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    change(repo, "a", true);
    scenario(repo, "shop.cart.01", Some("a.F1"));
    scenario(repo, "shop.cart.02", Some("a.F1"));
    let path = repo.join(".canon/specs/kind=feature/area=shop/cart.feature");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("@case:happy\n  @flow:a.F1\n  @shop.cart.02", "@case:failure\n  @flow:a.F1\n  @shop.cart.02"),
    )
    .unwrap();
    for id in ["shop.cart.01", "shop.cart.02"] {
        assert!(attest(repo, id, "run").status.success());
    }
    change(repo, "b", false);
    scenario(repo, "shop.cart.03", Some("b.F1"));
    ok(repo, &["inventory", "sync"]);
    change(repo, "c", false);
    let ship = run(repo, &["subject", "move", "s", "--to", "shipped"]);
    assert_eq!(ship.status.code(), Some(1), "{}", stderr(&ship));
    assert!(stderr(&ship).contains(".canon/changes/b/brief.md"), "{}", stderr(&ship));
    assert!(stderr(&ship).contains("`c` is a pending draft"), "a refused ship names what it set aside: {}", stderr(&ship));
    let json = run(repo, &["subject", "move", "s", "--to", "shipped", "--json"]);
    assert_eq!(json.status.code(), Some(1));
    assert!(stdout(&json).contains("`c` is a pending draft"), "{}", stdout(&json));

    // Retag shop.cart.03 for a: b and c are both set aside, and the line names both.
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("@flow:b.F1", "@flow:a.F1")).unwrap();
    ok(repo, &["inventory", "sync"]);
    assert!(attest(repo, "shop.cart.03", "run").status.success());
    let ship = run(repo, &["subject", "move", "s", "--to", "shipped"]);
    assert!(ship.status.success(), "{}", stderr(&ship));
    assert!(
        stderr(&ship).contains("`b`, `c` are pending drafts"),
        "{}",
        stderr(&ship)
    );
}

/// evidence.add.102: evidence recorded after the subject adopted its first
/// change (here while it was specced) is not the existing behavior the
/// exemption keeps re-verifying.
#[test]
fn evidence_add_102_evidence_recorded_after_adoption_does_not_exempt() {
    let dir = repo_with_subject("proposed");
    let repo = dir.path();
    ok(repo, &["subject", "move", "s", "--to", "specced"]);
    change(repo, "a", false);
    scenario(repo, "shop.cart.01", None);
    scenario(repo, "shop.cart.02", None);
    let first = attest(repo, "shop.cart.01", "while specced");
    assert!(first.status.success(), "{}", stderr(&first));
    ok(repo, &["subject", "move", "s", "--to", "building"]);
    let again = attest(repo, "shop.cart.01", "after building");
    assert_eq!(again.status.code(), Some(1), "{}", stderr(&again));
    assert!(stderr(&again).contains("BRIEF_UNAPPROVED"), "{}", stderr(&again));
}

/// context.status.82: a pending draft whose brief has no blockers and
/// advisories left gets the planner's polish step first, then the human
/// report step; neither holds back the subject's own steps.
#[test]
fn context_status_82_a_pending_draft_with_advisories_is_polished_before_it_is_reported() {
    let dir = repo_with_subject("verifying");
    let repo = dir.path();
    change(repo, "a", true);
    scenario(repo, "shop.cart.01", Some("a.F1"));
    ok(repo, &["change", "new", "b", "--subject", "s", "--title", "B"]);
    std::fs::write(brief_path(repo, "b"), brief("").replace(" ==**F1 is the happy path.**==", "")).unwrap();

    let status: Value = serde_json::from_str(&stdout(&ok(repo, &["status", "--json"]))).unwrap();
    let steps = status["meta"]["next"].as_array().unwrap();
    let position = |needle: &str| steps.iter().position(|s| s["command"].as_str().unwrap().contains(needle));
    let polish = position("canon skills show brief-polish").unwrap_or_else(|| panic!("{steps:?}"));
    let report = position("show the pending draft's brief").unwrap_or_else(|| panic!("{steps:?}"));
    assert_eq!(report, polish + 1, "{steps:?}");
    assert_eq!(steps[polish]["role"], "planner");
    assert!(steps[polish]["command"].as_str().unwrap().contains("`canon brief check b`"), "{}", steps[polish]);
    let why = steps[polish]["why"].as_str().unwrap();
    assert!(why.contains("1 advisory finding(s)") && why.contains("prose.key-sentence") && why.contains("not blocked meanwhile"), "{why}");
    assert!(steps.len() > 2, "the subject's own steps stay: {steps:?}");
}
