//! `canon regime` — the clap tree of the group.

use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum RegimeCommand {
    /// Build a canonical regime key for hook scripts
    Show(RegimeShowArgs),
}

#[derive(Args)]
pub struct RegimeShowArgs {
    /// The <role> segment (canonicalized)
    #[arg(long = "role-segment", value_name = "NAME")]
    pub role_segment: String,
    /// The <repo> segment (canonicalized)
    #[arg(long = "repo-name", value_name = "NAME")]
    pub repo_name: String,
    /// The <area> segment (canonicalized)
    #[arg(long)]
    pub area: String,
    /// The <hash> segment (6-64-char lowercase hex; passed through, never re-hashed)
    #[arg(long)]
    pub hash: String,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
