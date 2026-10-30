//! The changes a subject answers for (0.31 `canon-0-31-loop`, F3), against the
//! built binary: reopen, ship, status and evidence judge every change a
//! scenario of the subject tags, with an `Approved:` line, or a chore,
//! whatever the order of adoption and approval; a never-approved untagged
//! feature change is a pending draft that blocks nothing.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const FEATURE: &str = ".canon/specs/kind=feature/area=shop/cart.feature";

fn git_seed(repo: &Path) {
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(repo).output().expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env_remove("CANON_ROLE").output().expect("spawn canon binary")
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

/// A filled-in English chore brief.
fn chore_brief() -> String {
    "---\nkind: chore\n---\n\n# Bump the client\n\n\
## Intent\n\nBump the client\n\nThe client has a security fix.\n\n\
## Category\n\n`Dependency`.\n\n\
## Invariants\n\n- Requests and responses stay byte-identical. ==**No behavior changes.**==\n\n\
## Checks\n\n- The suite runs again at the chore commit. ==**The suite is the check.**==\n\n\
## Out of scope\n\nNone.\n"
        .to_string()
}

fn brief_path(repo: &Path, change: &str) -> std::path::PathBuf {
    repo.join(format!(".canon/changes/{change}/brief.md"))
}

/// A repo with subjects `s` (verifying) and `t` (building), no change yet.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git_seed(dir.path());
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none", "--no-agents-md", "--no-policy"]);
    for id in ["s", "t"] {
        ok(repo, &["subject", "new", id, "--domain", "dev", "--title", id]);
    }
    ok(repo, &["feature", "new", "shop.cart", "--title", "Cart", "--actor", "u"]);
    for step in ["specced", "building", "verifying"] {
        ok(repo, &["subject", "move", "s", "--to", step]);
    }
    for step in ["specced", "building"] {
        ok(repo, &["subject", "move", "t", "--to", step]);
    }
    dir
}

/// `change` adopted by `subject` with a complete brief, approved when `approve`.
fn change(repo: &Path, subject: &str, change: &str, approve: bool) {
    ok(repo, &["change", "new", change, "--subject", subject, "--title", change]);
    std::fs::write(brief_path(repo, change), brief("")).unwrap();
    if approve {
        approve_all(repo, &[change]);
    }
}

fn approve_all(repo: &Path, changes: &[&str]) {
    let mut args = vec!["brief", "approve"];
    args.extend(changes);
    args.extend(["--approver", "Ana", "--note", "Go"]);
    ok(repo, &args);
}

/// A scenario of `subject` tagged `@flow:<change>.F1`, `case` happy or failure.
fn scenario(repo: &Path, id: &str, subject: &str, flow_change: Option<&str>, case: &str) {
    ok(repo, &["scenario", "new", id, "--title", id, "--subject", subject, "--case", case]);
    if let Some(change) = flow_change {
        let path = repo.join(FEATURE);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replace(&format!("  @{id}\n"), &format!("  @flow:{change}.F1\n  @{id}\n"))).unwrap();
    }
}

/// The two scenarios (a happy and a failure case) that cover change `c`'s F1
/// for `subject`.
fn cover(repo: &Path, first: u32, subject: &str, c: &str) {
    scenario(repo, &format!("shop.cart.{first:02}"), subject, Some(c), "happy");
    scenario(repo, &format!("shop.cart.{:02}", first + 1), subject, Some(c), "failure");
}

fn attest_args<'a>(scenario: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["evidence", "add", "--project", "root", "--scenario", scenario, "--class", "test-run", "--ref", "run", "--role", "implementer", "--actor", "impl", "--promote"];
    args.extend(extra);
    args
}

fn attest(repo: &Path, scenario: &str) -> Output {
    run(repo, &attest_args(scenario, &[]))
}

/// Attests while the brief waits for the human: only the waiver gets a scenario of an unapproved change its first verdict.
fn attest_waived(repo: &Path, scenario: &str) {
    ok(repo, &attest_args(scenario, &["--override-reason", "the human has not answered yet"]));
}

fn ship(repo: &Path, subject: &str) -> Output {
    run(repo, &["subject", "move", subject, "--to", "shipped"])
}

fn reopen(repo: &Path, subject: &str) -> Output {
    run(repo, &["subject", "move", subject, "--to", "verifying", "--reason", "fix"])
}

fn status(repo: &Path) -> Value {
    serde_json::from_str::<Value>(&stdout(&ok(repo, &["status", "--json"]))).unwrap()["data"].clone()
}

fn subject<'a>(status: &'a Value, id: &str) -> &'a Value {
    status["subjects"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap()
}

/// subject.lifecycle.42: two tagged changes, one approved: reopen's sibling
/// guard, ship, names the unapproved one; approving both, in either order,
/// lets the subject ship and then reopen.
#[test]
fn subject_lifecycle_42_every_tagged_change_must_be_approved_in_either_order() {
    for (first, second) in [("a", "b"), ("b", "a")] {
        let dir = repo();
        let repo = dir.path();
        change(repo, "s", "a", false);
        change(repo, "s", "b", false);
        cover(repo, 1, "s", "a");
        cover(repo, 3, "s", "b");
        for id in ["shop.cart.01", "shop.cart.02", "shop.cart.03", "shop.cart.04"] {
            attest_waived(repo, id);
        }
        // Both unapproved: both named, and the hint is one call with both ids.
        let out = ship(repo, "s");
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        let err = stderr(&out);
        assert!(err.contains(".canon/changes/a/brief.md") && err.contains(".canon/changes/b/brief.md"), "{err}");
        assert!(err.contains("`canon brief approve a b --approver <their name> --note \"<their words>\" --relayed-via <the question tool>"), "one call with both ids: {err}");

        approve_all(repo, &[first]);
        let out = ship(repo, "s");
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        let err = stderr(&out);
        assert!(err.contains(&format!(".canon/changes/{second}/brief.md")), "{err}");
        assert!(!err.contains(&format!(".canon/changes/{first}/brief.md")), "the approved one is not named: {err}");

        approve_all(repo, &[second]);
        let out = ship(repo, "s");
        assert!(out.status.success(), "{first} then {second}: {}", stderr(&out));
        let out = reopen(repo, "s");
        assert!(out.status.success(), "{}", stderr(&out));
    }
}

/// subject.lifecycle.43: an approved tagged change edited afterwards refuses
/// reopen and ship naming only that change, and so does the same change with
/// its tags removed.
#[test]
fn subject_lifecycle_43_an_edited_tagged_change_refuses_and_removing_its_tags_does_not_help() {
    let dir = repo();
    let repo = dir.path();
    change(repo, "s", "a", true);
    change(repo, "s", "b", true);
    cover(repo, 1, "s", "a");
    cover(repo, 3, "s", "b");
    for id in ["shop.cart.01", "shop.cart.02", "shop.cart.03", "shop.cart.04"] {
        assert!(attest(repo, id).status.success());
    }
    assert!(ship(repo, "s").status.success());

    let text = std::fs::read_to_string(brief_path(repo, "b")).unwrap();
    std::fs::write(brief_path(repo, "b"), text.replace("Social sign-in.", "Social sign-in, later.")).unwrap();
    for (case, strip) in [("edited", false), ("edited and untagged", true)] {
        if strip {
            let path = repo.join(FEATURE);
            let feature = std::fs::read_to_string(&path).unwrap();
            std::fs::write(&path, feature.replace("  @flow:b.F1\n", "")).unwrap();
        }
        let out = reopen(repo, "s");
        assert_eq!(out.status.code(), Some(1), "{case}: {}", stderr(&out));
        let err = stderr(&out);
        assert!(err.contains(".canon/changes/b/brief.md changed after its approval (BRIEF_CHANGED_SINCE_APPROVAL)"), "{case}: {err}");
        assert!(!err.contains(".canon/changes/a/brief.md"), "{case}: only the edited change is named: {err}");
        assert!(!err.contains("has no `Approved:` line"), "an edit reads differently from a brief never approved: {err}");
    }
    approve_all(repo, &["b"]);
    assert!(reopen(repo, "s").status.success());
}

/// subject.lifecycle.44: an approved feature brief no scenario of the subject
/// tags still refuses ship once it is edited, and is named.
#[test]
fn subject_lifecycle_44_an_edited_approved_brief_no_scenario_tags_refuses_ship() {
    let dir = repo();
    let repo = dir.path();
    change(repo, "s", "a", true);
    cover(repo, 1, "s", "a");
    // `c`'s flow is covered by another subject's scenarios, so `s` answers for it through its approval only.
    change(repo, "s", "c", true);
    cover(repo, 3, "t", "c");
    for id in ["shop.cart.01", "shop.cart.02"] {
        assert!(attest(repo, id).status.success());
    }
    let out = ship(repo, "s");
    assert!(out.status.success(), "{}", stderr(&out));
    ok(repo, &["subject", "move", "s", "--to", "verifying", "--reason", "fix"]);

    let text = std::fs::read_to_string(brief_path(repo, "c")).unwrap();
    std::fs::write(brief_path(repo, "c"), text.replace("Social sign-in.", "Social sign-in, later.")).unwrap();
    let out = ship(repo, "s");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains(".canon/changes/c/brief.md changed after its approval (BRIEF_CHANGED_SINCE_APPROVAL)"), "{}", stderr(&out));
    assert!(!stderr(&out).contains(".canon/changes/a/brief.md"), "{}", stderr(&out));
    let board = status(repo);
    let entry = subject(&board, "s")["briefs"].as_array().unwrap().iter().find(|b| b["change"] == "c").unwrap().clone();
    assert_eq!((entry["status"].as_str(), entry["reason"].as_str()), (Some("changed"), Some("approved")), "{entry}");

    approve_all(repo, &["c"]);
    assert!(ship(repo, "s").status.success());
}

/// subject.lifecycle.45: an unapproved chore, and an approved one edited
/// afterwards, refuse reopen (no override) and ship.
#[test]
fn subject_lifecycle_45_a_chore_that_is_not_approved_now_refuses_reopen_and_ship() {
    let dir = repo();
    let repo = dir.path();
    change(repo, "s", "a", true);
    cover(repo, 1, "s", "a");
    for id in ["shop.cart.01", "shop.cart.02"] {
        assert!(attest(repo, id).status.success());
    }
    assert!(ship(repo, "s").status.success());

    ok(repo, &["change", "new", "bump", "--subject", "s", "--change-kind", "chore", "--title", "Bump"]);
    std::fs::write(brief_path(repo, "bump"), chore_brief()).unwrap();
    // A chore governs from the moment it is adopted: no scenario can tag it.
    let out = run(repo, &["subject", "move", "s", "--to", "verifying", "--reason", "fix", "--override-reason", "go on"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains(".canon/changes/bump/brief.md has no `Approved:` line"), "{}", stderr(&out));
    let board = status(repo);
    let entry = &subject(&board, "s")["briefs"].as_array().unwrap().iter().find(|b| b["change"] == "bump").unwrap().clone();
    assert_eq!((entry["status"].as_str(), entry["reason"].as_str()), (Some("unapproved"), Some("chore")), "{entry}");

    approve_all(repo, &["bump"]);
    let text = std::fs::read_to_string(brief_path(repo, "bump")).unwrap();
    std::fs::write(brief_path(repo, "bump"), text.replace("security fix", "security fix, urgent")).unwrap();
    let out = reopen(repo, "s");
    assert_eq!(out.status.code(), Some(1), "an edited chore refuses the reopen too: {}", stderr(&out));
    assert!(stderr(&out).contains(".canon/changes/bump/brief.md changed after its approval"), "{}", stderr(&out));
    approve_all(repo, &["bump"]);
    assert!(reopen(repo, "s").status.success(), "an approved chore lets the subject reopen");
    let text = std::fs::read_to_string(brief_path(repo, "bump")).unwrap();
    std::fs::write(brief_path(repo, "bump"), text.replace("security fix", "security fix, urgent!")).unwrap();
    let out = ship(repo, "s");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains(".canon/changes/bump/brief.md changed after its approval"), "{}", stderr(&out));
}

/// subject.lifecycle.46: a never-approved untagged feature brief blocks
/// neither reopen, evidence nor ship and is a pending draft in status.
#[test]
fn subject_lifecycle_46_a_never_approved_untagged_feature_brief_blocks_nothing() {
    let dir = repo();
    let repo = dir.path();
    change(repo, "s", "a", true);
    cover(repo, 1, "s", "a");
    for id in ["shop.cart.01", "shop.cart.02"] {
        assert!(attest(repo, id).status.success());
    }
    assert!(ship(repo, "s").status.success());
    change(repo, "s", "d", false);

    let out = reopen(repo, "s");
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains("`d` is a pending draft"), "{}", stderr(&out));
    for id in ["shop.cart.01", "shop.cart.02"] {
        assert!(attest(repo, id).status.success(), "evidence is not blocked");
    }
    let board = status(repo);
    let s = subject(&board, "s");
    assert_eq!(s["pendingDrafts"], serde_json::json!(["d"]), "{s}");
    assert!(s["briefs"].as_array().unwrap().iter().all(|b| b["change"] != "d"), "a draft is listed, never judged: {s}");
    assert!(ship(repo, "s").status.success(), "{}", "the draft does not block ship either");
}

/// context.status.86: status lists one brief per governing change with its
/// state and reason, only drafts as pending drafts, and the new version.
#[test]
fn context_status_86_status_lists_a_brief_per_governing_change_with_state_and_reason() {
    let dir = repo();
    let repo = dir.path();
    change(repo, "s", "tagged-one", false);
    cover(repo, 1, "s", "tagged-one");
    change(repo, "s", "approved-one", true);
    ok(repo, &["change", "new", "bump", "--subject", "s", "--change-kind", "chore", "--title", "Bump"]);
    std::fs::write(brief_path(repo, "bump"), chore_brief()).unwrap();
    change(repo, "s", "draft-one", false);

    let board = status(repo);
    assert_eq!(board["statusVersion"], 4);
    let s = subject(&board, "s");
    let rows: Vec<(String, String, String)> = s["briefs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| (b["change"].as_str().unwrap().into(), b["status"].as_str().unwrap().into(), b["reason"].as_str().unwrap().into()))
        .collect();
    assert_eq!(
        rows,
        [
            ("tagged-one".to_string(), "unapproved".to_string(), "tagged".to_string()),
            ("approved-one".to_string(), "approved".to_string(), "approved".to_string()),
            ("bump".to_string(), "unapproved".to_string(), "chore".to_string()),
        ]
    );
    for entry in s["briefs"].as_array().unwrap() {
        assert!(entry["brief"].as_str().unwrap().ends_with("/brief.md") && entry["findings"].is_object(), "{entry}");
    }
    assert_eq!(s["pendingDrafts"], serde_json::json!(["draft-one"]), "{s}");
    assert!(s["flow"].as_array().unwrap().iter().all(|step| step["change"] == "tagged-one"), "flow holds the tagged changes' steps: {s}");
    let text = stdout(&ok(repo, &["status"]));
    assert!(text.contains("brief [tagged-one, tagged]: not approved"), "{text}");
    assert!(text.contains("brief [approved-one, approved]: approved in"), "{text}");
    assert!(text.contains("brief [bump, chore]: not approved"), "{text}");
}

/// evidence.add.123: a tagged scenario waits only for the changes its tags
/// name, and one whose tag was removed reads as untagged.
#[test]
fn evidence_add_123_a_tagged_scenario_waits_only_for_its_tagged_changes() {
    let dir = repo();
    let repo = dir.path();
    change(repo, "s", "a", true);
    change(repo, "s", "b", true);
    cover(repo, 1, "s", "a");
    cover(repo, 3, "s", "b");
    for id in ["shop.cart.01", "shop.cart.03"] {
        assert!(attest(repo, id).status.success(), "{id}");
    }
    let text = std::fs::read_to_string(brief_path(repo, "b")).unwrap();
    std::fs::write(brief_path(repo, "b"), text.replace("Social sign-in.", "Social sign-in, later.")).unwrap();

    assert!(attest(repo, "shop.cart.01").status.success(), "b's edit does not block a scenario tagged for a");
    let out = attest(repo, "shop.cart.03");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("BRIEF_CHANGED_SINCE_APPROVAL") && stderr(&out).contains(".canon/changes/b/brief.md"), "{}", stderr(&out));

    // Without its tag, the scenario answers to every approved change: b's edit still refuses it.
    let path = repo.join(FEATURE);
    let feature = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, feature.replace("  @flow:a.F1\n  @shop.cart.01\n", "  @shop.cart.01\n")).unwrap();
    let out = attest(repo, "shop.cart.01");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("BRIEF_CHANGED_SINCE_APPROVAL") && stderr(&out).contains(".canon/changes/b/brief.md"), "{}", stderr(&out));
}

/// evidence.add.124: an untagged scenario's evidence is refused when an
/// approved change was edited since, and goes through once it is approved again.
#[test]
fn evidence_add_124_an_untagged_scenario_is_refused_while_an_approved_change_is_edited() {
    let dir = repo();
    let repo = dir.path();
    change(repo, "s", "a", true);
    scenario(repo, "shop.cart.01", "s", None, "happy");
    assert!(attest(repo, "shop.cart.01").status.success());
    let text = std::fs::read_to_string(brief_path(repo, "a")).unwrap();
    std::fs::write(brief_path(repo, "a"), text.replace("Social sign-in.", "Social sign-in, later.")).unwrap();

    let out = attest(repo, "shop.cart.01");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("BRIEF_CHANGED_SINCE_APPROVAL") && stderr(&out).contains(".canon/changes/a/brief.md"), "{}", stderr(&out));
    approve_all(repo, &["a"]);
    assert!(attest(repo, "shop.cart.01").status.success());
}

/// evidence.add.125: an unapproved chore and a never-approved feature brief
/// block no evidence; the chore still blocks the ship.
#[test]
fn evidence_add_125_an_unapproved_chore_and_a_never_approved_brief_do_not_block_evidence() {
    let dir = repo();
    let repo = dir.path();
    change(repo, "s", "a", true);
    scenario(repo, "shop.cart.01", "s", None, "happy");
    ok(repo, &["change", "new", "bump", "--subject", "s", "--change-kind", "chore", "--title", "Bump"]);
    std::fs::write(brief_path(repo, "bump"), chore_brief()).unwrap();
    change(repo, "s", "d", false);

    let out = attest(repo, "shop.cart.01");
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!stdout(&out).contains("brief_override") && !stderr(&out).contains("override"), "{}{}", stdout(&out), stderr(&out));
    let out = ship(repo, "s");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains(".canon/changes/bump/brief.md has no `Approved:` line"), "{}", stderr(&out));
}
