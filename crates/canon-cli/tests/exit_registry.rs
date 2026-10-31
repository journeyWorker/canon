//! The exit of every error comes from its code's registry row and from nothing
//! else (`canon-0-33-atip-l0` F7, scenarios `atip.l0.23` to `atip.l0.25`):
//! no call site overrides an exit, every row is exit 1 or 2, the thirteen
//! exit-2 codes named for a failure during the work are decided, and a target
//! that cannot be written refuses before anything changes.
//!
//! Tests are named by scenario id (`atip_l0_23_…` proves `atip.l0.23`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_cli::cli_error::{all_codes, code_info};
use serde_json::Value;
use tempfile::TempDir;

/// Every `*.rs` file under `dir`, recursively.
fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// The `src` trees of every crate of the workspace.
fn workspace_sources() -> Vec<PathBuf> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("crates/canon-cli has a parent");
    let mut found = Vec::new();
    for entry in fs::read_dir(crates).unwrap() {
        let src = entry.unwrap().path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut found);
        }
    }
    found.sort();
    found
}

/// The code lines of `path` (comment lines left out), numbered from 1.
fn code_lines(path: &Path) -> Vec<(usize, String)> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.lines().enumerate().filter(|(_, line)| !line.trim_start().starts_with("//")).map(|(at, line)| (at + 1, line.to_string())).collect()
}

/// The text of each `<opener>…)` argument on `line`, up to its closing parenthesis.
fn arguments<'a>(line: &'a str, opener: &str) -> Vec<&'a str> {
    line.match_indices(opener).filter_map(|(at, _)| line[at + opener.len()..].split_once(')').map(|(argument, _)| argument.trim())).collect()
}

/// Whether `argument` is an integer literal (`7`, `101u8`).
fn literal(argument: &str) -> Option<u32> {
    let digits = argument.trim_end_matches("u8").trim_end_matches("i32");
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then(|| digits.parse().expect("digits parse"))
}

/// atip.l0.23: no call site sets an exit. The one definition site of the
/// override is gone, so no `.with_exit(` is left in any crate, and no source
/// of canon-cli ends the process or builds an `ExitCode` from a literal that is
/// not 0, 1 or 2 (`from(code)` of a variable is how a command passes the exit
/// its error already holds).
#[test]
fn atip_l0_23_no_call_site_overrides_an_exit_and_no_literal_exit_is_outside_0_1_2() {
    let sources = workspace_sources();
    assert!(sources.len() > 100, "the scan saw only {} source files; the workspace layout changed", sources.len());

    let mut overrides = Vec::new();
    let mut stray = Vec::new();
    let mut canon_cli_files = 0;
    let canon_cli_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for path in &sources {
        let in_canon_cli = path.starts_with(&canon_cli_src);
        canon_cli_files += usize::from(in_canon_cli);
        for (at, line) in code_lines(path) {
            if line.contains(".with_exit(") {
                overrides.push(format!("{}:{at}: {}", path.display(), line.trim()));
            }
            if !in_canon_cli {
                continue;
            }
            for opener in ["ExitCode::from(", "process::exit("] {
                for argument in arguments(&line, opener) {
                    if let Some(value) = literal(argument) {
                        if value > 2 {
                            stray.push(format!("{}:{at}: {opener}{argument})", path.display()));
                        }
                    }
                }
            }
        }
    }
    assert!(canon_cli_files > 50, "the scan saw only {canon_cli_files} canon-cli source files");
    assert!(overrides.is_empty(), "a call site sets an exit; a situation that needs another exit needs another code:\n{}", overrides.join("\n"));
    assert!(stray.is_empty(), "an exit other than 0, 1 or 2 is built from a literal:\n{}", stray.join("\n"));
}

/// The scan's own parser reads what it is meant to read.
#[test]
fn atip_l0_23_the_scan_reads_literals_and_leaves_variables_alone() {
    assert_eq!(arguments("return ExitCode::from(3);", "ExitCode::from("), ["3"]);
    assert_eq!(arguments("ExitCode::from(code)", "ExitCode::from("), ["code"]);
    assert_eq!(arguments("ExitCode::from(error.emit(\"x\", json))", "ExitCode::from("), ["error.emit(\"x\", json"]);
    assert_eq!(literal("101u8"), Some(101));
    assert_eq!(literal("code"), None);
    assert_eq!(literal(""), None);
}

/// atip.l0.23: every row of the registry is exit 1 (the command ran and its
/// answer is negative or incomplete) or exit 2 (it refused to run).
#[test]
fn atip_l0_23_every_registry_row_is_exit_1_or_2() {
    let rows = all_codes();
    assert!(rows.len() > 100, "the registry holds {} rows", rows.len());
    let wrong: Vec<String> = rows.iter().filter(|row| !matches!(row.exit, 1 | 2)).map(|row| format!("{} exits {}", row.code, row.exit)).collect();
    assert!(wrong.is_empty(), "a registry exit is 1 or 2: {wrong:?}");
}

/// The exit and retryable of `code`'s registry row.
fn row(code: &str) -> (u8, bool) {
    let info = code_info(code).unwrap_or_else(|| panic!("{code} is not in the registry"));
    (info.exit, info.retryable)
}

/// atip.l0.25: the thirteen exit-2 codes named for a failure during the work,
/// each decided by where its failure can happen. A code that can happen both
/// before and during the work is split: the old name keeps the failure during
/// the work (exit 1) and the new one is the precondition (exit 2). One that
/// can only happen before the work stays exit 2. One that can only happen
/// during it is exit 1.
#[test]
fn atip_l0_25_the_thirteen_checked_codes_hold_their_decided_exits() {
    // (code, decided exit)
    let decided: [(&str, u8); 13] = [
        // split: the failure during the work is the old code, exit 1; the precondition is the partner below.
        ("ARTIFACTS_STORE_FAILED", 1),
        ("ARTIFACTS_STORE_UNREACHABLE", 2), // the precondition partner of ARTIFACTS_STORE_FAILED
        ("EVIDENCE_STORE_FAILED", 1),
        ("HOOKS_INSTALL_FAILED", 1),
        ("INVENTORY_FAILED", 1),
        ("POLICY_APPROVAL_FAILED", 1),
        ("RETENTION_EXPORT_FAILED", 1),
        ("RETENTION_PURGE_FAILED", 1),
        // only during the work: exit 1.
        ("INIT_SKILL_INSTALL_FAILED", 1),
        ("MIGRATE_PINS_FAILED", 1),
        // only before the work: exit 2.
        ("LEDGER_ARCHIVE_UNAVAILABLE", 2),
        ("PLUGIN_SYNC_FAILED", 2),
        ("RETRIEVE_FAILED", 2),
    ];
    for (code, exit) in decided {
        assert_eq!(row(code).0, exit, "{code}");
    }
}

/// atip.l0.25: the precondition side of each split, the situations that had
/// two exits and now have one code each, and `WRITE_FAILED`/`WRITE_UNAVAILABLE`.
#[test]
fn atip_l0_25_every_split_gives_each_exit_its_own_code() {
    let partners: [(&str, u8); 18] = [
        ("EVIDENCE_STORE_UNAVAILABLE", 2),
        ("HOOKS_INSTALL_REFUSED", 2),
        ("INVENTORY_REFUSED", 2),
        ("POLICY_APPROVAL_REFUSED", 2),
        ("RETENTION_EXPORT_REFUSED", 2),
        ("RETENTION_PURGE_REFUSED", 2),
        // `ADAPTER_REFUSED` was exit 1 and 2.
        ("ADAPTER_REFUSED", 2),
        ("ADAPTER_RESPONSE_INVALID", 1),
        // `EVIDENCE_BATCH_REFUSED` was exit 1 (a refused promotion) and 2 (a preflight, a write failure).
        ("EVIDENCE_BATCH_REFUSED", 1),
        // a write that began and failed, and the same failure known before anything changed.
        ("WRITE_FAILED", 1),
        ("WRITE_UNAVAILABLE", 2),
        // the closed policy gate of `subject move` was forced to exit 1 over a registry 2.
        ("POLICY_APPROVAL_UNREADABLE", 1),
        ("POLICY_CHANGED_SINCE_APPROVAL", 1),
        ("SUBJECT_GATE_VIOLATION", 1),
        // `brief approve` computed the exit of these; the registry row is the exit now.
        ("BRIEF_UNREADABLE", 2),
        ("BRIEF_WRITE_FAILED", 1),
        ("BRIEF_APPROVE_REFUSED", 2),
        ("BRIEF_UNAPPROVABLE", 2),
    ];
    for (code, exit) in partners {
        assert_eq!(row(code).0, exit, "{code}");
    }
    // The splits that keep a retry meaningful say so.
    assert!(row("ARTIFACTS_STORE_FAILED").1 && row("ARTIFACTS_STORE_UNREACHABLE").1 && row("EVIDENCE_STORE_FAILED").1 && row("EVIDENCE_STORE_UNAVAILABLE").1);
}

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-l0").output().expect("spawn canon binary")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

/// Every path under `dir` (relative, sorted), `.git` left out.
fn listing(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, found: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            found.push(path.strip_prefix(root).unwrap().display().to_string());
            if path.is_dir() {
                walk(root, &path, found);
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, dir, &mut found);
    found.sort();
    found
}

/// Gives a directory its permissions back when dropped, so the temp dir can be removed.
struct Restore<'a>(&'a Path, fs::Permissions);

impl Drop for Restore<'_> {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.0, self.1.clone());
    }
}

/// atip.l0.24: a command that writes into a directory that cannot be written
/// refuses before anything changes: exit 2, `WRITE_UNAVAILABLE`, and the
/// directory holds what it held.
#[cfg(unix)]
#[test]
fn atip_l0_24_a_directory_that_cannot_be_written_refuses_with_exit_2_and_changes_nothing() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().expect("a temp dir under std::env::temp_dir()");
    let repo = dir.path();
    let init = canon(repo, &["init", "--providers", "none"]);
    assert!(init.status.success(), "{}{}", String::from_utf8_lossy(&init.stdout), String::from_utf8_lossy(&init.stderr));

    let original = fs::metadata(repo).unwrap().permissions();
    let mut read_only = original.clone();
    read_only.set_mode(0o555);
    fs::set_permissions(repo, read_only).unwrap();
    let _restore = Restore(repo, original);

    // A process that can write anyway (root) cannot show the refusal.
    if fs::write(repo.join(".probe"), b"").is_ok() {
        eprintln!("skipped: this process can write into a directory made read-only (running as root?), so the refusal cannot be shown");
        let _ = fs::remove_file(repo.join(".probe"));
        return;
    }

    let before = listing(repo);
    let out = canon(repo, &["gate", "install-hooks", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let document = json(&out);
    assert_eq!(document["ok"], false, "{document}");
    assert_eq!(document["error"]["code"], "WRITE_UNAVAILABLE", "{document}");
    assert_eq!(document["error"]["retryable"], false, "{document}");
    assert_eq!(listing(repo), before, "a refused write leaves the directory as it was");
    assert!(!repo.join(".claude").exists() && !repo.join(".codex").exists(), "no hook file was started");
}
