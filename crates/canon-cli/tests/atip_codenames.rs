//! `canon-0-36-codes-rename`, scenario `atip.codenames.01` (`.03` and `.04` are in `codes_names.rs`): a code renamed by 0.36 answers the call that
//! raised its old name with its new code, exit, reason and field. Against the built `canon` binary.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::Path;
use std::process::{Command, Output};

use canon_learn::{ParquetStrategyStore, StrategyId, StrategyItem, StrategyItemFields, StrategyStore, TrajectoryId};
use canon_model::ids::{regime_key, RegimeKey, RoleId};
use chrono::Utc;
use serde_json::Value;
use tempfile::TempDir;

fn canon(dir: &Path, args: &[&str], role: Option<&str>) -> Output {
    let mut command = hermetic::canon();
    command.args(args).arg("--json").current_dir(dir).env_remove("CANON_ROLE").env_remove("CANON_ACTOR");
    if let Some(role) = role {
        command.env("CANON_ROLE", role);
    }
    command.output().expect("spawn canon")
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

/// A git repo with one commit, set up by `canon init`.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(d).output().unwrap();
        assert!(out.status.success(), "git {args:?}");
    }
    let out = canon(d, &["init", "--no-harness"], None);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    dir
}

/// Runs `args` and checks the failure: its exit, `error.code`, `error.reason`, `error.field` and that it is not retryable.
fn fails(dir: &Path, role: Option<&str>, args: &[&str], exit: i32, code: &str, reason: Option<&str>, field: Option<&str>) -> Value {
    let out = canon(dir, args, role);
    let b = body(&out);
    assert_eq!(out.status.code(), Some(exit), "{args:?}: {b}");
    assert_eq!(b["ok"], false, "{args:?}: {b}");
    let error = &b["error"];
    assert_eq!(error["code"], code, "{args:?}: {b}");
    assert_eq!(error["reason"].as_str(), reason, "{args:?}: {b}");
    assert_eq!(error["field"].as_str(), field, "{args:?}: {b}");
    assert_eq!(error["retryable"], false, "{args:?}: {b}");
    b
}

const CONTEXT_MANIFEST: &str = r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["README.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#;

fn fixture_regime() -> RegimeKey {
    RegimeKey::parse(regime_key("dev", "canon", "join-spine", "9c93d024b1a2")).unwrap()
}

fn seed_strategy(repo: &Path) -> StrategyId {
    let store = ParquetStrategyStore::open(repo.join(".canon").join("learn").join("strategies"));
    let id = StrategyId::new();
    let item = StrategyItem::new(StrategyItemFields {
        id,
        regime_key: fixture_regime(),
        role: RoleId::parse("dev").unwrap(),
        title: "review guidance".into(),
        description: "one-liner".into(),
        content: "content".into(),
        source_trajectory_ids: vec![TrajectoryId::new()],
        recorded_at: Utc::now(),
    });
    store.append(&item).expect("seed strategy");
    id
}

/// atip.codenames.01: the codes no other scenario raises by a call answer with their new name, the exit of the table, `retryable` false, the
/// reason and the `field` they had. The codes with a reason are asked for it, the others for none.
#[test]
fn atip_codenames_01_a_call_that_raised_an_old_name_raises_the_new_one_with_the_same_exit_field_and_reason() {
    // CONTEXT_PATH_INVALID and CONTEXT_INPUT_INVALID (reason SECRET): `context-pack add`.
    let dir = repo();
    let d = dir.path();
    std::fs::write(d.join("README.md"), "api_key=do-not-store\n").unwrap();
    std::fs::write(d.join("cm.json"), CONTEXT_MANIFEST).unwrap();
    fails(d, None, &["context-pack", "add", "--manifest", "cm.json"], 2, "CONTEXT_INPUT_INVALID", Some("SECRET"), None);
    std::fs::write(d.join("up.json"), CONTEXT_MANIFEST.replace("README.md", "../outside")).unwrap();
    fails(d, None, &["context-pack", "add", "--manifest", "up.json"], 2, "CONTEXT_PATH_INVALID", None, None);

    // FEATURE_PATH_INVALID: a `--feature` file outside every spec root.
    let outside = TempDir::new().unwrap();
    let feature = outside.path().join("x.feature");
    fails(d, Some("implementer"), &["scenario", "new", "a.b.01", "--title", "t", "--feature", feature.to_str().unwrap()], 2, "FEATURE_PATH_INVALID", None, Some("--feature"));

    // PLAN_SOURCES_REQUIRED: `--task` in a repo that configures no plan sources.
    fails(d, Some("implementer"), &["dispatch", "begin", "--role", "implementer", "--regime", "implementer/r/a/0123456789ab", "--task", "sp#1"], 2, "PLAN_SOURCES_REQUIRED", None, None);

    // CONFIG_INVALID (reason DASHBOARD): a `learn:` section the dashboard cannot read.
    let config = std::fs::read_to_string(d.join("canon.yaml")).unwrap();
    std::fs::write(d.join("canon.yaml"), format!("{config}\nlearn:\n  root: [1, 2]\n")).unwrap();
    fails(d, None, &["dashboard", "--port", "0"], 2, "CONFIG_INVALID", Some("DASHBOARD"), None);
    std::fs::write(d.join("canon.yaml"), config).unwrap();

    // FINDING_CONFLICT: a finding that is still staged cannot be closed.
    let added = canon(d, &["finding", "add", "--change", "c1", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "a1", "--summary", "s", "--no-promote"], Some("reviewer"));
    assert_eq!(added.status.code(), Some(0), "{}", String::from_utf8_lossy(&added.stdout));
    fails(d, Some("reviewer"), &["finding", "close", "c1", "--round", "1", "--seq", "1", "--disposition", "rejected", "--reason", "r", "--actor", "a1"], 2, "FINDING_CONFLICT", None, None);

    // SUBJECT_CONFLICT (reason TRANSITION): a status change off the chain.
    let made = canon(d, &["subject", "new", "demo", "--domain", "d", "--title", "T", "--actor", "a", "--session", "s"], Some("planner"));
    assert_eq!(made.status.code(), Some(0), "{}", String::from_utf8_lossy(&made.stdout));
    fails(d, Some("planner"), &["subject", "move", "demo", "--to", "shipped", "--actor", "a", "--session", "s"], 2, "SUBJECT_CONFLICT", Some("TRANSITION"), Some("--to"));

    // BRIEF_CHECK_FAILED (reason BLOCKERS) and BRIEF_INVALID (reason UNAPPROVABLE): the fresh brief holds placeholders; a brief that ends in an open fence takes no approval.
    let change = canon(d, &["change", "new", "add-login", "--subject", "demo", "--title", "Login"], Some("planner"));
    assert_eq!(change.status.code(), Some(0), "{}", String::from_utf8_lossy(&change.stdout));
    let checked = fails(d, Some("planner"), &["brief", "check", "add-login"], 1, "BRIEF_CHECK_FAILED", Some("BLOCKERS"), None);
    assert!(checked["data"]["findings"].is_array(), "the report stays in data: {checked}");
    std::fs::write(d.join(".canon/changes/add-login/brief.md"), "# x\n\n```\nopen\n").unwrap();
    let shown = body(&canon(d, &["brief", "show", "add-login", "--body"], Some("planner")));
    let digest = format!("add-login={}", shown["data"]["digest"].as_str().unwrap());
    fails(d, Some("orchestrator"), &["brief", "approve", "add-login", "--approver", "A", "--note", "n", "--relayed-via", "q", "--actor", "a", "--digest", &digest], 2, "BRIEF_INVALID", Some("UNAPPROVABLE"), None);

    // STRATEGY_FILE_INVALID and TRAJECTORY_NOT_FOUND: `strategy` on a seeded strategy.
    let id = seed_strategy(d).to_string();
    std::fs::write(d.join("eval.json"), "not json").unwrap();
    fails(d, None, &["strategy", "approval-payload", &id, "--evaluation", "eval.json", "--approver", "a", "--approval-at", "2030-01-01T00:00:00Z"], 2, "STRATEGY_FILE_INVALID", None, None);
    let ghost = TrajectoryId::new().to_string();
    fails(d, None, &["strategy", "rollback", &id, "--reason", "r", "--actor", "a", "--signature-file", "sig", "--approval-at", "2030-01-01T00:00:00Z", "--contradicting-trajectory", &ghost], 2, "TRAJECTORY_NOT_FOUND", None, None);
}

/// atip.codenames.01: TASK_UNSUPPORTED, the plan dialect that cannot be flipped in place (`superpowers`).
#[test]
fn atip_codenames_01_a_task_of_a_plan_dialect_that_cannot_be_flipped_is_task_unsupported() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    std::fs::write(d.join("canon.yaml"), "tiers:\n  local:\n    backend: git\n    root: .canon/ledger\nplans:\n  sources:\n    - dialect: superpowers\n      root: docs/plans\n").unwrap();
    std::fs::create_dir_all(d.join("docs/plans")).unwrap();
    std::fs::write(d.join("docs/plans/sp.md"), "# Plan\n\n### Task 1: Do it\n\n- [ ] step\n").unwrap();
    fails(d, None, &["task", "done", "sp#1"], 2, "TASK_UNSUPPORTED", None, None);
}

/// atip.codenames.01: SKILLS_CHECK_FAILED, reason DRIFT: a provider whose skills are not installed.
#[test]
fn atip_codenames_01_skills_check_that_finds_drift_is_skills_check_failed_with_the_reason_drift() {
    let dir = repo();
    let d = dir.path();
    let out = canon(d, &["skills", "check", "--harness", "claude"], None);
    let b = body(&out);
    assert_eq!(out.status.code(), Some(1), "{b}");
    assert_eq!((b["error"]["code"].as_str(), b["error"]["reason"].as_str(), b["error"]["retryable"].as_bool()), (Some("SKILLS_CHECK_FAILED"), Some("DRIFT"), Some(false)), "{b}");
}

/// atip.codenames.01: RETRIEVE_INVALID (it was `RETRIEVE_FAILED`, exit 2): a repo directory that cannot make a regime key refuses the retrieval (a `--domain` that is no kebab
/// slug is `USAGE` since 0.37, `canon-0-37-option-values`; the directory whose name is blank is the other way to a key the grammar refuses).
#[test]
fn atip_codenames_01_retrieve_with_a_domain_that_makes_no_regime_key_is_retrieve_invalid() {
    let parent = TempDir::new().unwrap();
    let d = parent.path().join(" ");
    std::fs::create_dir(&d).unwrap();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(&d).output().unwrap();
        assert!(out.status.success(), "git {args:?}");
    }
    let out = canon(&d, &["init", "--no-harness"], None);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    fails(&d, Some("implementer"), &["retrieve", "--for-role", "implementer", "--domain", "dev"], 2, "RETRIEVE_INVALID", None, None);
    // A domain that is no slug is the usage of the option now.
    fails(&d, Some("implementer"), &["retrieve", "--for-role", "implementer", "--domain", ""], 2, "USAGE", None, Some("--domain"));
}
