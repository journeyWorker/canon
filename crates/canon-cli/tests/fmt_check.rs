//! Integration test for `canon format` (S11 task 2.1), invoking the
//! actually-built `canon` binary against the `canon-fmt` crate's own
//! fixture corpus (which reproduces a donor project's real, audited drift)
//! — never `canon_cli`'s library functions in-process, same discipline
//! as `tier_age.rs`/`query.rs`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../canon-fmt/fixtures/consumer-corpus/pre/spec")
}

fn run_canon(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `canon format <DIR>` validates unconditionally: a corpus with real
/// violations exits nonzero and lists each audited failure class once.
#[test]
fn format_exits_nonzero_and_lists_the_audited_failure_classes() {
    let output = run_canon(&["format", &fixture_root().to_string_lossy()]);
    assert!(!output.status.success(), "format on a corpus with real violations must exit nonzero");
    let text = stdout(&output);

    // The observed class set comes from the fixture's own report, so a
    // class the CLI drops, duplicates or invents fails here.
    let report = canon_fmt::check(&fixture_root());
    let observed = report.observed_classes();
    for class in ["layout-grammar", "missing-actor", "free-text-ref", "joined-ref", "abbreviated-sha", "one-way-backref"] {
        assert!(observed.iter().any(|c| c.as_str() == class), "the audited fixture must still observe `{class}`");
    }
    let headings: Vec<&str> = text.lines().filter(|line| line.starts_with('[')).collect();
    let expected: Vec<String> =
        observed.iter().map(|class| format!("[{}] {} occurrence(s)", class.as_str(), report.count_by_class(*class))).collect();
    for heading in &expected {
        assert_eq!(headings.iter().filter(|line| *line == heading).count(), 1, "expected exactly one `{heading}` heading in output:\n{text}");
    }
    assert_eq!(headings.len(), expected.len(), "one heading per observed class and no other:\n{text}");
}

/// `--repo <repo-dir>` combined with a repo-relative `<DIR>` resolves the
/// identical corpus, and produces byte-identical stdout and the same exit
/// code as the absolute-path invocation.
#[test]
fn format_with_repo_flag_resolves_identically_to_the_absolute_form() {
    let fixture = fixture_root();
    let relative_root = fixture.file_name().expect("fixture root has a final component");
    // `--repo` names a canon repo (it must hold a canon.yaml): the fixture corpus, linked in beside one.
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\n").unwrap();
    std::os::unix::fs::symlink(&fixture, repo.path().join(relative_root)).unwrap();

    let bare = run_canon(&["format", &repo.path().join(relative_root).to_string_lossy()]);
    let with_repo = run_canon(&["format", &relative_root.to_string_lossy(), "--repo", &repo.path().to_string_lossy()]);

    assert_eq!(with_repo.status.code(), bare.status.code(), "exit code must match the bare positional form");
    assert_eq!(stdout(&with_repo), stdout(&bare), "stdout must be byte-identical to the bare positional form");
}
