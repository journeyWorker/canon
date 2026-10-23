//! `canon completion <SHELL>`: the shell completion script, generated from the
//! clap tree at run time (`clap_complete`), so it completes exactly what the
//! parser accepts.

use schemars::JsonSchema;
use serde::Serialize;

use crate::cli::CompletionShell;
use crate::reports::Report;

/// `--json` of `canon completion`: the script in an envelope.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompletionReport {
    /// `bash`, `zsh` or `fish`.
    pub shell: &'static str,
    /// The script, as `canon completion <shell>` prints it.
    pub script: String,
}

impl Report for CompletionReport {}

/// The completion script of `shell` and the shell's name.
pub fn script(shell: CompletionShell) -> CompletionReport {
    let (target, name) = match shell {
        CompletionShell::Bash => (clap_complete::Shell::Bash, "bash"),
        CompletionShell::Zsh => (clap_complete::Shell::Zsh, "zsh"),
        CompletionShell::Fish => (clap_complete::Shell::Fish, "fish"),
    };
    let mut script = Vec::new();
    clap_complete::generate(target, &mut crate::cli::command(), "canon", &mut script);
    CompletionReport { shell: name, script: String::from_utf8_lossy(&script).into_owned() }
}
