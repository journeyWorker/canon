//! The command tree the first parse of an invocation runs on (atip/1 spec 11.7 step 6).
//!
//! The contract answers a failure to parse (an unknown option or command word, a missing option value, a value that is not of its
//! `valueType` or not one of its `possibleValues`) first, then `required`, then the domains, the constraints and the validators.
//! clap's value parsers refuse a value the moment they read it, so a value outside a declared domain (`--change Bad`, `--limit 0`)
//! would answer ahead of a missing required argument and of a parse failure later on the line. This tree takes such a value as
//! text: an argument that has a domain, and every integer argument (its Rust type's bounds are its domain), is read here by a
//! parser that refuses only what is no integer, or no text at all, and the domain stage (`enforce`) judges the value from the
//! declarations. The real tree parses the call afterwards, for the typed arguments the handlers read, and only a call the domain
//! stage let through.

use atip::domain::SAFE_INTEGER;
use clap::{Arg, Command};

use super::domain_for;

/// `command` with the arguments that have a domain read as text (integers as `i64`): the tree to parse an invocation on before
/// its arguments are judged. Closed sets (`possibleValues`) stay as they are: a value outside one is a failure to parse.
pub fn tree(command: Command) -> Command {
    walk(command, &mut Vec::new())
}

/// The failure to parse that comes before the one clap reported, when clap dropped it. clap reads an option's value when the next token
/// begins, and when that token is an unknown option or command word, a stray operand or an option that takes no value, it answers for that
/// token and drops the verdict on the value it was holding (`--seq x --bogus` is answered by `--bogus`). The contract answers the earliest
/// failure on the line (spec 11.7 step 6), so the words before the reported token are parsed again: a failure they have is earlier, whatever
/// it is, except that something is still to come (a missing `required` argument or command word).
pub fn earlier_failure(tree: &Command, args: &[std::ffi::OsString], error: &clap::Error) -> Option<clap::Error> {
    use clap::error::{ContextKind, ContextValue, ErrorKind};
    if !matches!(error.kind(), ErrorKind::UnknownArgument | ErrorKind::InvalidSubcommand | ErrorKind::NoEquals | ErrorKind::TooManyValues | ErrorKind::InvalidUtf8) {
        return None;
    }
    let named: Vec<String> = [ContextKind::InvalidArg, ContextKind::InvalidSubcommand]
        .into_iter()
        .filter_map(|kind| match error.get(kind) {
            Some(ContextValue::String(text)) => Some(text.clone()),
            Some(ContextValue::Strings(texts)) => texts.first().cloned(),
            _ => None,
        })
        .filter_map(|text| text.split_whitespace().next().map(str::to_string))
        .collect();
    let end = args.iter().skip(1).position(|word| word == "--").map_or(args.len(), |at| at + 1);
    let at = 1 + args.get(1..end)?.iter().position(|word| {
        let word = word.to_string_lossy();
        named.iter().any(|name| word == name.as_str() || word.starts_with(&format!("{name}=")))
    })?;
    match tree.clone().try_get_matches_from(args[..at].to_vec()) {
        Err(earlier) if !matches!(earlier.kind(), ErrorKind::MissingRequiredArgument | ErrorKind::MissingSubcommand | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand | ErrorKind::ArgumentConflict | ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => Some(earlier),
        _ => None,
    }
}

fn walk(command: Command, path: &mut Vec<String>) -> Command {
    let children: Vec<String> = command.get_subcommands().map(|sub| sub.get_name().to_string()).collect();
    let mut command = command;
    for name in children {
        path.push(name.clone());
        command = command.mut_subcommand(&name, |sub| walk(sub, path));
        path.pop();
    }
    if path.is_empty() {
        return command;
    }
    let joined = path.join(" ");
    command.mut_args(|arg| relax(&joined, arg))
}

/// `arg` with the parser that leaves the domain to the domain stage, or as it was when it has none to leave.
fn relax(path: &str, arg: Arg) -> Arg {
    if !arg.get_action().takes_values() || arg.get_value_parser().possible_values().is_some() {
        return arg;
    }
    if atip::project::is_integer_arg(&arg) {
        return arg.value_parser(signed_safe_integer);
    }
    let has_domain = domain_for(path, &atip::constraint::name_of(&arg), Some(&atip::project::value_name(&arg))).is_some();
    if has_domain {
        arg.value_parser(clap::value_parser!(String))
    } else {
        arg
    }
}

/// An integer the contract's grammar reads (spec 11.5): digits with an optional sign, `-0` and `+0` the number 0, nothing outside
/// ±(2^53-1) rounded into range. A value no integer, or beyond the safe range, is not of the `valueType`: a failure to parse.
fn signed_safe_integer(text: &str) -> Result<i64, String> {
    let unsigned = text.strip_prefix('+').unwrap_or(text);
    let (negative, digits) = unsigned.strip_prefix('-').map_or((false, unsigned), |digits| (true, digits));
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("`{text}` is not an integer"));
    }
    let magnitude = digits.parse::<u128>().ok().filter(|value| *value <= u128::from(SAFE_INTEGER as u64));
    let Some(magnitude) = magnitude else { return Err(format!("`{text}` is beyond 9007199254740991 (2^53-1): an integer within ±{SAFE_INTEGER}")) };
    let value = i64::try_from(magnitude).unwrap_or(SAFE_INTEGER);
    Ok(if negative { -value } else { value })
}
