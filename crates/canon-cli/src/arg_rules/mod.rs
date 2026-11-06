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
pub(crate) fn groups() -> [&'static Tables; 5] {
    [&SHARED, &evidence::TABLES, &authoring::TABLES, &reads::TABLES, &values::TABLES]
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
    ("schema", &[Rule::at_most_one(&["schemas", "errors", "error", "warnings", "warning"]), Rule::at_most_one(&["path", "errors", "error", "warnings", "warning"])]),
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

/// The white space the patterns below are written with. A domain `pattern` is `^`, a restricted I-Regexp body and `$` (spec 10.7.4,
/// Annex V2): no `.`, no `\d`, no `\s`, no `|` outside parentheses. Since atip/1 1.5 the body says white space as the Unicode
/// property `White_Space` (`\p{White_Space}`, `\P{White_Space}`), outside and inside a class, a name Rust `regex` and ECMAScript with
/// the `u` flag read the same: U+0009 to U+000D, U+0020, U+0085, U+00A0, U+1680, U+2000 to U+200A, U+2028, U+2029, U+202F, U+205F and
/// U+3000. A pattern carries no raw line terminator (`^\^.*\$$` of the contract's schema must match it, and `.` does not match one).
macro_rules! white_space {
    () => {
        "\\p{White_Space}"
    };
}

/// A character that is not white space.
macro_rules! not_white_space {
    () => {
        "\\P{White_Space}"
    };
}

/// Any character but a line separator (U+000A to U+000D, U+0085, U+2028 and U+2029). `White_Space` is the space separators, the tab and
/// those seven, so this class (Annex V2) names none of them.
macro_rules! line_char {
    () => {
        "[\\P{White_Space}\\p{Zs}\\t]"
    };
}

/// The text of [`line_char!`] for the code that reads a pattern's shape.
pub(crate) const LINE_CHAR: &str = line_char!();

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
mod values;

/// A kebab-case slug: `[a-z0-9]+(-[a-z0-9]+)*` (change, subject and role ids, a domain, a lane).
pub const KEBAB: &str = "^[a-z0-9]+(-[a-z0-9]+)*$";
/// `<area>.<surface>.<nn>`: two `[a-z0-9-]+` words and two or more digits.
pub const SCENARIO_ID: &str = "^[a-z0-9-]+\\.[a-z0-9-]+\\.[0-9]{2,}$";
/// `<change>#<n>(.<n>)*`.
pub const TASK_ID: &str = "^[a-z0-9]+(-[a-z0-9]+)*#[0-9]+(\\.[0-9]+)*$";
/// A ULID: 26 characters of Crockford base32, either case.
pub const ULID: &str = "^[0-9A-HJKMNP-TV-Za-hjkmnp-tv-z]{26}$";
/// A session id: not empty, no leading or trailing white space, no control character.
pub const SESSION_ID: &str = concat!("^[^\\p{Cc}", white_space!(), "]([^\\p{Cc}]*[^\\p{Cc}", white_space!(), "])?$");
/// A time as a time option reads it (`--expiry`, `--as-of`, `--before`, `--after`, `--since`): an RFC 3339 full-date (`2030-01-01`, that day's UTC
/// midnight) or date-time (`2030-01-01T10:00:00Z`): four digits of year, two of month and day and of each field of the time, `T` and `Z` in either
/// case, an optional fraction and `Z` or `+HH:MM` / `-HH:MM`; no sign on the year, no white space and no other separator (the shape only; that the
/// date exists is a validator's).
pub const TIMESTAMP: &str = "^[0-9]{4}-[0-9]{2}-[0-9]{2}([Tt][0-9]{2}:[0-9]{2}:[0-9]{2}(\\.[0-9]+)?([Zz]|[-+][0-9]{2}:[0-9]{2}))?$";
/// A time as `--approval-at` reads it: an RFC 3339 date-time and nothing else, the spelling of [`TIMESTAMP`] with a time.
pub const DATE_TIME: &str = "^[0-9]{4}-[0-9]{2}-[0-9]{2}[Tt][0-9]{2}:[0-9]{2}:[0-9]{2}(\\.[0-9]+)?([Zz]|[-+][0-9]{2}:[0-9]{2})$";
/// `--providers` of `init` and of the `skills` commands (0.37 `canon-0-37-option-forms`): `none`, or one or more of `claude`, `codex`, `omp`, `pi` (lowercase)
/// joined by single commas: no white space, no empty item, `none` alone. A name written twice is taken.
pub const PROVIDERS: &str = "^(none|(claude|codex|omp|pi)(,(claude|codex|omp|pi))*)$";
/// Text with at least one character that is not white space.
pub const NOT_BLANK: &str = concat!("^", any_char!(), "*", not_white_space!(), any_char!(), "*$");
/// One line (no line separator), possibly empty.
pub const ONE_LINE: &str = concat!("^", line_char!(), "*$");
/// A name or a sentence (`--actor`, `--approver`, `--relayed-via`, `--reason`, `--summary`, `--override-reason`): one line of text that is taken
/// with the white space around it (the parser trims it, and a value that is nothing but white space is refused), with no control
/// character, line separator or paragraph separator inside it. The inner class is `[^\p{Cc}\p{Zl}\p{Zp}]` (a space and a no-break space are
/// in it, a tab, an escape and a line break are not); the first and the last character are not white space.
pub const LINE_TEXT: &str = concat!("^", white_space!(), "*([^\\p{Cc}\\p{Zl}\\p{Zp}]*[^\\p{Cc}", white_space!(), "])", white_space!(), "*$");
/// A regime key: four `/` separated segments, the first three without white space, the last 6 to 64 lowercase hex digits (`retrieve --regime`, `dispatch begin --regime`).
pub const REGIME_KEY: &str = concat!("^[^/", white_space!(), "]+/[^/", white_space!(), "]+/[^/", white_space!(), "]+/[0-9a-f]{6,64}$");
/// One line that neither starts nor ends with white space, and is not empty.
pub const TRIMMED_ONE_LINE: &str = concat!("^", not_white_space!(), "(", line_char!(), "*", not_white_space!(), ")?$");
/// A lowercase hex sha-256: 64 digits.
pub const SHA256_HEX: &str = "^[0-9a-f]{64}$";

// -------------------------------------------------------------------- shared

/// The rows every command shares: the safety pair of the six destructive leaves, the id grammars by value name, and
/// the time grammar.
static SHARED: Tables = Tables {
    rules: &[
        ("purge", SAFETY),
        ("artifacts move", SAFETY),
        ("artifacts prune", SAFETY),
        ("migrate", SAFETY),
        ("ledger compact", SAFETY),
        ("tier age", SAFETY),
    ],
    validators: &[],
    domains: &[
        DomainRow::new(Scope::ValueName("CHANGE_ID"), Domain::matching(KEBAB), "USAGE").inside(&["add-login", "a", "a1-b2"]).outside(&["Add-login", "add_login", "-a", "a-", "a--b", ""]),
        DomainRow::new(Scope::ValueName("SUBJECT_ID"), Domain::matching(KEBAB), "USAGE").inside(&["demo", "a-1"]).outside(&["Demo", "de mo", "a--b"]),
        DomainRow::new(Scope::ValueName("ROLE"), Domain::matching(KEBAB), "USAGE").inside(&["reviewer", "x-y"]).outside(&["Reviewer", "x_y", "-x"]),
        DomainRow::new(Scope::ValueName("PROJECT_ID"), Domain::matching(KEBAB), "USAGE").inside(&["root", "a-1"]).outside(&["-a", "Root", "a_b", "a-", "a--b"]),
        DomainRow::new(Scope::ValueName("SCENARIO_ID"), Domain::matching(SCENARIO_ID), "USAGE").inside(&["auth.login.01", "a-.-b.123"]).outside(&["auth.login.1", "Auth.login.01", "auth.login", "a.b.c.01"]),
        DomainRow::new(Scope::ValueName("TASK_ID"), Domain::matching(TASK_ID), "USAGE").inside(&["add-login#1", "add-login#1.2.3", "a#007"]).outside(&["Add#1", "add-login", "add-login#", "add-login#1.", "add-login#a"]),
        DomainRow::new(Scope::ValueName("RUN_ID"), Domain::matching(ULID), "USAGE").inside(&["01ARZ3NDEKTSV4RRFFQ69G5FAV", "01arz3ndektsv4rrffq69g5fav"]).outside(&["01ARZ3NDEKTSV4RRFFQ69G5FA", "01ARZ3NDEKTSV4RRFFQ69G5FAI", "01ARZ3NDEKTSV4RRFFQ69G5FAVV"]),
        DomainRow::new(Scope::ValueName("REVIEW_ID"), Domain::matching(ULID), "USAGE").inside(&["01ARZ3NDEKTSV4RRFFQ69G5FAV"]).outside(&["not-a-ulid", "01ARZ3NDEKTSV4RRFFQ69G5FAL"]),
        DomainRow::new(Scope::ValueName("STRATEGY_ID"), Domain::matching(ULID), "USAGE").inside(&["01ARZ3NDEKTSV4RRFFQ69G5FAV"]).outside(&["not-a-ulid", "01ARZ3NDEKTSV4RRFFQ69G5FAO"]),
        DomainRow::new(Scope::ValueName("SESSION_ID"), Domain::matching(SESSION_ID), "USAGE").inside(&["s-1", "a b", "a\u{a0}b", "a\u{feff}"]).outside(&[" s", "s ", "a\tb", "\u{a0}s", "s\u{3000}", "\u{2003}s"]),
        DomainRow::new(Scope::ValueName("TIMESTAMP"), Domain::matching(TIMESTAMP), "USAGE")
            .says("is an invalid timestamp: an RFC 3339 date (`2030-01-01`, UTC midnight) or date-time (`2030-01-01T10:00:00Z`) is expected")
            .inside(&["2030-01-01", "2030-01-01T00:00:00Z", "2030-01-01t10:00:00z", "2030-01-01T10:00:00.5+09:00", "2030-01-01T10:00:00-02:00"])
            .outside(&["tomorrow", "2030-1-1", "2030-1-01", "2030-01-1", " 2030-01-01", "2030-01-01 ", "+2030-01-01", "20300-01-01", "2030- 01-01", "2030-01-01 10:00:00Z", "2030-01-01T10:00", "2030-01-01T10:00:00", "2030-01-01T10:00:00+0200", "2030-01-01T10:00:00\u{2212}02:00", "01/02/2030"]),
        // `--approval-at` is signed into the payload: a date-time, which a date alone is not.
        DomainRow::new(Scope::Arguments(&["evidence add", "evidence approval-payload", "strategy approval-payload", "strategy rollback"], "approval-at"), Domain::matching(DATE_TIME), "USAGE")
            .says("is an invalid timestamp: an RFC 3339 date-time (`2030-01-01T10:00:00Z`) is expected; a date alone is refused")
            .inside(&["2030-01-01T00:00:00Z", "2030-01-01t10:00:00z", "2030-01-01T10:00:00.123456789+09:00", "2030-01-01T10:00:00-02:00"])
            .outside(&["tomorrow", "2030-01-01", "2030-1-1T10:00:00Z", " 2030-01-01T10:00:00Z", "2030-01-01T10:00:00Z ", "+2030-01-01T10:00:00Z", "2030-01-01 10:00:00Z", "2030-01-01T10:00", "2030-01-01T10:00:00", "2030-01-01T10:00:00+0200", "2030-01-01T10:00:00\u{2212}02:00"]),
        // The most records or hits per page, and the size limit of a bound file: at least 1 (clap's `range(1..)`).
        DomainRow::new(Scope::Arguments(&["query", "export", "finding list", "divergence list", "search", "retrieve", "artifacts prune"], "limit"), Domain::at_least(1), "USAGE"),
        DomainRow::new(Scope::Everywhere("max-artifact-mib"), Domain::at_least(1), "USAGE"),
        DomainRow::new(Scope::Everywhere("keep-days"), Domain::at_least(1), "USAGE"),
    ],
};

/// What the validator of a time option says (every command that has one writes the same sentence): the spelling is the option's domain, that the
/// date exists is this.
pub const CALENDAR: &str = "Reads the value as RFC 3339: a date (`2030-01-01`, UTC midnight) or a date-time (`2030-01-01T10:00:00Z`); the date must exist on the calendar: `2026-02-30` is refused.";
/// What the validator of `--approval-at` says: the same, for a date-time alone.
pub const CALENDAR_DATE_TIME: &str = "Reads the value as an RFC 3339 date-time (`2030-01-01T10:00:00Z`), the instant signed into the payload; the date and the time must exist on the calendar and the clock: `2026-02-30T10:00:00Z` and `2030-01-01T25:00:00Z` are refused.";

/// What the validators of a relayed approval say (`brief approve`, `policy approve`, `divergence accept`, `migrate`): the environment is
/// a stateless input (spec 11.7), so the role and the actor the variables fill are read here, not by a precondition.
pub const ROLE_DESCRIPTION: &str = "`--relayed-via` records a human's answer on behalf of an agent: a role is declared, with `--as` or the `CANON_ROLE` variable.";
pub const ACTOR_DESCRIPTION: &str = "A relayed approval names the agent that recorded it: `--actor`, else the `CANON_ACTOR` variable, is given and is one word.";
/// The tool of a relayed approval is one word, the same on the four commands that take `--relayed-via`.
pub const VIA_DESCRIPTION: &str = "`--relayed-via` names the tool the human was asked with as one word: no white space, parenthesis or quote (the `Approved:` line writes it as `via <tool>`).";
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
