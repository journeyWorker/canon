//! The constraints, domains and validators `canon schema` declares for the arguments of every command, held to the
//! real parser and handlers (0.34 `canon-0-34-atip-constraints`, atip/1 spec 10.7.4, 10.7.5, 10.8). Tests are named by scenario
//! id (`atip.constraints.NN`).
//!
//! The declarations are the documents the generator builds ([`canon_cli::spec::commands`]), read back as data. Every sweep
//! is over those documents and runs its calls through the built binary in a copy of the story repo the examples run in:
//!
//! - a rule: a call that breaks it and no other rule is refused with the rule's code, exit 2, a `field` that is one of its
//!   arguments, and writes nothing; a call that meets it while exercising it is not refused for that reason;
//! - a domain: a value just inside it is not refused for it, one just outside is, with the code the table names;
//! - a validator: every `rejects` call is refused with the validator's code, exit 2, and none is one of the examples.
//!
//! What no declaration covers is found the other way: the survey of the sources lists every place a handler refuses an argument and
//! fails when one is not classified.

#[path = "support/hermetic.rs"]
mod hermetic;
mod harness;
mod rule_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

use atip::doc::RuleKind;
use rule_support::{code_of, constraints, declared_role, docs, label, provision, solve, words_of, Goal, Outcome};
use serde_json::{json, Value};
use tempfile::TempDir;

/// The story repo (`tests/harness::fixture`) and the home the calls run with, built once.
struct Story {
    repo: TempDir,
    home: TempDir,
}

static STORY: LazyLock<Story> = LazyLock::new(|| {
    let home = TempDir::new().expect("a home");
    let repo = harness::fixture(home.path());
    Story { repo, home }
});

fn base() -> &'static Path {
    STORY.repo.path()
}

/// The provisioned repo of a command, built once per command.
fn provisioned(path: &str) -> Arc<rule_support::Provision> {
    static CACHE: LazyLock<Mutex<BTreeMap<String, Arc<rule_support::Provision>>>> = LazyLock::new(|| Mutex::new(BTreeMap::new()));
    let mut cache = CACHE.lock().expect("the cache");
    Arc::clone(cache.entry(path.to_string()).or_insert_with(|| Arc::new(provision(path, base(), home()))))
}

fn home() -> &'static Path {
    STORY.home.path()
}

/// The names a `field` can have for an argument of a rule: `--name`, and for an operand its value name too.
fn field_names(doc: &Value, rule: &atip::doc::ConstraintDoc) -> Vec<String> {
    let mut names = Vec::new();
    for member in atip::constraint::members(rule) {
        names.push(format!("--{member}"));
        names.push(member.clone());
        if let Some(operand) = doc["operands"].as_array().into_iter().flatten().find(|operand| operand["name"] == member.as_str()) {
            let value_name = operand["valueName"].as_str().unwrap_or_default();
            names.push(value_name.to_string());
            names.push(format!("<{value_name}>"));
        }
    }
    names
}

/// Whether the outcome is the refusal of `rule`: exit 2, its code, and a `field` of one of its arguments (or none).
fn refuses_for(outcome: &Outcome, doc: &Value, rule: &atip::doc::ConstraintDoc) -> bool {
    !outcome.ok && outcome.exit == 2 && outcome.code.as_deref() == Some(code_of(rule).as_str()) && outcome.field.as_ref().is_none_or(|field| field_names(doc, rule).contains(field))
}

/// The name of the argument a `field` stands for: `--name` is `name`, an operand's value name is the operand.
fn field_argument(doc: &Value, field: &str) -> Option<String> {
    if let Some(name) = field.strip_prefix("--") {
        return Some(name.to_string());
    }
    doc["operands"].as_array().into_iter().flatten().find(|operand| operand["valueName"] == field || format!("<{}>", operand["valueName"].as_str().unwrap_or_default()) == field).and_then(|operand| operand["name"].as_str().map(str::to_string))
}

/// Whether a call that MEETS `rule` was refused for it: the refusal has the rule's code and names an argument of the rule that the
/// call did not give (a violation names the argument at fault, spec 10.8.1). A refusal downstream that reuses the code and names
/// an argument the call did give (a file that does not exist, a state) is another check's.
fn refuses_for_unmet(outcome: &Outcome, doc: &Value, rule: &atip::doc::ConstraintDoc, words: &[String]) -> bool {
    if !refuses_for(outcome, doc, rule) {
        return false;
    }
    let Some(field) = outcome.field.as_deref().and_then(|field| field_argument(doc, field)) else { return false };
    !rule_support::parse_call(doc, words).is_ok_and(|parsed| parsed.given.contains_key(&field))
}

/// Runs `jobs` on a handful of threads and returns the problems they report.
fn in_parallel<T: Sync>(jobs: &[T], work: impl Fn(&T) -> Vec<String> + Sync) -> Vec<String> {
    let next = Mutex::new(0usize);
    let problems = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| loop {
                let at = {
                    let mut next = next.lock().expect("the counter");
                    let at = *next;
                    *next += 1;
                    at
                };
                let Some(job) = jobs.get(at) else { return };
                let found = work(job);
                problems.lock().expect("the problems").extend(found);
            });
        }
    });
    let mut problems = problems.into_inner().expect("the problems");
    problems.sort();
    problems
}

/// Whether a command is in the sweep: `only` when given, else `ARG_RULES_COMMANDS` (comma-separated) when set, else every command.
fn selected(doc: &Value, only: &[&str]) -> bool {
    let command = doc["command"].as_str().unwrap_or_default();
    if !only.is_empty() {
        return only.contains(&command);
    }
    std::env::var("ARG_RULES_COMMANDS").map_or(true, |list| list.split(',').any(|name| name.trim() == command))
}

/// One rule of one command held to the binary: the problems it has (empty when it is true).
pub fn drive_rule(doc: &Value, at: usize) -> Vec<String> {
    let path = doc["command"].as_str().unwrap_or_default();
    let rules = constraints(doc);
    let rule = &rules[at];
    let provision = provisioned(path);
    let mut problems = Vec::new();
    let tag = format!("`{path}` {}", label(rule));
    match solve(doc, &provision.example, &rules, Goal::Violate(at)) {
        None => problems.push(format!("{tag}: no call breaks it")),
        Some(found) => {
            let words = words_of(doc, &provision.example, &found.assignment);
            let outcome = rule_support::run_empty(&words, declared_role(path, &found.assignment));
            if !found.also_breaks.is_empty() {
                problems.push(format!("{tag}: no call breaks only it (`{}` also breaks {:?})", words.join(" "), found.also_breaks.iter().map(|other| label(&rules[*other])).collect::<Vec<_>>()));
            }
            if !refuses_for(&outcome, doc, rule) {
                problems.push(format!("{tag}: `{}` was not refused for it: exit {} ok {} code {:?} field {:?} message {:?}", words.join(" "), outcome.exit, outcome.ok, outcome.code, outcome.field, outcome.message));
            } else if outcome.field.is_none() && rule.code.is_some() {
                problems.push(format!("{tag}: `{}` was refused with no `field` (it names the argument at fault)", words.join(" ")));
            }
            if !outcome.changed.is_empty() {
                problems.push(format!("{tag}: `{}` was refused and wrote {:?}", words.join(" "), outcome.changed));
            }
        }
    }
    // A call that meets it is not refused for it.
    match solve(doc, &provision.example, &rules, Goal::Satisfy(at)) {
        None => {
            if !matches!(rule.rule, RuleKind::AtLeastOne | RuleKind::ExactlyOne | RuleKind::AtMostOne | RuleKind::Requires | RuleKind::Forbids) {
                problems.push(format!("{tag}: no call meets it"));
            } else {
                problems.push(format!("{tag}: no call meets it and exercises it"));
            }
        }
        Some(found) => {
            let words = words_of(doc, &provision.example, &found.assignment);
            let outcome = rule_support::run_empty(&words, declared_role(path, &found.assignment));
            if refuses_for_unmet(&outcome, doc, rule, &words) {
                problems.push(format!("{tag}: `{}` meets it and was refused for it: field {:?} message {:?}", words.join(" "), outcome.field, outcome.message));
            }
        }
    }
    problems
}

/// Every rule of every command (or of the commands `only` names) driven through the binary.
pub fn drive_all_rules(only: &[&str]) -> Vec<String> {
    let jobs: Vec<(&'static Value, usize)> = docs().into_iter().filter(|doc| selected(doc, only)).flat_map(|doc| (0..constraints(doc).len()).map(move |at| (doc, at))).collect();
    in_parallel(&jobs, |(doc, at)| drive_rule(doc, *at))
}

/// atip.constraints.03: every rule a command document lists (clap's and the tables') is true of the binary.
#[test]
fn atip_constraints_03_every_declared_rule_is_refused_when_broken_and_not_when_met() {
    let problems = drive_all_rules(&[]);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

// ------------------------------------------------------------------------------------------------ domains

/// Whether the argument clap blames (its own words for it) is the argument `name`.
fn blames(blamed: &str, name: &str, value_name: &str) -> bool {
    blamed.contains(&format!("--{name}")) || (!value_name.is_empty() && blamed.contains(value_name)) || blamed == name
}

/// Values just inside and just outside the domain of an argument, as the command line writes them: `(text, inside)`.
fn boundaries(domain: &Value, row: Option<&canon_cli::arg_rules::DomainRow>) -> Vec<(String, bool, bool)> {
    // The third member: the bound is the row's own (the code of the row), not the Rust integer type's (clap's `USAGE`).
    let own = |keyword: &str| row.is_some_and(|row| row.domain.to_value().get(keyword) == domain.get(keyword));
    let mut cases = Vec::new();
    let number = |keyword: &str| domain.get(keyword).and_then(Value::as_i64);
    if let Some(minimum) = number("minimum") {
        cases.push((minimum.to_string(), true, own("minimum")));
        cases.push(((minimum - 1).to_string(), false, own("minimum")));
    }
    if let Some(maximum) = number("maximum") {
        cases.push((maximum.to_string(), true, own("maximum")));
        cases.push(((maximum + 1).to_string(), false, own("maximum")));
    }
    if let Some(min) = number("minLength") {
        cases.push(("a".repeat(usize::try_from(min).unwrap_or(0)), true, true));
        if min >= 1 {
            cases.push(("a".repeat(usize::try_from(min - 1).unwrap_or(0)), false, true));
        }
    }
    if let Some(max) = number("maxLength") {
        cases.push(("a".repeat(usize::try_from(max).unwrap_or(0)), true, true));
        cases.push(("a".repeat(usize::try_from(max + 1).unwrap_or(0)), false, true));
    }
    if let Some(row) = row {
        cases.extend(row.inside.iter().map(|value| ((*value).to_string(), true, true)));
        cases.extend(row.outside.iter().map(|value| ((*value).to_string(), false, true)));
    }
    cases
}

/// One argument's domain held to the parser: a value just inside is not refused for it, one just outside is, with the
/// code its row names. A clap-level domain (`USAGE`) is asked of clap in process; a handler's, of the binary.
pub fn drive_domain(doc: &Value, argument: &Value) -> Vec<String> {
    let path = doc["command"].as_str().unwrap_or_default();
    let name = argument["name"].as_str().unwrap_or_default();
    let value_name = argument["valueName"].as_str().unwrap_or_default();
    let Some(domain) = argument.get("domain") else { return Vec::new() };
    let row = canon_cli::arg_rules::domain_for(path, name, argument["valueName"].as_str());
    let code = row.map_or("USAGE", |row| row.code);
    let provision = provisioned(path);
    let rules = constraints(doc);
    let Ok(parsed) = rule_support::parse_call(doc, &provision.example) else { return vec![format!("`{path}`: the example does not parse")] };
    let base = rule_support::assignment_of(&parsed);
    let mut problems = Vec::new();
    for (value, inside, own_bound) in boundaries(domain, row) {
        let code = if own_bound { code } else { "USAGE" };
        let tag = format!("`{path}` --{name} {value:?} ({} {domain})", if inside { "inside" } else { "outside" });
        let mut assignment = base.clone();
        assignment.insert(name.to_string(), vec![value.clone()]);
        if let Err(problem) = rule_support::repair(doc, &provision.example, &rules, &mut assignment, name) {
            problems.push(format!("{tag}: {problem}"));
            continue;
        }
        let words = words_of(doc, &provision.example, &assignment);
        if code == "USAGE" {
            // clap's value parser first, in process; a value it accepts is then asked of the binary (a handler's own USAGE).
            match (inside, rule_support::clap_refusal(&words)) {
                (false, Some((blamed, message))) if !blames(&blamed, name, value_name) => {
                    problems.push(format!("{tag}: refused for something else: {message}"));
                    continue;
                }
                (true, Some((blamed, message))) if blames(&blamed, name, value_name) => {
                    problems.push(format!("{tag}: `{}` was refused for it: {message}", words.join(" ")));
                    continue;
                }
                (true, Some((_, message))) => {
                    problems.push(format!("{tag}: the call is refused for something else: {message}"));
                    continue;
                }
                (false, Some(_)) => continue,
                (_, None) => {}
            }
        }
        let outcome = rule_support::run_empty(&words, declared_role(path, &assignment));
        let names = [format!("--{name}"), name.to_string(), format!("<{value_name}>"), value_name.to_string()];
        let refused_for_it = !outcome.ok && outcome.exit == 2 && outcome.code.as_deref() == Some(code) && outcome.field.as_ref().is_some_and(|field| names.contains(field));
        match (inside, refused_for_it) {
            (true, true) => problems.push(format!("{tag}: `{}` was refused for it: {:?}", words.join(" "), outcome.message)),
            (false, false) => problems.push(format!("{tag}: `{}` was not refused with {code}: exit {} code {:?} field {:?} {:?}", words.join(" "), outcome.exit, outcome.code, outcome.field, outcome.message)),
            _ => {}
        }
        if !inside && !outcome.changed.is_empty() {
            problems.push(format!("{tag}: refused and wrote {:?}", outcome.changed));
        }
    }
    problems
}

/// Every domain of every command (or of the commands `only` names).
pub fn drive_all_domains(only: &[&str]) -> Vec<String> {
    let jobs: Vec<(&'static Value, &'static Value)> = docs()
        .into_iter()
        .filter(|doc| selected(doc, only))
        .flat_map(|doc| doc["options"].as_array().into_iter().flatten().chain(doc["operands"].as_array().into_iter().flatten()).filter(|argument| argument.get("domain").is_some()).map(move |argument| (doc, argument)))
        .collect();
    in_parallel(&jobs, |(doc, argument)| drive_domain(doc, argument))
}

/// atip.constraints.05: every domain a command document lists is true of the parser at its boundaries.
#[test]
fn atip_constraints_05_every_declared_domain_agrees_with_the_parser_just_inside_and_outside() {
    let problems = drive_all_domains(&[]);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

// ---------------------------------------------------------------------------------------------- validators

/// One validator held to the binary: each `rejects` call parses by the document, breaks no rule or domain, is not an
/// example, and is refused with the validator's code, exit 2, naming one of its arguments, writing nothing.
pub fn drive_validator(doc: &Value, validator: &Value) -> Vec<String> {
    let path = doc["command"].as_str().unwrap_or_default();
    let code = validator["code"].as_str().unwrap_or_default();
    let arguments: Vec<String> = validator["arguments"].as_array().into_iter().flatten().filter_map(|name| name.as_str().map(str::to_string)).collect();
    let examples: Vec<&str> = doc["examples"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let mut problems = Vec::new();
    for call in validator["rejects"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        let tag = format!("`{path}` validator of {arguments:?}: `{call}`");
        if examples.contains(&call) {
            problems.push(format!("{tag}: is also an example"));
        }
        let mut words = atip::split_argv(call);
        if words.first().map(String::as_str) != Some("canon") {
            problems.push(format!("{tag}: does not start with `canon`"));
            continue;
        }
        words.remove(0);
        for problem in rule_support::document_problems(doc, &words) {
            problems.push(format!("{tag}: {problem}"));
        }
        words.retain(|word| word != "--json");
        // The call carries what it needs (`--as`, `--role`): the environment of a validator is empty.
        let outcome = rule_support::run_empty(&words, None);
        let names: Vec<String> = arguments.iter().flat_map(|name| [format!("--{name}"), name.clone()]).collect();
        let operand_fields: Vec<String> = arguments.iter().filter_map(|name| argument_value_name(doc, name)).flat_map(|value_name| [value_name.clone(), format!("<{value_name}>")]).collect();
        if outcome.ok || outcome.exit != 2 || outcome.code.as_deref() != Some(code) {
            problems.push(format!("{tag}: was not refused with {code}: exit {} code {:?} field {:?} {:?}", outcome.exit, outcome.code, outcome.field, outcome.message));
        } else if outcome.field.as_ref().is_none_or(|field| !names.contains(field) && !operand_fields.contains(field)) {
            problems.push(format!("{tag}: was refused with {code} but its field {:?} is none of {arguments:?}", outcome.field));
        }
        if !outcome.changed.is_empty() {
            problems.push(format!("{tag}: was refused and wrote {:?}", outcome.changed));
        }
    }
    problems
}

fn argument_value_name(doc: &Value, name: &str) -> Option<String> {
    rule_support::argument(doc, name).and_then(|argument| argument["valueName"].as_str().map(str::to_string))
}

/// Every validator of every command (or of the commands `only` names).
pub fn drive_all_validators(only: &[&str]) -> Vec<String> {
    let jobs: Vec<(&'static Value, &'static Value)> = docs().into_iter().filter(|doc| selected(doc, only)).flat_map(|doc| doc["validators"].as_array().into_iter().flatten().map(move |validator| (doc, validator))).collect();
    in_parallel(&jobs, |(doc, validator)| drive_validator(doc, validator))
}

/// atip.constraints.07: every validator's `rejects` is refused with its code.
#[test]
fn atip_constraints_07_every_validator_refuses_every_call_it_rejects() {
    let problems = drive_all_validators(&[]);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

// ------------------------------------------------------------------------------------------------ examples

/// An example satisfies `required`, every domain and every rule of its command, and parses by the real parser.
pub fn example_problems(doc: &Value) -> Vec<String> {
    let path = doc["command"].as_str().unwrap_or_default();
    let mut problems = Vec::new();
    for example in doc["examples"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        let mut words = atip::split_argv(example);
        words.remove(0);
        for problem in rule_support::document_problems(doc, &words) {
            problems.push(format!("`{path}` example `{example}`: {problem}"));
        }
        if let Some((blamed, message)) = rule_support::clap_refusal(&words) {
            problems.push(format!("`{path}` example `{example}`: the parser refuses it ({blamed}): {message}"));
        }
    }
    problems
}

/// atip.constraints.12: every example of every command document meets the constraints and the domains it declares.
#[test]
fn atip_constraints_12_every_example_meets_the_declared_constraints_and_domains() {
    let problems: Vec<String> = docs().into_iter().filter(|doc| selected(doc, &[])).flat_map(example_problems).collect();
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

// ------------------------------------------------------------------------------------------------ the survey

fn survey_problems(extra: &[(&str, &str)]) -> Vec<String> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = rule_support::sites::sites_of(&src, extra);
    rule_support::sites::problems(&found, &rule_support::sites::classified(), &docs())
}

/// atip.constraints.04: every place the sources refuse an argument is classified, and what is classified as declared is declared.
#[test]
fn atip_constraints_04_every_refusal_of_an_argument_in_the_sources_is_classified() {
    let problems = survey_problems(&[]);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

/// atip.constraints.04: a refusal added to a handler that no document declares and no table classifies fails the survey.
#[test]
fn atip_constraints_04_a_new_handler_check_that_no_document_declares_is_found() {
    let seeded = "fn run_add() {\n    let _ = CliError::refused(\"EVIDENCE_ARGUMENT_REQUIRED\", \"--a needs --b\");\n}\n";
    let problems = survey_problems(&[("evidence_seeded.rs", seeded)]);
    assert!(problems.iter().any(|problem| problem.contains("evidence_seeded.rs:run_add:EVIDENCE_ARGUMENT_REQUIRED#1")), "{problems:?}");
    // The same text in a test module is not a site.
    let in_tests = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn b() { let _ = \"EVIDENCE_ARGUMENT_REQUIRED\"; }\n}\n";
    assert!(!survey_problems(&[("evidence_seeded.rs", in_tests)]).iter().any(|problem| problem.contains("evidence_seeded.rs")), "a test module is not read");
}

// ------------------------------------------------------------------------------- the sweeps bite

/// A copy of the real document of `path` with a rule or a validator added, which the binary does not enforce.
fn seeded(path: &str, member: &str, extra: Value) -> Value {
    let mut doc = harness::leaf(path).clone();
    let list = doc.as_object_mut().expect("a document").entry(member).or_insert_with(|| Value::Array(Vec::new()));
    list.as_array_mut().expect("a list").push(extra);
    doc
}

/// atip.constraints.09: a validator whose `rejects` call the binary accepts, one refused with another code, and one that is also an
/// example are each found by the sweep.
#[test]
fn atip_constraints_09_a_validator_whose_rejects_is_not_refused_is_found() {
    let accepted = seeded("query", "validators", serde_json::json!({ "arguments": ["kind"], "code": "USAGE", "description": "Refuses nothing at all.", "rejects": ["canon query --kind change --json"] }));
    let problems = drive_validator(&accepted, accepted["validators"].as_array().and_then(|list| list.last()).expect("the seeded validator"));
    assert!(problems.iter().any(|problem| problem.contains("was not refused with USAGE")), "{problems:?}");
    let other_code = seeded("query", "validators", serde_json::json!({ "arguments": ["limit"], "code": "QUERY_ARGUMENT_INVALID", "description": "Claims another code.", "rejects": ["canon query --kind change --limit 0 --json"] }));
    let problems = drive_validator(&other_code, other_code["validators"].as_array().and_then(|list| list.last()).expect("the seeded validator"));
    assert!(problems.iter().any(|problem| problem.contains("was not refused with QUERY_ARGUMENT_INVALID")), "{problems:?}");
    let example = harness::leaf("query")["examples"][0].clone();
    let also = seeded("query", "validators", serde_json::json!({ "arguments": ["kind"], "code": "USAGE", "description": "Is an example.", "rejects": [example] }));
    let problems = drive_validator(&also, also["validators"].as_array().and_then(|list| list.last()).expect("the seeded validator"));
    assert!(problems.iter().any(|problem| problem.contains("is also an example")), "{problems:?}");
}

/// atip.constraints.13: a constraint declared with no check behind it, a code the binary does not use and a domain that is wrong
/// are each found by the sweeps (the handler's check removed makes its declaration's test fail).
#[test]
fn atip_constraints_13_a_declaration_with_no_check_behind_it_is_found() {
    let doc = seeded("query", "constraints", serde_json::json!({ "rule": "atMostOne", "names": ["kind", "since"] }));
    let at = doc["constraints"].as_array().map_or(0, Vec::len) - 1;
    let problems = drive_rule(&doc, at);
    assert!(problems.iter().any(|problem| problem.contains("was not refused for it")), "{problems:?}");
    let wrong_code = seeded("finding close", "constraints", serde_json::json!({ "rule": "requires", "if": { "name": "disposition", "in": ["rejected"] }, "then": ["reason"], "code": "USAGE" }));
    let at = wrong_code["constraints"].as_array().map_or(0, Vec::len) - 1;
    let problems = drive_rule(&wrong_code, at);
    assert!(!problems.is_empty(), "a rule whose code the binary does not give is found");
    let mut domain = harness::leaf("query").clone();
    let limit = domain["options"].as_array_mut().and_then(|options| options.iter_mut().find(|option| option["name"] == "limit")).expect("--limit");
    limit["domain"] = serde_json::json!({ "minimum": 5 });
    let argument = domain["options"].as_array().and_then(|options| options.iter().find(|option| option["name"] == "limit")).expect("--limit").clone();
    let problems = drive_domain(&domain, &argument);
    assert!(!problems.is_empty(), "a domain the parser does not hold is found: {problems:?}");
}

// ------------------------------------------------------------------------------- every subset of the arguments of the rules

/// A deterministic generator for the sampled subsets.
struct Xorshift(u64);

impl Xorshift {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % bound as u64).unwrap_or(0)
    }
}

/// The groups of names that share a rule: arguments no rule connects cannot change each other's answer.
fn components(rules: &[atip::doc::ConstraintDoc]) -> Vec<Vec<String>> {
    let mut groups: Vec<std::collections::BTreeSet<String>> = Vec::new();
    for rule in rules {
        let members: std::collections::BTreeSet<String> = atip::constraint::members(rule).into_iter().collect();
        let (touching, rest): (Vec<_>, Vec<_>) = groups.into_iter().partition(|group| !group.is_disjoint(&members));
        let mut merged = members;
        for group in touching {
            merged.extend(group);
        }
        groups = rest;
        groups.push(merged);
    }
    groups.into_iter().map(|group| group.into_iter().collect()).collect()
}

/// How many combinations of one group are enumerated before the rest are sampled (`ARG_RULES_SUBSETS` overrides it).
fn subset_budget() -> usize {
    std::env::var("ARG_RULES_SUBSETS").ok().and_then(|value| value.parse().ok()).unwrap_or(1500)
}

/// One command: every combination (all of them up to the budget, then a fixed-seed sample) of the states of the arguments each group of
/// its rules names (absent, or given with each value an `in` list needs and one it does not) is run in an empty directory. A call that
/// breaks a rule is refused with the code of a rule it breaks and a `field` that rule names; a call that breaks none is refused by none.
pub fn drive_subsets(doc: &Value) -> Vec<String> {
    let path = doc["command"].as_str().unwrap_or_default();
    let rules = constraints(doc);
    if rules.is_empty() {
        return Vec::new();
    }
    let example = provisioned(path).example.clone();
    let mut problems = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for names in components(&rules) {
        let options: Vec<Vec<Option<String>>> = names
            .iter()
            .map(|name| {
                let mut states = vec![None];
                states.extend(rule_support::candidates(doc, &example, &rules, name).into_iter().map(Some));
                states
            })
            .collect();
        let total: usize = options.iter().map(Vec::len).try_fold(1usize, usize::checked_mul).unwrap_or(usize::MAX);
        let budget = subset_budget();
        let mut generator = Xorshift(0x9E37_79B9_7F4A_7C15 ^ path.len() as u64 ^ (names.len() as u64) << 32);
        let picks: Vec<Vec<usize>> = if total <= budget {
            (0..total).map(|mut n| options.iter().map(|states| { let at = n % states.len(); n /= states.len(); at }).collect()).collect()
        } else {
            (0..budget).map(|_| options.iter().map(|states| generator.below(states.len())).collect()).collect()
        };
        for pick in picks {
            let assignment: rule_support::Assignment = names.iter().zip(&pick).zip(&options).filter_map(|((name, at), states)| states[*at].clone().map(|value| (name.clone(), vec![value]))).collect();
            let words = words_of(doc, &example, &assignment);
            if !seen.insert(words.clone()) {
                continue;
            }
            let Ok(parsed) = rule_support::parse_call(doc, &words) else { continue };
            let broken = atip::constraint::violations(&rules, &parsed.given);
            let outcome = rule_support::run_empty(&words, declared_role(path, &parsed.text));
            let call = words.join(" ");
            let fields: Vec<String> = broken.iter().flat_map(|(at, field)| field_names_of(doc, &rules[*at], field)).collect();
            if broken.is_empty() {
                // No rule is broken: no refusal that is a rule's (its code, naming an argument of one of the rules, that the call left out).
                // (a validator's refusal, with its own code and naming an argument it reads, is not a rule's.)
                let validator_refusal = doc["validators"].as_array().into_iter().flatten().any(|validator| {
                    outcome.code.as_deref() == validator["code"].as_str() && outcome.field.as_deref().and_then(|field| field_argument(doc, field)).is_some_and(|field| validator["arguments"].as_array().is_some_and(|names| names.contains(&json!(field))))
                });
                let rule_refusal = !validator_refusal && rules.iter().any(|rule| refuses_for_unmet(&outcome, doc, rule, &words));
                if rule_refusal {
                    problems.push(format!("`{path}` `{call}` breaks no rule and was refused for one: code {:?} field {:?} {:?}", outcome.code, outcome.field, outcome.message));
                }
            } else {
                let codes: Vec<String> = broken.iter().map(|(at, _)| code_of(&rules[*at])).collect();
                let refused_right = !outcome.ok && outcome.exit == 2 && outcome.code.as_ref().is_some_and(|code| codes.contains(code)) && outcome.field.as_ref().is_some_and(|field| fields.contains(field));
                if !refused_right {
                    problems.push(format!("`{path}` `{call}` breaks {:?} and answered exit {} code {:?} field {:?} {:?}", broken.iter().map(|(at, _)| label(&rules[*at])).collect::<Vec<_>>(), outcome.exit, outcome.code, outcome.field, outcome.message));
                }
                if !outcome.changed.is_empty() {
                    problems.push(format!("`{path}` `{call}` was refused and wrote {:?}", outcome.changed));
                }
            }
        }
    }
    problems
}

/// The spellings a `field` naming `argument` of `rule` may have.
fn field_names_of(doc: &Value, rule: &atip::doc::ConstraintDoc, argument: &str) -> Vec<String> {
    let mut all = vec![format!("--{argument}"), argument.to_string()];
    if let Some(operand) = doc["operands"].as_array().into_iter().flatten().find(|operand| operand["name"] == argument) {
        all.push(operand["valueName"].as_str().unwrap_or_default().to_string());
    }
    let _ = rule;
    all
}

/// atip.constraints.19: every combination of the arguments that the rules of a command name, run in an empty directory, is refused by a
/// rule it breaks (with its code and a field that rule names) or by none, whatever else is given with it.
#[test]
fn atip_constraints_19_every_combination_of_the_arguments_of_the_rules_is_answered_by_the_rules() {
    let jobs: Vec<&'static Value> = docs().into_iter().filter(|doc| selected(doc, &[])).collect();
    let problems = in_parallel(&jobs, |doc| drive_subsets(doc));
    assert!(problems.is_empty(), "{} problems, the first thirty:\n{}", problems.len(), problems.iter().take(30).cloned().collect::<Vec<_>>().join("\n"));
}
