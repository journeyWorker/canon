//! Integration test for `canon/skills/canon-retrieve/pre-dispatch.sh`
//! (S8 part2, task 3.1) — the generic pre-dispatch hook script, run as
//! a real subprocess against the real `canon` binary, exactly as
//! Claude Code/Codex would invoke it (PreToolUse hook JSON on stdin,
//! `additionalContext` JSON on stdout, always exit `0`).
//!
//! Skips (never fails) when an OPTIONAL external tool the script
//! itself already treats as optional (`jq`, `git`) is missing from the
//! test-running environment — matching the script's own fail-soft
//! contract (`command -v jq >/dev/null 2>&1 || exit 0`); this file
//! proves the script's LOGIC where those tools exist, it does not make
//! `cargo test -p canon-cli` depend on them being present everywhere.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use canon_learn::{ParquetStrategyStore, StrategyId, StrategyItem, StrategyStore, TrajectoryId};
use canon_model::ids::{RegimeKey, RoleId, regime_key};
use chrono::Utc;

fn have(cmd: &str) -> bool {
    Command::new("sh").arg("-c").arg(format!("command -v {cmd}")).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../canon/skills/canon-retrieve/pre-dispatch.sh")
}

/// The exact `sha256(<area>)[..12]` derivation the script itself
/// performs when `CANON_RETRIEVE_HASH` is unset — computed here via
/// the same `sha256sum`/`shasum` tools the script shells out to, so
/// this test never hardcodes a digest that could silently drift from
/// the script's own logic.
fn area_hash(area: &str) -> String {
    let quoted = shell_quote(area);
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("printf '%s' {quoted} | sha256sum 2>/dev/null | cut -c1-12 || printf '%s' {quoted} | shasum -a 256 | cut -c1-12"))
        .output()
        .expect("compute area hash");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn seed_strategy(repo: &Path, regime_key: &RegimeKey, role: &RoleId, title: &str, content: &str) {
    let store = ParquetStrategyStore::open(repo.join("canon").join("learn").join("strategies"));
    let item = StrategyItem::new(StrategyId::new(), regime_key.clone(), role.clone(), title, "description", content, vec![TrajectoryId::new()], Utc::now());
    store.append(&item).expect("seed strategy");
}

fn run_hook(repo: &Path, stdin_json: &str) -> Output {
    let canon_bin_dir = Path::new(env!("CARGO_BIN_EXE_canon")).parent().unwrap().to_path_buf();
    let existing_path = std::env::var("PATH").unwrap_or_default();
    let path = format!("{}:{existing_path}", canon_bin_dir.display());

    let mut child = Command::new("sh")
        .arg(script_path())
        .current_dir(repo)
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pre-dispatch.sh");
    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(stdin_json.as_bytes()).expect("write stdin");
    }
    child.wait_with_output().expect("wait for pre-dispatch.sh")
}

fn init_git_repo(dir: &Path) {
    Command::new("git").arg("init").arg("-q").arg(dir).status().expect("git init");
}

#[test]
fn pre_dispatch_hook_surfaces_guidance_as_additional_context_for_a_task_dispatch() {
    if !have("jq") || !have("git") {
        eprintln!("skipping: jq/git not on PATH in this environment");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    // Seed under the SAME key the hook now assembles via `canon
    // regime-key` (the Rust write-path serializer, `s8` fix): the RAW
    // basename, never a shell slug.
    let repo_raw = dir.path().file_name().unwrap().to_string_lossy().into_owned();
    let hash = area_hash("general");
    let regime = RegimeKey::parse(regime_key("code-reviewer", &repo_raw, "general", &hash)).unwrap();
    seed_strategy(dir.path(), &regime, &RoleId::parse("code-reviewer").unwrap(), "review for null derefs", "always check Option before unwrap");

    let stdin_json = r#"{"tool_name":"Task","tool_input":{"subagent_type":"code-reviewer","description":"review the diff"}}"#;
    let output = run_hook(dir.path(), stdin_json);
    assert!(output.status.success(), "hook must always exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.trim().is_empty(), "expected additionalContext JSON on stdout, got nothing");
    let envelope: serde_json::Value = serde_json::from_str(stdout.trim()).expect("hook stdout must be one JSON object");
    assert_eq!(envelope["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    let ctx = envelope["hookSpecificOutput"]["additionalContext"].as_str().expect("additionalContext must be a string");
    assert!(ctx.contains("review for null derefs"), "{ctx}");
    assert!(ctx.contains("always check Option before unwrap"), "{ctx}");
}

/// Regression for the whole-branch-review `s8-retrieve-before-task`
/// finding: a repo dirname containing `_` is PRESERVED by the Rust
/// `regime_key` canonicalizer (`my_repo_v2` stays `my_repo_v2`), and
/// the hook now assembles `--regime` through that SAME serializer
/// (`canon regime-key`), so it queries the IDENTICAL namespace a
/// strategy was written to by the S4/S6/S14 write path. The pre-fix
/// hook slugified `my_repo_v2` to `my-repo-v2` and `retrieve_guidance`
/// silently fail-softed to empty. A controlled-name `git init`ed subdir
/// gives a basename that is NOT the random `.tmp*` tempdir leaf.
#[test]
fn pre_dispatch_hook_regime_preserves_underscores_matching_the_rust_write_path() {
    if !have("jq") || !have("git") {
        eprintln!("skipping: jq/git not on PATH in this environment");
        return;
    }
    let parent = tempfile::tempdir().unwrap();
    let repo = parent.path().join("my_repo_v2");
    std::fs::create_dir(&repo).unwrap();
    init_git_repo(&repo);
    let hash = area_hash("general");
    let regime = RegimeKey::parse(regime_key("code-reviewer", "my_repo_v2", "general", &hash)).unwrap();
    assert!(regime.as_str().contains("/my_repo_v2/"), "sanity: the write-path key keeps the underscore, got {}", regime.as_str());
    seed_strategy(&repo, &regime, &RoleId::parse("code-reviewer").unwrap(), "underscore regime guidance", "guidance keyed under my_repo_v2");

    let stdin_json = r#"{"tool_name":"Task","tool_input":{"subagent_type":"code-reviewer"}}"#;
    let output = run_hook(&repo, stdin_json);
    assert!(output.status.success(), "hook must always exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.trim().is_empty(), "hook must surface guidance from the underscore-preserving namespace (pre-fix slugify would miss it); got nothing");
    let envelope: serde_json::Value = serde_json::from_str(stdout.trim()).expect("hook stdout must be one JSON object");
    let ctx = envelope["hookSpecificOutput"]["additionalContext"].as_str().expect("additionalContext must be a string");
    assert!(ctx.contains("underscore regime guidance"), "{ctx}");
}

#[test]
fn pre_dispatch_hook_is_silent_and_exits_zero_with_no_guidance_stored() {
    if !have("jq") || !have("git") {
        eprintln!("skipping: jq/git not on PATH in this environment");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());

    let stdin_json = r#"{"tool_name":"Task","tool_input":{"subagent_type":"code-reviewer"}}"#;
    let output = run_hook(dir.path(), stdin_json);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty(), "no guidance stored must be silent");
}

#[test]
fn pre_dispatch_hook_is_silent_and_exits_zero_for_a_non_task_tool_call() {
    if !have("jq") || !have("git") {
        eprintln!("skipping: jq/git not on PATH in this environment");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    let repo_raw = dir.path().file_name().unwrap().to_string_lossy().into_owned();
    let hash = area_hash("general");
    let regime = RegimeKey::parse(regime_key("code-reviewer", &repo_raw, "general", &hash)).unwrap();
    seed_strategy(dir.path(), &regime, &RoleId::parse("code-reviewer").unwrap(), "should never surface", "irrelevant to a non-Task tool call");

    let stdin_json = r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#;
    let output = run_hook(dir.path(), stdin_json);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty(), "a non-Task dispatch must never surface guidance");
}

#[test]
fn pre_dispatch_hook_is_silent_and_exits_zero_when_canon_is_missing_from_path() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new("/bin/sh")
        .arg(script_path())
        .current_dir(dir.path())
        .env("PATH", "/nonexistent-path-for-this-test-only")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(b"{}")?;
            child.wait_with_output()
        })
        .expect("spawn pre-dispatch.sh with an empty PATH");
    assert!(output.status.success(), "a missing `canon`/`jq` must never fail the hook");
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
}
