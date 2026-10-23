//! D6 (0.28): the old spellings of the destructive leaves and of the leaves
//! renamed with them are usage errors that name the new form.

use std::process::{Command, Output};

fn canon(args: &[&str]) -> Output {
    let dir = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).arg("--json").current_dir(dir.path()).output().expect("spawn canon")
}

fn usage_hint(args: &[&str]) -> String {
    let out = canon(args);
    assert_eq!(out.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["error"]["code"], "USAGE", "{args:?}: {body}");
    body["error"]["hint"].as_str().unwrap_or_default().to_string()
}

#[test]
fn removed_destructive_spellings_exit_2_and_name_the_commit_form() {
    for args in [
        &["purge", "--kind", "session", "--before", "2026-01-01", "--plan"][..],
        &["purge", "--kind", "session", "--before", "2026-01-01", "--confirm", "abc"],
        &["artifacts", "move", "--tier", "cold", "--dry-run"],
        &["migrate", "--dry-run"],
        &["ledger", "compact", "--dry-run"],
        &["tier", "age", "--dry-run"],
    ] {
        assert!(usage_hint(args).contains("--commit --plan-digest"), "{args:?}");
    }
}

#[test]
fn renamed_leaves_and_options_exit_2_and_name_the_new_spelling() {
    assert!(usage_hint(&["evidence", "vault"]).contains("artifacts store"));
    assert!(usage_hint(&["ingest", "artifacts"]).contains("ingest records"));
    assert!(usage_hint(&["artifacts", "migrate", "--to", "cold"]).contains("artifacts move --tier"));
    assert!(usage_hint(&["artifacts", "move", "--to", "cold"]).contains("--tier"));
    assert!(usage_hint(&["ingest", "plans", "--dialect", "openspec", "--source", "plans"]).contains("--plan-source"));
}

#[test]
fn commit_and_plan_digest_come_together_and_purge_takes_only_its_four_kinds() {
    for args in [&["ledger", "compact", "--commit"][..], &["tier", "age", "--plan-digest", "abc"], &["migrate", "--commit"]] {
        assert_eq!(canon(args).status.code(), Some(2), "{args:?}");
    }
    assert_eq!(canon(&["purge", "--kind", "evidence", "--before", "2026-01-01"]).status.code(), Some(2), "evidence is not a purge kind");
}
