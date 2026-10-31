//! `atip`'s command line: one command, `check`.
//!
//! Parsed by hand, so the checker names no argument-parsing framework and
//! answers every malformed line with the contract's codes: `UNKNOWN_COMMAND`,
//! `COMMAND_REQUIRED` or `USAGE` with the argument at fault in `field`.

use std::ffi::OsString;

use crate::output::{Code, Failure};

/// The levels the contract defines; only `L0` has checks in this release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    L0,
    L1,
    L2,
}

/// The arguments of `atip check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckArgs {
    pub bin: OsString,
    pub level: Level,
    pub fixture: OsString,
}

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Version,
    RootHelp,
    CheckHelp,
    Check(CheckArgs),
}

const COMMANDS: [&str; 1] = ["check"];
const ROOT_OPTIONS: [&str; 3] = ["--json", "--version", "--help"];
const CHECK_OPTIONS: [&str; 5] = ["--bin", "--level", "--fixture", "--json", "--help"];
const LEVELS: [&str; 3] = ["L0", "L1", "L2"];

pub const ROOT_HELP: &str = "atip: run a built binary and check the atip/1 contract from outside

Usage: atip check --bin <BINARY> --level <LEVEL> --fixture <FILE> [--json]

Commands:
  check    Run the checks of a level against a binary

Options:
  -h, --help       Print help
  -V, --version    Print the version

Run atip check --help for the options of check.";

pub const CHECK_HELP: &str = "atip check: run the checks of a level against a built binary

Usage: atip check --bin <BINARY> --level <LEVEL> --fixture <FILE> [--json]

Options:
      --bin <BINARY>     The binary to check: a path, or a name found on PATH
      --level <LEVEL>    The level the tool claims: L0 (L1 and L2 are not supported yet)
      --fixture <FILE>   The tool's fixture file: JSON with setup, env, timeoutSeconds, probe and registry
      --json             Print one JSON envelope on stdout instead of text
  -h, --help             Print help

Every MUST check passing exits 0, a failed level exits 1 (LEVEL_NOT_MET), a refusal exits 2.";

/// Parses the arguments after the program name.
pub fn parse(args: &[OsString]) -> Result<Command, Failure> {
    let mut version = false;
    let mut help = false;
    let mut position = 0;
    let mut command_at = None;
    while let Some(arg) = args.get(position) {
        let word = arg.to_str();
        match word {
            Some("--") => {
                command_at = (position + 1 < args.len()).then_some(position + 1);
                break;
            }
            Some("--json") => {}
            Some("--version" | "-V") => version = true,
            Some("--help" | "-h") => help = true,
            Some(option) if option.starts_with('-') && option.len() > 1 => return Err(unknown_option(option, &ROOT_OPTIONS, None)),
            _ => {
                command_at = Some(position);
                break;
            }
        }
        position += 1;
    }
    if version {
        return Ok(Command::Version);
    }
    let Some(command_at) = command_at else {
        return if help { Ok(Command::RootHelp) } else { Err(Failure::new(Code::CommandRequired, "atip was run without a command").expected(COMMANDS).hint("Run atip check --bin <BINARY> --level L0 --fixture <FILE>.")) };
    };
    let word = args[command_at].to_string_lossy();
    if word != "check" {
        return Err(Failure::new(Code::UnknownCommand, format!("{word} is not a command of atip")).field("command").expected(COMMANDS).received(word.as_ref()).suggestions(close(&word, &COMMANDS)));
    }
    parse_check(&args[command_at + 1..])
}

fn parse_check(args: &[OsString]) -> Result<Command, Failure> {
    let usage = |message: String| Failure::new(Code::Usage, message).in_command("check");
    let mut bin = None;
    let mut level = None;
    let mut fixture = None;
    let mut help = false;
    let mut position = 0;
    while let Some(arg) = args.get(position) {
        let Some(word) = arg.to_str() else {
            return Err(usage("an argument is not valid UTF-8".to_string()).field("ARGUMENT"));
        };
        if word == "--" {
            if let Some(operand) = args.get(position + 1) {
                return Err(unexpected_operand(&operand.to_string_lossy()));
            }
            break;
        }
        let (name, inline) = match word.split_once('=') {
            Some((name, value)) if word.starts_with("--") => (name, Some(value)),
            _ => (word, None),
        };
        match name {
            "--json" if inline.is_none() => {}
            "--help" | "-h" if inline.is_none() => help = true,
            "--bin" | "--level" | "--fixture" => {
                let value: OsString = match inline {
                    Some(value) => value.into(),
                    None => match args.get(position + 1) {
                        Some(next) if !next.to_string_lossy().starts_with('-') => {
                            position += 1;
                            next.clone()
                        }
                        _ => return Err(usage(format!("the option {name} needs a value")).field(name)),
                    },
                };
                let slot = match name {
                    "--bin" => &mut bin,
                    "--level" => &mut level,
                    _ => &mut fixture,
                };
                if slot.replace(value).is_some() {
                    return Err(usage(format!("the option {name} was given twice")).field(name));
                }
            }
            option if option.starts_with('-') && option.len() > 1 => return Err(unknown_option(option, &CHECK_OPTIONS, Some("check"))),
            operand => return Err(unexpected_operand(operand)),
        }
        position += 1;
    }
    if help {
        return Ok(Command::CheckHelp);
    }
    let required = |value: Option<OsString>, name: &str| value.ok_or_else(|| usage(format!("the option {name} is required")).field(name));
    let bin = required(bin, "--bin")?;
    let level = required(level, "--level")?;
    let fixture = required(fixture, "--fixture")?;
    let level = match level.to_str() {
        Some("L0") => Level::L0,
        Some("L1") => Level::L1,
        Some("L2") => Level::L2,
        _ => {
            let given = level.to_string_lossy();
            return Err(usage(format!("{given} is not a level")).field("--level").expected(LEVELS).received(given.as_ref()).suggestions(close(&given.to_ascii_uppercase(), &LEVELS)));
        }
    };
    Ok(Command::Check(CheckArgs { bin, level, fixture }))
}

fn unexpected_operand(operand: &str) -> Failure {
    Failure::new(Code::Usage, format!("atip check takes no operand, got {operand}")).in_command("check").field("ARGUMENT").received(operand)
}

fn unknown_option(option: &str, valid: &[&'static str], command: Option<&'static str>) -> Failure {
    let name = option.split_once('=').map_or(option, |(name, _)| name);
    let mut failure = Failure::new(Code::Usage, format!("{name} is not an option of {}", command.map_or("atip", |_| "atip check"))).field(name).expected(valid.iter().copied()).suggestions(close(name, valid));
    failure.command = command;
    failure
}

/// The words of `valid` nearest to `word`, when they are within edit distance 2 of it.
fn close(word: &str, valid: &[&str]) -> Vec<String> {
    let distances: Vec<(usize, &str)> = valid.iter().map(|candidate| (distance(word, candidate), *candidate)).collect();
    let nearest = distances.iter().map(|(distance, _)| *distance).min().unwrap_or(usize::MAX);
    distances.into_iter().filter(|(distance, _)| *distance == nearest && nearest <= 2).map(|(_, candidate)| candidate.to_string()).collect()
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = previous + usize::from(ca != *cb);
            previous = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{distance, parse, CheckArgs, Command, Level};
    use crate::output::Code;

    fn args(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    fn code(words: &[&str]) -> (Code, Option<String>, Option<&'static str>) {
        let failure = parse(&args(words)).expect_err("must be refused");
        (failure.code, failure.field.clone(), failure.command)
    }

    #[test]
    fn atip_check_06_a_complete_check_line_parses_with_either_option_spelling() {
        let expected = Command::Check(CheckArgs { bin: "target/x".into(), level: Level::L0, fixture: "f.json".into() });
        assert_eq!(parse(&args(&["check", "--bin", "target/x", "--level", "L0", "--fixture", "f.json"])).ok(), Some(expected.clone()));
        assert_eq!(parse(&args(&["--json", "check", "--bin=target/x", "--fixture=f.json", "--level=L0", "--json"])).ok(), Some(expected));
        assert!(matches!(parse(&args(&["check", "--level", "L2", "--bin", "b", "--fixture", "f"])).ok(), Some(Command::Check(CheckArgs { level: Level::L2, .. }))));
    }

    #[test]
    fn atip_check_06_the_root_answers_a_missing_or_unknown_command_with_its_own_code() {
        assert_eq!(code(&[]), (Code::CommandRequired, None, None));
        assert_eq!(code(&["--json"]), (Code::CommandRequired, None, None));
        assert_eq!(code(&["nosuch-xyz", "--json"]), (Code::UnknownCommand, Some("command".to_string()), None));
        assert_eq!(code(&["--bogus", "check"]), (Code::Usage, Some("--bogus".to_string()), None));
        assert_eq!(parse(&args(&["--version"])).ok(), Some(Command::Version));
        assert_eq!(parse(&args(&["-h"])).ok(), Some(Command::RootHelp));
        assert_eq!(parse(&args(&["check", "--help"])).ok(), Some(Command::CheckHelp));
        let failure = parse(&args(&["chekc"])).expect_err("unknown");
        assert_eq!((failure.expected.as_slice(), failure.suggestions.as_slice()), (["check".to_string()].as_slice(), ["check".to_string()].as_slice()));
    }

    #[test]
    fn atip_check_06_a_malformed_check_line_is_a_usage_error_naming_the_argument() {
        assert_eq!(code(&["check", "--bogus-xyz", "--json"]), (Code::Usage, Some("--bogus-xyz".to_string()), Some("check")));
        assert_eq!(code(&["check", "--json", "--bogus-xyz"]), (Code::Usage, Some("--bogus-xyz".to_string()), Some("check")));
        assert_eq!(code(&["check", "--level", "L0", "--fixture", "f"]), (Code::Usage, Some("--bin".to_string()), Some("check")));
        assert_eq!(code(&["check", "--bin"]), (Code::Usage, Some("--bin".to_string()), Some("check")));
        assert_eq!(code(&["check", "--bin", "--level"]), (Code::Usage, Some("--bin".to_string()), Some("check")));
        assert_eq!(code(&["check", "--bin", "a", "--bin", "b"]), (Code::Usage, Some("--bin".to_string()), Some("check")));
        assert_eq!(code(&["check", "--bin", "a", "--level", "L9", "--fixture", "f"]), (Code::Usage, Some("--level".to_string()), Some("check")));
        assert_eq!(code(&["check", "--", "--json"]), (Code::Usage, Some("ARGUMENT".to_string()), Some("check")));
        assert_eq!(code(&["check", "stray"]), (Code::Usage, Some("ARGUMENT".to_string()), Some("check")));
        let level = parse(&args(&["check", "--bin", "a", "--level", "l1", "--fixture", "f"])).expect_err("lower case is not a level");
        assert_eq!(level.suggestions, ["L1"]);
    }

    #[test]
    fn edit_distance_counts_single_edits() {
        assert_eq!((distance("check", "check"), distance("chekc", "check"), distance("", "ab")), (0, 2, 2));
    }
}
