//! Integration test for `canon fmt --check` (S11 task 2.1), invoking the
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

#[test]
fn fmt_check_exits_nonzero_and_lists_audited_gap_categories() {
    let output = run_canon(&["fmt", "--check", &fixture_root().to_string_lossy()]);
    assert!(!output.status.success(), "fmt --check on a corpus with real violations must exit nonzero");
    let text = stdout(&output);
    for class in ["layout-grammar", "missing-actor", "free-text-ref", "joined-ref", "abbreviated-sha", "one-way-backref"] {
        assert!(text.contains(&format!("[{class}]")), "expected `[{class}]` in output:\n{text}");
    }
}

/// s26 `repo-flag-uniformity` D1/F3: `--repo <repo-dir>` combined with a
/// repo-relative positional `<root>` resolves the identical corpus, and
/// produces byte-identical stdout/exit-code, to the bare-positional
/// invocation above (`resolve_repo_root(repo).join(root)` == the fixture
/// root when `repo` is the fixture root's parent and `root` is its final
/// path component).
#[test]
fn fmt_check_with_repo_flag_resolves_identically_to_the_bare_positional_form() {
    let fixture = fixture_root();
    let repo = fixture.parent().expect("fixture root has a parent");
    let relative_root = fixture.file_name().expect("fixture root has a final component");

    let bare = run_canon(&["fmt", "--check", &fixture.to_string_lossy()]);
    let with_repo = run_canon(&[
        "fmt",
        "--check",
        &relative_root.to_string_lossy(),
        "--repo",
        &repo.to_string_lossy(),
    ]);

    assert_eq!(with_repo.status.code(), bare.status.code(), "exit code must match the bare positional form");
    assert_eq!(stdout(&with_repo), stdout(&bare), "stdout must be byte-identical to the bare positional form");
}
