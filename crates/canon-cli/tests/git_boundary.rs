//! `canon-0-39-git-boundary` through the real binary: every git call canon makes goes through one path that clears the git
//! environment, reads file names NUL-terminated, and ends within a deadline and an output limit (F1-F5 of the brief).
//!
//! The deadline of a debug build is shortened with `CANON_TEST_GIT_DEADLINE_MS`; a `git` wrapper on the `PATH` that sleeps on
//! one subcommand plays the stuck git. The wrapper leaves its `sleep` running when canon kills it, so each test also shows that
//! canon does not wait for a grandchild that holds the pipes.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;

const SCENARIO: &str = "game.run.01";

// ── The binary and git ──

/// `canon <args> --json` in `dir` with `env` added; `PATH` is replaced by `path` when given.
fn canon_env(dir: &Path, args: &[&str], env: &[(&str, String)], path: Option<&std::ffi::OsStr>) -> Output {
    let mut command = hermetic::canon();
    command.args(args).arg("--json").current_dir(dir).env("CANON_ACTOR", "canon");
    for (name, value) in env {
        command.env(name, value);
    }
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command.output().expect("spawn canon")
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn ok(dir: &Path, args: &[&str]) -> Value {
    let out = canon_env(dir, args, &[], None);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    body(&out)
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(dir).output().expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn commit_all(dir: &Path, message: &str) -> String {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

fn write(dir: &Path, relative: &str, text: &str) {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A git repo that has nothing to do with the project: a different commit, a different tree, a different index.
fn other_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    write(dir.path(), "other.txt", "not the project\n");
    write(dir.path(), "second.txt", "not the project either\n");
    commit_all(dir.path(), "other");
    dir
}

/// A committed canon project with one scenario (`game.run.01`) and one source file.
fn project() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    ok(repo, &["scenario", "new", SCENARIO, "--title", "A run ends at zero health", "--case", "failure"]);
    ok(repo, &["inventory", "sync"]);
    write(repo, "src/main.rs", "fn main() {}\n");
    commit_all(repo, "project");
    dir
}

fn evidence_add_args<'a>(reference: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["evidence", "add", "--scenario", SCENARIO, "--project", "root", "--class", "test-run", "--ref", reference, "--role", "implementer"];
    args.extend_from_slice(extra);
    args
}

fn evidence_records(repo: &Path) -> Vec<Value> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(repo.join(".canon/ledger/kind=evidence")).map(|d| d.filter_map(Result::ok).map(|e| e.path()).collect()).unwrap_or_default();
    paths.sort();
    paths.iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect()
}

fn record_of<'a>(records: &'a [Value], reference: &str) -> &'a Value {
    records.iter().find(|record| record["evidence"]["ref"] == reference).unwrap_or_else(|| panic!("no record with ref {reference}: {records:?}"))
}

/// Every file of `dir` (not `.git`) with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().display().to_string(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The `PATH` that puts a `git` wrapper first: it logs the call and sleeps 30 seconds when its arguments match `pattern` (a `case` glob
/// over the arguments joined by spaces with one space on each side), and runs the real git otherwise. The `sleep` is a child of the
/// wrapper, so killing the wrapper leaves a grandchild that holds git's pipes.
fn stuck_git(dir: &Path, pattern: &str, log: &Path) -> std::ffi::OsString {
    let original = std::env::var_os("PATH").unwrap();
    let real = std::env::split_paths(&original).map(|d| d.join("git")).find(|p| p.is_file()).expect("git is on the PATH");
    let script = format!("#!/bin/sh\ncase \" $* \" in\n  {pattern})\n    echo \"$*\" >> '{}'\n    sleep 30\n    ;;\nesac\nexec '{}' \"$@\"\n", log.display(), real.display());
    let wrapper = dir.join("git");
    std::fs::write(&wrapper, script).unwrap();
    // The wrapper is a `/bin/sh` script: this test is for a unix host.
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::join_paths(std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&original))).unwrap()
}

const SHORT_DEADLINE: (&str, &str) = ("CANON_TEST_GIT_DEADLINE_MS", "1000");

fn short_deadline() -> [(&'static str, String); 1] {
    [(SHORT_DEADLINE.0, SHORT_DEADLINE.1.to_string())]
}

// ── F1: the environment ──

/// F1: a shell that points git at another repository does not point canon's git there. The record names the project's commit and code
/// tree, the ones a run without the variables records.
#[test]
fn f1_evidence_add_records_the_repos_commit_and_code_tree_whatever_git_dir_the_shell_holds() {
    let dir = project();
    let repo = dir.path();
    let other = other_repo();
    let other_head = git(other.path(), &["rev-parse", "HEAD"]);

    let plain = canon_env(repo, &evidence_add_args("plain", &[]), &[], None);
    assert_eq!(plain.status.code(), Some(0), "{}", String::from_utf8_lossy(&plain.stderr));
    // A second add of the same cell supersedes the first record, so each is read right after it is written.
    let plain = record_of(&evidence_records(repo), "plain").clone();
    let env = [("GIT_DIR", other.path().join(".git").display().to_string()), ("GIT_WORK_TREE", other.path().display().to_string())];
    let held = canon_env(repo, &evidence_add_args("held", &[]), &env, None);
    assert_eq!(held.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&held.stdout), String::from_utf8_lossy(&held.stderr));
    let held = record_of(&evidence_records(repo), "held").clone();
    let head = git(repo, &["rev-parse", "HEAD"]);
    assert_eq!(held["evidence_sha"], head.as_str(), "the commit of --repo, not of GIT_DIR's repository ({other_head})");
    assert_ne!(held["evidence_sha"], other_head.as_str());
    assert_eq!(held["evidence_sha"], plain["evidence_sha"]);
    assert!(held["code_tree"].is_string(), "{held}");
    assert_eq!(held["code_tree"], plain["code_tree"], "the code tree of --repo, not of GIT_WORK_TREE");
}

/// F1: the three object-store variables and `GIT_INDEX_FILE` are not read either; `git status` of a context pack is not computed against the
/// index the shell names, so the pack id (which holds the state digest) is the one a run without the variable makes.
#[test]
fn f1_a_git_index_file_in_the_shell_does_not_change_the_state_digest_of_a_context_pack() {
    let dir = project();
    let repo = dir.path();
    let other = other_repo();
    ok(repo, &["init", "--no-harness"]);
    write(repo, "doc.md", "hi\n");
    write(repo, "m.json", r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#);
    commit_all(repo, "pack inputs");
    // A tracked file changed and not committed, so the state digest holds a diff.
    write(repo, "src/main.rs", "fn main() { println!(\"changed\"); }\n");

    let plain = ok(repo, &["context-pack", "add", "--manifest", "m.json"]);
    let id = plain["data"]["id"].as_str().unwrap().to_string();
    let env = [("GIT_INDEX_FILE", other.path().join(".git/index").display().to_string())];
    let held = canon_env(repo, &["context-pack", "add", "--manifest", "m.json"], &env, None);
    assert_eq!(held.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&held.stdout), String::from_utf8_lossy(&held.stderr));
    assert_eq!(body(&held)["data"]["id"], id.as_str(), "the same state is the same pack");
    let shown = canon_env(repo, &["context-pack", "show", &id], &env, None);
    assert_eq!(shown.status.code(), Some(0), "{}", String::from_utf8_lossy(&shown.stderr));
    assert_eq!(body(&shown)["data"]["id"], id.as_str());
}

/// F1: `GIT_DIR` in the shell, naming a repository that tracks the same path, does not make `migrate --commit` stage its removal of
/// `openspec/changes/.gitkeep` in that repository's index: the move stays in the project.
#[test]
fn f1_migrate_commit_does_not_change_the_index_of_the_repository_git_dir_names() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    write(repo, "canon.yaml", MIGRATE_YAML);
    write(repo, "openspec/changes/.gitkeep", "");
    write(repo, ".gitignore", ".canon/hot.db*\n");
    commit_all(repo, "a 0.16 repo");
    let other = other_repo();
    write(other.path(), "openspec/changes/.gitkeep", "");
    commit_all(other.path(), "tracks the same placeholder");
    let index_before = std::fs::read(other.path().join(".git/index")).unwrap();
    let env = [("GIT_DIR", other.path().join(".git").display().to_string())];

    let preview = canon_env(repo, &["migrate"], &env, None);
    assert_eq!(preview.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&preview.stdout), String::from_utf8_lossy(&preview.stderr));
    let digest = body(&preview)["data"]["planDigest"].as_str().unwrap().to_string();
    let committed = canon_env(repo, &["migrate", "--commit", "--plan-digest", &digest], &env, None);
    assert_eq!(committed.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&committed.stdout), String::from_utf8_lossy(&committed.stderr));

    assert_eq!(std::fs::read(other.path().join(".git/index")).unwrap(), index_before, "the other repository's index is as it was");
    assert_eq!(git(other.path(), &["status", "--porcelain"]), "", "nothing of the project was staged in the other repository");
    assert!(!repo.join("openspec/changes/.gitkeep").exists(), "the migration ran in --repo");
    assert!(git(repo, &["status", "--porcelain"]).contains("openspec/changes/.gitkeep"), "and git of --repo staged its removal");
}

const MIGRATE_YAML: &str = "\
tiers:
  local:
    backend: git
    root: .canon/ledger
  hot:
    backend: sqlite
    path: .canon/hot.db
specs:
  roots:
    - id: root
      root: specs
plans:
  sources:
    - dialect: openspec
      root: .
write:
  promote: auto
";

/// F1, failure path: a directory that is no git work tree stays `COMMIT_REQUIRED` when the shell names a git directory: the variable is not
/// a way to make `--repo` a repository.
#[test]
fn f1_git_dir_does_not_make_a_directory_that_is_not_a_work_tree_one() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    ok(repo, &["scenario", "new", SCENARIO, "--title", "A run ends at zero health", "--case", "failure"]);
    ok(repo, &["inventory", "sync"]);
    let other = other_repo();

    let without = canon_env(repo, &evidence_add_args("none", &[]), &[], None);
    assert_eq!(body(&without)["error"]["code"], "COMMIT_REQUIRED", "{}", body(&without));

    let before = snapshot(repo);
    let env = [("GIT_DIR", other.path().join(".git").display().to_string()), ("GIT_WORK_TREE", other.path().display().to_string())];
    let held = canon_env(repo, &evidence_add_args("held", &[]), &env, None);
    let b = body(&held);
    assert_eq!(held.status.code(), Some(2), "{b}");
    assert_eq!(b["error"]["code"], "COMMIT_REQUIRED", "{b}");
    assert_eq!(snapshot(repo), before, "nothing was written");
}

// ── F2: file names ──

/// F2: a surface named `é.rs` or `a"b.rs` is stale when the file is changed and committed: the changed-file list is read NUL-terminated, so
/// the name git gives is the name the surface declares (git writes `"\303\251.rs"` otherwise). Spaces and directories: `canon-gate`'s unit tests.
#[test]
fn f2_a_declared_surface_with_a_non_ascii_name_or_a_quote_goes_stale_when_it_changes() {
    for name in ["é.rs", "a\"b.rs"] {
        let dir = project();
        let repo = dir.path();
        write(repo, name, "fn a() {}\n");
        commit_all(repo, "the surface");
        let added = canon_env(repo, &evidence_add_args("surface", &["--surface-ref", name]), &[], None);
        assert_eq!(added.status.code(), Some(0), "{name}: {}{}", String::from_utf8_lossy(&added.stdout), String::from_utf8_lossy(&added.stderr));
        commit_all(repo, "evidence");
        assert!(canon_env(repo, &["gate", "check"], &[], None).status.success(), "{name}: the evidence is current before the change");

        write(repo, name, "fn a() { /* changed */ }\n");
        commit_all(repo, "change the surface");
        let gate = canon_env(repo, &["gate", "check"], &[], None);
        let text = format!("{}{}", String::from_utf8_lossy(&gate.stdout), String::from_utf8_lossy(&gate.stderr));
        assert_eq!(gate.status.code(), Some(1), "{name}: {text}");
        assert!(text.contains("stale-evidence") && text.contains(SCENARIO), "{name}: {text}");
    }
}

/// F2: the approval payload's surface is the name as it is, `é.rs`.
#[test]
fn f2_the_approval_payload_lists_a_non_ascii_surface_as_it_is() {
    let dir = project();
    let repo = dir.path();
    write(repo, "é.rs", "fn a() {}\n");
    write(repo, "a\"b.rs", "fn b() {}\n");
    let sha = commit_all(repo, "files with names git quotes");
    let out = canon_env(repo, &["evidence", "approval-payload", "--scenario", SCENARIO, "--project", "root", "--rev", &sha, "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z"], &[], None);
    assert_eq!(out.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let surface: Vec<String> = body(&out)["data"]["surface"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert!(surface.contains(&"é.rs".to_string()) && surface.contains(&"a\"b.rs".to_string()), "{surface:?}");
}

/// A commit whose index has `src/\xff.rs`, a name no UTF-8 string spells; the entry never touches the file system.
fn commit_a_name_that_is_not_utf8(repo: &Path) -> String {
    let blob = {
        let mut child = Command::new("git").args(["hash-object", "-w", "--stdin"]).current_dir(repo).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
        std::io::Write::write_all(child.stdin.as_mut().unwrap(), b"x").unwrap();
        String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap().trim().to_string()
    };
    let mut entry = format!("100644 {blob}\tsrc/").into_bytes();
    entry.extend_from_slice(b"\xff.rs\n");
    let mut child = Command::new("git").args(["update-index", "--add", "--index-info"]).current_dir(repo).stdin(std::process::Stdio::piped()).spawn().unwrap();
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), &entry).unwrap();
    assert!(child.wait().unwrap().success());
    git(repo, &["commit", "-q", "-m", "a name that is not UTF-8"]);
    git(repo, &["rev-parse", "HEAD"])
}

/// F2, failure path: a commit that changed a name no UTF-8 string spells has no path list. The approval payload cannot be made for it, and
/// the staleness check does not call the evidence stale on the strength of a list it could not read.
#[test]
fn f2_a_name_that_is_not_utf8_gives_no_surface_and_never_a_stale_verdict() {
    let dir = project();
    let repo = dir.path();
    let added = canon_env(repo, &evidence_add_args("surface", &["--surface-ref", "src/main.rs"]), &[], None);
    assert_eq!(added.status.code(), Some(0), "{}", String::from_utf8_lossy(&added.stderr));
    commit_all(repo, "evidence");
    let sha = commit_a_name_that_is_not_utf8(repo);

    let payload = canon_env(repo, &["evidence", "approval-payload", "--scenario", SCENARIO, "--project", "root", "--rev", &sha, "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z"], &[], None);
    assert_ne!(payload.status.code(), Some(0), "no surface list, no payload: {}", String::from_utf8_lossy(&payload.stdout));
    assert!(String::from_utf8_lossy(&payload.stdout).contains("not UTF-8"), "{}", String::from_utf8_lossy(&payload.stdout));

    let gate = canon_env(repo, &["gate", "check"], &[], None);
    let text = format!("{}{}", String::from_utf8_lossy(&gate.stdout), String::from_utf8_lossy(&gate.stderr));
    assert!(!text.contains("stale-evidence"), "the diff query has no answer, so no staleness is asserted: {text}");
}

// ── F3: the deadline ──

/// F3: `..HEAD` sleeps in the `git` ahead on the `PATH`; `gate check` ends within seconds of the one-second deadline, asks the stuck query
/// once, and does not wait for the grandchild that holds git's pipes.
#[test]
fn f3_gate_check_ends_within_seconds_when_the_staleness_queries_stop_answering() {
    let dir = project();
    let repo = dir.path();
    let added = canon_env(repo, &evidence_add_args("surface", &["--surface-ref", "src/main.rs"]), &[], None);
    assert_eq!(added.status.code(), Some(0), "{}", String::from_utf8_lossy(&added.stderr));
    commit_all(repo, "evidence");
    write(repo, "src/main.rs", "fn main() { /* changed */ }\n");
    write(repo, "src/other.rs", "fn other() {}\n");
    commit_all(repo, "code moved");

    let bin = TempDir::new().unwrap();
    let log = bin.path().join("calls.log");
    let path = stuck_git(bin.path(), "*\"..HEAD\"*", &log);
    let started = Instant::now();
    let gate = canon_env(repo, &["gate", "check"], &short_deadline(), Some(&path));
    let took = started.elapsed();
    assert!(took < Duration::from_secs(20), "gate check waited {took:?} on a git that sleeps 30 seconds");
    assert!(gate.status.code().is_some(), "gate check ended with a status, not a signal");
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert_eq!(calls.lines().count(), 1, "one query past its deadline closes the staleness queries of the run: {calls}");
    let text = String::from_utf8_lossy(&gate.stdout);
    assert!(!text.contains("stale-evidence"), "a query with no answer is not a stale verdict: {text}");
}

/// F3, failure path: `add` sleeps, so the code tree cannot be made. `evidence add` is refused as `COMMIT_REQUIRED`, the message names `git add`
/// and the deadline, and nothing is written.
#[test]
fn f3_evidence_add_is_refused_with_commit_required_when_the_code_tree_cannot_be_made_in_time() {
    let dir = project();
    let repo = dir.path();
    let before = snapshot(repo);
    let bin = TempDir::new().unwrap();
    let log = bin.path().join("calls.log");
    let path = stuck_git(bin.path(), "*\" add \"*", &log);
    let started = Instant::now();
    let out = canon_env(repo, &evidence_add_args("stuck", &[]), &short_deadline(), Some(&path));
    assert!(started.elapsed() < Duration::from_secs(20), "waited {:?}", started.elapsed());
    let b = body(&out);
    assert_eq!(out.status.code(), Some(2), "{b}");
    assert_eq!(b["error"]["code"], "COMMIT_REQUIRED", "{b}");
    let message = b["error"]["message"].as_str().unwrap();
    assert!(message.contains("`git add`") && message.contains("1s"), "the message names the git command and the deadline: {message}");
    assert_eq!(snapshot(repo), before, "nothing was written");
}

/// F3: a context pack that cannot ask git in time is `CONTEXT_GIT_REQUIRED`, naming the git command and the deadline.
#[test]
fn f3_context_pack_add_is_refused_with_context_git_required_when_git_stops_answering() {
    let dir = project();
    let repo = dir.path();
    write(repo, "doc.md", "hi\n");
    write(repo, "m.json", r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#);
    commit_all(repo, "pack inputs");
    let before = snapshot(repo);
    let bin = TempDir::new().unwrap();
    let log = bin.path().join("calls.log");
    let path = stuck_git(bin.path(), "*\" diff \"*", &log);
    let out = canon_env(repo, &["context-pack", "add", "--manifest", "m.json"], &short_deadline(), Some(&path));
    let b = body(&out);
    assert_eq!(out.status.code(), Some(2), "{b}");
    assert_eq!(b["error"]["code"], "CONTEXT_GIT_REQUIRED", "{b}");
    let message = b["error"]["message"].as_str().unwrap();
    assert!(message.contains("`git diff`") && message.contains("1s"), "{message}");
    assert_eq!(snapshot(repo), before, "nothing was written");
}

// ── F4: the scan ──

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn scan(root: &Path) -> Output {
    Command::new("python3").arg(repo_root().join("scripts/check_git_boundary.py")).arg(root).output().expect("python3 runs the scan")
}

/// F4: the repository passes its own scan.
#[test]
fn f4_no_crate_source_starts_git_outside_the_one_path() {
    let out = scan(&repo_root());
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
}

/// F4: a `Command::new("git")` outside the path is reported with its file and line and the scan exits 1: on one line, with the argument on
/// the next line, in a `#[cfg(test)]` module, and in a `crates/*/src` of any crate. The path itself, a `crates/*/tests` file and a comment are not.
#[test]
fn f4_a_launch_outside_the_path_is_reported_with_file_and_line_and_exits_1() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(root, "crates/a/src/one.rs", "fn one() {\n    std::process::Command::new(\"git\").arg(\"status\");\n}\n");
    write(root, "crates/a/src/next.rs", "fn next() {\n    let _ = Command::new(\n        \"git\",\n    );\n}\n");
    write(root, "crates/b/src/fixture.rs", "pub fn x() {}\n\n#[cfg(test)]\nmod tests {\n    fn init() {\n        std::process::Command::new(\"git\").status().unwrap();\n    }\n}\n");
    write(root, "crates/b/src/raw.rs", "fn raw() { Command::new(r#\"git\"#); Command::new(\"/usr/bin/git\"); }\n");
    write(root, "crates/canon-store/src/git.rs", "fn the_path() { Command::new(\"git\"); }\n");
    write(root, "crates/a/tests/integration.rs", "fn fixture() { Command::new(\"git\"); }\n");
    write(root, "crates/a/src/comment.rs", "// Command::new(\"git\") is how the old code started it\n/// Command::new(\"git\")\nfn quiet() { Command::new(\"ssh-keygen\"); Command::new(\"gitk\"); }\n");

    let out = scan(root);
    let report = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{report}");
    for hit in ["crates/a/src/one.rs:2:", "crates/a/src/next.rs:2:", "crates/b/src/fixture.rs:6:", "crates/b/src/raw.rs:1:"] {
        assert!(report.contains(hit), "{hit} is missing from:\n{report}");
    }
    assert_eq!(report.lines().filter(|line| line.contains("starts git directly")).count(), 5, "the two launches of raw.rs are two lines of one file and one hit each:\n{report}");
    for quiet in ["crates/canon-store/src/git.rs", "crates/a/tests/integration.rs", "crates/a/src/comment.rs"] {
        assert!(!report.contains(quiet), "{quiet} is not a hit:\n{report}");
    }
}

/// F4: a tree with no launch outside the path is clean: exit 0 and nothing printed.
#[test]
fn f4_a_tree_without_a_launch_outside_the_path_is_clean() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "crates/a/src/lib.rs", "fn ok() { Command::new(\"ssh-keygen\"); GitCall::new(repo); }\n");
    let out = scan(dir.path());
    assert_eq!((out.status.code(), out.stdout.len(), out.stderr.len()), (Some(0), 0, 0), "{}", String::from_utf8_lossy(&out.stderr));
}

/// F4: the scan runs where the lint runs: `just lint` and the CI clippy job, and the rule is written down.
#[test]
fn f4_the_scan_is_wired_into_just_lint_the_clippy_job_and_the_coding_rules() {
    let root = repo_root();
    let justfile = std::fs::read_to_string(root.join("justfile")).unwrap();
    let lint = justfile.split("\nlint:").nth(1).and_then(|rest| rest.split("\n\n").next()).expect("the justfile has a lint recipe");
    assert!(lint.contains("scripts/check_git_boundary.py"), "{lint}");
    let workflow = std::fs::read_to_string(root.join(".github/workflows/test.yml")).unwrap();
    let clippy = workflow.split("\n  clippy:").nth(1).and_then(|rest| rest.split("\n  atip:").next()).or_else(|| workflow.split("\n  clippy:").nth(1)).expect("the workflow has a clippy job");
    assert!(clippy.contains("scripts/check_git_boundary.py"), "the clippy job runs the scan");
    assert!(workflow.contains("\"scripts/check_git_boundary.py\""), "a change to the scan runs the workflow");
    let rules = std::fs::read_to_string(root.join("docs/design/rust-coding-rules.md")).unwrap();
    assert!(rules.contains("canon_store::git::GitCall"), "docs/design/rust-coding-rules.md states the rule");
}

// ── F5: the changelog ──

/// F5: the CHANGELOG's `Unreleased` section has the entry under Changed: it says the environment is ignored, a non-ASCII name is read as it
/// is, the deadline and the limits, and that an approval recorded for such a commit is made again.
#[test]
fn f5_the_changelog_says_what_changed_and_when_to_approve_again() {
    let changelog = std::fs::read_to_string(repo_root().join("CHANGELOG.md")).unwrap();
    let unreleased = changelog.split("\n## ").nth(1).expect("an Unreleased section");
    assert!(unreleased.starts_with("Unreleased"), "the first section is Unreleased: {}", &unreleased[..unreleased.len().min(40)]);
    let changed = unreleased.split("\n### Changed").nth(1).unwrap_or_else(|| panic!("Unreleased has a Changed section"));
    let lines: Vec<&str> = changed.lines().collect();
    let start = lines.iter().position(|line| line.contains("canon-0-39-git-boundary")).expect("an entry names `canon-0-39-git-boundary`");
    // The entry is its bullet and the indented bullets under it.
    let entry = lines[start..].iter().enumerate().take_while(|(i, line)| *i == 0 || line.starts_with("  ")).map(|(_, line)| *line).collect::<Vec<_>>().join("\n");
    for needle in ["GIT_DIR", "GIT_INDEX_FILE", "non-ASCII", "300 seconds", "8 MiB", "64 MiB", "approve again"] {
        assert!(entry.contains(needle), "the entry does not say `{needle}`: {entry}");
    }
}
