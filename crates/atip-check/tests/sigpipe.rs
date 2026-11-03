//! A document written to the stdin of a tool that has already exited does not end the checker.
//!
//! `atip` restores the default SIGPIPE action once a run is over, so that a closed stdout ends it by the signal (spec 8.4). The calls of a
//! run write documents to the stdin of the binary under test, and a tool that refuses a call without reading stdin has exited by the time the
//! writer runs: with the default action the write would kill `atip` (exit by signal 13, no output). This test is the only test of its
//! process, because it changes the process-wide SIGPIPE action.

use atip_check::exec::{Call, Exit, Runner};
use atip_check::fixture::{load, Fixture};

#[expect(
    unsafe_code,
    reason = "libc::signal is an unsafe FFI call with no safe std equivalent for restoring SIGPIPE"
)]
fn default_sigpipe() {
    // SAFETY: the one test of this process, before any thread of its own exists; it only changes the SIGPIPE action.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("fixture.json");
    std::fs::write(&path, r#"{"timeoutSeconds": 20}"#).expect("write");
    load(&path).expect("a fixture")
}

#[test]
fn atip_check_sigpipe_a_tool_that_exits_without_reading_stdin_is_no_signal_for_the_checker() {
    default_sigpipe();
    let mut runner = Runner::new("/bin/sh".as_ref(), &fixture()).expect("runner");
    // A megabyte does not fit the pipe: the write is still going when the tool has exited, and fails with EPIPE.
    let document = vec![b'x'; 1 << 20];
    for _ in 0..20 {
        let call = Call::new(["-c", "exit 2"]).with_stdin(document.clone());
        let outcome = runner.run(&call).expect("the run goes on");
        assert_eq!(outcome.exit, Exit::Code(2));
    }
}
