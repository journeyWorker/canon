//! Integration test for `canon ingest sessions [--watch] [--home <dir>]`
//! (S3 `s3-session-ingest`), invoking the actually-built `canon` binary
//! against an offline fixture (`support::Fixture`), zero network, no
//! credentials.
//!
//! Covers ReviewS3Full finding 4: when `canon-store`'s tiers aren't
//! reachable (or `session`/`run`/`event` aren't routed),
//! `canon_cli::ingest`'s documented JSON fallback must actually emit BY
//! DEFAULT — no `--json` flag required — so the only copy of the
//! normalized output is never silently discarded.

mod support;

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn write_omp_fixture_home(home: &std::path::Path) {
    let session_dir = home.join(".omp/agent/sessions/-tmp-proj");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("s1.jsonl"),
        "{\"type\":\"session\",\"id\":\"cli_ing_ses_1\",\"cwd\":\"/tmp/proj\"}\n\
         {\"type\":\"message\",\"timestamp\":\"2026-07-01T00:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"secret directive prose\"}}\n\
         {\"type\":\"message\",\"timestamp\":\"2026-07-01T00:00:01Z\",\"message\":{\"role\":\"assistant\",\"model\":\"gpt-4o-mini\",\"provider\":\"openai\",\"usage\":{\"input\":10,\"output\":5}}}\n",
    )
    .unwrap();
}

/// `canon ingest sessions` runs with `use_env_roots: true`
/// unconditionally (`canon_cli::ingest::run`), so an AMBIENT
/// `CODEX_HOME`/`CANON_INGEST_CLAUDE_SESSIONS_DIR`/
/// `CANON_INGEST_OMP_SESSIONS_DIR`/`HERMES_HOME` in the calling shell
/// (e.g. this very agent's own coding-CLI runtime home) would
/// otherwise leak real session data into an isolated `--home` test —
/// clear them for the child process so the fixture home is the ONLY
/// thing scanned, matching how `fixture.run_canon` isolates
/// `CANON_R2_LOCAL_ROOT` for the tier layer.
fn run_canon_ingest(fixture: &support::Fixture, extra_args: &[&str]) -> Output {
    let bin = Path::new(env!("CARGO_BIN_EXE_canon"));
    let mut args: Vec<String> = vec!["ingest".to_string(), "sessions".to_string()];
    args.extend(extra_args.iter().map(|s| s.to_string()));
    args.push("--repo".to_string());
    args.push(fixture.canon_yaml_path().parent().unwrap().display().to_string());
    Command::new(bin)
        .args(&args)
        .env("CANON_R2_LOCAL_ROOT", fixture.r2_root())
        .env_remove("CODEX_HOME")
        .env_remove("CANON_INGEST_CLAUDE_SESSIONS_DIR")
        .env_remove("CANON_INGEST_OMP_SESSIONS_DIR")
        .env_remove("HERMES_HOME")
        .output()
        .expect("spawning the built `canon` binary")
}

#[test]
fn unrouted_default_run_prints_the_normalized_json_fallback_without_the_json_flag() {
    // Empty routing/aging -> `session`/`run`/`event` are unrouted, so
    // `canon_cli::ingest::run` takes the documented-seam `unwritten`
    // path. NO `--json` flag is passed here — this is the exact
    // default invocation ReviewS3Full finding 4 flags.
    let fixture = support::Fixture::new("", "");
    let home = tempfile::tempdir().unwrap();
    write_omp_fixture_home(home.path());

    let output = run_canon_ingest(&fixture, &["--home", &home.path().display().to_string(), "--all-workspaces"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));

    let stdout = support::stdout(&output);
    assert!(stdout.contains("printing JSON instead"), "human summary must still claim the fallback: {stdout}");

    // The human summary line and the metadata-only JSON body are both on
    // stdout; locate the object's start and parse the remainder.
    let json_start = stdout.find('{').unwrap_or_else(|| panic!("no JSON object found in default (non --json) output: {stdout}"));
    let payload: Value = serde_json::from_str(&stdout[json_start..]).expect("valid JSON on stdout even without --json");
    let sessions = payload["sessions"].as_array().expect("metadata session summaries");
    assert_eq!(sessions.len(), 1, "only the isolated fixture home's one session, no ambient env leakage: {stdout}");
    assert_eq!(sessions[0]["sessionId"], "cli_ing_ses_1", "the only normalized output must not be discarded");
    assert!(payload["events"].is_array(), "metadata fallback must include event summaries");
    assert!(payload["failure"]["class"].is_string(), "metadata fallback must include stable failure class");
    assert!(!stdout.contains("secret directive prose"), "metadata-only fallback must omit directive text: {stdout}");
}

#[test]
fn routed_default_run_persists_and_prints_no_json_body() {
    let fixture = support::Fixture::new("  session: local\n  run: local\n  event: local\n", "");
    let home = tempfile::tempdir().unwrap();
    write_omp_fixture_home(home.path());

    let output = run_canon_ingest(&fixture, &["--home", &home.path().display().to_string(), "--all-workspaces"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));

    let stdout = support::stdout(&output);
    assert!(stdout.contains("runs written: 1"), "stdout: {stdout}");
    assert!(!stdout.contains("printing JSON instead"), "a fully-routed/persisted run has nothing to fall back to: {stdout}");
    assert!(!stdout.trim_end().ends_with(']'), "no JSON body should print when everything was persisted: {stdout}");
}

#[test]
fn privacy_opt_in_toggle_rescans_unchanged_source_bytes() {
    let fixture = support::Fixture::new("  session: local\n  run: local\n  event: local\n", "");
    let home = tempfile::tempdir().unwrap();
    write_omp_fixture_home(home.path());

    let first = run_canon_ingest(&fixture, &["--home", &home.path().display().to_string(), "--all-workspaces"]);
    assert!(first.status.success(), "stderr: {}", support::stderr(&first));

    std::fs::write(
        fixture.canon_yaml_path(),
        "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  session: local\n  run: local\n  event: local\ningest:\n  sessions:\n    privacy:\n      capture_user_directives: true\n      max_directive_chars: 4096\n",
    )
    .unwrap();
    let second = run_canon_ingest(&fixture, &["--home", &home.path().display().to_string(), "--all-workspaces"]);
    assert!(second.status.success(), "stderr: {}", support::stderr(&second));
    assert!(support::stdout(&second).contains("omp: 1 file(s) scanned, 1 reparsed"), "privacy policy change must invalidate the unchanged-file cursor: {}", support::stdout(&second));
}

#[test]
fn help_shows_all_workspaces_flag_and_no_stale_wave_text() {
    let bin = Path::new(env!("CARGO_BIN_EXE_canon"));
    let output = Command::new(bin).args(["ingest", "sessions", "--help"]).output().expect("spawning the built `canon` binary");
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));

    let text = support::stdout(&output);
    assert!(text.contains("--all-workspaces"), "{text}");
    assert!(!text.contains("Wave 1"), "help text must not claim a stale Wave-1-only adapter set: {text}");
    assert!(!text.contains("omp` only"), "help text must not claim `omp` is the only registered adapter: {text}");
}

fn privacy_yaml(privacy: &str) -> String {
    format!(
        "tiers:\n  local: {{ backend: git, root: .canon/ledger }}\nrouting:\n  session: local\n  run: local\n  event: local\ningest:\n  sessions:\n    privacy:\n{privacy}"
    )
}

fn ledger_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn capture_privacy_positive_bound_persists_a_bounded_user_directive() {
    let fixture = support::Fixture::new("  session: local\n  run: local\n  event: local\n", "");
    let home = tempfile::tempdir().unwrap();
    write_omp_fixture_home(home.path());
    std::fs::write(fixture.canon_yaml_path(), privacy_yaml("      capture_user_directives: true\n      max_directive_chars: 6\n")).unwrap();

    let output = run_canon_ingest(&fixture, &["--home", &home.path().display().to_string(), "--all-workspaces"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));

    let mut directive_texts = Vec::new();
    for path in ledger_files(&fixture.git_root()) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
        if value["label"] == "user_directive" {
            directive_texts.push(value["detail"]["text"].as_str().unwrap_or_default().to_string());
        }
    }
    assert_eq!(directive_texts, vec!["secret".to_string()], "exactly one user_directive carrying the turn cut to 6 characters");
}

#[test]
fn capture_privacy_rejects_an_absent_zero_or_negative_bound_and_persists_nothing() {
    for bound in ["", "      max_directive_chars: 0\n", "      max_directive_chars: -5\n"] {
        let fixture = support::Fixture::new("  session: local\n  run: local\n  event: local\n", "");
        let home = tempfile::tempdir().unwrap();
        write_omp_fixture_home(home.path());
        std::fs::write(fixture.canon_yaml_path(), privacy_yaml(&format!("      capture_user_directives: true\n{bound}"))).unwrap();
        let before = fixture.git_file_count();

        let output = run_canon_ingest(&fixture, &["--home", &home.path().display().to_string(), "--all-workspaces"]);
        assert!(!output.status.success(), "bound {bound:?} must be rejected; stdout: {}", support::stdout(&output));
        assert_eq!(fixture.git_file_count(), before, "bound {bound:?}: nothing may be persisted");
    }
}
