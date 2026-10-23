//! `git_rev`: revision resolution and the code tree (0.28 D7b).

use std::path::Path;
use std::process::Command;

use canon_cli::git_rev::{code_exclude, code_tree, resolve_rev};
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").current_dir(repo).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn write(repo: &Path, rel: &str, body: &str) {
    let path = repo.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn init() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "t"]);
    git(dir.path(), &["config", "user.email", "t@example.com"]);
    dir
}

fn commit_all(repo: &Path, msg: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", msg]);
}

fn seeded() -> TempDir {
    let dir = init();
    let p = dir.path();
    write(p, "src/main.rs", "fn main() {}\n");
    write(p, ".gitignore", "target/\n");
    write(p, ".canon/specs/kind=feature/area=a/x.feature", "Feature: x\n");
    write(p, "docs/guide.txt", "guide\n");
    write(p, "README.md", "readme\n");
    commit_all(p, "one");
    dir
}

fn tree(repo: &Path) -> String {
    code_tree(repo, None).unwrap().tree
}

#[test]
fn rev_forms_resolve_to_the_full_sha() {
    let dir = seeded();
    let p = dir.path();
    let first = git(p, &["rev-parse", "HEAD"]);
    write(p, "src/lib.rs", "pub fn f() {}\n");
    commit_all(p, "two");
    let head = git(p, &["rev-parse", "HEAD"]);
    assert_ne!(head, first);
    assert_eq!(resolve_rev(p, "HEAD").unwrap(), head);
    assert_eq!(resolve_rev(p, &head[..8]).unwrap(), head);
    assert_eq!(resolve_rev(p, "main").unwrap(), head);
    assert_eq!(resolve_rev(p, "HEAD~1").unwrap(), first);
    assert_eq!(head.len(), 40);
    assert_eq!(resolve_rev(p, "no-such-rev").unwrap_err().code, "NO_COMMIT");
}

#[test]
fn code_tree_of_the_working_tree_equals_the_commit_made_from_it() {
    let dir = seeded();
    let p = dir.path();
    let clean = code_tree(p, None).unwrap();
    assert!(!clean.dirty);
    write(p, "src/main.rs", "fn main() { println!(); }\n");
    write(p, "src/new.rs", "// untracked\n");
    let before = code_tree(p, None).unwrap();
    assert!(before.dirty);
    assert_ne!(before.tree, clean.tree);
    commit_all(p, "edit");
    let after = code_tree(p, Some("HEAD")).unwrap();
    assert_eq!(after.tree, before.tree);
    assert!(!after.dirty);
    assert!(!code_tree(p, None).unwrap().dirty);
    assert_eq!(code_tree(p, Some("HEAD~1")).unwrap().tree, clean.tree);
}

#[test]
fn deleted_files_are_absent() {
    let dir = seeded();
    let p = dir.path();
    write(p, "src/extra.rs", "x\n");
    commit_all(p, "extra");
    let with = tree(p);
    std::fs::remove_file(p.join("src/extra.rs")).unwrap();
    let without = code_tree(p, None).unwrap();
    assert_ne!(without.tree, with);
    assert!(without.dirty);
}

#[test]
fn only_non_code_edits_leave_the_tree_alone() {
    let dir = seeded();
    let p = dir.path();
    let base = tree(p);
    write(p, ".canon/specs/kind=feature/area=a/x.feature", "Feature: x changed\n");
    write(p, ".canon/ledger/record.json", "{}\n");
    write(p, "docs/deep/more.txt", "more\n");
    write(p, "NOTES.md", "notes\n");
    write(p, "src/sub/CHANGES.md", "changes\n");
    write(p, "target/out.bin", "bin\n");
    let now = code_tree(p, None).unwrap();
    assert_eq!(now.tree, base);
    assert!(!now.dirty);
    write(p, "src/main.rs", "fn main() { 1; }\n");
    assert_ne!(tree(p), base);
}

#[test]
fn configured_spec_roots_and_custom_excludes_apply() {
    let dir = seeded();
    let p = dir.path();
    write(p, "canon.yaml", "specs:\n  roots:\n    - id: root\n      root: behaviors\ncode:\n  exclude: [\"generated/**\", \"*.snap\"]\n");
    write(p, "behaviors/a.feature", "Feature: a\n");
    write(p, "generated/out.rs", "g\n");
    write(p, "src/t.snap", "s\n");
    write(p, "docs/guide.txt", "guide changed\n");
    commit_all(p, "config");
    let base = tree(p);
    assert_eq!(code_exclude(p).unwrap(), ["generated/**", "*.snap"]);
    write(p, "behaviors/a.feature", "Feature: a2\n");
    write(p, "generated/out.rs", "g2\n");
    write(p, "src/t.snap", "s2\n");
    assert_eq!(tree(p), base);
    // docs and markdown are code under a custom exclude list.
    write(p, "docs/guide.txt", "guide changed again\n");
    assert_ne!(tree(p), base);
}

#[test]
fn exclude_defaults_and_malformed_config() {
    let dir = init();
    assert_eq!(code_exclude(dir.path()).unwrap(), ["docs/**", "*.md"]);
    write(dir.path(), "canon.yaml", "code:\n  exclude: 5\n");
    assert_eq!(code_exclude(dir.path()).unwrap_err().code, "CONFIG_UNREADABLE");
}

#[test]
fn a_repo_without_commits_still_has_a_code_tree() {
    let dir = init();
    let p = dir.path();
    let empty = code_tree(p, None).unwrap();
    assert!(!empty.dirty);
    write(p, "src/a.rs", "a\n");
    write(p, "README.md", "r\n");
    let one = code_tree(p, None).unwrap();
    assert!(one.dirty);
    assert_ne!(one.tree, empty.tree);
    assert_eq!(code_tree(p, Some("HEAD")).unwrap_err().code, "NO_COMMIT");
    assert_eq!(resolve_rev(p, "HEAD").unwrap_err().code, "NO_COMMIT");
}

#[test]
fn outside_a_git_work_tree_is_no_commit() {
    let dir = TempDir::new().unwrap();
    assert_eq!(resolve_rev(dir.path(), "HEAD").unwrap_err().code, "NO_COMMIT");
    let err = code_tree(dir.path(), None).unwrap_err();
    assert_eq!((err.code, err.exit), ("NO_COMMIT", 2));
}
