//! `canon adapter` — the clap tree of the group.

use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum AdapterCommand {
    /// Read and validate one provider-neutral response envelope
    Validate(AdapterValidateArgs),
    /// Authorize declared capabilities against repository policy (preflight only)
    Authorize(AdapterAuthorizeArgs),
}

#[derive(Args)]
pub struct AdapterValidateArgs {
    /// JSON response file to validate
    #[arg(long)]
    pub response: PathBuf,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output the normalized core summary as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct AdapterAuthorizeArgs {
    /// JSON response file to validate and authorize
    #[arg(long)]
    pub response: PathBuf,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output authorization result as JSON
    #[arg(long)]
    pub json: bool,
}
