//! Integration test for `canon context [--repo][--json]` (S12
//! `context-authoring-surface`), invoking the actually-built `canon`
//! binary — never `canon_cli::context`'s library functions in-process,
//! same discipline as `fmt_check.rs`/`tier_age.rs`/`query.rs`.
//!
//! Byte-stability, JSON/outline agreement, and the same-registry proof are
//! covered as library-level unit tests in `src/context.rs` (they need no
//! subprocess); this file covers invariant 1 (capability query, not
//! validation) specifically, which needs the real binary boundary — `canon
//! context`'s exit code, spawned as its own process, against a corpus a
//! sibling `canon fmt --check` run (also spawned) proves has real
//! violations.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The exact corpus `fmt_check.rs`'s own
/// `fmt_check_exits_nonzero_and_lists_audited_gap_categories` test proves
/// fails `canon fmt --check` (40 violations across 8 failure classes) —
/// reused here rather than a second fixture, so this test's "the corpus has
/// real diagnostics" premise is never just asserted, it is the same corpus
/// already exercised by that test.
fn violating_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../canon-fmt/fixtures/consumer-corpus/pre/spec")
}

fn run_canon(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Invariant 1 (capability query, not validation): `canon context` exits 0
/// and emits a full surface (every one of the thirteen registered kinds,
/// every enum domain, every join key) even though the SAME corpus fails
/// `canon fmt --check` with real violations.
#[test]
fn context_exits_zero_with_a_full_surface_even_when_the_corpus_fails_fmt_check() {
    let corpus = violating_corpus();

    let fmt_output = run_canon(&["fmt", "--check", &corpus.to_string_lossy()]);
    assert!(!fmt_output.status.success(), "sanity check: this corpus must still fail `canon fmt --check`");

    let context_output = run_canon(&["context", "--repo", &corpus.to_string_lossy()]);
    assert!(
        context_output.status.success(),
        "canon context must exit 0 regardless of corpus diagnostics; stderr: {}",
        String::from_utf8_lossy(&context_output.stderr)
    );

    let text = stdout(&context_output);
    assert!(text.starts_with("capabilityVersion:"), "expected the default outline to lead with capabilityVersion:\n{text}");
    assert!(text.contains("kinds (13):"), "expected all thirteen record kinds regardless of corpus violations:\n{text}");
    assert!(text.contains("enums ("), "expected the enums section present:\n{text}");
    assert!(text.contains("joinKeys ("), "expected the joinKeys section present:\n{text}");
    assert!(text.contains("cel:"), "expected the S13 CEL binding section present in the default outline:\n{text}");

    let json_output = run_canon(&["context", "--repo", &corpus.to_string_lossy(), "--json"]);
    assert!(json_output.status.success(), "canon context --json must also exit 0 against a violating corpus");
    let json: serde_json::Value = serde_json::from_slice(&json_output.stdout).expect("--json output must parse as JSON");
    assert_eq!(json["kinds"].as_object().map(|o| o.len()), Some(13), "JSON surface must list all thirteen kinds");
    assert_eq!(json["cel"].as_object().map(|o| o.len()), Some(13), "JSON surface must carry a per-kind cel section for all thirteen kinds");
    let task_cel = &json["cel"]["task"];
    assert!(task_cel["fields"].as_object().is_some_and(|f| !f.is_empty()), "cel.task.fields must be non-empty: {json}");
    assert!(task_cel["functions"].as_array().is_some_and(|f| !f.is_empty()), "cel.task.functions must be non-empty: {json}");
}

/// `--repo` defaults to `.` — a bare `canon context` from a directory with
/// no `canon.yaml`/`.canon/policy.yaml` at all still resolves and exits 0
/// (policy degrades to documented defaults, matching the library-level
/// `resolve_surface_never_fails_on_a_repo_with_no_canon_state_at_all` test).
#[test]
fn context_with_no_repo_flag_defaults_to_cwd_and_still_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_canon")).arg("context").current_dir(dir.path()).output().expect("spawn canon binary");
    assert!(output.status.success(), "canon context with no --repo must default to `.` and still exit 0");
    assert!(stdout(&output).contains("kinds (13):"));
}

/// D7/task 1.4: `canon context` invoked from a SUBDIRECTORY of a fixture
/// repo — no `--repo` flag, so clap's `.` default applies — resolves the
/// nearest ANCESTOR `canon.yaml` as the project root, matching `canon
/// fmt`/`canon gate`'s own root convention, and surfaces THAT root's real
/// `.canon/policy.yaml` (`trust_required: p1: human`), never a
/// subdirectory-relative default (the "no canon state at all" degraded
/// policy `context_with_no_repo_flag_defaults_to_cwd_and_still_exits_zero`
/// exercises above).
#[test]
fn context_from_a_subdirectory_resolves_the_ancestor_repo_root_policy() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("canon.yaml"), "tiers:\n  git: { root: .canon/ledger }\n").unwrap();
    std::fs::create_dir_all(repo.path().join(".canon")).unwrap();
    std::fs::write(repo.path().join(".canon/policy.yaml"), "trust_required:\n  p1: human\n").unwrap();

    let subdir = repo.path().join("nested").join("deep");
    std::fs::create_dir_all(&subdir).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_canon")).args(["context", "--json"]).current_dir(&subdir).output().expect("spawn canon binary");
    assert!(output.status.success(), "canon context from a subdirectory must still exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("--json output must parse as JSON");
    assert_eq!(
        json["policy"]["clean"].as_bool(),
        Some(true),
        "the repo root's own policy.yaml must load cleanly (walk-up found it), not degrade to defaults:\n{json}"
    );
    assert_eq!(
        json["policy"]["trust_required"]["p1"]["value"],
        serde_json::json!("human"),
        "the repo ROOT's trust_required.p1 must surface from the subdirectory invocation:\n{json}"
    );
}
