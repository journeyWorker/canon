//! `canon adapter` — the clap tree of the group.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// The flags of `context-pack new`.
#[derive(clap::Args)]
pub struct ContextPackNewArgs {
    /// Repository-relative input manifest
    #[arg(long)]
    pub manifest: PathBuf,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output the complete pack manifest as JSON
    #[arg(long)]
    pub json: bool,
}

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
    /// Optional repo root for context-pack join verification
    #[arg(long)]
    pub repo: Option<PathBuf>,
    /// Output the normalized core summary as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct AdapterAuthorizeArgs {
    /// JSON response file to validate and authorize
    #[arg(long)]
    pub response: PathBuf,
    /// Repository root containing `.canon/policy.yaml`
    #[arg(long)]
    pub repo: PathBuf,
    /// Output authorization result as JSON
    #[arg(long)]
    pub json: bool,
}
