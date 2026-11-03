//! The L1 rules that read one registry document or one report, as functions of the document alone.
//!
//! The checks of a run call these on every document the run saw, and the tests of the contract's examples call them on a
//! file, so one implementation judges both (as [`super::rules`] does for L0). Each function returns one
//! `(condition, problem)` pair per condition judged, `None` when it holds.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::claim::Claim;
use crate::l1::invocation;
use crate::l1::model::CommandDoc;
use crate::l1::regex;
use crate::l1::project;
use crate::schema::schemas;

/// One condition judged: its id, and what is wrong when something is.
pub type Finding = (&'static str, Option<String>);

/// The minor a claim stands for, as the `atip` crate names it.
pub fn minor_of(claim: Claim) -> atip::Minor {
    atip::Minor::parse(claim.as_str()).expect("a claim names a minor the crate knows")
}

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

/// `L1.registry`, for the entries of an `errors` answer of a document that declares `minor`: `codes-unique`, `reserved-exit`.
/// What a reserved code must be registered as is the `atip` crate's table, and the check is `atip::registration_problem`,
/// the one a tool's own tests call.
pub fn registry_findings(entries: &[Value], minor: atip::Minor) -> Vec<Finding> {
    let codes: Vec<&str> = entries.iter().map(code_of).collect();
    let duplicated = repeated(&codes);
    let wrong: Vec<String> = entries
        .iter()
        .filter_map(|entry| {
            let code = code_of(entry);
            let reserved = atip::reserved_code(code, minor)?;
            let retryable = entry.get("retryable").and_then(Value::as_bool);
            // `L0.ok-exit` already judges the exit of the four L0 codes, so the row is judged by its retryability alone.
            let exit = if L0_RESERVED.contains(&code) { Some(i64::from(reserved.exit)) } else { entry.get("exit").and_then(Value::as_i64) };
            atip::registration_problem(minor, code, exit, retryable)
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
    if document.get("ok") == Some(&Value::Bool(false)) {
        // A failure carries the version of the output the command would have printed (spec 6.11): the one output, or the one `meta.output` names.
        let name = document.pointer("/meta/output").and_then(Value::as_str);
        let Some(output) = doc.output(name) else { return Vec::new() };
        let version = document.pointer("/meta/schemaVersion").and_then(Value::as_str);
        return vec![(
            "schema-version",
            (version != Some(output.schema_version.as_str())).then(|| {
                format!("{}: meta.schemaVersion is {}, the output {} is {}", doc.command, version.unwrap_or("missing"), output.name(), output.schema_version)
            }),
        )];
    }
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
    // A path that is not declared (a key of a declared map only for a document that declares 1.1) is not one a selection may name (spec 6.10).
    let v11 = document.pointer("/meta/contractVersion").is_some();
    let undeclared: Vec<&String> = kept.iter().filter(|path| !project::selectable(doc, output, path, v11)).collect();
    let leaks = project::leaks(data, output, kept);
    let mut findings: Vec<Finding> = vec![(
        "selection-declared",
        (!undeclared.is_empty()).then(|| format!("{}: meta.fields names {}, which the output does not declare", doc.command, undeclared.iter().map(|path| path.as_str()).collect::<Vec<_>>().join(", "))),
    ), (
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
/// option names (`L1.grammar`: kebab-case, no `-id`, the reserved shapes), a `null` in `meta` or `error` (`L0.no-null`) and
/// members of a later minor (`L0.contract-version`). The document is validated with the words and shapes `L1.grammar` judges
/// replaced by ones that are right, so the schema reports only what is wrong besides them.
pub fn schema_doc_problems(document: &Value, claim: Claim) -> Vec<String> {
    let mut prepared = crate::claim::for_envelope_schema(document, claim);
    if let Some(data) = prepared.get_mut("data") {
        neutralise_grammar(data);
    }
    schemas()
        .schema_doc_problem_list(&prepared)
        .into_iter()
        .map(|problem| problem.to_string())
        .collect()
}

/// A kebab-case word standing for `word`: itself when it is one, else a fixed one.
fn kebab_for(word: &str, at: usize) -> String {
    let trimmed = word.strip_suffix("-id").unwrap_or(word);
    if is_kebab(trimmed) {
        trimmed.to_string()
    } else {
        format!("x{at}")
    }
}

fn neutralise_words(value: &mut Value) {
    if let Some(words) = value.as_array_mut() {
        for (at, word) in words.iter_mut().enumerate() {
            if let Some(text) = word.as_str() {
                *word = Value::String(kebab_for(text, at));
            }
        }
    }
}

fn plain_patterns(value: &mut Value) {
    match value {
        Value::Object(members) => {
            if members.get("pattern").is_some_and(Value::is_string) {
                members.insert("pattern".to_string(), Value::String("^x$".to_string()));
            }
            for member in members.values_mut() {
                plain_patterns(member);
            }
        }
        Value::Array(items) => {
            for item in items {
                plain_patterns(item);
            }
        }
        _ => {}
    }
}

/// `data` of a tree, group or command document with the words and shapes of `L1.grammar` made right (see [`schema_doc_problems`]).
fn neutralise_grammar(data: &mut Value) {
    for key in ["commands"] {
        if let Some(entries) = data.get_mut(key).and_then(Value::as_array_mut) {
            for entry in entries {
                if let Some(path) = entry.get_mut("path") {
                    neutralise_words(path);
                }
            }
        }
    }
    if let Some(path) = data.get_mut("path") {
        neutralise_words(path);
    }
    // Rule 5 (`input.batch` iff an output has `batch: true`) is `L1.walk`'s `rule-5`; the schema says it too, so the schema is shown the two in agreement.
    if data.get("input").is_some() {
        let batch_output = data.get("outputs").and_then(Value::as_array).is_some_and(|outputs| outputs.iter().any(|output| output.get("batch") == Some(&Value::Bool(true))));
        if let Some(input) = data.get_mut("input").and_then(Value::as_object_mut) {
            input.insert("batch".to_string(), Value::Bool(batch_output));
            // `maxItems` goes with `batch: true` and only with it (spec 10.10).
            if batch_output {
                input.entry("maxItems").or_insert(Value::from(1));
            } else {
                input.remove("maxItems");
            }
        }
    }
    for key in ["operands", "options"] {
        let Some(list) = data.get_mut(key).and_then(Value::as_array_mut) else { continue };
        for (at, arg) in list.iter_mut().enumerate() {
            let Some(object) = arg.as_object_mut() else { continue };
            // A domain that rule 3 reports (outside its valueType's profile, or not a JSON Schema that compiles) is `L1.walk`'s `rule-3-domain-*`: the schema is shown a plain one.
            let unusable = crate::l1::model::Arg::parse(&Value::Object(object.clone()), key == "options");
            if crate::l1::rules::domain_unusable(&unusable) {
                object.insert("domain".to_string(), serde_json::json!({"minLength": 0}));
            }
            // A `pattern` is `L1.patterns`' (its form, anchors and compile): the schema judges the document with a plain one.
            if let Some(domain) = object.get_mut("domain") {
                plain_patterns(domain);
            }
            let name = object.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
            if key == "options" {
                let fixed = kebab_for(&name, at);
                if let Some((_, shape)) = RESERVED_SHAPES.iter().find(|(reserved, _)| *reserved == fixed) {
                    object.insert("valueType".to_string(), Value::String((*shape).to_string()));
                }
                if FLAG_ONLY.contains(&fixed.as_str()) {
                    object.insert("flagOnly".to_string(), Value::Bool(true));
                }
                object.insert("name".to_string(), Value::String(fixed));
            } else {
                object.insert("name".to_string(), Value::String(kebab_for(&name, at)));
            }
        }
    }
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

/// `L1.grammar`, for the entries of a tree (their paths) and one command document: `kebab-words`, `no-id-suffix`, `reserved-shape`. For a 1.2 claim
/// `limit`, `cursor`, `all` and `fields` are flag-only too (spec 11.6).
pub fn grammar_findings(entry_paths: &[(&str, &[String])], doc: Option<&CommandDoc>, minor: atip::Minor) -> Vec<Finding> {
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
            if atip::flag_only_options(minor).contains(&option.name.as_str()) && !option.flag_only {
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


/// Every `pattern` of a domain schema, at the positions the profile allows (spec 10.7.4): `Err` with a description for one that is not a string.
fn patterns_of(domain: &Value, found: &mut Vec<Result<String, String>>) {
    let Some(object) = domain.as_object() else { return };
    match object.get("pattern") {
        Some(Value::String(pattern)) => found.push(Ok(pattern.clone())),
        Some(other) => found.push(Err(format!("pattern {other} is not a string"))),
        None => {}
    }
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for child in properties.values() {
            patterns_of(child, found);
        }
    }
    for key in ["items", "additionalProperties"] {
        if let Some(child) = object.get(key) {
            patterns_of(child, found);
        }
    }
}

/// The patterns of the domains of `doc`: the argument's spelling and the pattern, or what is wrong with a `pattern` that is not a string.
pub fn domain_patterns(doc: &CommandDoc) -> Vec<(String, Result<String, String>)> {
    let mut all = Vec::new();
    for arg in doc.args() {
        let Some(domain) = &arg.domain else { continue };
        let mut found = Vec::new();
        patterns_of(domain, &mut found);
        all.extend(found.into_iter().map(|pattern| (arg.spelling(), pattern)));
    }
    all
}

/// `L1.patterns`, for the patterns of one command document: `form` (the restricted I-Regexp form and an ECMAScript `u` compile) and
/// `rust-compiles` (the Rust `regex` crate at its default limits), one pair of each per pattern. The comparison of the two engines on a value
/// (`engines-agree`) needs values and is a run's.
pub fn pattern_findings(doc: &CommandDoc) -> Vec<Finding> {
    let mut findings: Vec<Finding> = Vec::new();
    for (spelling, pattern) in domain_patterns(doc) {
        let (form, rust) = match &pattern {
            Ok(pattern) => (regex::form_problem(pattern), regex::rust_problem(pattern)),
            Err(why) => (Some(why.clone()), None),
        };
        let shown = pattern.clone().unwrap_or_default();
        findings.push(("form", form.map(|why| format!("{}: the pattern {shown:?} of {spelling} {why}", doc.command))));
        findings.push(("rust-compiles", rust.map(|why| format!("{}: the pattern {shown:?} of {spelling}: {why}", doc.command))));
    }
    findings
}

/// `L1.examples`, for one command document: the nine conditions of `invocation::check_line` for each line of `examples`.
pub fn examples_findings(doc: &CommandDoc, tool: &str) -> Vec<Finding> {
    let mut findings: Vec<Finding> = Vec::new();
    for line in &doc.examples {
        for (condition, problem) in invocation::check_line(doc, tool, line, true) {
            findings.push((condition, problem.map(|why| format!("{}: example `{line}`: {why}", doc.command))));
        }
    }
    findings
}


/// `L1.reserved-applicable`, for one command document: the reserved codes of the features the command offers are in its `errors`.
pub fn reserved_applicable_findings(doc: &CommandDoc) -> Vec<Finding> {
    let lists = |codes: &[&str]| -> Vec<String> {
        codes.iter().filter(|code| !doc.errors.iter().any(|known| known == *code)).map(|code| (*code).to_string()).collect()
    };
    let features: [(&'static str, bool, &[&str]); 5] = [
        ("input-codes", doc.input.is_some(), &["INPUT_INVALID", "INPUT_CONFLICT", "INPUT_TOO_LARGE"]),
        ("batch-code", doc.input.as_ref().is_some_and(|input| input.batch), &["BATCH_INCOMPLETE"]),
        ("commit-code", doc.has_option("commit"), &["PLAN_STALE"]),
        ("operation-code", doc.has_option("operation"), &["OPERATION_REUSED"]),
        ("cursor-code", doc.has_option("cursor"), &["CURSOR_INVALID"]),
    ];
    features
        .into_iter()
        .map(|(condition, offered, codes)| {
            let missing = if offered { lists(codes) } else { Vec::new() };
            (
                condition,
                (!missing.is_empty()).then(|| format!("{} offers the feature of {} but does not list {} in errors", doc.command, codes.join(", "), missing.join(", "))),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use atip::Minor;

    use super::{registry_findings, reasons_findings, warnings_findings};

    fn problems(findings: Vec<super::Finding>) -> Vec<&'static str> {
        findings.into_iter().filter(|(_, problem)| problem.is_some()).map(|(condition, _)| condition).collect()
    }

    #[test]
    fn atip_checkl1_documents_registry_reasons_and_warnings_name_their_conditions() {
        let good = vec![json!({"code": "USAGE", "exit": 2, "retryable": false, "reasons": [{"reason": "A_B"}]})];
        assert!(problems(registry_findings(&good, Minor::V1_1)).is_empty() && problems(reasons_findings(&good)).is_empty());
        let twice = vec![json!({"code": "X"}), json!({"code": "X"})];
        assert_eq!(problems(registry_findings(&twice, Minor::V1_1)), ["codes-unique"]);
        assert_eq!(problems(warnings_findings(&twice)), ["codes-unique"]);
        let wrong = vec![json!({"code": "CURSOR_INVALID", "exit": 1, "retryable": false})];
        assert_eq!(problems(registry_findings(&wrong, Minor::V1_1)), ["reserved-exit"]);
        let reasons = vec![json!({"code": "C", "reasons": [{"reason": "a"}, {"reason": "a", "exit": 2}]})];
        assert_eq!(problems(reasons_findings(&reasons)), ["reason-form", "reasons-unique", "no-exit-on-reason"]);
    }
}
