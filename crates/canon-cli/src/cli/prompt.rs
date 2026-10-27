//! `canon prompt` — the clap tree of the group.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// The flags of `prompt add`.
#[derive(Debug, clap::Args)]
pub struct PromptAddArgs {
    /// The bundle to register, `NAME@VERSION`
    #[arg(value_name = "NAME@VERSION", value_parser = crate::dispatch::parse_prompt_bundle)]
    pub bundle: crate::context_pack::PromptBundleSelection,
    /// Repository-relative input manifest
    #[arg(long, value_name = "FILE")]
    pub manifest: PathBuf,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output the complete bundle manifest as JSON
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Subcommand)]
pub enum PromptCommand {
    /// Add a prompt bundle from a repository-local JSON manifest
    Add(PromptAddArgs),
    /// Select a registered prompt bundle version
    Show(PromptShowArgs),
}

#[derive(Debug, Args)]
pub struct PromptShowArgs {
    /// The bundle to show, `NAME@VERSION`
    #[arg(value_name = "NAME@VERSION", value_parser = crate::dispatch::parse_prompt_bundle)]
    pub bundle: crate::context_pack::PromptBundleSelection,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output the complete bundle manifest as JSON
    #[arg(long)]
    pub json: bool,
}
