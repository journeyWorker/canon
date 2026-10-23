//! 0.28 D7b through the real binary (T17): every leaf that records "the commit
//! a record is about" defaults `--rev` to `HEAD`, resolves any revision to the
//! full sha, records `code_tree` and `worktree_dirty` for the code it saw,
//! refuses nothing for uncommitted code, and refuses `NO_COMMIT` outside git.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_cli::git_rev::code_tree;
use serde_json::Value;
use tempfile::TempDir;

const FEATURE: &str = ".canon/specs/kind=feature/area=shop/cart.feature";
const SCENARIO: &str = "shop.cart.01";
const CHORE_BRIEF: &str = "# Bump the client\n\n<!-- canon:kind chore -->\n\n## Intent\n\nBump the client\n\nApproved: Journey 2026-10-12T00:00:00Z — \"go\"\n";

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").env_remove("CANON_ROLE").current_dir(repo).output().expect("spawn canon")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

fn ok(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}{}", text(&out.stdout), text(&out.stderr));
    text(&out.stdout)
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").current_dir(repo).args(args).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", text(&out.stderr));
    text(&out.stdout).trim().to_string()
}

fn write(repo: &Path, rel: &str, body: &str) {
    let path = repo.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn commit_all(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", message]);
}

fn feature() -> String {
    format!(
        "Feature: shop cart\n  # canon: {{\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{{\"agent_id\":\"canon\"}}}}\n\n  # canon: {{\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{{\"agent_id\":\"canon\"}}}}\n  @{SCENARIO}\n  Scenario: Cart behavior\n    When the shopper adds an item\n    Then the cart has it\n"
    )
}

/// A git repo with one commit holding code, an initialized canon (promote on
/// write), one indexed scenario and one chore brief.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.name", "t"]);
    git(r, &["config", "user.email", "t@example.com"]);
    ok(r, &["init", "--repo", ".", "--no-policy"]);
    write(r, FEATURE, &feature());
    write(r, ".canon/changes/bump-client/brief.md", CHORE_BRIEF);
    write(r, "src/main.rs", "fn main() {}\n");
    ok(r, &["inventory", "sync"]);
    commit_all(r, "one");
    dir
}

fn files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "json") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out.sort();
    out
}

/// The committed ledger records of `kind`.
fn records(repo: &Path, kind: &str) -> Vec<Value> {
    files(&repo.join(".canon/ledger").join(format!("kind={kind}"))).into_iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect()
}

fn head(repo: &Path) -> String {
    git(repo, &["rev-parse", "HEAD"])
}

/// One invocation per leaf that takes a commit option, none of them giving
/// one. `finding close` needs a finding to close, which the first call of
/// `setup` records.
fn leaves() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("review scenario", vec!["review", "scenario", SCENARIO, "--actor-id", "rev-1", "--role", "reviewer", "--original-spec-ref", "docs/prd.md"]),
        ("review change", vec!["review", "change", "bump-client", "--actor-id", "rev-2", "--role", "reviewer", "--original-spec-ref", "docs/prd.md"]),
        ("divergence add", vec!["divergence", "add", SCENARIO, "--actor-id", "rev-1", "--role", "reviewer"]),
        ("divergence defer", vec!["divergence", "defer", SCENARIO, "--actor-id", "rev-1", "--role", "reviewer", "--reason", "later", "--expiry", "2099-01-01"]),
        ("divergence resolve", vec!["divergence", "resolve", SCENARIO, "--actor-id", "rev-1", "--role", "reviewer"]),
        ("evidence add", vec!["evidence", "add", "--scenario-id", SCENARIO, "--class", "test-run", "--ref", "cargo test", "--role", "implementer"]),
        (
            "evidence audit",
            vec![
                "evidence", "audit", SCENARIO, "--ref", "audit-run", "--evaluator-id", "ev", "--evaluator-version", "1", "--evaluator-digest",
                "0000000000000000000000000000000000000000000000000000000000000000", "--audit-result", "confirmed", "--role", "reviewer",
            ],
        ),
        ("finding add", vec!["finding", "add", "--change-id", "bump-client", "--round", "1", "--seq", "1", "--severity", "note", "--actor-id", "rev-1", "--summary", "nit", "--disposition", "fixed"]),
        ("finding close", vec!["finding", "close", "bump-client", "--round", "2", "--seq", "1", "--disposition", "fixed"]),
    ]
}

fn run_leaf(repo: &Path, argv: &[&str], extra: &[&str]) -> Output {
    let mut args: Vec<&str> = argv.to_vec();
    args.extend_from_slice(extra);
    args.push("--json");
    canon(repo, &args)
}

/// `finding close` closes a finding that is open and committed.
fn open_finding_for_close(repo: &Path) {
    ok(repo, &["finding", "add", "--change-id", "bump-client", "--round", "2", "--seq", "1", "--severity", "note", "--actor-id", "rev-1", "--summary", "open one"]);
}

fn error_of(out: &Output) -> Value {
    serde_json::from_str::<Value>(&text(&out.stdout)).unwrap_or_else(|e| panic!("{e}: {}{}", text(&out.stdout), text(&out.stderr)))["error"].clone()
}

#[test]
fn every_rev_default_resolves_head_and_records_the_code_it_saw() {
    let dir = repo();
    let r = dir.path();
    open_finding_for_close(r);
    let head = head(r);
    let tree = code_tree(r, None).unwrap();
    assert!(!tree.dirty);
    for (name, argv) in leaves() {
        let out = run_leaf(r, &argv, &[]);
        assert!(out.status.success(), "{name}: {}{}", text(&out.stdout), text(&out.stderr));
        if name.starts_with("divergence") {
            // The ledger keeps the latest divergence of a scenario, so each leaf's record is read as it lands.
            let all = records(r, "divergence");
            assert_eq!(all.len(), 1, "{name}");
            assert_eq!(all[0]["sha"], head.as_str(), "{name}");
            assert_eq!(all[0]["code_tree"], tree.tree, "{name}");
            assert_eq!(all[0]["worktree_dirty"], false, "{name}");
        }
    }

    let sha_of = |record: &Value, field: &str| record[field].as_str().map(str::to_string);
    let review = records(r, "review");
    assert_eq!(review.len(), 2, "scenario and change review");
    for record in &review {
        assert_eq!(sha_of(record, "pin").as_deref(), Some(head.as_str()));
        assert_eq!(record["code_tree"], tree.tree);
        assert_eq!(record["worktree_dirty"], false);
    }
    let evidence = records(r, "evidence_record");
    assert_eq!(evidence.len(), 2, "declared and audit evidence");
    for record in &evidence {
        assert_eq!(sha_of(record, "evidence_sha").as_deref(), Some(head.as_str()));
        assert_eq!(record["code_tree"], tree.tree);
        assert_eq!(record["worktree_dirty"], false);
    }
    let findings = records(r, "finding");
    let added = findings.iter().find(|f| f["round"] == 1).expect("the added finding");
    assert_eq!(sha_of(added, "reviewed_sha").as_deref(), Some(head.as_str()));
    assert_eq!(sha_of(added, "resolution_sha").as_deref(), Some(head.as_str()), "fixed defaults --resolution-rev to HEAD");
    assert_eq!(added["code_tree"], tree.tree);
    assert_eq!(added["resolution_code_tree"], tree.tree);
    let closed = findings.iter().filter(|f| f["round"] == 2).max_by_key(|f| f["at"].as_str().unwrap().to_string()).unwrap();
    assert_eq!(sha_of(closed, "resolution_sha").as_deref(), Some(head.as_str()));

    let payload = ok(r, &["evidence", "approval-payload", "--scenario-id", SCENARIO, "--project-id", "root", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z"]);
    assert!(payload.contains(&head), "the signed payload binds HEAD: {payload}");
}

#[test]
fn an_explicit_short_sha_branch_and_head_tilde_one_record_the_full_sha() {
    let dir = repo();
    let r = dir.path();
    let first = head(r);
    write(r, "src/lib.rs", "pub fn f() {}\n");
    commit_all(r, "two");
    let second = head(r);
    assert_ne!(first, second);
    for (reviewer, rev, expected) in [("a-short", &first[..8], &first), ("a-branch", "main", &second), ("a-tilde", "HEAD~1", &first), ("a-full", second.as_str(), &second)] {
        let out = canon(r, &["review", "scenario", SCENARIO, "--actor-id", reviewer, "--role", "reviewer", "--original-spec-ref", "docs/prd.md", "--rev", rev]);
        assert!(out.status.success(), "{rev}: {}", text(&out.stderr));
        let record = records(r, "review").into_iter().find(|rec| rec["reviewer"] == reviewer).unwrap();
        assert_eq!(record["pin"], expected.as_str(), "--rev {rev}");
        assert_eq!(record["worktree_dirty"], false, "a named commit is not a dirty working tree");
        assert_eq!(record["code_tree"], code_tree(r, Some(expected)).unwrap().tree, "--rev {rev} records the code of that commit");
    }
    let unknown = canon(r, &["divergence", "add", SCENARIO, "--actor-id", "x", "--role", "reviewer", "--rev", "no-such-rev", "--json"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(error_of(&unknown)["code"], "NO_COMMIT");
}

#[test]
fn a_dirty_working_tree_is_recorded_with_the_code_tree_of_the_commit_made_from_it() {
    let dir = repo();
    let r = dir.path();
    write(r, "src/main.rs", "fn main() { println!(\"changed\"); }\n");
    write(r, "src/new.rs", "pub fn untracked() {}\n");
    for (name, argv) in leaves().into_iter().filter(|(name, _)| matches!(*name, "review scenario" | "divergence add" | "evidence add")) {
        let out = run_leaf(r, &argv, &[]);
        assert!(out.status.success(), "{name} refuses nothing on uncommitted code: {}{}", text(&out.stdout), text(&out.stderr));
    }
    let review = records(r, "review").remove(0);
    assert_eq!(review["worktree_dirty"], true);
    assert_eq!(review["pin"], head(r), "the revision is still HEAD");
    for kind in ["divergence", "evidence_record"] {
        assert_eq!(records(r, kind)[0]["worktree_dirty"], true, "{kind}");
    }
    let reviewed_tree = review["code_tree"].as_str().unwrap().to_string();
    assert_ne!(reviewed_tree, code_tree(r, Some("HEAD")).unwrap().tree, "the review saw code HEAD does not hold");

    commit_all(r, "commit made from the reviewed tree");
    assert_eq!(code_tree(r, Some("HEAD")).unwrap().tree, reviewed_tree, "the commit has the code the review recorded");
    let now = code_tree(r, None).unwrap();
    assert_eq!(now.tree, reviewed_tree, "and so does the working tree");
    assert!(!now.dirty);

    write(r, "src/main.rs", "fn main() { panic!() }\n");
    assert_ne!(code_tree(r, None).unwrap().tree, reviewed_tree, "once the code changed, the working tree's code tree differs from the recorded one");
}

#[test]
fn outside_git_and_with_no_commit_every_leaf_refuses_no_commit_and_writes_nothing() {
    for with_git_but_no_commit in [false, true] {
        let dir = TempDir::new().unwrap();
        let r = dir.path();
        if with_git_but_no_commit {
            git(r, &["init", "-q", "-b", "main"]);
        }
        ok(r, &["init", "--repo", ".", "--no-policy"]);
        write(r, FEATURE, &feature());
        write(r, ".canon/changes/bump-client/brief.md", CHORE_BRIEF);
        ok(r, &["inventory", "sync"]);
        let before = files(&r.join(".canon/ledger")).len();
        for (name, argv) in leaves() {
            let out = run_leaf(r, &argv, &[]);
            assert_eq!(out.status.code(), Some(2), "{name}: {}{}", text(&out.stdout), text(&out.stderr));
            let error = error_of(&out);
            assert_eq!(error["code"], "NO_COMMIT", "{name}: {error}");
            assert!(error["hint"].as_str().is_some_and(|h| h.contains("git")), "{name} says how to fix it: {error}");
        }
        let payload = canon(r, &["evidence", "approval-payload", "--scenario-id", SCENARIO, "--project-id", "root", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z", "--json"]);
        assert_eq!(payload.status.code(), Some(2));
        assert_eq!(error_of(&payload)["code"], "NO_COMMIT");
        assert_eq!(files(&r.join(".canon/ledger")).len(), before, "a refusal writes nothing");
    }
}
