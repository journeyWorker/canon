//! The L1 rules that read one registry document or one report, as functions of the document alone.
//!
//! The checks of a run call these on every document the run saw, and the tests of the contract's examples call them on a
//! file, so one implementation judges both (as [`super::rules`] does for L0). Each function returns one
//! `(condition, problem)` pair per condition judged, `None` when it holds.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::claim::Claim;
use crate::l1::model::CommandDoc;
use crate::l1::project;
use crate::schema::{schemas, Problem};

/// One condition judged: its id, and what is wrong when something is.
pub type Finding = (&'static str, Option<String>);

/// The twelve reserved codes with their exit (spec 7.2.1); every one is not retryable.
pub const RESERVED_CODES: [(&str, i64); 12] = [
    ("USAGE", 2),
    ("UNKNOWN_COMMAND", 2),
    ("COMMAND_REQUIRED", 2),
    ("INTERNAL", 1),
    ("UNVERIFIABLE", 1),
    ("INPUT_INVALID", 2),
    ("INPUT_CONFLICT", 2),
    ("INPUT_TOO_LARGE", 2),
    ("BATCH_INCOMPLETE", 1),
    ("PLAN_STALE", 2),
    ("OPERATION_REUSED", 2),
    ("CURSOR_INVALID", 2),
];
/// The four reserved codes whose exit `L0.ok-exit` already judges.
pub const L0_RESERVED: [&str; 4] = ["USAGE", "UNKNOWN_COMMAND", "COMMAND_REQUIRED", "INTERNAL"];

fn join(items: &[String]) -> String {
    items.join("; ")
}

fn code_of(entry: &Value) -> &str {
    entry.get("code").and_then(Value::as_str).unwrap_or_default()
}

/// The entries of `names` that occur more than once.
fn repeated<'a>(names: &[&'a str]) -> BTreeSet<&'a str> {
    names.iter().copied().filter(|name| names.iter().filter(|other| *other == name).count() > 1).collect()
}

/// `L1.registry`, for the entries of an `errors` answer: `codes-unique`, `reserved-exit`.
pub fn registry_findings(entries: &[Value]) -> Vec<Finding> {
    let codes: Vec<&str> = entries.iter().map(code_of).collect();
    let duplicated = repeated(&codes);
    let wrong: Vec<String> = entries
        .iter()
        .filter_map(|entry| {
            let code = code_of(entry);
            let (_, exit) = RESERVED_CODES.iter().find(|(name, _)| *name == code)?;
            let got_exit = entry.get("exit").and_then(Value::as_i64);
            let retryable = entry.get("retryable").and_then(Value::as_bool);
            let exit_wrong = !L0_RESERVED.contains(&code) && got_exit != Some(*exit);
            (exit_wrong || retryable != Some(false)).then(|| {
                format!(
                    "{code} is exit {} retryable {}, reserved is exit {exit} retryable false",
                    got_exit.map_or("?".to_string(), |exit| exit.to_string()),
                    retryable.map_or("?".to_string(), |flag| flag.to_string())
                )
            })
        })
        .collect();
    vec![
        (
            "codes-unique",
            (!duplicated.is_empty()).then(|| format!("the registry lists {} more than once", duplicated.iter().copied().collect::<Vec<_>>().join(", "))),
        ),
        ("reserved-exit", (!wrong.is_empty()).then(|| join(&wrong))),
    ]
}

/// `L1.reasons`, for the entries of an `errors` answer that list `reasons`: `reason-form`, `reasons-unique`, `no-exit-on-reason`, one set per entry.
pub fn reasons_findings(entries: &[Value]) -> Vec<Finding> {
    let mut findings: Vec<Finding> = Vec::new();
    for entry in entries {
        let code = code_of(entry);
        let Some(reasons) = entry.get("reasons").and_then(Value::as_array) else { continue };
        let names: Vec<&str> = reasons.iter().filter_map(|reason| reason.get("reason").and_then(Value::as_str)).collect();
        let malformed: Vec<&str> = names.iter().copied().filter(|name| !is_upper_snake(name)).collect();
        findings.push(("reason-form", (!malformed.is_empty()).then(|| format!("under {code} the reason {} is not UPPER_SNAKE_CASE", malformed.join(", ")))));
        let twice = repeated(&names);
        findings.push((
            "reasons-unique",
            (!twice.is_empty()).then(|| format!("{code} lists the reason {} more than once", twice.iter().copied().collect::<Vec<_>>().join(", "))),
        ));
        let with_exit: Vec<&str> = reasons
            .iter()
            .filter(|reason| reason.get("exit").is_some() || reason.get("retryable").is_some())
            .filter_map(|reason| reason.get("reason").and_then(Value::as_str))
            .collect();
        findings.push((
            "no-exit-on-reason",
            (!with_exit.is_empty()).then(|| format!("under {code} the reason {} has its own exit or retryable: a reason shares its code's", with_exit.join(", "))),
        ));
    }
    findings
}

/// `L1.warnings`, for the entries of a `warnings` answer: `codes-unique`.
pub fn warnings_findings(entries: &[Value]) -> Vec<Finding> {
    let codes: Vec<&str> = entries.iter().map(code_of).collect();
    let twice = repeated(&codes);
    vec![(
        "codes-unique",
        (!twice.is_empty()).then(|| format!("the warning registry lists {} more than once", twice.iter().copied().collect::<Vec<_>>().join(", "))),
    )]
}

/// `UPPER_SNAKE_CASE` as the registry writes reasons and codes.
pub fn is_upper_snake(name: &str) -> bool {
    !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.split('_').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
}

/// `L1.output-data`, for one success envelope of `doc`: `data-validates`, `schema-version`. Empty when the envelope names no output
/// of the command, carries `meta.fields` (`L1.fields` judges a projection) or belongs to a stream (`L2.stream`).
pub fn output_findings(doc: &CommandDoc, document: &Value) -> Vec<Finding> {
    if document.pointer("/meta/fields").is_some() {
        return Vec::new();
    }
    let name = document.pointer("/meta/output").and_then(Value::as_str);
    let Some(output) = doc.output(name) else {
        return if name.is_none() && doc.outputs.len() > 1 {
            vec![("data-validates", Some(format!("{}: meta.output is missing but the command has {} outputs", doc.command, doc.outputs.len())))]
        } else if name.is_some() {
            vec![("data-validates", Some(format!("{}: meta.output {} names no output of the command", doc.command, name.unwrap_or_default())))]
        } else {
            Vec::new()
        };
    };
    if output.kind == "stream" {
        return Vec::new();
    }
    let Ok(validator) = doc.compile_ref(&output.schema_ref) else { return Vec::new() };
    let data = document.get("data").unwrap_or(&Value::Null);
    let problems: Vec<String> = validator.iter_errors(data).map(|error| format!("{error} at {}", error.instance_path())).collect();
    let version = document.pointer("/meta/schemaVersion").and_then(Value::as_str);
    vec![
        (
            "data-validates",
            (!problems.is_empty()).then(|| format!("{}: data does not validate against {}: {}", doc.command, output.schema_ref, join(&problems[..problems.len().min(3)]))),
        ),
        (
            "schema-version",
            (version != Some(output.schema_version.as_str())).then(|| {
                format!("{}: meta.schemaVersion is {}, the output {} is {}", doc.command, version.unwrap_or("missing"), output.name(), output.schema_version)
            }),
        ),
    ]
}

/// `L1.fields`, for one projected success envelope of `doc`, from its `meta.fields` alone: `no-unselected` (no member of the records that
/// `meta.fields` does not cover), `projected-schema` (the data validates with `required` reduced along the kept paths). When
/// `unprojected` is the same invocation's answer without `--fields`, `selected-present` too. `kept` is the selection plus the output's `keep`.
pub fn fields_findings(doc: &CommandDoc, document: &Value, kept: &[String], unprojected: Option<&Value>) -> Vec<Finding> {
    let name = document.pointer("/meta/output").and_then(Value::as_str);
    let Some(output) = doc.output(name).filter(|output| output.fields.is_some()).or_else(|| doc.outputs.iter().find(|output| output.fields.is_some())) else {
        return Vec::new();
    };
    let data = document.get("data").unwrap_or(&Value::Null);
    let leaks = project::leaks(data, output, kept);
    let mut findings: Vec<Finding> = vec![(
        "no-unselected",
        (!leaks.is_empty()).then(|| format!("{}: members nothing selected: {}", doc.command, join(&leaks[..leaks.len().min(3)]))),
    )];
    if let Some(plain) = unprojected {
        let dropped = project::dropped(plain.get("data").unwrap_or(&Value::Null), data, output, kept);
        findings.push((
            "selected-present",
            (!dropped.is_empty()).then(|| format!("{}: selected members the unprojected answer has are missing: {}", doc.command, join(&dropped[..dropped.len().min(3)]))),
        ));
    }
    if let Ok(validator) = project::projected_validator(doc, output, kept) {
        let problems: Vec<String> = validator.iter_errors(data).map(|error| format!("{error} at {}", error.instance_path())).collect();
        findings.push((
            "projected-schema",
            (!problems.is_empty()).then(|| format!("{}: data does not validate against the projected schema: {}", doc.command, join(&problems[..problems.len().min(3)]))),
        ));
    }
    findings
}


/// The problems of a `schema` answer against `schema-doc.schema.json`, but for what another check owns: the path words and
/// option names (`L1.grammar`), a `null` in `meta` or `error` (`L0.no-null`) and members of a later minor (`L0.contract-version`).
pub fn schema_doc_problems(document: &Value, claim: Claim) -> Vec<String> {
    let prepared = crate::claim::for_envelope_schema(document, claim);
    schemas()
        .schema_doc_problem_list(&prepared)
        .into_iter()
        .filter(|problem| !owned_by_grammar(problem))
        .map(|problem| problem.to_string())
        .collect()
}

/// Whether a schema-document problem is about a word or a name that `L1.grammar` owns: a path word or an option or operand name that is not lower kebab-case, or an option ending in `-id`.
fn owned_by_grammar(problem: &Problem) -> bool {
    let path = problem.path.as_str();
    let word_position = path == "/data/command"
        || path.starts_with("/data/path/")
        || (path.starts_with("/data/commands/") && (path.ends_with("/command") || path.contains("/path/")))
        || ((path.starts_with("/data/options/") || path.starts_with("/data/operands/")) && path.ends_with("/name"));
    word_position && (problem.keyword == "pattern" || problem.keyword == "not")
}

/// The option names with one reserved shape (spec 9.8, 10.13 rule 8): name and value type.
const RESERVED_SHAPES: [(&str, &str); 9] = [
    ("json", "switch"),
    ("commit", "switch"),
    ("all", "switch"),
    ("input", "string"),
    ("plan-digest", "string"),
    ("cursor", "string"),
    ("fields", "string"),
    ("operation", "string"),
    ("limit", "integer"),
];
/// The reserved options that are always flag-only (spec 10.7.1).
const FLAG_ONLY: [&str; 4] = ["json", "input", "commit", "plan-digest"];

/// `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`.
pub fn is_kebab(word: &str) -> bool {
    let mut parts = word.split('-');
    let first_ok = parts
        .next()
        .is_some_and(|part| part.starts_with(|c: char| c.is_ascii_lowercase()) && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    first_ok && parts.all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

/// `L1.grammar`, for the entries of a tree (their paths) and one command document: `kebab-words`, `no-id-suffix`, `reserved-shape`.
pub fn grammar_findings(entry_paths: &[(&str, &[String])], doc: Option<&CommandDoc>) -> Vec<Finding> {
    let mut bad_words: Vec<String> = Vec::new();
    for (command, path) in entry_paths {
        bad_words.extend(path.iter().filter(|word| !is_kebab(word)).map(|word| format!("the path word {word:?} of {command}")));
    }
    let mut no_id: Vec<String> = Vec::new();
    let mut shapes: Vec<String> = Vec::new();
    if let Some(doc) = doc {
        bad_words.extend(doc.path.iter().filter(|word| !is_kebab(word)).map(|word| format!("the path word {word:?} of {}", doc.command)));
        for arg in doc.args() {
            if !is_kebab(&arg.name) {
                bad_words.push(format!("the name {:?} of {} in {}", arg.name, arg.spelling(), doc.command));
            }
        }
        for option in &doc.options {
            if option.name.ends_with("-id") {
                no_id.push(format!("{} has the option --{}", doc.command, option.name));
            }
            if let Some((_, shape)) = RESERVED_SHAPES.iter().find(|(name, _)| *name == option.name) {
                if option.value_type != *shape {
                    shapes.push(format!("{}: --{} is {}, reserved as {shape}", doc.command, option.name, option.value_type));
                }
            }
            if FLAG_ONLY.contains(&option.name.as_str()) && !option.flag_only {
                shapes.push(format!("{}: --{} is not flagOnly", doc.command, option.name));
            }
        }
    }
    bad_words.sort();
    bad_words.dedup();
    vec![
        ("kebab-words", (!bad_words.is_empty()).then(|| format!("not lower kebab-case: {}", join(&bad_words[..bad_words.len().min(3)])))),
        ("no-id-suffix", (!no_id.is_empty()).then(|| format!("an option name ends in -id (spec 9.5): {}", join(&no_id[..no_id.len().min(3)])))),
        ("reserved-shape", (!shapes.is_empty()).then(|| format!("a reserved option has not its reserved shape (spec 9.8): {}", join(&shapes[..shapes.len().min(3)])))),
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{registry_findings, reasons_findings, warnings_findings};

    fn problems(findings: Vec<super::Finding>) -> Vec<&'static str> {
        findings.into_iter().filter(|(_, problem)| problem.is_some()).map(|(condition, _)| condition).collect()
    }

    #[test]
    fn atip_checkl1_documents_registry_reasons_and_warnings_name_their_conditions() {
        let good = vec![json!({"code": "USAGE", "exit": 2, "retryable": false, "reasons": [{"reason": "A_B"}]})];
        assert!(problems(registry_findings(&good)).is_empty() && problems(reasons_findings(&good)).is_empty());
        let twice = vec![json!({"code": "X"}), json!({"code": "X"})];
        assert_eq!(problems(registry_findings(&twice)), ["codes-unique"]);
        assert_eq!(problems(warnings_findings(&twice)), ["codes-unique"]);
        let wrong = vec![json!({"code": "CURSOR_INVALID", "exit": 1, "retryable": false})];
        assert_eq!(problems(registry_findings(&wrong)), ["reserved-exit"]);
        let reasons = vec![json!({"code": "C", "reasons": [{"reason": "a"}, {"reason": "a", "exit": 2}]})];
        assert_eq!(problems(reasons_findings(&reasons)), ["reason-form", "reasons-unique", "no-exit-on-reason"]);
    }
}
