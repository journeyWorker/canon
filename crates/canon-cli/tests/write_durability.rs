//! 0.39 `canon-0-39-write-durability`: the source check that no write copy is left outside `canon-store`, and the release check for the
//! CHANGELOG. The notes of the change say what is fixed (directory fsync,
//! one write path, one-write audit lines, the `rekey` write lock) and what changed (a new `hot.db` is 0600), that it is Unix only and
//! Windows is as before, and how to narrow an existing `hot.db`.

use std::path::Path;

/// The section of the release this change ships in: `## 0.39.0`.
fn section() -> String {
    let changelog = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).expect("the CHANGELOG");
    let start = changelog.find("\n## 0.39.0").expect("the CHANGELOG has a 0.39.0 section");
    let end = changelog[start + 1..].find("\n## ").map_or(changelog.len(), |at| start + 1 + at);
    changelog[start..end].to_string()
}

/// What the section lacks about this change.
fn changelog_problems(section: &str) -> Vec<String> {
    let part = |name: &str| section.split("\n### ").find(|part| part.starts_with(name)).unwrap_or_default().to_string();
    let (fixed, changed) = (part("Fixed"), part("Changed"));
    let mut problems = Vec::new();
    for (heading, text, needles) in [
        (
            "Fixed",
            &fixed,
            &[
                ("the directory fsync after the rename", "directory fsync"),
                ("the artifact blobs", "artifact"),
                ("the four private write copies", "private"),
                ("the audit line in one write", "one write"),
                ("the rekey write lock", "BEGIN IMMEDIATE"),
                ("Windows is as before", "Windows"),
            ][..],
        ),
        ("Changed", &changed, &[("a new hot.db is 0600", "0600"), ("how to narrow an existing hot.db", "chmod 600 .canon/hot.db"), ("Windows is as before", "Windows")][..]),
    ] {
        if !text.lines().any(|l| l.contains("canon-0-39-write-durability")) {
            problems.push(format!("no entry of `canon-0-39-write-durability` under `### {heading}`"));
        }
        for (what, needle) in needles {
            if !text.contains(needle) {
                problems.push(format!("the `{heading}` entry does not say {what} (`{needle}`)"));
            }
        }
    }
    problems
}

/// tier.durability.15 (F8): the CHANGELOG names the fixes and the new `hot.db` mode.
#[test]
fn tier_durability_15_the_changelog_names_the_durability_fixes_and_the_new_hot_db_mode() {
    assert_eq!(changelog_problems(&section()), Vec::<String>::new());
}

/// tier.durability.16 (F8 failure path): a CHANGELOG without the entry or a piece of it is named by the release check.
#[test]
fn tier_durability_16_a_changelog_without_the_entry_or_a_piece_of_it_is_named_by_the_release_check() {
    let section = section();
    assert_eq!(changelog_problems("## Unreleased\n\n### Fixed\n\n- Something else.\n").len(), 2 + 6 + 3, "an empty section names both entries and each piece of them");
    let without_entry: String = section.lines().filter(|l| !l.contains("canon-0-39-write-durability")).collect::<Vec<_>>().join("\n");
    assert!(changelog_problems(&without_entry).iter().any(|p| p.contains("no entry")), "{:?}", changelog_problems(&without_entry));
    let without_chmod = section.replace("chmod 600 .canon/hot.db", "make it narrower");
    assert!(changelog_problems(&without_chmod).iter().any(|p| p.contains("chmod 600 .canon/hot.db")), "{:?}", changelog_problems(&without_chmod));
    let without_lock = section.replace("BEGIN IMMEDIATE", "a lock");
    assert!(changelog_problems(&without_lock).iter().any(|p| p.contains("BEGIN IMMEDIATE")), "{:?}", changelog_problems(&without_lock));
}

/// tier.durability.06 (F4): the fixed temp names of the four private write copies (`canon.yaml`, the legacy migration, the brief migration, the strategy activation file) are
/// gone from every crate's source; the one write path is `canon_store::write_atomic`.
#[test]
fn tier_durability_06_no_source_outside_canon_store_keeps_a_private_temp_and_rename_write() {
    fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&crates).unwrap().flatten() {
        let src = entry.path().join("src");
        if src.is_dir() {
            sources(&src, &mut found);
        }
    }
    assert!(found.len() > 100, "the source scan found {} files", found.len());
    let gone = [".canon-migrate.tmp", "canon-migrate.tmp", "\"canon-tmp\"", "md.activation"];
    let left: Vec<String> = found
        .iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            gone.iter().find(|name| text.contains(*name)).map(|name| format!("{} still names `{name}`", path.display()))
        })
        .collect();
    assert_eq!(left, Vec::<String>::new());
}
