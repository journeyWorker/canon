//! Integration tests for `canon policy approve` and the approval
//! advisory `canon gate check` and `canon status` print (0.15 D5,
//! dogfood eval 2 G12), through the built `canon` binary.

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

/// context.status.40: the advisory follows the policy's bytes: present
/// before approval, gone after it, back (naming both digests) once the
/// policy changes. `gate check`, `status` and `status --json` all say so,
/// both name the repair command. Never approved stays advisory (the gate
/// exits 0); a changed policy is a violation of `gate check` (0.22 D3).
#[test]
fn gate_check_and_status_advise_until_the_current_policy_is_approved() {
    let dir = initialized_repo();
    let repo = dir.path();
    const REPAIR: &str = "canon policy approve --by <who>";

    let check = |expect: Option<&str>, gate_exit: i32| {
        let gate = run_canon(&["gate", "check"], repo);
        let status = run_canon(&["status"], repo);
        let json = run_canon(&["status", "--json"], repo);
        assert_eq!(gate.status.code(), Some(gate_exit), "gate exit: {}{}", stdout(&gate), stderr(&gate));
        assert_eq!(status.status.code(), Some(0), "status: {}", stderr(&status));
        assert_eq!(json.status.code(), Some(0), "status --json: {}", stderr(&json));
        let report: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("status --json prints one JSON object");
        let warnings: Vec<&str> = report["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
        let policy_warnings: Vec<&str> = warnings.iter().copied().filter(|w| w.starts_with("policy not approved") || w.starts_with("policy changed")).collect();
        match expect {
            Some(text) => {
                if gate_exit == 0 {
                    assert!(stdout(&gate).contains(&format!("advisory: {text}")), "gate check: {}", stdout(&gate));
                    assert!(stdout(&gate).contains(REPAIR), "the advisory names the command: {}", stdout(&gate));
                } else {
                    assert!(stdout(&gate).contains("policy changed since approval"), "gate check: {}", stdout(&gate));
                    assert!(stdout(&gate).contains("canon policy approve --by <human>"), "the violation names the re-approval: {}", stdout(&gate));
                }
                let line = stdout(&status).lines().find(|line| line.starts_with(&format!("warning: {text}"))).map(str::to_string);
                let line = line.unwrap_or_else(|| panic!("status: {}", stdout(&status)));
                assert!(line.contains(REPAIR), "the status warning names the command: {line}");
                assert_eq!(policy_warnings.len(), 1, "one policy warning in --json: {warnings:?}");
                assert!(policy_warnings[0].starts_with(text) && policy_warnings[0].contains(REPAIR), "{warnings:?}");
                assert_eq!(format!("warning: {}", policy_warnings[0]), line, "--json carries the same warning text");
            }
            None => {
                assert!(!stdout(&gate).contains("policy not approved") && !stdout(&gate).contains("policy changed"), "gate check: {}", stdout(&gate));
                assert!(!stdout(&status).contains("policy not approved") && !stdout(&status).contains("policy changed"), "status: {}", stdout(&status));
                assert!(policy_warnings.is_empty(), "status --json: {warnings:?}");
            }
        }
    };

    check(Some("policy not approved"), 0);
    let approved = run_canon(&["policy", "approve", "--by", "alice"], repo);
    assert_eq!(approved.status.code(), Some(0), "stderr: {}", stderr(&approved));
    check(None, 0);

    let policy_path = repo.join(".canon/policy.yaml");
    let approved_digest = format!("sha256:{:x}", Sha256::digest(std::fs::read(&policy_path).unwrap()));
    assert_eq!(approval(repo)["policy_digest"], approved_digest.as_str());
    let mut policy = std::fs::read_to_string(&policy_path).unwrap();
    policy.push_str("# loosened later\n");
    std::fs::write(&policy_path, &policy).unwrap();
    let current_digest = format!("sha256:{:x}", Sha256::digest(policy.as_bytes()));
    check(Some("policy changed since approval"), 1);
    let gate = stdout(&run_canon(&["gate", "check"], repo));
    let changed = gate.lines().find(|line| line.contains("policy changed since approval")).expect("the changed-policy violation");
    assert!(changed.contains(&format!("is {current_digest}")), "the violation names the current digest: {changed}");
    assert!(changed.contains(&format!("approved {approved_digest}")), "and the approved one: {changed}");
    assert_ne!(approved_digest, current_digest);

    assert_eq!(approved_digest, approval(repo)["policy_digest"].as_str().unwrap(), "the record still names the approved bytes");
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
    assert!(agents.contains("canon brief approve <slug> --by <the human> --note"), "{agents}");
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
