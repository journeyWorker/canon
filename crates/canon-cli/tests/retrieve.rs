//! Integration tests for `canon retrieve --role <r> --regime <k> [--k
//! <n>] [--repo <dir>] [--json]` (S8 part2, `s8-retrieve-before-task`,
//! task 1.1), invoking the actually-built `canon` binary
//! (`env!("CARGO_BIN_EXE_canon")`) — never `canon_cli::retrieve`'s
//! library functions in-process, matching `tests/context.rs`/
//! `tests/gate.rs`'s own discipline: pure logic (fail-soft cap-at-k,
//! the human/JSON formatters, the role/regime usage check) is already
//! unit-tested inside `src/retrieve.rs` itself; this file covers the
//! real-process boundary — exit codes and stdout/stderr against a
//! seeded-on-disk store the binary itself never wrote.

use std::path::Path;
use std::process::{Command, Output};

use canon_cli::retrieve::derive_candidates;
use canon_learn::{ParquetStrategyStore, StrategyId, StrategyItem, StrategyStore, TrajectoryId};
use canon_model::ids::{RegimeKey, RoleId, SubjectId, regime_key};
use chrono::Utc;

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Seed a `<repo>/.canon/learn/strategies` `ParquetStrategyStore` with
/// one item, matching `canon_learn::LearnConfig::default`'s own
/// operator-local root — the exact path `canon retrieve` resolves to
/// when `--repo` carries no `canon.yaml` `learn:` override.
fn seed_strategy(repo: &Path, regime_key: &RegimeKey, role: &RoleId, title: &str) {
    let store = ParquetStrategyStore::open(repo.join(".canon").join("learn").join("strategies"));
    let item =
        StrategyItem::new(StrategyId::new(), regime_key.clone(), role.clone(), title, "description", "content", vec![TrajectoryId::new()], Utc::now());
    store.append(&item).expect("seed strategy");
}

fn dev_regime() -> RegimeKey {
    RegimeKey::parse(regime_key("dev", "canon", "join-spine", "9c93d024b1a2")).unwrap()
}

#[test]
fn retrieve_over_a_seeded_store_returns_the_role_and_regime_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let regime = dev_regime();
    seed_strategy(dir.path(), &regime, &RoleId::parse("dev").unwrap(), "a validated strategy");

    let output = run_canon(&["retrieve", "--role", "dev", "--regime", regime.as_str(), "--repo", "."], dir.path());
    assert!(output.status.success(), "canon retrieve must exit 0; stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("1 guidance item(s)"), "{text}");
    assert!(text.contains("a validated strategy"), "{text}");

    let json_output = run_canon(&["retrieve", "--role", "dev", "--regime", regime.as_str(), "--repo", ".", "--json"], dir.path());
    assert!(json_output.status.success());
    let guidance: Vec<serde_json::Value> = serde_json::from_slice(&json_output.stdout).expect("--json output must parse as a JSON array");
    assert_eq!(guidance.len(), 1);
    assert_eq!(guidance[0]["title"], "a validated strategy");
}

/// Fail-soft (design decision 3): a repo with no seeded store at all —
/// no `canon.yaml`, no `.canon/learn/strategies` directory — still
/// exits `0` and prints an explicit empty result, never an error.
#[test]
fn retrieve_against_a_nonexistent_store_prints_empty_and_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let regime = dev_regime();

    let output = run_canon(&["retrieve", "--role", "dev", "--regime", regime.as_str(), "--repo", "."], dir.path());
    assert!(output.status.success(), "an empty/nonexistent store must still exit 0; stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("0 guidance item(s)"), "{}", stdout(&output));
}

/// `--k` caps the printed result even against a real spawned process.
#[test]
fn retrieve_respects_a_k_cap_over_the_real_binary() {
    let dir = tempfile::tempdir().unwrap();
    let regime = dev_regime();
    for i in 0..3 {
        seed_strategy(dir.path(), &regime, &RoleId::parse("dev").unwrap(), &format!("strategy {i}"));
    }

    let output = run_canon(&["retrieve", "--role", "dev", "--regime", regime.as_str(), "--repo", ".", "--k", "2", "--json"], dir.path());
    assert!(output.status.success());
    let guidance: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(guidance.len(), 2);
}

/// The `--role`/`--regime` caller-contract check: a mismatch is a
/// clean, reported usage error (exit `2`), never a panic and never
/// silently ignored (`canon_cli::retrieve`'s own module doc).
#[test]
fn retrieve_rejects_a_role_regime_mismatch_with_exit_code_two() {
    let dir = tempfile::tempdir().unwrap();
    let regime = RegimeKey::parse(regime_key("content", "canon", "join-spine", "9c93d024b1a2")).unwrap();

    let output = run_canon(&["retrieve", "--role", "dev", "--regime", regime.as_str(), "--repo", "."], dir.path());
    assert_eq!(output.status.code(), Some(2), "stdout: {} stderr: {}", stdout(&output), stderr(&output));
    assert!(stderr(&output).contains("does not match"), "{}", stderr(&output));
}

/// An unparseable `--regime` (not the `<role>/<repo>/<area>/<hash>`
/// grammar) is rejected by `clap`'s own value-parser machinery before
/// `canon_cli::retrieve::run` is ever called — never a panic.
#[test]
fn retrieve_rejects_a_malformed_regime_value_via_clap() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_canon(&["retrieve", "--role", "dev", "--regime", "not-a-regime-key"], dir.path());
    assert!(!output.status.success());
    assert!(stderr(&output).contains("regime"), "{}", stderr(&output));
}

#[test]
fn retrieve_help_documents_role_and_regime_flags() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_canon(&["retrieve", "--help"], dir.path());
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("--role"), "{text}");
    assert!(text.contains("--regime"), "{text}");
    assert!(text.contains("--k "), "{text}");
    assert!(text.contains("--json"), "{text}");
}

fn subject_id(s: &str) -> SubjectId {
    SubjectId::parse(s).unwrap()
}

/// s36 derived-candidates path over the real binary: with the
/// subject-scoped namespace populated, `--domain`/`--subject` serves the
/// `<domain>-<subject_id>` hit — no fallback, no stderr note, `--json`
/// stays a raw array. The candidate keys are DERIVED with the same
/// `canon_cli::retrieve::derive_candidates` the binary uses, so the
/// seeded regime and the queried regime are identical by construction.
#[test]
fn retrieve_domain_subject_serves_the_subject_scoped_hit() {
    let dir = tempfile::tempdir().unwrap();
    let role = RoleId::parse("planning").unwrap();
    let subj = subject_id("my-subject");
    let candidates = derive_candidates(dir.path(), &role, "planning", Some(&subj)).unwrap();
    seed_strategy(dir.path(), &candidates[0], &role, "subject-scoped");
    seed_strategy(dir.path(), &candidates[1], &role, "domain-scoped");

    let repo = dir.path().to_str().unwrap();
    let output =
        run_canon(&["retrieve", "--role", "planning", "--domain", "planning", "--subject", "my-subject", "--repo", repo, "--json"], dir.path());
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let guidance: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).expect("--json output must parse as a JSON array");
    assert_eq!(guidance.len(), 1);
    assert_eq!(guidance[0]["title"], "subject-scoped");
    assert!(!stderr(&output).contains("fell back"), "no fallback ⇒ no serving note; stderr: {}", stderr(&output));
}

/// s36 derived-candidates fallback over the real binary: an EMPTY
/// subject namespace falls back to the `<domain>` candidate. `--json`
/// stdout stays the raw array (backward compatible); the serving-regime
/// note goes to stderr, and the human table surfaces the fallback line.
#[test]
fn retrieve_domain_subject_falls_back_to_domain_when_subject_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let role = RoleId::parse("planning").unwrap();
    let subj = subject_id("my-subject");
    let candidates = derive_candidates(dir.path(), &role, "planning", Some(&subj)).unwrap();
    // Only the DOMAIN namespace is seeded.
    seed_strategy(dir.path(), &candidates[1], &role, "domain-scoped");

    let repo = dir.path().to_str().unwrap();
    let output =
        run_canon(&["retrieve", "--role", "planning", "--domain", "planning", "--subject", "my-subject", "--repo", repo, "--json"], dir.path());
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let guidance: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).expect("--json stdout stays a raw JSON array on fallback");
    assert_eq!(guidance.len(), 1);
    assert_eq!(guidance[0]["title"], "domain-scoped");
    assert!(stderr(&output).contains("fell back"), "fallback must note the serving regime on stderr: {}", stderr(&output));

    // Human mode surfaces the fallback line on stdout.
    let human =
        run_canon(&["retrieve", "--role", "planning", "--domain", "planning", "--subject", "my-subject", "--repo", repo], dir.path());
    assert!(human.status.success(), "stderr: {}", stderr(&human));
    assert!(stdout(&human).contains("fell back from"), "human output notes the fallback: {}", stdout(&human));
}

/// `--regime` and `--domain` together is a loud usage error (exit `2`),
/// mirroring the role/regime-mismatch convention (s36 XOR enforcement).
#[test]
fn retrieve_rejects_regime_and_domain_together_with_exit_two() {
    let dir = tempfile::tempdir().unwrap();
    let regime = dev_regime();
    let output = run_canon(&["retrieve", "--role", "dev", "--regime", regime.as_str(), "--domain", "planning", "--repo", "."], dir.path());
    assert_eq!(output.status.code(), Some(2), "a scope conflict must exit 2; stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("mutually exclusive"), "stderr names the conflict: {}", stderr(&output));
}

