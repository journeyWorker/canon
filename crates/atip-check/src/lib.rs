//! `atip check`: run a built binary and check the atip/1 contract from outside.
//!
//! The checker feeds argv to the binary and reads its stdout, stderr, exit
//! status and the files it changes. It knows nothing of the tool's source or
//! of the framework the tool is built with. Its own output follows the
//! contract at L0, so it is checked by itself.
//!
//! - [`checks`]: the 16 L0 checks (approaches/checker.md 6).
//! - [`exec`]: running the binary in a fresh copy of the fixture state.
//! - [`fixture`]: the fixture file's L0 keys.
//! - [`schema`]: the contract schemas, as validators.
//! - [`snapshot`]: the files under a directory, before and after.
//! - [`report`]: one entry per check, and whether the level is met.
//! - [`cli`], [`output`], [`app`]: the command line, what it prints, and the run.
//!
//! The crate starts inside the canon workspace and moves to the atip
//! repository, so it names no canon type and depends on no canon crate.

#[cfg(not(unix))]
compile_error!("atip check runs on macOS and Linux: it needs a pseudo-terminal and process groups");

pub mod app;
pub mod checks;
pub mod cli;
pub mod exec;
pub mod fixture;
pub mod output;
pub mod report;
pub mod schema;
pub mod snapshot;
