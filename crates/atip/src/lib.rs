//! `atip`: what a command-line tool shares to speak the atip/1 contract.
//!
//! The crate starts inside the canon workspace and moves to the atip
//! repository, so it names no canon type and depends on no canon crate.
//!
//! - [`error`]: [`CliError`], its builder, [`ErrorBody`], [`redact`], the
//!   error-code row type [`CodeInfo`] and the exit values [`EXIT_FAILED`] and
//!   [`EXIT_REFUSED`]. A tool owns its table of rows; the crate owns the shape.
//! - [`schema`]: the contract's envelope, error and registry-entry JSON
//!   Schemas, vendored verbatim and pinned by digest, and the validators that
//!   use them ([`envelope_problems`], [`registry_entry_problems`]).
//! - [`doc`]: the documents `schema --json` answers with (tree, group, command,
//!   option, operand, output, input), typed and deriving `JsonSchema`.
//! - [`fields`]: field paths and projection (spec 6.10): the path grammar, the
//!   projection of a record, the `fields` declaration of an output with the checks
//!   spec 10.13 puts on it, and the validation of a projected answer against the
//!   output's schema with `required` reduced.
//! - [`project`]: the generic projection of a clap tree onto those documents:
//!   the tree and its paths, `valueType`, typed defaults, the `--input`
//!   fragment schema, the schemas a command document carries, and the help
//!   line that points at `schema`. It guesses nothing: a custom value parser
//!   without a table entry fails.
//! - [`argv`]: the JSON-mode flag and the argv test that decides it.
//! - [`envelope`]: a document as one compact line, and printing it.
//! - [`minor`]: [`Minor`], the minors of atip/1 the crate knows (1.0, 1.1, 1.2), and
//!   [`CONTRACT_VERSION`], the newest one the crate implements. A tool's own
//!   declaration of its minor is its own constant, not this one.
//! - [`reserved`]: the table of the thirteen reserved error codes (name, exit,
//!   retryability, the minor that introduced it) and the check that a tool
//!   registers one as the contract reserves it.
//! - [`shell`]: quoting a word for a `command` string, splitting that string
//!   back into an argv, and telling a runnable one from one that still has a
//!   `<placeholder>`.

pub mod argv;
pub mod constraint;
pub mod doc;
pub mod domain;
pub mod envelope;
pub mod error;
pub mod fields;
pub mod minor;
pub mod project;
pub mod reserved;
pub mod schema;
pub mod shell;

pub use argv::{argv_wants_json, json_mode, set_json_mode, wants_json};
pub use envelope::{line, print_line};
pub use error::{count_with_exit, find_code, find_warning, reason_of_code, reasons_with_exit, redact, with_row, CliError, CodeInfo, ErrorBody, ReasonInfo, WarningInfo, EXIT_FAILED, EXIT_REFUSED};
pub use minor::{Minor, UnknownMinor};
pub use project::{flag_only_options, flag_only_problems};
pub use reserved::{registration_problem, reserved_code, ReservedCode, RESERVED_CODES};
pub use schema::{envelope_problems, error_problems, registry_entry_problems, schema_doc_problems, warning_entry_problems, BATCH_REPORT_SCHEMA, CONTRACT, CONTRACT_VERSION, ENVELOPE_SCHEMA, ERROR_SCHEMA, INPUT_DOCUMENT_SCHEMA, REGISTRY_ENTRY_SCHEMA, SCHEMA_DOC_SCHEMA, STREAM_LINE_SCHEMA, WARNING_ENTRY_SCHEMA};
pub use shell::{has_placeholder, quote, split_argv};
