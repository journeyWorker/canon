//! Why a file or state could not be read, as the code of the answer (`canon-0-36-codes-stored`, atip/1 spec 7.2.2, 8.3).
//!
//! A refusal to read says what is wrong with what was read, never that the read "was unreadable":
//!
//! - the system refuses access: `<SUBJECT>_DENIED`;
//! - the caller named a path that is no file or no directory (`EISDIR`, `ENOTDIR`) or a file a person edits is wrong
//!   (it does not parse): `<SUBJECT>_INVALID`;
//! - the caller named a path that does not exist: `<SUBJECT>_NOT_FOUND`;
//! - state canon wrote does not parse or does not agree with itself: `<SUBJECT>_CORRUPT`;
//! - an operating-system error a repeat can pass (`EIO`, an interrupted or timed-out call): `<SUBJECT>_UNAVAILABLE`, the only
//!   code of these that is retryable.
//!
//! Most errors that reach a command boundary are text (`e.to_string()` of a store or config error), and an operating-system
//! error keeps its number in that text (`Permission denied (os error 13)`): [`of_text`] reads the number back, so the cause is
//! decided by the error the system gave, not by words a person wrote.

use std::io::ErrorKind;

use crate::cli_error::{CliError, CliErrorExt as _};

/// What the system said about a failed read or write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cause {
    /// Access refused (`EACCES`, `EPERM`, a read-only file system).
    Denied,
    /// The path does not exist.
    NotFound,
    /// A directory where a file was meant, or a file where a directory was meant.
    WrongPath,
    /// An error a repeat can pass (`EIO`, an interrupted, timed-out or would-block call).
    Unavailable,
    /// Anything else, or no operating-system error at all (a parse failure).
    Other,
}

/// The cause an I/O error names.
pub(crate) fn of_io(e: &std::io::Error) -> Cause {
    if e.raw_os_error() == Some(EIO) {
        return Cause::Unavailable;
    }
    match e.kind() {
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => Cause::Denied,
        ErrorKind::NotFound => Cause::NotFound,
        ErrorKind::IsADirectory | ErrorKind::NotADirectory => Cause::WrongPath,
        ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::TimedOut => Cause::Unavailable,
        _ => Cause::Other,
    }
}

/// `EIO` is 5 on every platform canon runs on.
const EIO: i32 = 5;

/// The cause named by the last `(os error N)` of an error's text; [`Cause::Other`] when the text carries none.
pub(crate) fn of_text(text: &str) -> Cause {
    const MARK: &str = "(os error ";
    let Some(at) = text.rfind(MARK) else { return Cause::Other };
    let digits: String = text[at + MARK.len()..].chars().take_while(char::is_ascii_digit).collect();
    match digits.parse::<i32>() {
        Ok(number) => of_io(&std::io::Error::from_raw_os_error(number)),
        Err(_) => Cause::Other,
    }
}

/// A canon.yaml (or a policy file) that cannot be used: `CONFIG_DENIED` when the system refused the read, `CONFIG_INVALID`
/// otherwise (the file is user-edited, so a file that does not parse or whose section is unusable is a value the user gave).
pub(crate) fn config_error(message: impl Into<String>) -> CliError {
    let message = message.into();
    match of_text(&message) {
        Cause::Denied => CliError::refused("CONFIG_DENIED", message).hint("make canon.yaml (and the policy file the message names) readable by this user, then run the command again"),
        _ => CliError::refused("CONFIG_INVALID", message).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again"),
    }
}

/// The prefixes a store error's text starts with when a record it read does not parse or does not agree with its layout
/// (`canon_store::tier::StoreError::{Json, Yaml, Layout, Parquet}`): state canon wrote, broken.
const CORRUPT_PREFIXES: [&str; 4] = ["json: ", "yaml: ", "layout violation: ", "parquet/arrow: "];

/// The repository state that cannot be read: `REPO_DENIED` when the system refused access, `LEDGER_CORRUPT` when a record canon wrote does not
/// parse or does not agree with its layout ([`CORRUPT_PREFIXES`]), `REPO_INVALID` for anything else (a path that is no repository, a tier
/// canon.yaml names that cannot be attached).
pub(crate) fn repo_error(message: impl Into<String>) -> CliError {
    let message = message.into();
    if of_text(&message) == Cause::Denied {
        return CliError::refused("REPO_DENIED", message).hint("make the repository readable by this user, or pass --repo <path> for another root");
    }
    if CORRUPT_PREFIXES.iter().any(|prefix| message.starts_with(prefix) || message.contains(&format!(": {prefix}"))) {
        return CliError::refused("LEDGER_CORRUPT", message).hint("run `canon gate check` to name the broken records, then restore them from git or write them again");
    }
    CliError::refused("REPO_INVALID", message).hint("check that --repo names a directory that holds canon.yaml and that its tiers can be attached, then run the command again")
}

/// A file or directory of the repository that cannot be read (not ledger state): `REPO_DENIED` when the system refused access,
/// `REPO_INVALID` otherwise (the path is missing, or is not what it must be).
pub(crate) fn repo_io_error(message: impl Into<String>) -> CliError {
    let message = message.into();
    match of_text(&message) {
        Cause::Denied => CliError::refused("REPO_DENIED", message).hint("make the repository readable by this user, or pass --repo <path> for another root"),
        _ => CliError::refused("REPO_INVALID", message).hint("check that --repo names a directory that holds canon.yaml, then run the command again"),
    }
}

/// The code of a refused read of a file named by the caller or edited by a person: denied, or invalid (not a file, not text). A
/// file that does not exist is `not_found` when the subject has that code, and invalid otherwise.
pub(crate) fn file_code(e: &std::io::Error, denied: &'static str, invalid: &'static str, not_found: Option<&'static str>) -> &'static str {
    match of_io(e) {
        Cause::Denied => denied,
        Cause::NotFound => not_found.unwrap_or(invalid),
        _ => invalid,
    }
}

/// The code of a brief file that cannot be read: `BRIEF_DENIED`, `BRIEF_NOT_FOUND`, or `BRIEF_INVALID` (a directory stands where the
/// file belongs, or it is not UTF-8 text).
pub(crate) fn brief_code(e: &std::io::Error) -> &'static str {
    file_code(e, "BRIEF_DENIED", "BRIEF_INVALID", Some("BRIEF_NOT_FOUND"))
}

/// The code of a detached signature file that cannot be read: the caller named it with `--signature-file`, so a path that is not there
/// is `NOT_FOUND`, a refused read `DENIED`, and anything else (a directory, text that is not UTF-8) `INVALID`.
pub(crate) fn signature_code(error: &std::io::Error) -> &'static str {
    file_code(error, "STRATEGY_SIGNATURE_DENIED", "STRATEGY_SIGNATURE_INVALID", Some("STRATEGY_SIGNATURE_NOT_FOUND"))
}

/// The code of a scenario file a brief's flow steps point at that cannot be read: `SPEC_ROOT_DENIED` when the system refused the
/// read, `SPEC_ROOT_INVALID` otherwise (a directory, text that is not UTF-8, a path outside the spec roots).
pub(crate) fn spec_root_code(text: &str) -> &'static str {
    if of_text(text) == Cause::Denied {
        "SPEC_ROOT_DENIED"
    } else {
        "SPEC_ROOT_INVALID"
    }
}

/// The code of a refused write whose cause is only known as text: `WRITE_DENIED` when the system refused it, `WRITE_INVALID`
/// otherwise (the target's directory is missing or not a directory).
pub(crate) fn write_code_of_text(text: &str) -> &'static str {
    if of_text(text) == Cause::Denied {
        "WRITE_DENIED"
    } else {
        "WRITE_INVALID"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io(kind: ErrorKind) -> std::io::Error {
        std::io::Error::from(kind)
    }

    /// atip.codestored.02: the cause is read from the error the system gave, whether it is an `io::Error` or the text of one.
    #[test]
    fn atip_codestored_02_a_cause_is_read_from_the_os_error_in_the_error_or_its_text() {
        for (error, cause) in [
            (io(ErrorKind::PermissionDenied), Cause::Denied),
            (io(ErrorKind::NotFound), Cause::NotFound),
            (io(ErrorKind::IsADirectory), Cause::WrongPath),
            (io(ErrorKind::NotADirectory), Cause::WrongPath),
            (std::io::Error::from_raw_os_error(EIO), Cause::Unavailable),
            (io(ErrorKind::TimedOut), Cause::Unavailable),
            (io(ErrorKind::InvalidData), Cause::Other),
        ] {
            assert_eq!(of_io(&error), cause, "{error}");
        }
        assert_eq!(of_text(&format!("cannot read `canon.yaml`: {}", std::io::Error::from_raw_os_error(13))), Cause::Denied);
        assert_eq!(of_text(&format!("cannot read `canon.yaml`: {}", std::io::Error::from_raw_os_error(EIO))), Cause::Unavailable);
        assert_eq!(of_text("canon.yaml is not valid YAML: mapping values are not allowed here"), Cause::Other);
        assert_eq!(of_text("a (os error ) without a number"), Cause::Other);
    }

    /// atip.codestored.02: a denied read is `DENIED`, a broken user file `INVALID`, and broken state canon wrote `CORRUPT`.
    #[test]
    fn atip_codestored_02_the_helpers_give_the_code_of_the_cause() {
        let denied = format!("cannot read `x`: {}", std::io::Error::from_raw_os_error(13));
        assert_eq!(config_error(denied.clone()).code, "CONFIG_DENIED");
        assert_eq!(config_error("not valid YAML").code, "CONFIG_INVALID");
        assert_eq!(repo_error(denied).code, "REPO_DENIED");
        assert_eq!(repo_error(format!("cannot read: {}", std::io::Error::from_raw_os_error(20))).code, "REPO_INVALID");
        assert_eq!(repo_error("json: expected value at line 1 column 1").code, "LEDGER_CORRUPT");
        assert_eq!(repo_error("subject registry: json: expected value at line 1 column 1").code, "LEDGER_CORRUPT");
        assert_eq!(repo_error("local tier (postgres) is not attached (`CANON_PG_DSN` is unset)").code, "REPO_INVALID");
        assert_eq!(file_code(&io(ErrorKind::PermissionDenied), "BRIEF_DENIED", "BRIEF_INVALID", Some("BRIEF_NOT_FOUND")), "BRIEF_DENIED");
        assert_eq!(file_code(&io(ErrorKind::NotFound), "BRIEF_DENIED", "BRIEF_INVALID", Some("BRIEF_NOT_FOUND")), "BRIEF_NOT_FOUND");
        assert_eq!(file_code(&io(ErrorKind::IsADirectory), "BRIEF_DENIED", "BRIEF_INVALID", Some("BRIEF_NOT_FOUND")), "BRIEF_INVALID");
        // A detached signature the caller names: missing is NOT_FOUND, refused DENIED, a directory or text that is not UTF-8 INVALID.
        assert_eq!(signature_code(&io(ErrorKind::NotFound)), "STRATEGY_SIGNATURE_NOT_FOUND");
        assert_eq!(signature_code(&io(ErrorKind::PermissionDenied)), "STRATEGY_SIGNATURE_DENIED");
        assert_eq!(signature_code(&io(ErrorKind::IsADirectory)), "STRATEGY_SIGNATURE_INVALID");
        assert_eq!(signature_code(&io(ErrorKind::InvalidData)), "STRATEGY_SIGNATURE_INVALID");
        // A scenario file of a brief's flow, and a write target known only as text.
        assert_eq!(spec_root_code(&format!("cannot read: {}", std::io::Error::from_raw_os_error(13))), "SPEC_ROOT_DENIED");
        assert_eq!(spec_root_code("stream did not contain valid UTF-8"), "SPEC_ROOT_INVALID");
        assert_eq!(write_code_of_text(&format!("cannot write: {}", std::io::Error::from_raw_os_error(13))), "WRITE_DENIED");
        assert_eq!(write_code_of_text(&format!("cannot write: {}", std::io::Error::from_raw_os_error(20))), "WRITE_INVALID");
        // The one retryable cause is an operating-system error a repeat can pass.
        assert_eq!(of_text(&format!("cannot read: {}", std::io::Error::from_raw_os_error(EIO))), Cause::Unavailable);
    }
}
