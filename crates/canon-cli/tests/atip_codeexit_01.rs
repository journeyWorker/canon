//! `canon-0-36-codes-exit`, scenario `atip.codeexit.01`: every refusal of the brief's table exits 2 with its new code and no `data`, and the
//! work tree and `.canon/` hold the bytes they held. Each refusal here is provoked for real, against the built `canon` binary, from the
//! recipe the command's own tests use; the registry (`canon_cli::cli_error::all_codes()`) says the `retryable` of each.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

// ── The binary, the registry, the snapshot ──

fn canon_with(dir: &Path, args: &[&str], role: Option<&str>, path: Option<&Path>) -> Output {
    let mut command = hermetic::canon();
    command.args(args).arg("--json").current_dir(dir).env_remove("CANON_ROLE").env_remove("CANON_ACTOR");
    if let Some(role) = role {
        command.env("CANON_ROLE", role);
    }
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command.output().expect("spawn canon")
}

fn canon(dir: &Path, args: &[&str], role: Option<&str>) -> Output {
    canon_with(dir, args, role, None)
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

fn ok(dir: &Path, args: &[&str], role: Option<&str>) -> Value {
    let out = canon(dir, args, role);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    body(&out)
}

/// `args` with its exit checked, for the steps that set a repo up (a conformance case's steps say each exit).
fn step(dir: &Path, args: &[&str], exit: i32) {
    let out = canon(dir, args, None);
    assert_eq!(out.status.code(), Some(exit), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// The registry's row of `code`: its exit and whether the same call can be retried.
fn registered(code: &str) -> &'static canon_cli::cli_error::CodeInfo {
    canon_cli::cli_error::all_codes().iter().find(|info| info.code == code).unwrap_or_else(|| panic!("{code} is not in the registry"))
}

/// Every file of `dir` (the work tree and `.canon/`, not `.git`) with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().display().to_string(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// Runs `args` (with `path` as the `PATH` when given) and checks the refusal of the brief's table: exit 2, `error.code == code`,
/// `error.retryable` the registry's, no `data`, and every file of `dir` as it was; returns the document.
fn refused_with(dir: &Path, role: Option<&str>, path: Option<&Path>, args: &[&str], code: &str) -> Value {
    let info = registered(code);
    assert_eq!(info.exit, 2, "{code}: the table's exit is 2");
    let before = snapshot(dir);
    let out = canon_with(dir, args, role, path);
    let b = body(&out);
    assert_eq!(out.status.code(), Some(2), "{args:?}: {b}");
    assert_eq!(b["ok"], Value::Bool(false), "{args:?}: {b}");
    assert_eq!(b["error"]["code"].as_str(), Some(code), "{args:?}: {b}");
    assert_eq!(b["error"]["retryable"].as_bool(), Some(info.retryable), "{code}: the registry's retryable: {b}");
    assert!(b.get("data").is_none(), "{code}: a refusal carries no data: {b}");
    assert!(snapshot(dir) == before, "{code} {args:?}: the refusal left the work tree or .canon/ changed");
    b
}

fn refused(dir: &Path, role: Option<&str>, args: &[&str], code: &str) -> Value {
    refused_with(dir, role, None, args, code)
}

fn git_in(dir: &Path, args: &[&str]) {
    let out = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn git_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        git_in(dir.path(), args);
    }
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The `repo/` fixture of the conformance case `name`, copied, `git init`ed and committed as the conformance replay does.
fn case_repo(name: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases").join(name).join("repo"), dir.path());
    for args in [&["init", "-q", "-b", "main"][..], &["add", "-A"], &["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "--allow-empty", "-m", "fixture"]] {
        git_in(dir.path(), args);
    }
    dir
}

// ── The repos ──

fn brief_path(repo: &Path) -> PathBuf {
    repo.join(".canon/changes/add-login/brief.md")
}

/// A complete English PRD brief with flow steps `F1`, `F2`; `extra` is appended.
fn en_brief(extra: &str) -> String {
    format!(
        "# Add login\n\n\
## Intent\n\nAdd login\n\nVisitors lose their cart when they switch devices.\n\n\
## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor submits a wrong password → sees an error → can retry.\n\n\
## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n### F2 Failed submit\n\nFive failures lock the form for thirty seconds.\n\n\
## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie.\n- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==\n\n\
## Questions and assumptions\n\nNone.\n\n\
## Out of scope\n\nSocial sign-in.\n\n{extra}"
    )
}

/// A repo with subject `demo`, change `add-login` with a clean brief (not yet approved) and one task.
fn repo_with_clean_brief() -> TempDir {
    let dir = git_repo();
    let repo = dir.path();
    ok(repo, &["init", "--no-harness"], None);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"], Some("planner"));
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"], Some("planner"));
    std::fs::write(brief_path(repo), en_brief("")).unwrap();
    let tasks = repo.join(".canon/changes/add-login/tasks.md");
    let mut text = std::fs::read_to_string(&tasks).unwrap_or_else(|_| "# Tasks: Add login\n".to_string());
    text.push_str("\n- [ ] 1 Write the failure-path scenarios\n");
    std::fs::write(&tasks, text).unwrap();
    ok(repo, &["ingest", "plans"], None);
    dir
}

const ATTEST: [&str; 12] = ["evidence", "add", "--task", "add-login#1", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor", "impl"];

/// A repo `canon init`ed on a commit, with a run of `dev` begun; returns the repo and the run id.
fn repo_with_run() -> (TempDir, String) {
    let dir = git_repo();
    ok(dir.path(), &["init", "--no-harness"], None);
    let begun = ok(dir.path(), &["dispatch", "begin", "--role", "dev", "--regime", "dev/canon/join-spine/9c93d024b1a2"], None);
    let run_id = begun["data"]["runId"].as_str().expect("a run id").to_string();
    (dir, run_id)
}

/// A repo `canon init`ed on a commit that looks like canon's checkout to the dashboard (`packages/dashboard`, a page in `dist/`).
fn repo_with_dashboard() -> TempDir {
    let dir = git_repo();
    let l = dir.path();
    ok(l, &["init", "--no-harness"], None);
    let app = l.join("packages/dashboard");
    std::fs::create_dir_all(app.join("dist/snapshot")).unwrap();
    std::fs::write(app.join("package.json"), r#"{"name":"@canon-dev/dashboard"}"#).unwrap();
    for page in ["index.html", "briefs.html"] {
        std::fs::write(app.join("dist").join(page), format!("<html>canon dashboard {page}</html>")).unwrap();
    }
    std::fs::write(app.join("dist/snapshot/manifest.json"), r#"{"demo":true}"#).unwrap();
    dir
}

/// A repo with a `canon.yaml` whose `plans.sources` names `root`, and the openspec change `widget` under `openspec/`.
fn repo_with_openspec(root: &str) -> TempDir {
    let dir = git_repo();
    let d = dir.path();
    std::fs::write(d.join("canon.yaml"), format!("tiers:\n  local: {{ backend: git, root: .canon/ledger }}\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: {root}\n")).unwrap();
    std::fs::create_dir_all(d.join("openspec/changes/widget")).unwrap();
    std::fs::write(d.join("openspec/changes/widget/proposal.md"), "## Why\n\nWidgets.\n").unwrap();
    std::fs::write(d.join("openspec/changes/widget/tasks.md"), "- [ ] 1.1 wire it\n").unwrap();
    dir
}

// ── Brief and task ──

/// `BRIEF_APPROVAL_REQUIRED`, `BRIEF_APPROVAL_STALE`, `TASK_EVIDENCE_REQUIRED`: the preconditions of a write.
#[test]
fn atip_codeexit_01_the_brief_and_the_task_refusals_are_exit_2_and_write_nothing() {
    let dir = repo_with_clean_brief();
    let repo = dir.path();
    refused(repo, Some("implementer"), &ATTEST, "BRIEF_APPROVAL_REQUIRED");
    refused(repo, None, &["task", "done", "add-login#1"], "TASK_EVIDENCE_REQUIRED");

    // The precondition fixed: the same call is taken; an edit after the approval is stale.
    ok(repo, &["brief", "approve", "--approver", "Ana", "--note", "Go."], None);
    ok(repo, &ATTEST, Some("implementer"));
    let text = std::fs::read_to_string(brief_path(repo)).unwrap();
    std::fs::write(brief_path(repo), text.replace("Social sign-in.", "Social sign-in, later.")).unwrap();
    refused(repo, Some("implementer"), &ATTEST, "BRIEF_APPROVAL_STALE");
}

/// `TASK_EVIDENCE_INVALID` and `EVIDENCE_CONTRACT_CONFLICT`: a task whose typed evidence atom the gate cannot take, and a record that
/// differs from the task's typed contract.
#[test]
fn atip_codeexit_01_the_task_evidence_refusals_are_exit_2_and_write_nothing() {
    let atom = |kind: &str, evidence_ref: &str| format!("- id: it-typed#1\n  tag: task\n  attrs:\n    desc: \"typed pilot task\"\n    status: open\n    evidence:\n      kind: {kind}\n      ref: \"{evidence_ref}\"\n");
    let repo_with_atom = |kind: &str, evidence_ref: &str| {
        let dir = git_repo();
        let repo = dir.path();
        std::fs::write(repo.join("canon.yaml"), "tiers:\n  local:\n    backend: git\n    root: .canon/ledger\nplans:\n  sources:\n    - dialect: openspec\n      root: .\n").unwrap();
        std::fs::create_dir_all(repo.join(".canon/vocab/canon.core/directives")).unwrap();
        std::fs::write(repo.join(".canon/policy.yaml"), "trust_required:\n  test-run: agent\n").unwrap();
        let core = repo.join(".canon/vocab/canon.core");
        std::fs::write(core.join("plugin.yaml"), "id: canon.core\nversion: \"0.1.0\"\nkind: core\nexports:\n  directives: directives/\n  enums: enums.yaml\n").unwrap();
        std::fs::write(
            core.join("directives/task.yaml"),
            "directives:\n  - name: task\n    attrs:\n      - name: desc\n        type: string\n        required: true\n      - name: status\n        type: { domain: task-status }\n        required: true\n      - name: evidence\n        type: evidence\n        required: true\n",
        )
        .unwrap();
        std::fs::write(core.join("enums.yaml"), "enums:\n  task-status:\n    - open\n    - done\n").unwrap();
        std::fs::create_dir_all(repo.join("openspec/changes/it-typed")).unwrap();
        std::fs::write(repo.join("openspec/changes/it-typed/tasks.md"), "- [ ] 1 Do the typed thing\n").unwrap();
        std::fs::write(repo.join("openspec/changes/it-typed/tasks.vocab.yaml"), atom(kind, evidence_ref)).unwrap();
        std::fs::write(repo.join("openspec/changes/it-typed/proposal.md"), "## Why\n\nA typed task.\n").unwrap();
        dir
    };

    // TASK_EVIDENCE_INVALID: the atom declares an `evidence.kind` outside the policy-derived domain.
    let dir = repo_with_atom("not-a-real-kind", "whatever");
    let b = refused(dir.path(), None, &["task", "done", "it-typed#1", "--repo", "."], "TASK_EVIDENCE_INVALID");
    assert!(b["error"]["message"].as_str().unwrap().contains("E-BAD-EVIDENCE-KIND"), "{b}");

    // EVIDENCE_CONTRACT_CONFLICT: the atom declares `test-run` / `cargo test -p it`; this record carries another kind.
    let dir = repo_with_atom("test-run", "cargo test -p it");
    ok(dir.path(), &["ingest", "plans"], None);
    let conflicting = ["evidence", "add", "--task", "it-typed#1", "--class", "manual-review", "--ref", "reviewer sign-off", "--role", "implementer", "--repo", "."];
    refused(dir.path(), None, &conflicting, "EVIDENCE_CONTRACT_CONFLICT");
}

/// `EVIDENCE_NOTE_INVALID` and `EVIDENCE_REPORT_CONFLICT`: the two refusals of `evidence add`, on the conformance cases' repos.
#[test]
fn atip_codeexit_01_the_evidence_add_refusals_are_exit_2_and_write_nothing() {
    let args = |extra: &[&'static str]| -> Vec<&'static str> {
        let mut args = vec!["evidence", "add", "--scenario", "cart.add.01", "--project", "root", "--class", "test-run", "--ref", "cargo test cart_add", "--role", "implementer", "--actor", "impl-agent"];
        args.extend_from_slice(extra);
        args
    };
    // conformance/cases/evidence-add-fabrication-marker-whole-token
    let dir = case_repo("evidence-add-fabrication-marker-whole-token");
    refused(dir.path(), None, &args(&["--promote", "--summary", "atlas check: n/a"]), "EVIDENCE_NOTE_INVALID");
    step(dir.path(), &args(&["--promote", "--summary", "atlas check: 12 cases ran"]), 0);

    // conformance/cases/evidence-add-refused-report-contradiction
    let dir = case_repo("evidence-add-refused-report-contradiction");
    let report = args(&["--report", "junit:reports/junit.xml", "--verdict", "faithful"]);
    let b = refused(dir.path(), None, &report, "EVIDENCE_REPORT_CONFLICT");
    assert!(b["error"]["message"].as_str().unwrap().contains("cart.add.01"), "the refusal names the contradicting case: {b}");
}

// ── Gate, promotion, subject ──

/// `PROMOTION_INVALID`: a `gate promote` that meets a staged record the gate refuses.
#[test]
fn atip_codeexit_01_gate_promote_with_a_refused_record_is_exit_2_and_leaves_it_staged() {
    let dir = git_repo();
    let repo = dir.path();
    ok(repo, &["init", "--no-harness"], None);
    ok(repo, &["finding", "add", "--change", "c1", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "a1", "--summary", "first", "--promote"], Some("reviewer"));
    let ledger = repo.join(".canon/ledger/kind=finding");
    let mut committed = std::fs::read_dir(&ledger).unwrap().map(|entry| entry.unwrap().path());
    let path = committed.next().expect("the promoted finding");
    let mut record: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    record["summary"] = Value::String("first, changed".to_string());
    let name = path.file_name().unwrap().to_str().unwrap().rsplit_once('_').unwrap().0.to_string();
    let target = repo.join(".canon/ledger/_staging/kind=finding").join(format!("{name}_ffffffff.json"));
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
    let b = refused(repo, None, &["gate", "promote"], "PROMOTION_INVALID");
    assert!(b["error"].get("details").is_none(), "one refused record is the error itself: {b}");
    assert!(target.is_file(), "the record stays staged");
}

/// `SUBJECT_APPROVAL_REQUIRED`: a move to `shipped` without the approval of its brief or reviews.
#[test]
fn atip_codeexit_01_a_subject_move_without_its_approval_is_exit_2_and_changes_no_status() {
    let dir = git_repo();
    let repo = dir.path();
    ok(repo, &["init", "--no-harness"], None);
    ok(repo, &["subject", "new", "demo", "--domain", "d", "--title", "T"], Some("planner"));
    for to in ["specced", "building", "verifying"] {
        ok(repo, &["subject", "move", "demo", "--to", to], Some("planner"));
    }
    refused(repo, Some("planner"), &["subject", "move", "demo", "--to", "shipped"], "SUBJECT_APPROVAL_REQUIRED");
}

/// `SUBJECT_INVALID`: an open blocker finding on the subject's adopted change (conformance/cases/review-gate-open-blocker).
#[test]
fn atip_codeexit_01_a_subject_move_the_gate_refuses_for_another_rule_is_subject_invalid() {
    let dir = case_repo("review-gate-open-blocker");
    let b = refused(dir.path(), None, &["subject", "move", "cart", "--to", "verifying"], "SUBJECT_INVALID");
    assert!(b["error"]["message"].as_str().unwrap().contains("open-blocker"), "{b}");
}

/// `POLICY_APPROVAL_STALE`: a policy edited after its approval refuses a ship (the `subject-ship-allowed` repo, as `policy_fail_closed`).
#[test]
fn atip_codeexit_01_a_policy_edited_after_its_approval_refuses_the_ship_with_exit_2() {
    let policy = "spec_coverage:\n  require_cases: [failure]\n  require_review: {}\n";
    let dir = case_repo("subject-ship-allowed");
    let repo = dir.path();
    std::fs::write(repo.join(".canon/policy.yaml"), policy).unwrap();
    ok(repo, &["policy", "approve", "--approver", "human", "--note", "reviewed"], None);
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_cases: []\n").unwrap();
    refused(repo, None, &["subject", "move", "checkout", "--to", "shipped"], "POLICY_APPROVAL_STALE");

    // The approved bytes restored: the policy no longer refuses (the review it requires does).
    std::fs::write(repo.join(".canon/policy.yaml"), policy).unwrap();
    refused(repo, None, &["subject", "move", "checkout", "--to", "shipped"], "SUBJECT_APPROVAL_REQUIRED");
}

// ── Divergence, strategy ──

/// `DIVERGENCE_INVALID`: `divergence resolve` whose candidate the gate cannot promote.
#[test]
fn atip_codeexit_01_a_divergence_the_gate_refuses_is_exit_2_and_writes_nothing() {
    let dir = git_repo();
    let repo = dir.path();
    ok(repo, &["init", "--no-harness"], None);
    // A file where the committed ledger holds the kind's directory: the candidate stages, and its promotion is refused.
    std::fs::create_dir_all(repo.join(".canon/ledger")).unwrap();
    std::fs::write(repo.join(".canon/ledger/kind=divergence"), "not a directory\n").unwrap();
    refused(repo, None, &["divergence", "resolve", "auth.login.01", "--project", "root", "--actor", "alice", "--role", "reviewer"], "DIVERGENCE_INVALID");
}

fn strategy_item(content: &str, quarantined: bool) -> canon_learn::StrategyItem {
    use canon_learn::{StrategyId, StrategyItem, StrategyItemFields, StrategyLifecycle, TrajectoryId};
    let regime = canon_model::ids::RegimeKey::parse(canon_model::ids::regime_key("dev", "canon", "join-spine", "9c93d024b1a2")).unwrap();
    let item = StrategyItem::new(StrategyItemFields {
        id: StrategyId::new(),
        regime_key: regime,
        role: canon_model::ids::RoleId::parse("dev").unwrap(),
        title: "review guidance".into(),
        description: "one-liner".into(),
        content: content.into(),
        source_trajectory_ids: vec![TrajectoryId::new()],
        recorded_at: chrono::Utc::now(),
    });
    if quarantined { item.with_lifecycle(StrategyLifecycle::Quarantined) } else { item }
}

fn store_strategy(repo: &Path, item: &canon_learn::StrategyItem) {
    use canon_learn::StrategyStore as _;
    canon_learn::ParquetStrategyStore::open(repo.join(".canon").join("learn").join("strategies")).append(item).expect("seed strategy");
}

/// `STRATEGY_PROMOTION_INVALID` (no corroborating trajectory) and `STRATEGY_EVALUATION_INVALID` (an evaluation of another candidate).
#[test]
fn atip_codeexit_01_the_strategy_refusals_are_exit_2_and_write_nothing() {
    // The promotion gate blocks a strategy no trajectory corroborates (tests/strategy_promote.rs).
    let dir = TempDir::new().unwrap();
    let item = strategy_item("unproven insight", false);
    store_strategy(dir.path(), &item);
    let id = item.id.to_string();
    refused(dir.path(), None, &["strategy", "promote", &id, "--repo", "."], "STRATEGY_PROMOTION_INVALID");

    // `approval-payload` validates the evaluation against the candidate: this one was made for the candidate before its content changed.
    use canon_learn::{PromotionEvaluation, PromotionEvaluationFields};
    let dir = TempDir::new().unwrap();
    let evaluated = strategy_item("the evaluated content", true);
    let evaluation = PromotionEvaluation::new(
        &evaluated,
        PromotionEvaluationFields {
            candidate_version: "candidate-v2".into(),
            baseline_result_digests: vec!["baseline-panel-0".to_string(), "baseline-panel-1".to_string()],
            candidate_result_digests: vec!["candidate-panel-0".to_string(), "candidate-panel-1".to_string()],
            corpus_version: "corpus-v1".into(),
            eval_version: "paired-eval-v1".into(),
            context_digest: "context-digest-v1".into(),
            policy_digest: "policy-digest-v1".into(),
            model_digest: "model-digest-v1".into(),
            tool_digest: "tool-digest-v1".into(),
            paired_metrics: [("pairs".to_string(), 2.0), ("uplift".to_string(), 0.75), ("regressions".to_string(), 0.0)].into_iter().collect(),
            passed: true,
            decision: "promote".into(),
        },
    );
    std::fs::write(dir.path().join("evaluation.json"), serde_json::to_vec_pretty(&evaluation).unwrap()).unwrap();
    let mut stored = evaluated.clone();
    stored.content = "content edited after the evaluation".into();
    store_strategy(dir.path(), &stored);
    let id = evaluated.id.to_string();
    refused(
        dir.path(),
        None,
        &["strategy", "approval-payload", &id, "--evaluation", "evaluation.json", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z", "--repo", "."],
        "STRATEGY_EVALUATION_INVALID",
    );
}

// ── Commands that cannot start ──

/// `CONTEXT_GIT_REQUIRED` (no `git` on the `PATH`), and the same call with `git` on it.
#[test]
fn atip_codeexit_01_context_git_required_is_exit_2_and_writes_nothing() {
    let dir = git_repo();
    let p = dir.path();
    ok(p, &["init", "--no-harness"], None);
    std::fs::write(p.join("doc.md"), "hi\n").unwrap();
    std::fs::write(p.join("m.json"), r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#).unwrap();
    let no_git = TempDir::new().unwrap();
    refused_with(p, None, Some(no_git.path()), &["context-pack", "add", "--manifest", "m.json"], "CONTEXT_GIT_REQUIRED");
    ok(p, &["context-pack", "add", "--manifest", "m.json"], None);
}

/// `CONTEXT_OBJECT_CONFLICT`: an immutable object of the store holds other bytes than the ones the call would store. The first `context-pack add`
/// stores them; the bytes of one object are then changed; the same call finds everything stored, and refuses on the changed object.
#[test]
fn atip_codeexit_01_context_object_conflict_is_exit_2_and_writes_nothing() {
    let dir = git_repo();
    let d = dir.path();
    ok(d, &["init", "--no-harness"], None);
    std::fs::write(d.join("doc.md"), "hi\n").unwrap();
    std::fs::write(d.join("m.json"), r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#).unwrap();
    let objects = d.join(".canon/context-packs/objects");
    let stored = |dir: &Path| -> Vec<PathBuf> { std::fs::read_dir(dir).map(|entries| entries.map(|e| e.unwrap().path()).collect()).unwrap_or_default() };
    let before: BTreeSet<PathBuf> = stored(&objects).into_iter().collect();
    ok(d, &["context-pack", "add", "--manifest", "m.json"], None);
    let mut added: Vec<PathBuf> = stored(&objects).into_iter().filter(|path| !before.contains(path)).collect();
    added.sort();
    let victim = added.first().expect("the add stored objects");
    let mut bytes = std::fs::read(victim).unwrap();
    bytes.extend_from_slice(b"\n");
    std::fs::write(victim, bytes).unwrap();
    refused(d, None, &["context-pack", "add", "--manifest", "m.json"], "CONTEXT_OBJECT_CONFLICT");
}

/// `INGEST_SOURCE_INVALID`: `root:` one level above the `changes` directory; with the root right the same call takes the change.
#[test]
fn atip_codeexit_01_ingest_source_invalid_is_exit_2_and_writes_nothing() {
    let dir = repo_with_openspec("openspec");
    refused(dir.path(), None, &["ingest", "plans"], "INGEST_SOURCE_INVALID");
    let dir = repo_with_openspec("openspec/changes");
    ok(dir.path(), &["ingest", "plans"], None);
}

/// `DASHBOARD_UNSUPPORTED` (a repo whose `packages/dashboard` is someone else's app) and `DASHBOARD_PORT_UNAVAILABLE` (the port is taken).
#[test]
fn atip_codeexit_01_the_dashboard_refusals_are_exit_2_and_write_nothing() {
    let dir = git_repo();
    let d = dir.path();
    ok(d, &["init", "--no-harness"], None);
    let package = d.join("packages/dashboard");
    std::fs::create_dir_all(package.join("dist")).unwrap();
    std::fs::write(package.join("package.json"), r#"{"name":"@acme/dashboard"}"#).unwrap();
    std::fs::write(package.join("dist/index.html"), "<html>someone else's app</html>").unwrap();
    refused(d, None, &["dashboard", "--port", "0"], "DASHBOARD_UNSUPPORTED");

    let taken = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = taken.local_addr().unwrap().port().to_string();
    let live = repo_with_dashboard();
    let b = refused(live.path(), None, &["dashboard", "--port", &port], "DASHBOARD_PORT_UNAVAILABLE");
    assert!(b["error"]["message"].as_str().unwrap().contains(&port), "{b}");
}

/// `DISPATCH_RUN_UNAVAILABLE`: a `dispatch end` while another process holds the run's close lock (`<run>.json.lock`); retryable, so the same
/// call closes the run once the lock is gone.
#[test]
fn atip_codeexit_01_dispatch_run_unavailable_is_exit_2_retryable_and_writes_nothing() {
    let (dir, run_id) = repo_with_run();
    let repo = dir.path();
    let lock = repo.join(".canon/dispatch").join(format!("{run_id}.json.lock"));
    std::fs::write(&lock, b"").unwrap();
    assert!(registered("DISPATCH_RUN_UNAVAILABLE").retryable);
    let b = refused(repo, None, &["dispatch", "end", &run_id, "--run-outcome", "succeeded"], "DISPATCH_RUN_UNAVAILABLE");
    assert!(b["error"]["message"].as_str().unwrap().contains(".json.lock"), "the message names the lock: {b}");
    std::fs::remove_file(&lock).unwrap();
    ok(repo, &["dispatch", "end", &run_id, "--run-outcome", "succeeded"], None);
}

/// `LAYOUT_MIGRATION_REQUIRED`: a spec root still on the pre-0.17 layout is refused, never read.
#[test]
fn atip_codeexit_01_a_pre_0_17_spec_root_is_exit_2_and_writes_nothing() {
    let dir = case_repo("gate-check-refused-pre-0-17-layout");
    let b = refused(dir.path(), None, &["scenario", "new", "cart.add.03", "--title", "Another add", "--case", "failure"], "LAYOUT_MIGRATION_REQUIRED");
    assert!(b["error"]["message"].as_str().unwrap().contains("canon migrate"), "{b}");
}

// ── The table ──

/// The exit-2 codes of the brief's table that this file runs: each is the subject of a test above.
const RUN_HERE: &[&str] = &[
    "BRIEF_APPROVAL_REQUIRED",
    "BRIEF_APPROVAL_STALE",
    "CONTEXT_GIT_REQUIRED",
    "CONTEXT_OBJECT_CONFLICT",
    "DASHBOARD_UNSUPPORTED",
    "DASHBOARD_PORT_UNAVAILABLE",
    "DISPATCH_RUN_UNAVAILABLE",
    "DIVERGENCE_INVALID",
    "EVIDENCE_CONTRACT_CONFLICT",
    "EVIDENCE_NOTE_INVALID",
    "EVIDENCE_REPORT_CONFLICT",
    "TASK_EVIDENCE_REQUIRED",
    "TASK_EVIDENCE_INVALID",
    "INGEST_SOURCE_INVALID",
    "STRATEGY_EVALUATION_INVALID",
    "STRATEGY_PROMOTION_INVALID",
    "POLICY_APPROVAL_STALE",
    "PROMOTION_INVALID",
    "SUBJECT_APPROVAL_REQUIRED",
    "SUBJECT_INVALID",
    "LAYOUT_MIGRATION_REQUIRED",
];

/// The brief's table (`.canon/changes/canon-0-36-codes-exit/brief.md`): the new codes of its rows that answer exit 2. The adapter, context-pack
/// verify and report-check rows answer exit 1 or 0 and are not refusals.
const TABLE_EXIT_2: &[&str] = &[
    "BRIEF_APPROVAL_STALE",
    "BRIEF_APPROVAL_REQUIRED",
    "CONTEXT_GIT_REQUIRED",
    "CONTEXT_OBJECT_CONFLICT",
    "DASHBOARD_UNSUPPORTED",
    "DASHBOARD_PORT_UNAVAILABLE",
    "DISPATCH_RUN_UNAVAILABLE",
    "DIVERGENCE_INVALID",
    "EVIDENCE_CONTRACT_CONFLICT",
    "EVIDENCE_NOTE_INVALID",
    "EVIDENCE_REPORT_CONFLICT",
    "TASK_EVIDENCE_REQUIRED",
    "TASK_EVIDENCE_INVALID",
    "INGEST_SOURCE_INVALID",
    "STRATEGY_EVALUATION_INVALID",
    "STRATEGY_PROMOTION_INVALID",
    "POLICY_APPROVAL_STALE",
    "PROMOTION_INVALID",
    "SUBJECT_APPROVAL_REQUIRED",
    "SUBJECT_INVALID",
    "EVIDENCE_STALE",
    "LAYOUT_MIGRATION_REQUIRED",
];

/// The exit-2 code of the table that no command can be made to answer: `EVIDENCE_STALE` is the code of a gate violation of class `stale-evidence`
/// that `subject move` would map (`gate_refusal::subject_code`), but `stale-evidence` is raised only by the artifact-store and staleness checks of
/// `canon gate check` (a check: exit 1 `GATE_CHECK_FAILED`, with `data`), which no transition guard runs.
const NOT_PROVOKABLE: &[&str] = &["EVIDENCE_STALE"];

/// `EVIDENCE_STALE` cannot be provoked from a process (see [`NOT_PROVOKABLE`]); what is checked is the registry's row (exit 2, not retryable) and
/// the source's mapping of the stale class to the code.
#[test]
fn atip_codeexit_01_evidence_stale_is_the_registrys_exit_2_and_the_one_mapping_of_the_stale_class() {
    let info = registered("EVIDENCE_STALE");
    assert_eq!((info.exit, info.retryable), (2, false));
    let mapping = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/gate_refusal.rs")).unwrap();
    assert!(mapping.contains("FailureClass::StaleEvidence => \"EVIDENCE_STALE\""), "subject_code maps the stale class to the code");
    assert!(mapping.contains("FailureClass::OldLayout => \"LAYOUT_MIGRATION_REQUIRED\""), "and the old-layout class to its code");
}

/// atip.codeexit.01: the codes run here and the code that cannot be run are the exit-2 codes of the brief's table, none missing, none extra, and
/// each is the registry's exit 2.
#[test]
fn atip_codeexit_01_every_code_of_the_table_is_run_here() {
    let run: BTreeSet<&str> = RUN_HERE.iter().copied().collect();
    let skipped: BTreeSet<&str> = NOT_PROVOKABLE.iter().copied().collect();
    let table: BTreeSet<&str> = TABLE_EXIT_2.iter().copied().collect();
    assert_eq!(run.len(), RUN_HERE.len(), "a code is listed once");
    assert_eq!(table.len(), 22, "the table has 22 exit-2 codes besides the adapter, context-pack verify and report-check rows");
    assert!(run.is_disjoint(&skipped));
    assert_eq!(run.union(&skipped).copied().collect::<BTreeSet<_>>(), table);
    assert_eq!(run.len(), 21, "21 codes are run for real");
    for code in &table {
        assert_eq!(registered(code).exit, 2, "{code}");
    }
}
