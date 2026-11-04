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

/// The cause an I/O error names: the kind it carries first, then the raw operating-system number.
pub(crate) fn of_io(e: &std::io::Error) -> Cause {
    cause_of(e.kind(), e.raw_os_error())
}

/// [`of_io`] on what an error carries. The kind decides first, because a raw number means something else on another platform: `5` is `EIO`
/// on a unix system and `ERROR_ACCESS_DENIED` on Windows, which reports it as [`ErrorKind::PermissionDenied`]. Only a unix error with no
/// kind of its own (`Uncategorized`) is read by its number.
fn cause_of(kind: ErrorKind, raw: Option<i32>) -> Cause {
    match kind {
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => Cause::Denied,
        ErrorKind::NotFound => Cause::NotFound,
        ErrorKind::IsADirectory | ErrorKind::NotADirectory => Cause::WrongPath,
        ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::TimedOut => Cause::Unavailable,
        _ => raw_cause(raw),
    }
}

/// The cause of a raw operating-system number whose kind named none: `EIO` on a unix system, nothing elsewhere.
#[cfg(unix)]
fn raw_cause(raw: Option<i32>) -> Cause {
    if raw == Some(EIO) {
        Cause::Unavailable
    } else {
        Cause::Other
    }
}

#[cfg(not(unix))]
fn raw_cause(_raw: Option<i32>) -> Cause {
    Cause::Other
}

/// `EIO` is 5 on a unix system; on Windows 5 is `ERROR_ACCESS_DENIED`.
#[cfg_attr(not(unix), allow(dead_code))]
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

/// A canon.yaml (or a policy file) that cannot be used: `CONFIG_DENIED` when the system refused the read, `CONFIG_UNAVAILABLE` when
/// the system failed it in a way a repeat can pass, `CONFIG_INVALID` otherwise (the file is user-edited, so a file that does not parse
/// or whose section is unusable is a value the user gave).
pub(crate) fn config_error(message: impl Into<String>) -> CliError {
    let message = message.into();
    match of_text(&message) {
        Cause::Denied => CliError::refused("CONFIG_DENIED", message).hint("make canon.yaml (and the policy file the message names) readable by this user, then run the command again"),
        Cause::Unavailable => CliError::refused("CONFIG_UNAVAILABLE", message).hint("the system failed the read of canon.yaml (or the policy file the message names) in a way a repeat can pass: wait a moment, then run the command again"),
        _ => CliError::refused("CONFIG_INVALID", message).reason("CANON_YAML").hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again"),
    }
}

/// What the text of a `canon_store::tier::StoreError` says when the fault is in the tiers canon.yaml declares, not in the repository on
/// disk: a record kind with no `routing:` entry (`UnroutedKind`), a tier policy that does not hold (`Policy`), a rung with no `tiers.<rung>`
/// entry (`TierUnavailable` with no backend). A user edits those sections, so the fault is the config's.
const TIERS_FAULTS: [&str; 2] = ["no `TierPolicy.routing` entry (canon.yaml)", " tier is not configured ("];

/// The repository state that cannot be read: `REPO_DENIED` when the system refused access, `REPO_UNAVAILABLE` when the system failed the
/// read in a way a repeat can pass, `CONFIG_INVALID` (reason `TIERS`) when the fault is in the tiers canon.yaml declares
/// ([`TIERS_FAULTS`]), `REPO_INVALID` for anything else (a path that is no repository, a tier canon.yaml names that cannot be attached).
pub(crate) fn repo_error(message: impl Into<String>) -> CliError {
    let message = message.into();
    match of_text(&message) {
        Cause::Denied => return CliError::refused("REPO_DENIED", message).hint("make the repository readable by this user, or pass --repo <path> for another root"),
        Cause::Unavailable => return CliError::refused("REPO_UNAVAILABLE", message).hint(REPO_UNAVAILABLE_HINT),
        _ => {}
    }
    if message.starts_with("policy: ") || TIERS_FAULTS.iter().any(|fault| message.contains(fault)) {
        return CliError::refused("CONFIG_INVALID", message).reason("TIERS").hint("fix the `tiers:`, `routing:` and `aging:` sections of canon.yaml (every record kind needs a `routing:` entry; `canon init --check` names the problem), then run the command again");
    }
    CliError::refused("REPO_INVALID", message).hint("check that --repo names a directory that holds canon.yaml and that its tiers can be attached, then run the command again")
}

/// The next step of a repository read the system failed in a way a repeat can pass.
const REPO_UNAVAILABLE_HINT: &str = "the system failed the read in a way a repeat can pass: wait a moment, then run the command again";

/// A file or directory of the repository that cannot be read (not ledger state): `REPO_DENIED` when the system refused access,
/// `REPO_UNAVAILABLE` when the system failed the read in a way a repeat can pass, `REPO_INVALID` otherwise (the path is missing, or is not
/// what it must be).
pub(crate) fn repo_io_error(message: impl Into<String>) -> CliError {
    let message = message.into();
    match of_text(&message) {
        Cause::Denied => CliError::refused("REPO_DENIED", message).hint("make the repository readable by this user, or pass --repo <path> for another root"),
        Cause::Unavailable => CliError::refused("REPO_UNAVAILABLE", message).hint(REPO_UNAVAILABLE_HINT),
        _ => CliError::refused("REPO_INVALID", message).hint("check that --repo names a directory that holds canon.yaml, then run the command again"),
    }
}

/// A read that failed before any work was done: the repository's retryable `REPO_UNAVAILABLE` when the system failed it in a way a repeat can
/// pass, and `None` for any other failure, which the caller answers with its own `_FAILED` or `_INCOMPLETE` code. A command that failed
/// before it began changes nothing, so it is a refusal (exit 2, retryable) and not a failure (spec 8.3); only a failure after work started is
/// the command's `_FAILED` or `_INCOMPLETE`.
pub(crate) fn unavailable_before_work(message: &str) -> Option<CliError> {
    (of_text(message) == Cause::Unavailable).then(|| CliError::refused("REPO_UNAVAILABLE", message).hint(REPO_UNAVAILABLE_HINT))
}

/// The codes a subject has for a file it cannot read: the one for each cause of [`Cause`] it tells apart.
pub(crate) struct FileCodes {
    pub denied: &'static str,
    pub invalid: &'static str,
    /// The code for a path that is not there; `invalid` when the subject has none.
    pub not_found: Option<&'static str>,
    /// The code for an operating-system error a repeat can pass (`EIO`): the subject's `_UNAVAILABLE`.
    pub unavailable: &'static str,
}

impl FileCodes {
    /// The code of a refused read of a file named by the caller or edited by a person: denied, unavailable (the system failed in a
    /// way a repeat can pass), or invalid (not a file, not text). A file that does not exist is `not_found` when the subject has that
    /// code, and invalid otherwise.
    pub(crate) fn of(&self, e: &std::io::Error) -> &'static str {
        match of_io(e) {
            Cause::Denied => self.denied,
            Cause::NotFound => self.not_found.unwrap_or(self.invalid),
            Cause::Unavailable => self.unavailable,
            Cause::WrongPath | Cause::Other => self.invalid,
        }
    }
}

const BRIEF_FILE: FileCodes = FileCodes { denied: "BRIEF_DENIED", invalid: "BRIEF_INVALID", not_found: Some("BRIEF_NOT_FOUND"), unavailable: "BRIEF_UNAVAILABLE" };
const SIGNATURE_FILE: FileCodes = FileCodes { denied: "STRATEGY_SIGNATURE_DENIED", invalid: "STRATEGY_SIGNATURE_INVALID", not_found: Some("STRATEGY_SIGNATURE_NOT_FOUND"), unavailable: "STRATEGY_SIGNATURE_UNAVAILABLE" };

/// The code of a brief file that cannot be read: `BRIEF_DENIED`, `BRIEF_NOT_FOUND`, `BRIEF_UNAVAILABLE` (the system failed the read in a
/// way a repeat can pass), or `BRIEF_INVALID` (a directory stands where the file belongs, or it is not UTF-8 text).
pub(crate) fn brief_code(e: &std::io::Error) -> &'static str {
    BRIEF_FILE.of(e)
}

/// A `BRIEF_INVALID` that came from reading brief.md says why with the reason `UNREADABLE` (a directory stands where the file belongs, or it
/// is not UTF-8 text); the codes of the other causes of a read have one situation each and no reason. The other reasons of the code
/// (`UNAPPROVABLE`, `BLOCKERS`) are about the text that was read.
pub(crate) fn brief_unreadable(error: CliError) -> CliError {
    if error.code == "BRIEF_INVALID" {
        error.reason("UNREADABLE")
    } else {
        error
    }
}

/// The error of a brief.md the system could not read: [`brief_code`] of the cause, with [`brief_unreadable`]'s reason.
pub(crate) fn brief_read_error(e: &std::io::Error, message: impl Into<String>) -> CliError {
    brief_unreadable(CliError::failed(brief_code(e), message).hint("check that the change's brief.md is a UTF-8 text file the user can read, then run the command again"))
}

/// The code of a detached signature file that cannot be read: the caller named it with `--signature-file`, so a path that is not there
/// is `NOT_FOUND`, a refused read `DENIED`, an error a repeat can pass `UNAVAILABLE`, and anything else (a directory, text that is
/// not UTF-8) `INVALID`.
pub(crate) fn signature_code(error: &std::io::Error) -> &'static str {
    SIGNATURE_FILE.of(error)
}

const PLAN_CORPUS_FILE: FileCodes = FileCodes { denied: "PLAN_CORPUS_DENIED", invalid: "PLAN_CORPUS_INVALID", not_found: None, unavailable: "PLAN_CORPUS_UNAVAILABLE" };
const HOOKS_FILE: FileCodes = FileCodes { denied: "HOOKS_FILE_DENIED", invalid: "HOOKS_FILE_CONFLICT", not_found: None, unavailable: "HOOKS_FILE_UNAVAILABLE" };

/// The code of a plan document that cannot be read: `PLAN_CORPUS_DENIED`, `PLAN_CORPUS_UNAVAILABLE` (the system failed the read in a way a
/// repeat can pass), or `PLAN_CORPUS_INVALID` (not a file, not UTF-8 text, or missing).
pub(crate) fn plan_corpus_code(e: &std::io::Error) -> &'static str {
    PLAN_CORPUS_FILE.of(e)
}

/// [`plan_corpus_code`] for a failure known only as the text of the error.
pub(crate) fn plan_corpus_code_of_text(text: &str) -> &'static str {
    match of_text(text) {
        Cause::Denied => PLAN_CORPUS_FILE.denied,
        Cause::Unavailable => PLAN_CORPUS_FILE.unavailable,
        _ => PLAN_CORPUS_FILE.invalid,
    }
}

/// The code of a hook file `gate install-hooks` cannot read or create: `HOOKS_FILE_DENIED`, `HOOKS_FILE_UNAVAILABLE` (the system failed
/// the call in a way a repeat can pass), or `HOOKS_FILE_CONFLICT` (a directory where the file belongs, text that does not parse).
pub(crate) fn hooks_file_code(e: &std::io::Error) -> &'static str {
    HOOKS_FILE.of(e)
}

const POLICY_FILE: FileCodes = FileCodes { denied: "POLICY_DENIED", invalid: "POLICY_INVALID", not_found: None, unavailable: "POLICY_UNAVAILABLE" };

/// The code of the policy file `policy approve` cannot read: `POLICY_DENIED`, `POLICY_UNAVAILABLE` (the system failed the read in a way a
/// repeat can pass), or `POLICY_INVALID` (a directory where the file belongs, text that is not UTF-8).
pub(crate) fn policy_code(e: &std::io::Error) -> &'static str {
    POLICY_FILE.of(e)
}

/// The code of a policy approval that cannot be read, known only as text: `POLICY_APPROVAL_DENIED` when the system refused the read,
/// `POLICY_APPROVAL_UNAVAILABLE` when it failed in a way a repeat can pass, and `POLICY_APPROVAL_CORRUPT` otherwise (the file canon wrote
/// does not parse).
pub(crate) fn policy_approval_code_of_text(text: &str) -> &'static str {
    match of_text(text) {
        Cause::Denied => "POLICY_APPROVAL_DENIED",
        Cause::Unavailable => "POLICY_APPROVAL_UNAVAILABLE",
        _ => "POLICY_APPROVAL_CORRUPT",
    }
}

/// The code of a `migrate` preview that cannot use its scratch directory, known only as text: `MIGRATION_DENIED` when the system refused
/// the call, `MIGRATION_UNAVAILABLE` when it failed in a way a repeat can pass, and `MIGRATION_INVALID` otherwise (the scratch path is no
/// directory, a record the preview reads cannot be used).
pub(crate) fn migration_code_of_text(text: &str) -> &'static str {
    match of_text(text) {
        Cause::Denied => "MIGRATION_DENIED",
        Cause::Unavailable => "MIGRATION_UNAVAILABLE",
        _ => "MIGRATION_INVALID",
    }
}

const SPEC_ROOT_DIR: FileCodes = FileCodes { denied: "SPEC_ROOT_DENIED", invalid: "SPEC_ROOT_INVALID", not_found: None, unavailable: "SPEC_ROOT_UNAVAILABLE" };

/// The code of a spec root directory of canon.yaml that cannot be read (a brief's flow steps need its scenarios): `SPEC_ROOT_DENIED` when
/// the system refuses it, `SPEC_ROOT_UNAVAILABLE` when it failed in a way a repeat can pass, `SPEC_ROOT_INVALID` otherwise (a file stands
/// where the directory belongs). A root that is not there holds no scenarios and is no refusal.
pub(crate) fn spec_root_dir_code(e: &std::io::Error) -> &'static str {
    SPEC_ROOT_DIR.of(e)
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

    /// The classification of the helpers (the real calls that reach each code are the tests of `tests/atip_codestored.rs`): a denied read is
    /// `DENIED`, a broken user file `INVALID`, a path that is the wrong kind `INVALID`, and `UNAVAILABLE` only an error a repeat can pass.
    #[test]
    fn atip_codestored_02_the_helpers_give_the_code_of_the_cause() {
        let denied = format!("cannot read `x`: {}", std::io::Error::from_raw_os_error(13));
        assert_eq!(config_error(denied.clone()).code, "CONFIG_DENIED");
        assert_eq!(config_error("not valid YAML").code, "CONFIG_INVALID");
        assert_eq!(repo_error(denied).code, "REPO_DENIED");
        assert_eq!(repo_error(format!("cannot read: {}", std::io::Error::from_raw_os_error(20))).code, "REPO_INVALID");
        // The tiers canon.yaml declares are the config's, whatever the repository on disk looks like.
        let unrouted = "Change: no `TierPolicy.routing` entry (canon.yaml) — every write/read must resolve through the declarative policy, never a hardcoded default";
        let unrouted = repo_error(unrouted);
        assert_eq!((unrouted.code, unrouted.reason_name()), ("CONFIG_INVALID", Some("TIERS")));
        assert_eq!(repo_error("policy: tiers.hot is missing").code, "CONFIG_INVALID");
        assert_eq!(repo_error("local tier (postgres) is not attached (`CANON_PG_DSN` is unset)").code, "REPO_INVALID");
        assert_eq!(repo_error("json: expected value at line 1 column 1").code, "REPO_INVALID");
        assert_eq!(brief_code(&io(ErrorKind::PermissionDenied)), "BRIEF_DENIED");
        assert_eq!(brief_code(&io(ErrorKind::NotFound)), "BRIEF_NOT_FOUND");
        assert_eq!(brief_code(&io(ErrorKind::IsADirectory)), "BRIEF_INVALID");
        // A detached signature the caller names: missing is NOT_FOUND, refused DENIED, a directory or text that is not UTF-8 INVALID.
        assert_eq!(signature_code(&io(ErrorKind::NotFound)), "STRATEGY_SIGNATURE_NOT_FOUND");
        assert_eq!(signature_code(&io(ErrorKind::PermissionDenied)), "STRATEGY_SIGNATURE_DENIED");
        assert_eq!(signature_code(&io(ErrorKind::IsADirectory)), "STRATEGY_SIGNATURE_INVALID");
        assert_eq!(signature_code(&io(ErrorKind::InvalidData)), "STRATEGY_SIGNATURE_INVALID");
        // A spec root directory, and a write target known only as text.
        assert_eq!(spec_root_dir_code(&std::io::Error::from_raw_os_error(13)), "SPEC_ROOT_DENIED");
        assert_eq!(spec_root_dir_code(&io(ErrorKind::NotADirectory)), "SPEC_ROOT_INVALID");
        assert_eq!(spec_root_dir_code(&io(ErrorKind::InvalidData)), "SPEC_ROOT_INVALID");
        assert_eq!(write_code_of_text(&format!("cannot write: {}", std::io::Error::from_raw_os_error(13))), "WRITE_DENIED");
        assert_eq!(write_code_of_text(&format!("cannot write: {}", std::io::Error::from_raw_os_error(20))), "WRITE_INVALID");
        // The one retryable cause is an operating-system error a repeat can pass.
        assert_eq!(of_text(&format!("cannot read: {}", std::io::Error::from_raw_os_error(EIO))), Cause::Unavailable);
    }

    /// An operating-system error a repeat can pass (`EIO`), injected into each helper as the error the system gave.
    fn eio() -> std::io::Error {
        std::io::Error::from_raw_os_error(EIO)
    }

    /// atip.codestored.11: a read the system fails in a way a repeat can pass is the subject's `_UNAVAILABLE`, the one code of these that is
    /// retryable (exit 2); a call the same helper answers for another cause keeps its code.
    #[test]
    fn atip_codestored_11_a_read_the_system_fails_for_a_repeat_to_pass_is_unavailable() {
        let text = format!("cannot read `x`: {}", eio());
        for (error, code) in [
            (config_error(text.clone()), "CONFIG_UNAVAILABLE"),
            (repo_error(text.clone()), "REPO_UNAVAILABLE"),
            (repo_io_error(text.clone()), "REPO_UNAVAILABLE"),
        ] {
            assert_eq!((error.code, error.exit, error.retryable), (code, 2, true), "{error:?}");
        }
        assert_eq!(brief_code(&eio()), "BRIEF_UNAVAILABLE");
        assert_eq!(signature_code(&eio()), "STRATEGY_SIGNATURE_UNAVAILABLE");
        assert_eq!(spec_root_dir_code(&eio()), "SPEC_ROOT_UNAVAILABLE");
        // Interrupted and timed-out calls are the same cause as EIO; a directory in the file's place is still the caller's mistake.
        assert_eq!(brief_code(&io(ErrorKind::TimedOut)), "BRIEF_UNAVAILABLE");
        assert_eq!(brief_code(&io(ErrorKind::Interrupted)), "BRIEF_UNAVAILABLE");
        assert_eq!(brief_code(&io(ErrorKind::IsADirectory)), "BRIEF_INVALID");
        // EIO inside a message is read before the text is classified; text that only looks like a store error says nothing about the cause.
        assert_eq!(repo_error(format!("json: {text}")).code, "REPO_UNAVAILABLE");
        // Only an error a repeat can pass is UNAVAILABLE: a directory, a missing path or a parse failure never is.
        for other in [io(ErrorKind::IsADirectory), io(ErrorKind::NotFound), io(ErrorKind::InvalidData), io(ErrorKind::AlreadyExists)] {
            assert_ne!(of_io(&other), Cause::Unavailable, "{other}");
            for code in [brief_code(&other), signature_code(&other), plan_corpus_code(&other), hooks_file_code(&other), policy_code(&other), spec_root_dir_code(&other)] {
                assert!(!code.ends_with("_UNAVAILABLE"), "{other}: {code}");
            }
        }
        // A plan document and a hook file, as the error and as its text.
        assert_eq!(plan_corpus_code(&eio()), "PLAN_CORPUS_UNAVAILABLE");
        assert_eq!(plan_corpus_code_of_text(&text), "PLAN_CORPUS_UNAVAILABLE");
        assert_eq!(plan_corpus_code(&std::io::Error::from_raw_os_error(13)), "PLAN_CORPUS_DENIED");
        assert_eq!(plan_corpus_code(&io(ErrorKind::IsADirectory)), "PLAN_CORPUS_INVALID");
        assert_eq!(hooks_file_code(&eio()), "HOOKS_FILE_UNAVAILABLE");
        assert_eq!(hooks_file_code(&std::io::Error::from_raw_os_error(13)), "HOOKS_FILE_DENIED");
        assert_eq!(hooks_file_code(&io(ErrorKind::IsADirectory)), "HOOKS_FILE_CONFLICT");
        // The policy, its approval and the migrate preview.
        assert_eq!(policy_code(&eio()), "POLICY_UNAVAILABLE");
        assert_eq!(policy_code(&std::io::Error::from_raw_os_error(13)), "POLICY_DENIED");
        assert_eq!(policy_code(&io(ErrorKind::IsADirectory)), "POLICY_INVALID");
        assert_eq!(policy_approval_code_of_text(&text), "POLICY_APPROVAL_UNAVAILABLE");
        assert_eq!(policy_approval_code_of_text("expected value at line 1 column 1"), "POLICY_APPROVAL_CORRUPT");
        assert_eq!(migration_code_of_text(&text), "MIGRATION_UNAVAILABLE");
        assert_eq!(migration_code_of_text("could not create /tmp/x: Permission denied (os error 13)"), "MIGRATION_DENIED");
    }

    /// atip.reasons.10: the kind of an error decides before its raw number, because `5` is `EIO` on a unix system and `ERROR_ACCESS_DENIED` on
    /// Windows, which reports it as `PermissionDenied`: an error of that kind with raw `5` is a refused read, never one a repeat can pass; the raw
    /// `EIO` is read only for an error whose kind names no cause, and only on a unix system.
    #[test]
    fn atip_reasons_10_the_kind_of_an_error_is_read_before_its_raw_number() {
        assert_eq!(cause_of(ErrorKind::PermissionDenied, Some(EIO)), Cause::Denied, "Windows: 5 is ERROR_ACCESS_DENIED");
        assert_eq!(cause_of(ErrorKind::ReadOnlyFilesystem, Some(EIO)), Cause::Denied);
        assert_eq!(cause_of(ErrorKind::NotFound, Some(EIO)), Cause::NotFound);
        assert_eq!(cause_of(ErrorKind::IsADirectory, Some(EIO)), Cause::WrongPath);
        assert_eq!(cause_of(ErrorKind::TimedOut, None), Cause::Unavailable, "a kind a repeat can pass needs no number");
        assert_eq!(cause_of(ErrorKind::PermissionDenied, None), Cause::Denied);
        // An error with no kind of its own is `Other`, and its raw number is read only where `5` is `EIO`.
        #[cfg(unix)]
        assert_eq!(cause_of(ErrorKind::Other, Some(EIO)), Cause::Unavailable);
        #[cfg(unix)]
        assert_eq!(of_io(&std::io::Error::from_raw_os_error(EIO)), Cause::Unavailable);
        #[cfg(unix)]
        assert_eq!(of_text("cannot read x: Input/output error (os error 5)"), Cause::Unavailable);
        #[cfg(not(unix))]
        assert_eq!(cause_of(ErrorKind::Other, Some(EIO)), Cause::Other, "no raw EIO outside a unix system");
        assert_eq!(cause_of(ErrorKind::Other, None), Cause::Other);
        assert_eq!(cause_of(ErrorKind::InvalidData, Some(22)), Cause::Other);
    }

    /// atip.reasons.11: a read the system failed before any work was done is `REPO_UNAVAILABLE` (a refusal, exit 2, retryable); any other failure has no
    /// answer here and is the caller's own `_FAILED` or `_INCOMPLETE`.
    #[test]
    fn atip_reasons_11_only_a_read_the_system_failed_for_a_repeat_to_pass_is_unavailable_before_work() {
        let error = unavailable_before_work(&format!("io: {}", eio())).expect("an EIO before any work is a refusal");
        assert_eq!((error.code, error.exit, error.retryable), ("REPO_UNAVAILABLE", 2, true));
        for other in ["io: Permission denied (os error 13)", "json: expected value at line 1 column 1", "no such record"] {
            assert!(unavailable_before_work(other).is_none(), "{other}");
        }
    }

    /// atip.reasons.05: canon.yaml that cannot be used is `CONFIG_INVALID` with the reason `CANON_YAML`; a refused read and a read a repeat can pass
    /// have their own codes and no reason.
    #[test]
    fn atip_reasons_05_a_canon_yaml_that_cannot_be_used_is_config_invalid_with_its_reason() {
        let invalid = config_error("canon.yaml is not valid YAML: mapping values are not allowed here");
        assert_eq!((invalid.code, invalid.reason_name(), invalid.exit, invalid.retryable), ("CONFIG_INVALID", Some("CANON_YAML"), 2, false));
        let denied = config_error(format!("cannot read `canon.yaml`: {}", std::io::Error::from_raw_os_error(13)));
        assert_eq!((denied.code, denied.reason_name()), ("CONFIG_DENIED", None));
        let unavailable = config_error(format!("cannot read `canon.yaml`: {}", eio()));
        assert_eq!((unavailable.code, unavailable.reason_name(), unavailable.retryable), ("CONFIG_UNAVAILABLE", None, true));
        // A brief that cannot be read as text says why; the other causes of a read keep their codes and have no reason.
        let not_text = brief_read_error(&io(ErrorKind::InvalidData), "cannot read `brief.md`");
        assert_eq!((not_text.code, not_text.reason_name()), ("BRIEF_INVALID", Some("UNREADABLE")));
        let denied = brief_read_error(&io(ErrorKind::PermissionDenied), "cannot read `brief.md`");
        assert_eq!((denied.code, denied.reason_name()), ("BRIEF_DENIED", None));
    }
}
