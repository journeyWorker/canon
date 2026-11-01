//! The validators clap's value parsers used to run: the calendar of a `<TIMESTAMP>` option, the forms of `--surface-ref` and
//! `--introduced-rev`. The parsers take the text now, and the checks run here, in the stateless stage after the declared domains and
//! constraints (ATIP 11.7 step 6: required, domains, constraints, validators), whatever the argument order of the call.

use clap::parser::ValueSource;
use clap::{ArgMatches, Command};

use crate::cli_error::{CliError, CliErrorExt as _};

/// The first of these validators the invocation breaks, with the label of its command; `None` when it breaks none.
pub fn check(root: &Command, matches: &ArgMatches) -> Option<(String, CliError)> {
    let mut command = root;
    let mut leaf = matches;
    let mut words: Vec<&str> = Vec::new();
    while let Some((name, sub)) = leaf.subcommand() {
        command = command.find_subcommand(name)?;
        leaf = sub;
        words.push(name);
    }
    if words.is_empty() {
        return None;
    }
    let label = format!("canon {}", words.join(" "));
    let args = command.get_positionals().chain(command.get_arguments().filter(|arg| !arg.is_positional())).filter(|arg| !atip::project::is_builtin_arg(arg));
    for arg in args {
        let id = arg.get_id().as_str();
        if leaf.try_contains_id(id).ok() != Some(true) || leaf.value_source(id) != Some(ValueSource::CommandLine) {
            continue;
        }
        let Some(long) = arg.get_long() else { continue };
        let judge: fn(&str) -> Result<(), String> = if atip::project::value_name(arg) == "TIMESTAMP" {
            |text| crate::timestamp::parse(text).map(|_| ()).map_err(|error| error.to_string())
        } else if long == "surface-ref" {
            |text| crate::evidence::parse_surface_ref(text).map(|_| ()).map_err(|error| error.to_string())
        } else if long == "introduced-rev" {
            |text| crate::finding::parse_introduced_rev(text).map(|_| ()).map_err(|error| error.to_string())
        } else {
            continue;
        };
        for raw in leaf.try_get_raw(id).ok().flatten().into_iter().flatten() {
            let text = raw.to_string_lossy();
            if let Err(problem) = judge(&text) {
                let field = format!("--{long}");
                let error = CliError::refused("USAGE", format!("invalid value '{text}' for '{field}': {problem}"))
                    .hint(format!("`canon schema {} --json` lists the validators of the command", words.join(" ")))
                    .field(field)
                    .received(text.to_string());
                return Some((label, error));
            }
        }
    }
    None
}
