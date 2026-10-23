//! The guide comes from the binary (0.28 D10): `canon skills show`, the stub `skills install`
//! writes, `init` without a `canon/skills` tree, and `skills check` on an older lock.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

const SKILLS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../canon/skills");

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env_remove("CANON_SKILLS_SOURCE").output().expect("spawn canon")
}

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

fn source(path: &str) -> Vec<u8> {
    std::fs::read(Path::new(SKILLS).join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn reference_stems() -> Vec<String> {
    let mut stems: Vec<String> = std::fs::read_dir(Path::new(SKILLS).join("reference")).unwrap().flatten().filter_map(|entry| entry.path().file_stem().map(|stem| stem.to_string_lossy().into_owned())).collect();
    stems.sort();
    stems
}

/// `skills show canon` prints the embedded source, byte for byte, in a directory that holds no repo.
#[test]
fn skills_show_prints_the_embedded_guide_bytes_without_a_repo() {
    let empty = tempfile::tempdir().unwrap();
    let out = canon(empty.path(), &["skills", "show", "canon"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, source("SKILL.src.md"));
    let role = canon(empty.path(), &["skills", "show", "role-implementer"]);
    assert_eq!(role.stdout, source("reference/role-implementer.md"));
    assert!(out.stderr.is_empty());
}

/// `--full` appends every reference after the core guide; on another name it changes nothing.
#[test]
fn skills_show_full_appends_every_reference_and_is_a_no_op_elsewhere() {
    let empty = tempfile::tempdir().unwrap();
    let full = String::from_utf8(canon(empty.path(), &["skills", "show", "canon", "--full"]).stdout).unwrap();
    assert!(full.starts_with(&String::from_utf8(source("SKILL.src.md")).unwrap()));
    let mut at = 0;
    for stem in reference_stems() {
        let text = String::from_utf8(source(&format!("reference/{stem}.md"))).unwrap();
        let found = full[at..].find(&text).unwrap_or_else(|| panic!("{stem} follows the core guide, in order"));
        at += found + text.len();
    }
    assert_eq!(canon(empty.path(), &["skills", "show", "role-planner", "--full"]).stdout, source("reference/role-planner.md"));
}

/// `--json` envelopes `{name, version, markdown}`.
#[test]
fn skills_show_json_carries_name_version_and_markdown() {
    let empty = tempfile::tempdir().unwrap();
    let out = canon(empty.path(), &["skills", "show", "canon", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let document = json(&out);
    assert_eq!(document["ok"], true);
    assert_eq!(document["data"]["name"], "canon");
    assert_eq!(document["data"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(document["data"]["markdown"].as_str().unwrap().as_bytes(), source("SKILL.src.md"));
    assert_eq!(document["meta"]["command"], "skills show");
}

/// The names are `canon` and each reference stem; `canon schema skills show` lists them.
#[test]
fn the_guide_names_are_the_schema_possible_values() {
    let empty = tempfile::tempdir().unwrap();
    let schema = json(&canon(empty.path(), &["schema", "skills", "show", "--json"]));
    let listed: Vec<String> = schema["data"]["operands"][0]["possibleValues"].as_array().unwrap().iter().map(|name| name.as_str().unwrap().to_string()).collect();
    let mut expected = vec!["canon".to_string()];
    expected.extend(reference_stems());
    assert_eq!(listed, expected);
    let unknown = canon(empty.path(), &["skills", "show", "not-a-guide", "--json"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(json(&unknown)["ok"], false);
}

/// A role that is not a role neither filters nor fails `skills show`; every real role reads it.
#[test]
fn skills_show_ignores_the_declared_role() {
    let empty = tempfile::tempdir().unwrap();
    for role in ["bogus", "planner", "implementer", "reviewer", "orchestrator"] {
        let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["skills", "show", "canon"]).current_dir(empty.path()).env("CANON_ROLE", role).output().unwrap();
        assert_eq!(out.status.code(), Some(0), "CANON_ROLE={role}");
        assert_eq!(out.stdout, source("SKILL.src.md"));
    }
}

/// `init` in an empty directory installs the stub and the scripts from the binary: no `canon/skills`
/// tree, no `reference/**`.
#[test]
fn init_installs_the_stub_without_a_skills_tree() {
    let repo = tempfile::tempdir().unwrap();
    assert!(!repo.path().join("canon").exists());
    let out = canon(repo.path(), &["init", "--providers", "claude,codex", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(!repo.path().join("canon").exists(), "init writes no canon/skills tree");
    for projection in [".claude/skills/canon", ".agents/skills/canon"] {
        let skill = std::fs::read_to_string(repo.path().join(projection).join("SKILL.md")).unwrap();
        assert!(skill.starts_with("---\nname: canon\ndescription: "), "{skill}");
        assert!(skill.contains(env!("CARGO_PKG_VERSION")), "the stub carries the binary version");
        assert!(skill.contains("canon skills show canon") && skill.contains("canon schema"));
        assert!(skill.len() < 3000, "a stub, not the guide");
        assert!(!repo.path().join(projection).join("reference").exists());
        assert_eq!(std::fs::read(repo.path().join(projection).join("scripts/canon-retrieve-pre-dispatch.sh")).unwrap(), source("scripts/canon-retrieve-pre-dispatch.sh"));
    }
    let lock: Value = serde_json::from_slice(&std::fs::read(repo.path().join(".canon/skills/.install-lock.json")).unwrap()).unwrap();
    assert!(!lock["files"].as_object().unwrap().keys().any(|key| key.contains("/reference/")));
    let agents = std::fs::read_to_string(repo.path().join("AGENTS.md")).unwrap();
    assert!(agents.contains("canon skills show canon"), "the AGENTS block points at the binary's guide");
    assert_eq!(canon(repo.path(), &["skills", "check"]).status.code(), Some(0));
    assert_eq!(json(&canon(repo.path(), &["skills", "install", "--json"]))["data"]["changed"], false, "a second install changes nothing");
}

/// `skills check` reports a stub installed by an older canon as outdated, exit 1.
#[test]
fn skills_check_reports_an_older_lock_as_outdated() {
    let repo = tempfile::tempdir().unwrap();
    assert_eq!(canon(repo.path(), &["init", "--providers", "claude"]).status.code(), Some(0));
    assert_eq!(canon(repo.path(), &["skills", "check"]).status.code(), Some(0));
    let lock_path = repo.path().join(".canon/skills/.install-lock.json");
    let mut lock: Value = serde_json::from_slice(&std::fs::read(&lock_path).unwrap()).unwrap();
    lock["canon_version"] = "0.1.0".into();
    std::fs::write(&lock_path, serde_json::to_vec_pretty(&lock).unwrap()).unwrap();
    let out = canon(repo.path(), &["skills", "check", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let document = json(&out);
    assert_eq!(document["error"]["code"], "SKILLS_DRIFT");
    assert_eq!(document["data"]["outdated"]["installedBy"], "0.1.0");
    assert_eq!(document["data"]["clean"], false);
    assert_eq!(canon(repo.path(), &["skills", "install"]).status.code(), Some(0));
    assert_eq!(canon(repo.path(), &["skills", "check"]).status.code(), Some(0), "install rewrites the lock");
}
