//! `canon task` — the clap tree of the group.

use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum TaskCommand {
    /// Flip one task checkbox, gated on real evidence (fails closed)
    Done(TaskDoneArgs),
}

#[derive(Args)]
pub struct TaskDoneArgs {
    /// The task id (<change_id>#<n>)
    #[arg(value_name = "TASK_ID")]
    pub task_id: String,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
