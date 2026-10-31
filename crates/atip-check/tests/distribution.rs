//! What the checker is and is not as a program: it ends by signal on a closed stdout, and the fake tool is not part of it.

mod support;

use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use support::{Harness, ATIP, FAKE_TOOL};

/// SIGPIPE is 13 on every unix the checker runs on (macOS, Linux).
const SIGPIPE: i32 = 13;

/// `atip` with a stdout whose reader has gone before the first write.
fn with_closed_stdout(args: &[&str]) -> std::process::Output {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let dir = tempfile::tempdir().expect("tempdir");
    Command::new(ATIP)
        .args(args)
        .current_dir(dir.path())
        .env("TMPDIR", dir.path())
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .expect("run atip")
}

#[test]
fn atip_check_54_a_closed_stdout_ends_atip_by_sigpipe_and_never_by_the_panic_exit_101() {
    let (harness, other) = (Harness::new(), Harness::new());
    let fixture = harness.write("fixture.json", &harness.fixture(&json!({})));
    let failing = other.write("failing.json", &other.fixture(&json!({"hintNull": true})));
    let (fixture, failing) = (
        fixture.to_str().expect("utf8").to_string(),
        failing.to_str().expect("utf8").to_string(),
    );
    let runs: Vec<Vec<&str>> = vec![
        vec!["--help"],
        vec!["--version"],
        vec!["check", "--help"],
        vec![
            "check",
            "--bin",
            FAKE_TOOL,
            "--level",
            "L0",
            "--fixture",
            &fixture,
            "--json",
        ],
        vec![
            "check",
            "--bin",
            FAKE_TOOL,
            "--level",
            "L0",
            "--fixture",
            &fixture,
        ],
        vec!["nosuch-xyz", "--json"],
        vec![
            "check",
            "--bin",
            FAKE_TOOL,
            "--level",
            "L0",
            "--fixture",
            &failing,
            "--json",
        ],
    ];
    for args in runs {
        let output = with_closed_stdout(&args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("panicked"), "{args:?} panicked: {stderr}");
        assert_ne!(output.status.code(), Some(101), "{args:?}");
        assert_eq!(
            output.status.signal(),
            Some(SIGPIPE),
            "{args:?} should end by SIGPIPE, got {:?}",
            output.status
        );
    }
    // A failed level in text mode writes only to stderr: with stdout closed it still ends 1, as the contract says.
    let text = with_closed_stdout(&[
        "check",
        "--bin",
        FAKE_TOOL,
        "--level",
        "L0",
        "--fixture",
        &failing,
    ]);
    assert_eq!(text.status.code(), Some(1), "{:?}", text.status);
}

#[test]
fn atip_check_55_the_fake_tool_is_a_test_only_binary_the_checker_does_not_ship() {
    let manifest = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
            "--manifest-path",
        ])
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .output()
        .expect("cargo metadata");
    assert!(
        manifest.status.success(),
        "{}",
        String::from_utf8_lossy(&manifest.stderr)
    );
    let metadata: Value = serde_json::from_slice(&manifest.stdout).expect("json");
    let package = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|package| package["name"] == "atip-check")
        .expect("atip-check");
    let bins: Vec<(&str, &Value)> = package["targets"]
        .as_array()
        .expect("targets")
        .iter()
        .filter(|target| target["kind"] == json!(["bin"]))
        .map(|target| {
            (
                target["name"].as_str().unwrap_or_default(),
                &target["required-features"],
            )
        })
        .collect();
    assert_eq!(
        bins.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        ["atip", "atip-fake-tool"]
    );
    assert!(
        bins[0].1.is_null() || bins[0].1 == &json!([]),
        "atip builds without a feature: {}",
        bins[0].1
    );
    assert_eq!(
        bins[1].1,
        &json!(["test-support"]),
        "the fake tool needs the test-only feature"
    );
    assert_eq!(
        package["features"]["default"],
        json!([]),
        "no default feature builds it"
    );
    // The checker itself is built without the feature when a tool builds or installs it: `cargo build -p atip-check` leaves it out.
    let normal = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .flat_map(|package| {
            package["dependencies"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|dependency| dependency["name"] == "atip-check" && dependency["kind"].is_null())
        .count();
    assert_eq!(normal, 0, "no workspace crate depends on atip-check with its test-support feature as a normal dependency");
}
