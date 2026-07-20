//! Integration tests for `canon dashboard [--repo <dir>] [--snapshot
//! <dir>] [--port <n>]` (S9 part3, `s9-unified-surface` tasks.md 6.1),
//! invoking the actually-built `canon` binary
//! (`env!("CARGO_BIN_EXE_canon")`) — matching `tests/report.rs`/
//! `tests/gate.rs`'s own discipline: pure logic (route/path resolution)
//! is already unit-tested inside `src/dashboard.rs` itself; this file
//! covers the real-process boundary — a real bound TCP server actually
//! answering real HTTP requests.
//!
//! Every test here runs `canon dashboard --repo <the real canon repo
//! root> --snapshot packages/dashboard/fixtures/snapshot --port 0`: an
//! EXPLICIT, already-`manifest.json`-carrying snapshot dir (the
//! committed dashboard fixture, task 7.2's own selftest oracle — see
//! `tests/selftest_fixture.rs`), so this file never needs `duckdb` on
//! `PATH` and never regenerates or otherwise touches the real repo's
//! `canon/` tree — a fully read-only exercise of the server itself.
//! `--port 0` asks the OS for any free port; the actually-bound port is
//! parsed back out of the process's own first stdout line (module doc
//! of `canon_cli::dashboard`: always printed, regardless of what was
//! requested), so parallel test runs never collide on a fixed port.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

fn run_canon(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// `crates/canon-cli` -> `crates` -> the real canon repo root — the
/// checkout this binary itself was compiled from, whose already-built
/// `packages/dashboard/dist` and committed `packages/dashboard/
/// fixtures/snapshot` this file serves read-only.
fn canon_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

/// A live `canon dashboard` child process plus the port it actually
/// bound (parsed from its own first stdout line) — killed on `Drop` so
/// a failing assertion never leaks a background server.
struct LiveDashboard {
    child: Child,
    port: u16,
}

impl Drop for LiveDashboard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawns `canon dashboard --repo <repo> --snapshot <snapshot> --port
/// 0` and blocks (bounded) until its own "serving http://127.0.0.1:
/// <port>/…" line appears on stdout, parsing the port back out — the
/// server is provably ready to accept connections by the time this
/// returns (module doc: the print happens right before the accept
/// loop starts).
fn spawn_dashboard(repo: &Path, snapshot: &Path) -> LiveDashboard {
    let mut child = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["dashboard", "--repo", repo.to_str().unwrap(), "--snapshot", snapshot.to_str().unwrap(), "--port", "0"])
        .current_dir(repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn canon dashboard");

    let mut out = BufReader::new(child.stdout.take().expect("piped stdout"));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        loop {
            line.clear();
            match out.read_line(&mut line) {
                Ok(0) => {
                    let _ = tx.send(None);
                    return;
                }
                Ok(_) => {
                    if let Some(port) = line.trim_end().rsplit(':').next().and_then(|tail| tail.split('/').next()).and_then(|p| p.parse::<u16>().ok()) {
                        if line.contains("serving http://") {
                            let _ = tx.send(Some(port));
                            return;
                        }
                    }
                }
                Err(_) => {
                    let _ = tx.send(None);
                    return;
                }
            }
        }
    });

    let port = match rx.recv_timeout(Duration::from_secs(20)) {
        Ok(Some(port)) => port,
        Ok(None) => panic!("canon dashboard exited before printing a `serving` line"),
        Err(_) => panic!("canon dashboard never printed its `serving` line within 20s"),
    };

    LiveDashboard { child, port }
}

/// A minimal hand-rolled HTTP/1.1 GET client (no dependency added —
/// same "hand-roll instead of adding an HTTP crate" discipline the
/// server itself follows, module doc of `canon_cli::dashboard`).
/// Returns `(status_code, body_bytes)`.
fn http_get(port: u16, path: &str) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to canon dashboard");
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read response");

    let split_at = raw.windows(4).position(|w| w == b"\r\n\r\n").expect("response has a header/body separator");
    let head = String::from_utf8_lossy(&raw[..split_at]).into_owned();
    let body = raw[split_at + 4..].to_vec();
    let status_line = head.lines().next().expect("status line");
    let code: u16 = status_line.split_whitespace().nth(1).expect("status code").parse().expect("numeric status code");
    (code, body)
}

#[test]
fn dashboard_help_smoke() {
    let output = run_canon(&["dashboard", "--help"], Path::new("."));
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("--repo"), "{text}");
    assert!(text.contains("--snapshot"), "{text}");
    assert!(text.contains("--port"), "{text}");
}

#[test]
fn dashboard_errors_clearly_when_the_app_is_not_built() {
    let repo = tempfile::tempdir().unwrap();
    let snapshot = canon_repo_root().join("packages/dashboard/fixtures/snapshot");
    let output = run_canon(&["dashboard", "--repo", ".", "--snapshot", snapshot.to_str().unwrap(), "--port", "0"], repo.path());
    assert!(!output.status.success());
    assert!(stderr(&output).contains("not built"), "{}", stderr(&output));
    assert!(stderr(&output).contains("packages/dashboard/dist"), "{}", stderr(&output));
}

#[test]
fn dashboard_errors_clearly_when_repo_is_missing_packages_dashboard_entirely() {
    let repo = tempfile::tempdir().unwrap();
    let snapshot = repo.path().join("some-snapshot");
    std::fs::create_dir_all(&snapshot).unwrap();
    std::fs::write(snapshot.join("manifest.json"), "{}").unwrap();
    let output = run_canon(&["dashboard", "--repo", ".", "--snapshot", snapshot.to_str().unwrap(), "--port", "0"], repo.path());
    assert!(!output.status.success());
    assert!(stderr(&output).contains("not built"), "{}", stderr(&output));
}

#[test]
fn dashboard_serves_the_built_app_at_root_and_the_live_snapshot_route() {
    let repo = canon_repo_root();
    let snapshot_dir = repo.join("packages/dashboard/fixtures/snapshot");
    assert!(snapshot_dir.join("manifest.json").is_file(), "the committed dashboard fixture must exist for this test to be meaningful");
    let dist_dir = repo.join("packages/dashboard/dist");
    if !dist_dir.join("index.html").is_file() {
        eprintln!("skipping: packages/dashboard is not built (no dist/index.html) — run `bun run build` in packages/dashboard first");
        return;
    }

    let live = spawn_dashboard(&repo, &snapshot_dir);

    // `/` serves the built app's own index.html.
    let (code, body) = http_get(live.port, "/");
    assert_eq!(code, 200);
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("canon dashboard"), "{html}");

    // The live-snapshot route serves the EXACT fixture manifest.json —
    // never regenerated (an explicit --snapshot with an existing
    // manifest.json is served as-is, module doc of
    // `canon_cli::dashboard`).
    let expected_manifest = std::fs::read(snapshot_dir.join("manifest.json")).unwrap();
    let (code, body) = http_get(live.port, "/live-snapshot/manifest.json");
    assert_eq!(code, 200);
    assert_eq!(body, expected_manifest, "the served manifest.json must be byte-identical to the committed fixture — never regenerated over an explicit, already-populated --snapshot dir");

    // One of the declared Parquet tables, byte-identical too.
    let expected_parquet = std::fs::read(snapshot_dir.join("mart_trust_matrix.parquet")).unwrap();
    let (code, body) = http_get(live.port, "/live-snapshot/mart_trust_matrix.parquet");
    assert_eq!(code, 200);
    assert_eq!(body, expected_parquet);

    // A nonexistent path 404s cleanly.
    let (code, _) = http_get(live.port, "/this-does-not-exist");
    assert_eq!(code, 404);

    // Path-traversal is rejected at the real HTTP boundary, not just
    // the unit-tested `resolve_file` helper.
    let (code, _) = http_get(live.port, "/live-snapshot/../../../../../../etc/passwd");
    assert_eq!(code, 404);
}
