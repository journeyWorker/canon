//! `canon migrate --revert` (0.30 `ledger-naming`, migration step 8): after a committed migrate it puts every file the
//! migrate touched back byte for byte (moves undone, `hot.db` restored, files the migrate wrote removed), and the git
//! index with them. Through the real binary on a 0.29 ledger fixture and on a 0.16 spec/openspec layout.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").env_remove("CANON_ROLE").current_dir(repo).output().expect("spawn canon")
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(repo).output().expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn write(repo: &Path, path: &str, body: &str) {
    let file = repo.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, body).unwrap();
}

fn commit_all(repo: &Path) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "state"]);
}

/// A fresh git repo holding the 0.29 ledger fixture (`.gitignore` is stored as `gitignore`), committed.
fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger-0.29"), dir.path());
    fs::rename(dir.path().join("gitignore"), dir.path().join(".gitignore")).unwrap();
    git(dir.path(), &["init", "-q"]);
    commit_all(dir.path());
    dir
}

/// Every file of the repo but `.git` and the backups, by path: bytes and permission bits.
fn tree(repo: &Path) -> BTreeMap<String, (Vec<u8>, u32)> {
    fn walk(repo: &Path, dir: &Path, out: &mut BTreeMap<String, (Vec<u8>, u32)>) {
        use std::os::unix::fs::PermissionsExt;
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(repo).unwrap().to_string_lossy().replace('\\', "/");
            if relative == ".git" || relative == ".canon/migrate-backup" {
                continue;
            }
            if path.is_dir() {
                out.insert(format!("{relative}/"), (Vec::new(), 0));
                walk(repo, &path, out);
            } else {
                out.insert(relative, (fs::read(&path).unwrap(), fs::metadata(&path).unwrap().permissions().mode() & 0o7777));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(repo, repo, &mut out);
    out
}

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("not json ({e}): {}", text(out)))
}

fn digest_of(out: &Output) -> String {
    json(out)["data"]["planDigest"].as_str().unwrap_or_else(|| panic!("no planDigest: {}", text(out))).to_string()
}

/// Migrate `repo` (preview, then commit), returning the migrate's backup dir.
fn migrate(repo: &Path) -> String {
    let preview = canon(repo, &["migrate", "--json"]);
    assert!(preview.status.success(), "{}", text(&preview));
    let committed = canon(repo, &["migrate", "--commit", "--plan-digest", &digest_of(&preview), "--json"]);
    assert!(committed.status.success(), "{}", text(&committed));
    json(&committed)["data"]["backupDir"].as_str().expect("the migrate reports its backup dir").to_string()
}

/// Revert `repo`: the preview writes nothing; the commit acts.
fn revert(repo: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["migrate", "--revert"];
    args.extend_from_slice(extra);
    let before = tree(repo);
    let mut preview_args = args.clone();
    preview_args.push("--json");
    let preview = canon(repo, &preview_args);
    assert!(preview.status.success(), "{}", text(&preview));
    assert_eq!(json(&preview)["data"]["committed"], false);
    assert_eq!(tree(repo), before, "the revert preview wrote something");
    let mut commit_args = args;
    let digest = digest_of(&preview);
    commit_args.extend(["--commit", "--plan-digest", &digest, "--json"]);
    let out = canon(repo, &commit_args);
    assert!(out.status.success(), "{}", text(&out));
    out
}

#[test]
fn reverting_the_migrate_of_a_0_29_ledger_restores_the_tree_and_the_index() {
    let dir = fixture_repo();
    let repo = dir.path();
    let (before, index, status) = (tree(repo), git(repo, &["ls-files", "-s"]), git(repo, &["status", "--porcelain"]));
    assert_eq!(status, "", "the fixture is committed");

    let backup = migrate(repo);
    assert_ne!(tree(repo), before, "the migrate moved and rewrote files");
    assert!(repo.join(&backup).join("tree-manifest.json").is_file(), "{backup} holds the snapshot");
    assert!(!repo.join(".canon/ledger/kind=evidence_record").exists(), "the old kind directory is gone after the migrate");

    let reverted = revert(repo, &[]);
    assert_eq!(json(&reverted)["data"]["committed"], true);
    let after = tree(repo);
    for (path, state) in &before {
        assert_eq!(after.get(path), Some(state), "{path} differs after the revert");
    }
    assert_eq!(after.keys().collect::<Vec<_>>(), before.keys().collect::<Vec<_>>(), "files appeared or vanished");
    assert_eq!(git(repo, &["ls-files", "-s"]), index, "the git index is back to the 0.29 paths (the `git mv`s undone)");
    assert_eq!(git(repo, &["status", "--porcelain", "--", ".", ":!.canon/migrate-backup"]), "", "a clean work tree again");
    assert!(repo.join(&backup).is_dir(), "the backup stays after the revert");
}

#[test]
fn a_second_revert_has_nothing_to_restore_and_a_migrate_after_a_revert_works() {
    let dir = fixture_repo();
    let repo = dir.path();
    let before = tree(repo);
    migrate(repo);
    revert(repo, &[]);

    let again = canon(repo, &["migrate", "--revert", "--json"]);
    assert!(again.status.success(), "{}", text(&again));
    assert_eq!(json(&again)["data"]["effects"].as_array().unwrap().len(), 0, "{}", text(&again));

    // The 0.29 repo migrates again, to the same 0.30 tree as the first time.
    migrate(repo);
    assert!(repo.join(".canon/ledger/kind=evidence").is_dir());
    revert(repo, &[]);
    assert_eq!(tree(repo), before);
}

#[test]
fn the_revert_preview_lists_the_undone_moves_and_a_wrong_digest_writes_nothing() {
    let dir = fixture_repo();
    let repo = dir.path();
    migrate(repo);
    let migrated = tree(repo);

    let preview = canon(repo, &["migrate", "--revert", "--json"]);
    let effects = json(&preview)["data"]["effects"].clone();
    let effects = effects.as_array().unwrap();
    assert!(effects.iter().any(|e| e["action"] == "remove" && e["path"].as_str().is_some_and(|p| p.starts_with(".canon/ledger/kind=evidence/"))), "{effects:?}");
    assert!(effects.iter().any(|e| e["action"] == "restore" && e["path"].as_str().is_some_and(|p| p.starts_with(".canon/ledger/kind=evidence_record/"))), "{effects:?}");
    assert!(effects.iter().any(|e| e["action"] == "restore" && e["path"] == ".canon/hot.db"), "hot.db is restored whole: {effects:?}");
    assert!(effects.iter().any(|e| e["action"] == "restore" && e["record"] == "git index"), "{effects:?}");

    let wrong = canon(repo, &["migrate", "--revert", "--commit", "--plan-digest", "0000", "--json"]);
    assert_eq!(wrong.status.code(), Some(2), "{}", text(&wrong));
    assert!(text(&wrong).contains("PLAN_STALE"), "{}", text(&wrong));
    assert_eq!(tree(repo), migrated, "a plan digest that does not match writes nothing");
}

#[test]
fn a_repo_no_migrate_ran_in_refuses_the_revert() {
    let dir = fixture_repo();
    let out = canon(dir.path(), &["migrate", "--revert", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("MIGRATE_REFUSED"), "{}", text(&out));
    let named = canon(dir.path(), &["migrate", "--revert", "--backup", ".canon/migrate-backup/none", "--json"]);
    assert_eq!(named.status.code(), Some(2), "{}", text(&named));
    // `--backup` is only a revert's option.
    assert_eq!(canon(dir.path(), &["migrate", "--backup", "x"]).status.code(), Some(2));
}

#[test]
fn the_revert_names_a_backup_and_undoes_the_other_steps_too() {
    // A 0.16 layout: spec corpus under specs/features, an openspec change canon wrote, an openspec source in canon.yaml.
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_path_buf();
    write(
        &repo,
        "canon.yaml",
        "tiers:\n  local:\n    backend: git\n    root: .canon/ledger\nrouting:\n  change: local\n  subject: local\n  scenario: local\n  review: local\n  evidence: local\n  finding: local\nspecs:\n  roots:\n    - id: root\n      root: specs\nplans:\n  sources:\n    - dialect: openspec\n      root: .\nwrite:\n  promote: auto\n",
    );
    write(
        &repo,
        "specs/features/kind=feature/area=cart/add.feature",
        "Feature: cart add\n  # canon: {\"schema\":1,\"at\":\"2026-10-09T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}\n\n  # canon: {\"schema\":1,\"at\":\"2026-10-09T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}\n  @subject:cart\n  @case:happy\n  @cart.add.01\n  Scenario: Adding an item puts it in the cart\n    When the shopper adds an item\n    Then the cart holds that item\n",
    );
    write(&repo, ".gitignore", "target/\n");
    git(&repo, &["init", "-q"]);
    commit_all(&repo);
    let before = tree(&repo);
    let index = git(&repo, &["ls-files", "-s"]);

    let backup = migrate(&repo);
    assert_ne!(tree(&repo), before, "the migrate moved the spec root and rewrote canon.yaml and .gitignore");
    assert!(repo.join(".canon/specs/kind=feature/area=cart/add.feature").is_file());

    revert(&repo, &["--backup", &backup]);
    // `.canon/` itself stays: it holds the backup, which the comparison leaves out.
    let mut after = tree(&repo);
    if !before.contains_key(".canon/") {
        after.remove(".canon/");
    }
    assert_eq!(after, before, "the spec root, canon.yaml, .gitignore and everything the migrate wrote are as before");
    assert_eq!(git(&repo, &["ls-files", "-s"]), index);
    assert_eq!(git(&repo, &["status", "--porcelain", "--", ".", ":!.canon/migrate-backup"]), "");
}
