//! `canon-0-36-codes-rename`, scenarios `atip.codenames.01`, `.02`, `.05`, `.08` and `.09` (`.03` and `.04` are in `codes_names.rs`,
//! `.06` and `.07` in `old_names.rs`): a code renamed by 0.36 answers the call that raised its old name, the old names are in no
//! registry view and are refused by `schema --error`, and the CHANGELOG carries the replacement row of every code that left the registry.
//! Against the built `canon` binary.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Output};

use canon_learn::{ParquetStrategyStore, StrategyId, StrategyItem, StrategyItemFields, StrategyStore, TrajectoryId};
use canon_model::ids::{regime_key, RegimeKey, RoleId};
use chrono::Utc;
use serde_json::Value;
use tempfile::TempDir;

mod old_codes;

/// The old name and the new one of each code `canon-0-36-codes-rename` renames (the brief's table).
const RENAMED: &[(&str, &str)] = &[
    ("APPROVAL_INCOMPLETE", "APPROVAL_INVALID"),
    ("BRIEF_CHANGED_SINCE_PAGE_LOAD", "BRIEF_PAGE_STALE"),
    ("BRIEF_HAS_BLOCKERS", "BRIEF_CHECK_FAILED"),
    ("BRIEF_UNAPPROVABLE", "BRIEF_INVALID"),
    ("CHANGE_AMBIGUOUS", "CHANGE_REQUIRED"),
    ("CHANGE_INVALID_ARGUMENT", "CHANGE_ARGUMENT_INVALID"),
    ("CONFIG_MISSING", "CONFIG_REQUIRED"),
    ("CONTEXT_INPUT_MISSING", "CONTEXT_INPUT_NOT_FOUND"),
    ("CONTEXT_SECRET_DETECTED", "CONTEXT_INPUT_INVALID"),
    ("CONTEXT_UNSAFE_PATH", "CONTEXT_PATH_INVALID"),
    ("DASHBOARD_BAD_REQUEST", "DASHBOARD_REQUEST_INVALID"),
    ("DASHBOARD_CONFIG_INVALID", "CONFIG_INVALID"),
    ("DASHBOARD_ORIGIN_REFUSED", "DASHBOARD_REQUEST_DENIED"),
    ("DEMO_NOT_INITIALIZED", "DEMO_REQUIRED"),
    ("DISPATCH_INVALID_ARGUMENT", "DISPATCH_ARGUMENT_INVALID"),
    ("DISPATCH_ROLE_REGIME_MISMATCH", "DISPATCH_ROLE_CONFLICT"),
    ("DIVERGENCE_INVALID_ARGUMENT", "DIVERGENCE_ARGUMENT_INVALID"),
    ("FEATURE_OUTSIDE_SPEC_ROOTS", "FEATURE_PATH_INVALID"),
    ("FINDING_INVALID_ARGUMENT", "FINDING_ARGUMENT_INVALID"),
    ("FINDING_RESOLUTION_REV_UNEXPECTED", "FINDING_RESOLUTION_REV_CONFLICT"),
    ("FINDING_SEQ_OCCUPIED", "FINDING_EXISTS"),
    ("FINDING_STAGED", "FINDING_CONFLICT"),
    ("FINDING_UNKNOWN_SCENARIO", "SCENARIO_NOT_FOUND"),
    ("FORMAT_VIOLATIONS", "FORMAT_CHECK_FAILED"),
    ("GATE_TASK_INVALID_ARGUMENT", "TASK_ID_INVALID"),
    ("GATE_TASK_UNSUPPORTED", "TASK_UNSUPPORTED"),
    ("GATE_VIOLATIONS", "GATE_CHECK_FAILED"),
    ("HUMAN_ONLY", "ACTOR_DENIED"),
    ("INIT_CONFIG_EXISTS", "CONFIG_EXISTS"),
    ("INIT_INVALID_ARGUMENT", "INIT_ARGUMENT_INVALID"),
    ("LEARN_DEMOTION_FAILED", "STRATEGY_DEMOTION_FAILED"),
    ("LEARN_INVALID_ARGUMENT", "STRATEGY_ARGUMENT_INVALID"),
    ("LEARN_INVALID_INPUT", "STRATEGY_FILE_INVALID"),
    ("LEARN_STRATEGY_NOT_FOUND", "STRATEGY_NOT_FOUND"),
    ("LEARN_TRAJECTORY_NOT_FOUND", "TRAJECTORY_NOT_FOUND"),
    ("MIGRATE_UNCOMMITTED", "MIGRATION_CONFLICT"),
    ("NO_COMMIT", "COMMIT_REQUIRED"),
    ("PLAN_SOURCES_MISSING", "PLAN_SOURCES_REQUIRED"),
    ("QUERY_INVALID_ARGUMENT", "QUERY_ARGUMENT_INVALID"),
    ("RELAY_DIGEST_MISMATCH", "RELAY_DIGEST_STALE"),
    ("RETRIEVE_FAILED", "RETRIEVE_INVALID"),
    ("REVIEW_INVALID_ARGUMENT", "REVIEW_ARGUMENT_INVALID"),
    ("SCAFFOLD_INVALID_ARGUMENT", "SCAFFOLD_ARGUMENT_INVALID"),
    ("SEARCH_QUERY_EMPTY", "SEARCH_QUERY_REQUIRED"),
    ("SKILLS_DRIFT", "SKILLS_CHECK_FAILED"),
    ("SPEC_ROOT_UNRESOLVED", "SPEC_ROOT_REQUIRED"),
    ("SUBJECT_INVALID_ARGUMENT", "SUBJECT_ARGUMENT_INVALID"),
    ("SUBJECT_INVALID_TRANSITION", "SUBJECT_CONFLICT"),
    ("UNKNOWN_ROLE", "ROLE_INVALID"),
    ("UNKNOWN_SPEC_ROOT", "SPEC_ROOT_NOT_FOUND"),
];

fn canon(dir: &Path, args: &[&str], role: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
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
    let out = canon(d, &["init", "--providers", "none"], None);
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
    let out = canon(d, &["skills", "check", "--providers", "claude"], None);
    let b = body(&out);
    assert_eq!(out.status.code(), Some(1), "{b}");
    assert_eq!((b["error"]["code"].as_str(), b["error"]["reason"].as_str(), b["error"]["retryable"].as_bool()), (Some("SKILLS_CHECK_FAILED"), Some("DRIFT"), Some(false)), "{b}");
}

fn registry_text(args: &[&str]) -> String {
    let out = canon(Path::new(env!("CARGO_MANIFEST_DIR")), args, None);
    assert_eq!(out.status.code(), Some(0), "{args:?}");
    String::from_utf8(out.stdout).unwrap()
}

/// `word` in `text` as a whole identifier.
fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        let part = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        !part(before) && !part(after)
    })
}

/// atip.codenames.02: no registry view, no `errors` of a command and no failure names an old name; `CONFIG_MISSING` is still the warning's name.
#[test]
fn atip_codenames_02_an_old_name_is_in_no_registry_view_and_no_command_lists_it() {
    let all = registry_text(&["schema", "--errors"]);
    let tree = registry_text(&["schema"]);
    for (old, new) in RENAMED {
        if *old == "CONFIG_MISSING" {
            continue;
        }
        assert!(!has_word(&all, old), "`{old}` is in `canon schema --errors`");
        assert!(!has_word(&tree, old), "`{old}` is in `canon schema`");
        assert!(has_word(&all, new), "`{new}` is not in `canon schema --errors`");
    }
    assert!(!has_word(&all, "CONFIG_MISSING"), "the error `CONFIG_MISSING` is `CONFIG_REQUIRED`: only the warning keeps the name");
    assert!(has_word(&registry_text(&["schema", "--warnings"]), "CONFIG_MISSING"), "the warning keeps its name");
}

/// atip.codenames.02 (failure case): a call site that builds an old name as a string is not in the registry, so the registry-match test of the sources fails.
#[test]
fn atip_codenames_02_a_source_that_builds_an_old_name_does_not_match_the_registry() {
    let registered: BTreeSet<&str> = canon_cli::cli_error::all_codes().iter().map(|info| info.code).collect();
    for (old, new) in RENAMED {
        if *old == "CONFIG_MISSING" {
            continue;
        }
        assert!(!registered.contains(old), "`{old}` is registered");
        assert!(registered.contains(new), "`{new}` is not registered");
    }
    let sources = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/grammar_t5_errors.rs")).unwrap();
    assert!(sources.contains("every_code_a_source_builds_is_registered"), "the registry-match test of the sources exists");
}

/// atip.codenames.05: `schema --error <old name>` is refused as any unknown name is: `USAGE`, `field` `--error`, with no word about a rename.
#[test]
fn atip_codenames_05_schema_error_refuses_each_old_name_as_an_unknown_code() {
    let dir = TempDir::new().unwrap();
    let unknown = body(&canon(dir.path(), &["schema", "--error", "NOSUCH_CODE"], None));
    for (old, _) in RENAMED.iter().filter(|(old, _)| *old != "CONFIG_MISSING") {
        let out = canon(dir.path(), &["schema", "--error", old], None);
        let b = body(&out);
        assert_eq!(out.status.code(), Some(2), "{old}: {b}");
        assert_eq!((b["error"]["code"].as_str(), b["error"]["field"].as_str(), b["error"]["received"].as_str()), (Some("USAGE"), Some("--error"), Some(*old)), "{old}: {b}");
        let message = b["error"]["message"].as_str().unwrap();
        assert_eq!(message.replace(old, "X"), unknown["error"]["message"].as_str().unwrap().replace("NOSUCH_CODE", "X"), "{old}: the answer is the one an unknown code gets");
        assert!(!message.to_lowercase().contains("renamed"), "{message}");
    }
}

/// The 0.35 code names of the registry (`tests/fixtures/codes-0.35.tsv`).
fn codes_0_35() -> Vec<String> {
    old_codes::codes_0_35().iter().map(|code| (*code).to_string()).collect()
}

/// The rows of the replacement table of the CHANGELOG's Unreleased section: `(old, new)`. A cell may name several codes (a code split by
/// condition names each new code; two old codes may share one row): every backticked code name of the cell counts, other backticked text
/// (an option, a reason) does not.
fn changelog_rows(changelog: &str) -> Vec<(String, String)> {
    let start = changelog.find("\n## Unreleased").expect("an Unreleased section");
    let rest = &changelog[start + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |n| n + 3);
    let codes = |cell: &str| -> Vec<String> {
        cell.split('`').skip(1).step_by(2).filter(|c| !c.is_empty() && c.chars().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')).map(str::to_string).collect()
    };
    // The table of reasons (`| Code | Reasons now | …`) names reasons, not renames: its rows are not rows of old and new codes.
    let mut in_reasons_table = false;
    rest[..end]
        .lines()
        .filter(|line| {
            if line.starts_with("| Code | Reasons now |") {
                in_reasons_table = true;
            } else if !line.starts_with('|') {
                in_reasons_table = false;
            }
            !in_reasons_table
        })
        .flat_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            match cells.as_slice() {
                ["", old, new, ..] => {
                    let (olds, news) = (codes(old), codes(new));
                    olds.iter().flat_map(|o| news.iter().map(move |n| (o.clone(), n.clone()))).collect::<Vec<_>>()
                }
                _ => Vec::new(),
            }
        })
        .collect()
}

/// atip.codenames.08: every code that left the registry since 0.35 has its row in the CHANGELOG, and the rows say the new name the registry holds.
#[test]
fn atip_codenames_08_the_changelog_holds_a_row_for_each_code_that_left_the_registry() {
    let changelog = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).unwrap();
    let rows = changelog_rows(&changelog);
    let registered: BTreeSet<&str> = canon_cli::cli_error::all_codes().iter().map(|info| info.code).collect();
    for (old, new) in RENAMED {
        let row = rows.iter().find(|(o, _)| o == old).unwrap_or_else(|| panic!("the CHANGELOG has no row for `{old}`"));
        assert_eq!(row.1, *new, "`{old}`");
    }
    for (old, new) in &rows {
        assert!(registered.contains(new.as_str()), "the row `{old}` → `{new}` names a code the registry does not hold");
    }
    let missing: Vec<String> = codes_0_35().into_iter().filter(|code| !registered.contains(code.as_str())).filter(|code| !rows.iter().any(|(old, _)| old == code)).collect();
    assert!(missing.is_empty(), "codes that left the registry with no row in the CHANGELOG: {missing:?}");
}

/// atip.codenames.09 (failure case): a code of 0.35 that is not in the registry and not in the table is found; so is a row that names a code the registry lacks.
#[test]
fn atip_codenames_09_a_code_missing_from_the_registry_and_from_the_table_is_found() {
    let table = "\n## Unreleased\n\n| 0.35 | 0.36 | `reason` |\n|---|---|---|\n| `HUMAN_ONLY` | `ACTOR_DENIED` | |\n| `NO_COMMIT` | `NO_SUCH_CODE` | |\n\n## 0.35.0\n";
    let rows = changelog_rows(table);
    assert_eq!(rows, [("HUMAN_ONLY".to_string(), "ACTOR_DENIED".to_string()), ("NO_COMMIT".to_string(), "NO_SUCH_CODE".to_string())]);
    let registered: BTreeSet<&str> = canon_cli::cli_error::all_codes().iter().map(|info| info.code).collect();
    assert!(!registered.contains("NO_SUCH_CODE"), "the row names a code the registry lacks");
    let gone = ["HUMAN_ONLY", "NO_COMMIT", "UNKNOWN_ROLE"];
    let missing: Vec<&str> = gone.into_iter().filter(|code| !rows.iter().any(|(old, _)| old == code)).collect();
    assert_eq!(missing, ["UNKNOWN_ROLE"], "a code that left without a row is named");
    assert!(codes_0_35().iter().any(|code| code == "HUMAN_ONLY"), "the fixture holds the 0.35 names");
}

/// atip.codenames.01: RETRIEVE_INVALID (it was `RETRIEVE_FAILED`, exit 2): a `--domain` that cannot make a regime key refuses the retrieval.
#[test]
fn atip_codenames_01_retrieve_with_a_domain_that_makes_no_regime_key_is_retrieve_invalid() {
    let dir = repo();
    fails(dir.path(), Some("implementer"), &["retrieve", "--for-role", "implementer", "--domain", ""], 2, "RETRIEVE_INVALID", None, None);
}
