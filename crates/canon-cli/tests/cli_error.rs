//! The shared error contract (0.20 D3): every refusal and operational
//! failure carries a stable `code`, in the same two shapes. Human mode puts
//! `canon <command>: <CODE> — <message>` on stderr; `--json` puts
//! `{"error":{"code","message","hint","retryable"}}` on stdout. Exit codes
//! keep their 0.19 meaning. Against the actually-built `canon` binary.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).output().expect("spawn canon binary")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn ok(repo: &Path, args: &[&str]) {
    let out = run(repo, args);
    assert!(out.status.success(), "{args:?}: {}", stderr(&out));
}

/// The `error` object of a `--json` failure, with the whole shape asserted.
fn json_error(out: &Output) -> Value {
    let body: Value = serde_json::from_str(&stdout(out)).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", stdout(out)));
    let error = body.get("error").unwrap_or_else(|| panic!("no `error` object: {body}")).clone();
    let keys: BTreeSet<&str> = error.as_object().expect("error is an object").keys().map(String::as_str).collect();
    // `field`, `expected` and `received` appear only on a refusal about one argument.
    let required = BTreeSet::from(["code", "message", "hint", "retryable", "suggestions"]);
    assert!(required.is_subset(&keys) && keys.is_subset(&BTreeSet::from(["code", "message", "hint", "retryable", "suggestions", "field", "expected", "received"])), "{error}");
    assert!(error["suggestions"].is_array(), "{error}");
    assert!(error.get("expected").is_none() || error.get("field").is_some(), "`expected` always comes with `field`: {error}");
    assert!(error["code"].is_string() && error["message"].is_string() && error["retryable"].is_boolean(), "{error}");
    assert!(error["hint"].is_null() || error["hint"].is_string(), "{error}");
    error
}

/// The code a human-mode failure puts on its first stderr line:
/// `canon <command>: <CODE> — <message>`.
fn human_code(out: &Output) -> String {
    let err = stderr(out);
    let line = err.lines().find(|l| l.starts_with("canon ")).unwrap_or_else(|| panic!("no `canon …:` line on stderr: {err}"));
    let (_, rest) = line.split_once(": ").expect("`canon <command>: <CODE> — …`");
    let (code, _) = rest.split_once(" — ").unwrap_or_else(|| panic!("no ` — ` after the code: {line}"));
    assert!(code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'), "{code}");
    code.to_string()
}

fn brief_path(repo: &Path) -> std::path::PathBuf {
    repo.join(".canon/changes/add-login/brief.md")
}

/// A repo with subject `demo`, change `add-login` (a placeholder-free brief
/// that is not yet approved) and one task, so `evidence add` and `gate task`
/// have something to refuse.
fn repo_with_unapproved_brief() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    let tasks = repo.join(".canon/changes/add-login/tasks.md");
    let mut text = std::fs::read_to_string(&tasks).unwrap_or_else(|_| "# Tasks: Add login\n".to_string());
    text.push_str("\n- [ ] 1 Write the failure-path scenarios\n");
    std::fs::write(&tasks, text).unwrap();
    ok(repo, &["ingest", "plans"]);
    assert!(brief_path(repo).is_file());
    dir
}

#[test]
fn evidence_add_on_an_unapproved_brief_is_refused_with_its_code_and_the_next_step() {
    let dir = repo_with_unapproved_brief();
    let out = run(
        dir.path(),
        &["evidence", "add", "--task-id", "add-login#1", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl"],
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert_eq!(human_code(&out), "BRIEF_UNAPPROVED");
    assert!(stderr(&out).contains("\nhint: stop, report the brief to the human"), "the next step is its own hint line: {}", stderr(&out));
}

#[test]
fn gate_task_without_evidence_is_blocked_with_its_code() {
    let dir = repo_with_unapproved_brief();
    let out = run(dir.path(), &["task", "done", "add-login#1"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert_eq!(human_code(&out), "GATE_TASK_BLOCKED");
}

#[test]
fn finding_close_of_an_unknown_seq_is_refused() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), "routing:\n  finding: local\n").unwrap();
    let out = run(dir.path(), &["finding", "close", "some-change", "--round", "1", "--seq", "9", "--disposition", "rejected", "--reason", "by design"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert_eq!(human_code(&out), "FINDING_NOT_FOUND");
}

#[test]
fn query_scope_errors_and_unknown_kinds_have_codes_in_json_mode() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);

    // A flag that does not apply to the kind: canon's own refusal.
    let out = run(repo, &["query", "--kind", "subject", "--change-id", "add-login", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert_eq!(json_error(&out)["code"], "QUERY_INVALID_ARGUMENT");

    // A kind that does not exist: clap rejects it, and `--json` still gets JSON.
    let out = run(repo, &["query", "--kind", "no-such-kind", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert_eq!(json_error(&out)["code"], "USAGE");
}

#[test]
fn init_over_a_finished_repo_is_refused() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    let out = run(repo, &["init", "--providers", "none", "--no-agents-md"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert_eq!(human_code(&out), "INIT_CONFIG_EXISTS");
}

#[test]
fn an_unknown_flag_with_json_is_a_usage_error_on_stdout_and_without_it_clap_speaks() {
    let dir = TempDir::new().unwrap();
    let out = run(dir.path(), &["status", "--no-such-flag", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    let error = json_error(&out);
    assert_eq!(error["code"], "USAGE");
    assert!(error["message"].as_str().unwrap().contains("--no-such-flag"), "{error}");

    let out = run(dir.path(), &["status", "--no-such-flag"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stdout(&out).is_empty() && stderr(&out).contains("--no-such-flag"), "{}", stderr(&out));

    // Help is not an error, with or without --json.
    let out = run(dir.path(), &["status", "--help", "--json"]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn json_failures_from_a_command_with_its_own_flag_carry_code_message_hint_and_retryable() {
    let dir = repo_with_unapproved_brief();
    let repo = dir.path();

    let out = run(repo, &["brief", "approve", "--approver", "Ana", "--note", " ", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    let error = json_error(&out);
    assert_eq!((error["code"].as_str(), error["retryable"].as_bool()), (Some("APPROVAL_INCOMPLETE"), Some(false)), "{error}");
    assert!(stderr(&out).is_empty(), "JSON mode writes nothing to stderr: {}", stderr(&out));
    let human = run(repo, &["brief", "approve", "--approver", "Ana", "--note", " "]);
    assert_eq!(human_code(&human), "APPROVAL_INCOMPLETE", "human mode says the same thing on stderr: {}", stderr(&human));

    // Exits come from the registry: a missing brief is exit 2.
    let out = run(repo, &["brief", "check", "no-such-change", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert_eq!(json_error(&out)["code"], "BRIEF_NOT_FOUND");

    let out = run(repo, &["subject", "move", "no-such-subject", "--to", "specced", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert_eq!(json_error(&out)["code"], "SUBJECT_NOT_FOUND");
}

// ───────────────────────────── the registry ─────────────────────────────

/// Every `.rs` under `src/`, as text.
fn sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            out.push((path.file_name().unwrap().to_string_lossy().to_string(), std::fs::read_to_string(&path).unwrap()));
        }
    }
    out
}

/// A code literal where the sources hand one to the shared error type: the
/// first argument of `CliError::refused`/`failed`, of `brief approve`'s
/// `refuse(json, …)`, or a `=> "CODE"` arm of a variant-to-code `match`.
fn constructed_codes() -> BTreeSet<String> {
    let screaming = |s: &str| !s.is_empty() && s.starts_with(|c: char| c.is_ascii_uppercase()) && s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    let mut found = BTreeSet::new();
    for (name, text) in sources() {
        if name == "cli_error.rs" {
            continue;
        }
        for marker in ["CliError::refused(", "CliError::failed(", "refuse(", "=>"] {
            for (at, _) in text.match_indices(marker) {
                let mut rest = text[at + marker.len()..].trim_start();
                if marker == "refuse(" {
                    let Some(after) = rest.strip_prefix("json,") else { continue };
                    rest = after.trim_start();
                }
                let Some(quoted) = rest.strip_prefix('"') else { continue };
                let Some((literal, _)) = quoted.split_once('"') else { continue };
                if screaming(literal) {
                    found.insert(literal.to_string());
                }
            }
        }
    }
    found
}

#[test]
fn the_code_registry_is_unique_sorted_and_matches_the_codes_the_sources_construct() {
    let registry = canon_cli::cli_error::all_codes();
    let codes: Vec<&str> = registry.iter().map(|info| info.code).collect();
    let unique: BTreeSet<&str> = codes.iter().copied().collect();
    assert_eq!(unique.len(), codes.len(), "a code is registered twice");
    assert!(codes.windows(2).all(|w| w[0] < w[1]), "the registry is sorted by code");
    assert!(registry.iter().all(|info| !info.meaning.trim().is_empty() && info.code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')), "every code is SCREAMING_SNAKE with a meaning");

    let constructed = constructed_codes();
    let unregistered: Vec<_> = constructed.iter().filter(|c| !unique.contains(c.as_str())).collect();
    assert!(unregistered.is_empty(), "codes used but not in cli_error::all_codes(): {unregistered:?}");

    // The other direction: a registered code the sources never name is dead
    // weight (or the schema would list a code no command can emit).
    let text: String = sources().into_iter().filter(|(name, _)| name != "cli_error.rs").map(|(_, t)| t).collect();
    let dead: Vec<_> = codes.iter().filter(|c| !text.contains(&format!("\"{c}\"")) && !text.contains(&format!("({c})"))).collect();
    assert!(dead.is_empty(), "registered but never used: {dead:?}");
}

#[test]
fn every_code_declares_its_exit_and_retryability_and_the_d4_families_map_to_two() {
    for info in canon_cli::cli_error::all_codes() {
        assert!(matches!(info.exit, 1 | 2), "{} declares exit {} (only 1 and 2 are errors)", info.code, info.exit);
        assert!(!info.meaning.trim().is_empty(), "{} has no meaning", info.code);
        if info.code.ends_with("_NOT_FOUND") || info.code.ends_with("_UNREADABLE") || info.code.ends_with("_INVALID_ARGUMENT") {
            assert_eq!(info.exit, 2, "{} names a missing, unreadable or invalid input, so it is a refusal (exit 2)", info.code);
        }
    }
    let exit_of = |code: &str| canon_cli::cli_error::code_info(code).unwrap_or_else(|| panic!("{code} is not registered")).exit;
    assert_eq!(exit_of("BRIEF_HAS_BLOCKERS"), 1, "blockers are a finding");
    assert_eq!(exit_of("GATE_VIOLATIONS"), 1);
    assert_eq!(exit_of("CONFIG_MISSING"), 2);
    assert_eq!(exit_of("USAGE"), 2);
    assert!(canon_cli::cli_error::code_info("DISPATCH_END_IN_PROGRESS").unwrap().retryable, "a concurrent close is the retryable case");
    assert!(!canon_cli::cli_error::code_info("USAGE").unwrap().retryable);
}

#[test]
fn a_cli_error_takes_its_exit_and_retryability_from_the_registry() {
    use canon_cli::cli_error::CliError;
    assert_eq!(CliError::refused("BRIEF_NOT_FOUND", "x").exit, 2);
    assert_eq!(CliError::failed("BRIEF_NOT_FOUND", "x").exit, 2, "the constructor does not choose the exit");
    assert_eq!(CliError::failed("GATE_VIOLATIONS", "x").exit, 1);
    assert!(CliError::refused("DISPATCH_END_IN_PROGRESS", "x").retryable);
}

#[test]
fn a_received_value_is_redacted_when_it_looks_like_a_secret() {
    use canon_cli::cli_error::redact;
    assert_eq!(redact(Some("--api-token"), "abc"), "<redacted>");
    assert_eq!(redact(None, "CANON_SECRET=hunter2"), "CANON_SECRET=<redacted>");
    assert_eq!(redact(None, "ghp_0123456789abcdef"), "<redacted>");
    assert_eq!(redact(None, "Ab3dEf6hIj9kLm2nOp5qRs8tUv1"), "<redacted>");
    assert_eq!(redact(Some("--kind"), "evidence"), "evidence");
    let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    assert_eq!(redact(Some("--plan-digest"), digest), digest, "a digest is not a secret");
    assert_eq!(redact(Some("--signature-file"), "approval.sig"), "approval.sig");
}
