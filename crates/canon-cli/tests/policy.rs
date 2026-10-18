//! Integration tests for `canon policy approve` / `canon policy show`
//! and what reads the approval (0.15 D5, dogfood eval 2 G12; 0.22 D1/D2,
//! #71): the advisory for a policy never approved, and the hold a policy
//! changed since its approval puts on `canon gate check`, `canon subject
//! status` and `canon status`, through the built `canon` binary.

use std::path::Path;
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A write command's stdout without its closing `next:` line (0.21 D2).
fn without_next(text: &str) -> String {
    text.lines().filter(|line| !line.starts_with("next: ")).map(|line| format!("{line}\n")).collect()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

const APPROVAL: &str = ".canon/policy.approval.json";

/// A repo `canon init` set up, starter policy included, inside a git
/// work tree (so `.gitignore` questions have an answer).
fn initialized_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    let git = Command::new("git").arg("-C").arg(dir.path()).args(["init", "-q"]).status().expect("git must be on PATH");
    assert!(git.success());
    let init = run_canon(&["init", "--no-agents-md"], dir.path());
    assert_eq!(init.status.code(), Some(0), "stderr: {}", stderr(&init));
    dir
}

fn approval(repo: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(repo.join(APPROVAL)).expect("reading the approval")).expect("valid JSON")
}

#[test]
fn approve_records_the_policy_digest_the_approver_the_time_and_the_note() {
    let dir = initialized_repo();
    let repo = dir.path();

    let out = run_canon(&["policy", "approve", "--by", "alice", "--note", "reviewed the starter policy"], repo);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(without_next(&stdout(&out)).ends_with("— written directly; nothing to promote\n"), "the direct-write suffix: {}", stdout(&out));

    let policy = std::fs::read(repo.join(".canon/policy.yaml")).unwrap();
    let recorded = approval(repo);
    assert_eq!(recorded["policy_digest"], format!("sha256:{:x}", Sha256::digest(&policy)), "{recorded}");
    assert_eq!(recorded["approved_by"], "alice");
    assert_eq!(recorded["note"], "reviewed the starter policy");
    assert!(chrono::DateTime::parse_from_rfc3339(recorded["approved_at"].as_str().unwrap()).is_ok(), "{recorded}");
    let text = std::fs::read_to_string(repo.join(APPROVAL)).unwrap();
    assert!(text.starts_with("{\n  \""), "pretty JSON: {text}");

    let without_note = run_canon(&["policy", "approve", "--by", "bob"], repo);
    assert_eq!(without_note.status.code(), Some(0), "stderr: {}", stderr(&without_note));
    assert_eq!(approval(repo)["note"], serde_json::Value::Null, "a re-approval replaces the record");
    assert_eq!(approval(repo)["approved_by"], "bob");
}

#[test]
fn approve_refuses_a_repo_with_no_policy() {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join("canon.yaml"), "").unwrap();
    let out = run_canon(&["policy", "approve", "--by", "alice"], dir.path());
    assert_eq!(out.status.code(), Some(2), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("no policy to approve"), "{}", stderr(&out));
    assert!(!dir.path().join(APPROVAL).exists(), "a refused approve writes nothing");
}

#[test]
fn approve_refuses_an_empty_approver() {
    let dir = initialized_repo();
    let out = run_canon(&["policy", "approve", "--by", "  "], dir.path());
    assert_eq!(out.status.code(), Some(2));
    assert!(!dir.path().join(APPROVAL).exists());
}

const REPAIR: &str = "canon policy approve --by <who>";

/// A copy of the conformance fixture `subject-ship-allowed` (subject
/// `checkout`, `verifying`, every ship guard met, a policy never
/// approved), inside a git work tree.
fn shippable_repo() -> tempfile::TempDir {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    let dir = tempfile::tempdir().expect("a temp dir");
    copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases/subject-ship-allowed/repo"), dir.path());
    let git = Command::new("git").arg("-C").arg(dir.path()).args(["init", "-q"]).status().expect("git must be on PATH");
    assert!(git.success());
    dir
}

fn write_policy(repo: &Path, text: &str) {
    std::fs::write(repo.join(".canon/policy.yaml"), text).unwrap();
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_str(&stdout(output)).unwrap_or_else(|e| panic!("one JSON document on stdout ({e}): {}", stdout(output)))
}

/// The #71 reproduction's policy: approved with `require_review`, then
/// the key deleted.
const WITH_REVIEW: &str = "spec_coverage:\n  require_cases: [failure]\n  require_review: {}\n";
const WITHOUT_REVIEW: &str = "spec_coverage:\n  require_cases: [failure]\n";

fn approve(repo: &Path, by: &str) {
    let out = run_canon(&["policy", "approve", "--by", by], repo);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
}

fn subject_state(repo: &Path, id: &str) -> String {
    let shown = run_canon(&["subject", "status", id, "--json"], repo);
    json(&shown)["data"]["status"].as_str().unwrap_or_else(|| panic!("{}", stdout(&shown))).to_string()
}

/// policy.expressions.32: the gate advises while the policy was never
/// approved, is clean once it is, and fails with `unapproved-policy`
/// (both digests, the approver, the time) once the policy changes, until
/// a human approves the change.
#[test]
fn policy_expressions_32_the_gate_fails_on_a_policy_changed_since_its_approval() {
    let dir = initialized_repo();
    let repo = dir.path();

    let gate = run_canon(&["gate", "check"], repo);
    assert_eq!(gate.status.code(), Some(0), "a policy never approved is an advisory: {}", stdout(&gate));
    assert!(stdout(&gate).contains("advisory: policy not approved") && stdout(&gate).contains(REPAIR), "{}", stdout(&gate));

    approve(repo, "alice");
    let gate = run_canon(&["gate", "check"], repo);
    assert_eq!(gate.status.code(), Some(0), "{}", stdout(&gate));
    assert!(!stdout(&gate).contains("advisory: policy") && !stdout(&gate).contains("unapproved-policy"), "an approved policy says nothing: {}", stdout(&gate));

    let policy_path = repo.join(".canon/policy.yaml");
    let approved_digest = format!("sha256:{:x}", Sha256::digest(std::fs::read(&policy_path).unwrap()));
    let mut policy = std::fs::read_to_string(&policy_path).unwrap();
    policy.push_str("# loosened later\n");
    std::fs::write(&policy_path, &policy).unwrap();
    let current_digest = format!("sha256:{:x}", Sha256::digest(policy.as_bytes()));

    let gate = run_canon(&["gate", "check"], repo);
    assert_eq!(gate.status.code(), Some(1), "a changed policy fails the gate: {}", stdout(&gate));
    let line = stdout(&gate).lines().map(str::trim).find(|line| line.starts_with("unapproved-policy .canon/policy.yaml — changed since its approval")).map(str::to_string);
    let line = line.unwrap_or_else(|| panic!("the violation line: {}", stdout(&gate)));
    assert!(line.contains(&format!("the file is {current_digest}")) && line.contains(&format!("`alice` approved {approved_digest} at ")), "{line}");
    assert!(line.contains(REPAIR) && line.contains("canon brief approve"), "the violation names how a human lifts it: {line}");
    assert!(!stdout(&gate).contains("advisory: policy"), "a violation, not an advisory: {}", stdout(&gate));

    let report = json(&run_canon(&["gate", "check", "--json"], repo));
    assert_eq!(report["error"]["code"], "GATE_VIOLATIONS", "{report}");
    let classes: Vec<&str> = report["data"]["violations"].as_array().unwrap().iter().map(|v| v["class"].as_str().unwrap()).collect();
    assert_eq!(classes, ["unapproved-policy"], "{report}");

    approve(repo, "bob");
    let gate = run_canon(&["gate", "check"], repo);
    assert_eq!(gate.status.code(), Some(0), "the human's approval lifts it: {}", stdout(&gate));
}

/// policy.expressions.33: an approval file that does not read as one
/// fails the gate, naming the file and the parse error; an approval
/// rewrites it.
#[test]
fn policy_expressions_33_an_unreadable_approval_fails_the_gate() {
    let dir = initialized_repo();
    let repo = dir.path();
    std::fs::write(repo.join(APPROVAL), "{\"policy_digest\":").unwrap();

    let gate = run_canon(&["gate", "check"], repo);
    assert_eq!(gate.status.code(), Some(1), "{}", stdout(&gate));
    let line = stdout(&gate).lines().map(str::trim).find(|line| line.starts_with("unapproved-policy .canon/policy.approval.json — not a readable policy approval (")).map(str::to_string);
    let line = line.unwrap_or_else(|| panic!("the violation line: {}", stdout(&gate)));
    assert!(line.contains(REPAIR), "{line}");

    approve(repo, "alice");
    assert_eq!(run_canon(&["gate", "check"], repo).status.code(), Some(0));
}

/// policy.expressions.34: a policy never approved stays an advisory: the
/// gate exits 0, status keeps a warning and no human step, and the
/// subject ships.
#[test]
fn policy_expressions_34_a_policy_never_approved_stays_an_advisory() {
    let dir = shippable_repo();
    let repo = dir.path();
    assert!(!repo.join(APPROVAL).exists());

    let gate = run_canon(&["gate", "check"], repo);
    assert_eq!(gate.status.code(), Some(0), "{}", stdout(&gate));
    assert!(stdout(&gate).contains("advisory: policy not approved"), "{}", stdout(&gate));

    let report = json(&run_canon(&["status", "--json"], repo));
    assert!(report["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().starts_with("policy not approved")), "{report}");
    assert!(report["next"].as_array().unwrap().iter().all(|step| step["actor"] == "agent"), "no human step: {report}");

    let shipped = run_canon(&["subject", "status", "checkout", "--to", "shipped", "--actor-id", "impl"], repo);
    assert_eq!(shipped.status.code(), Some(0), "stderr: {}", stderr(&shipped));
    assert_eq!(subject_state(repo, "checkout"), "shipped");
}

/// policy.expressions.35: `canon policy show` reads back every state,
/// in human and JSON form, and always exits 0; `policy approve`'s first
/// next step is that read.
#[test]
fn policy_expressions_35_policy_show_reads_back_the_approval_state() {
    let show = |repo: &Path| {
        let human = run_canon(&["policy", "show"], repo);
        assert_eq!(human.status.code(), Some(0), "stderr: {}", stderr(&human));
        let out = run_canon(&["policy", "show", "--json"], repo);
        assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
        (stdout(&human), json(&out)["data"].clone())
    };

    let bare = tempfile::tempdir().unwrap();
    std::fs::write(bare.path().join("canon.yaml"), "").unwrap();
    let (human, data) = show(bare.path());
    assert_eq!((data["state"].as_str(), &data["policyDigest"], &data["approval"]), (Some("none"), &serde_json::Value::Null, &serde_json::Value::Null), "{data}");
    assert!(human.contains("state: none"), "{human}");

    let dir = initialized_repo();
    let repo = dir.path();
    let digest = format!("sha256:{:x}", Sha256::digest(std::fs::read(repo.join(".canon/policy.yaml")).unwrap()));
    let (human, data) = show(repo);
    assert_eq!(data["state"], "unapproved", "{data}");
    assert_eq!(data["policy"], ".canon/policy.yaml");
    assert_eq!(data["policyDigest"], digest.as_str());
    assert!(human.contains("state: unapproved") && human.contains(REPAIR), "{human}");

    let approved = run_canon(&["policy", "approve", "--by", "alice", "--note", "read it", "--json"], repo);
    assert_eq!(approved.status.code(), Some(0), "stderr: {}", stderr(&approved));
    assert_eq!(json(&approved)["meta"]["next"][0]["command"], "canon policy show", "{}", stdout(&approved));
    let (human, data) = show(repo);
    assert_eq!(data["state"], "approved", "{data}");
    assert_eq!(data["approval"]["approvedBy"], "alice");
    assert_eq!(data["approval"]["note"], "read it");
    assert_eq!(data["approval"]["policyDigest"], digest.as_str());
    assert!(chrono::DateTime::parse_from_rfc3339(data["approval"]["approvedAt"].as_str().unwrap()).is_ok(), "{data}");
    assert!(human.contains(&format!("approved: {digest} by `alice` at ")), "{human}");

    let mut policy = std::fs::read_to_string(repo.join(".canon/policy.yaml")).unwrap();
    policy.push_str("# edited\n");
    write_policy(repo, &policy);
    let (human, data) = show(repo);
    assert_eq!(data["state"], "changed", "{data}");
    assert_eq!(data["approval"]["policyDigest"], digest.as_str(), "the approval still names the approved bytes");
    assert_ne!(data["policyDigest"], digest.as_str());
    assert!(human.contains("state: changed") && human.contains("`canon gate check` fails"), "{human}");

    std::fs::write(repo.join(APPROVAL), "not json").unwrap();
    let (human, data) = show(repo);
    assert_eq!(data["state"], "unreadable", "{data}");
    assert!(data["approvalError"].as_str().is_some_and(|e| !e.is_empty()), "{data}");
    assert_eq!(data["approval"], serde_json::Value::Null);
    assert!(human.contains("state: unreadable") && human.contains("does not read"), "{human}");
}

/// subject.lifecycle.35, the #71 reproduction: approve a policy with
/// `require_review`, delete the key, and every transition that runs the
/// gate's guards refuses with `POLICY_CHANGED_SINCE_APPROVAL`, an
/// override included, leaving the record unchanged; the human's
/// approval lifts it.
#[test]
fn subject_lifecycle_35_a_changed_policy_refuses_every_guarded_transition_override_or_not() {
    let dir = shippable_repo();
    let repo = dir.path();
    write_policy(repo, WITH_REVIEW);
    approve(repo, "alice");
    write_policy(repo, WITHOUT_REVIEW);

    let refused = |args: &[&str], waived: bool| {
        let out = run_canon(args, repo);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stdout(&out));
        let error = &json(&out)["error"];
        assert_eq!(error["code"], "POLICY_CHANGED_SINCE_APPROVAL", "{error}");
        let message = error["message"].as_str().unwrap();
        assert!(message.contains("unapproved-policy .canon/policy.yaml — changed since its approval") && message.contains("`alice` approved"), "{message}");
        let hint = error["hint"].as_str().unwrap();
        assert!(hint.contains("canon policy show"), "{hint}");
        assert_eq!(hint.contains("--override-reason does not waive it"), waived, "{hint}");
    };

    refused(&["subject", "status", "checkout", "--to", "shipped", "--actor-id", "impl", "--json"], false);
    refused(&["subject", "status", "checkout", "--to", "shipped", "--actor-id", "impl", "--override-reason", "ship it anyway", "--json"], true);
    assert_eq!(subject_state(repo, "checkout"), "verifying", "the record is unchanged");

    for args in [["subject", "new", "extra", "--domain", "dev", "--title", "Extra", "--actor-id", "impl"].as_slice(), &["subject", "status", "extra", "--to", "specced", "--actor-id", "impl"], &["subject", "status", "extra", "--to", "building", "--actor-id", "impl"]] {
        assert_eq!(run_canon(args, repo).status.code(), Some(0), "unguarded transitions still move: {args:?}");
    }
    refused(&["subject", "status", "extra", "--to", "verifying", "--actor-id", "impl", "--json"], false);
    assert_eq!(subject_state(repo, "extra"), "building");

    approve(repo, "alice");
    let shipped = run_canon(&["subject", "status", "checkout", "--to", "shipped", "--actor-id", "impl"], repo);
    assert_eq!(shipped.status.code(), Some(0), "the human's approval lifts it: {}", stderr(&shipped));

    write_policy(repo, WITH_REVIEW);
    refused(&["subject", "status", "checkout", "--to", "verifying", "--reason", "follow-up", "--actor-id", "impl", "--json"], false);
    assert_eq!(subject_state(repo, "checkout"), "shipped", "the reopen refuses too");
}

/// subject.lifecycle.36: an approval file that does not read refuses the
/// ship with `POLICY_APPROVAL_UNREADABLE`, an override included.
#[test]
fn subject_lifecycle_36_an_unreadable_policy_approval_refuses_the_ship() {
    let dir = shippable_repo();
    let repo = dir.path();
    std::fs::write(repo.join(APPROVAL), "{}").unwrap();
    for override_reason in [None, Some("ship it anyway")] {
        let mut args = vec!["subject", "status", "checkout", "--to", "shipped", "--actor-id", "impl", "--json"];
        args.extend(override_reason.iter().flat_map(|reason| ["--override-reason", *reason]));
        let out = run_canon(&args, repo);
        assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
        let error = &json(&out)["error"];
        assert_eq!(error["code"], "POLICY_APPROVAL_UNREADABLE", "{error}");
        assert!(error["message"].as_str().unwrap().contains("unapproved-policy .canon/policy.approval.json — not a readable policy approval"), "{error}");
    }
    assert_eq!(subject_state(repo, "checkout"), "verifying");
}

/// context.status.40: a policy never approved is a status warning; one
/// changed since its approval, or an unreadable approval, is a `human`
/// step and no subject is suggested to move on. Status exits 0 either way.
#[test]
fn context_status_40_status_stops_for_the_human_while_the_policy_is_not_the_approved_one() {
    let dir = shippable_repo();
    let repo = dir.path();
    let status = || {
        let human = run_canon(&["status"], repo);
        assert_eq!(human.status.code(), Some(0), "stderr: {}", stderr(&human));
        let out = run_canon(&["status", "--json"], repo);
        assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
        (stdout(&human), json(&out))
    };
    let moves = |report: &serde_json::Value| report["next"].as_array().unwrap().iter().any(|step| step["command"].as_str().unwrap().starts_with("canon subject status"));

    let (human, report) = status();
    let warning = human.lines().find(|line| line.starts_with("warning: policy not approved")).unwrap_or_else(|| panic!("{human}")).to_string();
    assert!(warning.contains(REPAIR), "{warning}");
    assert!(report["warnings"].as_array().unwrap().iter().any(|w| format!("warning: {}", w.as_str().unwrap()) == warning), "--json carries the same warning: {report}");
    assert!(moves(&report), "an unapproved policy withholds nothing: {report}");

    write_policy(repo, WITH_REVIEW);
    approve(repo, "alice");
    write_policy(repo, WITHOUT_REVIEW);
    for state in ["changed", "unreadable"] {
        if state == "unreadable" {
            std::fs::write(repo.join(APPROVAL), "[]").unwrap();
        }
        let (human, report) = status();
        let steps = report["next"].as_array().unwrap();
        let held = steps.iter().find(|step| step["actor"] == "human" && step["command"].as_str().unwrap().starts_with("stop and report .canon/policy.yaml to the human"));
        let held = held.unwrap_or_else(|| panic!("{state}: the human step: {report}"));
        assert!(held["why"].as_str().unwrap().contains("unapproved-policy") && held["why"].as_str().unwrap().contains("canon policy show"), "{held}");
        assert_eq!(held["role"], serde_json::Value::Null);
        assert!(!moves(&report), "{state}: no transition is suggested beside it: {report}");
        assert!(human.contains("human: stop and report .canon/policy.yaml to the human"), "{human}");
        assert!(!report["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().starts_with("policy")), "{state}: a step, not a warning: {report}");
    }
}

#[test]
fn a_repo_without_a_policy_gets_no_approval_advisory() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let init = run_canon(&["init", "--no-agents-md", "--no-policy"], dir.path());
    assert_eq!(init.status.code(), Some(0), "stderr: {}", stderr(&init));
    let gate = run_canon(&["gate", "check"], dir.path());
    assert!(!stdout(&gate).contains("policy not approved"), "{}", stdout(&gate));
}

#[test]
fn init_names_the_approval_command_for_its_starter_policy() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let init = run_canon(&["init"], dir.path());
    assert_eq!(init.status.code(), Some(0), "stderr: {}", stderr(&init));
    assert!(stdout(&init).contains("canon policy approve --by <who>"), "{}", stdout(&init));
    let agents = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(agents.contains("canon brief approve --change <slug> --by <the human> --note"), "{agents}");
}

/// The approval is authored and must reach git: neither the
/// `.gitignore` `canon init` writes nor canon's own ignores it.
#[test]
fn the_approval_file_is_never_ignored() {
    let ignored = |repo: &Path| Command::new("git").arg("-C").arg(repo).args(["check-ignore", "-q", APPROVAL]).status().expect("git").success();

    let dir = initialized_repo();
    assert!(!ignored(dir.path()), "the .gitignore `canon init` writes must not ignore {APPROVAL}");

    let canon_repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(!ignored(&canon_repo), "canon's own .gitignore must not ignore {APPROVAL}");
}
