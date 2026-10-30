//! Relayed approvals (0.32 `canon-0-32-relay-approval`) against the built
//! `canon` binary: `brief show --body`, an agent recording the answer a human
//! gave in the conversation (`--relayed-via`) for `brief approve` and `policy
//! approve`, the `canon.yaml` switch that turns it off, and the guidance that
//! tells an agent how. The divergence and migrate legs sit with their own
//! fixtures (`divergence_accept.rs`, `brief_format.rs`); the dashboard leg in
//! `dashboard_briefs.rs`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn git_seed(repo: &Path) {
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(repo).output().expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

fn run_as(repo: &Path, role: Option<&str>, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_canon"));
    cmd.args(args).current_dir(repo).env_remove("CANON_ROLE").env_remove("CANON_ACTOR");
    if let Some(role) = role {
        cmd.env("CANON_ROLE", role);
    }
    cmd.output().expect("spawn canon binary")
}

fn run(repo: &Path, args: &[&str]) -> Output {
    run_as(repo, None, args)
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = run(repo, args);
    assert!(out.status.success(), "{args:?}: {}", text(&out));
    out
}

fn json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", text(o)))
}

fn code(o: &Output) -> String {
    json(o)["error"]["code"].as_str().unwrap_or_default().to_string()
}

/// A complete English brief, so `canon brief approve` accepts it.
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

fn brief_path(repo: &Path, change: &str) -> PathBuf {
    repo.join(format!(".canon/changes/{change}/brief.md"))
}

fn read(repo: &Path, change: &str) -> String {
    std::fs::read_to_string(brief_path(repo, change)).unwrap()
}

/// A repo with a policy file and the changes `a`, `b`, `c`, each with a clean brief.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git_seed(dir.path());
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none", "--no-agents-md"]);
    ok(repo, &["subject", "new", "s", "--domain", "dev", "--title", "S"]);
    for change in ["a", "b", "c"] {
        ok(repo, &["change", "new", change, "--subject", "s", "--title", change]);
        std::fs::write(brief_path(repo, change), brief()).unwrap();
    }
    dir
}

/// The digest `brief show --body` prints for `change`: its last line.
fn shown_digest(repo: &Path, change: &str) -> String {
    let out = ok(repo, &["brief", "show", change, "--body"]);
    String::from_utf8(out.stdout).unwrap().lines().last().unwrap().strip_prefix("digest: ").unwrap().to_string()
}

fn digest_arg(repo: &Path, change: &str) -> String {
    format!("{change}={}", shown_digest(repo, change))
}

/// The files a refused call must leave alone.
fn snapshot(repo: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = ["a", "b", "c"].iter().map(|c| (c.to_string(), std::fs::read(brief_path(repo, c)).unwrap())).collect();
    out.push(("policy".to_string(), std::fs::read(repo.join(".canon/policy.approval.json")).unwrap_or_default()));
    out
}

fn approved_line(repo: &Path, change: &str) -> String {
    read(repo, change).lines().find(|l| l.starts_with("Approved: ")).unwrap().to_string()
}

const ANSWER: [&str; 4] = ["--approver", "Ana", "--note", "Yes, approve both."];

/// subject.lifecycle.48: `brief show --body` prints the brief file's bytes
/// and a last line with the digest `brief approve` records; `--json` carries
/// both as fields; nothing is written.
#[test]
fn subject_lifecycle_48_brief_show_body_prints_the_bytes_and_the_digest_an_approval_records() {
    let dir = repo();
    let r = dir.path();
    let before = std::fs::read(brief_path(r, "a")).unwrap();
    let out = ok(r, &["brief", "show", "a", "--body"]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    let digest = stdout.lines().last().unwrap().strip_prefix("digest: ").unwrap().to_string();
    assert_eq!(stdout, format!("{}digest: {digest}\n", String::from_utf8(before.clone()).unwrap()), "the brief's bytes, then the digest line");
    assert!(digest.starts_with("sha256:") && digest.len() == "sha256:".len() + 64, "{digest}");
    assert_eq!(std::fs::read(brief_path(r, "a")).unwrap(), before, "a read writes nothing");

    let doc = json(&ok(r, &["brief", "show", "a", "--body", "--json"]));
    assert_eq!(doc["data"]["body"].as_str().unwrap().as_bytes(), &*before);
    assert_eq!(doc["data"]["digest"], digest.as_str());
    assert_eq!(doc["data"]["change"], "a");

    ok(r, &["brief", "approve", "a", "--approver", "Ana", "--note", "Go."]);
    assert!(approved_line(r, "a").ends_with(&format!("(brief {digest})")), "the approval records the shown digest: {}", approved_line(r, "a"));
    // A brief whose file has no final newline gets one before the digest line.
    std::fs::write(brief_path(r, "b"), brief().trim_end()).unwrap();
    let plain = String::from_utf8(ok(r, &["brief", "show", "b", "--body"]).stdout).unwrap();
    assert!(plain.contains("Social sign-in.\ndigest: sha256:"), "{plain}");
    let shown = plain.lines().last().unwrap().strip_prefix("digest: ").unwrap().to_string();
    ok(r, &["brief", "approve", "b", "--approver", "Ana", "--note", "Go."]);
    assert!(approved_line(r, "b").ends_with(&format!("(brief {shown})")), "{}", approved_line(r, "b"));
}

/// subject.lifecycle.49: an unknown change and an unreadable brief are
/// refused as `brief show` refuses them, and print no brief text.
#[test]
fn subject_lifecycle_49_brief_show_body_refuses_an_unknown_change_and_an_unreadable_brief() {
    let dir = repo();
    let r = dir.path();
    for args in [&["brief", "show", "missing", "--body", "--json"][..], &["brief", "show", "missing", "--json"]] {
        let out = run(r, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", text(&out));
        assert_eq!(code(&out), "BRIEF_NOT_FOUND");
    }
    std::fs::write(brief_path(r, "c"), [0xff, 0xfe, 0x00, 0x41]).unwrap();
    let plain = run(r, &["brief", "show", "c", "--json"]);
    let body = run(r, &["brief", "show", "c", "--body", "--json"]);
    assert_eq!(code(&body), "BRIEF_UNREADABLE");
    assert_eq!((code(&body), body.status.code()), (code(&plain), plain.status.code()));
    assert!(!String::from_utf8_lossy(&body.stdout).contains("digest:"), "{}", text(&body));
}

/// subject.lifecycle.52: a planner's relayed approval of two briefs marks
/// both `Approved:` lines and differs from a human's only by the marker;
/// `status` and `brief show` name the relay.
#[test]
fn subject_lifecycle_52_a_relayed_approval_of_two_briefs_marks_both_lines_and_status_and_show_name_it() {
    let relayed = repo();
    let human = repo();
    let (r, h) = (relayed.path(), human.path());
    let (a, b) = (digest_arg(r, "a"), digest_arg(r, "b"));
    let mut args = vec!["brief", "approve", "a", "b", "--relayed-via", "AskUserQuestion", "--actor", "plan-1", "--digest", &a, "--digest", &b];
    args.extend(ANSWER);
    let out = run_as(r, Some("planner"), &args);
    assert!(out.status.success(), "{}", text(&out));
    let mut plain = vec!["brief", "approve", "a", "b"];
    plain.extend(ANSWER);
    ok(h, &plain);

    let marker = "(relayed by planner plan-1 via AskUserQuestion)";
    for change in ["a", "b"] {
        let line = approved_line(r, change);
        assert!(line.contains(&format!(" — \"Yes, approve both.\" {marker} (brief sha256:")), "{line}");
        // The marker sits outside the digest: the same text approved by a human binds to the same digest.
        let human_line = approved_line(h, change);
        let strip = |l: &str| -> String { l.split_whitespace().enumerate().filter(|(i, _)| *i != 2).map(|(_, w)| w).collect::<Vec<_>>().join(" ") };
        assert_eq!(strip(&line.replace(&format!(" {marker}"), "")), strip(&human_line), "{change}: only the marker differs");
        assert!(canon_cli::brief::approval_of(&read(r, change)).approved(), "{change} reads as approved");
    }
    assert!(!approved_line(h, "a").contains("relayed"), "a human's line has no marker");
    let policy: Value = serde_json::from_str(&std::fs::read_to_string(r.join(".canon/policy.approval.json")).unwrap()).unwrap();
    assert_eq!(policy["relay"]["actor"], "plan-1", "the policy approval of the same call carries the relay too: {policy}");

    let report = json(&ok(r, &["brief", "approve", "--json", "a", "--approver", "Ana", "--note", "Yes, approve both."]));
    assert_eq!(report["ok"], true, "a human's repeat of the answer rewrites the line without a marker: {report}");
    let status = json(&ok(r, &["status", "--json"]));
    let briefs = status["data"]["subjects"].as_array().map(|subjects| subjects.iter().flat_map(|s| s["briefs"].as_array().cloned().unwrap_or_default()).collect::<Vec<_>>()).unwrap_or_default();
    let relays: Vec<&Value> = briefs.iter().filter(|b| b.get("relay").is_some()).collect();
    assert_eq!(relays.len(), 1, "b stays relayed, a was re-recorded by the human: {status}");
    assert_eq!(relays[0]["change"], "b");
    assert_eq!(relays[0]["relay"], serde_json::json!({ "role": "planner", "actor": "plan-1", "tool": "AskUserQuestion" }));
    let shown = json(&ok(r, &["brief", "show", "b", "--json"]));
    assert_eq!(shown["data"]["relay"]["tool"], "AskUserQuestion", "{shown}");
    assert!(text(&ok(r, &["brief", "show", "b"])).contains("relayed by planner `plan-1` via AskUserQuestion"));
    assert!(text(&ok(r, &["status"])).contains("(relayed by planner plan-1 via AskUserQuestion)"));
}

/// subject.lifecycle.53: a shown digest that differs, or is missing, refuses
/// the call with exit 2 and writes nothing, even for an id whose digest is right.
#[test]
fn subject_lifecycle_53_a_relayed_approval_with_a_wrong_or_missing_digest_writes_nothing() {
    let dir = repo();
    let r = dir.path();
    let before = snapshot(r);
    let a = digest_arg(r, "a");
    let stale = format!("a=sha256:{}", "0".repeat(64));
    let base = ["--relayed-via", "ask", "--actor", "plan-1"];
    let call = |ids: &[&str], digests: &[&str]| {
        let mut args = vec!["brief", "approve", "--json"];
        args.extend(ids);
        args.extend(base);
        args.extend(ANSWER);
        for d in digests {
            args.extend(["--digest", d]);
        }
        run_as(r, Some("planner"), &args)
    };
    let mismatch = call(&["a"], &[&stale]);
    assert_eq!(mismatch.status.code(), Some(2), "{}", text(&mismatch));
    assert_eq!(code(&mismatch), "RELAY_DIGEST_MISMATCH");
    let missing = call(&["b"], &[&a]);
    assert_eq!(missing.status.code(), Some(2), "{}", text(&missing));
    assert_eq!(code(&missing), "USAGE");
    let none = call(&["a"], &[]);
    assert_eq!((none.status.code(), code(&none).as_str()), (Some(2), "USAGE"));
    let both = call(&["a", "b"], &[&a, &format!("b=sha256:{}", "1".repeat(64))]);
    assert_eq!(both.status.code(), Some(2), "{}", text(&both));
    assert_eq!(code(&both), "BRIEF_APPROVE_REFUSED", "{}", text(&both));
    assert!(text(&both).contains("RELAY_DIGEST_MISMATCH"), "{}", text(&both));
    assert_eq!(snapshot(r), before, "no brief and no policy approval changed");
    assert!(!r.join(".canon/policy.approval.json").exists());
    // The brief edited after it was shown is refused too.
    let shown = digest_arg(r, "a");
    std::fs::write(brief_path(r, "a"), brief() + "\nAn edit after the human read it.\n").unwrap();
    let edited = call(&["a"], &[&shown]);
    assert_eq!((edited.status.code(), code(&edited).as_str()), (Some(2), "RELAY_DIGEST_MISMATCH"), "{}", text(&edited));
    assert!(!read(r, "a").contains("Approved:"));
}

/// subject.lifecycle.54: a relayed approval needs the human's name and
/// answer and an actor; a relay option with no role declared is refused.
#[test]
fn subject_lifecycle_54_a_relayed_approval_without_name_answer_actor_or_role_is_refused() {
    let dir = repo();
    let r = dir.path();
    let before = snapshot(r);
    let a = digest_arg(r, "a");
    let relayed = |role: Option<&str>, extra: &[&str]| {
        let mut args = vec!["brief", "approve", "a", "--json", "--relayed-via", "ask", "--digest", &a];
        args.extend(extra);
        run_as(r, role, &args)
    };
    let cases: Vec<(Option<&str>, Vec<&str>, &str)> = vec![
        (Some("planner"), vec!["--approver", "Ana", "--note", "ok"], "no actor"),
        (Some("planner"), vec!["--actor", "plan-1", "--approver", "  ", "--note", "ok"], "blank name"),
        (Some("planner"), vec!["--actor", "plan-1", "--approver", "Ana", "--note", " "], "blank answer"),
        (Some("planner"), vec!["--actor", "two words", "--approver", "Ana", "--note", "ok"], "actor that is not one word"),
        (None, vec!["--actor", "plan-1", "--approver", "Ana", "--note", "ok"], "no role declared"),
    ];
    for (role, extra, what) in cases {
        let out = relayed(role, &extra);
        assert_eq!(out.status.code(), Some(2), "{what}: {}", text(&out));
        let found = code(&out);
        assert!(["USAGE", "APPROVAL_INCOMPLETE"].contains(&found.as_str()), "{what}: {found}");
    }
    let no_role = relayed(None, &["--actor", "plan-1", "--approver", "Ana", "--note", "ok"]);
    assert_eq!(code(&no_role), "USAGE");
    assert!(json(&no_role)["error"]["message"].as_str().unwrap().contains("no agent role"), "{}", text(&no_role));
    let env_actor = {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_canon"));
        cmd.args(["brief", "approve", "a", "--relayed-via", "ask", "--approver", "Ana", "--note", "ok", "--digest", &a]).current_dir(r).env("CANON_ROLE", "planner").env("CANON_ACTOR", "from-env");
        cmd.output().unwrap()
    };
    assert!(env_actor.status.success(), "CANON_ACTOR names the actor: {}", text(&env_actor));
    assert!(approved_line(r, "a").contains("(relayed by planner from-env via ask)"), "{}", approved_line(r, "a"));
    std::fs::write(brief_path(r, "a"), &before[0].1).unwrap();
    std::fs::remove_file(r.join(".canon/policy.approval.json")).unwrap();
    assert_eq!(snapshot(r), before, "the refused calls wrote nothing");
}

/// subject.lifecycle.55: each agent role without the relay option is refused `HUMAN_ONLY` on
/// brief approve, policy approve and divergence accept, the hint naming the relay procedure
/// before the human's call; nothing is written.
#[test]
fn subject_lifecycle_55_an_agent_role_without_the_relay_option_is_human_only_and_the_hint_names_the_procedure() {
    let dir = repo();
    let r = dir.path();
    let before = snapshot(r);
    let leaves: [(&str, &[&str]); 3] = [
        ("brief approve", &["brief", "approve", "a", "--approver", "Ana", "--note", "ok", "--json"]),
        ("policy approve", &["policy", "approve", "--approver", "Ana", "--note", "ok", "--json"]),
        ("divergence accept", &["divergence", "accept", "s.x.01", "--actor", "agent-1", "--reason", "r", "--tracking", "https://example.com/1", "--role", "reviewer", "--json"]),
    ];
    for (leaf, args) in leaves {
        for role in ["planner", "implementer", "reviewer", "orchestrator"] {
            let out = run_as(r, Some(role), args);
            assert_eq!(out.status.code(), Some(2), "{leaf} as {role}: {}", text(&out));
            assert_eq!(code(&out), "HUMAN_ONLY", "{leaf} as {role}");
            let hint = json(&out)["error"]["hint"].as_str().unwrap().to_string();
            let (relay, human) = (hint.find("--relayed-via").expect(&hint), hint.find("their own shell").expect(&hint));
            assert!(relay < human, "{leaf}: the relay procedure comes first: {hint}");
            assert!(hint.contains("question tool") && hint.contains(leaf), "{leaf}: {hint}");
            if leaf == "brief approve" {
                assert!(hint.contains("canon brief show") && hint.contains("--body") && hint.contains("--digest a="), "{hint}");
            }
        }
    }
    assert_eq!(snapshot(r), before);
}

/// subject.lifecycle.56: a relayed policy approval records `relay` in the
/// approval file and reports it; the same refusals apply.
#[test]
fn subject_lifecycle_56_a_relayed_policy_approval_records_its_relay_and_refuses_like_the_brief() {
    let dir = repo();
    let r = dir.path();
    let file = r.join(".canon/policy.approval.json");
    let none = run_as(r, Some("reviewer"), &["policy", "approve", "--approver", "Ana", "--note", "ok", "--json"]);
    assert_eq!((none.status.code(), code(&none).as_str()), (Some(2), "HUMAN_ONLY"));
    let no_note = run_as(r, Some("reviewer"), &["policy", "approve", "--approver", "Ana", "--relayed-via", "ask", "--actor", "rev-1", "--json"]);
    assert_eq!((no_note.status.code(), code(&no_note).as_str()), (Some(2), "USAGE"), "{}", text(&no_note));
    let no_role = run(r, &["policy", "approve", "--approver", "Ana", "--note", "ok", "--relayed-via", "ask", "--actor", "rev-1", "--json"]);
    assert_eq!((no_role.status.code(), code(&no_role).as_str()), (Some(2), "USAGE"), "{}", text(&no_role));
    assert!(!file.exists(), "the refused calls wrote nothing");

    let out = run_as(r, Some("reviewer"), &["policy", "approve", "--approver", "Ana", "--note", "Yes, the policy is fine.", "--relayed-via", "chat", "--actor", "rev-1", "--json"]);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(json(&out)["data"]["relay"], serde_json::json!({ "role": "reviewer", "actor": "rev-1", "tool": "chat" }));
    let record: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(record["approved_by"], "Ana");
    assert_eq!(record["note"], "Yes, the policy is fine.");
    assert_eq!(record["relay"], serde_json::json!({ "role": "reviewer", "actor": "rev-1", "tool": "chat" }));
    let shown = json(&ok(r, &["policy", "show", "--json"]));
    assert_eq!(shown["data"]["state"], "approved");
    assert_eq!(shown["data"]["relay"]["actor"], "rev-1", "{shown}");
    // The same answer by the human is a different record: it drops the relay.
    ok(r, &["policy", "approve", "--approver", "Ana", "--note", "Yes, the policy is fine."]);
    let human: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert!(human.get("relay").is_none(), "{human}");
}

/// subject.lifecycle.58: a line without the marker reads as before, the
/// marker is outside the digest, and a hand edit of it does not unbind.
#[test]
fn subject_lifecycle_58_old_approved_lines_read_as_before_and_the_marker_is_outside_the_digest() {
    let dir = repo();
    let r = dir.path();
    ok(r, &["brief", "approve", "a", "--approver", "Ana", "--note", "Go."]);
    let old = read(r, "a");
    assert!(canon_cli::brief::approval_of(&old).approved());
    assert!(!approved_line(r, "a").contains("relayed"));
    let digest = canon_cli::brief::brief_digest(&old);
    let marked = old.replace(" (brief sha256:", " (relayed by planner plan-1 via ask) (brief sha256:");
    assert_eq!(canon_cli::brief::brief_digest(&marked), digest, "the marker is not in the digest");
    assert!(matches!(canon_cli::brief::approval_of(&marked), canon_cli::brief::Approval::Bound { .. }));
    let edited = marked.replace("via ask", "via chat");
    assert!(matches!(canon_cli::brief::approval_of(&edited), canon_cli::brief::Approval::Bound { .. }), "editing the marker does not change the binding");
    assert_eq!(canon_cli::relay::parse_marker(canon_ingest::markdown::field(&edited, "Approved:").unwrap()).unwrap().tool, "chat");
    assert!(canon_cli::brief::approved_at(canon_ingest::markdown::field(&marked, "Approved:").unwrap()).is_some(), "the time still reads");
    // A relayed approval repeated byte for byte changes nothing (`unchanged`); the human's repeat of it is a change.
    let a = digest_arg(r, "b");
    let args = ["brief", "approve", "b", "--approver", "Ana", "--note", "Go.", "--relayed-via", "ask", "--actor", "plan-1", "--digest", &a];
    assert!(run_as(r, Some("planner"), &args).status.success());
    let first = read(r, "b");
    let again = run_as(r, Some("planner"), &[&args[..], &["--json"]].concat());
    assert_eq!(json(&again)["meta"]["unchanged"], true, "{}", text(&again));
    assert_eq!(read(r, "b"), first);
}

/// subject.lifecycle.59: `approval: { relay: deny }` makes every relayed call
/// `HUMAN_ONLY` naming the key; a malformed section is refused.
#[test]
fn subject_lifecycle_59_a_denied_relay_is_human_only_and_the_hint_names_the_key() {
    let dir = repo();
    let r = dir.path();
    let yaml = std::fs::read_to_string(r.join("canon.yaml")).unwrap();
    std::fs::write(r.join("canon.yaml"), format!("{yaml}approval:\n  relay: deny\n")).unwrap();
    let before = snapshot(r);
    let a = digest_arg(r, "a");
    let brief_call = run_as(r, Some("planner"), &["brief", "approve", "a", "--approver", "Ana", "--note", "ok", "--relayed-via", "ask", "--actor", "plan-1", "--digest", &a, "--json"]);
    let policy_call = run_as(r, Some("planner"), &["policy", "approve", "--approver", "Ana", "--note", "ok", "--relayed-via", "ask", "--actor", "plan-1", "--json"]);
    for out in [&brief_call, &policy_call] {
        assert_eq!(out.status.code(), Some(2), "{}", text(out));
        assert_eq!(code(out), "HUMAN_ONLY");
        let doc = json(out);
        let named = format!("{}{}", doc["error"]["message"].as_str().unwrap(), doc["error"]["hint"].as_str().unwrap());
        assert!(named.contains("approval.relay"), "{named}");
        assert!(doc["error"]["hint"].as_str().unwrap().contains("canon "), "the hint carries the human's call: {doc}");
    }
    assert_eq!(snapshot(r), before);
    // `migrate --commit` is refused the same way, whether or not its plan re-binds an `Approved:` line.
    let preview = ok(r, &["migrate", "--json"]);
    let digest = json(&preview)["data"]["planDigest"].as_str().unwrap().to_string();
    let migrate = run_as(r, Some("planner"), &["migrate", "--commit", "--plan-digest", &digest, "--relayed-via", "ask", "--actor", "plan-1", "--approver", "Ana", "--note", "ok", "--json"]);
    assert_eq!((migrate.status.code(), code(&migrate).as_str()), (Some(2), "HUMAN_ONLY"), "{}", text(&migrate));
    assert!(json(&migrate)["error"]["message"].as_str().unwrap().contains("approval.relay"), "{}", text(&migrate));
    // The human's own call is not affected.
    ok(r, &["brief", "approve", "a", "--approver", "Ana", "--note", "ok"]);
    // A malformed section is refused rather than read as allow.
    std::fs::write(r.join("canon.yaml"), format!("{yaml}approval:\n  relay: maybe\n")).unwrap();
    let bad = run_as(r, Some("planner"), &["policy", "approve", "--approver", "Ana", "--note", "ok", "--relayed-via", "ask", "--actor", "plan-1", "--json"]);
    assert_eq!((bad.status.code(), code(&bad).as_str()), (Some(2), "CONFIG_UNREADABLE"), "{}", text(&bad));
}

/// subject.lifecycle.60: without the key relaying is allowed, and a fresh
/// `canon init` shows the key as a comment.
#[test]
fn subject_lifecycle_60_without_the_key_relaying_is_allowed_and_init_shows_it_commented() {
    let dir = repo();
    let r = dir.path();
    let yaml = std::fs::read_to_string(r.join("canon.yaml")).unwrap();
    assert!(yaml.contains("# approval:") && yaml.contains("#   relay: allow"), "{yaml}");
    assert!(!yaml.lines().any(|l| l.starts_with("approval:")), "the key is a comment, not a setting");
    let a = digest_arg(r, "a");
    let out = run_as(r, Some("planner"), &["brief", "approve", "a", "--approver", "Ana", "--note", "ok", "--relayed-via", "ask", "--actor", "plan-1", "--digest", &a]);
    assert!(out.status.success(), "no key means allow: {}", text(&out));
    std::fs::write(r.join("canon.yaml"), format!("{yaml}approval:\n  relay: allow\n")).unwrap();
    let allowed = run_as(r, Some("planner"), &["policy", "approve", "--approver", "Ana", "--note", "ok", "--relayed-via", "ask", "--actor", "plan-1"]);
    assert!(allowed.status.success(), "relay: allow is the default spelled out: {}", text(&allowed));
}

fn skills_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../canon/skills")
}

fn guide(name: &str) -> String {
    std::fs::read_to_string(skills_root().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Every guide the binary ships: the skill source and each reference.
fn all_guides() -> Vec<(String, String)> {
    let mut out = vec![("SKILL.src.md".to_string(), guide("SKILL.src.md"))];
    let mut names: Vec<String> = std::fs::read_dir(skills_root().join("reference")).unwrap().filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".md")).collect();
    names.sort();
    for name in names {
        let text = guide(&format!("reference/{name}"));
        out.push((name, text));
    }
    out
}

/// The sentences of `text` (blank lines and `. ` end one; single line breaks do not).
fn sentences(text: &str) -> Vec<String> {
    text.split("\n\n").flat_map(|para| para.replace('\n', " ").split(". ").map(|s| s.trim().to_string()).collect::<Vec<_>>()).filter(|s| !s.is_empty()).collect()
}

/// Whether `sentence` tells an agent to hand the brief to the human and stop, or to report and wait.
fn stops_and_hands_off(sentence: &str) -> bool {
    let s = sentence.to_lowercase();
    // The refusal path of `approval: { relay: deny }` names the human's own call by design.
    if s.contains("approval.relay") || s.contains("relay: deny") {
        return false;
    }
    let report_verb = ["report the brief", "report the exact call", "report the command", "report that exact call"];
    ["stops for the human", "stop for the human", "handed to the human", "is the report;", "stop and report", "stops and reports", "stop, report", "say so and wait", "report and wait", "stop and wait"]
        .iter()
        .any(|p| s.contains(p))
        || report_verb.iter().any(|p| s.contains(p))
        || s.match_indices("report").any(|(at, _)| s[at..].chars().take(60).collect::<String>().contains(" and wait"))
}

/// subject.lifecycle.50: every guide that describes recording a relayed answer names the
/// show / ask / count-only-explicit steps (or points at the planner guide that does), and the
/// planner, skill and orchestrator guides carry the whole procedure, `out://` fallbacks and
/// the subagent rule included.
#[test]
fn subject_lifecycle_50_the_guides_show_ask_and_record_with_the_relay_option() {
    let guides = all_guides();
    let procedure = ["SKILL.src.md", "role-planner.md", "role-orchestrator.md"];
    for (name, body) in &guides {
        if procedure.contains(&name.as_str()) {
            for needle in ["brief show", "--body", "out://", "64 KiB", "subagent", "`ask`", "AskUserQuestion", "--relayed-via", "explicit approval", "--digest"] {
                assert!(body.contains(needle), "{name} lacks `{needle}`");
            }
        } else if body.contains("--relayed-via") {
            assert!(body.contains("role-planner") || body.contains("AskUserQuestion") || body.contains("question tool"), "{name} mentions the relay option without the procedure or a pointer to it");
        }
    }
    let planner = guide("reference/role-planner.md");
    for needle in ["divergence accept", "migrate\n   --commit", "policy approve"] {
        assert!(planner.contains(needle), "role-planner.md lacks the same procedure for `{needle}`");
    }
    assert!(guide("reference/role-reviewer.md").contains("--relayed-via") && guide("reference/role-reviewer.md").contains("AskUserQuestion"));
    assert!(canon_cli::guide::stub("canon/SKILL.md", "0.0.0").contains("--relayed-via"), "the installed stub");
}

/// subject.lifecycle.51: a conditional answer is a revision request, no answer records
/// nothing, and no sentence of any guide tells an agent to report to the human and stop
/// or wait; `canon skills check` is clean once installed.
#[test]
fn subject_lifecycle_51_conditional_answers_are_revisions_and_no_guide_says_report_and_wait() {
    let planner = guide("reference/role-planner.md");
    assert!(planner.contains("A question, a condition") && planner.contains("revision request"), "role-planner.md");
    assert!(planner.contains("No answer means\n     nothing is recorded: wait."), "role-planner.md");
    assert!(guide("SKILL.src.md").contains("A question or a condition is a\n     revision request"), "SKILL.src.md");
    let mut found = Vec::new();
    for (name, body) in all_guides() {
        for sentence in sentences(&body) {
            if stops_and_hands_off(&sentence) {
                found.push(format!("{name}: {sentence}"));
            }
        }
    }
    assert!(found.is_empty(), "sentences that still hand the brief over and stop:\n{}", found.join("\n"));
    // The scanner would catch the sentences the old guides held.
    for old in ["The brief is handed to the human and you stop.", "The brief is the report; stop, and put open questions in.", "Report the exact call, slug filled in, and wait.", "The agent checks it, then stops and reports it.", "Say so and wait."] {
        assert!(stops_and_hands_off(old), "{old}");
    }
    assert!(!stops_and_hands_off("Show the brief, ask, and record an explicit approval."));
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["init", "--providers", "claude", "--no-agents-md"]);
    let check = run(dir.path(), &["skills", "check"]);
    assert!(check.status.success(), "{}", text(&check));
}

/// subject.lifecycle.57 (never silently ignored): `migrate --relayed-via` with no role declared is
/// `USAGE`, without `--commit` is `USAGE`, and a plan that re-binds nothing still honours the option.
#[test]
fn subject_lifecycle_57_the_relay_option_of_migrate_is_never_ignored() {
    let dir = repo();
    let r = dir.path();
    let digest = json(&ok(r, &["migrate", "--json"]))["data"]["planDigest"].as_str().unwrap().to_string();
    let relayed = ["--relayed-via", "ask", "--actor", "plan-1", "--approver", "Ana", "--note", "ok", "--json"];
    let commit = ["migrate", "--commit", "--plan-digest", digest.as_str()];
    let no_role = run(r, &[&commit[..], &relayed[..]].concat());
    assert_eq!((no_role.status.code(), code(&no_role).as_str()), (Some(2), "USAGE"), "{}", text(&no_role));
    assert!(json(&no_role)["error"]["message"].as_str().unwrap().contains("no agent role"), "{}", text(&no_role));
    let preview = run_as(r, Some("planner"), &[&["migrate"][..], &relayed[..]].concat());
    assert_eq!((preview.status.code(), code(&preview).as_str()), (Some(2), "USAGE"), "{}", text(&preview));
    let yaml = std::fs::read_to_string(r.join("canon.yaml")).unwrap();
    std::fs::write(r.join("canon.yaml"), format!("{yaml}approval:\n  relay: deny\n")).unwrap();
    let digest = json(&ok(r, &["migrate", "--json"]))["data"]["planDigest"].as_str().unwrap().to_string();
    let denied = run_as(r, Some("planner"), &[&["migrate", "--commit", "--plan-digest", digest.as_str()][..], &relayed[..]].concat());
    assert_eq!((denied.status.code(), code(&denied).as_str()), (Some(2), "HUMAN_ONLY"), "{}", text(&denied));
}
