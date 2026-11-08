//! `canon-0-39-staleness-scope` through the real binary: a directory or a glob declared as the surface of evidence goes stale like a
//! file, a surface that matches nothing is said when it is saved, and the cells whose base commit git cannot read are counted by
//! `gate check` and `status` alike (F1-F5 of the brief).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

// ── The binary and git ──

/// `canon <args> --json` in `dir`; `PATH` is replaced by `path` when given.
fn canon_at(dir: &Path, args: &[&str], path: Option<&std::ffi::OsStr>) -> Output {
    canon_with(dir, args, path, &[])
}

fn canon_with(dir: &Path, args: &[&str], path: Option<&std::ffi::OsStr>, env: &[(&str, &str)]) -> Output {
    let mut command = hermetic::canon();
    command.args(args).arg("--json").current_dir(dir).env("CANON_ACTOR", "canon").envs(env.iter().copied());
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command.output().expect("spawn canon")
}

fn canon(dir: &Path, args: &[&str]) -> Output {
    canon_at(dir, args, None)
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", text(out)))
}

fn ok(dir: &Path, args: &[&str]) -> Value {
    let out = canon(dir, args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", text(&out));
    body(&out)
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(dir).output().expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

fn write(dir: &Path, relative: &str, content: &str) {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn scenario(n: usize) -> String {
    format!("game.run.{n:02}")
}

/// A committed canon project with `scenarios` scenarios (`game.run.01` ...) and the files of `files`, each holding `1`.
fn project(scenarios: usize, files: &[&str]) -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    for n in 1..=scenarios {
        ok(repo, &["scenario", "new", &scenario(n), "--title", &format!("A run ends {n}"), "--case", "failure"]);
    }
    ok(repo, &["inventory", "sync"]);
    for file in files {
        write(repo, file, "1");
    }
    commit_all(repo, "project");
    dir
}

/// `canon evidence add` for scenario `n` with `surface` declared (none for an empty string).
fn add_evidence(repo: &Path, n: usize, surface: &str) -> Output {
    let id = scenario(n);
    let reference = format!("run-{n}");
    let mut args = vec!["evidence", "add", "--scenario", &id, "--project", "root", "--class", "test-run", "--ref", &reference, "--role", "implementer"];
    if !surface.is_empty() {
        args.extend(["--surface-ref", surface]);
    }
    canon(repo, &args)
}

fn add_evidence_ok(repo: &Path, n: usize, surface: &str) {
    let out = add_evidence(repo, n, surface);
    assert_eq!(out.status.code(), Some(0), "evidence add for {surface}: {}", text(&out));
}

/// The `stale-evidence` subjects and details of `gate check`, and its exit status.
fn stale(repo: &Path) -> (Option<i32>, Vec<(String, String)>) {
    let out = canon(repo, &["gate", "check"]);
    let document = body(&out);
    let violations = document["data"]["violations"].as_array().cloned().unwrap_or_default();
    let mut stale: Vec<(String, String)> = violations
        .iter()
        .filter(|v| v["class"] == "stale-evidence")
        .map(|v| (v["subject"].as_str().unwrap().to_string(), v["message"].as_str().unwrap().to_string()))
        .collect();
    stale.sort();
    (out.status.code(), stale)
}

fn subjects(stale: &[(String, String)]) -> Vec<&str> {
    stale.iter().map(|(subject, _)| subject.as_str()).collect()
}

// ── F1: the match ──

/// gate.staleness-scope.01 (F1): evidence that declares `src/`, `src`, `src/**` and `src/*.rs` is stale once `src/a.rs` is edited and committed, and each reason names the
/// declaration that matched and the path it matched.
#[test]
fn gate_staleness_scope_01_a_directory_and_a_glob_declared_as_the_surface_go_stale_when_a_file_under_them_changes() {
    let dir = project(4, &["src/a.rs", "lib/z.rs"]);
    let repo = dir.path();
    let declared = ["src/", "src", "src/**", "src/*.rs"];
    for (i, surface) in declared.iter().enumerate() {
        add_evidence_ok(repo, i + 1, surface);
    }
    commit_all(repo, "evidence");
    let (status, stale_now) = stale(repo);
    assert_eq!((status, stale_now.len()), (Some(0), 0), "the evidence is current before the edit");

    write(repo, "src/a.rs", "2");
    commit_all(repo, "edit src/a.rs");
    let (status, stale_after) = stale(repo);
    assert_eq!(status, Some(1));
    assert_eq!(subjects(&stale_after), [scenario(1), scenario(2), scenario(3), scenario(4)], "{stale_after:?}");
    for (i, surface) in declared.iter().enumerate() {
        let detail = &stale_after[i].1;
        assert!(detail.contains(&format!("`{surface}`")) && detail.contains("`src/a.rs`"), "the reason names the declaration and the path: {detail}");
    }
}

/// gate.staleness-scope.02 (F1, failure path): a file below `src/x` does not match `src/*.rs`; the root `Cargo.toml` is not the manifest of a crate; the exact file and `effect:` answer
/// as before.
#[test]
fn gate_staleness_scope_02_a_glob_that_stops_at_a_slash_and_a_name_at_the_root_are_not_stale_for_a_file_further_down() {
    let dir = project(4, &["src/a.rs", "src/x/b.rs", "Cargo.toml", "crates/x/Cargo.toml"]);
    let repo = dir.path();
    for (i, surface) in ["src/*.rs", "Cargo.toml", "src/a.rs", "effect:payments"].iter().enumerate() {
        add_evidence_ok(repo, i + 1, surface);
    }
    commit_all(repo, "evidence");

    write(repo, "src/x/b.rs", "2");
    write(repo, "crates/x/Cargo.toml", "2");
    commit_all(repo, "edit two files that none of the four covers");
    let (status, stale_now) = stale(repo);
    assert_eq!((status, subjects(&stale_now)), (Some(0), Vec::<&str>::new()), "{stale_now:?}");

    write(repo, "src/a.rs", "2");
    commit_all(repo, "edit the exact file");
    let (status, stale_after) = stale(repo);
    assert_eq!(status, Some(1));
    assert_eq!(subjects(&stale_after), [scenario(1), scenario(3)], "`src/*.rs` and `src/a.rs` cover src/a.rs; the root manifest and the effect do not: {stale_after:?}");
}

/// gate.staleness-scope.03 (F1): `code.exclude`, the policy's `code_scopes` and a surface ref read a pattern with a `/` the same way. Each row is judged by the code tree (a file
/// that is excluded leaves the tree as it was), by the compiled scope, and by the surface matcher.
#[test]
fn gate_staleness_scope_03_the_three_readers_of_a_pattern_with_a_slash_give_one_answer() {
    use canon_model::path_glob::{Anchor, Glob};
    let table: [(&str, &str, bool); 10] = [
        ("src/**", "src/a.rs", true),
        ("src/*.rs", "src/a.rs", true),
        ("src/*.rs", "src/x/b.rs", false),
        ("src/", "src/a.rs", true),
        ("/src/**", "src/x/b.rs", true),
        ("lib/*/m.rs", "lib/a/m.rs", true),
        ("lib/*/m.rs", "lib/a/b/m.rs", false),
        ("lib/**/m.rs", "lib/a/b/m.rs", true),
        ("a?c/x", "abc/x", true),
        ("a?c/x", "a/c/x", false),
    ];
    for (pattern, path, expected) in table {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        git(repo, &["init", "-q", "-b", "main"]);
        write(repo, "canon.yaml", &format!("code:\n  exclude:\n    - '{pattern}'\n"));
        write(repo, "keep.txt", "1");
        commit_all(repo, "keep");
        let without = canon_cli::git_rev::code_tree(repo, None).unwrap().tree;
        write(repo, path, "1");
        let with = canon_cli::git_rev::code_tree(repo, None).unwrap().tree;
        let by_exclude = with == without;
        let by_scope = canon_cli::code_scopes::compile(&[pattern.to_string()]).iter().any(|glob| glob.matches(path));
        let by_surface = Glob::new(pattern, Anchor::Root).matches(path);
        assert_eq!((by_exclude, by_scope, by_surface), (expected, expected, expected), "`{pattern}` against `{path}`: code.exclude, code_scopes, surface ref");
    }
}

// ── F2: the warning at write time ──

/// gate.staleness-scope.04 (F2): a value that matches no file is saved with `SURFACE_REF_UNMATCHED` naming it, and the exit status is 0; a value that matches, and an effect, say nothing.
#[test]
fn gate_staleness_scope_04_a_surface_that_matches_no_file_is_saved_with_a_warning() {
    let dir = project(3, &["src/a.rs"]);
    let repo = dir.path();
    let out = add_evidence(repo, 1, "src/nope/**");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let warnings = body(&out)["meta"]["warnings"].as_array().cloned().unwrap_or_default();
    let unmatched: Vec<&Value> = warnings.iter().filter(|w| w["code"] == "SURFACE_REF_UNMATCHED").collect();
    assert_eq!(unmatched.len(), 1, "{warnings:?}");
    assert!(unmatched[0]["message"].as_str().unwrap().contains("`src/nope/**`"), "{unmatched:?}");
    let saved = std::fs::read_dir(repo.join(".canon/ledger/kind=evidence")).unwrap().filter_map(Result::ok).any(|entry| std::fs::read_to_string(entry.path()).unwrap().contains("src/nope/**"));
    assert!(saved, "the value is recorded as given");

    for (n, surface) in [(2, "src/"), (3, "effect:payments")] {
        let out = add_evidence(repo, n, surface);
        assert_eq!(out.status.code(), Some(0), "{}", text(&out));
        let said: Vec<Value> = body(&out)["meta"]["warnings"].as_array().cloned().unwrap_or_default();
        assert!(said.iter().all(|w| w["code"] != "SURFACE_REF_UNMATCHED"), "`{surface}` matches: {said:?}");
    }
}

/// gate.staleness-scope.05 (F2, failure path): a value outside the repository is refused as before, with `USAGE`, and nothing is saved.
#[test]
fn gate_staleness_scope_05_a_surface_outside_the_repository_is_still_refused_as_usage() {
    let dir = project(1, &["src/a.rs"]);
    let repo = dir.path();
    let out = add_evidence(repo, 1, "../x");
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("USAGE"), "{}", text(&out));
    assert!(!repo.join(".canon/ledger/kind=evidence").exists() || std::fs::read_dir(repo.join(".canon/ledger/kind=evidence")).unwrap().next().is_none());
}

// ── F3 and F4: the cells git could not read ──

/// The advisory of `gate check` and the warning of `status`, as the message each says (`None` when it says nothing).
fn said(repo: &Path, path: Option<&std::ffi::OsStr>) -> (Option<String>, Option<String>) {
    said_with(repo, path, &[])
}

fn said_with(repo: &Path, path: Option<&std::ffi::OsStr>, env: &[(&str, &str)]) -> (Option<String>, Option<String>) {
    let gate = body(&canon_with(repo, &["gate", "check"], path, env));
    let advisory = gate["data"]["advisories"].as_array().and_then(|list| list.iter().find(|a| a["kind"] == "staleness_unresolvable")).map(|a| a["message"].as_str().unwrap().to_string());
    let status = body(&canon_with(repo, &["status"], path, env));
    let warning = status["meta"]["warnings"].as_array().and_then(|list| list.iter().find(|w| w["code"] == "STALENESS_UNRESOLVABLE")).map(|w| w["message"].as_str().unwrap().to_string());
    (advisory, warning)
}

/// Both say one sentence, and it is the same one; the sentence is returned.
fn both_say(repo: &Path, path: Option<&std::ffi::OsStr>) -> String {
    let (advisory, warning) = said(repo, path);
    let advisory = advisory.unwrap_or_else(|| panic!("gate check has no staleness_unresolvable advisory"));
    assert_eq!(Some(&advisory), warning.as_ref(), "gate check and status say the same count and reasons");
    advisory
}

/// A project whose one evidence record (surface `src/`) has its base commit on a branch `main` does not contain: the record's ledger was moved
/// to `main` by `git checkout side -- .canon/ledger`.
fn ledger_moved_from_another_branch() -> TempDir {
    let dir = project(1, &["src/main.rs"]);
    let repo = dir.path();
    git(repo, &["checkout", "-q", "-b", "side"]);
    write(repo, "src/main.rs", "2");
    commit_all(repo, "side work");
    add_evidence_ok(repo, 1, "src/");
    commit_all(repo, "side evidence");
    git(repo, &["checkout", "-q", "main"]);
    git(repo, &["checkout", "side", "--", ".canon/ledger"]);
    write(repo, "src/main.rs", "3");
    commit_all(repo, "main takes the ledger");
    dir
}

/// A project with its evidence recorded, then the code changed, over three commits: the base of the record is the first.
fn project_with_a_change_after_its_evidence() -> TempDir {
    let dir = project(1, &["src/main.rs"]);
    let repo = dir.path();
    add_evidence_ok(repo, 1, "src/");
    commit_all(repo, "evidence");
    write(repo, "src/main.rs", "2");
    commit_all(repo, "code moved");
    dir
}

fn clone_of(origin: &Path, shallow: bool) -> TempDir {
    let dir = TempDir::new().unwrap();
    let url = format!("file://{}", origin.display());
    let mut args = vec!["clone", "-q"];
    if shallow {
        args.extend(["--depth", "1"]);
    }
    args.extend([url.as_str(), "."]);
    git(dir.path(), &args);
    dir
}

/// gate.staleness-scope.06 (F3, F4): a shallow clone has no base commit; `gate check` says `base_missing 1`, with the instruction to unshallow, and `status`
/// says the same; the violation list and the exit status are those of a base that adds nothing.
#[test]
fn gate_staleness_scope_06_a_shallow_clone_is_counted_as_base_missing_with_the_unshallow_instruction() {
    let origin = project_with_a_change_after_its_evidence();
    let shallow = clone_of(origin.path(), true);
    let message = both_say(shallow.path(), None);
    assert!(message.starts_with("1 (subject, role) cell(s)") && message.contains("base_missing 1") && message.contains(&scenario(1)), "{message}");
    assert!(message.contains("git fetch --unshallow"), "{message}");
    let (status, stale_shallow) = stale(shallow.path());
    assert_eq!((status, stale_shallow.len()), (Some(0), 0), "the cell adds nothing, as it did before the count");
}

/// gate.staleness-scope.07 (F3, failure path): in a full clone of the same repository neither says anything, and the evidence is stale there as in the
/// repository itself.
#[test]
fn gate_staleness_scope_07_a_full_clone_has_every_base_and_says_nothing() {
    let origin = project_with_a_change_after_its_evidence();
    let (status, stale_origin) = stale(origin.path());
    assert_eq!((status, stale_origin.len()), (Some(1), 1), "in the repository itself the evidence is stale");
    let full = clone_of(origin.path(), false);
    assert_eq!(said(full.path(), None), (None, None), "a full clone has every base");
    assert_eq!(stale(full.path()), (Some(1), stale_origin), "and the same violations as the repository");
}

/// gate.staleness-scope.11 (F4): `status` and `gate check` count the same cells for the same reasons: one sentence, said twice, in a shallow clone and with no
/// git on the `PATH`.
#[test]
fn gate_staleness_scope_11_status_and_gate_check_say_the_same_count_and_reasons() {
    let origin = project_with_a_change_after_its_evidence();
    let shallow = clone_of(origin.path(), true);
    let (advisory, warning) = said(shallow.path(), None);
    assert!(advisory.is_some() && advisory == warning, "{advisory:?} vs {warning:?}");
    let empty = TempDir::new().unwrap();
    let (advisory, warning) = said(origin.path(), Some(empty.path().as_os_str()));
    assert!(advisory.as_deref().is_some_and(|m| m.contains("git_unavailable 1")) && advisory == warning, "{advisory:?} vs {warning:?}");
}

/// gate.staleness-scope.08 (F3, F4): a base commit that `git gc --prune=now` removed is `base_missing`, and the clone is not shallow, so there is no instruction to unshallow.
#[test]
fn gate_staleness_scope_08_a_pruned_base_commit_is_counted_as_base_missing() {
    let dir = ledger_moved_from_another_branch();
    let repo = dir.path();
    git(repo, &["branch", "-q", "-D", "side"]);
    git(repo, &["reflog", "expire", "--expire=now", "--all"]);
    git(repo, &["gc", "-q", "--prune=now"]);

    let message = both_say(repo, None);
    assert!(message.contains("base_missing 1") && !message.contains("--unshallow"), "{message}");
    let (status, stale_now) = stale(repo);
    assert_eq!((status, stale_now.len()), (Some(0), 0), "a base that is gone adds nothing");
}

/// gate.staleness-scope.09 (F3, F4): a ledger moved from another branch has a base that is there but is not an ancestor: `base_not_ancestor 1`, and the verdict is judged from
/// `<base>..HEAD` as it was, which is stale here (src/main.rs differs between the two).
#[test]
fn gate_staleness_scope_09_a_ledger_moved_from_another_branch_is_counted_as_base_not_ancestor_and_judged_from_the_range() {
    let dir = ledger_moved_from_another_branch();
    let repo = dir.path();
    let message = both_say(repo, None);
    assert!(message.contains("base_not_ancestor 1") && !message.contains("base_missing"), "{message}");
    let (status, stale_now) = stale(repo);
    assert_eq!((status, subjects(&stale_now)), (Some(1), vec![scenario(1).as_str()]), "{stale_now:?}");
}

/// gate.staleness-scope.10 (F3, F4): with no `git` on the `PATH` git cannot answer: `git_unavailable`, and the violations are the ones that need no git.
#[test]
fn gate_staleness_scope_10_no_git_on_the_path_is_counted_as_git_unavailable() {
    let origin = project_with_a_change_after_its_evidence();
    let repo = origin.path();
    let empty = TempDir::new().unwrap();
    let path = empty.path().as_os_str();
    let message = both_say(repo, Some(path));
    assert!(message.contains("git_unavailable 1") && !message.contains("base_missing"), "{message}");
}

/// A `git` ahead on the `PATH` that sleeps on `diff ... ..HEAD` and runs the real one otherwise.
fn git_that_sleeps_on_the_range_diff(dir: &Path) -> std::ffi::OsString {
    let original = std::env::var_os("PATH").unwrap();
    let real = std::env::split_paths(&original).map(|d| d.join("git")).find(|p| p.is_file()).expect("git is on the PATH");
    let script = format!("#!/bin/sh\ncase \" $* \" in\n  *\" diff \"*\"..HEAD\"*) sleep 30 ;;\nesac\nexec '{}' \"$@\"\n", real.display());
    let wrapper = dir.join("git");
    std::fs::write(&wrapper, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::join_paths(std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&original))).unwrap()
}

/// gate.staleness-scope.10 (F3, F4, failure path): the base probes answer but the `<base>..HEAD` diff runs past its deadline: the cell is `git_unavailable`, said by
/// `gate check` and `status` alike, and no verdict changes (no `stale-evidence`).
#[test]
fn gate_staleness_scope_10_a_range_diff_that_runs_past_its_deadline_is_counted_as_git_unavailable() {
    let origin = project_with_a_change_after_its_evidence();
    let repo = origin.path();
    let bin = TempDir::new().unwrap();
    let path = git_that_sleeps_on_the_range_diff(bin.path());
    let env = [("CANON_TEST_GIT_DEADLINE_MS", "1000")];
    let (advisory, warning) = said_with(repo, Some(&path), &env);
    assert!(advisory.as_deref().is_some_and(|m| m.contains("git_unavailable 1")) && advisory == warning, "{advisory:?} vs {warning:?}");
    let gate = body(&canon_with(repo, &["gate", "check"], Some(&path), &env));
    let stale = gate["data"]["violations"].as_array().cloned().unwrap_or_default().iter().filter(|v| v["class"] == "stale-evidence").count();
    assert_eq!(stale, 0, "a query with no answer is not a stale verdict: {gate}");
}

/// gate.staleness-scope.10 (F3, F4, failure path): one commit changes the declared surface and adds a name no UTF-8 string spells; the diff cannot be read, so the
/// cell is `git_unavailable` in `gate check` and `status`, and not stale.
#[test]
fn gate_staleness_scope_10_a_name_that_is_not_utf8_in_the_range_is_counted_as_git_unavailable() {
    let dir = project(1, &["src/a.rs"]);
    let repo = dir.path();
    add_evidence_ok(repo, 1, "src/a.rs");
    commit_all(repo, "evidence");
    write(repo, "src/a.rs", "2");
    git(repo, &["add", "-A"]);
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
    git(repo, &["commit", "-q", "-m", "surface changed and a name that is not UTF-8"]);

    let message = both_say(repo, None);
    assert!(message.contains("git_unavailable 1"), "{message}");
    assert_eq!(stale(repo).1.len(), 0, "no list, no stale verdict");
}

/// gate.staleness-scope.12 (F4, failure path): with nothing unread neither says anything; the two report what the other does or both are silent.
#[test]
fn gate_staleness_scope_12_when_every_base_is_read_neither_gate_check_nor_status_says_anything() {
    let dir = project_with_a_change_after_its_evidence();
    assert_eq!(said(dir.path(), None), (None, None));
    let empty = project(1, &["src/main.rs"]);
    assert_eq!(said(empty.path(), None), (None, None), "no evidence, no cell");
}

// ── F5: the CHANGELOG ──

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// What the Unreleased section of `changelog` lacks about this change: the Changed entry (the new rule and what the gate asks for more, and that the
/// `paths` of a risk tier keep theirs) and the Added entries (the advisory and the warnings).
fn release_problems(changelog: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(unreleased) = changelog.split("\n## ").find(|section| section.starts_with("0.39.0")) else { return vec!["the CHANGELOG has no 0.39.0 section".to_string()] };
    let sections: Vec<&str> = unreleased.split("\n### ").collect();
    let section = |name: &str| sections.iter().find(|s| s.starts_with(name)).copied();
    let entry_of = |section: &str, needle: &str| -> Option<String> {
        let lines: Vec<&str> = section.lines().collect();
        let start = lines.iter().position(|line| line.contains(needle))?;
        Some(lines[start..].iter().enumerate().take_while(|(i, line)| *i == 0 || line.starts_with("  ")).map(|(_, line)| *line).collect::<Vec<_>>().join("\n"))
    };
    match section("Changed").and_then(|changed| entry_of(changed, "canon-0-39-staleness-scope")) {
        None => problems.push("0.39.0 has no Changed entry for canon-0-39-staleness-scope".to_string()),
        Some(entry) => {
            for needle in ["src/**", "`Cargo.toml`", "`**`", "`paths`", "next release that breaks compatibility", "stale-evidence", "code.exclude"] {
                if !entry.contains(needle) {
                    problems.push(format!("the Changed entry does not say `{needle}`"));
                }
            }
        }
    }
    match section("Added") {
        None => problems.push("0.39.0 has no Added section".to_string()),
        Some(added) => {
            for needle in ["staleness_unresolvable", "STALENESS_UNRESOLVABLE", "SURFACE_REF_UNMATCHED", "base_missing", "base_not_ancestor", "git_unavailable", "git fetch --unshallow"] {
                if !added.contains(needle) {
                    problems.push(format!("the Added section does not say `{needle}`"));
                }
            }
        }
    }
    problems
}

/// gate.staleness-scope.13 (F5): the CHANGELOG says the new rule, what the gate asks for more, the advisory and the warnings, and that the `paths` of a risk tier keep their reading.
#[test]
fn gate_staleness_scope_13_the_changelog_says_the_new_rule_the_advisory_and_the_warnings() {
    let changelog = std::fs::read_to_string(repo_root().join("CHANGELOG.md")).unwrap();
    assert_eq!(release_problems(&changelog), Vec::<String>::new());
}

/// gate.staleness-scope.14 (F5, failure path): a CHANGELOG without the entry, or without a piece of it, is named by what it lacks.
#[test]
fn gate_staleness_scope_14_a_changelog_without_the_entry_or_a_piece_of_it_is_named() {
    let without = "# Changelog\n\n## 0.39.0\n\n### Changed\n\n- an unrelated change\n\n## 0.38.0\n";
    let problems = release_problems(without);
    assert_eq!(problems[0], "0.39.0 has no Changed entry for canon-0-39-staleness-scope");
    assert_eq!(problems[1], "0.39.0 has no Added section");

    let thin = "# Changelog\n\n## 0.39.0\n\n### Changed\n\n- **x** (`canon-0-39-staleness-scope`): `src/**` is stale-evidence.\n\n### Added\n\n- `STALENESS_UNRESOLVABLE`.\n\n## 0.38.0\n";
    let problems = release_problems(thin);
    assert!(problems.contains(&"the Changed entry does not say `next release that breaks compatibility`".to_string()), "{problems:?}");
    assert!(problems.contains(&"the Added section does not say `SURFACE_REF_UNMATCHED`".to_string()), "{problems:?}");
    assert!(problems.contains(&"the Added section does not say `git fetch --unshallow`".to_string()), "{problems:?}");
}
