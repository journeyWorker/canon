//! `canon context-pack` — the clap tree of the group.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// The flags of `context-pack add`.
#[derive(Debug, clap::Args)]
pub struct ContextPackAddArgs {
    /// Repository-relative input manifest
    #[arg(long, value_name = "FILE")]
    pub manifest: PathBuf,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output the complete pack manifest as JSON
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Subcommand)]
pub enum ContextPackCommand {
    /// Create an immutable context pack from a repository-local JSON manifest
    Add(ContextPackAddArgs),
    /// Show a context pack after verifying its manifest and content objects
    Show(ContextPackShowArgs),
    /// Verify a context pack's manifest and immutable content objects
    Verify(ContextPackVerifyArgs),
}

#[derive(Debug, Args)]
pub struct ContextPackShowArgs {
    /// Context pack id (sha256:<64 hex digits>)
    #[arg(value_name = "PACK_ID")]
    pub id: String,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output the complete pack manifest as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ContextPackVerifyArgs {
    /// Context pack id (sha256:<64 hex digits>)
    #[arg(value_name = "PACK_ID")]
    pub id: String,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output a JSON verification result
    #[arg(long)]
    pub json: bool,
}
