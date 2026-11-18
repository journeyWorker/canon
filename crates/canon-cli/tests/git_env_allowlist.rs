//! `canon-0-40-git-env-allowlist` through the real binary: canon's git calls get only an allow list of `GIT_*` variables, and the diff and status canon reads
//! have the shape git gives with no configuration, whatever the shell or a gitconfig holds (F1-F3 of the brief). The name test itself is in
//! `canon-store`'s `git.rs` unit tests (`gate.git-env.02`, `03`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const SCENARIO: &str = "game.run.01";

fn canon_env(dir: &Path, args: &[&str], env: &[(&str, String)]) -> Output {
    let mut command = hermetic::canon();
    command.args(args).arg("--json").current_dir(dir).env("CANON_ACTOR", "canon");
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("spawn canon")
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn ok(dir: &Path, args: &[&str]) -> Value {
    ok_env(dir, args, &[])
}

fn ok_env(dir: &Path, args: &[&str], env: &[(&str, String)]) -> Value {
    let out = canon_env(dir, args, env);
    assert_eq!(out.status.code(), Some(0), "{args:?} {env:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    body(&out)
}

/// `git <args>` with no configuration of the host: the answer a user with no gitconfig gets.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", "-c", "protocol.file.allow=always"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        // Fixed dates, so that the commits (a submodule's among them) and the digest of the fixture are the same in every run.
        .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
        .output()
        .expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim_end_matches('\n').to_string()
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

/// A committed canon project with one scenario and one source file.
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

const MANIFEST: &str = r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#;

/// A dirty tree that every setting of the matrix can show in: two hunks 40 lines apart with a blank context line, three staged renames that are also edited
/// (more than a rename limit of 1 reaches), a file whose indent heuristic moves the hunk, a submodule that moved, an untracked file, and a file whose blob is long
/// enough to show an abbreviated hash.
fn dirty_project() -> TempDir {
    let dir = project();
    let repo = dir.path();
    ok(repo, &["init", "--no-harness"]);
    write(repo, "doc.md", "hi\n");
    write(repo, "m.json", MANIFEST);
    let body: String = (1..=60).map(|n| if n % 7 == 0 { "\n".to_string() } else { format!("line {n}\n") }).collect();
    write(repo, "src/long.rs", &body);
    write(repo, "src/old_name.rs", "pub fn renamed() {\n    let a = 1;\n    let b = 2;\n    a + b\n}\n");
    write(repo, "src/slider.txt", SLIDER_OLD);
    for n in 1..=2 {
        write(repo, &format!("src/old_{n}.rs"), &format!("pub fn other_{n}() {{\n    let a = 1;\n    let b = 2;\n    a + b\n}}\n"));
    }
    let upstream = TempDir::new().unwrap();
    git(upstream.path(), &["init", "-q", "-b", "main"]);
    write(upstream.path(), "lib.txt", "one\n");
    commit_all(upstream.path(), "one");
    git(repo, &["submodule", "add", "-q", upstream.path().to_str().unwrap(), "vendor"]);
    commit_all(repo, "pack inputs");
    let edited = body.replace("line 3\n", "line three\n").replace("line 50\n", "line fifty\n");
    write(repo, "src/long.rs", &edited);
    git(repo, &["mv", "src/old_name.rs", "src/new_name.rs"]);
    write(repo, "src/new_name.rs", "pub fn renamed() {\n    let a = 1;\n    let b = 3;\n    a + b\n}\n");
    for n in 1..=2 {
        git(repo, &["mv", &format!("src/old_{n}.rs"), &format!("src/new_{n}.rs")]);
        write(repo, &format!("src/new_{n}.rs"), &format!("pub fn other_{n}() {{\n    let a = 1;\n    let b = 3;\n    a + b\n}}\n"));
    }
    write(repo, "src/slider.txt", SLIDER_NEW);
    write(&repo.join("vendor"), "lib.txt", "two\n");
    commit_all(&repo.join("vendor"), "two");
    write(repo, "untracked.txt", "x\n");
    dir
}

/// A file whose added lines can slide, so that `diff.indentHeuristic=false` places the hunk differently (found by search: plain git prints the two differently).
const SLIDER_OLD: &str = "\n}\n    b();\n}\n    }\n    }\n    }\n    if (x) {\n";
const SLIDER_NEW: &str = "\n}\n    }\n}\n    b();\n}\n    }\n    }\n    }\n    if (x) {\n";

/// `context-pack add` of the dirty project: the pack id and the state digest it holds.
fn pack(repo: &Path, env: &[(&str, String)]) -> (String, String) {
    let added = ok_env(repo, &["context-pack", "add", "--manifest", "m.json"], env);
    let id = added["data"]["id"].as_str().unwrap_or_else(|| panic!("no id: {added}")).to_string();
    let shown = ok_env(repo, &["context-pack", "show", &id], env);
    let digest = shown["data"]["repo"]["stateDigest"].as_str().unwrap_or_else(|| panic!("no state digest: {shown}")).to_string();
    (id, digest)
}

/// What 0.39 hashed for this tree: the plain `git diff` and `git status` of the two calls, joined as `context_pack.rs` joins them (a NUL after the diff).
fn state_digest_of_0_39(repo: &Path) -> String {
    let diff = git(repo, &["diff", "--no-ext-diff", "--binary", "HEAD"]);
    let status = git(repo, &["status", "--porcelain=v1", "-z"]);
    // The registry's own paths are filtered out of the status by canon; this fixture has none that remain but `.canon/context-packs/`.
    let entries: Vec<&str> = status.split('\0').filter(|entry| !entry.is_empty() && !entry.contains(".canon/context-packs/")).collect();
    let mut state = format!("{diff}\n").into_bytes();
    state.push(0);
    for entry in entries {
        state.extend_from_slice(entry.as_bytes());
        state.push(0);
    }
    format!("sha256:{:x}", Sha256::digest(&state))
}

/// The settings of the brief's matrix: each changes the shape of what git prints, or the files it lists.
/// The value `ORDER_FILE` stands for in a row: the path of a file that puts the submodule and the renames first.
const ORDER_FILE: &str = "<order file>";

const MATRIX: [(&str, &str); 16] = [
    ("diff.context", "0"),
    ("diff.noprefix", "true"),
    ("diff.mnemonicPrefix", "true"),
    ("diff.srcPrefix", "x/"),
    ("diff.renames", "false"),
    ("diff.relative", "true"),
    ("diff.suppressBlankEmpty", "true"),
    ("color.ui", "always"),
    ("core.abbrev", "12"),
    ("status.showUntrackedFiles", "no"),
    ("status.renames", "false"),
    ("diff.ignoreSubmodules", "all"),
    ("diff.orderFile", ORDER_FILE),
    ("diff.submodule", "log"),
    ("diff.renameLimit", "1"),
    ("diff.indentHeuristic", "false"),
];

/// The row's value: `ORDER_FILE` becomes the path of a file in `dir` that moves the submodule and the renames before the rest.
fn matrix_value(dir: &Path, value: &str) -> String {
    if value != ORDER_FILE {
        return value.to_string();
    }
    let file = dir.join("order.txt");
    std::fs::write(&file, "vendor\nsrc/new_*\nsrc/slider.txt\n").unwrap();
    file.display().to_string()
}

/// What plain git (no canon options) prints for the diff and status of the pack, under the gitconfig file `global`: the precondition that a row of the matrix is a
/// setting that changes what git prints on this tree, so that a pinned option that did nothing would show.
fn plain_git_prints(repo: &Path, global: &Path) -> Vec<u8> {
    let mut printed = Vec::new();
    for args in [&["diff", "--no-ext-diff", "--binary", "HEAD"][..], &["status", "--porcelain=v1", "-z"][..]] {
        let out = Command::new("git").args(args).current_dir(repo).env("GIT_CONFIG_GLOBAL", global).env("GIT_CONFIG_NOSYSTEM", "1").output().expect("git must be on PATH");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        printed.extend(out.stdout);
    }
    printed
}

fn gitconfig_with(dir: &Path, name: &str, key: &str, value: &str) -> PathBuf {
    let file = dir.join(name);
    git(dir, &["config", "--file", file.to_str().unwrap(), key, value]);
    file
}

// ── F1: the environment ──

/// gate.git-env.01 (F1): a dirty tree's context pack has the id and state digest of a run without the variable, whichever of the variables that used to
/// reach git is in the shell.
#[test]
fn gate_git_env_01_a_git_variable_off_the_list_does_not_change_the_id_or_digest_of_a_dirty_pack() {
    let dir = dirty_project();
    let repo = dir.path();
    let plain = pack(repo, &[]);
    assert_eq!(plain.1, state_digest_of_0_39(repo), "the baseline is the digest of 0.39");
    let shells: [Vec<(&str, String)>; 4] = [
        vec![("GIT_DIFF_OPTS", "-u0".to_string())],
        vec![("GIT_CONFIG_COUNT", "1".to_string()), ("GIT_CONFIG_KEY_0", "diff.context".to_string()), ("GIT_CONFIG_VALUE_0", "0".to_string())],
        vec![("GIT_CONFIG_PARAMETERS", "'diff.context=0'".to_string())],
        vec![("GIT_TRACE", "1".to_string())],
    ];
    for env in shells {
        assert_eq!(pack(repo, &env), plain, "{env:?}");
    }
}

// ── F2: the shape of what canon reads ──

/// The state digest of `dirty_project()` as 0.39 made it with no gitconfig: the plain `git diff --no-ext-diff --binary HEAD` and `git status --porcelain=v1 -z`.
/// Fixed here, so that a change to the pinned options that moves a user's digest fails.
const DIRTY_PROJECT_DIGEST_0_39: &str = "sha256:6161226bf31349efe902741e022fd7770c7e09840bf9cc4db5e93bbef5f47631";

/// gate.git-env.04 (F2): each setting of the matrix in the home gitconfig (`GIT_CONFIG_GLOBAL`) leaves the state digest and id of a dirty pack as they are with
/// no setting, and that digest is the one 0.39 made.
#[test]
fn gate_git_env_04_a_home_gitconfig_does_not_change_the_state_digest_of_a_dirty_pack() {
    let dir = dirty_project();
    let repo = dir.path();
    let plain = pack(repo, &[]);
    assert_eq!(plain.1, DIRTY_PROJECT_DIGEST_0_39, "the digest of 0.39");
    assert_eq!(plain.1, state_digest_of_0_39(repo));
    let homes = TempDir::new().unwrap();
    for (key, value) in MATRIX {
        let value = matrix_value(homes.path(), value);
        let file = gitconfig_with(homes.path(), &format!("{key}.gitconfig"), key, &value);
        let env = [("GIT_CONFIG_GLOBAL", file.display().to_string())];
        // `diff.relative` only acts from a subdirectory; canon runs git at the root, so the row is a guard, not a visible change.
        if key != "diff.relative" {
            assert_ne!(plain_git_prints(repo, &file), plain_git_prints(repo, Path::new("/dev/null")), "{key}={value}: the precondition, plain git prints this tree differently under the setting");
        }
        assert_eq!(pack(repo, &env), plain, "{key}={value} in the home gitconfig");
    }
}

/// gate.git-env.05 (F2): the same settings in the repository's own `.git/config`, one at a time.
#[test]
fn gate_git_env_05_the_repositorys_own_git_config_does_not_change_the_state_digest_of_a_dirty_pack() {
    let dir = dirty_project();
    let repo = dir.path();
    let plain = pack(repo, &[]);
    let order = TempDir::new().unwrap();
    for (key, value) in MATRIX {
        let value = matrix_value(order.path(), value);
        git(repo, &["config", key, &value]);
        assert_eq!(pack(repo, &[]), plain, "{key}={value} in .git/config");
        git(repo, &["config", "--unset", key]);
    }
}

/// gate.git-env.06 (F2): `diff.ignoreSubmodules=all` does not hide a submodule update from the staleness check: evidence that declares the submodule as a
/// surface is `stale-evidence` after a commit that moves the submodule, in the home gitconfig and in the repository's.
#[test]
fn gate_git_env_06_ignore_submodules_all_does_not_hide_a_submodule_update_from_the_staleness_check() {
    for place in ["home", "repo"] {
        let dir = project();
        let repo = dir.path();
        let upstream = TempDir::new().unwrap();
        git(upstream.path(), &["init", "-q", "-b", "main"]);
        write(upstream.path(), "lib.txt", "one\n");
        commit_all(upstream.path(), "one");
        git(repo, &["submodule", "add", "-q", upstream.path().to_str().unwrap(), "vendor"]);
        commit_all(repo, "add the submodule");
        let added = canon_env(repo, &["evidence", "add", "--scenario", SCENARIO, "--project", "root", "--class", "test-run", "--ref", "sub", "--role", "implementer", "--surface-ref", "vendor"], &[]);
        assert_eq!(added.status.code(), Some(0), "{place}: {}{}", String::from_utf8_lossy(&added.stdout), String::from_utf8_lossy(&added.stderr));
        commit_all(repo, "evidence");

        write(&repo.join("vendor"), "lib.txt", "two\n");
        commit_all(&repo.join("vendor"), "two");
        let moved = commit_all(repo, "move the submodule");
        let base = git(repo, &["rev-parse", "HEAD~1"]);
        let homes = TempDir::new().unwrap();
        let (env, _kept) = match place {
            "home" => (vec![("GIT_CONFIG_GLOBAL", gitconfig_with(homes.path(), "g", "diff.ignoreSubmodules", "all").display().to_string())], ()),
            _ => {
                git(repo, &["config", "diff.ignoreSubmodules", "all"]);
                (vec![], ())
            }
        };
        // The setting is what hides it: plain git under it lists nothing for the moved submodule.
        let hidden = Command::new("git").args(["diff", "--name-only", &format!("{base}..{moved}")]).current_dir(repo).envs(env.iter().map(|(k, v)| (*k, v.as_str()))).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&hidden.stdout).trim(), "", "{place}: the precondition, git hides the update under the setting");

        let gate = canon_env(repo, &["gate", "check"], &env);
        let text = format!("{}{}", String::from_utf8_lossy(&gate.stdout), String::from_utf8_lossy(&gate.stderr));
        assert_eq!(gate.status.code(), Some(1), "{place}: {text}");
        assert!(text.contains("stale-evidence") && text.contains(SCENARIO), "{place}: {text}");
    }
}

/// gate.git-env.07 (F2, failure path): `core.autocrlf` is not pinned. A file committed with LF and rewritten with CRLF has the code tree of HEAD (not dirty) in a
/// repository set to `autocrlf=true`, and a different one (dirty) with no setting. A pinned `core.autocrlf` would break the first.
#[test]
fn gate_git_env_07_core_autocrlf_is_not_pinned_and_a_crlf_file_is_no_change_where_it_is_set() {
    let dir = project();
    let repo = dir.path();
    write(repo, "notes.txt", "one\ntwo\n");
    commit_all(repo, "an LF file");
    let args = |reference: &'static str| ["evidence", "add", "--scenario", SCENARIO, "--project", "root", "--class", "test-run", "--ref", reference, "--role", "implementer"];
    let record = |reference: &str| -> Value {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(repo.join(".canon/ledger/kind=evidence")).unwrap().filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        paths.iter().map(|p| serde_json::from_str::<Value>(&std::fs::read_to_string(p).unwrap()).unwrap()).find(|r| r["evidence"]["ref"] == reference).unwrap_or_else(|| panic!("no record {reference}"))
    };
    ok(repo, &args("clean"));
    let clean = record("clean");
    assert_eq!(clean["worktree_dirty"], false);

    write(repo, "notes.txt", "one\r\ntwo\r\n");
    let homes = TempDir::new().unwrap();
    let on = gitconfig_with(homes.path(), "autocrlf", "core.autocrlf", "true");
    ok_env(repo, &args("crlf-on"), &[("GIT_CONFIG_GLOBAL", on.display().to_string())]);
    let with = record("crlf-on");
    assert_eq!(with["worktree_dirty"], false, "{with}");
    assert_eq!(with["code_tree"], clean["code_tree"], "the CRLF file normalizes to the committed tree");

    ok(repo, &args("crlf-off"));
    let without = record("crlf-off");
    assert_eq!(without["worktree_dirty"], true, "{without}");
    assert_ne!(without["code_tree"], clean["code_tree"]);
}
