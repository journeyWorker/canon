//! `canon-cli`'s library surface: the parts of the binary that benefit from
//! integration-test coverage (`tests/skills_install.rs`) without spawning a
//! subprocess. `src/main.rs` is a thin `clap` wrapper around this module.

pub mod artifact_ingest;
pub mod context;
pub mod dashboard;
pub mod demo;
pub mod dispatch;
pub mod divergence;
pub mod fmt;
pub mod gate;
pub mod ingest;
pub mod init;
pub mod inventory;
pub mod inventory_selftest;
pub mod learn;
pub mod plans;
pub mod plugin_sync;
pub mod query;
pub mod report;
pub mod retrieve;
pub mod review;
pub mod scaffold;
pub mod subject;
pub mod selftest;
pub mod skills;
pub mod tier;
pub mod tiers;
