//! 0.38 test-determinism: no test starts `canon` with the caller's role or actor.
//!
//! canon reads `CANON_ROLE` and `CANON_ACTOR` from the environment, and an agent session that runs `cargo test` has both declared, so a
//! test that inherits them answers differently per caller. Every test file that starts the binary takes its command from
//! `support/hermetic.rs`, which carries neither variable; this test fails a file that does not. The suite itself is run under three
//! callers (no role, an implementer with an actor, an unknown role) to see the result does not change: `just test-callers` and the CI step
//! "unit tests under three callers" run the unit tests (`--lib`) that way, which this file's scan cannot: a unit test reaches the role through
//! `Role::current`, not through a command line.

use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`, `support/hermetic.rs` apart.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") && !path.ends_with("support/hermetic.rs") && !path.ends_with("hermetic_env.rs") {
            out.push(path);
        }
    }
}

/// The modules of shared helpers, which start canon through `crate::hermetic`.
const SHARED: [&str; 3] = ["mod support;", "mod harness;", "mod rule_support;"];

/// The code lines of a source file that name the binary under test or its hermetic command (comments left out).
fn binary_lines(text: &str) -> Vec<&str> {
    text.lines().filter(|line| (line.contains("CARGO_BIN_EXE_canon") || line.contains("hermetic::") || SHARED.iter().any(|shared| line.trim() == *shared)) && !line.trim_start().starts_with("//")).collect()
}

#[test]
fn every_test_file_starts_canon_through_the_hermetic_command() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"), &mut files);
    let mut problems = Vec::new();
    let mut starters = 0;
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let lines = binary_lines(&text);
        if lines.is_empty() {
            continue;
        }
        starters += 1;
        let name = file.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap().display().to_string();
        if !text.contains("mod hermetic;") && !text.contains("use crate::hermetic;") {
            problems.push(format!("{name}: starts the binary but declares no `mod hermetic;` (support/hermetic.rs; a shared module takes `use crate::hermetic;`)"));
        }
        for line in lines.iter().filter(|line| line.contains("Command::new(") && line.contains("CARGO_BIN_EXE_canon")) {
            problems.push(format!("{name}: starts the binary with `Command::new`, which inherits CANON_ROLE and CANON_ACTOR: `{}`", line.trim()));
        }
    }
    assert!(starters > 100, "the scan finds the test files that start canon ({starters})");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The command the tests start canon with carries neither caller variable, and a test's own value wins.
#[path = "support/hermetic.rs"]
mod hermetic;

#[test]
fn the_hermetic_command_inherits_no_role_and_no_actor() {
    let command = hermetic::canon();
    let envs: Vec<_> = command.get_envs().collect();
    for name in hermetic::CALLER_ENV {
        assert!(envs.iter().any(|(key, value)| *key == name && value.is_none()), "{name} is removed from the child's environment: {envs:?}");
    }
    let mut command = hermetic::canon();
    command.env("CANON_ROLE", "reviewer");
    assert!(command.get_envs().any(|(key, value)| key == "CANON_ROLE" && value.is_some()), "a test's own role is kept");
}
