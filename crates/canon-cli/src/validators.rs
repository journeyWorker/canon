//! The validators of the command tree (ATIP 10.8.3, 11.7 step 6): the stateless checks of an
//! invocation that the contract declares in `canon schema <command>` as `validators`. They run for
//! every invocation, after clap parsed it and BEFORE any precondition (`CONFIG_MISSING`,
//! `OLD_LAYOUT`) or state read, so a call they refuse is refused the same way in an empty
//! directory. The checks themselves sit next to the command that owns them (`check_*`, each taking
//! the parsed arguments and nothing else: no repository, no environment that is not an argument,
//! no file); this module only runs them, in one place, in front of the handlers, which do not call
//! them again.

use std::process::ExitCode;

use crate::cli::{Cli, Command};
use crate::cli::brief::BriefCommand;
use crate::cli::DivergenceCliCommand;
use crate::cli::evidence::EvidenceCommand;
use crate::cli::finding::FindingCommand;
use crate::cli::policy::PolicyCommand;
use crate::cli_error::{CliError, CliErrorExt as _};

/// Parses `args` in the order ATIP 11.7 step 6 fixes for a tool that declares 1.2, and answers the first failure, so that the same call
/// gets the same code and `field` from every tool: a failure to parse (an unknown option or command word, a missing option value, a value
/// that is no integer or not one of its `possibleValues`; the earliest on the line) → a missing `required` argument (operands in order, then
/// options in declaration order) → the declared domains → the declared constraints, each in the order of the document. A value outside a
/// domain is read as text by the first parse ([`crate::arg_rules::relaxed`]), so it cannot answer ahead of the failures that come before
/// it; the real command tree then parses what that let through, for the typed arguments the handlers read. The validators run later,
/// on the typed arguments ([`check`]).
///
/// `command` is the built command tree (`crate::cli::command()` after `build`). `Err` is the exit code, the refusal already printed.
pub fn parse(command: &clap::Command, args: Vec<std::ffi::OsString>) -> Result<clap::ArgMatches, ExitCode> {
    let relaxed = crate::arg_rules::relaxed::tree(crate::cli::command());
    let first = match relaxed.clone().try_get_matches_from(args.clone()) {
        Ok(matches) => matches,
        Err(error) => {
            // `--help --json` and `--version --json` answer a refusal, after the path is resolved (spec 4.4.1).
            if let Some(code) = crate::run::entry::help_json_refusal(&relaxed, &args, &error) {
                return Err(code);
            }
            // The failure that comes first on the line answers, though clap judged a value later than it (`--seq x --bogus`).
            let error = crate::arg_rules::relaxed::earlier_failure(&relaxed, &args, &error).unwrap_or(error);
            return Err(crate::run::entry::usage_error(&error));
        }
    };
    // The refusals below carry the repository the call named (`meta.context`), as they did when they were made after the real parse.
    crate::run::entry::record_context(command, &first);
    if let Some((label, error)) = crate::arg_rules::enforce::check(command, &first) {
        return Err(ExitCode::from(error.emit(&label, atip::argv_wants_json())));
    }
    command.clone().try_get_matches_from(args).map_err(|error| crate::run::entry::usage_error(&error))
}

/// The refusal of the first stateless fault of an invocation, with the label of its command, in the order of step 6 of 11.7: domains,
/// then constraints (`arg_rules::enforce`), then validators; `None` when there is none. The domains and constraints are the ones
/// [`parse`] already judged (a batch runs this on every element in preflight before it writes any). It reads nothing but the parsed
/// arguments and the role the environment declares.
pub fn refusal(cli: &Cli, root: &clap::Command, matches: &clap::ArgMatches) -> Option<(String, CliError)> {
    if let Some(found) = crate::arg_rules::enforce::check(root, matches) {
        return Some(found);
    }
    if let Some(found) = crate::arg_rules::semantic::check(root, matches) {
        return Some(found);
    }
    let (label, result): (&str, Result<(), CliError>) = match &cli.command {
        Command::Brief { action: BriefCommand::Approve(args) } => ("canon brief approve", crate::brief_approve::check_approve(args)),
        Command::Policy { action: PolicyCommand::Approve(args) } => ("canon policy approve", crate::relay::check_arguments("canon policy approve", &crate::relay::Call { via: args.relayed_via.as_deref(), actor: args.actor.as_deref(), approver: Some(&args.approver), note: args.note.as_deref() }).and_then(|()| crate::policy_approval::check_approver(&args.approver))),
        Command::Divergence { action: DivergenceCliCommand::Accept(args) } => ("canon divergence accept", crate::relay::check_arguments("canon divergence accept", &crate::relay::Call { via: args.relayed_via.as_deref(), actor: Some(&args.actor_id), approver: args.approver.as_deref(), note: args.note.as_deref() })),
        Command::Migrate(args) => ("canon migrate", crate::relay::check_arguments("canon migrate", &crate::relay::Call { via: args.relayed_via.as_deref(), actor: args.actor.as_deref(), approver: args.approver.as_deref(), note: args.note.as_deref() })),
        Command::Evidence { action } => match &**action {
            EvidenceCommand::Add(args) => ("canon evidence add", crate::evidence::check_add(args)),
            EvidenceCommand::Audit(args) => ("canon evidence audit", crate::evidence::check_audit(args)),
            _ => return None,
        },
        Command::Finding { action: FindingCommand::Add(args) } => ("canon finding add", crate::finding::check_add(args)),
        Command::Query(args) => ("canon query", crate::query::validate_scope(args.kind, args.change_id.as_ref(), args.status.as_deref(), args.domain.as_deref()).map_err(|error| error.to_cli_error())),
        Command::Export(args) => ("canon export", crate::retention::check_window(args)),
        _ => return None,
    };
    result.err().map(|error| (label.to_string(), error))
}

/// Refuses the invocation when it has a stateless fault: the exit code to end with, or `None`.
pub fn check(cli: &Cli, root: &clap::Command, matches: &clap::ArgMatches) -> Option<ExitCode> {
    refusal(cli, root, matches).map(|(label, error)| ExitCode::from(error.emit(&label, atip::argv_wants_json())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::FromArgMatches as _;

    /// `canon finding add` with `flags` set over a call that passes every check.
    fn finding(flags: &[(&str, &str)]) -> Option<u8> {
        let mut given = vec![("--change", "add-login"), ("--round", "1"), ("--seq", "1"), ("--severity", "note"), ("--actor", "alice"), ("--summary", "ok")];
        for (flag, value) in flags {
            match given.iter_mut().find(|(name, _)| name == flag) {
                Some(entry) => entry.1 = value,
                None => given.push((flag, value)),
            }
        }
        let mut argv = vec!["canon", "finding", "add"];
        argv.extend(given.iter().flat_map(|(flag, value)| [*flag, *value]));
        let mut command = crate::cli::command();
        command.build();
        let mut matches = command.clone().try_get_matches_from(argv).expect("the call parses");
        let cli = Cli::from_arg_matches_mut(&mut matches).expect("the call reads back");
        let Command::Finding { action: FindingCommand::Add(args) } = cli.command else { panic!("not `finding add`") };
        crate::finding::check_add(&args).err().map(|_| 2)
    }

    /// The call is refused (exit 2); the codes of the refusals are held by `tests/finding.rs` and `tests/validators_first.rs`.
    fn refused(result: Option<u8>) {
        assert_eq!(result, Some(2));
    }

    #[test]
    fn a_zero_round_or_seq_is_refused() {
        refused(finding(&[("--round", "0")]));
        refused(finding(&[("--seq", "0")]));
        assert_eq!(finding(&[]), None);
    }

    #[test]
    fn an_empty_actor_or_summary_is_refused() {
        for blank in ["", "   "] {
            refused(finding(&[("--summary", blank)]));
            refused(finding(&[("--actor", blank)]));
        }
    }

    /// Every member of the IMPORTED set, over every free-text field, so a later edit that narrows the
    /// check to `'\n'`, or that misses a field, fails here.
    #[test]
    fn every_row_line_break_in_every_free_text_field_is_refused() {
        for separator in canon_ingest::task_rows::ROW_LINE_BREAKS {
            let injected = format!("ok{separator}| forged | row |");
            for flag in ["--summary", "--actor", "--file-ref"] {
                refused(finding(&[(flag, &injected)]));
            }
        }
    }

    /// Every non-`Fixed` disposition, not just `Open`: `rejected` and `deferred` clear `resolution_sha` in
    /// the model, so a check that only refused `Open` would silently DROP a sha the caller supplied.
    #[test]
    fn a_resolution_rev_with_any_non_fixed_disposition_is_refused() {
        for disposition in ["open", "rejected", "deferred"] {
            refused(finding(&[("--disposition", disposition), ("--resolution-rev", "HEAD")]));
        }
        assert_eq!(finding(&[("--disposition", "fixed"), ("--resolution-rev", "HEAD")]), None);
    }
}
