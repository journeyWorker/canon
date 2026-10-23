//! `canon context-pack` — the clap tree of the group.

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
pub enum ContextPackCommand {
    /// Create an immutable context pack from a repository-local JSON manifest
    New(ContextPackNewArgs),
    /// Show a context pack after verifying its manifest and content objects
    Show(ContextPackShowArgs),
    /// Verify a context pack's manifest and immutable content objects
    Verify(ContextPackVerifyArgs),
}

#[derive(Args)]
pub struct ContextPackShowArgs {
    /// Context pack id (sha256:<64 hex digits>)
    #[arg(value_name = "PACK_ID")]
    pub id: String,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output the complete pack manifest as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ContextPackVerifyArgs {
    /// Context pack id (sha256:<64 hex digits>)
    #[arg(value_name = "PACK_ID")]
    pub id: String,
    /// Repo root (default: nearest ancestor with a canon.yaml)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Output a JSON verification result
    #[arg(long)]
    pub json: bool,
}
