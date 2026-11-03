//! What `canon schema` says about the arguments of a command that clap's tree does not (0.34, atip/1 spec 10.7.4, 10.8):
//! the `constraints` between arguments, the `domain` of one argument's own value, and the `validators`.
//!
//! clap says the integer type of an argument ([`atip::domain::implicit`]) and nothing else about how arguments go together:
//! no `conflicts_with`, `requires`, `required_unless_present` or `ArgGroup` is declared on the command tree, because clap enforces
//! those while it parses, before the domains, and the contract (atip/1 spec 11.7 step 6) answers a failure to parse first, then
//! `required`, then domains, then constraints, then validators. So everything else is a table, in this module and its children,
//! one child per group of commands:
//!
//! - [`Rule`]: a relation between arguments the parser or the handler enforces without reading state. [`rules_of`] lists a
//!   command's rules in the order its document gives them; `tests/arg_rules.rs` drives every rule through the real parser;
//! - [`DomainRow`]: the restriction of one argument's value (a minimum, a length, a pattern) with the code a value outside it is
//!   refused with, and values just inside and outside it that the tests run through the real parser;
//! - [`Validator`]: a stateless check neither can say (a calendar date that must exist, an order between two timestamps),
//!   with calls that it refuses.
//!
//! A table that disagrees with the binary fails `tests/arg_rules.rs`: a rule whose violation is not refused with its code, a
//! domain whose boundary is answered differently, a validator whose `rejects` is accepted. The other way, a check the
//! handlers make that no table declares, fails the survey of the sources in the same test.

use atip::constraint::{Rule, Validator};
use atip::domain::Domain;

/// Where a domain applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Every argument whose value name is this one (`TIMESTAMP`): the value parser the name stands for is shared.
    ValueName(&'static str),
    /// The argument of this name in each of these commands.
    Arguments(&'static [&'static str], &'static str),
    /// The argument of this name in every command that has it.
    Everywhere(&'static str),
}

/// The domain of an argument, the code a value outside it is refused with, and values the tests run through the
/// real parser: `inside` ones it does not refuse for this reason, `outside` ones it does.
#[derive(Debug, Clone, Copy)]
pub struct DomainRow {
    pub scope: Scope,
    pub domain: Domain,
    /// `USAGE` when clap's value parser refuses it, the command's own code when the handler does.
    pub code: &'static str,
    /// Text values just inside the domain (an integer domain's are its bounds).
    pub inside: &'static [&'static str],
    /// Text values just outside it (an integer domain's are the values next to its bounds).
    pub outside: &'static [&'static str],
    /// What the refusal of a value outside the domain says after the argument's name; empty: derived from the pattern.
    pub says: &'static str,
    /// The `reason` of the registered code (`EVIDENCE_APPROVAL_INVALID`: `MALFORMED`); empty: none.
    pub reason: &'static str,
}

impl DomainRow {
    pub const fn new(scope: Scope, domain: Domain, code: &'static str) -> DomainRow {
        DomainRow { scope, domain, code, inside: &[], outside: &[], says: "", reason: "" }
    }

    pub const fn inside(mut self, inside: &'static [&'static str]) -> DomainRow {
        self.inside = inside;
        self
    }

    pub const fn reason(mut self, reason: &'static str) -> DomainRow {
        self.reason = reason;
        self
    }

    pub const fn says(mut self, says: &'static str) -> DomainRow {
        self.says = says;
        self
    }

    pub const fn outside(mut self, outside: &'static [&'static str]) -> DomainRow {
        self.outside = outside;
        self
    }
}

/// One group's tables.
pub(crate) struct Tables {
    pub rules: &'static [(&'static str, &'static [Rule])],
    pub validators: &'static [(&'static str, &'static [Validator])],
    pub domains: &'static [DomainRow],
}

/// Every group's tables, and the rows shared by all.
pub(crate) fn groups() -> [&'static Tables; 4] {
    [&SHARED, &evidence::TABLES, &authoring::TABLES, &reads::TABLES]
}

/// The rules of the command `path` that its tables state themselves, in the order the tables list them.
pub fn rules_for(path: &str) -> Vec<Rule> {
    groups().iter().flat_map(|tables| tables.rules.iter().filter(|(command, _)| *command == path).flat_map(|(_, rules)| rules.iter().copied())).collect()
}

/// The relations that used to be clap's own (`conflicts_with`, `ArgGroup`), in the order the command documents list them.
const PORTED: &[(&str, &[Rule])] = &[
    ("init", &[Rule::at_most_one(&["check", "no-agents-md"]), Rule::at_most_one(&["check", "policy", "no-policy"]), Rule::at_most_one(&["check", "providers"]), Rule::at_most_one(&["check", "language"]), Rule::at_most_one(&["check", "dry-run"])]),
    ("migrate", &[Rule::at_most_one(&["revert", "relayed-via"])]),
    ("query", &[Rule::at_most_one(&["plugin", "history"])]),
    ("review change", &[Rule::exactly_one(&["upstream-ref", "original-spec-ref"])]),
    ("review scenario", &[Rule::exactly_one(&["upstream-ref", "original-spec-ref"])]),
    ("schema", &[Rule::at_most_one(&["path", "errors", "error", "warnings", "warning"])]),
];

/// The pairs of options a flattened group of arguments gives every command that has it: `--promote` and `--no-promote` of
/// every staged write, `--limit` and `--all` of every bounded listing.
const SHARED_PAIRS: &[[&str; 2]] = &[["promote", "no-promote"], ["limit", "all"]];

/// Every rule of the command `node`, whose path is `path`, in the order its document lists them (spec 10.8.1): its own relations,
/// the pair each flattened group of arguments gives it, then the rest of the tables. Constraints are checked in this order.
pub fn rules_of(node: &clap::Command, path: &str) -> Vec<Rule> {
    let has = |name: &str| node.get_arguments().any(|arg| arg.get_long() == Some(name) || (arg.is_positional() && atip::constraint::name_of(arg) == name));
    let mut rules: Vec<Rule> = PORTED.iter().filter(|(command, _)| *command == path).flat_map(|(_, rules)| rules.iter().copied()).collect();
    for pair in SHARED_PAIRS {
        if pair.iter().all(|name| has(name)) {
            rules.push(match pair {
                ["promote", "no-promote"] => Rule::at_most_one(&["promote", "no-promote"]),
                _ => Rule::at_most_one(&["limit", "all"]),
            });
        }
    }
    rules.extend(rules_for(path));
    rules
}

/// The validators of the command `path`, in the order the tables list them.
pub fn validators_for(path: &str) -> Vec<Validator> {
    groups().iter().flat_map(|tables| tables.validators.iter().filter(|(command, _)| *command == path).flat_map(|(_, validators)| validators.iter().copied())).collect()
}

/// The domain row of the argument `argument` of the command `path`, whose value name is `value_name`: a row of that
/// argument first, else the one of its value name.
pub fn domain_for(path: &str, argument: &str, value_name: Option<&str>) -> Option<&'static DomainRow> {
    let rows = || groups().into_iter().flat_map(|tables| tables.domains.iter());
    rows()
        .find(|row| match row.scope {
            Scope::Arguments(commands, name) => name == argument && commands.contains(&path),
            Scope::Everywhere(name) => name == argument,
            Scope::ValueName(_) => false,
        })
        .or_else(|| rows().find(|row| matches!(row.scope, Scope::ValueName(name) if Some(name) == value_name)))
}

/// Every domain row of every group, for the tests that hold each to the parser.
pub fn domain_rows() -> Vec<&'static DomainRow> {
    groups().into_iter().flat_map(|tables| tables.domains.iter()).collect()
}

/// The codes the tables name for `path`: what a command's `errors` must list besides what its sources construct.
pub fn codes_for(path: &str, arguments: &[(String, Option<String>)]) -> Vec<&'static str> {
    let mut codes: Vec<&'static str> = Vec::new();
    codes.extend(rules_for(path).iter().filter_map(|rule| rule.code));
    codes.extend(validators_for(path).iter().map(|validator| validator.code));
    for (name, value_name) in arguments {
        if let Some(row) = domain_for(path, name, value_name.as_deref()) {
            codes.push(row.code);
        }
    }
    codes.sort_unstable();
    codes.dedup();
    codes
}

// ------------------------------------------------------------------ patterns

/// The character classes the patterns below are written in. A domain `pattern` is `^`, a restricted I-Regexp body and
/// `$` (spec 10.7.4): no `.`, no `\d`, no `|` outside parentheses. The characters are written out because I-Regexp has no `\s`.
/// A pattern carries no raw line terminator (`^\^.*\$$` of the contract's schema must match it, and `.` does not match one),
/// so LF, CR, U+2028 and U+2029 are written as `\n`, `\r`, `\p{Zl}` and `\p{Zp}`.
macro_rules! white_space {
    () => {
        " \\t\\n\u{b}\u{c}\\r\u{85}\u{a0}\u{1680}\u{2000}-\u{200a}\\p{Zl}\\p{Zp}\u{202f}\u{205f}\u{3000}"
    };
}

/// The line separators a line of text may not carry: U+000A, U+000B, U+000C, U+000D, U+0085, U+2028 and U+2029.
macro_rules! line_breaks {
    () => {
        "\\n\\r\u{b}\u{c}\u{85}\\p{Zl}\\p{Zp}"
    };
}

/// Any character a command line can carry.
macro_rules! any_char {
    () => {
        "[^\u{0}]"
    };
}

pub mod enforce;
pub mod relaxed;
pub mod semantic;
mod authoring;
mod evidence;
mod reads;

/// A kebab-case slug: `[a-z0-9]+(-[a-z0-9]+)*` (change, subject and role ids, a domain, a lane).
pub const KEBAB: &str = "^[a-z0-9]+(-[a-z0-9]+)*$";
/// A spec corpus id: `[a-z0-9][a-z0-9-]*`.
pub const PROJECT_ID: &str = "^[a-z0-9][a-z0-9-]*$";
/// `<area>.<surface>.<nn>`: two `[a-z0-9-]+` words and two or more digits.
pub const SCENARIO_ID: &str = "^[a-z0-9-]+\\.[a-z0-9-]+\\.[0-9]{2,}$";
/// `<change>#<n>(.<n>)*`.
pub const TASK_ID: &str = "^[a-z0-9]+(-[a-z0-9]+)*#[0-9]+(\\.[0-9]+)*$";
/// A ULID: 26 characters of Crockford base32, either case.
pub const ULID: &str = "^[0-9A-HJKMNP-TV-Za-hjkmnp-tv-z]{26}$";
/// A session id: not empty, no leading or trailing white space, no control character.
pub const SESSION_ID: &str = concat!("^[^\\p{Cc}", white_space!(), "]([^\\p{Cc}]*[^\\p{Cc}", white_space!(), "])?$");
/// A time as every `<TIMESTAMP>` option reads it: an RFC 3339 time, or a date written with or without leading zeros
/// (the shape only; that the date exists is a validator's).
pub const TIMESTAMP: &str = concat!(
    "^(",
    "[", white_space!(), "]*[-+]?[0-9]+-[", white_space!(), "]*[0-9]{1,2}-[", white_space!(), "]*[0-9]{1,2}",
    "|[0-9]{4}-[0-9]{2}-[0-9]{2}[Tt ][0-9]{2}:[0-9]{2}:[0-9]{2}(\\.[0-9]+)?([Zz]|[-+\u{2212}][0-9]{2}:[0-9]{2})",
    ")$"
);
/// Text with at least one character that is not white space.
pub const NOT_BLANK: &str = concat!("^", any_char!(), "*[^", white_space!(), "]", any_char!(), "*$");
/// One line (no line separator), possibly empty.
pub const ONE_LINE: &str = concat!("^[^", line_breaks!(), "]*$");
/// One line with at least one character that is not white space.
pub const ONE_LINE_NOT_BLANK: &str = concat!("^[^", line_breaks!(), "]*[^", white_space!(), "][^", line_breaks!(), "]*$");
/// One line that neither starts nor ends with white space, and is not empty.
pub const TRIMMED_ONE_LINE: &str = concat!("^[^", white_space!(), "]([^", line_breaks!(), "]*[^", white_space!(), "])?$");
/// A lowercase hex sha-256: 64 digits.
pub const SHA256_HEX: &str = "^[0-9a-f]{64}$";

// -------------------------------------------------------------------- shared

/// The rows every command shares: the safety pair of the five destructive leaves, the id grammars by value name, and
/// the time grammar.
static SHARED: Tables = Tables {
    rules: &[
        ("purge", SAFETY),
        ("artifacts move", SAFETY),
        ("migrate", SAFETY),
        ("ledger compact", SAFETY),
        ("tier age", SAFETY),
    ],
    validators: &[],
    domains: &[
        DomainRow::new(Scope::ValueName("CHANGE_ID"), Domain::matching(KEBAB), "USAGE").inside(&["add-login", "a", "a1-b2"]).outside(&["Add-login", "add_login", "-a", "a-", "a--b", ""]),
        DomainRow::new(Scope::ValueName("SUBJECT_ID"), Domain::matching(KEBAB), "USAGE").inside(&["demo", "a-1"]).outside(&["Demo", "de mo", "a--b"]),
        DomainRow::new(Scope::ValueName("ROLE"), Domain::matching(KEBAB), "USAGE").inside(&["reviewer", "x-y"]).outside(&["Reviewer", "x_y", "-x"]),
        DomainRow::new(Scope::ValueName("PROJECT_ID"), Domain::matching(PROJECT_ID), "USAGE").inside(&["root", "a--b", "a-"]).outside(&["-a", "Root", "a_b"]),
        DomainRow::new(Scope::ValueName("SCENARIO_ID"), Domain::matching(SCENARIO_ID), "USAGE").inside(&["auth.login.01", "a-.-b.123"]).outside(&["auth.login.1", "Auth.login.01", "auth.login", "a.b.c.01"]),
        DomainRow::new(Scope::ValueName("TASK_ID"), Domain::matching(TASK_ID), "USAGE").inside(&["add-login#1", "add-login#1.2.3", "a#007"]).outside(&["Add#1", "add-login", "add-login#", "add-login#1.", "add-login#a"]),
        DomainRow::new(Scope::ValueName("RUN_ID"), Domain::matching(ULID), "USAGE").inside(&["01ARZ3NDEKTSV4RRFFQ69G5FAV", "01arz3ndektsv4rrffq69g5fav"]).outside(&["01ARZ3NDEKTSV4RRFFQ69G5FA", "01ARZ3NDEKTSV4RRFFQ69G5FAI", "01ARZ3NDEKTSV4RRFFQ69G5FAVV"]),
        DomainRow::new(Scope::ValueName("REVIEW_ID"), Domain::matching(ULID), "USAGE").inside(&["01ARZ3NDEKTSV4RRFFQ69G5FAV"]).outside(&["not-a-ulid", "01ARZ3NDEKTSV4RRFFQ69G5FAL"]),
        DomainRow::new(Scope::ValueName("STRATEGY_ID"), Domain::matching(ULID), "USAGE").inside(&["01ARZ3NDEKTSV4RRFFQ69G5FAV"]).outside(&["not-a-ulid", "01ARZ3NDEKTSV4RRFFQ69G5FAO"]),
        DomainRow::new(Scope::ValueName("SESSION_ID"), Domain::matching(SESSION_ID), "USAGE").inside(&["s-1", "a b"]).outside(&[" s", "s ", "a\tb"]),
        DomainRow::new(Scope::ValueName("TIMESTAMP"), Domain::matching(TIMESTAMP), "USAGE").says("is an invalid timestamp: an RFC 3339 time (`2030-01-01T10:00:00Z`) or a date (`2030-01-01`) is expected").inside(&["2030-01-01", "2030-01-01T00:00:00Z", "2030-1-1", "2030-01-01 10:00:00+02:00"]).outside(&["tomorrow", "2030-01-01T10:00", "2030-01-01T10:00:00", "2030-01-01T10:00:00+0200", "01/02/2030"]),
        // The most records or hits per page, and the size limit of a bound file: at least 1 (clap's `range(1..)`).
        DomainRow::new(Scope::Arguments(&["query", "export", "finding list", "divergence list", "search"], "limit"), Domain::at_least(1), "USAGE"),
        DomainRow::new(Scope::Everywhere("max-artifact-mib"), Domain::at_least(1), "USAGE"),
    ],
};

/// What the validator of a `<TIMESTAMP>` option says (every command that has one writes the same sentence): the shape is
/// the option's domain, that the date exists is this.
pub const CALENDAR: &str = "Reads the value as RFC 3339 (`2030-01-01T10:00:00Z`) or a date (`2030-01-01`, UTC midnight); the date must exist on the calendar: `2026-02-30` is refused.";

/// What the validators of a relayed approval say (`brief approve`, `policy approve`, `divergence accept`, `migrate`): the environment is
/// a stateless input (spec 11.7), so the role and the actor the variables fill are read here, not by a precondition.
pub const ROLE_DESCRIPTION: &str = "`--relayed-via` records a human's answer on behalf of an agent: a role is declared, with `--as` or the `CANON_ROLE` variable.";
pub const ACTOR_DESCRIPTION: &str = "A relayed approval names the agent that recorded it: `--actor`, else the `CANON_ACTOR` variable, is given and is one word.";
pub const ANSWER_RELAYED: &str = "A relayed approval carries the human's own name and answer word for word: with `--relayed-via`, `--approver` and `--note` are not blank.";

/// `--commit` and `--plan-digest` go together (0.28 D6): the bare run is the preview.
const SAFETY: &[Rule] = &[Rule::requires("commit", &["plan-digest"]), Rule::requires("plan-digest", &["commit"])];

// ---------------------------------------------------------- naming the argument at fault

/// The `field` a refusal gives for the argument clap named `raw` (its first word): `--name` for an option, the operand's
/// value name for an operand (spec 7.1.1): `[PATH]...`, `<PATH>` and `PATH` all are `PATH`.
pub fn argument_name(raw: &str) -> String {
    if raw.starts_with('-') {
        return raw.to_string();
    }
    let bare = raw.trim_matches(|c| matches!(c, '[' | ']' | '<' | '>' | '.'));
    if bare.is_empty() {
        raw.to_string()
    } else {
        bare.to_string()
    }
}

/// The arguments one entry of clap's "not provided" list stands for: an argument, or the members of a group
/// (`<--upstream-ref|--original-spec-ref>`).
pub fn missing_names(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with('<') && trimmed.contains('|') {
        return trimmed.trim_matches(|c| matches!(c, '<' | '>')).split('|').map(|member| argument_name(member.split_whitespace().next().unwrap_or_default())).collect();
    }
    vec![argument_name(trimmed.split_whitespace().next().unwrap_or_default())]
}

/// Of the arguments clap says are missing, the one the contract answers with (spec 11.7 step 6): the operands in their order, then
/// the options in their declaration order, the document's order, whatever order clap lists them in.
pub fn first_required(node: &clap::Command, missing: &[String]) -> Option<String> {
    let args: Vec<&clap::Arg> = node.get_positionals().chain(node.get_arguments().filter(|arg| !arg.is_positional())).filter(|arg| !atip::project::is_builtin_arg(arg)).collect();
    args.iter().map(|arg| enforce::field_of(&args, &atip::constraint::name_of(arg))).find(|field| missing.contains(field))
}
