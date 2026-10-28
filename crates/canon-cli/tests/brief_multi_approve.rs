//! `canon brief approve` with several change ids (0.31 `canon-0-31-loop`, F1),
//! against the built binary: one name, one answer and one timestamp for all,
//! every id checked before any is written, one refused id refuses the call.

use std::path::Path;
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
    cmd.args(args).current_dir(repo).env_remove("CANON_ROLE");
    if let Some(role) = role {
        cmd.env("CANON_ROLE", role);
    }
    cmd.output().expect("spawn canon binary")
}

fn run(repo: &Path, args: &[&str]) -> Output {
    run_as(repo, None, args)
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

/// The brief with its `Approved:` line's time replaced, so two approvals compare.
fn without_time(repo: &Path, change: &str) -> String {
    let text = std::fs::read_to_string(brief_path(repo, change)).unwrap();
    let line = text.lines().find(|l| l.starts_with("Approved: ")).unwrap().to_string();
    let stamp = line.split_whitespace().nth(2).unwrap().to_string();
    text.replace(&stamp, "<time>")
}

fn policy_approval(repo: &Path) -> Value {
    let mut value: Value = serde_json::from_str(&std::fs::read_to_string(repo.join(".canon/policy.approval.json")).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("approved_at");
    value
}

fn approved_at(repo: &Path, change: &str) -> String {
    let text = std::fs::read_to_string(brief_path(repo, change)).unwrap();
    let line = text.lines().find(|l| l.starts_with("Approved: ")).unwrap();
    line.split_whitespace().nth(2).unwrap().to_string()
}

const ANSWER: [&str; 4] = ["--approver", "Ana", "--note", "Go, all three."];

/// skills.install.69: three changes approved in one call carry one approver,
/// one answer and one time, equal to approving them one by one, and the same
/// call again changes nothing.
#[test]
fn skills_install_69_one_call_approves_several_changes_with_one_answer_and_one_time() {
    let many = repo();
    let each = repo();
    let mut args = vec!["brief", "approve", "a", "b", "c"];
    args.extend(ANSWER);
    let out = ok(many.path(), &args);
    assert!(stdout(&out).contains("a approved by `Ana`") && stdout(&out).contains("3 change(s); policy approval recorded too"), "{}", stdout(&out));
    for change in ["a", "b", "c"] {
        let mut single = vec!["brief", "approve", change];
        single.extend(ANSWER);
        ok(each.path(), &single);
    }

    let times: Vec<String> = ["a", "b", "c"].iter().map(|c| approved_at(many.path(), c)).collect();
    assert!(times.iter().all(|t| *t == times[0]), "one call writes one time: {times:?}");
    for change in ["a", "b", "c"] {
        assert_eq!(without_time(many.path(), change), without_time(each.path(), change), "{change} reads as approved one at a time");
    }
    assert_eq!(policy_approval(many.path()), policy_approval(each.path()));
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(many.path().join(".canon/policy.approval.json")).unwrap()).unwrap();
    assert_eq!(recorded["approved_at"], times[0].as_str(), "the policy approval carries the call's one time");
    let status: Value = serde_json::from_str(&stdout(&ok(many.path(), &["brief", "check", "a", "--json"]))).unwrap();
    assert_eq!(status["data"]["change"], "a");

    let before: Vec<String> = ["a", "b", "c"].iter().map(|c| std::fs::read_to_string(brief_path(many.path(), c)).unwrap()).collect();
    let policy_before = std::fs::read_to_string(many.path().join(".canon/policy.approval.json")).unwrap();
    let again = ok(many.path(), &[&["brief", "approve", "a", "b", "c", "--json"][..], &ANSWER].concat());
    let doc: Value = serde_json::from_str(&stdout(&again)).unwrap();
    let outcomes: Vec<&str> = doc["data"]["approvals"].as_array().unwrap().iter().map(|a| a["outcome"].as_str().unwrap()).collect();
    assert_eq!(outcomes, ["unchanged"; 3], "{doc}");
    assert_eq!(doc["meta"]["unchanged"], true, "{doc}");
    let after: Vec<String> = ["a", "b", "c"].iter().map(|c| std::fs::read_to_string(brief_path(many.path(), c)).unwrap()).collect();
    assert_eq!(before, after);
    assert_eq!(policy_before, std::fs::read_to_string(many.path().join(".canon/policy.approval.json")).unwrap());
}

/// skills.install.70: a brief with a blocker, an id with no brief and a brief
/// that ends inside an open code fence refuse the whole call, all three named
/// with their own codes, and nothing changes.
#[test]
fn skills_install_70_a_blocked_a_missing_and_an_unclosed_brief_refuse_all_and_write_nothing() {
    let dir = repo();
    let repo = dir.path();
    // `d` keeps the skeleton `change new` wrote, whose blockers refuse an approval.
    ok(repo, &["change", "new", "d", "--subject", "s", "--title", "d"]);
    std::fs::write(brief_path(repo, "c"), format!("{}\n```\nunclosed\n", brief())).unwrap();
    let before: Vec<String> = ["a", "b", "c", "d"].iter().map(|c| std::fs::read_to_string(brief_path(repo, c)).unwrap()).collect();

    let out = run(repo, &[&["brief", "approve", "d", "b", "ghost", "c", "--json"][..], &ANSWER].concat());
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    let doc: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(doc["error"]["code"], "BRIEF_APPROVE_REFUSED", "{doc}");
    let refused: Vec<(&str, &str)> = doc["data"]["refused"].as_array().unwrap().iter().map(|r| (r["changeId"].as_str().unwrap(), r["code"].as_str().unwrap())).collect();
    assert_eq!(refused, [("d", "BRIEF_APPROVE_REFUSED"), ("ghost", "BRIEF_NOT_FOUND"), ("c", "BRIEF_UNAPPROVABLE")], "{doc}");
    let message = doc["error"]["message"].as_str().unwrap();
    assert!(message.contains("`d`") && message.contains("`ghost`") && message.contains("`c`") && !message.contains("`b`"), "{message}");

    let after: Vec<String> = ["a", "b", "c", "d"].iter().map(|c| std::fs::read_to_string(brief_path(repo, c)).unwrap()).collect();
    assert_eq!(before, after, "no brief changed, not even the approvable ones");
    assert!(!repo.join(".canon/policy.approval.json").exists(), "no policy approval was recorded");

    // The human text names them too.
    let human = run(repo, &[&["brief", "approve", "d", "ghost", "c"][..], &ANSWER].concat());
    assert_eq!(human.status.code(), Some(2));
    assert!(stderr(&human).contains("BRIEF_APPROVE_REFUSED") && stderr(&human).contains("`ghost`"), "{}", stderr(&human));
}

/// skills.install.71: with several ids the call is human-only for every agent
/// role, writes nothing, and the hint carries every id.
#[test]
fn skills_install_71_several_ids_are_human_only_and_the_hint_names_them_all() {
    let dir = repo();
    let repo = dir.path();
    for role in ["planner", "implementer", "orchestrator"] {
        let out = run_as(repo, Some(role), &[&["brief", "approve", "a", "b", "c", "--json"][..], &ANSWER].concat());
        assert_eq!(out.status.code(), Some(2), "{role}: {}", stdout(&out));
        let doc: Value = serde_json::from_str(&stdout(&out)).unwrap();
        assert_eq!(doc["error"]["code"], "HUMAN_ONLY", "{doc}");
        let hint = doc["error"]["hint"].as_str().unwrap();
        assert!(hint.contains("canon brief approve a b c --approver <their name> --note"), "{role}: {hint}");
    }
    for change in ["a", "b", "c"] {
        assert!(!std::fs::read_to_string(brief_path(repo, change)).unwrap().contains("Approved:"), "{change} is untouched");
    }
}

/// skills.install.72: a repeated id counts once, one id prints as before and
/// several print one entry each; a `--input` array still stops at its first failure.
#[test]
fn skills_install_72_repeated_ids_count_once_and_one_id_prints_as_before() {
    let dir = repo();
    let repo = dir.path();
    let once = ok(repo, &[&["brief", "approve", "a", "a", "--json"][..], &ANSWER].concat());
    let doc: Value = serde_json::from_str(&stdout(&once)).unwrap();
    assert_eq!(doc["data"]["change"], "a", "a repeated id is one id, so the single-id document: {doc}");
    assert!(doc["data"].get("approvals").is_none());

    let two = ok(repo, &[&["brief", "approve", "b", "c", "--json"][..], &ANSWER].concat());
    let doc: Value = serde_json::from_str(&stdout(&two)).unwrap();
    let entries = doc["data"]["approvals"].as_array().unwrap();
    assert_eq!(entries.iter().map(|e| (e["change"].as_str().unwrap(), e["outcome"].as_str().unwrap())).collect::<Vec<_>>(), [("b", "written"), ("c", "written")], "{doc}");
    assert_eq!(doc["data"]["approvedBy"], "Ana");

    // `--input`: each element carries its own name and answer and the run stops at the first failure.
    let batch = serde_json::json!([
        {"operands": ["a"], "approver": "Ana", "note": "first"},
        {"operands": ["ghost"], "approver": "Ana", "note": "second"},
        {"operands": ["b"], "approver": "Ana", "note": "third"}
    ])
    .to_string();
    let out = run(repo, &["brief", "approve", "--input", &batch, "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let text = std::fs::read_to_string(brief_path(repo, "a")).unwrap();
    assert!(text.contains("\"first\""), "the approval before the failure stays: {text}");
    assert!(std::fs::read_to_string(brief_path(repo, "b")).unwrap().contains("\"Go, all three.\""), "b was not approved by the element after the failure");
}

fn repo_file(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// skills.install.73: the skills, README, status help and schema documentation
/// state the governing-change rule, the `briefs` list and the several-id call.
#[test]
fn skills_install_73_the_docs_state_the_governing_change_rule_and_the_briefs_list() {
    let readme = repo_file("README.md");
    let upgrading = readme.split("## 0.31.0 release").nth(1).and_then(|section| section.split("## 0.30.0 release").next()).expect("a 0.31.0 section");
    assert!(upgrading.contains("### Upgrading") && upgrading.contains("`statusVersion` is `4`") && upgrading.contains("`brief` object is\ngone; read `briefs` instead"), "{upgrading}");

    for (file, needles) in [
        ("canon/skills/reference/canon-subject.md", &["A subject answers for the changes it tags, approved or adopted as chores", "`briefs` is present", "pending draft", "takes one or more slugs"][..]),
        ("canon/skills/reference/canon-gate.md", &["a change the subject answers for"][..]),
        ("canon/skills/SKILL.src.md", &["every adopted change that is tagged", "one or more slugs"][..]),
        ("canon/skills/reference/canon-report-dashboard.md", &["selection box", "refuses the whole request"][..]),
    ] {
        let text = repo_file(file);
        for needle in needles {
            assert!(text.contains(needle), "{file} lacks `{needle}`");
        }
    }

    let help = run(Path::new("."), &["brief", "approve", "--help"]);
    assert!(stdout(&help).contains("[CHANGE_ID]...") && stdout(&help).contains("one refused id refuses the call"), "{}", stdout(&help));
    let status_help = run(Path::new("."), &["status", "--help"]);
    assert!(stdout(&status_help).contains("pending draft") && stdout(&status_help).contains("`briefs` lists one entry per change the subject answers for"), "{}", stdout(&status_help));
    let evidence_help = run(Path::new("."), &["evidence", "add", "--help"]);
    assert!(stdout(&evidence_help).contains("an unapproved chore or a pending draft never blocks evidence"), "{}", stdout(&evidence_help));
    assert!(repo_file("canon/skills/reference/canon-subject.md").contains("\"statusVersion\": 4"));
    let spec = repo_file("docs/cli/canon.cli.json");
    assert!(spec.contains("BriefApproveManyReport") && spec.contains("\"briefs\"") && spec.contains("brief approve --json"), "the schema document carries the new shapes");

    let dir = repo();
    let status: Value = serde_json::from_str(&stdout(&ok(dir.path(), &["status", "--json"]))).unwrap();
    assert_eq!(status["data"]["statusVersion"], 4);
}

/// skills.install.72: a disk error while writing exits 1, leaves the ids written
/// before it approved, marks the failed id and skips the rest; a rerun after the
/// fix reports the written ones unchanged.
#[test]
#[cfg(unix)]
fn skills_install_72_a_write_failure_keeps_earlier_ids_and_skips_the_rest() {
    use std::os::unix::fs::PermissionsExt;
    let dir = repo();
    let repo = dir.path();
    let locked = brief_path(repo, "b");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o444)).unwrap();
    if std::fs::OpenOptions::new().write(true).open(&locked).is_ok() {
        eprintln!("skipping: the files are writable despite mode 0444 (running as root)");
        return;
    }
    let out = run(repo, &[&["brief", "approve", "a", "b", "c", "--json"][..], &ANSWER].concat());
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let doc: Value = serde_json::from_str(&stdout(&out)).unwrap();
    let outcomes: Vec<&str> = doc["data"]["approvals"].as_array().unwrap().iter().map(|a| a["outcome"].as_str().unwrap()).collect();
    assert_eq!(outcomes, ["written", "failed", "skipped"], "{doc}");
    assert!(std::fs::read_to_string(brief_path(repo, "a")).unwrap().contains("Approved: Ana "));
    assert!(!std::fs::read_to_string(brief_path(repo, "c")).unwrap().contains("Approved:"));
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
    let again = ok(repo, &[&["brief", "approve", "a", "b", "c", "--json"][..], &ANSWER].concat());
    let doc: Value = serde_json::from_str(&stdout(&again)).unwrap();
    let outcomes: Vec<&str> = doc["data"]["approvals"].as_array().unwrap().iter().map(|a| a["outcome"].as_str().unwrap()).collect();
    assert_eq!(outcomes, ["unchanged", "written", "written"], "{doc}");
}
