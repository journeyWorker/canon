//! 0.38 `canon-0-38-code-scopes` (#130) through the real binary: the policy's `code_scopes`
//! key gives a scenario area or surface a set of code paths, and an approved chore asks a
//! scenario for new evidence only when a path of its scope changed since the scenario's
//! latest evidence. The ship guard (`subject move --to shipped`), `canon status` and
//! `canon gate check` share the rule; a scenario without a scope, without a recorded tree or
//! whose old tree cannot be had keeps the 0.37 rule.

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

#[path = "support/hermetic.rs"]
mod hermetic;

use canon_model::{Actor, Change, ChangeId, ChangeStatus, Envelope, EvidenceRecord, EvidenceVerdict, ProjectId, RecordKind, RoleId, ScenarioId, Sha, Subject, SubjectId, SubjectStatus};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::{Duration as Span, Utc};
use serde_json::Value;
use tempfile::TempDir;

const REQUIRE_REVIEW: &str = "spec_coverage:\n  require_evidence: true\n  require_review: {}\n";
const SCOPES: &str = "code_scopes:\n  core: [\"core/**\"]\n  app: [\"app/**\", \"core/**\"]\n";
const SCOPES_WITH_SURFACE: &str = "code_scopes:\n  core: [\"core/**\"]\n  app: [\"app/**\", \"core/**\"]\n  app.bulkread: [\"app/bulk/**\"]\n";
const REPORT_A: &str = r#"<testsuite><testcase classname="core" name="core.engine.01 runs"/></testsuite>"#;
const REPORT_B: &str = r#"<testsuite><testcase classname="app" name="app.bulkread.01 reads"/><testcase classname="app" name="app.screen.01 shows"/></testsuite>"#;
/// The batch for the scenarios that report `b.xml` bound: the ones a chore over `app/` asks for.
const BATCH: &str = "canon evidence add --all-scenarios --report junit:b.xml --role implementer";
const CHORE: &str = "---\nkind: chore\n---\n\n# Bump the client\n\n## Intent\n\nBump the client\n\nThe client is two versions behind.\n\n## Category\n\ndependency\n\n## Invariants\n\n- Nothing changes. ==**No behavior changes.**==\n\n## Checks\n\n- The suite runs again. ==**The suite is the check.**==\n\n## Out of scope\n\nNone.\n";
const CHORE_BRIEF: &str = ".canon/changes/bump-client/brief.md";
const GAP_REFUSAL: &str = "no evidence recorded after";
const REVIEW_REFUSAL: &str = "no review of change `bump-client`";
const SCENARIOS: [&str; 3] = ["core.engine.01", "app.bulkread.01", "app.screen.01"];

fn canon(repo: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
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

/// The feature file of scenario `id` (`<area>.<surface>.<nn>`), where the layout wants it.
fn feature_file(id: &str) -> (String, String) {
    let mut parts = id.split('.');
    let (area, surface) = (parts.next().unwrap(), parts.next().unwrap());
    let stamp = "# canon: {\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}";
    let body = format!("Feature: {surface}\n  {stamp}\n\n  {stamp}\n  @subject:demo\n  @{id}\n  Scenario: Behavior of {id}\n    When the user acts\n    Then it works\n");
    (format!(".canon/specs/kind=feature/area={area}/{surface}.feature"), body)
}

fn actor(id: &str, role: &str) -> Actor {
    Actor::new(id, RoleId::parse(role).unwrap())
}

fn approve_policy(repo: &Path) {
    ok(repo, &["policy", "approve", "--approver", "Journey", "--note", "go"]);
}

/// A git repo with code in `core/`, `app/` and `app/bulk/`, three scenarios of subject `demo`
/// (`core.engine.01`, `app.bulkread.01`, `app.screen.01`) evidenced by the reports `a.xml`
/// and `b.xml` and reviewed, the subject `verifying`, the chore change `bump-client` adopted
/// with an unapproved brief, `policy` approved, and all committed.
fn repo_with(policy: &str, approved: bool) -> TempDir {
    repo_custom(policy, approved, |_| {})
}

/// [`repo_with`], where `late` runs after the evidence is recorded and before it is reviewed.
fn repo_custom(policy: &str, approved: bool, late: impl FnOnce(&Path)) -> TempDir {
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.name", "t"]);
    git(r, &["config", "user.email", "t@example.com"]);
    ok(r, &["init", "--repo", ".", "--no-policy"]);
    let config = read(r, "canon.yaml");
    write(r, "canon.yaml", &format!("{config}\ncode:\n  exclude: [\"docs/**\", \"*.md\"]\n"));
    write(r, ".canon/policy.yaml", &format!("{REQUIRE_REVIEW}{policy}"));
    for id in SCENARIOS {
        let (path, body) = feature_file(id);
        write(r, &path, &body);
    }
    write(r, "core/lib.rs", "pub fn engine() {}\n");
    write(r, "app/main.rs", "fn main() {}\n");
    write(r, "app/bulk/read.rs", "pub fn read() {}\n");
    write(r, "util/notes.txt", "notes\n");
    write(r, "a.xml", REPORT_A);
    write(r, "b.xml", REPORT_B);
    ok(r, &["inventory", "sync"]);

    let ledger = GitTier::new(r.join(".canon/ledger"));
    let at = Utc::now() - Span::seconds(600);
    for change in ["add-engine", "bump-client"] {
        let envelope = Envelope::new(1, RecordKind::Change, at, actor("importer", "planner"));
        ledger.write(&Change::new(envelope, ChangeId::parse(change).unwrap(), "Seeded", "why", ChangeStatus::InProgress)).unwrap();
    }
    let envelope = Envelope::new(1, RecordKind::Subject, at, actor("canon", "implementer"));
    let subject = Subject::new(envelope, SubjectId::parse("demo").unwrap(), "Demo", "s", "dev", SubjectStatus::Verifying, RoleId::parse("implementer").unwrap())
        .with_change_ids(["add-engine", "bump-client"].iter().map(|c| ChangeId::parse(*c).unwrap()).collect());
    ledger.write(&subject).unwrap();
    write(r, ".canon/changes/add-engine/brief.md", "## Why\n\nSeeded.\n\nApproved: Journey 2026-10-12 — \"go\"\n");
    write(r, CHORE_BRIEF, CHORE);
    commit_all(r, "setup");
    if approved {
        approve_policy(r);
    }

    ok(r, &["evidence", "add", "--all-scenarios", "--report", "junit:a.xml", "--report", "junit:b.xml", "--role", "implementer"]);
    late(r);
    for scenario in SCENARIOS {
        ok(r, &["review", "scenario", scenario, "--actor", "reviewer-2", "--role", "reviewer", "--original-spec-ref", "docs/prd.md"]);
    }
    commit_all(r, "baseline");
    dir
}

fn repo() -> TempDir {
    repo_with(SCOPES, true)
}

/// The human approves the chore with `canon brief approve`, at least a second
/// after the evidence (an approval's time is to the second).
fn approve(repo: &Path) {
    std::thread::sleep(Duration::from_millis(1100));
    ok(repo, &["brief", "approve", "bump-client", "--approver", "Journey", "--note", "go"]);
}

/// The chore edits one file after its approval and commits it.
fn edit(repo: &Path, rel: &str) {
    write(repo, rel, &format!("{}// edited by the chore\n", std::fs::read_to_string(repo.join(rel)).unwrap_or_default()));
    commit_all(repo, &format!("the chore edits {rel}"));
}

fn ship(repo: &Path) -> Output {
    canon(repo, &["subject", "move", "demo", "--to", "shipped"])
}

fn status_json(repo: &Path) -> Value {
    serde_json::from_str(&ok(repo, &["status", "--json"])).unwrap()
}

/// The steps `canon status` lists about this subject, from its human form.
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

fn gate_json(repo: &Path) -> (Option<i32>, Value) {
    let out = canon(repo, &["gate", "check", "--json"]);
    let report: Value = serde_json::from_str(&text(&out.stdout)).unwrap_or_else(|e| panic!("gate check --json: {e}: {}{}", text(&out.stdout), text(&out.stderr)));
    (out.status.code(), report)
}

/// The `uncovered-cell` messages `canon gate check` names for subject `demo`.
fn gate_evidence_violations(repo: &Path) -> Vec<String> {
    let (_, report) = gate_json(repo);
    report["data"]["violations"]
        .as_array()
        .unwrap_or_else(|| panic!("data.violations[]: {report}"))
        .iter()
        .filter(|v| v["class"] == "uncovered-cell" && v["subject"] == "demo")
        .map(|v| v["message"].as_str().unwrap().to_string())
        .collect()
}

fn advisory_items(report: &Value, kind: &str) -> Vec<String> {
    report["data"]["advisories"].as_array().into_iter().flatten().filter(|a| a["kind"] == kind).flat_map(|a| a["items"].as_array().cloned().unwrap_or_default()).map(|i| i.as_str().unwrap().to_string()).collect()
}

fn chore_entry(status: &Value) -> Value {
    let demo = status["data"]["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "demo").unwrap_or_else(|| panic!("no subject demo: {status}"));
    demo["briefs"].as_array().unwrap().iter().find(|b| b["change"] == "bump-client").unwrap_or_else(|| panic!("no brief of bump-client: {demo}")).clone()
}

/// What the evidence gap of the ship refusal names: asserts the scenarios named and not.
fn assert_names(text: &str, named: &[&str], not_named: &[&str]) {
    for id in named {
        assert!(text.contains(&format!("`{id}`")), "`{id}` should be named: {text}");
    }
    for id in not_named {
        assert!(!text.contains(&format!("`{id}`")), "`{id}` should not be named: {text}");
    }
}

/// A later evidence record of `scenario` that names the code `tree` of commit `sha` (either may be missing),
/// written straight into the ledger: it is the scenario's latest evidence.
fn later_evidence(repo: &Path, scenario: &str, tree: Option<&str>, sha: Option<&str>) {
    let ledger = GitTier::new(repo.join(".canon/ledger"));
    let at = Utc::now();
    let mut record = EvidenceRecord::new(Envelope::current(RecordKind::Evidence, at, actor("canon", "implementer")), None, Some(ScenarioId::parse(scenario).unwrap()), None, EvidenceVerdict::Faithful)
        .with_project_id(ProjectId::parse("root").unwrap());
    if let Some(sha) = sha {
        record = record.with_evidence_sha(Sha::parse(sha).unwrap());
    }
    if let Some(tree) = tree {
        record = record.with_code(tree, false);
    }
    ledger.write(&record).unwrap();
}

fn head(repo: &Path) -> String {
    text(&Command::new("git").current_dir(repo).args(["rev-parse", "HEAD"]).output().unwrap().stdout).trim().to_string()
}

fn code_tree_now(repo: &Path) -> String {
    canon_cli::git_rev::code_tree(repo, None).expect("the code tree").tree
}

/// gate.code-scopes.01: the policy is approved, the chore changes one file of `app/`: the
/// `core` scenario is not asked for, the `app` scenarios are, with the changed path.
#[test]
fn gate_code_scopes_01_a_chore_that_changes_only_app_asks_the_app_scenarios_and_not_the_core_one() {
    let dir = repo();
    let r = dir.path();
    approve(r);
    edit(r, "app/main.rs");

    let out = ship(r);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains(GAP_REFUSAL), "{err}");
    assert_names(&err, &["app.bulkread.01", "app.screen.01"], &["core.engine.01"]);
    assert!(err.contains("changed in scope `app`") && err.contains("`app/main.rs`"), "the refusal names the changed path in the scope: {err}");
    let hint = err.lines().find(|l| l.starts_with("  hint: ")).unwrap_or_else(|| panic!("a refusal carries a hint: {err}"));
    assert!(hint.contains(&format!("`{BATCH}`")), "one batch command leads: {hint}");

    let words: Vec<&str> = BATCH.split(' ').skip(1).collect();
    ok(r, &words);
    let err = text(&ship(r).stderr);
    assert!(!err.contains(GAP_REFUSAL), "the batch closed the gap: {err}");
    assert!(err.contains(REVIEW_REFUSAL), "what remains is the chore's review: {err}");
}

/// gate.code-scopes.02: a policy edited after its approval, or never approved, declares no
/// scope; the edited one fails the move closed.
#[test]
fn gate_code_scopes_02_a_policy_edited_after_its_approval_or_never_approved_states_no_scope() {
    let dir = repo();
    let r = dir.path();
    approve(r);
    edit(r, "app/main.rs");
    write(r, ".canon/policy.yaml", &format!("{}# edited after the approval\n", read(r, ".canon/policy.yaml")));
    let out = ship(r);
    assert_eq!(out.status.code(), Some(2), "{}{}", text(&out.stdout), text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains("POLICY_APPROVAL_STALE") || err.contains("policy changed since approval"), "{err}");
    let violations = gate_evidence_violations(r);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_names(&violations[0], &SCENARIOS, &[]);
    assert!(!violations[0].contains("changed in scope"), "no scope stands: {violations:?}");
    assert!(chore_entry(&status_json(r)).get("scopeExempt").is_none());

    // Never approved: the scopes are written, nobody approved them (`canon brief approve` records the policy's approval in its own call, so the record is taken away).
    let dir = repo_with(SCOPES, false);
    let r = dir.path();
    approve(r);
    std::fs::remove_file(r.join(".canon/policy.approval.json")).unwrap();
    edit(r, "app/main.rs");
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL), "{err}");
    assert_names(&err, &SCENARIOS, &[]);
    assert!(!err.contains("changed in scope"), "{err}");
}

/// gate.code-scopes.03: a surface key wins over its area's: the chore changes `app/main.rs`,
/// `app.bulkread` (scope `app/bulk/**`) is not asked for although `app` covers the file.
#[test]
fn gate_code_scopes_03_the_surface_key_wins_over_the_area_key() {
    let dir = repo_with(SCOPES_WITH_SURFACE, true);
    let r = dir.path();
    approve(r);
    edit(r, "app/main.rs");
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL), "{err}");
    assert_names(&err, &["app.screen.01"], &["core.engine.01", "app.bulkread.01"]);
    assert!(err.contains("`app/main.rs`"), "{err}");
    let entry = chore_entry(&status_json(r));
    assert_eq!(entry["scopeExempt"]["scenarios"], 2, "{entry}");
    assert_eq!(entry["scopeExempt"]["keys"], serde_json::json!(["app.bulkread", "core"]), "{entry}");
}

/// gate.code-scopes.04: the chore changes a file of `core/`: the `core` and the `app`
/// scenarios are all asked for, each naming the path.
#[test]
fn gate_code_scopes_04_a_chore_that_changes_core_asks_the_core_and_the_app_scenarios() {
    let dir = repo();
    let r = dir.path();
    approve(r);
    edit(r, "core/lib.rs");
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL), "{err}");
    assert_names(&err, &SCENARIOS, &[]);
    assert!(err.contains("changed in scope `core`") && err.contains("changed in scope `app`") && err.contains("`core/lib.rs`"), "{err}");
}

/// gate.code-scopes.05: a file renamed out of the scope asks for the scenarios by its old path.
#[test]
fn gate_code_scopes_05_a_file_renamed_out_of_the_scope_asks_by_its_old_path() {
    let dir = repo();
    let r = dir.path();
    approve(r);
    write(r, "util/lib.rs", &read(r, "core/lib.rs"));
    std::fs::remove_file(r.join("core/lib.rs")).unwrap();
    commit_all(r, "the chore moves core/lib.rs to util/lib.rs");
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL), "{err}");
    assert_names(&err, &SCENARIOS, &[]);
    assert!(err.contains("`core/lib.rs`") && !err.contains("util/lib.rs"), "the old path is in scope, the new one is not: {err}");
}

/// gate.code-scopes.06: the ship guard, `canon status` and `canon gate check` ask for the same
/// scenarios, the same paths and the same batch command.
#[test]
fn gate_code_scopes_06_ship_status_and_gate_check_state_the_same_requirement() {
    let dir = repo();
    let r = dir.path();
    approve(r);
    edit(r, "app/main.rs");
    let err = text(&ship(r).stderr);
    let violations = gate_evidence_violations(r);
    assert_eq!(violations.len(), 1, "{violations:?}");
    let steps = status_steps(r);
    assert_eq!(evidence_steps(&steps).len(), 1, "{steps:?}");
    let why = steps[0]["why"].as_str().unwrap();
    for (who, said) in [("ship", err.as_str()), ("gate check", violations[0].as_str()), ("status", why)] {
        assert_names(said, &["app.bulkread.01", "app.screen.01"], &["core.engine.01"]);
        assert!(said.contains("changed in scope `app`") && said.contains("`app/main.rs`"), "{who}: {said}");
    }
    assert!(err.contains(&format!("`{BATCH}`")) && violations[0].contains(&format!("`{BATCH}`")), "{err} / {violations:?}");
    assert_eq!(steps[0]["command"], BATCH, "{steps:?}");
    assert_eq!(status_json(r)["meta"]["next"][0]["command"], format!("{BATCH} --json"));
}

/// gate.code-scopes.07: a chore whose changes lie outside every scope asks for no evidence, and
/// the three say how many scenarios the scopes let go, by which keys.
#[test]
fn gate_code_scopes_07_the_chore_line_says_how_many_scenarios_the_scopes_let_go_and_by_which_keys() {
    let dir = repo();
    let r = dir.path();
    approve(r);
    edit(r, "util/notes.txt");

    let err = text(&ship(r).stderr);
    assert!(err.contains(REVIEW_REFUSAL) && !err.contains(GAP_REFUSAL), "only the chore's review is asked: {err}");
    assert!(evidence_steps(&status_steps(r)).is_empty(), "{:?}", status_steps(r));
    assert!(gate_evidence_violations(r).is_empty());
    let entry = chore_entry(&status_json(r));
    assert_eq!(entry["scopeExempt"], serde_json::json!({"scenarios": 3, "keys": ["app", "core"]}), "{entry}");
    let human = ok(r, &["status"]);
    assert!(human.contains("3 scenario(s) need no new evidence by code scope (`app`, `core`)"), "{human}");
    let (_, report) = gate_json(r);
    let lines = advisory_items(&report, "chore_code_scopes");
    assert_eq!(lines.len(), 1, "{report}");
    assert!(lines[0].contains("chore change `bump-client`") && lines[0].contains("3 scenario(s)") && lines[0].contains("`app`, `core`"), "{lines:?}");

    // Part of the suite let go: the refusal says how many.
    let dir = repo();
    let r = dir.path();
    approve(r);
    edit(r, "app/main.rs");
    let err = text(&ship(r).stderr);
    assert!(err.contains("1 scenario(s) need nothing from this chore") && err.contains("(`core`)"), "{err}");
}

/// gate.code-scopes.08: no key, an area without a scope, and evidence without a recorded tree are judged as in 0.37.
#[test]
fn gate_code_scopes_08_no_key_no_scope_and_no_recorded_tree_are_judged_as_in_0_37() {
    // No `code_scopes` key: every scenario is asked for, with no scope note.
    let dir = repo_with("", true);
    let r = dir.path();
    approve(r);
    edit(r, "util/notes.txt");
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL), "{err}");
    assert_names(&err, &SCENARIOS, &[]);
    assert!(!err.contains("changed in scope") && !err.contains("need nothing from this chore"), "{err}");
    assert!(chore_entry(&status_json(r)).get("scopeExempt").is_none());

    // An area without a key: `core` is asked for, `app` is let go.
    let dir = repo_with("code_scopes:\n  app: [\"app/**\"]\n", true);
    let r = dir.path();
    approve(r);
    edit(r, "util/notes.txt");
    let err = text(&ship(r).stderr);
    assert_names(&err, &["core.engine.01"], &["app.bulkread.01", "app.screen.01"]);

    // Evidence that recorded no tree: asked for although its scope is untouched.
    let dir = repo_custom(SCOPES, true, |r| later_evidence(r, "core.engine.01", None, None));
    let r = dir.path();
    approve(r);
    edit(r, "util/notes.txt");
    let err = text(&ship(r).stderr);
    assert_names(&err, &["core.engine.01"], &["app.bulkread.01", "app.screen.01"]);
}

/// gate.code-scopes.09: evidence whose old tree cannot be had is asked for, scope untouched or not.
#[test]
fn gate_code_scopes_09_evidence_whose_tree_cannot_be_had_is_asked_for() {
    let dir = repo_custom(SCOPES, true, |r| {
        // The commit is there but its code tree is not the one the record names.
        later_evidence(r, "core.engine.01", Some(&"0".repeat(40)), Some(&head(r)));
        // The tree is right, the commit it names is gone.
        later_evidence(r, "app.screen.01", Some(&code_tree_now(r)), Some(&"1".repeat(40)));
    });
    let r = dir.path();
    approve(r);
    edit(r, "util/notes.txt");
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL), "{err}");
    assert_names(&err, &["core.engine.01", "app.screen.01"], &["app.bulkread.01"]);
    assert!(!err.contains("changed in scope"), "no diff, no paths to name: {err}");
}

/// gate.code-scopes.10: a pattern that matches no file and a key that matches no scenario are named
/// by `canon status` and `canon gate check`.
#[test]
fn gate_code_scopes_10_a_pattern_that_matches_no_file_and_a_key_that_matches_no_scenario_are_named() {
    let dir = repo_with("code_scopes:\n  core: [\"core/**\", \"nothing/**\"]\n  app: [\"app/**\"]\n  ghost: [\"core/**\"]\n", true);
    let r = dir.path();
    let status = status_json(r);
    let warnings: Vec<String> = status["meta"]["warnings"].as_array().unwrap_or_else(|| panic!("meta.warnings: {status}")).iter().filter(|w| w["code"] == "CODE_SCOPES_UNMATCHED").map(|w| w["message"].as_str().unwrap().to_string()).collect();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("`nothing/**`") && w.contains("matches no file")), "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("`ghost`") && w.contains("matches no scenario")), "{warnings:?}");
    let human = ok(r, &["status"]);
    assert!(human.contains("`nothing/**`"), "{human}");

    let (_, report) = gate_json(r);
    let items = advisory_items(&report, "code_scopes");
    assert_eq!(items.len(), 2, "{report}");
    assert!(items.iter().any(|w| w.contains("`nothing/**`")) && items.iter().any(|w| w.contains("`ghost`")), "{items:?}");

    // A repo that declares nothing, or declares only good scopes, hears nothing.
    for policy in ["", SCOPES] {
        let dir = repo_with(policy, true);
        let r = dir.path();
        let (_, report) = gate_json(r);
        assert!(advisory_items(&report, "code_scopes").is_empty(), "{policy}: {report}");
        let status = status_json(r);
        assert!(status["meta"]["warnings"].as_array().into_iter().flatten().all(|w| w["code"] != "CODE_SCOPES_UNMATCHED"), "{policy}: {status}");
    }
}

/// gate.code-scopes.11: the warnings do not change an exit code.
#[test]
fn gate_code_scopes_11_the_warnings_do_not_change_an_exit_code() {
    let bad = "code_scopes:\n  core: [\"nothing/**\"]\n  ghost: [\"core/**\"]\n";
    let plain = repo_with("", true);
    let warned = repo_with(bad, true);
    for (what, args) in [("status", vec!["status"]), ("gate check", vec!["gate", "check"])] {
        let before = canon(plain.path(), &args).status.code();
        let after = canon(warned.path(), &args).status.code();
        assert_eq!(before, after, "{what}: the warnings change no exit code");
    }
    let (code, report) = gate_json(warned.path());
    assert_eq!(code, Some(0), "{report}");
    assert_eq!(report["data"]["clean"], true, "{report}");
    assert!(!advisory_items(&report, "code_scopes").is_empty(), "{report}");

    // A malformed key is a warning too, and declares no scope.
    let dir = repo_with("code_scopes: [\"core/**\"]\n", true);
    let r = dir.path();
    let (code, report) = gate_json(r);
    assert_eq!(code, Some(0), "{report}");
    assert!(advisory_items(&report, "code_scopes").iter().any(|w| w.contains("malformed")), "{report}");
}

/// gate.code-scopes.14: a key whose patterns match no file declares no scope: the misspelt `Core/**` never exempts.
#[test]
fn gate_code_scopes_14_a_key_whose_patterns_match_no_file_declares_no_scope() {
    let dir = repo_with("code_scopes:\n  core: [\"Core/**\"]\n", true);
    let r = dir.path();
    approve(r);
    edit(r, "core/lib.rs");
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL), "{err}");
    assert_names(&err, &["core.engine.01"], &[]);
    assert!(!err.contains("changed in scope") && !err.contains("need nothing from this chore"), "the misspelt scope exempts and names nothing: {err}");
    let violations = gate_evidence_violations(r);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_names(&violations[0], &["core.engine.01"], &[]);
    let entry = chore_entry(&status_json(r));
    assert!(entry.get("scopeExempt").is_none(), "{entry}");

    // Even over a change outside every real scope: the key stands for nothing, the scenario follows the rule without scopes.
    let dir = repo_with("code_scopes:\n  core: [\"Core/**\"]\n", true);
    let r = dir.path();
    approve(r);
    edit(r, "util/notes.txt");
    assert_names(&text(&ship(r).stderr), &["core.engine.01"], &[]);

    // The warning says what follows from it, and a key with one live pattern still stands.
    let (code, report) = gate_json(r);
    assert_eq!(code, Some(1), "the evidence gap is the gate's: {report}");
    let items = advisory_items(&report, "code_scopes");
    assert!(items.iter().any(|w| w.contains("`Core/**`") && w.contains("declares no scope") && w.contains("0.37 rule")), "{items:?}");
    let dir = repo_with("code_scopes:\n  core: [\"Core/**\", \"core/**\"]\n", true);
    let r = dir.path();
    approve(r);
    edit(r, "util/notes.txt");
    let err = text(&ship(r).stderr);
    assert!(!err.contains("`core.engine.01`"), "one live pattern keeps the scope standing: {err}");
}

/// gate.code-scopes.15: an empty list is malformed: warned, no scope.
#[test]
fn gate_code_scopes_15_an_empty_list_is_malformed_and_declares_no_scope() {
    let dir = repo_with("code_scopes:\n  core: []\n  app: [\"app/**\"]\n", true);
    let r = dir.path();
    approve(r);
    edit(r, "util/notes.txt");
    let err = text(&ship(r).stderr);
    assert!(err.contains(GAP_REFUSAL), "{err}");
    assert_names(&err, &SCENARIOS, &[]);
    assert!(!err.contains("need nothing from this chore"), "{err}");
    let (_, report) = gate_json(r);
    let items = advisory_items(&report, "code_scopes");
    assert!(items.iter().any(|w| w.contains("malformed") && w.contains("`core` has no pattern")), "{items:?}");
    assert!(chore_entry(&status_json(r)).get("scopeExempt").is_none());
    let status = status_json(r);
    assert!(status["meta"]["warnings"].as_array().into_iter().flatten().any(|w| w["code"] == "CODE_SCOPES_UNMATCHED" && w["message"].as_str().unwrap().contains("malformed")), "{status}");
}

/// gate.code-scopes.16: `canon brief approve` does not approve a policy whose `code_scopes` differs from the last approved one.
#[test]
fn gate_code_scopes_16_a_brief_approval_leaves_the_policy_approval_stale_when_the_scopes_changed() {
    let dir = repo();
    let r = dir.path();
    let approval = read(r, ".canon/policy.approval.json");
    // An agent widens the scope of `core` and asks for the brief's approval.
    write(r, ".canon/policy.yaml", &read(r, ".canon/policy.yaml").replace("core: [\"core/**\"]", "core: [\"core/**\", \"util/**\"]"));
    std::thread::sleep(Duration::from_millis(1100));
    let out = ok(r, &["brief", "approve", "bump-client", "--approver", "Journey", "--note", "go"]);
    assert!(out.contains("policy approval was NOT recorded") && out.contains("`core`"), "{out}");
    assert!(!out.contains("`app`"), "only the changed key is named: {out}");
    assert_eq!(read(r, ".canon/policy.approval.json"), approval, "the policy approval is as it was");
    assert!(read(r, CHORE_BRIEF).contains("Approved: Journey"), "the brief is approved");
    let json: Value = serde_json::from_str(&ok(r, &["brief", "approve", "bump-client", "--approver", "Journey", "--note", "go", "--json"])).unwrap();
    assert_eq!(json["data"]["policyApproved"], false, "{json}");
    assert_eq!(json["data"]["policyHeld"], serde_json::json!(["core"]), "{json}");

    // The gate fails closed until a human approves the policy.
    let out = ship(r);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("POLICY_APPROVAL_STALE"), "{}", text(&out.stderr));
    assert_eq!(chore_entry(&status_json(r)).get("scopeExempt"), None);

    // The human approves the policy; a brief approval over unchanged scopes (a comment edit) records the policy approval as before.
    approve_policy(r);
    commit_all(r, "the human approves the widened policy");
    write(r, ".canon/policy.yaml", &format!("{}# a comment\n", read(r, ".canon/policy.yaml")));
    std::thread::sleep(Duration::from_millis(1100));
    let out = ok(r, &["brief", "approve", "bump-client", "--approver", "Journey", "--note", "go again"]);
    assert!(out.contains("policy approval recorded too") && !out.contains("NOT recorded"), "{out}");

    // A first approval, with no earlier one, is as before; the approved bytes that git does not hold (an approval never committed) cannot be compared, so the scopes count as changed.
    let dir = repo_with(SCOPES, false);
    let r = dir.path();
    let out = ok(r, &["brief", "approve", "bump-client", "--approver", "Journey", "--note", "go"]);
    assert!(out.contains("policy approval recorded too"), "{out}");
    assert!(r.join(".canon/policy.approval.json").is_file());
}

/// gate.code-scopes.17: over a change in `core/` the three ask for every scenario alike; none lets one go.
#[test]
fn gate_code_scopes_17_over_a_core_change_ship_status_and_gate_check_ask_for_every_scenario_alike() {
    let dir = repo();
    let r = dir.path();
    approve(r);
    edit(r, "core/lib.rs");
    let all = "canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer";
    let err = text(&ship(r).stderr);
    let violations = gate_evidence_violations(r);
    assert_eq!(violations.len(), 1, "{violations:?}");
    let steps = status_steps(r);
    assert_eq!(evidence_steps(&steps).len(), 1, "{steps:?}");
    let why = steps[0]["why"].as_str().unwrap();
    for (who, said) in [("ship", err.as_str()), ("gate check", violations[0].as_str()), ("status", why)] {
        assert_names(said, &SCENARIOS, &[]);
        assert!(said.contains("changed in scope `core`") && said.contains("changed in scope `app`") && said.contains("`core/lib.rs`"), "{who}: {said}");
        assert!(!said.contains("need nothing from this chore"), "{who}: {said}");
    }
    assert!(err.contains(&format!("`{all}`")) && violations[0].contains(&format!("`{all}`")), "{err} / {violations:?}");
    assert_eq!(steps[0]["command"], all, "{steps:?}");
    assert!(chore_entry(&status_json(r)).get("scopeExempt").is_none());
}

fn unreleased() -> String {
    let changelog = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).expect("the CHANGELOG");
    let start = changelog.find("\n## Unreleased").expect("the CHANGELOG has an Unreleased section");
    let end = changelog[start + 1..].find("\n## ").map_or(changelog.len(), |at| start + 1 + at);
    changelog[start..end].to_string()
}

/// What the CHANGELOG section lacks about this change: the Added entry for the key, the Changed entry
/// for the rule, and that no key leaves every verdict as it was.
fn changelog_problems(section: &str) -> Vec<String> {
    let part = |name: &str| -> String { section.split("\n### ").find(|part| part.starts_with(name)).unwrap_or_default().to_string() };
    let (added, changed) = (part("Added"), part("Changed"));
    let mut problems = Vec::new();
    let added_entry: Vec<&str> = added.lines().filter(|l| l.contains("canon-0-38-code-scopes")).collect();
    let changed_entry: Vec<&str> = changed.lines().filter(|l| l.contains("canon-0-38-code-scopes")).collect();
    if added_entry.is_empty() {
        problems.push("no entry of `canon-0-38-code-scopes` under `### Added`".to_string());
    }
    if changed_entry.is_empty() {
        problems.push("no entry of `canon-0-38-code-scopes` under `### Changed`".to_string());
    }
    for (what, needle) in [
        ("the policy key", "`code_scopes`"),
        ("the keys, an area or a surface", "surface"),
        ("that the policy is approved by a human", "approved"),
        ("that `canon brief approve` leaves the policy approval stale when the scopes changed", "stale"),
    ] {
        if !added_entry.join("\n").contains(needle) {
            problems.push(format!("the Added entry does not say {what} (`{needle}`)"));
        }
    }
    for (what, needle) in [
        ("that the gate asks for less", "asks for no new evidence"),
        ("that it is a chore's approval the rule is about", "chore"),
        ("that the changed paths decide", "changed since"),
        ("that a policy without the key behaves as before", "without the key"),
        ("that a key whose patterns match no file declares no scope", "none of whose patterns matches a file"),
        ("that an empty list is malformed", "list is empty"),
    ] {
        if !changed_entry.join("\n").contains(needle) {
            problems.push(format!("the Changed entry does not say {what} (`{needle}`)"));
        }
    }
    problems
}

/// gate.code-scopes.12: the CHANGELOG names the new key and the cases the gate asks less for.
#[test]
fn gate_code_scopes_12_the_changelog_names_the_key_and_what_the_gate_asks_less() {
    assert_eq!(changelog_problems(&unreleased()), Vec::<String>::new());
}

/// gate.code-scopes.13: a CHANGELOG without the entries, or without a piece of one, is named by the release check.
#[test]
fn gate_code_scopes_13_a_changelog_without_the_entries_is_named_by_the_release_check() {
    let section = unreleased();
    assert_eq!(changelog_problems("## Unreleased\n\n### Added\n\n- Something else.\n").len(), 2 + 4 + 6, "an empty section names both entries and each piece");
    let without_added: String = section.lines().filter(|l| !(l.contains("canon-0-38-code-scopes") && l.contains("`code_scopes`"))).collect::<Vec<_>>().join("\n");
    assert!(changelog_problems(&without_added).iter().any(|p| p.contains("no entry") && p.contains("Added")), "{:?}", changelog_problems(&without_added));
    let without_before = section.replace("without the key", "otherwise");
    assert!(changelog_problems(&without_before).iter().any(|p| p.contains("without the key")), "{:?}", changelog_problems(&without_before));
}
