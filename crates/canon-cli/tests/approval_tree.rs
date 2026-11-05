//! 0.37 `canon-0-37-approval-evidence` (#130) through the real binary: an
//! approval records the code tree of its moment (`(code tree <id>)` before the
//! brief digest, outside it), and a chore whose recorded tree is the current
//! code tree asks for no evidence, only its change-level review. The ship
//! guard (`subject move --to shipped`), `canon status` and `canon gate check`
//! share the rule; an approval with no recorded tree keeps the 0.36 rule.
//! The dashboard's leg is `dashboard_briefs.rs` (`gate.approval-tree.03`).

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

use canon_model::{Actor, ApprovalRelay, Change, ChangeId, ChangeStatus, Envelope, RecordKind, RoleId, Subject, SubjectId, SubjectStatus};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::{Duration as Span, SecondsFormat, Utc};
use serde_json::Value;
use tempfile::TempDir;

const FEATURE: &str = ".canon/specs/kind=feature/area=shop/cart.feature";
const REQUIRE_REVIEW: &str = "spec_coverage:\n  require_evidence: true\n  require_review: {}\n";
const REPORT_A: &str = r#"<testsuite><testcase classname="cart" name="shop.cart.01 adds"/></testsuite>"#;
const REPORT_B: &str = r#"<testsuite><testcase classname="cart" name="shop.cart.02 counts"/></testsuite>"#;
const BATCH: &str = "canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer";
const CHORE: &str = "---\nkind: chore\n---\n\n# Bump the client\n\n## Intent\n\nBump the client\n\nThe client is two versions behind.\n\n## Category\n\ndependency\n\n## Invariants\n\n- Nothing changes. ==**No behavior changes.**==\n\n## Checks\n\n- The suite runs again. ==**The suite is the check.**==\n\n## Out of scope\n\nNone.\n";
const CHORE_BRIEF: &str = ".canon/changes/bump-client/brief.md";

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

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").current_dir(repo).args(args).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", text(&out.stderr));
}

fn write(repo: &Path, rel: &str, body: &str) {
    let path = repo.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn read(repo: &Path, rel: &str) -> String {
    std::fs::read_to_string(repo.join(rel)).unwrap()
}

fn commit_all(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", message]);
}

fn feature() -> String {
    let stamp = "# canon: {\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}";
    let mut text = format!("Feature: shop cart\n  {stamp}\n");
    for n in 1..=2 {
        text.push_str(&format!("\n  {stamp}\n  @subject:demo\n  @shop.cart.0{n}\n  Scenario: Cart behavior {n}\n    When the shopper acts\n    Then it works\n"));
    }
    text
}

fn actor(id: &str, role: &str) -> Actor {
    Actor::new(id, RoleId::parse(role).unwrap())
}

/// A git repo with code, two scenarios of subject `demo` evidenced by the
/// reports `a.xml` and `b.xml` and reviewed, the subject `verifying`, the
/// chore change `bump-client` adopted with an unapproved brief, and the whole
/// thing committed. `generated/**` is not code.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.name", "t"]);
    git(r, &["config", "user.email", "t@example.com"]);
    ok(r, &["init", "--repo", ".", "--no-policy"]);
    let config = read(r, "canon.yaml");
    write(r, "canon.yaml", &format!("{config}\ncode:\n  exclude: [\"docs/**\", \"*.md\", \"generated/**\"]\n"));
    write(r, ".canon/policy.yaml", REQUIRE_REVIEW);
    write(r, FEATURE, &feature());
    write(r, "src/lib.rs", "pub fn cart() {}\n");
    write(r, "a.xml", REPORT_A);
    write(r, "b.xml", REPORT_B);
    ok(r, &["inventory", "sync"]);

    let ledger = GitTier::new(r.join(".canon/ledger"));
    let at = Utc::now() - Span::seconds(600);
    for change in ["add-cart", "bump-client"] {
        let envelope = Envelope::new(1, RecordKind::Change, at, actor("importer", "planner"));
        ledger.write(&Change::new(envelope, ChangeId::parse(change).unwrap(), "Seeded", "why", ChangeStatus::InProgress)).unwrap();
    }
    let envelope = Envelope::new(1, RecordKind::Subject, at, actor("canon", "implementer"));
    let subject = Subject::new(envelope, SubjectId::parse("demo").unwrap(), "Demo", "s", "dev", SubjectStatus::Verifying, RoleId::parse("implementer").unwrap())
        .with_change_ids(["add-cart", "bump-client"].iter().map(|c| ChangeId::parse(*c).unwrap()).collect());
    ledger.write(&subject).unwrap();
    write(r, ".canon/changes/add-cart/brief.md", "## Why\n\nSeeded.\n\nApproved: Journey 2026-10-12 — \"go\"\n");
    write(r, CHORE_BRIEF, CHORE);
    commit_all(r, "setup");

    ok(r, &["evidence", "add", "--all-scenarios", "--report", "junit:a.xml", "--report", "junit:b.xml", "--role", "implementer"]);
    for scenario in ["shop.cart.01", "shop.cart.02"] {
        ok(r, &["review", "scenario", scenario, "--actor", "reviewer-2", "--role", "reviewer", "--original-spec-ref", "docs/prd.md"]);
    }
    commit_all(r, "baseline");
    dir
}

/// Another change edits the code after the scenarios' evidence: their latest
/// evidence is on an older code tree than the current one.
fn other_change_edits_code(repo: &Path) {
    write(repo, "src/lib.rs", "pub fn cart() { /* another change */ }\n");
    commit_all(repo, "another change");
}

/// The human approves the chore with `canon brief approve`, at least a second
/// after the evidence (an approval's time is to the second).
fn approve(repo: &Path) {
    std::thread::sleep(Duration::from_millis(1100));
    ok(repo, &["brief", "approve", "bump-client", "--approver", "Journey", "--note", "go"]);
}

/// A chore approval written as 0.36 wrote it: no recorded tree, no digest.
fn approve_as_before_0_37(repo: &Path) {
    let at = (Utc::now() + Span::seconds(3)).to_rfc3339_opts(SecondsFormat::Secs, true);
    write(repo, CHORE_BRIEF, &format!("{CHORE}\nApproved: Journey {at} — \"go\"\n"));
}

fn approval_line(brief: &str) -> String {
    brief.lines().find_map(|l| l.strip_prefix("Approved: ")).unwrap_or_else(|| panic!("no Approved: line in {brief}")).to_string()
}

fn code_tree_now(repo: &Path) -> String {
    canon_cli::git_rev::code_tree(repo, None).expect("the code tree").tree
}

fn ship(repo: &Path) -> Output {
    canon(repo, &["subject", "move", "demo", "--to", "shipped"])
}

fn subject_status(repo: &Path) -> String {
    let report: Value = serde_json::from_str(&ok(repo, &["query", "--kind", "subject", "--json"])).unwrap();
    report["data"]["records"][0]["status"].as_str().unwrap().to_string()
}

/// The steps `canon status` lists about this subject, from its human form: a
/// step that holds a `<placeholder>` is not a `meta.next` entry in JSON mode.
fn status_steps(repo: &Path) -> Vec<Value> {
    let out = canon(repo, &["status"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let text = text(&out.stdout);
    let Some((_, block)) = text.split_once("\nnext:\n") else { return Vec::new() };
    let mut steps = Vec::new();
    let mut why = None;
    for line in block.lines() {
        if let Some(reason) = line.strip_prefix("  # ") {
            why = Some(reason.to_string());
        } else if let (Some(command), Some(reason)) = (line.strip_prefix("  "), why.take()) {
            let (actor, command) = command.strip_prefix("human: ").map_or(("agent", command), |rest| ("human", rest));
            if reason.starts_with("demo") {
                steps.push(serde_json::json!({"actor": actor, "command": command, "why": reason}));
            }
        }
    }
    steps
}

fn evidence_steps(steps: &[Value]) -> Vec<&Value> {
    steps.iter().filter(|s| s["command"].as_str().unwrap().contains("evidence add")).collect()
}

/// The `uncovered-cell` violations `canon gate check` names for subject `demo`.
fn gate_evidence_violations(repo: &Path) -> Vec<String> {
    let out = canon(repo, &["gate", "check", "--json"]);
    let report: Value = serde_json::from_str(&text(&out.stdout)).unwrap_or_else(|e| panic!("gate check --json: {e}: {}{}", text(&out.stdout), text(&out.stderr)));
    report["data"]["violations"]
        .as_array()
        .unwrap_or_else(|| panic!("data.violations[]: {report}"))
        .iter()
        .filter(|v| v["class"] == "uncovered-cell" && v["subject"] == "demo")
        .map(|v| v["message"].as_str().unwrap().to_string())
        .collect()
}

/// The `unreviewed-promotion` violations `canon gate check` names for subject `demo`.
fn gate_review_violations(repo: &Path) -> Vec<String> {
    let out = canon(repo, &["gate", "check", "--json"]);
    let report: Value = serde_json::from_str(&text(&out.stdout)).unwrap();
    report["data"]["violations"].as_array().unwrap().iter().filter(|v| v["class"] == "unreviewed-promotion" && v["subject"] == "demo").map(|v| v["message"].as_str().unwrap().to_string()).collect()
}

fn status_json(repo: &Path) -> Value {
    serde_json::from_str(&ok(repo, &["status", "--json"])).unwrap()
}

/// The `briefs[]` entry of the chore in subject `demo`.
fn chore_entry(status: &Value) -> Value {
    let demo = status["data"]["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "demo").unwrap_or_else(|| panic!("no subject demo: {status}"));
    demo["briefs"].as_array().unwrap().iter().find(|b| b["change"] == "bump-client").unwrap_or_else(|| panic!("no brief of bump-client: {demo}")).clone()
}

const REVIEW_REFUSAL: &str = "no review of change `bump-client`";
const GAP_REFUSAL: &str = "no evidence recorded after";

/// gate.approval-tree.01: the approval records the code tree of its moment
/// outside the digest; the same approval over the same code writes nothing;
/// approving over other code (uncommitted too) records the new tree.
#[test]
fn gate_approval_tree_01_approving_records_the_code_tree_outside_the_digest_and_again_the_new_one() {
    let dir = repo();
    let r = dir.path();
    approve(r);
    let brief = read(r, CHORE_BRIEF);
    let line = approval_line(&brief);
    let first = canon_cli::git_rev::code_tree(r, None).unwrap().tree;
    assert_eq!(canon_cli::brief::approval_tree(&line), Some(first.as_str()), "{line}");
    let digest = canon_cli::brief::brief_digest(&brief);
    assert!(line.ends_with(&format!("(code tree {first}) (brief {digest})")), "the marker sits before the digest: {line}");
    assert!(matches!(canon_cli::brief::approval_of(&brief), canon_cli::brief::Approval::Bound { .. }), "{line}");
    // The marker is outside the digest: the brief without it binds the same digest.
    let without = brief.replace(&format!(" (code tree {first})"), "");
    assert_eq!(canon_cli::brief::brief_digest(&without), digest);
    assert!(matches!(canon_cli::brief::approval_of(&without), canon_cli::brief::Approval::Bound { .. }));

    // The same approval over the same code writes nothing, a second later too.
    std::thread::sleep(Duration::from_millis(1100));
    ok(r, &["brief", "approve", "bump-client", "--approver", "Journey", "--note", "go"]);
    assert_eq!(read(r, CHORE_BRIEF), brief, "a repeat over the same code writes nothing");

    // Other code, not even committed: the approval records the tree it saw.
    write(r, "src/lib.rs", "pub fn cart() { /* edited, uncommitted */ }\n");
    ok(r, &["brief", "approve", "bump-client", "--approver", "Journey", "--note", "go"]);
    let again = read(r, CHORE_BRIEF);
    let second = code_tree_now(r);
    assert_ne!(first, second);
    assert_eq!(canon_cli::brief::approval_tree(&approval_line(&again)), Some(second.as_str()), "{again}");
    assert_eq!(again.matches("\nApproved: ").count(), 1, "re-approving leaves exactly one line: {again}");
    assert!(canon_cli::brief::approval_of(&again).approved());
}

/// gate.approval-tree.02: no git work tree, or no commit, records no tree and
/// raises no error.
#[test]
fn gate_approval_tree_02_an_approval_without_a_git_work_tree_or_a_commit_records_no_tree() {
    for init_git in [false, true] {
        let dir = TempDir::new().unwrap();
        let r = dir.path();
        if init_git {
            git(r, &["init", "-q", "-b", "main"]);
        }
        ok(r, &["init", "--providers", "none"]);
        ok(r, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
        ok(r, &["change", "new", "bump-client", "--subject", "demo", "--title", "Bump the client", "--change-kind", "chore"]);
        write(r, CHORE_BRIEF, CHORE);
        let out = canon(r, &["brief", "approve", "bump-client", "--approver", "Journey", "--note", "go"]);
        assert!(out.status.success(), "git work tree: {init_git}: {}{}", text(&out.stdout), text(&out.stderr));
        let brief = read(r, CHORE_BRIEF);
        let line = approval_line(&brief);
        assert!(!line.contains("(code tree"), "git work tree without a commit: {init_git}: {line}");
        assert_eq!(canon_cli::brief::approval_tree(&line), None);
        assert!(matches!(canon_cli::brief::approval_of(&brief), canon_cli::brief::Approval::Bound { .. }), "{line}");
    }
}

/// gate.approval-tree.04: the chore is approved over code another change edited after the evidence: the
/// scenarios' evidence is on an older tree, and the chore asks for none of it.
#[test]
fn gate_approval_tree_04_a_chore_over_unchanged_code_asks_for_no_evidence_only_its_change_review() {
    let dir = repo();
    let r = dir.path();
    other_change_edits_code(r);
    approve(r);

    let out = ship(r);
    assert_eq!(out.status.code(), Some(2), "the chore still needs its review: {}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains(REVIEW_REFUSAL), "{err}");
    assert!(!err.contains(GAP_REFUSAL) && !err.contains("shop.cart"), "no evidence is asked for, though it is on an older tree: {err}");
    let steps = status_steps(r);
    assert!(steps[0]["command"].as_str().unwrap().starts_with("canon review change bump-client"), "{steps:?}");
    assert!(evidence_steps(&steps).is_empty(), "{steps:?}");

    ok(r, &["review", "change", "bump-client", "--actor", "reviewer-3", "--role", "reviewer", "--original-spec-ref", ".canon/changes/bump-client/brief.md"]);
    let out = ship(r);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(subject_status(r), "shipped");
}

/// gate.approval-tree.05: without the change review the move is refused with
/// exit 2 and the review command leads; a brief edited after the approval needs a new review.
#[test]
fn gate_approval_tree_05_without_the_change_review_the_move_is_refused_with_exit_2_and_the_review_command() {
    let dir = repo();
    let r = dir.path();
    other_change_edits_code(r);
    approve(r);

    let out = ship(r);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    let problem = err.lines().find(|l| l.contains(REVIEW_REFUSAL)).unwrap_or_else(|| panic!("the refusal names the missing review: {err}"));
    assert!(problem.contains("canon review change bump-client"), "the problem line carries the command that records the review: {problem}");
    assert!(!err.contains("evidence add"), "{err}");
    let steps = status_steps(r);
    assert!(steps[0]["command"].as_str().unwrap().starts_with("canon review change bump-client"), "{steps:?}");

    // A review of the brief as approved, then the brief is edited and approved again: that review is stale.
    ok(r, &["review", "change", "bump-client", "--actor", "reviewer-3", "--role", "reviewer", "--original-spec-ref", ".canon/changes/bump-client/brief.md"]);
    write(r, CHORE_BRIEF, &read(r, CHORE_BRIEF).replace("The client is two versions behind.", "The client is three versions behind."));
    approve(r);
    let out = ship(r);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains("canon review change bump-client") && !err.contains(GAP_REFUSAL), "the edited brief needs a new review and still no evidence: {err}");
}

/// gate.approval-tree.14: the quire shape. A chore is approved, reviewed, its
/// code is committed with no evidence, and the same brief is approved again over
/// that code: the approval records the new tree and asks for no evidence, but the
/// review saw the code before and no longer counts.
#[test]
fn gate_approval_tree_14_a_chore_approved_again_over_code_that_came_after_its_review_asks_for_its_review_again() {
    let dir = repo();
    let r = dir.path();
    approve(r);
    let review = ["review", "change", "bump-client", "--actor", "reviewer-3", "--role", "reviewer", "--original-spec-ref", ".canon/changes/bump-client/brief.md"];
    ok(r, &review);
    let human = ok(r, &["status"]);
    assert!(human.contains("so it asks for no evidence") && !human.contains("review is still asked"), "a current review of this code: the line does not ask for it: {human}");

    write(r, "src/lib.rs", "pub fn cart() { /* the chore's code, after its review */ }\n");
    commit_all(r, "the chore's code");
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL), "before the re-approval the chore's code asks for the evidence: {err}");

    approve(r);
    assert_eq!(canon_cli::brief::approval_tree(&approval_line(&read(r, CHORE_BRIEF))), Some(code_tree_now(r).as_str()));
    let out = ship(r);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(!err.contains(GAP_REFUSAL), "the chore asks for no evidence: {err}");
    assert!(err.contains("saw other code than the approval it stands on") && err.contains("canon review change bump-client"), "the review of other code no longer counts: {err}");
    let gate = gate_review_violations(r);
    assert!(gate.len() == 1 && gate[0].contains("saw other code"), "gate check names the same review: {gate:?}");
    assert!(gate_evidence_violations(r).is_empty());
    let steps = status_steps(r);
    assert!(steps[0]["command"].as_str().unwrap().starts_with("canon review change bump-client"), "{steps:?}");
    assert!(ok(r, &["status"]).contains("so it asks for no evidence; its change-level review is still asked"));

    ok(r, &review);
    let human = ok(r, &["status"]);
    assert!(human.contains("so it asks for no evidence") && !human.contains("review is still asked"), "{human}");
    let out = ship(r);
    assert!(out.status.success(), "a review of the new code stands: {}", text(&out.stderr));
    assert_eq!(subject_status(r), "shipped");
}

/// gate.approval-tree.06: code changed after the approval: the scenarios and
/// one batch command; the command closes the gap.
#[test]
fn gate_approval_tree_06_a_code_change_after_the_approval_asks_for_the_scenarios_with_one_batch_command() {
    let dir = repo();
    let r = dir.path();
    approve(r);
    write(r, "src/lib.rs", "pub fn cart() { /* edited after the approval */ }\n");

    let out = ship(r);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains(GAP_REFUSAL) && err.contains("`shop.cart.01`") && err.contains("`shop.cart.02`"), "{err}");
    let hint = err.lines().find(|l| l.starts_with("  hint: ")).unwrap_or_else(|| panic!("a refusal carries a hint: {err}"));
    assert!(hint.contains(&format!("`{BATCH}`")), "the hint leads with the batch command: {hint}");
    assert!(!err.contains("--scenario"), "never a per-scenario loop: {err}");
    let steps = status_steps(r);
    assert_eq!(steps[0]["command"], BATCH, "{steps:?}");

    let words: Vec<&str> = BATCH.split(' ').skip(1).collect();
    ok(r, &words);
    let err = text(&ship(r).stderr);
    assert!(!err.contains(GAP_REFUSAL), "the batch closed the gap: {err}");
    assert!(err.contains(REVIEW_REFUSAL), "what remains is the chore's review: {err}");
}

/// gate.approval-tree.07: one edited file after the approval brings the
/// requirement back; a change confined to what `code.exclude` drops does not.
#[test]
fn gate_approval_tree_07_one_changed_file_brings_the_requirement_back_and_an_excluded_one_does_not() {
    let dir = repo();
    let r = dir.path();
    other_change_edits_code(r);
    approve(r);
    write(r, "src/lib.rs", &format!("{}// one more line\n", read(r, "src/lib.rs")));
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL) && err.contains("`shop.cart.01`") && err.contains("`shop.cart.02`"), "one edited file makes the tree differ: {err}");
    let hint = err.lines().find(|l| l.starts_with("  hint: ")).unwrap();
    assert!(hint.contains(&format!("`{BATCH}`")), "{hint}");

    let dir = repo();
    let r = dir.path();
    other_change_edits_code(r);
    approve(r);
    write(r, "generated/client.rs", "// generated, not code\n");
    write(r, "docs/changes.md", "# Changes\n");
    let out = ship(r);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains(REVIEW_REFUSAL) && !err.contains(GAP_REFUSAL), "files code.exclude drops leave the tree as it was: {err}");
}

/// gate.approval-tree.08: an approval with no recorded tree is judged as in
/// 0.36: over code that changed since the evidence it asks for every scenario,
/// over the evidence's own code tree it asks for none.
#[test]
fn gate_approval_tree_08_an_approval_with_no_recorded_tree_is_judged_as_in_0_36() {
    let dir = repo();
    let r = dir.path();
    other_change_edits_code(r);
    approve_as_before_0_37(r);
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL) && err.contains("`shop.cart.01`") && err.contains("`shop.cart.02`"), "evidence on an older tree is asked for, as 0.36 did: {err}");
    let hint = err.lines().find(|l| l.starts_with("  hint: ")).unwrap();
    assert!(hint.contains(&format!("`{BATCH}`")), "{hint}");

    let dir = repo();
    let r = dir.path();
    approve_as_before_0_37(r);
    write(r, "docs/guide.md", "# Guide\n");
    let err = text(&ship(r).stderr);
    assert!(err.contains(REVIEW_REFUSAL) && !err.contains(GAP_REFUSAL), "evidence on the current tree needs nothing, as in 0.36: {err}");
}

/// gate.approval-tree.09: lines written before 0.37 (with a digest, relayed
/// or not) are read by status and gate check with no error and no tree.
#[test]
fn gate_approval_tree_09_a_line_from_before_0_37_is_read_with_no_error_and_no_new_information() {
    let dir = repo();
    let r = dir.path();
    other_change_edits_code(r);
    let at = (Utc::now() + Span::seconds(3)).to_rfc3339_opts(SecondsFormat::Secs, true);
    for relay in [None, Some(ApprovalRelay { role: "planner".into(), actor: "plan-1".into(), tool: "ask".into(), approver: None, answer: None })] {
        let approved = canon_cli::brief_approve::approved_text(CHORE, "Journey", &at, "go", relay.as_ref(), None).unwrap();
        write(r, CHORE_BRIEF, &approved);
        assert!(!approval_line(&approved).contains("code tree"));

        let status = status_json(r);
        let entry = chore_entry(&status);
        assert_eq!(entry["status"], "approved", "{entry}");
        assert!(entry.get("treeUnchanged").is_none(), "{entry}");
        assert_eq!(entry["relay"].get("role").map(|v| v.as_str().unwrap().to_string()), relay.as_ref().map(|x| x.role.clone()), "{entry}");
        let human = ok(r, &["status"]);
        assert!(!human.contains("approved on") && !human.contains("code tree"), "{human}");

        let gate = canon(r, &["gate", "check", "--json"]);
        let report: Value = serde_json::from_str(&text(&gate.stdout)).unwrap();
        assert_eq!(report["data"]["clean"], false, "the evidence gap of a 0.36 line stands: {report}");
        assert!(gate.status.code() == Some(1) && report["error"]["code"] == "GATE_CHECK_FAILED", "{report}");
    }
}

/// gate.approval-tree.10: the move to shipped, status and gate check state
/// the same requirement.
#[test]
fn gate_approval_tree_10_ship_status_and_gate_check_state_the_same_requirement() {
    // Over unchanged code: none of the three asks for evidence; each asks for the review.
    let dir = repo();
    let r = dir.path();
    other_change_edits_code(r);
    approve(r);
    let err = text(&ship(r).stderr);
    assert!(err.contains(REVIEW_REFUSAL) && !err.contains(GAP_REFUSAL), "{err}");
    assert!(evidence_steps(&status_steps(r)).is_empty(), "{:?}", status_steps(r));
    let status = status_json(r);
    let next: Vec<Value> = status["meta"].get("next").and_then(Value::as_array).cloned().unwrap_or_default();
    assert!(next.iter().all(|n| !n["command"].as_str().unwrap().contains("evidence add")), "{status}");
    assert!(gate_evidence_violations(r).is_empty(), "{:?}", gate_evidence_violations(r));
    assert!(gate_review_violations(r).iter().any(|m| m.contains("bump-client")), "{:?}", gate_review_violations(r));

    // After a code change: all three name the same scenarios and the same batch command.
    write(r, "src/lib.rs", "pub fn cart() { /* the chore touched code after all */ }\n");
    let err = text(&ship(r).stderr);
    let violations = gate_evidence_violations(r);
    assert_eq!(violations.len(), 1, "{violations:?}");
    for scenario in ["`shop.cart.01`", "`shop.cart.02`"] {
        assert!(err.contains(scenario) && violations[0].contains(scenario), "{scenario}: {err} / {violations:?}");
    }
    assert!(err.contains(&format!("`{BATCH}`")) && violations[0].contains(&format!("`{BATCH}`")), "{err} / {violations:?}");
    let steps = status_steps(r);
    assert_eq!(steps[0]["command"], BATCH, "{steps:?}");
    assert_eq!(evidence_steps(&steps).len(), 1, "{steps:?}");
    assert_eq!(status_json(r)["meta"]["next"][0]["command"], format!("{BATCH} --json"));
}

/// gate.approval-tree.11: status says why a chore asks for nothing, and says
/// nothing of the kind over changed code.
#[test]
fn gate_approval_tree_11_status_says_why_a_chore_asks_for_nothing_and_not_over_changed_code() {
    let dir = repo();
    let r = dir.path();
    other_change_edits_code(r);
    approve(r);
    let human = ok(r, &["status"]);
    assert!(human.contains("the code tree is the one it was approved on, so it asks for no evidence"), "{human}");
    let entry = chore_entry(&status_json(r));
    assert_eq!(entry["treeUnchanged"], true, "{entry}");
    assert!(entry["approval"].as_str().unwrap().contains("(code tree "), "the approval line is shown whole: {entry}");

    write(r, "src/lib.rs", "pub fn cart() { /* edited after the approval */ }\n");
    let human = ok(r, &["status"]);
    assert!(!human.contains("approved on"), "{human}");
    let status = status_json(r);
    assert!(chore_entry(&status).get("treeUnchanged").is_none(), "{status}");
    assert_eq!(status["meta"]["next"][0]["command"], format!("{BATCH} --json"), "{status}");
}

fn unreleased() -> String {
    let changelog = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).expect("the CHANGELOG");
    let start = changelog.find("\n## Unreleased").or_else(|| changelog.find("\n## 0.37.0")).expect("the CHANGELOG has an Unreleased section");
    let end = changelog[start + 1..].find("\n## ").map_or(changelog.len(), |at| start + 1 + at);
    changelog[start..end].to_string()
}

/// What the CHANGELOG section lacks about this change: the Changed entry, the
/// marker of the approval line and the behavior of older approvals.
fn changelog_problems(section: &str) -> Vec<String> {
    let changed: String = section.split("\n### ").find(|part| part.starts_with("Changed")).unwrap_or_default().to_string();
    let entry: Vec<&str> = changed.lines().filter(|l| l.contains("canon-0-37-approval-evidence")).collect();
    let mut problems = Vec::new();
    if entry.is_empty() {
        problems.push("no entry of `canon-0-37-approval-evidence` under `### Changed`".to_string());
    }
    let entry = entry.join("\n");
    for (what, needle) in [
        ("the rule: a chore approval over an unchanged code tree asks for no evidence", "asks for no evidence"),
        ("the unchanged code tree", "code tree"),
        ("the marker of the approval line, `(code tree <id>)`", "(code tree <id>)"),
        ("that the marker is outside the digest", "outside the brief digest"),
        ("that an approval written before 0.37 behaves as it did", "written before 0.37"),
        ("that a review of other code no longer counts", "review of other code"),
        ("that the change review is still asked", "change-level review"),
    ] {
        if !entry.contains(needle) {
            problems.push(format!("the entry does not say {what} (`{needle}`)"));
        }
    }
    let hand_edit = "edited by hand like the approval time";
    if !changed.contains(hand_edit) {
        problems.push(format!("`### Changed` does not say that the marker, outside the digest, can be edited by hand like the approval time (`{hand_edit}`)"));
    }
    problems
}

/// gate.approval-tree.12: the CHANGELOG says what the gate asks for less, and
/// what the approval line records now.
#[test]
fn gate_approval_tree_12_the_changelog_names_the_rule_and_the_new_information_of_the_approval_line() {
    assert_eq!(changelog_problems(&unreleased()), Vec::<String>::new());
}

/// gate.approval-tree.13: a CHANGELOG without the entry, or without a piece of
/// it, is named by the release check.
#[test]
fn gate_approval_tree_13_a_changelog_without_the_entry_is_named_by_the_release_check() {
    let section = unreleased();
    assert_eq!(changelog_problems("## Unreleased\n\n### Changed\n\n- Something else.\n").len(), 1 + 7 + 1, "an empty Changed section names the entry, each piece of it and the hand-edit line");
    let without_entry: String = section.lines().filter(|l| !l.contains("canon-0-37-approval-evidence")).collect::<Vec<_>>().join("\n");
    assert!(changelog_problems(&without_entry).iter().any(|p| p.contains("no entry")), "{:?}", changelog_problems(&without_entry));
    let without_marker = section.replace("(code tree <id>)", "the tree marker");
    assert!(changelog_problems(&without_marker).iter().any(|p| p.contains("(code tree <id>)")), "{:?}", changelog_problems(&without_marker));
    let without_old = section.replace("written before 0.37", "written earlier");
    assert!(changelog_problems(&without_old).iter().any(|p| p.contains("written before 0.37")), "{:?}", changelog_problems(&without_old));
    let without_hand_edit = section.replace("edited by hand like the approval time", "edited");
    assert!(changelog_problems(&without_hand_edit).iter().any(|p| p.contains("edited by hand like the approval time")), "{:?}", changelog_problems(&without_hand_edit));
}
