//! Whether a run is in JSON mode.
//!
//! In JSON mode stdout carries exactly one document and stderr nothing.
//! The mode comes from two places: argv, read before the parse has produced a
//! command (a usage error must already answer in JSON), and the parsed
//! command's own `--json` flag, which the tool records with
//! [`set_json_mode`] once it has one.

use std::ffi::OsStr;
use std::sync::atomic::{AtomicBool, Ordering};

static JSON_MODE: AtomicBool = AtomicBool::new(false);

/// Marks the process as running in JSON mode: stderr stays empty.
pub fn set_json_mode(json: bool) {
    JSON_MODE.store(json, Ordering::Relaxed);
}

/// Whether the command runs with `--json`, as recorded by [`set_json_mode`].
pub fn json_mode() -> bool {
    JSON_MODE.load(Ordering::Relaxed)
}

/// Whether `args` (the words after the program name) ask for `--json`: the
/// word `--json` before the first `--`.
pub fn wants_json<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    args.into_iter().take_while(|arg| arg.as_ref() != "--").any(|arg| arg.as_ref() == "--json")
}

/// [`wants_json`] over this process's own argv, read because the parse may not
/// have produced a command yet.
pub fn argv_wants_json() -> bool {
    wants_json(std::env::args_os().skip(1))
}

#[cfg(test)]
mod tests {
    use super::wants_json;

    #[test]
    fn json_is_wanted_by_the_word_before_the_first_double_dash() {
        assert!(wants_json(["status", "--json"]));
        assert!(wants_json(["--json", "status"]));
        assert!(!wants_json(["status"]));
        assert!(!wants_json(["status", "--", "--json"]));
        assert!(!wants_json(["status", "--json-lines"]));
        assert!(!wants_json(Vec::<&str>::new()));
    }
}
