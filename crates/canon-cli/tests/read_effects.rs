//! `effect: read` in `canon schema` is a promise (#110): a command declared
//! `read` leaves the repository tree as it found it. Agents and sandboxes rely
//! on it, so this test runs every `read` leaf in a scratch repo with a
//! populated `.canon` (subject, change, scenario, evidence, review, finding)
//! and a scenario index that is out of date, then compares the whole tree
//! (every file's content, ignored files included, `.git` excluded) and
//! `git status --porcelain` before and after.
//!
//! The only writes a leaf may make are the paths its `effectNote` names, and
//! only when they are a cache the user never commits.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// What a `read` leaf may write, which its `effectNote` must name. A path here
/// is gitignored by `canon init`; the test checks that too.
const ALLOWED_WRITES: &[(&str, &[&str])] = &[("query", &[".canon/audit/access.jsonl"])];

/// Invocations beyond each leaf's schema example that reach a different read
/// path: `--json` renderings, other record kinds, the routes that used to
/// refresh the scenario index, open the hot tier or seed the report's roots.
/// The first word is the leaf the invocation belongs to.
const EXTRA_INVOCATIONS: &[&str] = &[
    "status --json",
    "status --fields subjects --json",
    "gate check --json",
    "gate check --release",
    "artifacts verify --full",
    "dispatch diff --json",
    "query --kind scenario --json",
    "query --kind evidence --json",
    "query --kind task",
    "query --kind run",
    "query --kind change --history",
    "finding list --change-id add-login --json",
    "divergence status --json",
    "format .canon/specs --json",
    "brief check --json",
    "policy show --json",
    "context --json",
    "retrieve --role reviewer --domain dev --json",
];

/// Commands that are not `read` leaves but promise a read-only mode.
const READ_ONLY_MODES: &[&str] = &["report --check", "report --check --json"];

fn canon(dir: &Path, home: &Path, args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(args)
        .current_dir(dir)
        .env("CANON_ACTOR", "read-effects")
        .env("HOME", home)
        .env("CANON_SKILLS_SOURCE", concat!(env!("CARGO_MANIFEST_DIR"), "/../../canon/skills"))
        .output()
        .expect("spawn canon binary")
}

fn ok(dir: &Path, home: &Path, args: &[&str]) {
    let out = canon(dir, home, &args.iter().map(|a| a.to_string()).collect::<Vec<_>>());
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A git repo whose `.canon` holds one of every record the read commands
/// look at, committed, with a scenario added to the specs after the last index
/// sync (so a reader that refreshed the index would write).
fn fixture(home: &Path) -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "."]);
    ok(repo, home, &["init", "--providers", "none"]);
    ok(repo, home, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, home, &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    ok(repo, home, &["feature", "new", "auth.login", "--title", "Login"]);
    let feature = ".canon/specs/kind=feature/area=auth/login.feature";
    ok(repo, home, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", feature]);
    ok(repo, home, &["inventory", "sync"]);
    ok(
        repo,
        home,
        &["evidence", "add", "--project-id", "root", "--scenario-id", "auth.login.01", "--kind", "test-run", "--ref", "npm test", "--verdict", "faithful", "--summary", "1 passed", "--actor-id", "impl", "--session-id", "s1", "--role", "implementer"],
    );
    ok(
        repo,
        home,
        &["review", "add", "--project-id", "root", "--scenario-id", "auth.login.01", "--reviewer", "alice", "--pin", "abc123", "--role", "reviewer", "--session-id", "s2", "--original-spec-ref", "docs/prd.md"],
    );
    ok(repo, home, &["finding", "add", "--change-id", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--reviewer", "bob", "--summary", "Naming nit"]);
    ok(repo, home, &["gate", "promote"]);
    // Specs ahead of the index: no `inventory sync` after this.
    ok(repo, home, &["scenario", "new", "auth.login.02", "--title", "Sign out", "--feature", feature]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture"]);
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Every file and directory under `root` except `.git`: relative path to
/// content (`None` for a directory). Ignored files are included, so a write
/// `git status` cannot see still shows.
fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_path_buf();
            if rel == Path::new(".git") {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                out.insert(rel, None);
                walk(root, &path, out);
            } else {
                out.insert(rel, Some(std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// The paths whose existence or content differs between two trees.
fn changed(before: &BTreeMap<PathBuf, Option<Vec<u8>>>, after: &BTreeMap<PathBuf, Option<Vec<u8>>>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = before.keys().chain(after.keys()).filter(|p| before.get(*p) != after.get(*p)).cloned().collect();
    paths.sort();
    paths.dedup();
    paths
}

fn words(line: &str) -> Vec<String> {
    let (mut out, mut current, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut current));
                    any = false;
                }
            }
            c => {
                current.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(current);
    }
    out
}

fn schema(dir: &Path, home: &Path, args: &[&str]) -> Value {
    let mut argv: Vec<String> = vec!["schema".into()];
    argv.extend(args.iter().map(|a| a.to_string()));
    argv.push("--json".into());
    let out = canon(dir, home, &argv);
    serde_json::from_slice::<Value>(&out.stdout).unwrap_or_else(|e| panic!("{argv:?}: {e}"))["data"].clone()
}

/// Runs `args` on a fresh copy of the fixture and returns what it changed.
fn run_on_copy(base: &Path, home: &Path, args: &[String]) -> (Vec<PathBuf>, String, String, Output) {
    let scratch = TempDir::new().unwrap();
    let repo = scratch.path().join("repo");
    copy_dir(base, &repo);
    let (tree_before, status_before) = (tree(&repo), git(&repo, &["status", "--porcelain"]));
    let out = canon(&repo, home, args);
    let (tree_after, status_after) = (tree(&repo), git(&repo, &["status", "--porcelain"]));
    (changed(&tree_before, &tree_after), status_before, status_after, out)
}

#[test]
fn every_read_leaf_leaves_the_repo_tree_unchanged_or_declares_its_write() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    let listing = schema(base.path(), home.path(), &[]);
    let leaves: Vec<String> = listing["commands"].as_array().unwrap().iter().filter(|c| c["effect"] == "read").map(|c| c["command"].as_str().unwrap().to_string()).collect();
    assert!(leaves.len() > 15, "the schema lists the read leaves: {leaves:?}");

    let mut invocations: Vec<(String, Vec<String>)> = Vec::new();
    for leaf in &leaves {
        let detail = schema(base.path(), home.path(), &leaf.split(' ').collect::<Vec<_>>());
        let example = detail["example"].as_str().unwrap_or_else(|| panic!("`{leaf}` has no example"));
        let mut argv = words(example);
        argv.remove(0);
        invocations.push((leaf.clone(), argv));
    }
    for line in EXTRA_INVOCATIONS.iter().chain(READ_ONLY_MODES) {
        let argv = words(line);
        let leaf = leaves.iter().chain(std::iter::once(&"report".to_string())).filter(|l| line.starts_with(l.as_str())).max_by_key(|l| l.len()).unwrap_or_else(|| panic!("`{line}` belongs to no read leaf")).clone();
        invocations.push((leaf, argv));
    }

    // The declared writes: the table, the notes and the gitignore agree.
    let gitignore = std::fs::read_to_string(base.path().join(".gitignore")).unwrap();
    for (leaf, paths) in ALLOWED_WRITES {
        let detail = schema(base.path(), home.path(), &leaf.split(' ').collect::<Vec<_>>());
        let note = detail["effectNote"].as_str().unwrap_or_else(|| panic!("`{leaf}` may write {paths:?} but has no effectNote"));
        for path in *paths {
            assert!(note.contains(path), "`{leaf}`'s effectNote must name {path}: {note}");
            let ignored_dir = format!("{}/", Path::new(path).parent().unwrap().display());
            assert!(gitignore.contains(path) || gitignore.contains(&ignored_dir), "{path} is a write the user never commits, so `canon init` must gitignore it");
        }
    }
    for leaf in &leaves {
        let detail = schema(base.path(), home.path(), &leaf.split(' ').collect::<Vec<_>>());
        if detail["effectNote"].is_string() {
            assert!(ALLOWED_WRITES.iter().any(|(l, _)| l == leaf), "`{leaf}` declares a read-effect note the test does not know: add it to ALLOWED_WRITES");
        }
    }

    let mut problems = Vec::new();
    for (leaf, argv) in &invocations {
        let allowed: Vec<PathBuf> = ALLOWED_WRITES.iter().filter(|(l, _)| l == leaf).flat_map(|(_, paths)| paths.iter().map(PathBuf::from)).collect();
        let (changed, status_before, status_after, out) = run_on_copy(base.path(), home.path(), argv);
        let unexpected: Vec<&PathBuf> = changed.iter().filter(|p| !allowed.iter().any(|a| a == *p || a.starts_with(p))).collect();
        if !unexpected.is_empty() {
            problems.push(format!("`canon {}` (leaf `{leaf}`) changed {unexpected:?}; stderr: {}", argv.join(" "), String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")));
        }
        if status_before != status_after && allowed.is_empty() {
            problems.push(format!("`canon {}` changed `git status --porcelain`:\n{status_before}\n->\n{status_after}", argv.join(" ")));
        }
        if status_before != status_after && !allowed.is_empty() {
            problems.push(format!("`canon {}` changed `git status --porcelain`, so its declared write is not a gitignored cache:\n{status_after}", argv.join(" ")));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// What `args` prints on a copy of the fixture, optionally after `canon inventory sync` brought the index up to date.
fn stdout_on_copy(base: &Path, home: &Path, sync_first: bool, args: &[&str]) -> String {
    let scratch = TempDir::new().unwrap();
    let repo = scratch.path().join("repo");
    copy_dir(base, &repo);
    if sync_first {
        ok(&repo, home, &["inventory", "sync"]);
    }
    let out = canon(&repo, home, &args.iter().map(|a| a.to_string()).collect::<Vec<_>>());
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn status_and_gate_check_grade_a_stale_index_as_a_sync_would_leave_it_without_writing_it() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    for args in [&["gate", "check", "--json"][..], &["gate", "check"][..], &["status", "--json"][..], &["status"][..]] {
        let stale = stdout_on_copy(base.path(), home.path(), false, args);
        let synced = stdout_on_copy(base.path(), home.path(), true, args);
        let (changed, _, _, _) = run_on_copy(base.path(), home.path(), &args.iter().map(|a| a.to_string()).collect::<Vec<_>>());
        assert!(changed.is_empty(), "{args:?} must not write the index or its archive: {changed:?}");
        assert_eq!(stale, synced, "{args:?}: a reader that does not write still answers from the specs as they are");
    }
    let gate = stdout_on_copy(base.path(), home.path(), false, &["gate", "check"]);
    assert!(gate.contains("auth.login.02"), "the scenario the index lacks is graded: {gate}");
}

#[test]
fn dispatch_diff_answers_from_an_empty_hot_tier_without_creating_the_file() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    assert!(!base.path().join(".canon/hot.db").exists(), "the fixture has never written the hot tier");
    let argv: Vec<String> = ["dispatch", "diff", "--json"].iter().map(|a| a.to_string()).collect();
    let (changed, _, _, out) = run_on_copy(base.path(), home.path(), &argv);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(changed.is_empty(), "no hot.db, no WAL sidecar: {changed:?}");
    let body: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["declared"], 0, "{body}");
    assert_eq!(body["notes"].as_array().map(Vec::len), Some(0), "an absent hot tier is an empty one, not a note: {body}");
}
