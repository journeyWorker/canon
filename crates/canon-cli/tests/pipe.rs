//! 0.20: `canon … | head` must end quietly. Rust starts with SIGPIPE ignored,
//! so a write to a stdout whose reader is gone used to panic `println!` (exit
//! 101, "failed printing to stdout: Broken pipe"); canon restores the default
//! action and dies of SIGPIPE like other command-line tools. Against the
//! actually-built `canon` binary, with a stdout pipe whose read end is closed
//! before the first write.
#![cfg(unix)]

use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

#[test]
fn a_closed_stdout_ends_canon_by_sigpipe_not_a_panic() {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let dir = tempfile::tempdir().expect("tempdir");
    let out = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["schema", "--errors"])
        .current_dir(dir.path())
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .expect("spawn canon binary");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("panicked"), "canon panicked on a closed stdout: {stderr}");
    assert_eq!(out.status.signal(), Some(libc_sigpipe()), "expected death by SIGPIPE, got {:?}", out.status);
}

/// SIGPIPE is 13 on every unix canon builds for (macOS, Linux).
fn libc_sigpipe() -> i32 {
    13
}
