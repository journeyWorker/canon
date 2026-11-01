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

/// Refuses the invocation when one of its validators does: the exit code to end with, or `None`.
pub fn check(cli: &Cli) -> Option<ExitCode> {
    let (label, json, result): (&str, bool, Result<(), CliError>) = match &cli.command {
        Command::Brief { action: BriefCommand::Approve(args) } => ("canon brief approve", args.json, crate::brief_approve::check_approve(args)),
        Command::Policy { action: PolicyCommand::Approve(args) } => ("canon policy approve", args.json, crate::relay::check_arguments("canon policy approve", &crate::relay::Call { via: args.relayed_via.as_deref(), actor: args.actor.as_deref(), approver: Some(&args.approver), note: args.note.as_deref() }).and_then(|()| crate::policy_approval::check_approver(&args.approver))),
        Command::Divergence { action: DivergenceCliCommand::Accept(args) } => ("canon divergence accept", args.json, crate::relay::check_arguments("canon divergence accept", &crate::relay::Call { via: args.relayed_via.as_deref(), actor: Some(&args.actor_id), approver: args.approver.as_deref(), note: args.note.as_deref() })),
        Command::Migrate(args) => ("canon migrate", args.json, crate::relay::check_arguments("canon migrate", &crate::relay::Call { via: args.relayed_via.as_deref(), actor: args.actor.as_deref(), approver: args.approver.as_deref(), note: args.note.as_deref() })),
        Command::Evidence { action } => match &**action {
            EvidenceCommand::Add(args) => ("canon evidence add", args.json, crate::evidence::check_add(args)),
            EvidenceCommand::Audit(args) => ("canon evidence audit", args.json, crate::evidence::check_audit(args)),
            _ => return None,
        },
        Command::Finding { action: FindingCommand::Add(args) } => return crate::finding::check_add(args).map(ExitCode::from),
        Command::Query(args) => ("canon query", args.json, crate::query::validate_scope(args.kind, args.change_id.as_ref(), args.status.as_deref(), args.domain.as_deref()).map_err(|error| error.to_cli_error())),
        Command::Export(args) => ("canon export", args.json, crate::retention::check_window(args)),
        _ => return None,
    };
    result.err().map(|error| ExitCode::from(error.emit(label, json)))
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
        crate::finding::check_add(&args)
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
