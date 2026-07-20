//! Integration test for `canon skills install` (S0 task 6.2), exercising
//! the `native-launcher`-adjacent `skill-materialization` spec scenarios:
//! verbatim `.claude/` copy + flattened `.codex/` output + a
//! timestamp-free content-hash/version lock, and idempotence across two
//! consecutive runs with no source change.
//!
//! The checked-in fixture (`fixtures/skills-repo/`) is never mutated: each
//! test copies it into a fresh tempdir first, so `cargo test` stays
//! side-effect-free against the repo tree.

use std::fs;
use std::path::Path;

use canon_cli::skills::{self, Lock};

fn copy_fixture_into(tmp: &Path) {
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/skills-repo");
    copy_dir_recursive(&fixture_root, tmp);
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}

#[test]
fn install_materializes_claude_and_codex_and_lock() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture_into(tmp.path());

    let source = tmp.path().join("canon/skills");
    let target = tmp.path();

    let report = skills::install(&source, target).expect("install should succeed");
    assert_eq!(report.installed.len(), 1);
    let skill = &report.installed[0];
    assert_eq!(skill.name, "example-skill");
    assert_eq!(skill.version, 1);
    assert!(skill.changed);

    // Verbatim Claude Code copy.
    let claude_path = target.join(".claude/skills/example-skill/SKILL.md");
    let original = fs::read_to_string(source.join("example-skill/SKILL.md")).unwrap();
    let materialized = fs::read_to_string(&claude_path).unwrap();
    assert_eq!(materialized, original, "claude materialization must be byte-verbatim");

    // Flattened Codex convention: no YAML frontmatter delimiters, header + body present.
    let codex_path = target.join(".codex/skills/example-skill.md");
    let codex_content = fs::read_to_string(&codex_path).unwrap();
    assert!(codex_content.starts_with("# example-skill\n"));
    assert!(!codex_content.contains("---\nname:"));
    assert!(codex_content.contains("This is a fixture"));

    // Gemini is never touched (decision 11).
    assert!(!target.join(".gemini").exists());

    // Lock: content hash + monotonic version, no generatedAt field anywhere.
    let lock_path = source.join(".install-lock.json");
    let lock_raw = fs::read_to_string(&lock_path).unwrap();
    assert!(!lock_raw.contains("generatedAt"));
    let lock: Lock = serde_json::from_str(&lock_raw).unwrap();
    let entry = lock.skills.get("example-skill").expect("lock entry for example-skill");
    assert!(entry.content_hash.starts_with("sha256:"));
    assert_eq!(entry.version, 1);
}

#[test]
fn install_is_idempotent_with_no_source_change() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture_into(tmp.path());

    let source = tmp.path().join("canon/skills");
    let target = tmp.path();

    let first = skills::install(&source, target).expect("first install should succeed");
    let claude_after_first = fs::read_to_string(target.join(".claude/skills/example-skill/SKILL.md")).unwrap();
    let codex_after_first = fs::read_to_string(target.join(".codex/skills/example-skill.md")).unwrap();
    let lock_after_first = fs::read_to_string(source.join(".install-lock.json")).unwrap();

    let second = skills::install(&source, target).expect("second install should succeed");
    let claude_after_second = fs::read_to_string(target.join(".claude/skills/example-skill/SKILL.md")).unwrap();
    let codex_after_second = fs::read_to_string(target.join(".codex/skills/example-skill.md")).unwrap();
    let lock_after_second = fs::read_to_string(source.join(".install-lock.json")).unwrap();

    assert_eq!(claude_after_first, claude_after_second, "claude output must be byte-identical across reruns");
    assert_eq!(codex_after_first, codex_after_second, "codex output must be byte-identical across reruns");
    assert_eq!(lock_after_first, lock_after_second, "lock must be byte-identical across reruns");

    // Second run reports "unchanged" (content hash matched the existing lock entry).
    assert!(!second.installed[0].changed);
    assert_eq!(second.installed[0].version, first.installed[0].version);
}

#[test]
fn content_change_bumps_version_not_timestamp() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture_into(tmp.path());

    let source = tmp.path().join("canon/skills");
    let target = tmp.path();

    let first = skills::install(&source, target).expect("first install should succeed");
    assert_eq!(first.installed[0].version, 1);

    // Mutate the skill's source content.
    let skill_md = source.join("example-skill/SKILL.md");
    let mut content = fs::read_to_string(&skill_md).unwrap();
    content.push_str("\nAppended content to force a hash change.\n");
    fs::write(&skill_md, content).unwrap();

    let second = skills::install(&source, target).expect("second install should succeed");
    assert!(second.installed[0].changed);
    assert_eq!(second.installed[0].version, 2, "version increments by exactly one");
}
