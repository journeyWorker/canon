//! The stateless stage of step 6 (ATIP 11.7): the domains and the constraints a command document declares, enforced from the
//! declarations themselves, after parsing and before any precondition or state read (`CONFIG_REQUIRED`, `LAYOUT_MIGRATION_REQUIRED`, a file, a
//! ledger), in the contract's order: a failure to parse and `required` (the parser's, `relaxed::tree`) → domains → constraints, in
//! the order the document lists them → validators (`validators::check`, which runs after this stage).
//!
//! The document is the enforcement: a relation or a domain the tables declare is checked here, so it cannot be declared and not
//! enforced, and cannot answer differently with and without a repository. A domain judges the argument object (spec 11.7 step 6): the
//! value given, else the one the environment variable fills, else the default; a constraint counts only what was GIVEN (a default
//! or an environment value does not, spec 10.8.1). The `field` of a violation is the first argument of the rule at fault in the
//! rule's own order, an operand by its value name.

use atip::constraint::{violations, Given};
use atip::doc::{ConstraintDoc, RuleKind};
use clap::parser::ValueSource;
use clap::{Arg, ArgMatches, Command};
use serde_json::{json, Value};

use super::{domain_for, rules_of, DomainRow};
use crate::cli_error::{CliError, CliErrorExt as _};
use atip::domain::Domain;

/// The refusal of the first declared domain, then the first declared constraint, that the invocation breaks, with the label of its
/// command; `None` when it breaks none. `root` is the real command tree (its arguments carry the Rust types whose bounds are
/// domains); `matches` may come from the relaxed tree or from the real one: only the raw values are read.
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
    let path = words.join(" ");
    let label = format!("canon {path}");
    let args: Vec<&Arg> = command.get_positionals().chain(command.get_arguments().filter(|arg| !arg.is_positional())).filter(|arg| !atip::project::is_builtin_arg(arg)).collect();
    let argument_object = values_of(leaf, &args, false);
    // Domains, in the document's order (operands, then options): for one argument, what its Rust type allows, then its row.
    for arg in &args {
        let name = atip::constraint::name_of(arg);
        let Some(values) = argument_object.get(&name) else { continue };
        let row = domain_for(&path, &name, Some(&atip::project::value_name(arg)));
        for (domain, code, says) in domains_of(arg, row) {
            let compiled = domain.to_value();
            for (value, text) in values {
                let comparable = matches!((&domain, value), (Domain::Integer { .. }, Value::Number(_)) | (Domain::Text { .. }, Value::String(_)));
                if comparable && !atip::domain::admits(&compiled, value).unwrap_or(true) {
                    let field = field_of(&args, &name);
                    let error = CliError::refused(code, format!("{field} {says}")).hint(points_to(&path, "lists the domain of each argument")).field(field).received(text.clone());
                    let error = match row {
                        Some(row) if code == row.code && !row.reason.is_empty() => error.reason(row.reason),
                        _ => error,
                    };
                    return Some((label, error));
                }
            }
        }
    }
    // Constraints, in the order the document lists them.
    let rules = rules_of(command, &path);
    let docs: Vec<ConstraintDoc> = rules.iter().map(atip::constraint::Rule::doc).collect();
    let given_values = values_of(leaf, &args, true);
    let given: Given = given_values.iter().map(|(name, values)| (name.clone(), values.iter().map(|(value, _)| value.clone()).collect())).collect();
    if let Some((at, fault)) = violations(&docs, &given).into_iter().next() {
        let (rule, table) = (&docs[at], rules[at]);
        let code = table.code.unwrap_or("USAGE");
        let message = table.says.map_or_else(|| message_of(&args, rule, &given), str::to_string);
        let value = rule.when.as_ref().and_then(|when| given_values.get(&when.name)).and_then(|values| values.first()).map(|(_, text)| text.clone()).unwrap_or_default();
        let hint = table.hint.map_or_else(|| rule.help.clone().map_or_else(|| points_to(&path, "lists the constraints of the command"), |help| format!("{help} {}", points_to(&path, "lists the constraints of the command"))), |hint| hint.replace("{value}", &value));
        let mut error = CliError::refused(code, message).hint(hint).field(field_of(&args, &fault));
        // The closed set of the argument at fault, as clap's own refusal of the pair gave it (`--policy`: light, standard, strict).
        let closed: Vec<String> = args.iter().find(|arg| atip::constraint::name_of(arg) == fault).map(|arg| arg.get_possible_values().iter().filter(|value| !value.is_hide_set()).map(|value| value.get_name().to_string()).collect()).unwrap_or_default();
        if !closed.is_empty() {
            error = error.expected(closed);
        }
        if let Some(reason) = table.reason {
            error = error.reason(reason);
        }
        return Some((label, error));
    }
    None
}

/// What a refusal of `path` points to: a working call, when the command has an example, and the command's document.
fn points_to(path: &str, lists: &str) -> String {
    match crate::schema::example_for(path) {
        Some(example) => format!("a working call: {example}; `canon schema {path} --json` {lists}"),
        None => format!("`canon schema {path} --json` {lists}"),
    }
}

/// The domains `arg` is held to, in the order they are judged, each with the code a value outside it is refused with and what the refusal
/// says. An integer first has what its Rust type allows (an unsigned parser refuses a negative value, a narrow one its overflow;
/// spec 10.7.4), with `USAGE`. A row whose code is `USAGE` is the parser's own minimum (`--limit`, one the Rust type does not carry): it is
/// judged with the type's bounds as one domain, the tighter of the two, and says them. A row with a code of its own is judged after, with
/// that code. An argument that has no domain has none.
fn domains_of(arg: &Arg, row: Option<&DomainRow>) -> Vec<(Domain, &'static str, String)> {
    let mut domains = Vec::new();
    let parser_row = row.is_none_or(|row| row.code == "USAGE");
    if let Some((minimum, maximum)) = atip::project::implicit_bounds(arg).filter(|(minimum, maximum)| minimum.is_some() || maximum.is_some()) {
        let (merged_minimum, merged_maximum) = merged_bounds(minimum, maximum, row);
        // An integer is within ±(2^53-1) (spec 10.7.5): that is the bound a refusal names when the type has none.
        let says = integer_says(merged_minimum, merged_maximum.or(Some(atip::domain::SAFE_INTEGER)));
        if parser_row {
            let says = row.filter(|row| !row.says.is_empty()).map_or(says, |row| row.says.to_string());
            domains.push((Domain::Integer { minimum: merged_minimum, maximum: merged_maximum }, "USAGE", says));
            return domains;
        }
        domains.push((Domain::Integer { minimum, maximum }, "USAGE", says));
    }
    if let Some(row) = row {
        let says = if row.says.is_empty() { says_for(&row.domain) } else { row.says.to_string() };
        domains.push((row.domain, row.code, says));
    }
    domains
}

/// The bounds of an integer argument: the tighter of its Rust type's and its row's.
fn merged_bounds(mut minimum: Option<i64>, mut maximum: Option<i64>, row: Option<&DomainRow>) -> (Option<i64>, Option<i64>) {
    if let Some(Domain::Integer { minimum: lower, maximum: upper }) = row.map(|row| row.domain) {
        minimum = minimum.max(lower).or(minimum).or(lower);
        maximum = match (maximum, upper) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }
    (minimum, maximum)
}

/// What a value outside a domain is, in words: from the pattern's shape (a slug, a non-blank text, one line), else the pattern.
fn says_for(domain: &Domain) -> String {
    if let Domain::Integer { minimum, maximum } = domain {
        return integer_says(*minimum, *maximum);
    }
    let Domain::Text { pattern: Some(pattern), .. } = domain else { return "is outside its domain".to_string() };
    let breaks = pattern.contains("[^\\n\\r");
    let blank = pattern.contains("[^ \\t");
    if *pattern == super::KEBAB || *pattern == super::PROJECT_ID {
        "must be a kebab-case slug: lowercase letters and digits, single hyphens between them".to_string()
    } else if breaks && blank {
        "must be one non-empty line: it is blank or carries a line separator".to_string()
    } else if breaks {
        "must be one line: it carries a line separator".to_string()
    } else if blank {
        "is empty or only white space".to_string()
    } else {
        format!("does not match its pattern {pattern}")
    }
}

/// The argument object of the invocation (spec 11.7 step 6): name to its values, as a document normalises them (an integer is a number,
/// a switch is `true`) with the text that was typed. `explicit_only` keeps what the command line gave, which is what a constraint counts
/// (spec 10.8.1); without it a value an environment variable fills and a default are there too, which is what a domain judges.
pub(crate) fn values_of(matches: &ArgMatches, args: &[&Arg], explicit_only: bool) -> std::collections::BTreeMap<String, Vec<(Value, String)>> {
    let mut given = std::collections::BTreeMap::new();
    for arg in args {
        let id = arg.get_id().as_str();
        if matches.try_contains_id(id).ok() != Some(true) {
            continue;
        }
        match matches.value_source(id) {
            Some(ValueSource::CommandLine) => {}
            Some(ValueSource::EnvVariable | ValueSource::DefaultValue) if !explicit_only => {}
            _ => continue,
        }
        let values: Vec<(Value, String)> = if arg.get_action().takes_values() {
            let integer = atip::project::is_integer_arg(arg);
            matches
                .try_get_raw(id)
                .ok()
                .flatten()
                .map(|raw| {
                    raw.map(|value| {
                        let text = value.to_string_lossy().into_owned();
                        let normal = if integer { text.trim_start_matches('+').parse::<i64>().map_or_else(|_| json!(text), |number| json!(number)) } else { json!(text) };
                        (normal, text)
                    })
                    .collect()
                })
                .unwrap_or_default()
        } else {
            vec![(json!(true), "true".to_string())]
        };
        given.insert(atip::constraint::name_of(arg), values);
    }
    given
}

/// The `field` of the argument `name` (spec 7.1.1): an option as written, an operand by its value name.
pub(crate) fn field_of(args: &[&Arg], name: &str) -> String {
    match args.iter().find(|arg| atip::constraint::name_of(arg) == name) {
        Some(arg) if arg.is_positional() => atip::project::value_name(arg),
        _ => format!("--{name}"),
    }
}

/// What a violated rule says, in the command's own argument spellings.
fn message_of(args: &[&Arg], rule: &ConstraintDoc, given: &Given) -> String {
    let list = |names: &[String]| names.iter().map(|name| format!("`{}`", field_of(args, name))).collect::<Vec<_>>().join(", ");
    let names = rule.names.clone().unwrap_or_default();
    let then = rule.then.clone().unwrap_or_default();
    let when = rule.when.as_ref().map(|when| {
        let argument = format!("`{}`", field_of(args, &when.name));
        match &when.values {
            Some(values) => format!("{argument} is {}", values.iter().map(|value| value.as_str().map_or_else(|| value.to_string(), str::to_string)).collect::<Vec<_>>().join(" or ")),
            None => format!("{argument} is given"),
        }
    });
    let several = names.iter().filter(|name| given.contains_key(*name)).count() > 1;
    match rule.rule {
        RuleKind::AtLeastOne => format!("one of {} is required, and none was given", list(&names)),
        RuleKind::ExactlyOne if several => format!("{} are mutually exclusive: exactly one is required", list(&names)),
        RuleKind::ExactlyOne => format!("exactly one of {} is required, and none was given", list(&names)),
        RuleKind::AtMostOne => format!("{} are exclusive: at most one may be given", list(&names)),
        RuleKind::Requires => format!("{}, which needs {}", when.unwrap_or_default(), list(&then)),
        RuleKind::Forbids => format!("{}, which cannot be given with {}", when.unwrap_or_default(), list(&then)),
    }
}

/// What an integer bound says: `must be an integer of at least 1`.
pub(crate) fn integer_says(minimum: Option<i64>, maximum: Option<i64>) -> String {
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => format!("must be an integer from {minimum} to {maximum}"),
        (Some(minimum), None) => format!("must be an integer of at least {minimum}"),
        (None, Some(maximum)) => format!("must be an integer of at most {maximum}"),
        (None, None) => "must be an integer".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A command with an option that has a default and an operand, to read the argument object of.
    fn probe() -> Command {
        Command::new("probe")
            .arg(Arg::new("name").value_name("SUBJECT_ID"))
            .arg(Arg::new("limit").long("limit").default_value("20").value_parser(clap::value_parser!(i64)))
            .arg(Arg::new("plain").long("plain").num_args(1))
    }

    /// atip.checkorder.16: a domain judges the argument object (the value given, else the environment's, else the default), a constraint
    /// only what was given (spec 11.7 step 6, 10.8.1).
    #[test]
    fn atip_checkorder_16_the_argument_object_has_the_default_and_a_constraint_sees_only_what_was_given() {
        let command = probe();
        let args: Vec<&Arg> = command.get_arguments().collect();
        let matches = command.clone().try_get_matches_from(["probe", "--plain", "x"]).expect("parses");
        let object = values_of(&matches, &args, false);
        assert_eq!(object.get("limit").map(|values| values[0].0.clone()), Some(json!(20)), "the default is in the argument object: {object:?}");
        assert_eq!(object.get("plain").map(|values| values[0].1.clone()), Some("x".to_string()));
        let given = values_of(&matches, &args, true);
        assert!(!given.contains_key("limit"), "a default is not given: {given:?}");
        assert!(given.contains_key("plain"));
        // What the line gives replaces the default, and is read as an integer.
        let matches = command.clone().try_get_matches_from(["probe", "--limit", "+7"]).expect("parses");
        assert_eq!(values_of(&matches, &args, false).get("limit").map(|values| values[0].0.clone()), Some(json!(7)));
    }

    /// atip.checkorder.16 (the failure path): a value outside an integer's bound is refused by the domain stage of the real tree and not by
    /// the parser; a value that is no integer is a failure to parse of the relaxed tree.
    #[test]
    fn atip_checkorder_16_an_integer_outside_its_bound_is_a_domain_and_no_integer_is_a_failure_to_parse() {
        let mut real = crate::cli::command();
        real.build();
        let relaxed = super::super::relaxed::tree(crate::cli::command());
        let parsed = relaxed.clone().try_get_matches_from(["canon", "query", "--kind", "change", "--limit", "0"]).expect("a limit of 0 parses as text");
        let (label, refusal) = check(&real, &parsed).expect("a limit of 0 is outside its domain");
        assert_eq!(label, "canon query");
        assert_eq!(refusal.field_name(), Some("--limit"));
        let negative = relaxed.clone().try_get_matches_from(["canon", "query", "--kind", "change", "--limit", "-3"]).expect("a negative limit parses as an integer");
        assert!(check(&real, &negative).is_some());
        for text in ["abc", "1.5", "", "9007199254740993"] {
            assert!(relaxed.clone().try_get_matches_from(["canon", "query", "--kind", "change", "--limit", text]).is_err(), "`{text}` is no integer of the safe range: a failure to parse");
        }
        // The real tree refuses what is outside the type itself.
        let refused = real.clone().try_get_matches_from(["canon", "query", "--kind", "change", "--limit", "0"]).expect_err("the real tree refuses a limit of 0 itself");
        assert_eq!(refused.kind(), clap::error::ErrorKind::ValueValidation);
    }
}
