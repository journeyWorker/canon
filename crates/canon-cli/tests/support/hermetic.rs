//! The one way a test starts the `canon` binary (0.38 test-determinism).
//!
//! canon reads the caller's declared role (`CANON_ROLE`) and actor (`CANON_ACTOR`) from the environment, and an agent session that runs
//! `cargo test` has both declared. A test that inherits them answers differently per caller, so a test starts canon here: the command
//! carries neither variable, and a test that wants a role or an actor sets it on the command it gets back. `hermetic_env.rs` fails
//! any test file that starts the binary another way.

use std::process::Command;

/// The environment variables that declare who runs canon; a test sets what it needs and inherits nothing.
pub const CALLER_ENV: [&str; 2] = ["CANON_ROLE", "CANON_ACTOR"];

/// `command` without the caller's role or actor: for a process that starts canon itself (a pty, a hook script).
pub fn scrub(command: &mut Command) -> &mut Command {
    for name in CALLER_ENV {
        command.env_remove(name);
    }
    command
}

/// The `canon` binary under test, without the caller's role or actor.
pub fn canon() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    scrub(&mut command);
    command
}
