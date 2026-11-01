//! The stateless stage of step 6 (ATIP 11.7): the domains and the constraints a command document declares, enforced from the
//! declarations themselves, after parsing and before any precondition or state read (`CONFIG_MISSING`, `OLD_LAYOUT`, a file, a
//! ledger), in the contract's order: required (clap) → domains → constraints, in the order the document lists them → validators
//! (`validators::check`, which runs after this stage).
//!
//! The document is the enforcement: a relation or a domain the tables declare is checked here, so it cannot be declared and not
//! enforced, and cannot answer differently with and without a repository. Only what was GIVEN counts (a default or an environment
//! value does not, spec 10.8.1). clap enforces its own relations and value parsers at parse time; this stage covers the rest, and the
//! relations clap skips when the argument a `requires` names conflicts with one present (`migrate --backup F --relayed-via ask`).
//! The `field` of a violation is the first argument of the rule at fault in the rule's own order, an operand by its value name.

use atip::constraint::{violations, Given};
use atip::doc::{ConstraintDoc, RuleKind};
use clap::parser::ValueSource;
use clap::{Arg, ArgMatches, Command};
use serde_json::{json, Value};

use super::{domain_for, rules_for};
use crate::cli_error::{CliError, CliErrorExt as _};
use atip::domain::Domain;

/// The refusal of the first declared domain, then the first declared constraint, that the invocation breaks, with the label of its
/// command; `None` when it breaks none.
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
    let given_values = given_of(leaf, &args);
    // Domains, in the document's order (operands, then options).
    for arg in &args {
        let name = atip::constraint::name_of(arg);
        let Some(values) = given_values.get(&name) else { continue };
        let Some(row) = domain_for(&path, &name, Some(&atip::project::value_name(arg))) else { continue };
        let domain = row.domain.to_value();
        for (value, text) in values {
            let comparable = matches!((&row.domain, value), (Domain::Integer { .. }, Value::Number(_)) | (Domain::Text { .. }, Value::String(_)));
            if comparable && !atip::domain::admits(&domain, value).unwrap_or(true) {
                let field = field_of(&args, &name);
                let says = if row.says.is_empty() { says_for(&row.domain) } else { row.says.to_string() };
                let error = CliError::refused(row.code, format!("{field} {says}")).hint(format!("`canon schema {path} --json` lists the domain of each argument")).field(field).received(text.clone());
                let error = if row.reason.is_empty() { error } else { error.reason(row.reason) };
                return Some((label, error));
            }
        }
    }
    // Constraints, in the order the document lists them: clap's relations, then the tables'.
    let mut rules = atip::constraint::clap_rules(command);
    let mut tables: Vec<Option<atip::constraint::Rule>> = vec![None; rules.len()];
    for table in rules_for(&path) {
        rules.push(table.doc());
        tables.push(Some(table));
    }
    let given: Given = given_values.iter().map(|(name, values)| (name.clone(), values.iter().map(|(value, _)| value.clone()).collect())).collect();
    if let Some((at, fault)) = violations(&rules, &given).into_iter().next() {
        let rule = &rules[at];
        let table = tables[at];
        let code = table.and_then(|table| table.code).unwrap_or("USAGE");
        let message = table.and_then(|table| table.says).map_or_else(|| message_of(&args, rule, &given), str::to_string);
        let value = rule.when.as_ref().and_then(|when| given_values.get(&when.name)).and_then(|values| values.first()).map(|(_, text)| text.clone()).unwrap_or_default();
        let hint = table.and_then(|table| table.hint).map_or_else(|| rule.help.clone().map_or_else(|| format!("`canon schema {path} --json` lists the constraints of the command"), |help| format!("{help} `canon schema {path} --json` lists the constraints of the command")), |hint| hint.replace("{value}", &value));
        let mut error = CliError::refused(code, message).hint(hint).field(field_of(&args, &fault));
        if let Some(reason) = table.and_then(|table| table.reason) {
            error = error.reason(reason);
        }
        return Some((label, error));
    }
    None
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

/// The arguments the invocation gave explicitly: name to its values, as a document normalises them (an integer is a number, a switch is
/// `true`) with the text that was typed.
fn given_of(matches: &ArgMatches, args: &[&Arg]) -> std::collections::BTreeMap<String, Vec<(Value, String)>> {
    let mut given = std::collections::BTreeMap::new();
    for arg in args {
        let id = arg.get_id().as_str();
        if matches.try_contains_id(id).ok() != Some(true) || matches.value_source(id) != Some(ValueSource::CommandLine) {
            continue;
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
pub(super) fn field_of(args: &[&Arg], name: &str) -> String {
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
pub(super) fn integer_says(minimum: Option<i64>, maximum: Option<i64>) -> String {
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => format!("must be an integer from {minimum} to {maximum}"),
        (Some(minimum), None) => format!("must be an integer of at least {minimum}"),
        (None, Some(maximum)) => format!("must be an integer of at most {maximum}"),
        (None, None) => "must be an integer".to_string(),
    }
}

/// The sentence the declared domain of the integer option `field` of `node` (command `path`) gives, in place of clap's own range sentence
/// (`-1 is not in 0..=4294967295`, which reads as if 0 were declared valid): the tighter of the Rust type's bounds and the table's.
pub fn declared_integer_says(node: &Command, path: &str, field: &str) -> Option<String> {
    let name = field.strip_prefix("--")?;
    let arg = node.get_arguments().find(|arg| arg.get_long() == Some(name))?;
    let (mut minimum, mut maximum) = atip::project::implicit_bounds(arg)?;
    if let Some(Domain::Integer { minimum: lower, maximum: upper }) = domain_for(path, name, Some(&atip::project::value_name(arg))).map(|row| row.domain) {
        minimum = minimum.max(lower).or(minimum).or(lower);
        maximum = match (maximum, upper) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }
    Some(integer_says(minimum, maximum))
}
