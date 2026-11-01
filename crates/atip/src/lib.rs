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
//! - [`project`]: the generic projection of a clap tree onto those documents:
//!   the tree and its paths, `valueType`, typed defaults, the `--input`
//!   fragment schema, the schemas a command document carries, and the help
//!   line that points at `schema`. It guesses nothing: a custom value parser
//!   without a table entry fails.
//! - [`argv`]: the JSON-mode flag and the argv test that decides it.
//! - [`envelope`]: a document as one compact line, and printing it.
//! - [`shell`]: quoting a word for a `command` string, splitting that string
//!   back into an argv, and telling a runnable one from one that still has a
//!   `<placeholder>`.

pub mod argv;
pub mod doc;
pub mod envelope;
pub mod error;
pub mod project;
pub mod schema;
pub mod shell;

pub use argv::{argv_wants_json, json_mode, set_json_mode, wants_json};
pub use envelope::{line, print_line};
pub use error::{find_code, find_warning, redact, CliError, CodeInfo, ErrorBody, ReasonInfo, WarningInfo, EXIT_FAILED, EXIT_REFUSED};
pub use schema::{envelope_problems, error_problems, registry_entry_problems, schema_doc_problems, warning_entry_problems, BATCH_REPORT_SCHEMA, CONTRACT, CONTRACT_VERSION, ENVELOPE_SCHEMA, ERROR_SCHEMA, INPUT_DOCUMENT_SCHEMA, REGISTRY_ENTRY_SCHEMA, SCHEMA_DOC_SCHEMA, STREAM_LINE_SCHEMA, WARNING_ENTRY_SCHEMA};
pub use shell::{has_placeholder, quote, split_argv};
