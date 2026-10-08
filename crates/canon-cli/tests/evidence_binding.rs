//! Experimental evidence binding end to end through the real binary:
//! `canon evidence add --report` reads a JUnit file the "team's runner"
//! wrote (canon runs nothing), and `experimental.evidence_binding`
//! decides whether `canon gate check` holds the scenario to it.

use std::path::Path;
use std::process::{Command, Output};

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(args)
        .env("CANON_ACTOR", "canon")
        .current_dir(repo)
        .output()
        .expect("spawn canon")
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn policy(repo: &Path, mode: &str) {
    std::fs::write(
        repo.join(".canon/policy.yaml"),
        format!("experimental:\n  evidence_binding:\n    mode: {mode}\n    strength: report\n"),
    )
    .unwrap();
}

fn attest(repo: &Path, extra: &[&str]) -> Output {
    let mut args = vec![
        "evidence",
        "add",
        "--scenario-id",
        "cart.add.01",
        "--project-id",
        "root",
        "--kind",
        "test-run",
        "--ref",
        "cargo nextest run",
        "--role",
        "implementer",
    ];
    args.extend_from_slice(extra);
    let out = canon(repo, &args);
    if out.status.success() {
        let promoted = canon(repo, &["gate", "promote"]);
        assert!(promoted.status.success(), "{}", text(&promoted));
    }
    out
}

const PASSING: &str = r#"<testsuite><testcase classname="cart" name="cart.add.01 refuses an out-of-stock item"/></testsuite>"#;
const FAILING: &str = r#"<testsuite><testcase classname="cart" name="cart.add.01 refuses an out-of-stock item"><failure/></testcase></testsuite>"#;

#[test]
fn require_mode_holds_a_scenario_to_a_passing_report_and_warn_never_fails() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo)
        .status()
        .unwrap()
        .success());
    let init = canon(repo, &["init", "--repo", "."]);
    assert!(init.status.success(), "{}", text(&init));
    let scenario = canon(
        repo,
        &[
            "scenario",
            "new",
            "cart.add.01",
            "--title",
            "An out-of-stock item is refused",
            "--case",
            "failure",
        ],
    );
    assert!(scenario.status.success(), "{}", text(&scenario));
    assert!(canon(repo, &["inventory", "sync"]).status.success());

    // Default (no section): nothing about binding is checked or printed.
    assert!(attest(repo, &[]).status.success());
    let off = canon(repo, &["gate", "check"]);
    assert!(
        off.status.success() && !text(&off).contains("evidence binding"),
        "{}",
        text(&off)
    );

    // require + strength report: an attested-only record is red.
    policy(repo, "require");
    let red = canon(repo, &["gate", "check"]);
    assert_eq!(red.status.code(), Some(1), "{}", text(&red));
    assert!(
        text(&red).contains("cart.add.01") && text(&red).contains("attested"),
        "{}",
        text(&red)
    );

    // A faithful claim over a FAILED case is refused and stages nothing.
    std::fs::create_dir_all(repo.join("reports")).unwrap();
    std::fs::write(repo.join("reports/junit.xml"), FAILING).unwrap();
    let contradicted = attest(repo, &["--report", "junit:reports/junit.xml"]);
    assert_eq!(
        contradicted.status.code(),
        Some(1),
        "{}",
        text(&contradicted)
    );
    assert!(
        text(&contradicted).contains("failed"),
        "{}",
        text(&contradicted)
    );
    assert_eq!(
        canon(repo, &["gate", "check"]).status.code(),
        Some(1),
        "the refused add must not have bound anything"
    );

    // The passing report binds: green, and the distribution says so.
    std::fs::write(repo.join("reports/junit.xml"), PASSING).unwrap();
    let bound = attest(repo, &["--report", "junit:reports/junit.xml"]);
    assert!(bound.status.success(), "{}", text(&bound));
    let green = canon(repo, &["gate", "check"]);
    assert!(green.status.success(), "{}", text(&green));
    assert!(
        text(&green).contains("report 1, artifact 0, attested-only 0"),
        "{}",
        text(&green)
    );

    // warn: a newer unbound attestation is an advisory, never a failure.
    policy(repo, "warn");
    assert!(attest(repo, &[]).status.success());
    let warn = canon(repo, &["gate", "check"]);
    assert!(
        warn.status.success(),
        "warn must not fail the gate: {}",
        text(&warn)
    );
    assert!(text(&warn).contains("warn cart.add.01"), "{}", text(&warn));
}
