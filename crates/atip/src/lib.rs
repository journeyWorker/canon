//! `atip`: what a command-line tool shares to speak the atip/1 contract.
//!
//! The crate starts inside the canon workspace and moves to the atip
//! repository, so it names no canon type and depends on no canon crate.
//!
//! - [`error`]: [`CliError`], its builder, [`ErrorBody`], [`redact`], the
//!   error-code row type [`CodeInfo`] and the exit values [`EXIT_FAILED`] and
//!   [`EXIT_REFUSED`]. A tool owns its table of rows; the crate owns the shape.
//! - [`argv`]: the JSON-mode flag and the argv test that decides it.
//! - [`envelope`]: a document as one compact line, and printing it.
//! - [`shell`]: quoting a word for a `command` string, splitting that string
//!   back into an argv, and telling a runnable one from one that still has a
//!   `<placeholder>`.

pub mod argv;
pub mod envelope;
pub mod error;
pub mod shell;

pub use argv::{argv_wants_json, json_mode, set_json_mode, wants_json};
pub use envelope::{line, print_line};
pub use error::{find_code, redact, CliError, CodeInfo, ErrorBody, EXIT_FAILED, EXIT_REFUSED};
pub use shell::{has_placeholder, quote, split_argv};
